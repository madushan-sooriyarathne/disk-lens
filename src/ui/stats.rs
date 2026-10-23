//! Stat card numbers (owner: A1). Port of `legacy-web/src/lib/stats.ts`.

use std::collections::HashSet;

use crate::recommend::model::{ItemStatus, RecItem, Recommendation, Tier};
use crate::scanner::tree::{SnapshotMeta, VolumeInfo};

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

fn ready(r: &Recommendation) -> impl Iterator<Item = &RecItem> {
    r.items.iter().filter(|i| i.status == ItemStatus::Ready)
}

fn tier_stat(recs: &[Recommendation], tier: Tier) -> TierStat {
    let mut stat = TierStat::default();
    for r in recs.iter().filter(|r| r.tier == tier) {
        let (n, size) = ready(r).fold((0, 0), |(n, s), i| (n + 1, s + i.size));
        if n > 0 {
            stat.count += 1;
            stat.size += size;
        }
    }
    stat
}

/// Every path belongs to one recommendation, so totals never double count.
pub fn compute_stats(
    meta: &SnapshotMeta,
    recs: &[Recommendation],
    selected: &HashSet<String>,
) -> Stats {
    let mut selected_size = 0;
    let mut selected_rows = 0;
    for r in recs {
        let (n, size) = ready(r)
            .filter(|i| selected.contains(&i.item_id))
            .fold((0, 0), |(n, s), i| (n + 1, s + i.size));
        if n > 0 {
            selected_rows += 1;
            selected_size += size;
        }
    }
    let VolumeInfo { total, available } = meta.volume;
    Stats {
        available,
        selected: selected_size,
        selected_rows,
        free_after_cleanup: available + selected_size,
        safe: tier_stat(recs, Tier::Safe),
        review: tier_stat(recs, Tier::Review),
        not_scanned: total
            .saturating_sub(available)
            .saturating_sub(meta.scanned_size),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recommend::model::MethodView;

    const GB: u64 = 1_000_000_000;

    /// The mock JSON keys the tests need; `view` and `_note` are ignored.
    #[derive(serde::Deserialize)]
    struct Mock {
        meta: SnapshotMeta,
        recommendations: Vec<Recommendation>,
        preselected: Vec<String>,
    }

    fn mock() -> Mock {
        serde_json::from_str(include_str!("../../assets/mock/snapshot.mock.json")).unwrap()
    }

    fn meta(total: u64, available: u64, scanned: u64) -> SnapshotMeta {
        SnapshotMeta {
            volume: VolumeInfo { total, available },
            scanned_size: scanned,
            ..mock().meta
        }
    }

    /// Items are `(id, size, blocked)`.
    fn rec(row: &str, tier: Tier, items: &[(&str, u64, bool)]) -> Recommendation {
        Recommendation {
            row_id: row.into(),
            rule_id: row.into(),
            label: row.into(),
            paths_hint: String::new(),
            tier,
            size: items.iter().map(|i| i.1).sum(),
            size_is_estimate: false,
            items: items
                .iter()
                .map(|&(id, size, blocked)| RecItem {
                    item_id: id.into(),
                    path: format!("/{id}"),
                    name: id.into(),
                    size,
                    status: if blocked {
                        ItemStatus::Blocked {
                            reason: "busy".into(),
                        }
                    } else {
                        ItemStatus::Ready
                    },
                    method: MethodView::Delete,
                })
                .collect(),
        }
    }

    fn gb(bytes: u64) -> f64 {
        bytes as f64 / GB as f64
    }

    #[test]
    fn matches_the_mockup_numbers() {
        let m = mock();
        let selected: HashSet<String> = m
            .recommendations
            .iter()
            .filter(|r| m.preselected.contains(&r.row_id))
            .flat_map(|r| r.items.iter().map(|i| i.item_id.clone()))
            .collect();
        let s = compute_stats(&m.meta, &m.recommendations, &selected);
        assert!((gb(s.safe.size) - 41.7).abs() < 0.05);
        assert_eq!(s.safe.count, 5);
        assert!((gb(s.review.size) - 56.0).abs() < 0.05);
        assert_eq!(s.review.count, 7);
        assert!((gb(s.selected) - 52.5).abs() < 0.05);
        assert_eq!(s.selected_rows, 5);
        assert!((gb(s.free_after_cleanup) - 92.0).abs() < 0.5);
        assert!((gb(s.not_scanned) - 288.0).abs() < 0.5);
    }

    #[test]
    fn selecting_nothing_leaves_free_space_unchanged() {
        let m = mock();
        let s = compute_stats(&m.meta, &m.recommendations, &HashSet::new());
        assert_eq!(s.selected, 0);
        assert_eq!(s.free_after_cleanup, m.meta.volume.available);
    }

    #[test]
    fn counts_only_ready_items_and_rows_that_have_one() {
        let recs = [
            rec(
                "a",
                Tier::Safe,
                &[("a1", 5 * GB, false), ("a2", 3 * GB, true)],
            ),
            rec("b", Tier::Safe, &[("b1", 2 * GB, true)]),
        ];
        let s = compute_stats(&meta(100 * GB, 40 * GB, 20 * GB), &recs, &HashSet::new());
        assert_eq!(
            s.safe,
            TierStat {
                size: 5 * GB,
                count: 1
            }
        );
    }

    #[test]
    fn ignores_a_selected_blocked_item() {
        let recs = [rec("a", Tier::Safe, &[("a1", 5 * GB, true)])];
        let selected = HashSet::from(["a1".to_string()]);
        let s = compute_stats(&meta(100 * GB, 40 * GB, 20 * GB), &recs, &selected);
        assert_eq!(s.selected, 0);
        assert_eq!(s.selected_rows, 0);
    }

    #[test]
    fn clamps_not_scanned_at_zero() {
        let s = compute_stats(&meta(100 * GB, 40 * GB, 90 * GB), &[], &HashSet::new());
        assert_eq!(s.not_scanned, 0);
    }
}
