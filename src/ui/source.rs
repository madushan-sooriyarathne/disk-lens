//! Where the UI gets its data: the real backend, or the mock snapshot when
//! `DISKLENS_MOCK` is set. Methods block; the model calls them off the UI thread.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde::Deserialize;

use crate::backend;
use crate::config::Config;
use crate::error::{Error, Result};
use crate::recommend::model::{ItemDetails, Recommendation, Subfolder};
use crate::scanner::tree::{ScanEvent, SnapshotMeta};
use crate::state::AppState;
use crate::view::{ViewKind, ViewMode, ViewNode};

pub enum Source {
    Live(Arc<AppState>),
    Mock(MockData),
}

/// `assets/mock/snapshot.mock.json`, which mirrors `docs/mockup.jpg`.
pub struct MockData {
    // Boxed so `Source` stays small next to `Live`.
    file: Box<MockFile>,
    /// `DISKLENS_MOCK=preselect`: start with the mockup's rows selected.
    preselect: bool,
    /// Pause between simulated scan steps.
    step: Duration,
    cancelled: Arc<AtomicBool>,
}

/// The JSON file's shape; `_note` is ignored.
#[derive(Deserialize)]
struct MockFile {
    meta: SnapshotMeta,
    view: ViewNode,
    recommendations: Vec<Recommendation>,
    preselected: Vec<String>,
}

/// Levels of children kept below the requested folder, like the backend's view.
const LEVELS: usize = 2;
const SCAN_STEPS: u64 = 12;

impl MockData {
    pub fn new(preselect: bool) -> Self {
        let file: MockFile =
            serde_json::from_str(include_str!("../../assets/mock/snapshot.mock.json"))
                .expect("assets/mock/snapshot.mock.json is valid");
        Self {
            file: Box::new(file),
            preselect,
            step: Duration::from_millis(150),
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Pause between simulated scan steps (150 ms by default).
    pub fn with_step(mut self, step: Duration) -> Self {
        self.step = step;
        self
    }

    fn start_scan(&self, mut on_event: impl FnMut(ScanEvent) + Send + 'static) {
        self.cancelled.store(false, Ordering::Relaxed);
        let cancelled = self.cancelled.clone();
        let meta = self.file.meta.clone();
        let step = self.step;
        std::thread::spawn(move || {
            for i in 1..=SCAN_STEPS {
                std::thread::sleep(step);
                if cancelled.load(Ordering::Relaxed) {
                    on_event(ScanEvent::Failed {
                        message: "Scan cancelled".into(),
                    });
                    return;
                }
                on_event(ScanEvent::Progress {
                    files: i * 120_000,
                    bytes: (meta.scanned_size as f64 / SCAN_STEPS as f64 * i as f64).round() as u64,
                    current: format!("~/Library/Caches/step-{i}"),
                });
            }
            on_event(ScanEvent::Finished { meta });
        });
    }

    fn view(&self, path: &str) -> Result<ViewNode> {
        let path = if path.is_empty() {
            &self.file.view.path
        } else {
            path
        };
        let hit = find(&self.file.view, path).ok_or_else(|| Error::NotInSnapshot(path.into()))?;
        Ok(truncate(hit, LEVELS))
    }

    fn details(&self, row_id: &str) -> Result<ItemDetails> {
        let row = self
            .file
            .recommendations
            .iter()
            .find(|r| r.row_id == row_id)
            .ok_or_else(|| Error::UnknownRow(row_id.into()))?;
        let mut dirs: Vec<&ViewNode> = row
            .items
            .iter()
            .filter_map(|i| find(&self.file.view, &i.path))
            .flat_map(|n| &n.children)
            .filter(|c| c.kind == ViewKind::Dir)
            .collect();
        dirs.sort_by_key(|n| std::cmp::Reverse(n.size));
        Ok(ItemDetails {
            row_id: row.row_id.clone(),
            description: format!(
                "What {} holds (mock text; real text comes from rules.toml).",
                row.label
            ),
            regenerates: "What happens after removal (mock text).".into(),
            items: row.items.clone(),
            largest_subfolders: dirs
                .into_iter()
                .take(5)
                .map(|c| Subfolder {
                    name: c.name.clone(),
                    path: c.path.clone(),
                    size: c.size,
                })
                .collect(),
        })
    }
}

fn find<'a>(node: &'a ViewNode, path: &str) -> Option<&'a ViewNode> {
    if node.path == path {
        return Some(node);
    }
    node.children
        .iter()
        .filter(|c| c.kind == ViewKind::Dir && path.starts_with(&c.path))
        .find_map(|c| find(c, path))
}

/// Keeps `levels` levels of children below `node`.
fn truncate(node: &ViewNode, levels: usize) -> ViewNode {
    ViewNode {
        children: if levels == 0 {
            Vec::new()
        } else {
            node.children
                .iter()
                .map(|c| truncate(c, levels - 1))
                .collect()
        },
        ..node.clone()
    }
}

impl Source {
    /// `DISKLENS_MOCK=1` serves the mock, `DISKLENS_MOCK=preselect` also selects
    /// the mockup's rows; otherwise the live backend.
    pub fn from_env() -> Self {
        match std::env::var("DISKLENS_MOCK").as_deref() {
            Ok("1" | "true") => Self::Mock(MockData::new(false)),
            Ok("preselect") => Self::Mock(MockData::new(true)),
            _ => Self::Live(Arc::new(AppState::new(Config::from_env()))),
        }
    }

    pub fn load_last(&self) -> Option<SnapshotMeta> {
        match self {
            Self::Live(state) => backend::load_last_snapshot(state),
            Self::Mock(m) => Some(m.file.meta.clone()),
        }
    }

    /// Returns at once; events arrive on another thread.
    pub fn start_scan(&self, on_event: impl FnMut(ScanEvent) + Send + 'static) -> Result<()> {
        match self {
            Self::Live(state) => backend::start_scan(state, on_event),
            Self::Mock(m) => {
                m.start_scan(on_event);
                Ok(())
            }
        }
    }

    pub fn cancel_scan(&self) {
        match self {
            Self::Live(state) => backend::cancel_scan(state),
            Self::Mock(m) => m.cancelled.store(true, Ordering::Relaxed),
        }
    }

    pub fn view(&self, path: &str, mode: ViewMode) -> Result<ViewNode> {
        match self {
            Self::Live(state) => backend::get_view(state, path.to_owned(), mode),
            // The mock has one tree for both modes.
            Self::Mock(m) => m.view(path),
        }
    }

    pub fn recommendations(&self) -> Result<Vec<Recommendation>> {
        match self {
            Self::Live(state) => backend::get_recommendations(state),
            Self::Mock(m) => Ok(m.file.recommendations.clone()),
        }
    }

    pub fn details(&self, row_id: &str) -> Result<ItemDetails> {
        match self {
            Self::Live(state) => backend::get_item_details(state, row_id.to_owned()),
            Self::Mock(m) => m.details(row_id),
        }
    }

    pub fn reveal(&self, path: &str) -> Result<()> {
        match self {
            Self::Live(state) => backend::reveal_in_finder(state, path.to_owned()),
            Self::Mock(_) => Ok(()),
        }
    }

    /// Row IDs to select after loading: the mock's `preselected` with
    /// `DISKLENS_MOCK=preselect`, otherwise none (S-09).
    pub fn preselected_rows(&self) -> Vec<String> {
        match self {
            Self::Mock(m) if m.preselect => m.file.preselected.clone(),
            _ => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recommend::model::Tier;

    const GB: f64 = 1e9;

    fn tier_total(m: &MockData, tier: Tier) -> f64 {
        m.file
            .recommendations
            .iter()
            .filter(|r| r.tier == tier)
            .map(|r| r.size)
            .sum::<u64>() as f64
            / GB
    }

    fn count(m: &MockData, tier: Tier) -> usize {
        m.file
            .recommendations
            .iter()
            .filter(|r| r.tier == tier)
            .count()
    }

    fn close(a: f64, b: f64, tol: f64) {
        assert!((a - b).abs() <= tol, "{a} != {b} (+-{tol})");
    }

    // The mock mirrors the mockup's numbers.
    #[test]
    fn tier_totals_and_counts_match_the_mockup() {
        let m = MockData::new(false);
        close(tier_total(&m, Tier::Safe), 41.7, 0.05);
        close(tier_total(&m, Tier::Review), 56.0, 0.05);
        assert_eq!(count(&m, Tier::Safe), 5);
        assert_eq!(count(&m, Tier::Review), 7);
    }

    #[test]
    fn selection_free_space_and_not_scanned_match_the_mockup() {
        let m = MockData::new(false);
        let selected = m
            .file
            .recommendations
            .iter()
            .filter(|r| m.file.preselected.contains(&r.row_id))
            .map(|r| r.size)
            .sum::<u64>() as f64
            / GB;
        close(selected, 52.5, 0.05);
        let (total, available) = (
            m.file.meta.volume.total as f64,
            m.file.meta.volume.available as f64,
        );
        close((available + selected * GB) / GB, 92.0, 0.5);
        close(
            (total - available - m.file.meta.scanned_size as f64) / GB,
            288.0,
            0.5,
        );
        close(m.file.view.size as f64 / GB, 135.0, 0.05);
    }

    fn depth(n: &ViewNode) -> usize {
        n.children.iter().map(|c| 1 + depth(c)).max().unwrap_or(0)
    }

    fn node(path: &str, children: Vec<ViewNode>) -> ViewNode {
        ViewNode {
            id: path.into(),
            name: path.into(),
            path: path.into(),
            size: 1,
            category: "dev_tools".into(),
            tier: None,
            rule_id: None,
            kind: ViewKind::Dir,
            children,
        }
    }

    #[test]
    fn truncate_keeps_two_levels() {
        let tree = node(
            "a",
            vec![node(
                "a/b",
                vec![node("a/b/c", vec![node("a/b/c/d", vec![])])],
            )],
        );
        let cut = truncate(&tree, LEVELS);
        assert_eq!(depth(&cut), 2);
        assert!(cut.children[0].children[0].children.is_empty());
    }

    #[test]
    fn view_returns_the_requested_folder() {
        let m = MockData::new(false);
        assert!(depth(&m.view(&m.file.view.path).unwrap()) <= LEVELS);
        let child = m
            .file
            .view
            .children
            .iter()
            .find(|c| c.kind == ViewKind::Dir)
            .unwrap();
        assert_eq!(m.view(&child.path).unwrap().path, child.path);
    }

    #[test]
    fn empty_path_means_root() {
        let m = MockData::new(false);
        assert_eq!(m.view("").unwrap(), m.view(&m.file.view.path).unwrap());
    }

    #[test]
    fn unknown_path_and_row_are_errors() {
        let s = Source::Mock(MockData::new(false));
        assert!(matches!(
            s.view("/nowhere", ViewMode::Category),
            Err(Error::NotInSnapshot(_))
        ));
        assert!(matches!(s.details("nope"), Err(Error::UnknownRow(_))));
    }

    #[test]
    fn details_list_the_largest_subfolders() {
        let m = MockData::new(false);
        for row in &m.file.recommendations {
            let d = m.details(&row.row_id).unwrap();
            assert_eq!(d.row_id, row.row_id);
            assert_eq!(d.items.len(), row.items.len());
            assert!(d.largest_subfolders.len() <= 5);
            assert!(
                d.largest_subfolders
                    .windows(2)
                    .all(|w| w[0].size >= w[1].size)
            );
        }
    }

    #[test]
    fn preselection_only_when_asked() {
        let none = Source::Mock(MockData::new(false));
        assert!(none.preselected_rows().is_empty());
        let rows = Source::Mock(MockData::new(true)).preselected_rows();
        assert_eq!(rows.len(), 5);
        assert!(rows.contains(&"npm-cache".to_string()));
    }

    #[test]
    fn mock_scan_emits_progress_then_finished() {
        let m = MockData::new(false).with_step(Duration::ZERO);
        let (tx, rx) = std::sync::mpsc::channel();
        m.start_scan(move |e| {
            let _ = tx.send(e);
        });
        let events: Vec<_> = rx.iter().collect();
        assert_eq!(events.len(), SCAN_STEPS as usize + 1);
        assert!(matches!(events.last(), Some(ScanEvent::Finished { .. })));
        assert!(matches!(
            &events[0],
            ScanEvent::Progress { files: 120_000, .. }
        ));
    }

    #[test]
    fn mock_scan_can_be_cancelled() {
        let s = Source::Mock(MockData::new(false).with_step(Duration::from_millis(20)));
        let (tx, rx) = std::sync::mpsc::channel();
        s.start_scan(move |e| {
            let _ = tx.send(e);
        })
        .unwrap();
        s.cancel_scan();
        let last = rx.iter().last().unwrap();
        assert!(matches!(last, ScanEvent::Failed { message } if message == "Scan cancelled"));
    }
}
