//! What the rules engine asks of the machine: running processes and apps, and
//! running external commands. Real implementations talk to macOS; the fakes let
//! tests describe a machine without touching it.

use std::collections::HashMap;
use std::io::{self, Read};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::Duration;

use wait_timeout::ChildExt;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Proc {
    pub name: String,
    /// Arguments joined with spaces.
    pub cmdline: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct App {
    pub bundle_id: String,
    pub name: String,
}

/// Running processes and GUI apps at one moment.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Facts {
    pub processes: Vec<Proc>,
    pub apps: Vec<App>,
}

pub trait System: Send + Sync {
    fn facts(&self) -> Facts;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output {
    pub success: bool,
    pub stdout: String,
}

/// Runs a binary with argv, never through a shell (S-05).
pub trait Runner: Send + Sync {
    fn run(&self, program: &Path, args: &[&str]) -> io::Result<Output>;
}

/// `sysinfo` for processes, `NSWorkspace.runningApplications` for apps.
pub struct MacSystem;

impl System for MacSystem {
    fn facts(&self) -> Facts {
        use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, UpdateKind};
        let mut sys = sysinfo::System::new();
        sys.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing().with_cmd(UpdateKind::Always),
        );
        let me = sysinfo::Pid::from_u32(std::process::id());
        let processes = sys
            .processes()
            .values()
            .filter(|p| p.pid() != me)
            .map(|p| {
                let name = p.name().to_string_lossy().into_owned();
                let cmd: Vec<_> = p.cmd().iter().map(|a| a.to_string_lossy()).collect();
                // Other users' processes hide their arguments; match on the name then.
                let cmdline = if cmd.is_empty() {
                    name.clone()
                } else {
                    cmd.join(" ")
                };
                Proc { name, cmdline }
            })
            .collect();
        Facts {
            processes,
            apps: running_apps(),
        }
    }
}

fn running_apps() -> Vec<App> {
    use objc2_app_kit::NSWorkspace;
    let apps = NSWorkspace::sharedWorkspace().runningApplications();
    apps.iter()
        .filter_map(|app| {
            let bundle_id = app.bundleIdentifier()?.to_string();
            let name = app
                .localizedName()
                .map_or_else(|| bundle_id.clone(), |n| n.to_string());
            Some(App { bundle_id, name })
        })
        .collect()
}

/// Real commands: absolute binary, argv, minimal environment, hard timeout.
pub struct ProcessRunner {
    pub timeout: Duration,
    /// Passed as `HOME`; tools read their own config from there.
    pub home: String,
}

impl Runner for ProcessRunner {
    fn run(&self, program: &Path, args: &[&str]) -> io::Result<Output> {
        if !program.is_absolute() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "program must be an absolute path",
            ));
        }
        let mut child = Command::new(program)
            .args(args)
            .env_clear()
            .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
            .env("HOME", &self.home)
            .env("LANG", "en_US.UTF-8")
            // Read-only git commands must not take the index lock.
            .env("GIT_OPTIONAL_LOCKS", "0")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        // Drain stdout on a thread so a large output cannot fill the pipe and stall the child.
        let mut out = child.stdout.take().expect("stdout is piped");
        let reader = std::thread::spawn(move || {
            let mut buf = Vec::new();
            out.read_to_end(&mut buf).map(|_| buf)
        });
        let Some(status) = child.wait_timeout(self.timeout)? else {
            let _ = child.kill();
            let _ = child.wait();
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                format!("{} timed out", program.display()),
            ));
        };
        let stdout = reader
            .join()
            .map_err(|_| io::Error::other("output reader panicked"))??;
        Ok(Output {
            success: status.success(),
            stdout: String::from_utf8_lossy(&stdout).into_owned(),
        })
    }
}

/// A machine described by tests.
#[derive(Debug, Default)]
pub struct FakeSystem(pub Mutex<Facts>);

impl FakeSystem {
    pub fn new(facts: Facts) -> Self {
        Self(Mutex::new(facts))
    }
}

impl System for FakeSystem {
    fn facts(&self) -> Facts {
        self.0.lock().expect("fake facts").clone()
    }
}

/// Canned outputs keyed by `"<program file name> <args...>"`; unknown commands fail
/// with `NotFound`. Every call is recorded.
#[derive(Debug, Default)]
pub struct FakeRunner {
    pub outputs: HashMap<String, Output>,
    pub calls: Mutex<Vec<String>>,
}

impl FakeRunner {
    pub fn with(mut self, command: &str, success: bool, stdout: &str) -> Self {
        self.outputs.insert(
            command.to_owned(),
            Output {
                success,
                stdout: stdout.to_owned(),
            },
        );
        self
    }
}

impl Runner for FakeRunner {
    fn run(&self, program: &Path, args: &[&str]) -> io::Result<Output> {
        let name = program
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        let key = std::iter::once(name.as_str())
            .chain(args.iter().copied())
            .collect::<Vec<_>>()
            .join(" ");
        self.calls.lock().expect("fake calls").push(key.clone());
        self.outputs
            .get(&key)
            .cloned()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, key))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn runner(timeout_ms: u64) -> ProcessRunner {
        ProcessRunner {
            timeout: Duration::from_millis(timeout_ms),
            home: "/tmp".into(),
        }
    }

    #[test]
    fn runs_argv_with_minimal_env() {
        let out = runner(5000).run(Path::new("/usr/bin/env"), &[]).unwrap();
        assert!(out.success);
        let mut keys: Vec<_> = out
            .stdout
            .lines()
            .filter_map(|l| l.split_once('=').map(|(k, _)| k))
            .collect();
        keys.sort();
        assert_eq!(keys, ["GIT_OPTIONAL_LOCKS", "HOME", "LANG", "PATH"]);
    }

    #[test]
    fn arguments_are_not_shell_interpreted() {
        let out = runner(5000)
            .run(Path::new("/bin/echo"), &["$HOME", "; ls", "`id`"])
            .unwrap();
        assert_eq!(out.stdout, "$HOME ; ls `id`\n");
    }

    #[test]
    fn large_output_does_not_deadlock() {
        let out = runner(10_000)
            .run(Path::new("/usr/bin/head"), &["-c", "1000000", "/dev/zero"])
            .unwrap();
        assert_eq!(out.stdout.len(), 1_000_000);
    }

    #[test]
    fn times_out_and_kills() {
        let started = std::time::Instant::now();
        let err = runner(200)
            .run(Path::new("/bin/sleep"), &["5"])
            .unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::TimedOut);
        assert!(started.elapsed() < Duration::from_secs(3));
    }

    #[test]
    fn refuses_relative_programs_and_reports_failure() {
        assert!(runner(1000).run(Path::new("echo"), &[]).is_err());
        assert!(
            !runner(1000)
                .run(Path::new("/usr/bin/false"), &[])
                .unwrap()
                .success
        );
    }

    #[test]
    fn fake_runner_matches_by_name_and_args() {
        let r = FakeRunner::default().with("git -C /x status", true, "ok");
        assert_eq!(
            r.run(Path::new("/usr/bin/git"), &["-C", "/x", "status"])
                .unwrap()
                .stdout,
            "ok"
        );
        assert!(r.run(Path::new("/usr/bin/git"), &["log"]).is_err());
        assert_eq!(r.calls.lock().unwrap().len(), 2);
    }

    #[test]
    fn mac_system_lists_processes_but_not_itself() {
        let facts = MacSystem.facts();
        assert!(facts.processes.iter().any(|p| p.name == "launchd"));
        let me = std::env::current_exe().unwrap();
        let me = me.to_string_lossy();
        assert!(
            !facts
                .processes
                .iter()
                .any(|p| p.cmdline.starts_with(me.as_ref()))
        );
    }
}
