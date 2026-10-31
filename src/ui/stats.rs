//! Overview numbers (PRD 13.4.4).

use std::collections::HashSet;

use super::theme::LEGEND_ORDER;
use crate::recommend::model::{ItemStatus, RecItem, Recommendation, Tier};
use crate::scanner::tree::{SnapshotMeta, VolumeInfo};
use crate::view::{CategoryTotal, ViewKind, ViewNode};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TierStat {
    pub size: u64,
    /// Ready items, as the Overview counts them.
    pub items: usize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Stats {
    /// Free space, purgeable space included.
    pub available: u64,
    pub purgeable: u64,
    pub selected: u64,
    pub selected_rows: usize,
    pub safe: TierStat,
    pub review: TierStat,
    pub not_scanned: u64,
}

/// One part of the Disk widget's capacity bar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Segment {
    Category(String),
    /// Everything outside the scan root ("System and apps").
    System,
    Free,
}

fn ready(r: &Recommendation) -> impl Iterator<Item = &RecItem> {
    r.items.iter().filter(|i| i.status == ItemStatus::Ready)
}

fn tier_stat(recs: &[Recommendation], tier: Tier) -> TierStat {
    let mut stat = TierStat::default();
    for i in recs.iter().filter(|r| r.tier == tier).flat_map(ready) {
        stat.items += 1;
        stat.size += i.size;
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
    let VolumeInfo {
        total,
        available,
        purgeable,
        ..
    } = meta.volume;
    Stats {
        available,
        purgeable,
        selected: selected_size,
        selected_rows,
        safe: tier_stat(recs, Tier::Safe),
        review: tier_stat(recs, Tier::Review),
        not_scanned: total
            .saturating_sub(available)
            .saturating_sub(meta.scanned_size),
    }
}

/// Categories in legend order, then System, then Free; empty parts are dropped.
pub fn capacity_segments(stats: &Stats, totals: &[CategoryTotal]) -> Vec<(Segment, u64)> {
    let size_of = |id: &str| {
        totals
            .iter()
            .filter(|t| t.category == id)
            .map(|t| t.size)
            .sum::<u64>()
    };
    LEGEND_ORDER
        .iter()
        .map(|&id| (Segment::Category(id.to_owned()), size_of(id)))
        .chain([
            (Segment::System, stats.not_scanned),
            (Segment::Free, stats.available),
        ])
        .filter(|(_, size)| *size > 0)
        .collect()
}

/// The `n` largest folders directly under `root`, without their children.
pub fn largest_folders(root: &ViewNode, n: usize) -> Vec<ViewNode> {
    let mut dirs: Vec<ViewNode> = root
        .children
        .iter()
        .filter(|c| c.kind == ViewKind::Dir)
        .map(|c| ViewNode {
            children: Vec::new(),
            ..c.clone()
        })
        .collect();
    dirs.sort_by_key(|c| std::cmp::Reverse(c.size));
    dirs.truncate(n);
    dirs
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
        view: ViewNode,
    }

    fn mock() -> Mock {
        serde_json::from_str(include_str!("../../assets/mock/snapshot.mock.json")).unwrap()
    }

    fn meta(total: u64, available: u64, scanned: u64) -> SnapshotMeta {
        SnapshotMeta {
            volume: VolumeInfo {
                total,
                available,
                ..Default::default()
            },
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

    fn mock_stats() -> Stats {
        let m = mock();
        let selected: HashSet<String> = m
            .recommendations
            .iter()
            .filter(|r| m.preselected.contains(&r.row_id))
            .flat_map(|r| r.items.iter().map(|i| i.item_id.clone()))
            .collect();
        compute_stats(&m.meta, &m.recommendations, &selected)
    }

    fn close(bytes: u64, want_gb: f64) -> bool {
        (gb(bytes) - want_gb).abs() < 0.05
    }

    #[test]
    fn safe_total_matches_the_mockup() {
        assert!(close(mock_stats().safe.size, 41.7));
    }

    #[test]
    fn safe_counts_ready_items_like_the_mockup() {
        assert_eq!(mock_stats().safe.items, 8);
    }

    #[test]
    fn review_total_matches_the_mockup() {
        assert!(close(mock_stats().review.size, 56.0));
    }

    #[test]
    fn review_counts_ready_items_like_the_mockup() {
        assert_eq!(mock_stats().review.items, 10);
    }

    #[test]
    fn after_cleanup_delta_matches_the_mockup() {
        assert!(close(mock_stats().selected, 52.5));
    }

    #[test]
    fn free_space_matches_the_mockup() {
        assert!(close(mock_stats().available, 39.5));
    }

    #[test]
    fn purgeable_matches_the_mockup() {
        assert!(close(mock_stats().purgeable, 4.2));
    }

    #[test]
    fn not_scanned_matches_the_mockup() {
        assert!(close(mock_stats().not_scanned, 288.0));
    }

    #[test]
    fn selecting_nothing_leaves_free_space_unchanged() {
        let m = mock();
        let s = compute_stats(&m.meta, &m.recommendations, &HashSet::new());
        assert_eq!(s.selected, 0);
    }

    #[test]
    fn counts_only_ready_items() {
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
                items: 1
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

    fn total(id: &str, size: u64) -> CategoryTotal {
        CategoryTotal {
            category: id.into(),
            size,
            folders: 1,
        }
    }

    #[test]
    fn capacity_segments_sum_to_the_volume_total() {
        let meta = meta(100 * GB, 30 * GB, 50 * GB);
        let s = compute_stats(&meta, &[], &HashSet::new());
        let totals = [total("code_projects", 20 * GB), total("ai_models", 30 * GB)];
        let sum: u64 = capacity_segments(&s, &totals).iter().map(|(_, b)| b).sum();
        assert_eq!(sum, 100 * GB);
    }

    #[test]
    fn capacity_segments_drop_empty_parts() {
        let meta = meta(100 * GB, 50 * GB, 50 * GB);
        let s = compute_stats(&meta, &[], &HashSet::new());
        let segments = capacity_segments(&s, &[total("ai_models", 50 * GB)]);
        assert_eq!(
            segments,
            [
                (Segment::Category("ai_models".into()), 50 * GB),
                (Segment::Free, 50 * GB)
            ]
        );
    }

    #[test]
    fn largest_folders_match_the_mockup() {
        let names: Vec<String> = largest_folders(&mock().view, 5)
            .into_iter()
            .map(|n| n.name)
            .collect();
        assert_eq!(names, ["Desktop", "Library", ".ollama", ".npm", ".android"]);
    }

    #[test]
    fn largest_folders_skip_files_and_merged_tiles() {
        let leaf = |name: &str, kind, size| ViewNode {
            id: name.into(),
            name: name.into(),
            path: format!("~/{name}"),
            size,
            category: "dev_tools".into(),
            tier: None,
            rule_id: None,
            kind,
            children: Vec::new(),
        };
        let root = ViewNode {
            children: vec![
                leaf("big.iso", ViewKind::File, 9 * GB),
                leaf("Other", ViewKind::Other, 8 * GB),
                leaf("Projects", ViewKind::Dir, GB),
            ],
            ..leaf("~", ViewKind::Dir, 18 * GB)
        };
        let names: Vec<String> = largest_folders(&root, 5)
            .into_iter()
            .map(|n| n.name)
            .collect();
        assert_eq!(names, ["Projects"]);
    }
}
