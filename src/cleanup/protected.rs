//! Hardcoded protected subtrees (RULES.md section 5). The list mirrors the
//! `[[protect]]` entries in rules.toml and a test keeps them equal; when they
//! disagree, both apply. Phase 4 adds the remaining section 5 checks.

use std::path::Path;

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

pub struct Protected {
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
            set: b.build().expect("protect set builds"),
        }
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
