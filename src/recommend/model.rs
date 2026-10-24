//! Recommendation types (PLAN.md section 3).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Tier {
    Safe,
    Review,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ItemStatus {
    Ready,
    Blocked { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MethodView {
    Delete,
    Trash,
    Command { display: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecItem {
    pub item_id: String,
    pub path: String,
    pub name: String,
    pub size: u64,
    pub status: ItemStatus,
    pub method: MethodView,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Recommendation {
    pub row_id: String,
    pub rule_id: String,
    pub label: String,
    pub paths_hint: String,
    pub tier: Tier,
    pub size: u64,
    pub size_is_estimate: bool,
    pub items: Vec<RecItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Subfolder {
    pub name: String,
    pub path: String,
    pub size: u64,
}

/// Everything the detail drawer shows beyond the row itself (PRD section 7.5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
