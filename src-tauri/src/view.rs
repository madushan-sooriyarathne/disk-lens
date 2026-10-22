//! Treemap view model sent to the frontend (PLAN.md section 3).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::recommend::model::Tier;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum ViewMode {
    Category,
    Cleanup,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum ViewKind {
    Dir,
    File,
    /// Several small siblings merged into one tile.
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ViewNode {
    pub id: String,
    pub name: String,
    pub path: String,
    #[ts(type = "number")]
    pub size: u64,
    pub category: String,
    pub tier: Option<Tier>,
    pub rule_id: Option<String>,
    pub kind: ViewKind,
    pub children: Vec<ViewNode>,
}
