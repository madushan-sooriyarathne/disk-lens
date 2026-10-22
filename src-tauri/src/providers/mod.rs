//! Tool-aware item enumeration for folders that cannot be cleaned by deleting
//! the matched folder (RULES.md section 4.3). Everything here is read-only.
//!
//! `ollama`, `simctl_unavailable` and `docker` act on machine-wide state, so in
//! sandbox mode they return nothing.

pub mod docker;
pub mod git_worktrees;
pub mod ollama;
pub mod simctl;

use std::path::{Path, PathBuf};

use crate::rules::guards::Env;
use crate::rules::model::Provider;
use crate::scanner::tree::Snapshot;

/// One cleanable unit found by a provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderItem {
    /// Unique within the rule; the item id is `"{rule_id}:{key}"`.
    pub key: String,
    /// Display name; also the `{name}` placeholder.
    pub name: String,
    /// Where the data lives, for Reveal in Finder and the path hint.
    pub path: PathBuf,
    pub size: u64,
    /// Snapshot node holding the data, when there is one.
    pub node: Option<u32>,
    /// `{main_repo}` for git worktrees.
    pub main_repo: Option<PathBuf>,
    /// `{project}` for split rows.
    pub project: Option<String>,
}

/// The folder a provider rule matched.
pub struct Found<'a> {
    pub snap: &'a Snapshot,
    pub node: u32,
    pub path: &'a Path,
}

pub fn items(provider: Provider, at: &Found, env: &Env) -> Vec<ProviderItem> {
    let global = matches!(
        provider,
        Provider::Ollama | Provider::SimctlUnavailable | Provider::Docker
    );
    if global && env.tools.is_sandbox() {
        return Vec::new();
    }
    match provider {
        Provider::Ollama => ollama::items(at),
        Provider::GitWorktrees => git_worktrees::items(at),
        Provider::SimctlUnavailable => simctl::items(at, env),
        Provider::Docker => docker::items(at, env),
    }
}

/// The child of `node` named `name`.
pub(crate) fn child(snap: &Snapshot, node: u32, name: &str) -> Option<u32> {
    snap.nodes[node as usize]
        .children
        .iter()
        .copied()
        .find(|&c| snap.nodes[c as usize].name == name)
}

#[cfg(test)]
pub(crate) mod test_support {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;

    use crate::scanner::tree::Snapshot;
    use crate::scanner::{ScanOptions, scan};
    use crate::view::dir_id;

    /// Canonical temp root holding `files` (relative path, contents).
    pub fn tree(files: &[(&str, &str)]) -> (tempfile::TempDir, PathBuf) {
        let t = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(t.path()).unwrap();
        for (rel, body) in files {
            let p = root.join(rel);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, body).unwrap();
        }
        (t, root)
    }

    pub fn snap(root: &Path) -> Snapshot {
        scan(
            root,
            &ScanOptions::default(),
            &Arc::new(AtomicBool::new(false)),
            &mut |_| {},
        )
        .unwrap()
    }

    pub fn node(snap: &Snapshot, path: &Path) -> u32 {
        dir_id(snap, path).unwrap()
    }
}
