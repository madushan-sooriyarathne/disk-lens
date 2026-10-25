//! Runtime configuration from environment. Debug builds default to dry-run.

use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// `DISKLENS_SCAN_ROOT` override; `None` means the user's home folder.
    pub scan_root: Option<PathBuf>,
    /// `DISKLENS_DRY_RUN`: log cleanup actions without performing them.
    pub dry_run: bool,
    /// `DISKLENS_DATA_DIR`, else `<scan_root>/../data` when sandboxed, else Application Support.
    pub data_dir: PathBuf,
    /// Audit log folder: `<scan_root>/../logs` when sandboxed, else `~/Library/Logs/DiskLens`.
    pub log_dir: PathBuf,
}

#[derive(Default)]
struct Vars {
    scan_root: Option<String>,
    dry_run: Option<String>,
    data_dir: Option<String>,
    home: Option<String>,
    debug_build: bool,
}

impl Config {
    pub fn from_env() -> Self {
        let var = |k| std::env::var(k).ok();
        Self::from_vars(Vars {
            scan_root: var("DISKLENS_SCAN_ROOT"),
            dry_run: var("DISKLENS_DRY_RUN"),
            data_dir: var("DISKLENS_DATA_DIR"),
            home: var("HOME"),
            debug_build: cfg!(debug_assertions),
        })
    }

    /// The folder to scan: the override, else the home folder.
    pub fn root(&self) -> Option<PathBuf> {
        self.scan_root
            .clone()
            .or_else(|| std::env::var_os("HOME").map(PathBuf::from))
    }

    fn from_vars(v: Vars) -> Self {
        let dry_run = match v.dry_run.as_deref() {
            Some("1" | "true") => true,
            Some("0" | "false") => false,
            _ => v.debug_build,
        };
        let scan_root = v.scan_root.filter(|r| !r.is_empty()).map(PathBuf::from);
        let home = Path::new(v.home.as_deref().unwrap_or("."));
        let data_dir = v
            .data_dir
            .filter(|d| !d.is_empty())
            .map(PathBuf::from)
            .or_else(|| scan_root.as_deref().and_then(|r| sandbox_dir(r, "data")))
            .unwrap_or_else(|| home.join("Library/Application Support/DiskLens"));
        let log_dir = scan_root
            .as_deref()
            .and_then(|r| sandbox_dir(r, "logs"))
            .unwrap_or_else(|| home.join("Library/Logs/DiskLens"));
        Self {
            scan_root,
            dry_run,
            data_dir,
            log_dir,
        }
    }
}

/// Sibling `name` folder of the scan root, so sandboxed runs never touch real Library.
fn sandbox_dir(root: &Path, name: &str) -> Option<PathBuf> {
    root.parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(|p| p.join(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(f: impl FnOnce(&mut Vars)) -> Config {
        let mut v = Vars {
            home: Some("/Users/x".into()),
            debug_build: true,
            ..Default::default()
        };
        f(&mut v);
        Config::from_vars(v)
    }

    #[test]
    fn debug_build_defaults_to_dry_run() {
        assert!(cfg(|_| {}).dry_run);
        assert!(!cfg(|v| v.debug_build = false).dry_run);
    }

    #[test]
    fn explicit_value_overrides_default() {
        assert!(
            cfg(|v| {
                v.debug_build = false;
                v.dry_run = Some("1".into());
            })
            .dry_run
        );
        assert!(!cfg(|v| v.dry_run = Some("0".into())).dry_run);
    }

    #[test]
    fn unrecognised_value_keeps_default() {
        assert!(cfg(|v| v.dry_run = Some("maybe".into())).dry_run);
    }

    #[test]
    fn scan_root_override() {
        let c = cfg(|v| v.scan_root = Some("./.sandbox/home".into()));
        assert_eq!(c.scan_root, Some(PathBuf::from("./.sandbox/home")));
        assert_eq!(cfg(|v| v.scan_root = Some(String::new())).scan_root, None);
    }

    #[test]
    fn data_dir_defaults_to_application_support() {
        assert_eq!(
            cfg(|_| {}).data_dir,
            PathBuf::from("/Users/x/Library/Application Support/DiskLens")
        );
    }

    #[test]
    fn sandboxed_scan_root_redirects_data_dir() {
        let c = cfg(|v| v.scan_root = Some("./.sandbox/home".into()));
        assert_eq!(c.data_dir, PathBuf::from("./.sandbox/data"));
    }

    #[test]
    fn log_dir_defaults_to_library_logs() {
        assert_eq!(
            cfg(|_| {}).log_dir,
            PathBuf::from("/Users/x/Library/Logs/DiskLens")
        );
    }

    #[test]
    fn sandboxed_scan_root_redirects_log_dir() {
        let c = cfg(|v| v.scan_root = Some("./.sandbox/home".into()));
        assert_eq!(c.log_dir, PathBuf::from("./.sandbox/logs"));
    }

    #[test]
    fn explicit_data_dir_wins() {
        let c = cfg(|v| {
            v.scan_root = Some("./.sandbox/home".into());
            v.data_dir = Some("/tmp/d".into());
        });
        assert_eq!(c.data_dir, PathBuf::from("/tmp/d"));
    }
}
