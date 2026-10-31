//! Recommendation rows from rule matches and providers.
//!
//! `analyze` runs once per snapshot: matching, providers and the minimum-size
//! filter. Rows are built from that on every request, because blocking guards
//! depend on what is running right now.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::model::{ItemDetails, ItemStatus, MethodView, RecItem, Recommendation, Subfolder, Tier};
use crate::providers::{self, Found, ProviderItem};
use crate::rules::category::Categories;
use crate::rules::guards::{self, Env};
use crate::rules::matcher::Matcher;
use crate::rules::model::{FallbackMethod, Method, Rule, Rules, SplitBy};
use crate::scanner::tree::Snapshot;
use crate::view::dir_path;

/// `min_size_mb` uses decimal megabytes, like the sizes the UI shows.
const MB: u64 = 1_000_000;
const SUBFOLDERS: usize = 5;

/// One cleanable unit for the current snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    /// `"{rule_id}:{node_id}"` or `"{rule_id}:{provider_key}"`.
    pub id: String,
    /// Index into `Rules::rules`.
    pub rule: usize,
    pub name: String,
    pub path: PathBuf,
    /// The folder the rule matched; a provider's items live inside it.
    pub matched: PathBuf,
    pub size: u64,
    pub node: Option<u32>,
    pub main_repo: Option<PathBuf>,
    pub project: Option<String>,
}

pub struct Analysis {
    pub snapshot_id: String,
    pub root: String,
    pub items: Vec<Item>,
    /// Snapshot nodes covered by an item -> rule index; their subtrees are hatched.
    marks: HashMap<u32, usize>,
    pub categories: Categories,
}

pub fn analyze(rules: &Rules, snap: &Snapshot, env: &Env) -> Analysis {
    let matcher = Matcher::new(rules, &snap.meta.root);
    let mut items = Vec::new();
    let mut marks = HashMap::new();
    for m in matcher.run(rules, snap, env) {
        let rule = &rules.rules[m.rule];
        let path = dir_path(snap, m.node);
        let found = match rule.provider {
            Some(p) => providers::items(
                p,
                &Found {
                    snap,
                    node: m.node,
                    path: &path,
                },
                env,
            ),
            None => vec![ProviderItem {
                key: m.node.to_string(),
                name: snap.nodes[m.node as usize].name.clone(),
                path: path.clone(),
                size: snap.nodes[m.node as usize].size,
                node: Some(m.node),
                main_repo: None,
                project: None,
            }],
        };
        for f in found {
            if f.size < rule.min_size_mb * MB {
                continue;
            }
            marks.insert(f.node.unwrap_or(m.node), m.rule);
            items.push(Item {
                id: format!("{}:{}", rule.id, f.key),
                rule: m.rule,
                name: f.name,
                path: f.path,
                matched: path.clone(),
                size: f.size,
                node: f.node,
                main_repo: f.main_repo,
                project: f.project,
            });
        }
    }
    Analysis {
        snapshot_id: snap.meta.id.clone(),
        root: snap.meta.root.clone(),
        items,
        marks,
        categories: Categories::new(rules, snap),
    }
}

impl Analysis {
    /// The rule whose item covers node `id` (the node or an ancestor), if any.
    pub fn rule_of(&self, snap: &Snapshot, id: u32) -> Option<usize> {
        let mut cur = Some(id);
        while let Some(n) = cur {
            if let Some(&r) = self.marks.get(&n) {
                return Some(r);
            }
            cur = snap.nodes[n as usize].parent;
        }
        None
    }

    /// The rule whose item is rooted exactly at node `id`.
    pub fn mark_rule(&self, id: u32) -> Option<usize> {
        self.marks.get(&id).copied()
    }

    /// Rows grouped per rule (or per project), sorted by size within each tier.
    pub fn recommendations(&self, rules: &Rules, env: &Env) -> Vec<Recommendation> {
        let mut rows: Vec<Recommendation> = Vec::new();
        let mut index: HashMap<String, usize> = HashMap::new();
        for item in &self.items {
            let rule = &rules.rules[item.rule];
            let (row_id, label) = row_of(rule, item);
            let i = *index.entry(row_id.clone()).or_insert_with(|| {
                rows.push(Recommendation {
                    row_id,
                    rule_id: rule.id.clone(),
                    label,
                    paths_hint: String::new(),
                    tier: rule.tier,
                    size: 0,
                    size_is_estimate: rule.size_is_estimate,
                    items: Vec::new(),
                });
                rows.len() - 1
            });
            rows[i].items.push(RecItem {
                item_id: item.id.clone(),
                path: item.path.display().to_string(),
                name: item.name.clone(),
                size: item.size,
                status: self.status(rule, item, env),
                method: self.method(rule, item, env),
            });
        }
        for row in &mut rows {
            row.items
                .sort_by(|a, b| b.size.cmp(&a.size).then_with(|| a.name.cmp(&b.name)));
            row.size = row.items.iter().map(|i| i.size).sum();
            let mut hints: Vec<String> = Vec::new();
            for i in &row.items {
                let h = tilde(&self.root, Path::new(&i.path));
                if !hints.contains(&h) {
                    hints.push(h);
                }
            }
            row.paths_hint = hints.join(", ");
        }
        rows.sort_by(|a, b| {
            tier_rank(a.tier)
                .cmp(&tier_rank(b.tier))
                .then_with(|| b.size.cmp(&a.size))
                .then_with(|| a.label.cmp(&b.label))
        });
        rows
    }

    pub fn details(
        &self,
        rules: &Rules,
        snap: &Snapshot,
        env: &Env,
        row_id: &str,
    ) -> Option<ItemDetails> {
        let row = self
            .recommendations(rules, env)
            .into_iter()
            .find(|r| r.row_id == row_id)?;
        let rule = rules.rule(&row.rule_id)?;
        let mut subs: Vec<u32> = self
            .items
            .iter()
            .filter(|i| row.items.iter().any(|r| r.item_id == i.id))
            .filter_map(|i| i.node)
            .flat_map(|n| snap.nodes[n as usize].children.iter().copied())
            .collect();
        subs.sort_by_key(|&c| std::cmp::Reverse(snap.nodes[c as usize].size));
        subs.truncate(SUBFOLDERS);
        Some(ItemDetails {
            row_id: row.row_id,
            description: rule.description.clone(),
            regenerates: rule.regenerates.clone(),
            items: row.items,
            largest_subfolders: subs
                .into_iter()
                .map(|c| Subfolder {
                    name: snap.nodes[c as usize].name.clone(),
                    path: dir_path(snap, c).display().to_string(),
                    size: snap.nodes[c as usize].size,
                })
                .collect(),
        })
    }

    fn status(&self, rule: &Rule, item: &Item, env: &Env) -> ItemStatus {
        if let Some(tool) = missing_tool(rule, env)
            && rule.fallback_method.is_none()
        {
            return ItemStatus::Blocked {
                reason: format!("{tool} not found"),
            };
        }
        match guards::blocked(&rule.guards, &item.path, env) {
            Some(reason) => ItemStatus::Blocked { reason },
            None => ItemStatus::Ready,
        }
    }

    fn method(&self, rule: &Rule, item: &Item, env: &Env) -> MethodView {
        match (rule.method, missing_tool(rule, env), rule.fallback_method) {
            (Method::Delete, ..) => MethodView::Delete,
            (Method::Trash, ..) => MethodView::Trash,
            (Method::Command, Some(_), Some(FallbackMethod::Delete)) => MethodView::Delete,
            (Method::Command, Some(_), Some(FallbackMethod::Trash)) => MethodView::Trash,
            (Method::Command, ..) => MethodView::Command {
                display: rule
                    .command
                    .iter()
                    .flatten()
                    .map(|arg| quote(&self.fill(arg, item)))
                    .collect::<Vec<_>>()
                    .join(" "),
            },
        }
    }

    /// Placeholders with `~`-shortened paths, for display only.
    fn fill(&self, arg: &str, item: &Item) -> String {
        let main = item
            .main_repo
            .as_deref()
            .map(|p| tilde(&self.root, p))
            .unwrap_or_default();
        arg.replace("{path}", &tilde(&self.root, &item.path))
            .replace("{name}", &item.name)
            .replace("{main_repo}", &main)
    }
}

/// `(row_id, label)`: one row per rule, or per project when the rule splits.
fn row_of(rule: &Rule, item: &Item) -> (String, String) {
    match (rule.split_rows_by, &item.project) {
        (Some(SplitBy::Project), Some(project)) => (
            format!("{}:{project}", rule.id),
            rule.row_label.as_deref().map_or_else(
                || format!("{} ({project})", rule.label),
                |l| l.replace("{project}", project),
            ),
        ),
        _ => (rule.id.clone(), rule.label.clone()),
    }
}

fn missing_tool<'r>(rule: &'r Rule, env: &Env) -> Option<&'r str> {
    rule.guards
        .requires_tool
        .as_deref()
        .filter(|t| env.tools.resolve(t).is_none())
}

fn tier_rank(t: Tier) -> u8 {
    match t {
        Tier::Safe => 0,
        Tier::Review => 1,
    }
}

/// `/Users/x/.npm` -> `~/.npm` for paths under `root`.
fn tilde(root: &str, path: &Path) -> String {
    match path.strip_prefix(root) {
        Ok(rel) if rel.as_os_str().is_empty() => "~".into(),
        Ok(rel) => format!("~/{}", rel.display()),
        Err(_) => path.display().to_string(),
    }
}

/// Single-quotes display arguments that contain spaces.
fn quote(arg: &str) -> String {
    if arg.contains(char::is_whitespace) {
        format!("'{arg}'")
    } else {
        arg.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;
    use std::time::{Duration, SystemTime};

    use super::*;
    use crate::platform::{App, Facts, FakeRunner};
    use crate::rules::tools::Tools;
    use crate::scanner::{ScanOptions, scan};

    const OLD: Duration = Duration::from_secs(90 * 86_400);

    struct World {
        _t: tempfile::TempDir,
        home: PathBuf,
        bin: PathBuf,
    }

    impl World {
        fn new() -> Self {
            let t = tempfile::tempdir().unwrap();
            let base = fs::canonicalize(t.path()).unwrap();
            let (home, bin) = (base.join("home"), base.join("bin"));
            fs::create_dir_all(&home).unwrap();
            fs::create_dir_all(&bin).unwrap();
            Self { _t: t, home, bin }
        }

        /// `mb` decimal megabytes of data in `rel/data`, last modified long ago.
        fn put(&self, rel: &str, mb: u64) {
            let dir = self.home.join(rel);
            fs::create_dir_all(&dir).unwrap();
            let f = dir.join("data");
            fs::write(&f, vec![b'x'; (mb * MB) as usize]).unwrap();
            age(&f);
            age(&dir);
        }

        fn tool(&self, name: &str) {
            fs::write(self.bin.join(name), "").unwrap();
            fs::set_permissions(self.bin.join(name), fs::Permissions::from_mode(0o755)).unwrap();
        }

        fn snap(&self) -> Snapshot {
            scan(
                &self.home,
                &ScanOptions::default(),
                &Arc::new(AtomicBool::new(false)),
                &mut |_| {},
            )
            .unwrap()
        }

        fn tools(&self) -> Tools {
            Tools::new(vec![self.bin.clone()], false)
        }
    }

    fn age(p: &Path) {
        fs::File::open(p)
            .unwrap()
            .set_modified(SystemTime::now() - OLD)
            .unwrap();
    }

    fn rows(w: &World, facts: &Facts) -> (Analysis, Vec<Recommendation>) {
        let rules = Rules::embedded();
        let snap = w.snap();
        let tools = w.tools();
        let runner = FakeRunner::default();
        let env = Env {
            facts,
            runner: &runner,
            tools: &tools,
            now: SystemTime::now(),
        };
        let a = analyze(rules, &snap, &env);
        let r = a.recommendations(rules, &env);
        (a, r)
    }

    #[test]
    fn rows_group_items_sum_sizes_and_sort_within_tiers() {
        let w = World::new();
        w.put(".npm/_cacache", 3);
        w.put(".npm/_npx", 1);
        w.put(".gradle/caches", 5);
        w.put(".cache/huggingface/hub", 2);
        w.put(".Trash", 9);
        let (_, rows) = rows(&w, &Facts::default());
        let got: Vec<(&str, Tier)> = rows.iter().map(|r| (r.row_id.as_str(), r.tier)).collect();
        assert_eq!(
            got,
            [
                ("gradle-caches", Tier::Safe),
                ("npm-cache", Tier::Safe),
                ("trash-bin", Tier::Review),
                ("huggingface-cache", Tier::Review),
            ]
        );
        let npm = &rows[1];
        assert_eq!(npm.items.len(), 2);
        assert_eq!(npm.size, npm.items.iter().map(|i| i.size).sum::<u64>());
        assert!(npm.size >= 4 * MB);
        assert_eq!(npm.paths_hint, "~/.npm/_cacache, ~/.npm/_npx");
        assert_eq!(npm.items[0].name, "_cacache", "largest first");
        assert!(npm.items[0].item_id.starts_with("npm-cache:"));
        assert!(npm.items.iter().all(|i| i.status == ItemStatus::Ready));
        assert_eq!(npm.items[0].method, MethodView::Delete);
        assert_eq!(
            rows[2].items[0].method,
            MethodView::Delete,
            "trash-bin deletes"
        );
        assert_eq!(rows[3].items[0].method, MethodView::Trash);
    }

    #[test]
    fn items_below_min_size_are_dropped() {
        let w = World::new();
        w.put("Library/Caches/com.big.app", 51);
        w.put("Library/Caches/com.small.app", 49);
        w.put("Library/Caches/pip", 0); // under the default 1 MB
        let (a, rows) = rows(&w, &Facts::default());
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].rule_id, "app-caches");
        assert_eq!(rows[0].items.len(), 1);
        assert_eq!(rows[0].items[0].name, "com.big.app");
        assert_eq!(a.items.len(), 1);
    }

    #[test]
    fn blocked_items_carry_reasons() {
        let w = World::new();
        w.put("Library/Developer/Xcode/DerivedData", 2);
        w.put(".gradle/caches", 2);
        let facts = Facts {
            apps: vec![App {
                bundle_id: "com.apple.dt.Xcode".into(),
                name: "Xcode".into(),
            }],
            ..Facts::default()
        };
        let (_, rows) = rows(&w, &facts);
        let dd = rows
            .iter()
            .find(|r| r.rule_id == "xcode-derived-data")
            .unwrap();
        assert_eq!(
            dd.items[0].status,
            ItemStatus::Blocked {
                reason: "Quit Xcode to clean".into()
            }
        );
        let gradle = rows.iter().find(|r| r.rule_id == "gradle-caches").unwrap();
        assert_eq!(gradle.items[0].status, ItemStatus::Ready);
    }

    #[test]
    fn commands_and_fallbacks() {
        let w = World::new();
        w.put("Library/Caches/Homebrew", 2);
        w.put("Library/pnpm/store", 2);
        // No tools: brew falls back to delete, pnpm is hidden.
        let (_, rows_none) = rows(&w, &Facts::default());
        let ids: Vec<&str> = rows_none.iter().map(|r| r.rule_id.as_str()).collect();
        assert_eq!(ids, ["homebrew-cache"]);
        assert_eq!(rows_none[0].items[0].method, MethodView::Delete);
        assert_eq!(rows_none[0].items[0].status, ItemStatus::Ready);

        w.tool("brew");
        w.tool("pnpm");
        let (_, rows_all) = rows(&w, &Facts::default());
        let method = |id: &str| {
            rows_all.iter().find(|r| r.rule_id == id).unwrap().items[0]
                .method
                .clone()
        };
        assert_eq!(
            method("homebrew-cache"),
            MethodView::Command {
                display: "brew cleanup --prune=all".into()
            }
        );
        assert_eq!(
            method("pnpm-store"),
            MethodView::Command {
                display: "pnpm store prune".into()
            }
        );
        let pnpm = rows_all.iter().find(|r| r.rule_id == "pnpm-store").unwrap();
        assert!(pnpm.size_is_estimate);
    }

    #[test]
    fn worktrees_split_by_project_with_filled_commands() {
        let w = World::new();
        w.tool("git");
        for (project, wt) in [("atlas", "wt-a"), ("atlas", "wt-b"), ("my app", "wt-c")] {
            let dir = format!("Desktop/{project}/.claude/worktrees/{wt}");
            w.put(&dir, 2);
            let gitdir = w
                .home
                .join(format!("Desktop/{project}/.git/worktrees/{wt}"));
            fs::write(
                w.home.join(&dir).join(".git"),
                format!("gitdir: {}\n", gitdir.display()),
            )
            .unwrap();
        }
        let (_, rows) = rows(&w, &Facts::default());
        let ids: Vec<&str> = rows.iter().map(|r| r.row_id.as_str()).collect();
        assert_eq!(ids, ["agent-worktrees:atlas", "agent-worktrees:my app"]);
        assert_eq!(rows[0].label, "atlas worktrees");
        assert_eq!(rows[0].items.len(), 2);
        assert_eq!(
            rows[1].items[0].method,
            MethodView::Command {
                display: "git -C '~/Desktop/my app' worktree remove '~/Desktop/my app/.claude/worktrees/wt-c'"
                    .into()
            }
        );
        // The fake runner knows no git commands, so git_clean cannot confirm a clean tree.
        assert_eq!(
            rows[0].items[0].status,
            ItemStatus::Blocked {
                reason: "Could not check git status".into()
            }
        );
    }

    #[test]
    fn details_list_the_largest_subfolders() {
        let w = World::new();
        for (name, mb) in [("a", 1), ("b", 6), ("c", 2), ("d", 5), ("e", 3), ("f", 4)] {
            w.put(&format!("Library/Developer/Xcode/DerivedData/{name}"), mb);
        }
        let rules = Rules::embedded();
        let snap = w.snap();
        let facts = Facts::default();
        let tools = w.tools();
        let runner = FakeRunner::default();
        let env = Env {
            facts: &facts,
            runner: &runner,
            tools: &tools,
            now: SystemTime::now(),
        };
        let a = analyze(rules, &snap, &env);
        let d = a.details(rules, &snap, &env, "xcode-derived-data").unwrap();
        let names: Vec<&str> = d
            .largest_subfolders
            .iter()
            .map(|s| s.name.as_str())
            .collect();
        assert_eq!(names, ["b", "d", "f", "e", "c"]);
        assert!(d.description.contains("Xcode"));
        assert_eq!(d.items.len(), 1);
        assert!(a.details(rules, &snap, &env, "nope").is_none());
    }

    #[test]
    fn marks_cover_matched_subtrees() {
        let w = World::new();
        w.put(".npm/_cacache/sub", 2);
        w.put(".npm/other", 2);
        let rules = Rules::embedded();
        let snap = w.snap();
        let facts = Facts::default();
        let tools = w.tools();
        let runner = FakeRunner::default();
        let env = Env {
            facts: &facts,
            runner: &runner,
            tools: &tools,
            now: SystemTime::now(),
        };
        let a = analyze(rules, &snap, &env);
        let id = |rel: &str| crate::view::dir_id(&snap, &w.home.join(rel)).unwrap();
        let npm = rules.rules.iter().position(|r| r.id == "npm-cache");
        assert_eq!(a.rule_of(&snap, id(".npm/_cacache")), npm);
        assert_eq!(a.rule_of(&snap, id(".npm/_cacache/sub")), npm);
        assert_eq!(a.rule_of(&snap, id(".npm/other")), None);
        assert_eq!(a.rule_of(&snap, id(".npm")), None);
    }

    #[test]
    fn tilde_and_quote() {
        assert_eq!(tilde("/Users/x", Path::new("/Users/x/.npm")), "~/.npm");
        assert_eq!(tilde("/Users/x", Path::new("/Users/x")), "~");
        assert_eq!(tilde("/Users/x", Path::new("/opt/y")), "/opt/y");
        assert_eq!(quote("a b"), "'a b'");
        assert_eq!(quote("ab"), "ab");
    }
}
