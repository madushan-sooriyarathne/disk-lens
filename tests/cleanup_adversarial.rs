//! Cleanup safety (PRD section 8, PLAN.md Phase 4 step 8). Every case here must
//! be refused or skipped, leaving the disk as it was. All files live in a
//! temporary scan root under `target/tmp`.

mod support;

use std::fs;
use std::os::unix::fs::symlink;

use disklens_lib::cleanup::model::{CleanupEvent, CleanupPlan, Outcome};
use disklens_lib::error::Error;
use disklens_lib::platform::{App, Facts, Proc};
use support::World;

fn npm_world() -> World {
    let mut w = World::new();
    w.put(".npm/_cacache", 2);
    w.scan();
    w
}

fn skipped_reason(w: &World, plan: &CleanupPlan) -> String {
    let (summary, _) = w.execute(plan, false);
    summary
        .skipped
        .first()
        .map(|s| s.reason.clone())
        .unwrap_or_default()
}

#[test]
fn symlink_swapped_in_after_the_scan_is_skipped() {
    let w = npm_world();
    let plan = w.plan_rules(&["npm-cache"]);
    fs::create_dir_all(w.home.join("keep")).unwrap();
    fs::write(w.home.join("keep/file"), "precious").unwrap();
    fs::remove_dir_all(w.home.join(".npm/_cacache")).unwrap();
    symlink(w.home.join("keep"), w.home.join(".npm/_cacache")).unwrap();
    w.execute(&plan, false);
    assert!(w.exists("keep/file"));
}

#[test]
fn symlink_swapped_in_after_the_scan_reports_why() {
    let w = npm_world();
    let plan = w.plan_rules(&["npm-cache"]);
    fs::remove_dir_all(w.home.join(".npm/_cacache")).unwrap();
    symlink(&w.outside, w.home.join(".npm/_cacache")).unwrap();
    assert_eq!(
        skipped_reason(&w, &plan),
        "Changed since scan: the folder is a symlink"
    );
}

#[test]
fn path_replaced_by_a_symlink_outside_home_is_skipped() {
    let w = npm_world();
    let plan = w.plan_rules(&["npm-cache"]);
    fs::create_dir_all(w.outside.join("_cacache")).unwrap();
    fs::write(w.outside.join("_cacache/file"), "precious").unwrap();
    fs::remove_dir_all(w.home.join(".npm")).unwrap();
    symlink(&w.outside, w.home.join(".npm")).unwrap();
    w.execute(&plan, false);
    assert!(w.outside.join("_cacache/file").exists());
}

#[test]
fn path_through_a_symlinked_parent_reports_why() {
    let w = npm_world();
    let plan = w.plan_rules(&["npm-cache"]);
    fs::create_dir_all(w.outside.join("_cacache")).unwrap();
    fs::remove_dir_all(w.home.join(".npm")).unwrap();
    symlink(&w.outside, w.home.join(".npm")).unwrap();
    assert_eq!(
        skipped_reason(&w, &plan),
        "Changed since scan: the folder is reached through a symlink"
    );
}

#[test]
fn protected_path_injected_into_a_plan_is_skipped() {
    let w = npm_world();
    fs::create_dir_all(w.home.join(".ssh")).unwrap();
    fs::write(w.home.join(".ssh/id_ed25519"), "key").unwrap();
    let mut plan = w.plan_rules(&["npm-cache"]);
    let ssh = w.home.join(".ssh").display().to_string();
    plan.actions[0].path = ssh.clone();
    plan.actions[0].matched = ssh;
    w.execute(&plan, false);
    assert!(w.exists(".ssh/id_ed25519"));
}

#[test]
fn path_outside_the_root_injected_into_a_plan_is_skipped() {
    let w = npm_world();
    fs::write(w.outside.join("file"), "precious").unwrap();
    let mut plan = w.plan_rules(&["npm-cache"]);
    let outside = w.outside.display().to_string();
    plan.actions[0].path = outside.clone();
    plan.actions[0].matched = outside;
    w.execute(&plan, false);
    assert!(w.outside.join("file").exists());
}

#[test]
fn injected_path_that_no_rule_matches_is_skipped() {
    let w = npm_world();
    w.put("Documents/thesis", 1);
    let mut plan = w.plan_rules(&["npm-cache"]);
    let thesis = w.home.join("Documents/thesis").display().to_string();
    plan.actions[0].path = thesis.clone();
    plan.actions[0].matched = thesis;
    assert_eq!(
        skipped_reason(&w, &plan),
        "Changed since scan: it no longer matches its rule"
    );
}

#[test]
fn unknown_item_id_is_left_out_of_the_plan() {
    let w = npm_world();
    let plan = w.plan(&["npm-cache:999999".into()]);
    assert!(plan.actions.is_empty());
}

#[test]
fn unknown_item_id_is_reported() {
    let w = npm_world();
    let plan = w.plan(&["npm-cache:999999".into()]);
    assert_eq!(
        plan.warnings,
        ["npm-cache:999999: not part of the current scan"]
    );
}

#[test]
fn item_id_from_a_previous_snapshot_is_refused() {
    let mut w = npm_world();
    let old = w.analysis().snapshot_id.clone();
    let ids = w.items("npm-cache");
    w.scan();
    let err = w.try_plan(&old, &ids, false).unwrap_err();
    assert!(matches!(err, Error::StaleSnapshot));
}

#[test]
fn folder_that_became_a_repository_root_is_skipped() {
    let w = npm_world();
    let plan = w.plan_rules(&["npm-cache"]);
    fs::create_dir_all(w.home.join(".npm/_cacache/.git")).unwrap();
    w.execute(&plan, false);
    assert!(w.exists(".npm/_cacache/.git"));
}

#[test]
fn repository_root_inside_a_contents_target_is_kept() {
    let mut w = World::new();
    w.put(".Trash/old-file", 2);
    fs::create_dir_all(w.home.join(".Trash/project/.git")).unwrap();
    w.scan();
    let plan = w.plan_rules(&["trash-bin"]);
    w.execute(&plan, false);
    assert_eq!(
        (w.exists(".Trash/project/.git"), w.exists(".Trash/old-file")),
        (true, false)
    );
}

#[test]
fn symlink_inside_a_contents_target_is_removed_but_not_followed() {
    let mut w = World::new();
    w.put(".Trash/old-file", 2);
    fs::write(w.outside.join("file"), "precious").unwrap();
    symlink(&w.outside, w.home.join(".Trash/link")).unwrap();
    w.scan();
    let plan = w.plan_rules(&["trash-bin"]);
    w.execute(&plan, false);
    assert_eq!(
        (w.exists(".Trash/link"), w.outside.join("file").exists()),
        (false, true)
    );
}

#[test]
fn folder_that_grew_past_the_threshold_is_skipped() {
    let mut w = npm_world();
    w.growth_floor = 0;
    let plan = w.plan_rules(&["npm-cache"]);
    w.grow(".npm/_cacache", 2_000_000);
    assert_eq!(
        skipped_reason(&w, &plan),
        "Changed since scan: it grew since the scan"
    );
}

#[test]
fn small_growth_does_not_skip() {
    let mut w = npm_world();
    w.growth_floor = 0;
    let plan = w.plan_rules(&["npm-cache"]);
    w.grow(".npm/_cacache", 100_000);
    w.execute(&plan, false);
    assert!(!w.exists(".npm/_cacache"));
}

#[test]
fn process_guard_that_starts_between_plan_and_execute_skips() {
    let mut w = World::new();
    w.put(".gradle/caches", 2);
    w.scan();
    let plan = w.plan_rules(&["gradle-caches"]);
    w.set_facts(Facts {
        processes: vec![Proc {
            name: "java".into(),
            cmdline: "java org.gradle.launcher.daemon.bootstrap.GradleDaemon 8.5".into(),
        }],
        ..Facts::default()
    });
    assert_eq!(skipped_reason(&w, &plan), "Quit the Gradle daemon to clean");
}

#[test]
fn process_guard_that_starts_between_plan_and_execute_keeps_the_folder() {
    let mut w = World::new();
    w.put(".gradle/caches", 2);
    w.scan();
    let plan = w.plan_rules(&["gradle-caches"]);
    w.set_facts(Facts {
        processes: vec![Proc {
            name: "java".into(),
            cmdline: "GradleDaemon".into(),
        }],
        ..Facts::default()
    });
    w.execute(&plan, false);
    assert!(w.exists(".gradle/caches"));
}

#[test]
fn item_blocked_when_planning_is_left_out() {
    let mut w = World::new();
    w.put("Library/Developer/Xcode/DerivedData/App-abc", 2);
    w.scan();
    w.set_facts(Facts {
        apps: vec![App {
            bundle_id: "com.apple.dt.Xcode".into(),
            name: "Xcode".into(),
        }],
        ..Facts::default()
    });
    let plan = w.plan_rules(&["xcode-derived-data"]);
    assert_eq!(plan.warnings, ["DerivedData: Quit Xcode to clean"]);
}

#[test]
fn skipped_items_are_audited() {
    let w = npm_world();
    let plan = w.plan_rules(&["npm-cache"]);
    fs::create_dir_all(w.home.join(".npm/_cacache/.git")).unwrap();
    let (_, entries) = w.execute(&plan, false);
    assert_eq!(
        entries
            .iter()
            .map(|e| e.result.as_str())
            .collect::<Vec<_>>(),
        ["skipped"]
    );
}

#[test]
fn dry_run_changes_nothing() {
    let w = npm_world();
    let plan = w.plan_rules(&["npm-cache"]);
    w.execute(&plan, true);
    assert!(w.exists(".npm/_cacache/data"));
}

#[test]
fn dry_run_reports_dry_run_outcomes() {
    let w = npm_world();
    let plan = w.plan_rules(&["npm-cache"]);
    let (_, entries) = w.execute(&plan, true);
    assert_eq!(
        entries
            .iter()
            .map(|e| (e.result.as_str(), e.dry_run))
            .collect::<Vec<_>>(),
        [("dry_run", true)]
    );
}

#[test]
fn failed_command_is_reported_and_the_rest_continues() {
    let mut w = World::new();
    w.tool("brew");
    w.put("Library/Caches/Homebrew/downloads", 2);
    w.put(".npm/_cacache", 2);
    w.scan();
    // The fake runner knows no `brew cleanup`, so the command fails.
    let plan = w.plan_rules(&["homebrew-cache", "npm-cache"]);
    let (summary, _) = w.execute(&plan, false);
    assert_eq!(
        (
            summary.done,
            summary.skipped.len(),
            w.exists(".npm/_cacache")
        ),
        (1, 1, false)
    );
}

#[test]
fn outcome_of_a_skipped_item_is_skipped() {
    let w = npm_world();
    let plan = w.plan_rules(&["npm-cache"]);
    fs::create_dir_all(w.home.join(".npm/_cacache/.git")).unwrap();
    let mut outcomes = Vec::new();
    w.execute_with(&plan, false, &mut |e| {
        if let CleanupEvent::ItemFinished { outcome, .. } = e {
            outcomes.push(outcome);
        }
    });
    assert_eq!(
        outcomes,
        [Outcome::Skipped {
            reason: "Changed since scan: the folder is a repository root".into()
        }]
    );
}
