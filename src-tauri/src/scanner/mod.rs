pub mod hardlinks;
pub mod snapshot;
pub mod tree;
pub mod volume;
pub mod walk;

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use crate::error::Result;
use tree::{Snapshot, SnapshotMeta, VolumeInfo};
pub use walk::Progress;

/// Scan `root` and return the snapshot. Setting `cancel` aborts with `Error::Cancelled`.
pub fn scan(
    root: &Path,
    cancel: &Arc<AtomicBool>,
    on_progress: &mut dyn FnMut(&Progress),
) -> Result<Snapshot> {
    let started = Instant::now();
    let walked = walk::walk(root, cancel, on_progress)?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let meta = SnapshotMeta {
        id: format!("{:x}", now.as_nanos()),
        root: root.display().to_string(),
        scanned_at: now.as_secs() as i64,
        duration_ms: started.elapsed().as_millis() as u64,
        volume: volume::volume_info(root).unwrap_or(VolumeInfo {
            total: 0,
            available: 0,
        }),
        scanned_size: walked.nodes.first().map_or(0, |n| n.size),
        inaccessible_count: walked.inaccessible_count,
    };
    Ok(Snapshot {
        meta,
        nodes: walked.nodes,
    })
}
