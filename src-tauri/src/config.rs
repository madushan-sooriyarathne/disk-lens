//! Runtime configuration from environment. Debug builds default to dry-run.

use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// `DISKLENS_SCAN_ROOT` override; `None` means the user's home folder.
    pub scan_root: Option<PathBuf>,
    /// `DISKLENS_DRY_RUN`: log cleanup actions without performing them.
    pub dry_run: bool,
}

impl Config {
    pub fn from_env() -> Self {
        Self::from_vars(
            std::env::var("DISKLENS_SCAN_ROOT").ok(),
            std::env::var("DISKLENS_DRY_RUN").ok(),
            cfg!(debug_assertions),
        )
    }

    fn from_vars(root: Option<String>, dry_run: Option<String>, debug_build: bool) -> Self {
        let dry_run = match dry_run.as_deref() {
            Some("1" | "true") => true,
            Some("0" | "false") => false,
            _ => debug_build,
        };
        Self {
            scan_root: root.filter(|r| !r.is_empty()).map(PathBuf::from),
            dry_run,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_build_defaults_to_dry_run() {
        assert!(Config::from_vars(None, None, true).dry_run);
        assert!(!Config::from_vars(None, None, false).dry_run);
    }

    #[test]
    fn explicit_value_overrides_default() {
        assert!(Config::from_vars(None, Some("1".into()), false).dry_run);
        assert!(!Config::from_vars(None, Some("0".into()), true).dry_run);
    }

    #[test]
    fn unrecognised_value_keeps_default() {
        assert!(Config::from_vars(None, Some("maybe".into()), true).dry_run);
    }

    #[test]
    fn scan_root_override() {
        let c = Config::from_vars(Some("./.sandbox/home".into()), None, true);
        assert_eq!(c.scan_root, Some(PathBuf::from("./.sandbox/home")));
        assert_eq!(
            Config::from_vars(Some(String::new()), None, true).scan_root,
            None
        );
    }
}
