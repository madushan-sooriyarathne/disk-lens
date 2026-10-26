#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("scan already running")]
    ScanRunning,
    #[error("scan cancelled")]
    Cancelled,
    #[error("scan root unavailable: {0}")]
    Root(String),
    #[error("no scan loaded")]
    NoSnapshot,
    #[error("no recommendation {0} in the current scan")]
    UnknownRow(String),
    #[error("path is not part of the scan: {0}")]
    NotInSnapshot(String),
    #[error("the scan changed since these items were listed; review the selection again")]
    StaleSnapshot,
    #[error("no cleanup plan {0}; build it again")]
    UnknownPlan(String),
    #[error("cleanup already running")]
    CleanupRunning,
    #[error("audit log unavailable: {0}")]
    Audit(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("snapshot error: {0}")]
    Snapshot(String),
    #[error("settings error: {0}")]
    Settings(String),
    #[error("{0}")]
    Update(String),
}

pub type Result<T> = std::result::Result<T, Error>;
