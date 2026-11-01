//! Reads a repository's working state in process with libgit2 for the
//! `git_active` guard (RULES.md section 4.2, PRD FR-GIT-1). Read-only.

use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use git2::{BranchType, Oid, Repository, StatusOptions};

// ponytail: count stops at 1000; nobody needs an exact number past that, the reason says "1000+".
const UNPUSHED_CAP: usize = 1000;

/// Uncommitted paths and the branch furthest ahead of every remote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoState {
    /// Working-tree-relative paths of every status entry.
    pub(crate) changed: Vec<PathBuf>,
    /// Commit count (capped) and branch name, or "HEAD" when detached.
    pub(crate) unpushed: Option<(usize, String)>,
}

/// Opens `repo` and collects its changed paths and unpushed commits.
pub fn read(repo: &Path) -> Result<RepoState, git2::Error> {
    let repo = Repository::open(repo)?;

    let mut options = StatusOptions::new();
    options
        .include_untracked(true)
        .recurse_untracked_dirs(false)
        .exclude_submodules(true)
        .include_ignored(false)
        // The check must never write to the repository.
        .update_index(false);
    let changed = repo
        .statuses(Some(&mut options))?
        .iter()
        .map(|entry| PathBuf::from(OsStr::from_bytes(entry.path_bytes())))
        .collect();

    Ok(RepoState {
        changed,
        unpushed: most_unpushed(&repo)?,
    })
}

/// The reason a repository counts as active for the artifact at `item_rel`
/// (relative to the repository root), or `None` when it is clean and pushed.
pub fn active_reason(state: &RepoState, item_rel: &Path) -> Option<String> {
    // The artifact itself is often untracked build output; it is what gets
    // removed, not the user's work.
    let changed = state
        .changed
        .iter()
        .filter(|p| !p.starts_with(item_rel))
        .count();
    if changed > 0 {
        return Some(match changed {
            1 => "1 uncommitted file".to_owned(),
            n => format!("{n} uncommitted files"),
        });
    }
    let (count, branch) = state.unpushed.as_ref()?;
    Some(match count {
        1 => format!("1 unpushed commit on {branch}"),
        n if *n >= UNPUSHED_CAP => format!("{UNPUSHED_CAP}+ unpushed commits on {branch}"),
        n => format!("{n} unpushed commits on {branch}"),
    })
}

fn most_unpushed(repo: &Repository) -> Result<Option<(usize, String)>, git2::Error> {
    let mut tips: Vec<(Oid, String)> = Vec::new();
    for branch in repo.branches(Some(BranchType::Local))? {
        let (branch, _) = branch?;
        let name = String::from_utf8_lossy(branch.name_bytes()?).into_owned();
        if let Some(tip) = branch.get().target() {
            tips.push((tip, name));
        }
    }
    if repo.head_detached()?
        && let Some(tip) = repo.head()?.target()
    {
        tips.push((tip, "HEAD".to_owned()));
    }

    let mut best: Option<(usize, String)> = None;
    for (tip, name) in tips {
        let count = count_unpushed(repo, tip)?;
        if count > best.as_ref().map_or(0, |(n, _)| *n) {
            best = Some((count, name));
        }
    }
    Ok(best)
}

fn count_unpushed(repo: &Repository, tip: Oid) -> Result<usize, git2::Error> {
    let mut walk = repo.revwalk()?;
    walk.push(tip)?;
    walk.hide_glob("refs/remotes/*")?;
    let mut count = 0;
    for oid in walk.take(UNPUSHED_CAP) {
        oid?;
        count += 1;
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use git2::{RepositoryInitOptions, Signature};
    use tempfile::TempDir;

    fn init(dir: &Path) -> Repository {
        let mut options = RepositoryInitOptions::new();
        options.initial_head("main");
        Repository::init_opts(dir, &options).unwrap()
    }

    /// Commits `file` with `contents` on the current branch.
    fn commit(repo: &Repository, file: &str, contents: &str) -> Oid {
        let workdir = repo.workdir().unwrap();
        std::fs::write(workdir.join(file), contents).unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new(file)).unwrap();
        index.write().unwrap();
        let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
        let sig = Signature::now("fixture", "fixture@example.com").unwrap();
        let parent = repo.head().ok().and_then(|h| h.peel_to_commit().ok());
        let parents: Vec<_> = parent.iter().collect();
        repo.commit(Some("HEAD"), &sig, &sig, "fixture", &tree, &parents)
            .unwrap()
    }

    /// Marks `oid` as already pushed to origin/main.
    fn push_to_origin(repo: &Repository, oid: Oid) {
        repo.reference("refs/remotes/origin/main", oid, true, "fixture")
            .unwrap();
    }

    /// A repo whose `main` is pushed.
    fn pushed() -> (TempDir, Repository) {
        let dir = TempDir::new().unwrap();
        let repo = init(dir.path());
        let oid = commit(&repo, "a.txt", "a");
        push_to_origin(&repo, oid);
        (dir, repo)
    }

    fn reason(dir: &TempDir, item: &str) -> Option<String> {
        active_reason(&read(dir.path()).unwrap(), Path::new(item))
    }

    #[test]
    fn clean_pushed_repo_is_not_active() {
        let (dir, _repo) = pushed();
        assert_eq!(reason(&dir, "node_modules"), None);
    }

    #[test]
    fn untracked_file_is_one_uncommitted_file() {
        let (dir, _repo) = pushed();
        std::fs::write(dir.path().join("new.txt"), "x").unwrap();
        assert_eq!(
            reason(&dir, "node_modules").as_deref(),
            Some("1 uncommitted file")
        );
    }

    #[test]
    fn modified_and_untracked_files_are_counted_together() {
        let (dir, _repo) = pushed();
        std::fs::write(dir.path().join("a.txt"), "changed").unwrap();
        std::fs::write(dir.path().join("new.txt"), "x").unwrap();
        assert_eq!(
            reason(&dir, "node_modules").as_deref(),
            Some("2 uncommitted files")
        );
    }

    #[test]
    fn changes_inside_the_item_are_ignored() {
        let (dir, _repo) = pushed();
        std::fs::create_dir(dir.path().join("target")).unwrap();
        std::fs::write(dir.path().join("target/out.o"), "x").unwrap();
        assert_eq!(reason(&dir, "target"), None);
    }

    #[test]
    fn branch_ahead_of_remote_reports_unpushed_commits() {
        let (dir, repo) = pushed();
        commit(&repo, "b.txt", "b");
        commit(&repo, "c.txt", "c");
        assert_eq!(
            reason(&dir, "node_modules").as_deref(),
            Some("2 unpushed commits on main")
        );
    }

    #[test]
    fn repo_without_remote_reports_unpushed_commit() {
        let dir = TempDir::new().unwrap();
        let repo = init(dir.path());
        commit(&repo, "a.txt", "a");
        assert_eq!(
            reason(&dir, "node_modules").as_deref(),
            Some("1 unpushed commit on main")
        );
    }

    #[test]
    fn detached_head_on_unreachable_commit_is_named_head() {
        let (dir, repo) = pushed();
        let sig = Signature::now("fixture", "fixture@example.com").unwrap();
        let parent = repo.head().unwrap().peel_to_commit().unwrap();
        let tree = parent.tree().unwrap();
        let orphan = repo
            .commit(None, &sig, &sig, "detached", &tree, &[&parent])
            .unwrap();
        repo.set_head_detached(orphan).unwrap();
        assert_eq!(
            reason(&dir, "node_modules").as_deref(),
            Some("1 unpushed commit on HEAD")
        );
    }

    #[test]
    fn corrupt_repo_fails_to_read() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir(dir.path().join(".git")).unwrap();
        std::fs::write(dir.path().join(".git/HEAD"), "garbage").unwrap();
        assert!(read(dir.path()).is_err());
    }

    #[test]
    fn empty_repo_is_not_active() {
        let dir = TempDir::new().unwrap();
        init(dir.path());
        assert_eq!(reason(&dir, "node_modules"), None);
    }
}
