//! Plans and execution on a temporary scan root under `target/tmp`: what each
//! method does, what the plan totals say, and what the summary reports.

mod support;

use disklens_lib::cleanup::model::PlannedMethod;
use support::{MB, World};

fn methods(w: &World, rule_ids: &[&str], permanent: bool) -> Vec<PlannedMethod> {
    let ids: Vec<String> = rule_ids.iter().flat_map(|r| w.items(r)).collect();
    w.try_plan(&w.analysis().snapshot_id, &ids, permanent)
        .unwrap()
        .actions
        .into_iter()
        .map(|a| a.method)
        .collect()
}

fn review_world() -> World {
    let mut w = World::new();
    w.put(".cache/huggingface/hub", 2);
    w.put("Library/Application Support/MobileSync/Backup/device-1", 3);
    w.put(".Trash/old", 2);
    w.scan();
    w
}

#[test]
fn delete_removes_the_folder() {
    let mut w = World::new();
    w.put(".npm/_cacache", 2);
    w.scan();
    let plan = w.plan_rules(&["npm-cache"]);
    w.execute(&plan, false);
    assert!(!w.exists(".npm/_cacache"));
}

#[test]
fn delete_contents_keeps_the_folder() {
    let mut w = World::new();
    w.put(".Trash/old", 2);
    w.scan();
    let plan = w.plan_rules(&["trash-bin"]);
    w.execute(&plan, false);
    assert_eq!((w.exists(".Trash"), w.exists(".Trash/old")), (true, false));
}

#[test]
fn trash_moves_the_folder_to_the_trash() {
    let mut w = World::new();
    w.put(".cache/huggingface/hub", 2);
    w.scan();
    let plan = w.plan_rules(&["huggingface-cache"]);
    w.execute(&plan, false);
    assert_eq!(
        (
            w.exists(".cache/huggingface/hub"),
            w.trash.join("hub").exists()
        ),
        (false, true)
    );
}

#[test]
fn summary_counts_freed_and_trashed_bytes_apart() {
    let mut w = World::new();
    w.put(".npm/_cacache", 2);
    w.put(".cache/huggingface/hub", 3);
    w.scan();
    let plan = w.plan_rules(&["npm-cache", "huggingface-cache"]);
    let (s, _) = w.execute(&plan, false);
    assert_eq!((s.freed / MB, s.trashed / MB, s.done), (2, 3, 2), "{s:?}");
}

#[test]
fn plan_totals_split_out_the_trash() {
    let w = review_world();
    let plan = w.plan_rules(&["huggingface-cache", "trash-bin"]);
    assert_eq!((plan.total / MB, plan.trash_total / MB), (4, 2));
}

#[test]
fn review_trash_items_offer_permanent_delete() {
    let w = review_world();
    assert!(w.plan_rules(&["huggingface-cache"]).permanent_available);
}

#[test]
fn rules_without_allow_permanent_do_not_offer_it() {
    let w = review_world();
    assert!(!w.plan_rules(&["ios-backups"]).permanent_available);
}

#[test]
fn permanent_review_deletes_instead_of_trashing() {
    let w = review_world();
    assert_eq!(
        methods(&w, &["huggingface-cache", "ios-backups"], true),
        [PlannedMethod::Trash, PlannedMethod::Delete],
        "ios-backups (larger, listed first) keeps the Trash"
    );
}

#[test]
fn contents_targets_plan_contents_methods() {
    let w = review_world();
    assert_eq!(
        methods(&w, &["trash-bin"], false),
        [PlannedMethod::DeleteContents]
    );
}

#[test]
fn commands_get_absolute_argv() {
    let mut w = World::new();
    w.tool("brew");
    w.put("Library/Caches/Homebrew/downloads", 2);
    w.scan();
    let brew = w.bin.join("brew").display().to_string();
    assert_eq!(
        methods(&w, &["homebrew-cache"], false),
        [PlannedMethod::Command {
            argv: vec![brew, "cleanup".into(), "--prune=all".into()]
        }]
    );
}

#[test]
fn command_runs_through_the_runner() {
    let mut w = World::new();
    w.tool("brew");
    w.put("Library/Caches/Homebrew/downloads", 2);
    w.runner = std::mem::take(&mut w.runner).with("brew cleanup --prune=all", true, "");
    w.scan();
    let plan = w.plan_rules(&["homebrew-cache"]);
    let (_, entries) = w.execute(&plan, false);
    assert_eq!(entries[0].result, "command_ran");
}

#[test]
fn missing_tool_falls_back_to_the_rules_fallback_method() {
    let mut w = World::new();
    w.put("Library/Caches/Homebrew/downloads", 2);
    w.scan();
    assert_eq!(
        methods(&w, &["homebrew-cache"], false),
        [PlannedMethod::DeleteContents]
    );
}

#[test]
fn every_action_is_audited_with_rule_path_method_and_bytes() {
    let mut w = World::new();
    w.put(".npm/_cacache", 2);
    w.scan();
    let plan = w.plan_rules(&["npm-cache"]);
    let (_, entries) = w.execute(&plan, false);
    let e = &entries[0];
    assert_eq!(
        (
            e.rule.as_str(),
            e.path.clone(),
            e.method.as_str(),
            e.bytes / MB,
            e.result.as_str(),
            e.dry_run
        ),
        (
            "npm-cache",
            w.home.join(".npm/_cacache").display().to_string(),
            "delete",
            2,
            "deleted",
            false
        )
    );
}
