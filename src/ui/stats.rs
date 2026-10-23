//! Stat card numbers (owner: A1). Port of `legacy-web/src/lib/stats.ts`.

use std::collections::HashSet;

use crate::recommend::model::Recommendation;
use crate::scanner::tree::SnapshotMeta;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TierStat {
    pub size: u64,
    /// Number of recommendation rows with at least one ready item.
    pub count: usize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Stats {
    pub available: u64,
    pub selected: u64,
    pub selected_rows: usize,
    pub free_after_cleanup: u64,
    pub safe: TierStat,
    pub review: TierStat,
    pub not_scanned: u64,
}

/// Every path belongs to one recommendation, so totals never double count.
pub fn compute_stats(
    meta: &SnapshotMeta,
    recs: &[Recommendation],
    selected: &HashSet<String>,
) -> Stats {
    todo!()
}
