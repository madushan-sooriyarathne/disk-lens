//! Resolves tool names to absolute binaries. GUI apps do not inherit the shell
//! `PATH`, so the login shell's `PATH` is read once and merged with the usual
//! install folders (PRD section 11).
//!
//! In sandbox mode (`DISKLENS_SCAN_ROOT` set) only `git` resolves. Global tools
//! such as `pnpm`, `brew`, `docker`, `ollama` and `xcrun` act on the real machine
//! whatever folder is scanned, so a sandbox run must never be able to aim them.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::platform::{ProcessRunner, Runner};

/// The only tools a sandbox run may use; they act on the paths they are given.
pub const SANDBOX_TOOLS: [&str; 1] = ["git"];
const SHELL_TIMEOUT: Duration = Duration::from_secs(3);
/// Relative to the user's real home folder.
const HOME_DIRS: [&str; 3] = [".bun/bin", ".local/bin", "Library/pnpm"];
const SYSTEM_DIRS: [&str; 4] = ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin", "/bin"];

#[derive(Debug, Clone)]
pub struct Tools {
    dirs: Vec<PathBuf>,
    sandbox: bool,
}

impl Tools {
    /// Login-shell `PATH` plus known folders. `home` is the real home folder.
    pub fn discover(home: &Path, sandbox: bool) -> Self {
        let shell = login_shell_path(home).unwrap_or_default();
        let mut dirs: Vec<PathBuf> = std::env::split_paths(&shell).collect();
        dirs.extend(HOME_DIRS.iter().map(|d| home.join(d)));
        dirs.extend(SYSTEM_DIRS.iter().map(PathBuf::from));
        Self::new(dirs, sandbox)
    }

    /// Searches `dirs` in order; relative and duplicate entries are dropped.
    pub fn new(dirs: Vec<PathBuf>, sandbox: bool) -> Self {
        let mut unique: Vec<PathBuf> = Vec::new();
        for d in dirs {
            if d.is_absolute() && !unique.contains(&d) {
                unique.push(d);
            }
        }
        Self {
            dirs: unique,
            sandbox,
        }
    }

    pub fn is_sandbox(&self) -> bool {
        self.sandbox
    }

    /// Absolute path of an executable named `name`, if allowed and found.
    pub fn resolve(&self, name: &str) -> Option<PathBuf> {
        if name.is_empty() || name.contains('/') {
            return None;
        }
        if self.sandbox && !SANDBOX_TOOLS.contains(&name) {
            return None;
        }
        self.dirs
            .iter()
            .map(|d| d.join(name))
            .find(|p| is_executable(p))
    }
}

fn is_executable(p: &Path) -> bool {
    std::fs::metadata(p).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

fn login_shell_path(home: &Path) -> Option<String> {
    let runner = ProcessRunner {
        timeout: SHELL_TIMEOUT,
        home: home.display().to_string(),
    };
    match runner.run(Path::new("/bin/zsh"), &["-lc", "printf %s \"$PATH\""]) {
        Ok(out) if out.success => Some(out.stdout),
        Ok(_) => None,
        Err(e) => {
            tracing::warn!("login shell PATH lookup failed: {e}");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    fn bin(dir: &Path, name: &str, mode: u32) {
        fs::create_dir_all(dir).unwrap();
        let p = dir.join(name);
        fs::write(&p, "#!/bin/sh\n").unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(mode)).unwrap();
    }

    #[test]
    fn first_executable_in_order_wins() {
        let t = tempfile::tempdir().unwrap();
        let (a, b) = (t.path().join("a"), t.path().join("b"));
        bin(&a, "pnpm", 0o644); // not executable
        bin(&b, "pnpm", 0o755);
        bin(&a, "brew", 0o755);
        bin(&b, "brew", 0o755);
        let tools = Tools::new(vec![a.clone(), b.clone()], false);
        assert_eq!(tools.resolve("pnpm"), Some(b.join("pnpm")));
        assert_eq!(tools.resolve("brew"), Some(a.join("brew")));
        assert_eq!(tools.resolve("docker"), None);
    }

    #[test]
    fn rejects_names_with_paths_and_relative_dirs() {
        let t = tempfile::tempdir().unwrap();
        bin(t.path(), "git", 0o755);
        let tools = Tools::new(vec![PathBuf::from("."), t.path().into()], false);
        assert_eq!(tools.dirs.len(), 1);
        assert_eq!(tools.resolve("../git"), None);
        assert_eq!(tools.resolve(""), None);
    }

    #[test]
    fn sandbox_resolves_only_git() {
        let t = tempfile::tempdir().unwrap();
        for name in ["git", "pnpm", "brew", "docker", "ollama", "xcrun"] {
            bin(t.path(), name, 0o755);
        }
        let tools = Tools::new(vec![t.path().into()], true);
        assert!(tools.resolve("git").is_some());
        for name in ["pnpm", "brew", "docker", "ollama", "xcrun"] {
            assert_eq!(tools.resolve(name), None, "{name}");
        }
    }

    #[test]
    fn discover_finds_system_git() {
        let home = std::env::var("HOME").unwrap();
        let tools = Tools::discover(Path::new(&home), true);
        assert!(tools.resolve("git").is_some());
        assert!(tools.resolve("xcrun").is_none());
    }
}
