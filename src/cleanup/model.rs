//! Cleanup contracts (PLAN.md section 3).

use serde::{Deserialize, Serialize};

use crate::recommend::model::Tier;

/// How one item is removed, decided when the plan is built.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlannedMethod {
    /// `remove_dir_all` on the item.
    Delete,
    /// `remove_dir_all` on each child; the folder stays.
    DeleteContents,
    /// Moves the item to the Trash.
    Trash,
    /// Moves each child to the Trash; the folder stays.
    TrashContents,
    /// Runs `argv`; `argv[0]` is an absolute path.
    Command { argv: Vec<String> },
}

impl PlannedMethod {
    pub fn is_trash(&self) -> bool {
        matches!(self, Self::Trash | Self::TrashContents)
    }

    /// Name used in the audit log.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Delete => "delete",
            Self::DeleteContents => "delete_contents",
            Self::Trash => "trash",
            Self::TrashContents => "trash_contents",
            Self::Command { .. } => "command",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedAction {
    pub item_id: String,
    pub rule_id: String,
    /// Row label, for grouping in the confirm dialog.
    pub label: String,
    pub name: String,
    pub tier: Tier,
    /// Where the data lives.
    pub path: String,
    /// The folder the rule matched; differs from `path` for provider items.
    pub matched: String,
    /// Size at scan time.
    pub size: u64,
    pub size_is_estimate: bool,
    /// The item is a scanned folder, so its growth since the scan can be measured.
    pub measurable: bool,
    pub method: PlannedMethod,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CleanupPlan {
    pub plan_id: String,
    /// The snapshot the plan was built from; any other snapshot invalidates it.
    pub snapshot_id: String,
    pub actions: Vec<PlannedAction>,
    pub total: u64,
    /// Part of `total` that goes to the Trash and is freed only when it is emptied.
    pub trash_total: u64,
    /// Review-tier trash items whose rule allows deleting them permanently instead.
    pub permanent_available: bool,
    /// Requested items left out of the plan, with the reason.
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Outcome {
    Deleted,
    Trashed,
    CommandRan,
    /// Dry run: checked and logged, nothing changed.
    DryRun,
    /// Refused before running, for example "changed since scan".
    Skipped {
        reason: String,
    },
    Failed {
        error: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkippedItem {
    pub item_id: String,
    pub name: String,
    pub reason: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CleanupSummary {
    /// Bytes no longer on disk.
    pub freed: u64,
    /// Bytes moved to the Trash.
    pub trashed: u64,
    /// Items that ran (or would have, in a dry run).
    pub done: u32,
    /// Skipped and failed items.
    pub skipped: Vec<SkippedItem>,
    pub dry_run: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CleanupEvent {
    ItemStarted {
        item_id: String,
    },
    ItemFinished {
        item_id: String,
        freed: u64,
        outcome: Outcome,
    },
    Finished {
        summary: CleanupSummary,
    },
}
