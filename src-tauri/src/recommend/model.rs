//! Recommendation types (PLAN.md section 3).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum Tier {
    Safe,
    Review,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum ItemStatus {
    Ready,
    Blocked { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum MethodView {
    Delete,
    Trash,
    Command { display: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct RecItem {
    pub item_id: String,
    pub path: String,
    pub name: String,
    #[ts(type = "number")]
    pub size: u64,
    pub status: ItemStatus,
    pub method: MethodView,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Recommendation {
    pub row_id: String,
    pub rule_id: String,
    pub label: String,
    pub paths_hint: String,
    pub tier: Tier,
    #[ts(type = "number")]
    pub size: u64,
    pub size_is_estimate: bool,
    pub items: Vec<RecItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Subfolder {
    pub name: String,
    pub path: String,
    #[ts(type = "number")]
    pub size: u64,
}

/// Everything the detail drawer shows beyond the row itself (PRD section 7.5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ItemDetails {
    pub row_id: String,
    /// "What it is".
    pub description: String,
    /// "What happens if removed".
    pub regenerates: String,
    pub items: Vec<RecItem>,
    /// The five largest subfolders across the row's items.
    pub largest_subfolders: Vec<Subfolder>,
}
