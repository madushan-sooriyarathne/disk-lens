use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::error::{Error, Result};
use crate::platform::Facts;
use crate::recommend::build::analyze;
use crate::recommend::model::{ItemDetails, Recommendation};
use crate::rules::model::Rules;
use crate::scanner::tree::{ScanEvent, Snapshot, SnapshotMeta};
use crate::scanner::{self, ScanOptions, snapshot};
use crate::state::{AppState, Current};
use crate::view::{self, ViewMode, ViewNode};

/// Starts a scan on its own thread and returns immediately; progress and the
/// result arrive through `on_event`, called on that thread.
pub fn start_scan(
    state: &Arc<AppState>,
    mut on_event: impl FnMut(ScanEvent) + Send + 'static,
) -> Result<()> {
    let root = state
        .config
        .root()
        .ok_or_else(|| Error::Root("no home folder".into()))?;
    // Absolute and symlink-free, so paths in views and snapshots are unambiguous.
    let root = std::fs::canonicalize(&root)
        .map_err(|e| Error::Root(format!("{}: {e}", root.display())))?;
    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut running = state.scan.lock().expect("scan lock");
        if running.is_some() {
            return Err(Error::ScanRunning);
        }
        *running = Some(cancel.clone());
    }
    let state = state.clone();
    // A scan blocks for a long time, so it gets a thread rather than a UI executor slot.
    std::thread::spawn(move || {
        let options = ScanOptions::from_rules(Rules::embedded());
        let result = scanner::scan(&root, &options, &cancel, &mut |p| {
            on_event(ScanEvent::Progress {
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
                on_event(ScanEvent::Progress {
                    files: snap.nodes[0].file_count,
                    bytes: snap.meta.scanned_size,
                    current: "Finding recommendations…".into(),
                });
                let meta = snap.meta.clone();
                install(&state, snap);
                ScanEvent::Finished { meta }
            }
            Err(e) => ScanEvent::Failed {
                message: e.to_string(),
            },
        };
        *state.scan.lock().expect("scan lock") = None;
        on_event(event);
    });
    Ok(())
}

/// Analyses `snap` and makes it current.
fn install(state: &AppState, snap: Snapshot) {
    let started = std::time::Instant::now();
    // Match-time guards only look at files, git history and tools.
    let facts = Facts::default();
    let analysis = analyze(Rules::embedded(), &snap, &state.machine.env(&facts));
    tracing::info!(
        "analysed {} items in {:?}",
        analysis.items.len(),
        started.elapsed()
    );
    *state.current.write().expect("current lock") = Some(Arc::new(Current {
        snapshot: snap,
        analysis,
    }));
}

pub fn cancel_scan(state: &Arc<AppState>) {
    if let Some(cancel) = state.scan.lock().expect("scan lock").as_ref() {
        cancel.store(true, Ordering::Relaxed);
    }
}

/// Loads the saved snapshot into state and returns its metadata.
pub fn load_last_snapshot(state: &Arc<AppState>) -> Option<SnapshotMeta> {
    let snap = snapshot::load(&state.config.data_dir)?;
    let meta = snap.meta.clone();
    install(state, snap);
    Some(meta)
}

fn current(state: &AppState) -> Result<Arc<Current>> {
    state
        .current
        .read()
        .expect("current lock")
        .clone()
        .ok_or(Error::NoSnapshot)
}

/// The folder at `node_path` plus two levels below it.
pub fn get_view(state: &Arc<AppState>, node_path: String, mode: ViewMode) -> Result<ViewNode> {
    let cur = current(state)?;
    view::build_view(
        &cur.snapshot,
        Path::new(&node_path),
        mode,
        Some((Rules::embedded(), &cur.analysis)),
    )
}

/// Rows for the current snapshot. Blocking guards are evaluated now, so calling
/// this again (for example on window focus) refreshes blocked states.
pub fn get_recommendations(state: &Arc<AppState>) -> Result<Vec<Recommendation>> {
    let cur = current(state)?;
    let facts = state.machine.system.facts();
    Ok(cur
        .analysis
        .recommendations(Rules::embedded(), &state.machine.env(&facts)))
}

pub fn get_item_details(state: &Arc<AppState>, row_id: String) -> Result<ItemDetails> {
    let cur = current(state)?;
    let facts = state.machine.system.facts();
    cur.analysis
        .details(
            Rules::embedded(),
            &cur.snapshot,
            &state.machine.env(&facts),
            &row_id,
        )
        .ok_or(Error::UnknownRow(row_id))
}

/// Shows a scanned folder or large file in Finder. Paths outside the snapshot are refused.
pub fn reveal_in_finder(state: &Arc<AppState>, node_path: String) -> Result<()> {
    let cur = current(state)?;
    let path = view::validated_path(&cur.snapshot, Path::new(&node_path))?;
    // argv, no shell.
    Command::new("/usr/bin/open").arg("-R").arg(path).status()?;
    Ok(())
}
