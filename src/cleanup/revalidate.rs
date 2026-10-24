//! Checks run immediately before each action (S-03, S-04). Nothing in the
//! plan is trusted: the path, the rule match, the size and the guards are all
//! checked against the disk and the machine as they are now.

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use super::model::{PlannedAction, PlannedMethod};
use super::protected::Protected;
use crate::rules::guards::{self, Env};
use crate::rules::matcher::Matcher;
use crate::rules::model::Rules;
use crate::scanner::tree::MediaExtensions;
use crate::scanner::walk;

/// An item grown by more than this fraction of its scanned size...
const GROWTH_RATIO: f64 = 0.2;
/// ...and by more than this many bytes has changed since the scan.
pub const GROWTH_FLOOR: u64 = 1_000_000_000;

pub struct Revalidator<'a> {
    rules: &'a Rules,
    root: &'a Path,
    matcher: Matcher,
    protected: Protected,
    growth_floor: u64,
}

impl<'a> Revalidator<'a> {
    /// `growth_floor` is `GROWTH_FLOOR` outside tests.
    pub fn new(rules: &'a Rules, root: &'a Path, growth_floor: u64) -> Self {
        let root_str = root.display().to_string();
        let extra: Vec<String> = rules.protect.iter().map(|p| p.path.clone()).collect();
        Self {
            rules,
            root,
            matcher: Matcher::new(rules, &root_str),
            protected: Protected::new(&root_str, &extra),
            growth_floor,
        }
    }

    pub fn protected(&self) -> &Protected {
        &self.protected
    }

    /// `Err(reason)` when `action` must be skipped.
    pub fn check(&self, action: &PlannedAction, env: &Env) -> Result<(), String> {
        let changed = |why: &str| Err(format!("Changed since scan: {why}"));
        let Some(rule_index) = self.rules.rules.iter().position(|r| r.id == action.rule_id) else {
            return changed("its rule no longer exists");
        };
        let rule = &self.rules.rules[rule_index];
        let path = Path::new(&action.path);
        let matched = Path::new(&action.matched);
        if let Some(why) = self.protected.refusal(path) {
            return changed(&format!("the folder {why}"));
        }
        if matched != path {
            if !path.starts_with(matched) {
                return changed("the item is not inside its matched folder");
            }
            if let Some(why) = self.protected.refusal(matched) {
                return changed(&format!("the matched folder {why}"));
            }
        }
        if self.matcher.winner(self.rules, matched, self.root, env) != Some(rule_index) {
            return changed("it no longer matches its rule");
        }
        if action.measurable {
            match measure(path) {
                Some(now) if self.grew(action.size, now) => {
                    return changed("it grew since the scan");
                }
                Some(_) => {}
                None => return changed("its size cannot be measured"),
            }
        }
        if let PlannedMethod::Command { argv } = &action.method
            && !argv
                .first()
                .is_some_and(|p| Path::new(p).is_absolute() && Path::new(p).is_file())
        {
            return Err("Command not found".into());
        }
        match guards::blocked(&rule.guards, path, env) {
            Some(reason) => Err(reason),
            None => Ok(()),
        }
    }

    fn grew(&self, scanned: u64, now: u64) -> bool {
        let growth = now.saturating_sub(scanned);
        growth > self.growth_floor && growth as f64 > scanned as f64 * GROWTH_RATIO
    }
}

/// Allocated bytes under `path` now, counted like the scanner counts them.
pub fn measure(path: &Path) -> Option<u64> {
    let never = Arc::new(AtomicBool::new(false));
    walk::walk(path, &MediaExtensions::default(), &never, &mut |_| {})
        .ok()
        .and_then(|w| w.nodes.first().map(|n| n.size))
}
