//! A Pro license changes which rules match, never how a free rule behaves: the same
//! scan gives the same rows, plan, script and dry run with and without one.
//!
//! The backend verifies licenses against the embedded key, which has no test signer,
//! so these tests flip `World::pro` (the `Env.pro` and `Engine.pro` the backend passes
//! from `AppState::is_pro`) instead of activating a token.

mod support;

use disklens_lib::cleanup::script::render;
use support::World;

const RULES: [&str; 3] = ["npm-cache", "trash-bin", "huggingface-cache"];
const GENERATED: i64 = 1_792_833_900;

fn world() -> World {
    let mut w = World::new();
    w.put(".npm/_cacache", 2);
    w.put(".Trash/old", 2);
    w.put(".cache/huggingface/hub", 2);
    w.scan();
    w
}

/// `f` on the same scan without a license, then with one.
fn free_and_pro<T>(w: &mut World, f: impl Fn(&World) -> T) -> (T, T) {
    w.pro = false;
    let free = f(w);
    w.pro = true;
    (free, f(w))
}

fn item_ids(w: &World) -> Vec<String> {
    let mut ids: Vec<String> = w.analysis().items.iter().map(|i| i.id.clone()).collect();
    ids.sort();
    ids
}

#[test]
fn scan_analysis_is_the_same() {
    let mut w = world();
    let free = item_ids(&w);
    w.pro = true;
    w.scan();
    assert_eq!(item_ids(&w), free);
}

#[test]
fn recommendations_are_the_same() {
    let mut w = world();
    let (free, pro) = free_and_pro(&mut w, |w| w.recommendations());
    assert_eq!(pro, free);
}

#[test]
fn cleanup_plan_is_the_same() {
    let mut w = world();
    let (free, pro) = free_and_pro(&mut w, |w| w.plan_rules(&RULES));
    assert_eq!(pro, free);
}

#[test]
fn exported_script_is_the_same() {
    let mut w = world();
    let (free, pro) = free_and_pro(&mut w, |w| render(&w.plan_rules(&RULES), GENERATED));
    assert_eq!(pro, free);
}

#[test]
fn dry_run_result_is_the_same() {
    let mut w = world();
    let (free, pro) = free_and_pro(&mut w, |w| w.execute(&w.plan_rules(&RULES), true).0);
    assert_eq!(pro, free);
}
