//! The backend functions end to end on a temporary scan root under
//! `target/tmp`: scan, plan, script, execute, audit, and plan invalidation.

mod support;

use std::sync::Arc;
use std::sync::mpsc;
use std::time::Duration;

use disklens_lib::backend;
use disklens_lib::cleanup::audit;
use disklens_lib::cleanup::model::{CleanupEvent, CleanupPlan, CleanupSummary};
use disklens_lib::config::Config;
use disklens_lib::error::Error;
use disklens_lib::scanner::tree::ScanEvent;
use disklens_lib::state::AppState;
use support::World;

const WAIT: Duration = Duration::from_secs(60);

fn state(w: &World, dry_run: bool) -> Arc<AppState> {
    let state = Arc::new(AppState::new(Config {
        scan_root: Some(w.home.clone()),
        dry_run,
        data_dir: w.home.with_file_name("data"),
        log_dir: w.logs.clone(),
    }));
    scan(&state);
    state
}

fn scan(state: &Arc<AppState>) -> String {
    let (tx, rx) = mpsc::channel();
    backend::start_scan(state, move |e| {
        if let ScanEvent::Finished { meta } = e {
            tx.send(meta.id).unwrap();
        }
    })
    .unwrap();
    rx.recv_timeout(WAIT).unwrap()
}

fn npm_plan(state: &Arc<AppState>) -> CleanupPlan {
    let snapshot_id = state
        .current
        .read()
        .unwrap()
        .as_ref()
        .unwrap()
        .snapshot
        .meta
        .id
        .clone();
    let ids: Vec<String> = backend::get_recommendations(state)
        .unwrap()
        .into_iter()
        .filter(|r| r.rule_id == "npm-cache")
        .flat_map(|r| r.items.into_iter().map(|i| i.item_id))
        .collect();
    backend::build_cleanup_plan(state, snapshot_id, ids, false).unwrap()
}

fn run(state: &Arc<AppState>, plan_id: &str) -> CleanupSummary {
    let (tx, rx) = mpsc::channel();
    backend::execute_cleanup(state, plan_id.into(), move |e| {
        if let CleanupEvent::Finished { summary } = e {
            tx.send(summary).unwrap();
        }
    })
    .unwrap();
    rx.recv_timeout(WAIT).unwrap()
}

fn world() -> World {
    let w = World::new();
    w.put(".npm/_cacache", 2);
    w
}

#[test]
fn execute_cleanup_deletes_the_planned_items() {
    let w = world();
    let state = state(&w, false);
    let plan = npm_plan(&state);
    run(&state, &plan.plan_id);
    assert!(!w.exists(".npm/_cacache"));
}

#[test]
fn execute_cleanup_writes_the_audit_log_in_log_dir() {
    let w = world();
    let state = state(&w, true);
    let plan = npm_plan(&state);
    run(&state, &plan.plan_id);
    let entries = audit::read(&w.logs.join("cleanup.log")).unwrap();
    assert_eq!(entries[0].item_id, plan.actions[0].item_id);
}

#[test]
fn dry_run_leaves_the_items_in_place() {
    let w = world();
    let state = state(&w, true);
    let plan = npm_plan(&state);
    run(&state, &plan.plan_id);
    assert!(w.exists(".npm/_cacache"));
}

#[test]
fn a_plan_runs_only_once() {
    let w = world();
    let state = state(&w, true);
    let plan = npm_plan(&state);
    run(&state, &plan.plan_id);
    let again = backend::execute_cleanup(&state, plan.plan_id, |_| {});
    assert!(matches!(again, Err(Error::UnknownPlan(_))));
}

#[test]
fn a_rescan_invalidates_plans() {
    let w = world();
    let state = state(&w, false);
    let plan = npm_plan(&state);
    scan(&state);
    let result = backend::execute_cleanup(&state, plan.plan_id, |_| {});
    assert!(matches!(result, Err(Error::UnknownPlan(_))));
}

#[test]
fn export_script_renders_the_stored_plan() {
    let w = world();
    let state = state(&w, true);
    let plan = npm_plan(&state);
    let script = backend::export_script(&state, &plan.plan_id).unwrap();
    assert!(script.contains(&format!("# Item {}", plan.actions[0].item_id)));
}
