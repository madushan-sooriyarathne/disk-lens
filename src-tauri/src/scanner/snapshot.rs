//! Snapshot persistence: JSON compressed with zstd, written atomically.

use std::fs;
use std::path::{Path, PathBuf};

use super::tree::Snapshot;
use crate::error::{Error, Result};

const FILE_NAME: &str = "snapshot.json.zst";
const ZSTD_LEVEL: i32 = 3;

pub fn snapshot_path(data_dir: &Path) -> PathBuf {
    data_dir.join(FILE_NAME)
}

pub fn save(data_dir: &Path, snapshot: &Snapshot) -> Result<()> {
    fs::create_dir_all(data_dir)?;
    let json = serde_json::to_vec(snapshot).map_err(|e| Error::Snapshot(e.to_string()))?;
    let packed = zstd::encode_all(json.as_slice(), ZSTD_LEVEL)?;
    let path = snapshot_path(data_dir);
    let tmp = path.with_extension("zst.tmp");
    fs::write(&tmp, packed)?;
    fs::rename(&tmp, &path)?; // atomic on the same volume
    Ok(())
}

/// `None` when there is no snapshot or it is unreadable; a corrupt file never panics.
pub fn load(data_dir: &Path) -> Option<Snapshot> {
    let packed = fs::read(snapshot_path(data_dir)).ok()?;
    let parsed = zstd::decode_all(packed.as_slice())
        .map_err(|e| e.to_string())
        .and_then(|json| serde_json::from_slice(&json).map_err(|e| e.to_string()));
    match parsed {
        Ok(s) => Some(s),
        Err(e) => {
            tracing::warn!("ignoring unreadable snapshot: {e}");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::tree::{DirNode, NodeFlags, SnapshotMeta, VolumeInfo};
    use super::*;

    fn sample() -> Snapshot {
        Snapshot {
            meta: SnapshotMeta {
                id: "s1".into(),
                root: "/r".into(),
                scanned_at: 1_700_000_000,
                duration_ms: 12,
                volume: VolumeInfo {
                    total: 100,
                    available: 40,
                },
                scanned_size: 8192,
                inaccessible_count: 1,
            },
            nodes: vec![DirNode {
                id: 0,
                parent: None,
                name: "r".into(),
                size: 8192,
                logical_size: 5000,
                file_count: 2,
                children: vec![],
                top_files: vec![],
                flags: NodeFlags::default(),
                category: String::new(),
            }],
        }
    }

    #[test]
    fn round_trips() {
        let dir = tempfile::tempdir().unwrap();
        save(dir.path(), &sample()).unwrap();
        assert_eq!(load(dir.path()), Some(sample()));
    }

    #[test]
    fn missing_or_corrupt_is_none() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(load(dir.path()), None);
        fs::write(snapshot_path(dir.path()), b"not zstd").unwrap();
        assert_eq!(load(dir.path()), None);
    }

    #[test]
    fn save_overwrites_without_leaving_temp_file() {
        let dir = tempfile::tempdir().unwrap();
        save(dir.path(), &sample()).unwrap();
        save(dir.path(), &sample()).unwrap();
        let names: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, vec![std::ffi::OsString::from(FILE_NAME)]);
    }
}
