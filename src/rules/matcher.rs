//! Rule matching over a snapshot (RULES.md section 4.1).
//!
//! One depth-first pass over directory nodes. Protected, inaccessible, symlink
//! and mount-point nodes stop the walk. A node matched by a rule becomes an item
//! and its subtree is consumed, so no path ever belongs to two rules.
//!
//! Glob semantics (pinned by tests): `*` never crosses `/`; `**` spans zero or
//! more whole segments, so `~/**/.next` also matches `~/.next`; a trailing `/**`
//! needs at least one more segment, so `**/node_modules/**` matches paths inside
//! a `node_modules` folder but not the folder itself.

use std::path::Path;

use globset::{GlobSet, GlobSetBuilder};

use super::guards::{self, Env};
use super::model::{Rule, Rules};
use super::{compile, expand};
use crate::cleanup::protected::Protected;
use crate::scanner::tree::Snapshot;

/// Rule `rule` (index into `Rules::rules`) matched directory node `node`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Match {
    pub rule: usize,
    pub node: u32,
}

pub struct Matcher {
    /// Every rule's `paths` globs in one set.
    paths: GlobSet,
    /// Per glob in `paths`: the rule it belongs to and its literal prefix length.
    owners: Vec<(usize, usize)>,
    /// Per rule: its `exclude` globs.
    excludes: Vec<GlobSet>,
    protected: Protected,
}

impl Matcher {
    /// Compiles the rules for a snapshot rooted at `root`.
    pub fn new(rules: &Rules, root: &str) -> Self {
        let mut paths = GlobSetBuilder::new();
        let mut owners = Vec::new();
        let mut excludes = Vec::new();
        for (i, rule) in rules.rules.iter().enumerate() {
            for p in &rule.paths {
                paths.add(compile(&expand(p, root)).expect("rule globs are validated"));
                owners.push((i, literal_prefix_len(p)));
            }
            let mut ex = GlobSetBuilder::new();
            for p in &rule.exclude {
                ex.add(compile(&expand(p, root)).expect("rule globs are validated"));
            }
            excludes.push(ex.build().expect("exclude set builds"));
        }
        let protect: Vec<String> = rules.protect.iter().map(|p| p.path.clone()).collect();
        Self {
            paths: paths.build().expect("path set builds"),
            owners,
            excludes,
            protected: Protected::new(root, &protect),
        }
    }

    /// Every match in the snapshot, in depth-first order.
    pub fn run(&self, rules: &Rules, snap: &Snapshot, env: &Env) -> Vec<Match> {
        let mut out = Vec::new();
        let mut hits = Vec::new();
        let root = Path::new(&snap.meta.root);
        let mut stack: Vec<(u32, String)> = snap.nodes[0]
            .children
            .iter()
            .rev()
            .map(|&c| (c, join(&snap.meta.root, &snap.nodes[c as usize].name)))
            .collect();
        while let Some((id, path)) = stack.pop() {
            let node = &snap.nodes[id as usize];
            let p = Path::new(&path);
            let f = node.flags;
            if f.inaccessible || f.symlink || f.mount || self.protected.is_protected_root(p) {
                continue;
            }
            if let Some(rule) = self.best(rules, p, root, env, &mut hits) {
                out.push(Match { rule, node: id });
                continue;
            }
            for &c in node.children.iter().rev() {
                stack.push((c, join(&path, &snap.nodes[c as usize].name)));
            }
        }
        out
    }

    /// The rule that claims `path` on disk now, chosen as `run` chooses it.
    /// Cleanup revalidation uses it to confirm an item still matches its rule.
    pub fn winner(&self, rules: &Rules, path: &Path, root: &Path, env: &Env) -> Option<usize> {
        self.best(rules, path, root, env, &mut Vec::new())
    }

    /// The winning rule for `path`: highest priority, then longest literal glob
    /// prefix, then the rule listed first.
    fn best(
        &self,
        rules: &Rules,
        path: &Path,
        root: &Path,
        env: &Env,
        hits: &mut Vec<usize>,
    ) -> Option<usize> {
        self.paths.matches_into(path, hits);
        let mut best: Option<(i32, usize, usize)> = None;
        for &glob in hits.iter() {
            let (rule, prefix) = self.owners[glob];
            let r = &rules.rules[rule];
            let better = match best {
                None => true,
                Some((pri, len, idx)) => {
                    (r.priority, prefix) > (pri, len)
                        || ((r.priority, prefix) == (pri, len) && rule < idx)
                }
            };
            // Cheap checks first: the guards may stat files or run git.
            if better && !self.excludes[rule].is_match(path) && match_time_ok(r, path, root, env) {
                best = Some((r.priority, prefix, rule));
            }
        }
        best.map(|(_, _, rule)| rule)
    }
}

/// `sibling_exists`, `requires_tool` (unless a fallback exists) and `project_idle_days`.
fn match_time_ok(rule: &Rule, path: &Path, root: &Path, env: &Env) -> bool {
    let g = &rule.guards;
    if !g.sibling_exists.is_empty() && !guards::sibling_exists(path, &g.sibling_exists) {
        return false;
    }
    if let Some(tool) = &g.requires_tool
        && rule.fallback_method.is_none()
        && env.tools.resolve(tool).is_none()
    {
        return false;
    }
    g.project_idle_days
        .is_none_or(|days| guards::project_idle(path, days, root, env))
}

/// Characters before the first glob metacharacter.
fn literal_prefix_len(pattern: &str) -> usize {
    pattern.find(['*', '?', '[', '{']).unwrap_or(pattern.len())
}

fn join(parent: &str, name: &str) -> String {
    let mut s = String::with_capacity(parent.len() + 1 + name.len());
    s.push_str(parent);
    if !parent.ends_with('/') {
        s.push('/');
    }
    s.push_str(name);
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(glob: &str, path: &str) -> bool {
        compile(glob).unwrap().compile_matcher().is_match(path)
    }

    #[test]
    fn double_star_spans_zero_or_more_segments() {
        assert!(m("/h/**/.next", "/h/.next"));
        assert!(m("/h/**/.next", "/h/a/b/.next"));
        assert!(!m("/h/**/.next", "/h/a/.nextx"));
        assert!(m("/h/**/node_modules", "/h/p/node_modules"));
    }

    #[test]
    fn trailing_double_star_needs_a_segment() {
        assert!(!m("**/node_modules/**", "/h/p/node_modules"));
        assert!(m("**/node_modules/**", "/h/p/node_modules/dep"));
        assert!(m(
            "**/node_modules/**",
            "/h/p/node_modules/dep/node_modules"
        ));
        assert!(!m("/h/Library/**", "/h/Library"));
        assert!(m("/h/Library/**", "/h/Library/Caches/x"));
    }

    #[test]
    fn single_star_stays_in_one_segment() {
        assert!(m("/h/Library/Caches/*", "/h/Library/Caches/com.x"));
        assert!(!m("/h/Library/Caches/*", "/h/Library/Caches/com.x/sub"));
        assert!(!m("/h/Library/Caches/*", "/h/Library/Caches"));
        assert!(m(
            "/h/Library/Caches/com.apple.*",
            "/h/Library/Caches/com.apple.Safari"
        ));
    }

    #[test]
    fn literal_prefix() {
        assert_eq!(literal_prefix_len("~/Library/Caches/Yarn"), 21);
        assert_eq!(literal_prefix_len("~/Library/Caches/*"), 17);
        assert_eq!(literal_prefix_len("~/**/.next"), 2);
    }

    #[test]
    fn join_paths() {
        assert_eq!(join("/", "a"), "/a");
        assert_eq!(join("/h", "a"), "/h/a");
    }
}
