use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use std::time::{Duration, SystemTime};

use crate::cleanup::model::CleanupPlan;
use crate::config::Config;
use crate::platform::{Facts, MacSystem, MacTrash, ProcessRunner, Runner, System, Trash};
use crate::recommend::build::Analysis;
use crate::rules::guards::Env;
use crate::rules::tools::Tools;
use crate::scanner::tree::Snapshot;

/// Read-only probes (`git status`, `simctl list`, `docker system df`) give up after this.
const PROBE_TIMEOUT: Duration = Duration::from_secs(30);
/// Cleanup commands (`brew cleanup`, `docker system prune`) give up after this (S-05).
const COMMAND_TIMEOUT: Duration = Duration::from_secs(600);

/// A snapshot and its analysis; they are always replaced together.
pub struct Current {
    pub snapshot: Snapshot,
    pub analysis: Analysis,
}

pub struct AppState {
    pub config: Config,
    /// Replaced by each completed scan or snapshot load.
    pub current: RwLock<Option<Arc<Current>>>,
    /// Cancel flag of the running scan, if any. Only one scan runs at a time.
    pub scan: Mutex<Option<Arc<AtomicBool>>>,
    /// Plans by `plan_id`, for the current snapshot only; any new snapshot clears them.
    pub plans: Mutex<HashMap<String, CleanupPlan>>,
    pub plan_seq: AtomicU64,
    /// A cleanup is running. Scans and other cleanups wait for it.
    pub cleaning: AtomicBool,
    pub machine: Machine,
}

impl AppState {
    pub fn new(config: Config) -> Self {
        let machine = Machine::real(&config);
        Self {
            config,
            current: RwLock::new(None),
            scan: Mutex::new(None),
            plans: Mutex::new(HashMap::new()),
            plan_seq: AtomicU64::new(0),
            cleaning: AtomicBool::new(false),
            machine,
        }
    }
}

/// The running processes, command runner and tools the rules engine consults.
pub struct Machine {
    pub system: Box<dyn System>,
    /// Read-only probes for guards and providers.
    pub runner: Box<dyn Runner>,
    /// Cleanup commands.
    pub commands: Box<dyn Runner>,
    pub trash: Box<dyn Trash>,
    /// The real home folder, where tools and their config live.
    home: PathBuf,
    sandbox: bool,
    /// Resolved on first use: the login-shell lookup takes up to a few seconds.
    tools: OnceLock<Tools>,
}

impl Machine {
    pub fn real(config: &Config) -> Self {
        let home = PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
        Self {
            system: Box::new(MacSystem),
            runner: Box::new(ProcessRunner {
                timeout: PROBE_TIMEOUT,
                home: home.display().to_string(),
            }),
            commands: Box::new(ProcessRunner {
                timeout: COMMAND_TIMEOUT,
                home: home.display().to_string(),
            }),
            trash: Box::new(MacTrash),
            home,
            sandbox: config.scan_root.is_some(),
            tools: OnceLock::new(),
        }
    }

    pub fn tools(&self) -> &Tools {
        self.tools
            .get_or_init(|| Tools::discover(&self.home, self.sandbox))
    }

    /// Full Disk Access is a property of this process, so the real home is probed
    /// even when a sandbox is scanned.
    pub fn full_disk_access(&self) -> bool {
        crate::platform::full_disk_access(&self.home)
    }

    pub fn env<'a>(&'a self, facts: &'a Facts) -> Env<'a> {
        Env {
            facts,
            runner: self.runner.as_ref(),
            tools: self.tools(),
            now: SystemTime::now(),
        }
    }
}
