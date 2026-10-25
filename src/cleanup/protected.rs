//! Hardcoded protections (RULES.md section 5). The protected list mirrors the
//! `[[protect]]` entries in rules.toml and a test keeps them equal; when they
//! disagree, both apply. `Protected::refusal` adds the other section 5 checks
//! and runs against the disk as it is now.

use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use globset::{GlobSet, GlobSetBuilder};

use crate::rules::{compile, expand};

/// Globs relative to the scan root, same syntax as `[[protect]]`.
pub const PROTECTED: [&str; 19] = [
    "~/Library/Mobile Documents",
    "~/Library/CloudStorage",
    "~/Library/Keychains",
    "~/Library/Mail",
    "~/Library/Messages",
    "~/Library/Photos",
    "~/Pictures/*.photoslibrary",
    "~/Library/Application Support/AddressBook",
    "~/Library/Calendars",
    "~/Library/Safari",
    "~/Library/Cookies",
    "~/Library/Preferences",
    "~/Library/Application Support/com.apple.TCC",
    "~/Library/Containers/com.apple.*",
    "~/Library/Group Containers/group.com.apple.*",
    "~/.ssh",
    "~/.gnupg",
    "~/.aws",
    "~/.kube",
];

/// Standard folders directly under the root that are never a target themselves.
const STANDARD_FOLDERS: [&str; 9] = [
    "Desktop",
    "Documents",
    "Downloads",
    "Library",
    "Pictures",
    "Movies",
    "Music",
    "Public",
    "Applications",
];

pub struct Protected {
    root: PathBuf,
    set: GlobSet,
}

impl Protected {
    /// The hardcoded list plus `extra` (the `[[protect]]` paths), anchored at `root`.
    pub fn new(root: &str, extra: &[String]) -> Self {
        let mut b = GlobSetBuilder::new();
        for p in PROTECTED
            .iter()
            .copied()
            .chain(extra.iter().map(String::as_str))
        {
            b.add(compile(&expand(p, root)).expect("protect globs are validated"));
        }
        Self {
            root: PathBuf::from(root),
            set: b.build().expect("protect set builds"),
        }
    }

    /// Why the cleanup engine must not touch `path`, or `None` when it may.
    /// Reads the disk: `path` must exist, be its own canonical form (no symlink
    /// anywhere in it), lie strictly inside the root, and be neither a standard
    /// folder, a symlink, a mount point, a repository root nor protected.
    pub fn refusal(&self, path: &Path) -> Option<String> {
        let md = match path.symlink_metadata() {
            Ok(md) => md,
            Err(e) => return Some(format!("cannot be read ({e})")),
        };
        if md.file_type().is_symlink() {
            return Some("is a symlink".into());
        }
        match std::fs::canonicalize(path) {
            Ok(c) if c == path => {}
            Ok(_) => return Some("is reached through a symlink".into()),
            Err(e) => return Some(format!("cannot be resolved ({e})")),
        }
        let Ok(rel) = path.strip_prefix(&self.root) else {
            return Some("is outside the scan root".into());
        };
        if rel.as_os_str().is_empty() {
            return Some("is the scan root".into());
        }
        if rel.components().count() == 1 && STANDARD_FOLDERS.iter().any(|f| rel == Path::new(f)) {
            return Some("is a standard folder".into());
        }
        match path.parent().map(Path::symlink_metadata) {
            Some(Ok(parent)) if parent.dev() == md.dev() => {}
            Some(Ok(_)) => return Some("is a mount point".into()),
            Some(Err(e)) => return Some(format!("parent cannot be read ({e})")),
            None => return Some("has no parent".into()),
        }
        if path
            .join(".git")
            .symlink_metadata()
            .is_ok_and(|g| g.is_dir())
        {
            return Some("is a repository root".into());
        }
        if self.is_protected(path) {
            return Some("is protected".into());
        }
        None
    }

    /// `path` is itself a protected path.
    pub fn is_protected_root(&self, path: &Path) -> bool {
        self.set.is_match(path)
    }

    /// `path` is a protected path or lies inside one.
    pub fn is_protected(&self, path: &Path) -> bool {
        path.ancestors().any(|p| self.set.is_match(p))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::model::Rules;

    #[test]
    fn hardcoded_list_equals_rules_toml() {
        let toml: Vec<&str> = Rules::embedded()
            .protect
            .iter()
            .map(|p| p.path.as_str())
            .collect();
        assert_eq!(toml, PROTECTED);
    }

    #[test]
    fn protects_paths_and_their_contents_only() {
        let p = Protected::new("/Users/x", &[]);
        for yes in [
            "/Users/x/.ssh",
            "/Users/x/.ssh/id_ed25519",
            "/Users/x/Library/Keychains/login.keychain-db",
            "/Users/x/Pictures/Photos Library.photoslibrary/database",
            "/Users/x/Library/Containers/com.apple.Safari/Data",
            "/Users/x/Library/Group Containers/group.com.apple.notes",
        ] {
            assert!(p.is_protected(Path::new(yes)), "{yes}");
        }
        for no in [
            "/Users/x",
            "/Users/x/Library",
            "/Users/x/Library/KeychainsX",
            "/Users/x/.sshx",
            "/Users/x/Pictures/a/b.photoslibrary",
            "/Users/x/Library/Containers/com.docker.docker",
            "/Users/y/.ssh",
        ] {
            assert!(!p.is_protected(Path::new(no)), "{no}");
        }
        assert!(p.is_protected_root(Path::new("/Users/x/.aws")));
        assert!(!p.is_protected_root(Path::new("/Users/x/.aws/config")));
    }

    #[test]
    fn extra_entries_and_glob_characters_in_root() {
        let p = Protected::new("/tmp/a[1]", &["~/Secret".into()]);
        assert!(p.is_protected(Path::new("/tmp/a[1]/Secret/x")));
        assert!(p.is_protected(Path::new("/tmp/a[1]/.kube")));
        assert!(!p.is_protected(Path::new("/tmp/a1/.kube")));
    }
}

#[cfg(test)]
mod refusal_tests {
    use std::fs;
    use std::os::unix::fs::symlink;

    use super::*;

    struct Home {
        _t: tempfile::TempDir,
        root: PathBuf,
        protected: Protected,
    }

    fn home(dirs: &[&str]) -> Home {
        let t = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(t.path()).unwrap().join("home");
        for d in dirs {
            fs::create_dir_all(root.join(d)).unwrap();
        }
        fs::create_dir_all(&root).unwrap();
        let protected = Protected::new(&root.display().to_string(), &[]);
        Home {
            _t: t,
            root,
            protected,
        }
    }

    impl Home {
        fn refusal(&self, rel: &str) -> Option<String> {
            self.protected.refusal(&self.root.join(rel))
        }
    }

    #[test]
    fn allows_an_ordinary_folder() {
        let h = home(&[".npm/_cacache"]);
        assert_eq!(h.refusal(".npm/_cacache"), None);
    }

    #[test]
    fn refuses_the_scan_root() {
        let h = home(&[]);
        assert_eq!(h.refusal("").as_deref(), Some("is the scan root"));
    }

    #[test]
    fn refuses_paths_outside_the_root() {
        let h = home(&[]);
        let outside = h.root.parent().unwrap().to_path_buf();
        assert_eq!(
            h.protected.refusal(&outside).as_deref(),
            Some("is outside the scan root")
        );
    }

    #[test]
    fn refuses_standard_folders_but_not_their_children() {
        let h = home(&["Library/Caches"]);
        assert_eq!(
            (h.refusal("Library").as_deref(), h.refusal("Library/Caches")),
            (Some("is a standard folder"), None)
        );
    }

    #[test]
    fn refuses_a_symlink() {
        let h = home(&["real"]);
        symlink(h.root.join("real"), h.root.join("link")).unwrap();
        assert_eq!(h.refusal("link").as_deref(), Some("is a symlink"));
    }

    #[test]
    fn refuses_a_path_through_a_symlinked_parent() {
        let h = home(&["real/cache"]);
        symlink(h.root.join("real"), h.root.join("link")).unwrap();
        assert_eq!(
            h.refusal("link/cache").as_deref(),
            Some("is reached through a symlink")
        );
    }

    #[test]
    fn refuses_a_repository_root() {
        let h = home(&["code/app/.git"]);
        assert_eq!(
            h.refusal("code/app").as_deref(),
            Some("is a repository root")
        );
    }

    #[test]
    fn allows_a_worktree_whose_git_is_a_file() {
        let h = home(&["code/wt"]);
        fs::write(h.root.join("code/wt/.git"), "gitdir: /x\n").unwrap();
        assert_eq!(h.refusal("code/wt"), None);
    }

    #[test]
    fn refuses_protected_paths_and_their_contents() {
        let h = home(&[".ssh/keys"]);
        assert_eq!(h.refusal(".ssh/keys").as_deref(), Some("is protected"));
    }

    #[test]
    fn refuses_a_missing_path() {
        let h = home(&[]);
        assert!(h.refusal("gone").unwrap().starts_with("cannot be read"));
    }
}
