//! Guards (RULES.md section 4.2).
//!
//! Match-time guards decide whether a rule matches at all. Blocking and safety
//! guards keep a matched item visible but unselectable, with a short reason.
//! Every check here is read-only. When a check cannot be completed the item is
//! treated as blocked or not matched, never as ready.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::model::Guards;
use super::tools::Tools;
use crate::platform::{Facts, Runner};

const LOCKFILES: [&str; 5] = [
    "package-lock.json",
    "pnpm-lock.yaml",
    "yarn.lock",
    "bun.lock",
    "bun.lockb",
];

/// Friendly names for the process substrings used in rules.toml.
const PROCESS_NAMES: [(&str, &str); 10] = [
    ("GradleDaemon", "the Gradle daemon"),
    ("xcodebuild", "xcodebuild"),
    ("qemu-system", "the Android emulator"),
    ("uv ", "uv"),
    ("brew ", "Homebrew"),
    ("pnpm", "pnpm"),
    ("cargo", "Cargo"),
    ("LM Studio", "LM Studio"),
    ("com.docker.backend", "Docker Desktop"),
    ("ollama", "Ollama"),
];

/// What guards may look at.
pub struct Env<'a> {
    pub facts: &'a Facts,
    pub runner: &'a dyn Runner,
    pub tools: &'a Tools,
    pub now: SystemTime,
}

// ---------------------------------------------------------------------------
// Match-time guards
// ---------------------------------------------------------------------------

/// Any of `names` exists in the folder that contains `path`.
pub fn sibling_exists(path: &Path, names: &[String]) -> bool {
    let Some(parent) = path.parent() else {
        return false;
    };
    names
        .iter()
        .any(|n| parent.join(n).symlink_metadata().is_ok())
}

/// The project containing `path` (its parent folder) has been idle for `days`.
/// Idle time is measured from the newest of the last commit, `package.json` and
/// any lockfile. With no evidence at all the project does not count as idle.
pub fn project_idle(path: &Path, days: u32, env: &Env) -> bool {
    let Some(project) = path.parent() else {
        return false;
    };
    let mut newest: Option<SystemTime> = None;
    let mut consider = |t: Option<SystemTime>| {
        if let Some(t) = t {
            newest = Some(newest.map_or(t, |n| n.max(t)));
        }
    };
    consider(mtime(&project.join("package.json")));
    for lock in LOCKFILES {
        consider(mtime(&project.join(lock)));
    }
    if let Some(git) = Git::new(env) {
        consider(git.last_commit(project));
    }
    newest.is_some_and(|t| age(env.now, t) >= Duration::from_secs(u64::from(days) * 86_400))
}

// ---------------------------------------------------------------------------
// Blocking and safety guards
// ---------------------------------------------------------------------------

/// Why the item at `path` cannot be cleaned right now, or `None` when it can.
pub fn blocked(g: &Guards, path: &Path, env: &Env) -> Option<String> {
    let facts = env.facts;
    for id in &g.app_not_running {
        if let Some(app) = facts.apps.iter().find(|a| &a.bundle_id == id) {
            return Some(format!("Quit {} to clean", app.name));
        }
    }
    if g.owner_app_not_running
        && let Some(owner) = path.file_name().map(|n| n.to_string_lossy())
    {
        if let Some(app) = facts
            .apps
            .iter()
            .find(|a| a.bundle_id.eq_ignore_ascii_case(&owner))
        {
            return Some(format!("Quit {} to clean", app.name));
        }
        if facts.processes.iter().any(|p| p.name == owner) {
            return Some(format!("Quit {owner} to clean"));
        }
    }
    for s in &g.process_not_running {
        if facts
            .processes
            .iter()
            .any(|p| p.cmdline.contains(s.as_str()))
        {
            return Some(format!("Quit {} to clean", process_name(s)));
        }
    }
    for s in &g.requires_running {
        if !facts
            .processes
            .iter()
            .any(|p| p.cmdline.contains(s.as_str()))
        {
            return Some(format!("Start {} to clean", process_name(s)));
        }
    }
    if g.git_clean
        && let Some(reason) = git_unclean(path, env)
    {
        return Some(reason);
    }
    if let Some(minutes) = g.quiet_minutes
        && changed_within(path, Duration::from_secs(u64::from(minutes) * 60), env.now)
    {
        return Some(format!("Changed in the last {minutes} minutes"));
    }
    None
}

fn process_name(substring: &str) -> &str {
    PROCESS_NAMES
        .iter()
        .find(|(s, _)| *s == substring)
        .map_or(substring.trim(), |(_, name)| name)
}

/// Uncommitted or untracked changes, or commits not on any remote branch.
fn git_unclean(path: &Path, env: &Env) -> Option<String> {
    let Some(git) = Git::new(env) else {
        return Some("Git not found".into());
    };
    match (git.dirty(path), git.unpushed(path)) {
        (Some(true), _) => Some("Has uncommitted changes".into()),
        (Some(false), Some(true)) => Some("Has commits not on a remote".into()),
        (Some(false), Some(false)) => None,
        _ => Some("Could not check git status".into()),
    }
}

/// `path` or one of its immediate children was modified within `window`.
/// A modification time in the future counts as recent.
fn changed_within(path: &Path, window: Duration, now: SystemTime) -> bool {
    let recent = |p: &Path| mtime(p).is_some_and(|t| age(now, t) < window);
    if recent(path) {
        return true;
    }
    std::fs::read_dir(path).is_ok_and(|entries| entries.flatten().any(|e| recent(&e.path())))
}

fn mtime(p: &Path) -> Option<SystemTime> {
    p.symlink_metadata().and_then(|m| m.modified()).ok()
}

/// Zero for times in the future.
fn age(now: SystemTime, t: SystemTime) -> Duration {
    now.duration_since(t).unwrap_or_default()
}

/// Read-only git queries through the resolved binary.
struct Git<'a> {
    bin: PathBuf,
    runner: &'a dyn Runner,
}

impl<'a> Git<'a> {
    fn new(env: &Env<'a>) -> Option<Self> {
        Some(Self {
            bin: env.tools.resolve("git")?,
            runner: env.runner,
        })
    }

    /// stdout of a successful `git -C <dir> <args>`.
    fn query(&self, dir: &Path, args: &[&str]) -> Option<String> {
        let dir = dir.to_str()?;
        let argv: Vec<&str> = ["-C", dir].iter().chain(args).copied().collect();
        let out = self.runner.run(&self.bin, &argv).ok()?;
        out.success.then_some(out.stdout)
    }

    fn last_commit(&self, dir: &Path) -> Option<SystemTime> {
        let secs: u64 = self
            .query(dir, &["log", "-1", "--format=%ct"])?
            .trim()
            .parse()
            .ok()?;
        Some(UNIX_EPOCH + Duration::from_secs(secs))
    }

    fn dirty(&self, dir: &Path) -> Option<bool> {
        Some(
            !self
                .query(dir, &["status", "--porcelain"])?
                .trim()
                .is_empty(),
        )
    }

    fn unpushed(&self, dir: &Path) -> Option<bool> {
        let out = self.query(
            dir,
            &["rev-list", "--max-count=1", "HEAD", "--not", "--remotes"],
        )?;
        Some(!out.trim().is_empty())
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::platform::{App, FakeRunner, Proc};

    const DAY: u64 = 86_400;

    fn proc(cmdline: &str) -> Proc {
        Proc {
            name: cmdline
                .split(' ')
                .next()
                .unwrap()
                .rsplit('/')
                .next()
                .unwrap()
                .into(),
            cmdline: cmdline.into(),
        }
    }

    fn app(bundle_id: &str, name: &str) -> App {
        App {
            bundle_id: bundle_id.into(),
            name: name.into(),
        }
    }

    fn git_tools(t: &Path) -> Tools {
        use std::os::unix::fs::PermissionsExt;
        let bin = t.join("bin");
        fs::create_dir_all(&bin).unwrap();
        fs::write(bin.join("git"), "").unwrap();
        fs::set_permissions(bin.join("git"), fs::Permissions::from_mode(0o755)).unwrap();
        Tools::new(vec![bin], true)
    }

    fn no_tools() -> Tools {
        Tools::new(vec![], false)
    }

    /// A folder whose contents were last changed `secs` ago.
    fn aged(t: &Path, name: &str, secs: u64) -> PathBuf {
        let dir = t.join(name);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("f"), "").unwrap();
        let when = SystemTime::now() - Duration::from_secs(secs);
        for p in [dir.join("f"), dir.clone()] {
            fs::File::open(&p).unwrap().set_modified(when).unwrap();
        }
        dir
    }

    struct Case {
        name: &'static str,
        guards: Guards,
        facts: Facts,
        expect: Option<&'static str>,
    }

    #[test]
    fn blocking_guards_table() {
        let t = tempfile::tempdir().unwrap();
        let old = aged(t.path(), "com.example.app", 3600);
        let runner = FakeRunner::default();
        let tools = no_tools();
        let g = |f: fn(&mut Guards)| {
            let mut g = Guards::default();
            f(&mut g);
            g
        };
        let cases = [
            Case {
                name: "no guards",
                guards: Guards::default(),
                facts: Facts::default(),
                expect: None,
            },
            Case {
                name: "app running",
                guards: g(|g| g.app_not_running = vec!["com.apple.dt.Xcode".into()]),
                facts: Facts {
                    apps: vec![app("com.apple.dt.Xcode", "Xcode")],
                    ..Facts::default()
                },
                expect: Some("Quit Xcode to clean"),
            },
            Case {
                name: "other app running",
                guards: g(|g| g.app_not_running = vec!["com.apple.dt.Xcode".into()]),
                facts: Facts {
                    apps: vec![app("com.apple.Safari", "Safari")],
                    ..Facts::default()
                },
                expect: None,
            },
            Case {
                name: "owner app by bundle id",
                guards: g(|g| g.owner_app_not_running = true),
                facts: Facts {
                    apps: vec![app("com.Example.App", "Example")],
                    ..Facts::default()
                },
                expect: Some("Quit Example to clean"),
            },
            Case {
                name: "owner process by name",
                guards: g(|g| g.owner_app_not_running = true),
                facts: Facts {
                    processes: vec![proc("/opt/x/com.example.app --flag")],
                    ..Facts::default()
                },
                expect: Some("Quit com.example.app to clean"),
            },
            Case {
                name: "process substring",
                guards: g(|g| g.process_not_running = vec!["GradleDaemon".into()]),
                facts: Facts {
                    processes: vec![proc(
                        "java -cp x org.gradle.launcher.daemon.GradleDaemon 8.0",
                    )],
                    ..Facts::default()
                },
                expect: Some("Quit the Gradle daemon to clean"),
            },
            Case {
                name: "process substring with trailing space",
                guards: g(|g| g.process_not_running = vec!["uv ".into()]),
                facts: Facts {
                    processes: vec![proc("/usr/bin/uvicorn app")],
                    ..Facts::default()
                },
                expect: None,
            },
            Case {
                name: "required process missing",
                guards: g(|g| g.requires_running = vec!["com.docker.backend".into()]),
                facts: Facts::default(),
                expect: Some("Start Docker Desktop to clean"),
            },
            Case {
                name: "required process running",
                guards: g(|g| g.requires_running = vec!["ollama".into()]),
                facts: Facts {
                    processes: vec![proc("/usr/local/bin/ollama serve")],
                    ..Facts::default()
                },
                expect: None,
            },
            Case {
                name: "quiet long enough",
                guards: g(|g| g.quiet_minutes = Some(10)),
                facts: Facts::default(),
                expect: None,
            },
            Case {
                name: "app wins over quiet",
                guards: g(|g| {
                    g.quiet_minutes = Some(120);
                    g.app_not_running = vec!["com.apple.dt.Xcode".into()];
                }),
                facts: Facts {
                    apps: vec![app("com.apple.dt.Xcode", "Xcode")],
                    ..Facts::default()
                },
                expect: Some("Quit Xcode to clean"),
            },
            Case {
                name: "changed recently",
                guards: g(|g| g.quiet_minutes = Some(120)),
                facts: Facts::default(),
                expect: Some("Changed in the last 120 minutes"),
            },
        ];
        for c in cases {
            let env = Env {
                facts: &c.facts,
                runner: &runner,
                tools: &tools,
                now: SystemTime::now(),
            };
            assert_eq!(
                blocked(&c.guards, &old, &env).as_deref(),
                c.expect,
                "{}",
                c.name
            );
        }
    }

    #[test]
    fn quiet_checks_immediate_children_and_future_times() {
        let t = tempfile::tempdir().unwrap();
        let dir = aged(t.path(), "cache", 3600);
        let facts = Facts::default();
        let (runner, tools) = (FakeRunner::default(), no_tools());
        let mut env = Env {
            facts: &facts,
            runner: &runner,
            tools: &tools,
            now: SystemTime::now(),
        };
        let g = Guards {
            quiet_minutes: Some(10),
            ..Guards::default()
        };
        assert_eq!(blocked(&g, &dir, &env), None);
        fs::write(dir.join("fresh"), "").unwrap();
        assert!(blocked(&g, &dir, &env).is_some(), "fresh child");
        fs::remove_file(dir.join("fresh")).unwrap();
        fs::File::open(&dir)
            .unwrap()
            .set_modified(SystemTime::now() - Duration::from_secs(3600))
            .unwrap();
        env.now = SystemTime::now() - Duration::from_secs(7200); // clock behind the files
        assert!(blocked(&g, &dir, &env).is_some(), "future mtime");
    }

    #[test]
    fn git_clean_states() {
        let t = tempfile::tempdir().unwrap();
        let tools = git_tools(t.path());
        let wt = t.path().join("wt");
        let w = wt.to_str().unwrap();
        let g = Guards {
            git_clean: true,
            ..Guards::default()
        };
        let facts = Facts::default();
        let check = |runner: FakeRunner| {
            let env = Env {
                facts: &facts,
                runner: &runner,
                tools: &tools,
                now: SystemTime::now(),
            };
            blocked(&g, &wt, &env)
        };
        let status = format!("git -C {w} status --porcelain");
        let revs = format!("git -C {w} rev-list --max-count=1 HEAD --not --remotes");
        assert_eq!(
            check(
                FakeRunner::default()
                    .with(&status, true, "")
                    .with(&revs, true, "")
            ),
            None
        );
        assert_eq!(
            check(
                FakeRunner::default()
                    .with(&status, true, "?? new.txt\n")
                    .with(&revs, true, "")
            ),
            Some("Has uncommitted changes".into())
        );
        assert_eq!(
            check(
                FakeRunner::default()
                    .with(&status, true, "")
                    .with(&revs, true, "abc123\n")
            ),
            Some("Has commits not on a remote".into())
        );
        assert_eq!(
            check(FakeRunner::default().with(&status, false, "")),
            Some("Could not check git status".into())
        );
        let env = Env {
            facts: &facts,
            runner: &FakeRunner::default(),
            tools: &no_tools(),
            now: SystemTime::now(),
        };
        assert_eq!(blocked(&g, &wt, &env), Some("Git not found".into()));
    }

    #[test]
    fn sibling_exists_looks_in_parent() {
        let t = tempfile::tempdir().unwrap();
        fs::create_dir_all(t.path().join("app/.next")).unwrap();
        fs::write(t.path().join("app/turbo.json"), "").unwrap();
        let next = t.path().join("app/.next");
        assert!(sibling_exists(
            &next,
            &["package.json".into(), "turbo.json".into()]
        ));
        assert!(!sibling_exists(&next, &["package.json".into()]));
    }

    #[test]
    fn project_idle_uses_newest_evidence() {
        let t = tempfile::tempdir().unwrap();
        let tools = git_tools(t.path());
        let facts = Facts::default();
        let proj = aged(t.path(), "proj", 90 * DAY);
        fs::rename(proj.join("f"), proj.join("package.json")).unwrap();
        let nm = proj.join("node_modules");
        let p = proj.to_str().unwrap();
        let log = format!("git -C {p} log -1 --format=%ct");
        let now = SystemTime::now();
        let secs_ago = |d: u64| {
            (now - Duration::from_secs(d * DAY))
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs()
                .to_string()
        };
        let idle = |runner: &FakeRunner| {
            let env = Env {
                facts: &facts,
                runner,
                tools: &tools,
                now,
            };
            project_idle(&nm, 30, &env)
        };
        // Not a git repo: only package.json (90 days) counts.
        assert!(idle(&FakeRunner::default()));
        // A commit 3 days ago makes it active.
        assert!(!idle(&FakeRunner::default().with(&log, true, &secs_ago(3))));
        assert!(idle(&FakeRunner::default().with(&log, true, &secs_ago(45))));
        // A fresh lockfile makes it active.
        fs::write(proj.join("pnpm-lock.yaml"), "").unwrap();
        assert!(!idle(&FakeRunner::default()));
    }

    #[test]
    fn project_without_evidence_is_not_idle() {
        let t = tempfile::tempdir().unwrap();
        fs::create_dir_all(t.path().join("p/node_modules")).unwrap();
        let facts = Facts::default();
        let env = Env {
            facts: &facts,
            runner: &FakeRunner::default(),
            tools: &no_tools(),
            now: SystemTime::now() + Duration::from_secs(365 * DAY),
        };
        assert!(!project_idle(&t.path().join("p/node_modules"), 30, &env));
    }
}
