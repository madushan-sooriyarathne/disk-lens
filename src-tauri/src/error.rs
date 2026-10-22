use serde::Serialize;

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
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("snapshot error: {0}")]
    Snapshot(String),
}

pub type Result<T> = std::result::Result<T, Error>;

/// Tauri commands return the message string to the frontend.
impl Serialize for Error {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}
