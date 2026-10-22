use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use tauri::State;
use tauri::ipc::Channel;

use crate::error::{Error, Result};
use crate::scanner::tree::{ScanEvent, SnapshotMeta};
use crate::scanner::{self, snapshot};
use crate::state::AppState;

/// Starts a scan on a blocking thread and returns immediately; progress and the
/// result arrive on `channel`.
#[tauri::command]
pub fn start_scan(state: State<'_, Arc<AppState>>, channel: Channel<ScanEvent>) -> Result<()> {
    let root = state
        .config
        .root()
        .ok_or_else(|| Error::Root("no home folder".into()))?;
    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut running = state.scan.lock().expect("scan lock");
        if running.is_some() {
            return Err(Error::ScanRunning);
        }
        *running = Some(cancel.clone());
    }
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let progress = channel.clone();
        let result = scanner::scan(&root, &cancel, &mut |p| {
            let _ = progress.send(ScanEvent::Progress {
                files: p.files,
                bytes: p.bytes,
                current: p.current.clone(),
            });
        });
        let event = match result {
            Ok(snap) => {
                if let Err(e) = snapshot::save(&state.config.data_dir, &snap) {
                    tracing::warn!("could not save snapshot: {e}");
                }
                let meta = snap.meta.clone();
                *state.snapshot.write().expect("snapshot lock") = Some(Arc::new(snap));
                ScanEvent::Finished { meta }
            }
            Err(e) => ScanEvent::Failed {
                message: e.to_string(),
            },
        };
        *state.scan.lock().expect("scan lock") = None;
        let _ = channel.send(event);
    });
    Ok(())
}

#[tauri::command]
pub fn cancel_scan(state: State<'_, Arc<AppState>>) {
    if let Some(cancel) = state.scan.lock().expect("scan lock").as_ref() {
        cancel.store(true, Ordering::Relaxed);
    }
}

/// Loads the saved snapshot into state and returns its metadata.
#[tauri::command]
pub fn load_last_snapshot(state: State<'_, Arc<AppState>>) -> Option<SnapshotMeta> {
    let snap = snapshot::load(&state.config.data_dir)?;
    let meta = snap.meta.clone();
    *state.snapshot.write().expect("snapshot lock") = Some(Arc::new(snap));
    Some(meta)
}
