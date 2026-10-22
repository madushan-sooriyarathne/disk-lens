use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, RwLock};

use crate::config::Config;
use crate::scanner::tree::Snapshot;

pub struct AppState {
    pub config: Config,
    /// The current snapshot; replaced by each completed scan.
    pub snapshot: RwLock<Option<Arc<Snapshot>>>,
    /// Cancel flag of the running scan, if any. Only one scan runs at a time.
    pub scan: Mutex<Option<Arc<AtomicBool>>>,
}

impl AppState {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            snapshot: RwLock::new(None),
            scan: Mutex::new(None),
        }
    }
}
