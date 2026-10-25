use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::cleanup::audit::Audit;
use crate::cleanup::execute::{Engine, execute};
use crate::cleanup::model::{CleanupEvent, CleanupPlan};
use crate::cleanup::plan::{self, Request};
use crate::cleanup::revalidate::GROWTH_FLOOR;
use crate::cleanup::script;
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
        if state.cleaning.load(Ordering::SeqCst) {
            return Err(Error::CleanupRunning);
        }
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
    // Plans name items of the previous snapshot.
    state.plans.lock().expect("plans lock").clear();
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

/// Plans cleaning `item_ids`, selected in snapshot `snapshot_id`. The plan is
/// kept for `export_script` and `execute_cleanup` until the next snapshot.
pub fn build_cleanup_plan(
    state: &Arc<AppState>,
    snapshot_id: String,
    item_ids: Vec<String>,
    permanent_review: bool,
) -> Result<CleanupPlan> {
    let cur = current(state)?;
    let facts = state.machine.system.facts();
    let seq = state.plan_seq.fetch_add(1, Ordering::Relaxed);
    let plan = plan::build(
        format!("{}-{seq}", cur.analysis.snapshot_id),
        Rules::embedded(),
        &cur.analysis,
        &state.machine.env(&facts),
        &Request {
            snapshot_id: &snapshot_id,
            item_ids: &item_ids,
            permanent_review,
        },
    )?;
    state
        .plans
        .lock()
        .expect("plans lock")
        .insert(plan.plan_id.clone(), plan.clone());
    Ok(plan)
}

/// The bash script equivalent of a stored plan (PRD section 7.7).
pub fn export_script(state: &Arc<AppState>, plan_id: &str) -> Result<String> {
    let plans = state.plans.lock().expect("plans lock");
    let plan = plans
        .get(plan_id)
        .ok_or_else(|| Error::UnknownPlan(plan_id.into()))?;
    Ok(script::render(
        plan,
        crate::cleanup::audit::now_ms() as i64 / 1000,
    ))
}

/// Runs a stored plan on its own thread and returns immediately; events
/// arrive through `on_event` on that thread. A plan runs at most once, and
/// only while its snapshot is current.
pub fn execute_cleanup(
    state: &Arc<AppState>,
    plan_id: String,
    mut on_event: impl FnMut(CleanupEvent) + Send + 'static,
) -> Result<()> {
    let cur = current(state)?;
    // Holding the scan lock keeps a scan from starting while this one claims the flag.
    let scan = state.scan.lock().expect("scan lock");
    if scan.is_some() {
        return Err(Error::ScanRunning);
    }
    if state.cleaning.swap(true, Ordering::SeqCst) {
        return Err(Error::CleanupRunning);
    }
    drop(scan);
    let release = |e: Error| {
        state.cleaning.store(false, Ordering::SeqCst);
        Err(e)
    };
    let Some(plan) = state.plans.lock().expect("plans lock").remove(&plan_id) else {
        return release(Error::UnknownPlan(plan_id));
    };
    if plan.snapshot_id != cur.snapshot.meta.id {
        return release(Error::StaleSnapshot);
    }
    let mut audit = match Audit::open(&state.config.log_dir) {
        Ok(a) => a,
        Err(e) => {
            return release(Error::Audit(format!(
                "{}: {e}",
                state.config.log_dir.display()
            )));
        }
    };
    let state = state.clone();
    std::thread::spawn(move || {
        let root = PathBuf::from(&cur.snapshot.meta.root);
        let machine = &state.machine;
        let engine = Engine {
            rules: Rules::embedded(),
            root: &root,
            system: machine.system.as_ref(),
            probes: machine.runner.as_ref(),
            commands: machine.commands.as_ref(),
            tools: machine.tools(),
            trash: machine.trash.as_ref(),
            dry_run: state.config.dry_run,
            growth_floor: GROWTH_FLOOR,
        };
        tracing::info!(
            "cleanup {} of {} items (dry run: {})",
            plan.plan_id,
            plan.actions.len(),
            engine.dry_run
        );
        // The flag is cleared before Finished goes out, so the rescan it triggers may start.
        let mut finished = None;
        execute(&engine, &plan, &mut audit, &mut |event| match event {
            CleanupEvent::Finished { .. } => finished = Some(event),
            other => on_event(other),
        });
        state.cleaning.store(false, Ordering::SeqCst);
        if let Some(event) = finished {
            on_event(event);
        }
    });
    Ok(())
}
