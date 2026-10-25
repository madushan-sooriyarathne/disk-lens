//! Audit log (S-07): one JSON line per action with timestamp, rule, path,
//! method, bytes and result. Dry runs are logged the same way (S-08).

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    /// Milliseconds since the Unix epoch.
    pub ts: u64,
    pub plan_id: String,
    pub item_id: String,
    pub rule: String,
    pub path: String,
    /// `PlannedMethod::as_str`.
    pub method: String,
    /// The command, for command actions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub argv: Option<Vec<String>>,
    /// Bytes freed or moved to the Trash (in a dry run: that would have been).
    pub bytes: u64,
    /// `deleted`, `trashed`, `command_ran`, `dry_run`, `skipped` or `failed`.
    pub result: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    pub dry_run: bool,
}

pub struct Audit {
    path: PathBuf,
    file: File,
}

impl Audit {
    /// Opens `<dir>/cleanup.log` for appending, creating the folder.
    pub fn open(dir: &Path) -> io::Result<Self> {
        fs::create_dir_all(dir)?;
        let path = dir.join("cleanup.log");
        let file = OpenOptions::new().create(true).append(true).open(&path)?;
        Ok(Self { path, file })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Appends `entry` and flushes it, so a crash loses nothing already done.
    pub fn record(&mut self, entry: &Entry) -> io::Result<()> {
        let mut line = serde_json::to_string(entry).map_err(io::Error::other)?;
        line.push('\n');
        self.file.write_all(line.as_bytes())?;
        self.file.sync_data()
    }
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

/// Every entry in the log at `path`.
pub fn read(path: &Path) -> io::Result<Vec<Entry>> {
    fs::read_to_string(path)?
        .lines()
        .map(|l| serde_json::from_str(l).map_err(io::Error::other))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(item: &str) -> Entry {
        Entry {
            ts: 1,
            plan_id: "p".into(),
            item_id: item.into(),
            rule: "npm-cache".into(),
            path: "/h/.npm/_cacache".into(),
            method: "delete".into(),
            argv: None,
            bytes: 10,
            result: "dry_run".into(),
            detail: None,
            dry_run: true,
        }
    }

    #[test]
    fn appends_entries_across_opens() {
        let t = tempfile::tempdir().unwrap();
        let dir = t.path().join("Logs/DiskLens");
        Audit::open(&dir).unwrap().record(&entry("a")).unwrap();
        Audit::open(&dir).unwrap().record(&entry("b")).unwrap();
        let ids: Vec<String> = read(&dir.join("cleanup.log"))
            .unwrap()
            .into_iter()
            .map(|e| e.item_id)
            .collect();
        assert_eq!(ids, ["a", "b"]);
    }
}
