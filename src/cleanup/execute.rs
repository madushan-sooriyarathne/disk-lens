//! Runs a plan one action at a time (PRD section 7.6). Every action is
//! revalidated first, logged afterwards, and a failure skips only that action.

use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::Path;
use std::time::SystemTime;

use super::audit::{self, Audit, Entry};
use super::model::{
    CleanupEvent, CleanupPlan, CleanupSummary, Outcome, PlannedAction, PlannedMethod, SkippedItem,
};
use super::protected::Protected;
use super::revalidate::{Revalidator, measure};
use crate::platform::{Runner, System, Trash};
use crate::rules::guards::Env;
use crate::rules::model::Rules;
use crate::rules::tools::Tools;

/// Everything execution touches. Tests pass fakes for the machine and the Trash.
pub struct Engine<'a> {
    pub rules: &'a Rules,
    /// Canonical scan root; nothing outside it is touched.
    pub root: &'a Path,
    pub system: &'a dyn System,
    /// Read-only probes for guards (`git status`).
    pub probes: &'a dyn Runner,
    /// Cleanup commands, with the 10 minute timeout (S-05).
    pub commands: &'a dyn Runner,
    pub tools: &'a Tools,
    pub trash: &'a dyn Trash,
    /// S-08: check and log every action without performing it.
    pub dry_run: bool,
    /// `revalidate::GROWTH_FLOOR` outside tests.
    pub growth_floor: u64,
}

/// What one action did.
struct Done {
    outcome: Outcome,
    bytes: u64,
    detail: Option<String>,
}

impl Done {
    fn skipped(reason: String) -> Self {
        Self {
            outcome: Outcome::Skipped { reason },
            bytes: 0,
            detail: None,
        }
    }
}

pub fn execute(
    engine: &Engine,
    plan: &CleanupPlan,
    audit: &mut Audit,
    on_event: &mut dyn FnMut(CleanupEvent),
) -> CleanupSummary {
    let revalidator = Revalidator::new(engine.rules, engine.root, engine.growth_floor);
    let mut summary = CleanupSummary {
        dry_run: engine.dry_run,
        ..CleanupSummary::default()
    };
    let mut ran: HashSet<&[String]> = HashSet::new();
    // Once the log cannot be written, nothing more runs (S-07).
    let mut audit_error: Option<String> = None;
    for action in &plan.actions {
        on_event(CleanupEvent::ItemStarted {
            item_id: action.item_id.clone(),
        });
        let done = match &audit_error {
            Some(e) => Done::skipped(format!("Audit log unavailable: {e}")),
            None => run(engine, &revalidator, action, &mut ran),
        };
        tally(&mut summary, action, &done);
        if audit_error.is_none() {
            let entry = entry(plan, action, &done, engine.dry_run);
            if let Err(e) = audit.record(&entry) {
                tracing::error!(
                    "could not write {} for {entry:?}: {e}",
                    audit.path().display()
                );
                audit_error = Some(e.to_string());
            }
        }
        on_event(CleanupEvent::ItemFinished {
            item_id: action.item_id.clone(),
            freed: done.bytes,
            outcome: done.outcome,
        });
    }
    on_event(CleanupEvent::Finished {
        summary: summary.clone(),
    });
    summary
}

fn run<'p>(
    engine: &Engine,
    revalidator: &Revalidator,
    action: &'p PlannedAction,
    ran: &mut HashSet<&'p [String]>,
) -> Done {
    let path = Path::new(&action.path);
    // One command can cover several items (`xcrun simctl delete unavailable`).
    if let PlannedMethod::Command { argv } = &action.method
        && ran.contains(argv.as_slice())
    {
        let gone = path.symlink_metadata().is_err();
        return Done {
            outcome: if engine.dry_run {
                Outcome::DryRun
            } else {
                Outcome::CommandRan
            },
            bytes: if engine.dry_run || gone {
                action.size
            } else {
                0
            },
            detail: Some("covered by the same command above".into()),
        };
    }
    // Guards see the machine as it is now (S-04).
    let facts = engine.system.facts();
    let env = Env {
        facts: &facts,
        runner: engine.probes,
        tools: engine.tools,
        now: SystemTime::now(),
    };
    if let Err(reason) = revalidator.check(action, &env) {
        return Done::skipped(reason);
    }
    let before = measure(path);
    if engine.dry_run {
        if let PlannedMethod::Command { argv } = &action.method {
            ran.insert(argv);
        }
        let bytes = match before {
            Some(b) if action.measurable => b,
            _ => action.size,
        };
        return Done {
            outcome: Outcome::DryRun,
            bytes,
            detail: None,
        };
    }
    let protected = revalidator.protected();
    let result = match &action.method {
        PlannedMethod::Delete => fs::remove_dir_all(path)
            .map(|()| None)
            .map_err(|e| e.to_string()),
        PlannedMethod::DeleteContents => each_child(protected, path, remove),
        PlannedMethod::Trash => engine
            .trash
            .trash(path)
            .map(|()| None)
            .map_err(|e| e.to_string()),
        PlannedMethod::TrashContents => each_child(protected, path, |c| engine.trash.trash(c)),
        PlannedMethod::Command { argv } => {
            ran.insert(argv);
            command(engine.commands, argv)
        }
    };
    let after = if path.symlink_metadata().is_ok() {
        measure(path).unwrap_or(0)
    } else {
        0
    };
    let bytes = before.unwrap_or(action.size).saturating_sub(after);
    match result {
        Ok(detail) => Done {
            outcome: match action.method {
                PlannedMethod::Delete | PlannedMethod::DeleteContents => Outcome::Deleted,
                PlannedMethod::Trash | PlannedMethod::TrashContents => Outcome::Trashed,
                PlannedMethod::Command { .. } => Outcome::CommandRan,
            },
            bytes,
            detail,
        },
        Err(error) => Done {
            outcome: Outcome::Failed { error },
            bytes,
            detail: None,
        },
    }
}

/// Removes a folder, a file or a symlink (never its target).
fn remove(path: &Path) -> io::Result<()> {
    if path.symlink_metadata()?.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
}

/// Applies `f` to each child of `dir`, keeping children the engine must not
/// touch (a repository root, a protected path). Symlinks are passed through:
/// removing or trashing one never touches its target. Continues past errors.
fn each_child(
    protected: &Protected,
    dir: &Path,
    mut f: impl FnMut(&Path) -> io::Result<()>,
) -> Result<Option<String>, String> {
    let entries = fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut kept = Vec::new();
    let mut errors = Vec::new();
    for entry in entries {
        let child = match entry {
            Ok(e) => e.path(),
            Err(e) => {
                errors.push(e.to_string());
                continue;
            }
        };
        let is_link = child
            .symlink_metadata()
            .is_ok_and(|m| m.file_type().is_symlink());
        if !is_link && let Some(why) = protected.refusal(&child) {
            kept.push(format!("{} {why}", child.display()));
            continue;
        }
        if let Err(e) = f(&child) {
            errors.push(format!("{}: {e}", child.display()));
        }
    }
    match (errors.first(), kept.is_empty()) {
        (Some(first), _) => Err(format!("{} items failed, first: {first}", errors.len())),
        (None, true) => Ok(None),
        (None, false) => Ok(Some(format!("kept {}", kept.join("; ")))),
    }
}

fn command(runner: &dyn Runner, argv: &[String]) -> Result<Option<String>, String> {
    let Some((program, args)) = argv.split_first() else {
        return Err("empty command".into());
    };
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    match runner.run(Path::new(program), &args) {
        Ok(out) if out.success => Ok(None),
        Ok(_) => Err(format!("{program} exited with an error")),
        Err(e) => Err(e.to_string()),
    }
}

fn tally(summary: &mut CleanupSummary, action: &PlannedAction, done: &Done) {
    let skip = |reason: &str| SkippedItem {
        item_id: action.item_id.clone(),
        name: action.name.clone(),
        reason: reason.to_owned(),
    };
    match &done.outcome {
        Outcome::Deleted | Outcome::CommandRan => {
            summary.freed += done.bytes;
            summary.done += 1;
        }
        Outcome::Trashed => {
            summary.trashed += done.bytes;
            summary.done += 1;
        }
        Outcome::DryRun => {
            if action.method.is_trash() {
                summary.trashed += done.bytes;
            } else {
                summary.freed += done.bytes;
            }
            summary.done += 1;
        }
        Outcome::Skipped { reason } => summary.skipped.push(skip(reason)),
        Outcome::Failed { error } => summary.skipped.push(skip(error)),
    }
}

fn entry(plan: &CleanupPlan, action: &PlannedAction, done: &Done, dry_run: bool) -> Entry {
    let (result, detail) = match &done.outcome {
        Outcome::Deleted => ("deleted", done.detail.clone()),
        Outcome::Trashed => ("trashed", done.detail.clone()),
        Outcome::CommandRan => ("command_ran", done.detail.clone()),
        Outcome::DryRun => ("dry_run", done.detail.clone()),
        Outcome::Skipped { reason } => ("skipped", Some(reason.clone())),
        Outcome::Failed { error } => ("failed", Some(error.clone())),
    };
    Entry {
        ts: audit::now_ms(),
        plan_id: plan.plan_id.clone(),
        item_id: action.item_id.clone(),
        rule: action.rule_id.clone(),
        path: action.path.clone(),
        method: action.method.as_str().to_owned(),
        argv: match &action.method {
            PlannedMethod::Command { argv } => Some(argv.clone()),
            _ => None,
        },
        bytes: done.bytes,
        result: result.to_owned(),
        detail,
        dry_run,
    }
}
