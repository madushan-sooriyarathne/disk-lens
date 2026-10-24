use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use std::time::{Duration, SystemTime};

use crate::config::Config;
use crate::platform::{Facts, MacSystem, ProcessRunner, Runner, System};
use crate::recommend::build::Analysis;
use crate::rules::guards::Env;
use crate::rules::tools::Tools;
use crate::scanner::tree::Snapshot;

/// Read-only probes (`git status`, `simctl list`, `docker system df`) give up after this.
const PROBE_TIMEOUT: Duration = Duration::from_secs(30);

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
    pub machine: Machine,
}

impl AppState {
    pub fn new(config: Config) -> Self {
        let machine = Machine::real(&config);
        Self {
            config,
            current: RwLock::new(None),
            scan: Mutex::new(None),
            machine,
        }
    }
}

/// The running processes, command runner and tools the rules engine consults.
pub struct Machine {
    pub system: Box<dyn System>,
    pub runner: Box<dyn Runner>,
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
            home,
            sandbox: config.scan_root.is_some(),
            tools: OnceLock::new(),
        }
    }

    pub fn tools(&self) -> &Tools {
        self.tools
            .get_or_init(|| Tools::discover(&self.home, self.sandbox))
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
