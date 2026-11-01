use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use std::time::{Duration, SystemTime};

use crate::cleanup::model::CleanupPlan;
use crate::config::Config;
use crate::license::{self, Status};
use crate::platform::{
    Access, Facts, MacAccess, MacSystem, MacTrash, ProcessRunner, Runner, System, Trash,
};
use crate::recommend::build::Analysis;
use crate::rules::guards::Env;
use crate::rules::tools::Tools;
use crate::scanner::tree::Snapshot;
use crate::settings;

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
    /// The stored license token. Not a verdict: `license()` checks it every time.
    pub license_token: RwLock<Option<String>>,
}

impl AppState {
    pub fn new(config: Config) -> Self {
        let machine = Machine::real(&config);
        let license_token = RwLock::new(settings::load(&config.data_dir).license);
        Self {
            config,
            current: RwLock::new(None),
            scan: Mutex::new(None),
            plans: Mutex::new(HashMap::new()),
            plan_seq: AtomicU64::new(0),
            cleaning: AtomicBool::new(false),
            machine,
            license_token,
        }
    }

    /// Checked against the clock on every call, so a trial ending mid-session is seen.
    pub fn license(&self) -> Status {
        let token = self.license_token.read().expect("license lock");
        license::current_status(token.as_deref())
    }

    pub fn is_pro(&self) -> bool {
        self.license().is_pro()
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
    /// Full Disk Access checks.
    pub access: Box<dyn Access>,
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
            access: Box::new(MacAccess),
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
        crate::platform::full_disk_access(self.access.as_ref(), &self.home)
    }

    /// Makes macOS list DiskLens under Full Disk Access (PRD FR-ONB-3a).
    pub fn preseed_access(&self) {
        crate::platform::preseed_access(self.access.as_ref(), &self.home);
    }

    pub fn env<'a>(&'a self, facts: &'a Facts, pro: bool) -> Env<'a> {
        Env {
            facts,
            runner: self.runner.as_ref(),
            tools: self.tools(),
            now: SystemTime::now(),
            pro,
        }
    }
}
