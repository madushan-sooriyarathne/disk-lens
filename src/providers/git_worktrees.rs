//! Linked git worktrees inside a matched folder: each child whose `.git` is a
//! file reading `gitdir: <main repo>/.git/worktrees/<name>`.

use std::fs;
use std::path::{Component, Path, PathBuf};

use super::{Found, ProviderItem};

pub fn items(at: &Found) -> Vec<ProviderItem> {
    let snap = at.snap;
    snap.nodes[at.node as usize]
        .children
        .iter()
        .filter_map(|&c| {
            let name = &snap.nodes[c as usize].name;
            let path = at.path.join(name);
            let main_repo = main_repo(&path)?;
            let project = main_repo.file_name()?.to_string_lossy().into_owned();
            Some(ProviderItem {
                key: c.to_string(),
                name: name.clone(),
                size: snap.nodes[c as usize].size,
                node: Some(c),
                path,
                main_repo: Some(main_repo),
                project: Some(project),
            })
        })
        .collect()
}

/// The main repository of the worktree at `dir`, from its `.git` file.
fn main_repo(dir: &Path) -> Option<PathBuf> {
    let dot_git = dir.join(".git");
    if !fs::symlink_metadata(&dot_git).ok()?.is_file() {
        return None;
    }
    let text = fs::read_to_string(&dot_git).ok()?;
    let gitdir = text.lines().find_map(|l| l.strip_prefix("gitdir:"))?.trim();
    let gitdir = normalize(&dir.join(gitdir));
    // <main>/.git/worktrees/<name>
    let worktrees = gitdir.parent()?;
    let git = worktrees.parent()?;
    if worktrees.file_name()? != "worktrees" || git.file_name()? != ".git" {
        return None;
    }
    git.parent().map(Path::to_path_buf)
}

/// Resolves `.` and `..` lexically; `join` keeps absolute gitdirs as they are.
fn normalize(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::test_support::{node, snap, tree};

    #[test]
    fn finds_linked_worktrees_only() {
        let (_t, root) = tree(&[
            ("atlas/.git/HEAD", "ref: refs/heads/main"),
            (
                "atlas/.claude/worktrees/wt-a/.git",
                "gitdir: ROOT/atlas/.git/worktrees/wt-a\n",
            ),
            ("atlas/.claude/worktrees/wt-a/src.rs", &"x".repeat(4096)),
            (
                "atlas/.claude/worktrees/wt-rel/.git",
                "gitdir: ../../../.git/worktrees/wt-rel\n",
            ),
            (
                "atlas/.claude/worktrees/clone/.git/HEAD",
                "ref: refs/heads/main",
            ),
            ("atlas/.claude/worktrees/plain/readme", "x"),
            (
                "atlas/.claude/worktrees/odd/.git",
                "gitdir: /somewhere/else\n",
            ),
        ]);
        let wt_a = root.join("atlas/.claude/worktrees/wt-a/.git");
        let fixed = fs::read_to_string(&wt_a)
            .unwrap()
            .replace("ROOT", root.to_str().unwrap());
        fs::write(&wt_a, fixed).unwrap();

        let s = snap(&root);
        let folder = root.join("atlas/.claude/worktrees");
        let at = Found {
            snap: &s,
            node: node(&s, &folder),
            path: &folder,
        };
        let mut got = items(&at);
        got.sort_by(|a, b| a.name.cmp(&b.name));
        let names: Vec<&str> = got.iter().map(|i| i.name.as_str()).collect();
        assert_eq!(names, ["wt-a", "wt-rel"]);
        for i in &got {
            assert_eq!(i.main_repo.as_deref(), Some(root.join("atlas").as_path()));
            assert_eq!(i.project.as_deref(), Some("atlas"));
            assert_eq!(i.node, crate::providers::child(&s, at.node, &i.name));
            assert_eq!(i.key, i.node.unwrap().to_string());
        }
        assert!(got[0].size >= 4096);
    }

    #[test]
    fn normalize_resolves_dots() {
        assert_eq!(
            normalize(Path::new("/a/b/../c/./d")),
            PathBuf::from("/a/c/d")
        );
    }
}
