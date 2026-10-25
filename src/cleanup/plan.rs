//! Item IDs to a `CleanupPlan` (PRD section 7.6, S-01 and S-04).
//!
//! Only items the rules engine produced for the current snapshot can be
//! planned. Item statuses come from `Analysis::recommendations`, so blocking
//! guards are evaluated now, exactly as the panel shows them.

use std::collections::HashSet;

use super::model::{CleanupPlan, PlannedAction, PlannedMethod};
use crate::error::{Error, Result};
use crate::recommend::build::{Analysis, Item};
use crate::recommend::model::{ItemStatus, MethodView, Tier};
use crate::rules::guards::Env;
use crate::rules::model::{Rule, Rules, Target};

/// What the UI asks to clean.
pub struct Request<'a> {
    /// The snapshot the selection was made in.
    pub snapshot_id: &'a str,
    pub item_ids: &'a [String],
    /// Delete review-tier trash items permanently, where the rule allows it.
    pub permanent_review: bool,
}

pub fn build(
    plan_id: String,
    rules: &Rules,
    analysis: &Analysis,
    env: &Env,
    req: &Request,
) -> Result<CleanupPlan> {
    if req.snapshot_id != analysis.snapshot_id {
        return Err(Error::StaleSnapshot);
    }
    let wanted: HashSet<&str> = req.item_ids.iter().map(String::as_str).collect();
    let mut warnings = Vec::new();
    let mut actions = Vec::new();
    let mut permanent_available = false;
    let mut found: HashSet<&str> = HashSet::new();
    // Row order, so the confirm dialog groups by rule like the panel does.
    for row in analysis.recommendations(rules, env) {
        for rec in &row.items {
            if !wanted.contains(rec.item_id.as_str()) {
                continue;
            }
            let Some(item) = analysis.items.iter().find(|i| i.id == rec.item_id) else {
                continue;
            };
            found.insert(item.id.as_str());
            if let ItemStatus::Blocked { reason } = &rec.status {
                warnings.push(format!("{}: {reason}", item.name));
                continue;
            }
            let rule = &rules.rules[item.rule];
            let review_trash = row.tier == Tier::Review && rec.method == MethodView::Trash;
            if review_trash && rule.allow_permanent {
                permanent_available = true;
            }
            let permanent = review_trash && rule.allow_permanent && req.permanent_review;
            let Some(method) = method(rule, item, &rec.method, permanent, env) else {
                warnings.push(format!("{}: tool not found", item.name));
                continue;
            };
            actions.push(PlannedAction {
                item_id: item.id.clone(),
                rule_id: rule.id.clone(),
                label: row.label.clone(),
                name: item.name.clone(),
                tier: row.tier,
                path: item.path.display().to_string(),
                matched: item.matched.display().to_string(),
                size: item.size,
                size_is_estimate: rule.size_is_estimate,
                measurable: item.node.is_some() && !rule.size_is_estimate,
                method,
            });
        }
    }
    let mut unknown: Vec<&str> = wanted.difference(&found).copied().collect();
    unknown.sort_unstable();
    for id in unknown {
        warnings.push(format!("{id}: not part of the current scan"));
    }
    let total = actions.iter().map(|a| a.size).sum();
    let trash_total = actions
        .iter()
        .filter(|a| a.method.is_trash())
        .map(|a| a.size)
        .sum();
    Ok(CleanupPlan {
        plan_id,
        snapshot_id: analysis.snapshot_id.clone(),
        actions,
        total,
        trash_total,
        permanent_available,
        warnings,
    })
}

/// The method the panel shows, made concrete: the rule's target decides
/// between the folder and its contents, and commands get absolute argv.
/// `None` when the command's tool no longer resolves.
fn method(
    rule: &Rule,
    item: &Item,
    shown: &MethodView,
    permanent: bool,
    env: &Env,
) -> Option<PlannedMethod> {
    let contents = rule.target == Target::Contents;
    Some(match (shown, contents) {
        (MethodView::Trash, _) if permanent => {
            if contents {
                PlannedMethod::DeleteContents
            } else {
                PlannedMethod::Delete
            }
        }
        (MethodView::Delete, false) => PlannedMethod::Delete,
        (MethodView::Delete, true) => PlannedMethod::DeleteContents,
        (MethodView::Trash, false) => PlannedMethod::Trash,
        (MethodView::Trash, true) => PlannedMethod::TrashContents,
        (MethodView::Command { .. }, _) => PlannedMethod::Command {
            argv: argv(rule, item, env)?,
        },
    })
}

/// The rule's command with placeholders filled with real paths. `None` when
/// the tool or a placeholder's value is missing.
fn argv(rule: &Rule, item: &Item, env: &Env) -> Option<Vec<String>> {
    let (program, args) = rule.command.as_deref()?.split_first()?;
    let program = env.tools.resolve(program)?;
    let path = item.path.display().to_string();
    let main = item.main_repo.as_deref().map(|p| p.display().to_string());
    let vars = [
        ("{path}", Some(path.as_str())),
        ("{name}", Some(item.name.as_str())),
        ("{main_repo}", main.as_deref()),
    ];
    let mut argv = vec![program.display().to_string()];
    for a in args {
        argv.push(fill(a, &vars)?);
    }
    Some(argv)
}

/// Replaces placeholders in one pass, so a value is never expanded again.
fn fill(arg: &str, vars: &[(&str, Option<&str>)]) -> Option<String> {
    let mut out = String::with_capacity(arg.len());
    let mut rest = arg;
    'next: while let Some(c) = rest.chars().next() {
        for (key, value) in vars {
            if let Some(after) = rest.strip_prefix(key) {
                out.push_str((*value)?);
                rest = after;
                continue 'next;
            }
        }
        out.push(c);
        rest = &rest[c.len_utf8()..];
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fill_does_not_expand_values_again() {
        let vars = [("{path}", Some("/h/{name}")), ("{name}", Some("x"))];
        assert_eq!(fill("{path}:{name}", &vars).as_deref(), Some("/h/{name}:x"));
    }

    #[test]
    fn fill_refuses_a_missing_value() {
        assert_eq!(fill("-C {main_repo}", &[("{main_repo}", None)]), None);
    }
}
