//! Selection rules (owner: A1). Port of `legacy-web/src/lib/selection.ts`.

use std::collections::HashSet;

use crate::recommend::model::{ItemStatus, Recommendation, Tier};
use crate::view::ViewNode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowState {
    None,
    Some,
    All,
}

/// Items that can be selected: blocked items never can.
pub fn ready_ids(row: &Recommendation) -> Vec<&str> {
    row.items
        .iter()
        .filter(|i| i.status == ItemStatus::Ready)
        .map(|i| i.item_id.as_str())
        .collect()
}

/// Tri-state of a row's checkbox over its ready items.
pub fn row_state(row: &Recommendation, selected: &HashSet<String>) -> RowState {
    let ids = ready_ids(row);
    let n = ids.iter().filter(|id| selected.contains(**id)).count();
    if n == 0 {
        RowState::None
    } else if n == ids.len() {
        RowState::All
    } else {
        RowState::Some
    }
}

/// A fully selected row clears; otherwise all its ready items are selected.
pub fn toggle_row(row: &Recommendation, selected: &HashSet<String>) -> HashSet<String> {
    let mut next = selected.clone();
    let all = row_state(row, selected) == RowState::All;
    for id in ready_ids(row) {
        if all {
            next.remove(id);
        } else {
            next.insert(id.to_string());
        }
    }
    next
}

/// Adds every ready safe-tier item; review selections stay as they are.
pub fn select_safe(recs: &[Recommendation], selected: &HashSet<String>) -> HashSet<String> {
    let mut next = selected.clone();
    for r in recs.iter().filter(|r| r.tier == Tier::Safe) {
        next.extend(ready_ids(r).into_iter().map(String::from));
    }
    next
}

/// Drops ids that vanished or became blocked, e.g. after a focus refresh.
pub fn prune_selection(recs: &[Recommendation], selected: &HashSet<String>) -> HashSet<String> {
    let ready: HashSet<&str> = recs.iter().flat_map(ready_ids).collect();
    selected
        .iter()
        .filter(|id| ready.contains(id.as_str()))
        .cloned()
        .collect()
}

/// The reason shown on a row whose items are all blocked, else `None`.
pub fn blocked_reason(row: &Recommendation) -> Option<&str> {
    if !ready_ids(row).is_empty() {
        return None;
    }
    match &row.items.first()?.status {
        ItemStatus::Blocked { reason } => Some(reason),
        ItemStatus::Ready => None,
    }
}

fn within(path: &str, dir: &str) -> bool {
    path.strip_prefix(dir)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
}

/// The row a matched tile belongs to: same rule, and an item at, above or
/// below the tile (split rows share a rule).
pub fn row_for_node<'a>(recs: &'a [Recommendation], node: &ViewNode) -> Option<&'a Recommendation> {
    let rule = node.rule_id.as_deref().filter(|r| !r.is_empty())?;
    let mut rows = recs.iter().filter(|r| r.rule_id == rule);
    let first = rows.next()?;
    std::iter::once(first)
        .chain(rows)
        .find(|r| {
            r.items
                .iter()
                .any(|i| within(&node.path, &i.path) || within(&i.path, &node.path))
        })
        .or(Some(first))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recommend::model::{MethodView, RecItem};
    use crate::view::ViewKind;

    /// Items are `(id, blocked reason)`.
    fn rec(row: &str, tier: Tier, items: &[(&str, Option<&str>)]) -> Recommendation {
        Recommendation {
            row_id: row.into(),
            rule_id: row.into(),
            label: row.into(),
            paths_hint: String::new(),
            tier,
            size: 0,
            size_is_estimate: false,
            items: items
                .iter()
                .map(|&(id, blocked)| RecItem {
                    item_id: id.into(),
                    path: format!("/{id}"),
                    name: id.into(),
                    size: 1,
                    status: match blocked {
                        Some(reason) => ItemStatus::Blocked {
                            reason: reason.into(),
                        },
                        None => ItemStatus::Ready,
                    },
                    method: MethodView::Delete,
                })
                .collect(),
        }
    }

    fn set(ids: &[&str]) -> HashSet<String> {
        ids.iter().map(|s| s.to_string()).collect()
    }

    fn sorted(s: &HashSet<String>) -> Vec<&str> {
        let mut v: Vec<&str> = s.iter().map(String::as_str).collect();
        v.sort();
        v
    }

    fn npm() -> Recommendation {
        rec(
            "npm",
            Tier::Safe,
            &[("a", None), ("b", None), ("c", Some("Changed recently"))],
        )
    }

    fn xcode() -> Recommendation {
        rec("xcode", Tier::Safe, &[("x", Some("Quit Xcode to clean"))])
    }

    fn trash() -> Recommendation {
        rec("trash", Tier::Review, &[("t", None)])
    }

    #[test]
    fn row_state_counts_ready_items_only() {
        assert_eq!(row_state(&npm(), &set(&[])), RowState::None);
        assert_eq!(row_state(&npm(), &set(&["a"])), RowState::Some);
        assert_eq!(row_state(&npm(), &set(&["a", "b"])), RowState::All);
        assert_eq!(row_state(&xcode(), &set(&[])), RowState::None);
    }

    #[test]
    fn toggle_row_selects_all_ready_items_then_clears_them() {
        let once = toggle_row(&npm(), &set(&["t"]));
        assert_eq!(sorted(&once), ["a", "b", "t"]);
        assert_eq!(sorted(&toggle_row(&npm(), &once)), ["t"]);
    }

    #[test]
    fn toggle_row_on_a_partly_selected_row_selects_it_fully() {
        assert_eq!(sorted(&toggle_row(&npm(), &set(&["a"]))), ["a", "b"]);
    }

    #[test]
    fn toggle_row_never_selects_blocked_items() {
        assert!(toggle_row(&xcode(), &set(&[])).is_empty());
    }

    #[test]
    fn select_safe_adds_every_ready_safe_item_and_keeps_review_choices() {
        let s = select_safe(&[npm(), xcode(), trash()], &set(&["t"]));
        assert_eq!(sorted(&s), ["a", "b", "t"]);
    }

    #[test]
    fn prune_selection_drops_vanished_and_newly_blocked_items() {
        let s = prune_selection(&[npm(), xcode()], &set(&["a", "c", "x", "gone"]));
        assert_eq!(sorted(&s), ["a"]);
    }

    #[test]
    fn blocked_reason_only_for_rows_with_nothing_ready() {
        assert_eq!(blocked_reason(&xcode()), Some("Quit Xcode to clean"));
        assert_eq!(blocked_reason(&npm()), None);
    }

    fn wt(project: &str, id: &str) -> Recommendation {
        Recommendation {
            rule_id: "wt".into(),
            ..rec(&format!("wt:{project}"), Tier::Review, &[(id, None)])
        }
    }

    fn tile(path: &str, rule: Option<&str>) -> ViewNode {
        ViewNode {
            id: path.into(),
            name: path.into(),
            path: path.into(),
            size: 1,
            category: "code_projects".into(),
            tier: rule.map(|_| Tier::Review),
            rule_id: rule.map(String::from),
            kind: ViewKind::Dir,
            children: vec![],
        }
    }

    #[test]
    fn row_for_node_picks_the_split_row_whose_item_contains_or_is_inside_the_tile() {
        let recs = [
            wt("atlas", "home/atlas/.claude/worktrees/a"),
            wt("pebble", "home/pebble/.claude/worktrees/b"),
        ];
        let row = |path| row_for_node(&recs, &tile(path, Some("wt"))).map(|r| r.row_id.as_str());
        assert_eq!(row("/home/pebble/.claude/worktrees/b"), Some("wt:pebble"));
        assert_eq!(
            row("/home/pebble/.claude/worktrees/b/src"),
            Some("wt:pebble")
        );
        assert_eq!(row("/home/atlas/.claude/worktrees"), Some("wt:atlas"));
    }

    #[test]
    fn row_for_node_ignores_unmatched_tiles_and_prefix_look_alikes() {
        let atlas = wt("atlas", "home/atlas/.claude/worktrees/a");
        let pebble = wt("pebble", "home/pebble/.claude/worktrees/b");
        assert!(row_for_node(std::slice::from_ref(&atlas), &tile("/home/atlas", None)).is_none());
        // "bb" is not inside "b", so no item matches and the first row wins.
        let recs = [atlas.clone(), pebble];
        let found = row_for_node(
            &recs,
            &tile("/home/pebble/.claude/worktrees/bb", Some("wt")),
        );
        assert_eq!(found, Some(&atlas));
    }
}
