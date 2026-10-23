//! Selection rules (owner: A1). Port of `legacy-web/src/lib/selection.ts`.

use std::collections::HashSet;

use crate::recommend::model::Recommendation;
use crate::view::ViewNode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowState {
    None,
    Some,
    All,
}

/// Items that can be selected: blocked items never can.
pub fn ready_ids(row: &Recommendation) -> Vec<&str> {
    todo!()
}

/// Tri-state of a row's checkbox over its ready items.
pub fn row_state(row: &Recommendation, selected: &HashSet<String>) -> RowState {
    todo!()
}

/// A fully selected row clears; otherwise all its ready items are selected.
pub fn toggle_row(row: &Recommendation, selected: &HashSet<String>) -> HashSet<String> {
    todo!()
}

/// Adds every ready safe-tier item; review selections stay as they are.
pub fn select_safe(recs: &[Recommendation], selected: &HashSet<String>) -> HashSet<String> {
    todo!()
}

/// Drops ids that vanished or became blocked, e.g. after a focus refresh.
pub fn prune_selection(recs: &[Recommendation], selected: &HashSet<String>) -> HashSet<String> {
    todo!()
}

/// The reason shown on a row whose items are all blocked, else `None`.
pub fn blocked_reason(row: &Recommendation) -> Option<&str> {
    todo!()
}

/// The row a matched tile belongs to: same rule, and an item at, above or
/// below the tile (split rows share a rule).
pub fn row_for_node<'a>(recs: &'a [Recommendation], node: &ViewNode) -> Option<&'a Recommendation> {
    todo!()
}
