//! Packages in global stores that no project under the scan root uses
//! (PRD FR-STORE-1). Each store has its own model of "used":
//!
//! - When pnpm hard-links store files into `node_modules`, a file with one link
//!   is used by no project. The scan already sums those bytes per folder. On APFS
//!   pnpm clones by default, so the sum is an upper bound (the rule says estimate).
//! - Bun clones packages on macOS, so link counts say nothing: a cached
//!   `name@version` is unused when no `bun.lock` names it.
//! - Cargo crates are unused when no `Cargo.lock` names them.
//! - Homebrew downloads belong to no project: `brew cleanup --dry-run` lists the
//!   old ones.
//!
//! Everything here is read-only and conservative. A lockfile that cannot be read,
//! or a snapshot from before lockfiles were recorded, means nothing is offered.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::{Found, ProviderItem, child};
use crate::rules::guards::Env;
use crate::scanner::walk::allocated_bytes;
use crate::view::{dir_id, dir_path};

/// One item for the store: the bytes in `<store>/<version>/files` with one link,
/// an upper bound of what no project uses. `pnpm store prune` removes the real orphans.
pub fn pnpm(at: &Found) -> Vec<ProviderItem> {
    let snap = at.snap;
    let size: u64 = snap.nodes[at.node as usize]
        .children
        .iter()
        .filter_map(|&v| child(snap, v, "files"))
        .map(|f| snap.nodes[f as usize].single_link_bytes)
        .sum();
    if size == 0 {
        return Vec::new();
    }
    vec![whole(at, "orphans", "Packages no project links to", size)]
}

/// Each `name@version@@@n` folder (also under `@scope/`) that no `bun.lock` under
/// the root names. A project with only the binary `bun.lockb` cannot be read, so
/// every package counts as used. Lockfiles inside `node_modules` came with an
/// installed package and are not projects.
pub fn bun(at: &Found) -> Vec<ProviderItem> {
    let Some(locks) = lockfiles(at, at.path) else {
        return Vec::new();
    };
    let text_dirs: HashSet<&Path> = locks
        .iter()
        .filter(|(_, n)| n == "bun.lock")
        .map(|(d, _)| d.as_path())
        .collect();
    if locks
        .iter()
        .any(|(d, n)| n == "bun.lockb" && !text_dirs.contains(d.as_path()))
    {
        return Vec::new();
    }
    let mut used = HashSet::new();
    for dir in &text_dirs {
        let file = dir.join("bun.lock");
        match fs::read_to_string(&file) {
            Ok(text) => bun_refs(&text, &mut used),
            Err(e) => {
                tracing::warn!("cannot read {}: {e}", file.display());
                return Vec::new();
            }
        }
    }
    let snap = at.snap;
    let mut out = Vec::new();
    for &c in &snap.nodes[at.node as usize].children {
        let name = &snap.nodes[c as usize].name;
        // `@G@`, `@GH@` and `@T@` folders hold git, GitHub and tarball dependencies,
        // which no lockfile names as `name@version`. A scope never has a second `@`.
        if name.strip_prefix('@').is_some_and(|r| r.contains('@')) {
            continue;
        }
        if name.starts_with('@') && !name.contains("@@@") {
            for &p in &snap.nodes[c as usize].children {
                let rel = format!("{name}/{}", snap.nodes[p as usize].name);
                push_bun(at, p, &rel, &used, &mut out);
            }
        } else {
            push_bun(at, c, name, &used, &mut out);
        }
    }
    out
}

/// Crates no `Cargo.lock` under the root names: `.crate` archives when the matched
/// folder is `registry/cache`, extracted sources when it is `registry/src`.
/// Lockfiles inside the registry (crates ship their own) do not count.
pub fn cargo(at: &Found) -> Vec<ProviderItem> {
    let Some(registry) = at.path.parent() else {
        return Vec::new();
    };
    let Some(locks) = lockfiles(at, registry) else {
        return Vec::new();
    };
    let mut used = HashSet::new();
    for (dir, name) in &locks {
        if name != "Cargo.lock" {
            continue;
        }
        let file = dir.join(name);
        let parsed = fs::read_to_string(&file)
            .map_err(|e| e.to_string())
            .and_then(|t| toml::from_str::<CargoLock>(&t).map_err(|e| e.to_string()));
        match parsed {
            Ok(lock) => used.extend(
                lock.package
                    .into_iter()
                    .filter(|p| p.source.as_deref().is_some_and(is_registry))
                    .map(|p| format!("{}-{}", p.name, p.version)),
            ),
            Err(e) => {
                tracing::warn!("cannot read {}: {e}", file.display());
                return Vec::new();
            }
        }
    }
    let snap = at.snap;
    let indexes = &snap.nodes[at.node as usize].children;
    match at.path.file_name().and_then(|n| n.to_str()) {
        Some("cache") => indexes
            .iter()
            .flat_map(|&i| crate_files(&dir_path(snap, i), &snap.nodes[i as usize].name, &used))
            .collect(),
        Some("src") => indexes
            .iter()
            .flat_map(|&i| {
                let index = &snap.nodes[i as usize].name;
                snap.nodes[i as usize]
                    .children
                    .iter()
                    .filter(|&&c| !used.contains(&snap.nodes[c as usize].name))
                    .map(move |&c| {
                        let name = snap.nodes[c as usize].name.clone();
                        ProviderItem {
                            key: format!("src/{index}/{name}"),
                            path: dir_path(snap, c),
                            size: snap.nodes[c as usize].size,
                            node: Some(c),
                            name,
                            main_repo: None,
                            project: None,
                        }
                    })
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// One item for the cache: what `brew cleanup --dry-run` would remove inside the
/// matched folder, sized from disk. `brew cleanup` removes it.
pub fn homebrew(at: &Found, env: &Env) -> Vec<ProviderItem> {
    let Some(brew) = env.tools.resolve("brew") else {
        return Vec::new();
    };
    let stdout = match env.runner.run(&brew, &["cleanup", "--dry-run"]) {
        Ok(o) if o.success => o.stdout,
        Ok(_) => {
            tracing::warn!("brew cleanup --dry-run failed");
            return Vec::new();
        }
        Err(e) => {
            tracing::warn!("brew cleanup --dry-run did not run: {e}");
            return Vec::new();
        }
    };
    let size = listed_size(at, &stdout);
    if size == 0 {
        return Vec::new();
    }
    vec![whole(at, "old", "Old versions and downloads", size)]
}

/// An item for the whole matched folder whose size is only part of it. `node` stays
/// `None` so cleanup does not compare the size with the folder's.
fn whole(at: &Found, key: &str, name: &str, size: u64) -> ProviderItem {
    ProviderItem {
        key: key.into(),
        name: name.into(),
        path: at.path.to_path_buf(),
        size,
        node: None,
        main_repo: None,
        project: None,
    }
}

/// Lockfiles under the root as (folder, file name), skipping those inside `skip`
/// or inside `node_modules`.
/// `None` when the snapshot predates lockfile recording.
fn lockfiles(at: &Found, skip: &Path) -> Option<Vec<(PathBuf, String)>> {
    let snap = at.snap;
    Some(
        snap.lockfiles
            .as_ref()?
            .iter()
            .map(|l| (dir_path(snap, l.node), l.name.clone()))
            .filter(|(d, _)| !d.starts_with(skip))
            .filter(|(d, _)| !d.components().any(|c| c.as_os_str() == "node_modules"))
            .collect(),
    )
}

/// `name@version` of every package a `bun.lock` lists: the first string of each
/// `"key": ["name@version", ...]` entry. The file is JSONC (trailing commas), so it
/// is read line by line.
fn bun_refs(text: &str, into: &mut HashSet<String>) {
    for line in text.lines() {
        if let Some((_, rest)) = line.split_once(": [\"")
            && let Some((package, _)) = rest.split_once('"')
        {
            into.insert(package.to_owned());
        }
    }
}

/// `node` becomes an item when its folder name is `name@version@@@n` and no
/// lockfile names `name@version`. Other entries are never items.
fn push_bun(at: &Found, node: u32, rel: &str, used: &HashSet<String>, out: &mut Vec<ProviderItem>) {
    let Some((package, n)) = rel.rsplit_once("@@@") else {
        return;
    };
    if n.is_empty() || !n.bytes().all(|b| b.is_ascii_digit()) || used.contains(package) {
        return;
    }
    out.push(ProviderItem {
        key: rel.to_owned(),
        name: package.to_owned(),
        path: dir_path(at.snap, node),
        size: at.snap.nodes[node as usize].size,
        node: Some(node),
        main_repo: None,
        project: None,
    });
}

#[derive(Deserialize)]
struct CargoLock {
    #[serde(default)]
    package: Vec<CargoPackage>,
}

#[derive(Deserialize)]
struct CargoPackage {
    name: String,
    version: String,
    source: Option<String>,
}

fn is_registry(source: &str) -> bool {
    source.starts_with("registry+") || source.starts_with("sparse+")
}

/// Unused `.crate` archives in one index folder. The snapshot keeps only the largest
/// files, so the folder is listed on disk.
fn crate_files(dir: &Path, index: &str, used: &HashSet<String>) -> Vec<ProviderItem> {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            tracing::warn!("cannot list {}: {e}", dir.display());
            return Vec::new();
        }
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some(stem) = name.strip_suffix(".crate") else {
            continue;
        };
        if used.contains(stem) {
            continue;
        }
        let path = entry.path();
        let Ok(md) = fs::symlink_metadata(&path) else {
            continue;
        };
        if !md.is_file() {
            continue;
        }
        out.push(ProviderItem {
            key: format!("cache/{index}/{name}"),
            size: allocated_bytes(&md),
            name,
            path,
            node: None,
            main_repo: None,
            project: None,
        });
    }
    out.sort_by(|a, b| a.key.cmp(&b.key));
    out
}

/// Allocated bytes of the paths a `brew cleanup --dry-run` lists inside the matched
/// folder, each counted once. Symlinks count nothing: their targets are listed too.
fn listed_size(at: &Found, stdout: &str) -> u64 {
    let mut seen = HashSet::new();
    let mut total = 0;
    for line in stdout.lines() {
        let Some(rest) = line.strip_prefix("Would remove: ") else {
            continue;
        };
        let path = Path::new(rest.rsplit_once(" (").map_or(rest, |(p, _)| p));
        if !path.starts_with(at.path) || !seen.insert(path.to_path_buf()) {
            continue;
        }
        total += match dir_id(at.snap, path) {
            Some(id) => at.snap.nodes[id as usize].size,
            None => fs::symlink_metadata(path)
                .ok()
                .filter(|md| md.is_file())
                .map_or(0, |md| allocated_bytes(&md)),
        };
    }
    total
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;
    use std::time::SystemTime;

    use super::*;
    use crate::platform::{Facts, FakeRunner};
    use crate::providers::test_support::{node, snap, tree};
    use crate::rules::tools::Tools;
    use crate::scanner::tree::Snapshot;

    const KB: usize = 8192;

    fn found<'a>(s: &'a Snapshot, path: &'a Path) -> Found<'a> {
        Found {
            snap: s,
            node: node(s, path),
            path,
        }
    }

    fn names(items: &[ProviderItem]) -> Vec<&str> {
        items.iter().map(|i| i.name.as_str()).collect()
    }

    fn bytes(n: usize) -> String {
        "x".repeat(n)
    }

    #[test]
    fn pnpm_counts_only_files_no_project_links_to() {
        let body = bytes(KB);
        let (_t, root) = tree(&[
            ("store/v3/files/aa/orphan", &body),
            ("store/v3/files/bb/used", &body),
            ("store/v3/index/aa.json", &body),
        ]);
        fs::create_dir_all(root.join("app/node_modules/dep")).unwrap();
        fs::hard_link(
            root.join("store/v3/files/bb/used"),
            root.join("app/node_modules/dep/index.js"),
        )
        .unwrap();
        let s = snap(&root);
        let store = root.join("store");
        let items = pnpm(&found(&s, &store));
        let orphan = allocated_bytes(&fs::metadata(root.join("store/v3/files/aa/orphan")).unwrap());
        assert_eq!(
            items.iter().map(|i| (i.size, i.node)).collect::<Vec<_>>(),
            [(orphan, None)]
        );
    }

    #[test]
    fn pnpm_offers_nothing_when_every_file_is_linked() {
        let (_t, root) = tree(&[("store/v3/files/bb/used", &bytes(KB))]);
        fs::hard_link(root.join("store/v3/files/bb/used"), root.join("link")).unwrap();
        let s = snap(&root);
        let store = root.join("store");
        assert!(pnpm(&found(&s, &store)).is_empty());
    }

    const BUN_LOCK: &str = r#"{
  "lockfileVersion": 1,
  "workspaces": { "": { "dependencies": { "used": "^1.0.0" } } },
  "packages": {
    "used": ["used@1.0.0", "", {}, "sha512-a"],
    "@s/kit": ["@s/kit@2.0.0", "", {}, "sha512-b"],
  },
}
"#;

    #[test]
    fn bun_lists_package_folders_no_lockfile_names() {
        let (_t, root) = tree(&[
            ("cache/used@1.0.0@@@1/index.js", "a"),
            ("cache/orphan@3.0.0@@@1/index.js", "a"),
            ("cache/@s/kit@2.0.0@@@1/index.js", "a"),
            ("cache/@s/kit@1.0.0@@@1/index.js", "a"),
            ("cache/used/link", "a"),
            ("cache/0155beb9d1c26288.npm", "a"),
            ("app/bun.lock", BUN_LOCK),
        ]);
        let s = snap(&root);
        let cache = root.join("cache");
        let mut got = names(&bun(&found(&s, &cache)))
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        got.sort();
        assert_eq!(got, ["@s/kit@1.0.0", "orphan@3.0.0"]);
    }

    #[test]
    fn bun_skips_git_github_and_tarball_dependency_folders() {
        let (_t, root) = tree(&[
            ("cache/@GH@owner-repo-0123456@@@1/package.json", "{}"),
            ("cache/@G@0123456/x@1.0.0@@@1/package.json", "{}"),
            ("cache/@T@0123456@@@1/package.json", "{}"),
            ("app/bun.lock", BUN_LOCK),
        ]);
        let s = snap(&root);
        let cache = root.join("cache");
        assert!(bun(&found(&s, &cache)).is_empty());
    }

    #[test]
    fn bun_counts_everything_used_when_a_project_has_only_bun_lockb() {
        let (_t, root) = tree(&[
            ("cache/orphan@3.0.0@@@1/index.js", "a"),
            ("app/bun.lockb", "binary"),
        ]);
        let s = snap(&root);
        let cache = root.join("cache");
        assert!(bun(&found(&s, &cache)).is_empty());
    }

    #[test]
    fn bun_ignores_lockfiles_shipped_inside_node_modules() {
        let (_t, root) = tree(&[
            ("cache/orphan@3.0.0@@@1/index.js", "a"),
            ("app/bun.lock", BUN_LOCK),
            ("app/node_modules/x/bun.lockb", "binary"),
        ]);
        let s = snap(&root);
        let cache = root.join("cache");
        assert_eq!(names(&bun(&found(&s, &cache))), ["orphan@3.0.0"]);
    }

    #[test]
    fn bun_ignores_lockfiles_inside_the_cache() {
        let (_t, root) = tree(&[
            ("cache/orphan@3.0.0@@@1/index.js", "a"),
            (
                "cache/orphan@3.0.0@@@1/bun.lock",
                r#"    "orphan": ["orphan@3.0.0", ""],"#,
            ),
        ]);
        let s = snap(&root);
        let cache = root.join("cache");
        assert_eq!(names(&bun(&found(&s, &cache))), ["orphan@3.0.0"]);
    }

    #[test]
    fn bun_offers_nothing_without_recorded_lockfiles() {
        let (_t, root) = tree(&[("cache/orphan@3.0.0@@@1/index.js", "a")]);
        let mut s = snap(&root);
        s.lockfiles = None;
        let cache = root.join("cache");
        assert!(bun(&found(&s, &cache)).is_empty());
    }

    const CARGO_LOCK: &str = r#"version = 4

[[package]]
name = "used"
version = "1.0.0"
source = "registry+https://github.com/rust-lang/crates.io-index"

[[package]]
name = "app"
version = "0.1.0"
"#;

    fn registry() -> (tempfile::TempDir, PathBuf) {
        tree(&[
            (
                "registry/cache/index.crates.io-1/used-1.0.0.crate",
                &bytes(KB),
            ),
            (
                "registry/cache/index.crates.io-1/orphan-0.1.0.crate",
                &bytes(KB),
            ),
            ("registry/src/index.crates.io-1/used-1.0.0/lib.rs", "a"),
            ("registry/src/index.crates.io-1/orphan-0.1.0/lib.rs", "a"),
            (
                "registry/src/index.crates.io-1/orphan-0.1.0/Cargo.lock",
                "[[package]]\nname = \"orphan\"\nversion = \"0.1.0\"\nsource = \"registry+x\"\n",
            ),
            ("app/Cargo.lock", CARGO_LOCK),
        ])
    }

    #[test]
    fn cargo_lists_unused_archives_with_their_size() {
        let (_t, root) = registry();
        let s = snap(&root);
        let cache = root.join("registry/cache");
        let items = cargo(&found(&s, &cache));
        let file = root.join("registry/cache/index.crates.io-1/orphan-0.1.0.crate");
        assert_eq!(
            items
                .iter()
                .map(|i| (i.name.as_str(), i.size, i.node))
                .collect::<Vec<_>>(),
            [(
                "orphan-0.1.0.crate",
                allocated_bytes(&fs::metadata(file).unwrap()),
                None
            )]
        );
    }

    #[test]
    fn cargo_lists_unused_sources() {
        let (_t, root) = registry();
        let s = snap(&root);
        let src = root.join("registry/src");
        assert_eq!(names(&cargo(&found(&s, &src))), ["orphan-0.1.0"]);
    }

    #[test]
    fn cargo_offers_nothing_when_a_lockfile_does_not_parse() {
        let (_t, root) = registry();
        fs::write(root.join("app/Cargo.lock"), "not = [toml").unwrap();
        let s = snap(&root);
        let src = root.join("registry/src");
        assert!(cargo(&found(&s, &src)).is_empty());
    }

    #[test]
    fn homebrew_sizes_listed_paths_inside_the_cache() {
        let (_t, root) = tree(&[
            ("Homebrew/downloads/old.tar.gz", &bytes(KB)),
            ("Homebrew/downloads/new.tar.gz", &bytes(KB)),
            ("Homebrew/old-dir/a", &bytes(KB)),
        ]);
        let s = snap(&root);
        let cache = root.join("Homebrew");
        let bin = root.join("bin");
        fs::create_dir_all(&bin).unwrap();
        fs::write(bin.join("brew"), "").unwrap();
        fs::set_permissions(bin.join("brew"), fs::Permissions::from_mode(0o755)).unwrap();
        let tools = Tools::new(vec![bin], false);
        let facts = Facts::default();
        let old = cache.join("downloads/old.tar.gz");
        let dir = cache.join("old-dir");
        let stdout = format!(
            "Would remove: {} (8KB)\nWould remove: {} (1 files, 8KB)\nWould remove: {} (8KB)\nWould remove: /opt/homebrew/Cellar/x/1.0 (3 files, 1MB)\n==> This operation would free approximately 1MB of disk space.\n",
            old.display(),
            dir.display(),
            old.display(),
        );
        let runner = FakeRunner::default().with("brew cleanup --dry-run", true, &stdout);
        let env = Env {
            facts: &facts,
            runner: &runner,
            tools: &tools,
            now: SystemTime::now(),
            pro: true,
        };
        let items = homebrew(&found(&s, &cache), &env);
        let want =
            allocated_bytes(&fs::metadata(&old).unwrap()) + s.nodes[node(&s, &dir) as usize].size;
        assert_eq!(
            items.iter().map(|i| (i.size, i.node)).collect::<Vec<_>>(),
            [(want, None)]
        );
    }

    #[test]
    fn homebrew_offers_nothing_when_brew_fails() {
        let (_t, root) = tree(&[("Homebrew/downloads/old.tar.gz", "a")]);
        let s = snap(&root);
        let cache = root.join("Homebrew");
        let bin = root.join("bin");
        fs::create_dir_all(&bin).unwrap();
        fs::write(bin.join("brew"), "").unwrap();
        fs::set_permissions(bin.join("brew"), fs::Permissions::from_mode(0o755)).unwrap();
        let tools = Tools::new(vec![bin], false);
        let facts = Facts::default();
        let runner = FakeRunner::default();
        let env = Env {
            facts: &facts,
            runner: &runner,
            tools: &tools,
            now: SystemTime::now(),
            pro: true,
        };
        assert!(homebrew(&found(&s, &cache), &env).is_empty());
    }
}
