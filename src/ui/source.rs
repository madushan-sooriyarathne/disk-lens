//! Where the UI gets its data: the real backend, or the mock snapshot when
//! `DISKLENS_MOCK` is set. Methods block; the model calls them off the UI thread.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Deserialize;

use crate::backend;
use crate::cleanup::model::{
    CleanupEvent, CleanupPlan, CleanupSummary, Outcome, PlannedAction, PlannedMethod,
};
use crate::cleanup::script;
use crate::config::Config;
use crate::error::{Error, Result};
use crate::recommend::model::{ItemDetails, MethodView, Recommendation, Subfolder, Tier};
use crate::scanner::tree::{ScanEvent, SnapshotMeta};
use crate::settings;
use crate::state::AppState;
use crate::view::{self, CategoryTotal, ViewKind, ViewNode};

use super::selection::within;

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
    /// The last plan built; the mock keeps one at a time.
    plan: Mutex<Option<CleanupPlan>>,
    /// Full Disk Access as the mock reports it. Granted, so mock runs skip onboarding.
    pub full_disk_access: AtomicBool,
    /// In memory only: the mock never writes settings.
    onboarding_done: AtomicBool,
}

/// The JSON file's shape; `_note` is ignored.
#[derive(Deserialize)]
struct MockFile {
    meta: SnapshotMeta,
    view: ViewNode,
    recommendations: Vec<Recommendation>,
    preselected: Vec<String>,
}

const SCAN_STEPS: u64 = 12;

impl MockData {
    pub fn new(preselect: bool) -> Self {
        let mut file: MockFile =
            serde_json::from_str(include_str!("../../assets/mock/snapshot.mock.json"))
                .expect("assets/mock/snapshot.mock.json is valid");
        // Reads "Scanned 2 h ago", like US-13.
        file.meta.scanned_at = crate::cleanup::audit::now_ms() as i64 / 1000 - 2 * 3_600;
        Self {
            file: Box::new(file),
            preselect,
            step: Duration::from_millis(150),
            cancelled: Arc::new(AtomicBool::new(false)),
            plan: Mutex::new(None),
            full_disk_access: AtomicBool::new(true),
            onboarding_done: AtomicBool::new(false),
        }
    }

    /// Pause between simulated scan steps (150 ms by default).
    pub fn with_step(mut self, step: Duration) -> Self {
        self.step = step;
        self
    }

    /// Age of the loaded snapshot, as Unix seconds (2 h ago by default).
    pub fn with_scanned_at(mut self, secs: i64) -> Self {
        self.file.meta.scanned_at = secs;
        self
    }

    fn start_scan(&self, mut on_event: impl FnMut(ScanEvent) + Send + 'static) {
        self.cancelled.store(false, Ordering::Relaxed);
        let cancelled = self.cancelled.clone();
        let mut meta = self.file.meta.clone();
        // A finished scan is fresh, like a real one.
        meta.scanned_at = crate::cleanup::audit::now_ms() as i64 / 1000;
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
        // One level, merged like the backend's view.
        let children = hit
            .children
            .iter()
            .map(|c| ViewNode {
                // Mock item sizes are illustrative and can exceed their folder.
                cleanable: self.cleanable(&c.path).min(c.size),
                children: Vec::new(),
                ..c.clone()
            })
            .collect();
        let mut view = ViewNode {
            cleanable: self.cleanable(&hit.path),
            children: Vec::new(),
            ..hit.clone()
        };
        view.children = view::merge_smaller(&view, children);
        Ok(view)
    }

    /// Bytes of mock items at or below `path`.
    fn cleanable(&self, path: &str) -> u64 {
        self.file
            .recommendations
            .iter()
            .flat_map(|r| &r.items)
            .filter(|i| within(&i.path, path))
            .map(|i| i.size)
            .sum()
    }

    /// Level-1 folders by their own category, or their children's when they have any.
    fn category_totals(&self) -> Vec<CategoryTotal> {
        let mut totals: Vec<CategoryTotal> = Vec::new();
        let tiles = self.file.view.children.iter().flat_map(|c| {
            if c.children.is_empty() {
                std::slice::from_ref(c).iter()
            } else {
                c.children.iter()
            }
        });
        for t in tiles {
            match totals.iter_mut().find(|e| e.category == t.category) {
                Some(e) => {
                    e.size += t.size;
                    e.folders += 1;
                }
                None => totals.push(CategoryTotal {
                    category: t.category.clone(),
                    size: t.size,
                    folders: 1,
                }),
            }
        }
        totals
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

impl MockData {
    /// The selected mock items as a plan; nothing is ever touched.
    fn build_plan(&self, item_ids: &[String], permanent_review: bool) -> CleanupPlan {
        let mut actions = Vec::new();
        let mut permanent_available = false;
        for row in &self.file.recommendations {
            for i in row.items.iter().filter(|i| item_ids.contains(&i.item_id)) {
                let review_trash = row.tier == Tier::Review && i.method == MethodView::Trash;
                permanent_available |= review_trash;
                let method = match &i.method {
                    MethodView::Trash if review_trash && permanent_review => PlannedMethod::Delete,
                    MethodView::Delete => PlannedMethod::Delete,
                    MethodView::Trash => PlannedMethod::Trash,
                    MethodView::Command { display } => PlannedMethod::Command {
                        argv: display.split_whitespace().map(String::from).collect(),
                    },
                };
                actions.push(PlannedAction {
                    item_id: i.item_id.clone(),
                    rule_id: row.rule_id.clone(),
                    label: row.label.clone(),
                    name: i.name.clone(),
                    tier: row.tier,
                    path: i.path.clone(),
                    matched: i.path.clone(),
                    size: i.size,
                    size_is_estimate: row.size_is_estimate,
                    measurable: false,
                    method,
                });
            }
        }
        let plan = CleanupPlan {
            plan_id: "mock-plan".into(),
            snapshot_id: self.file.meta.id.clone(),
            total: actions.iter().map(|a| a.size).sum(),
            trash_total: actions
                .iter()
                .filter(|a| a.method.is_trash())
                .map(|a| a.size)
                .sum(),
            actions,
            permanent_available,
            warnings: Vec::new(),
        };
        *self.plan.lock().expect("mock plan lock") = Some(plan.clone());
        plan
    }

    fn stored_plan(&self, plan_id: &str) -> Result<CleanupPlan> {
        self.plan
            .lock()
            .expect("mock plan lock")
            .clone()
            .filter(|p| p.plan_id == plan_id)
            .ok_or_else(|| Error::UnknownPlan(plan_id.into()))
    }

    /// Walks the plan as a dry run, one step per item.
    fn execute(&self, plan: CleanupPlan, mut on_event: impl FnMut(CleanupEvent) + Send + 'static) {
        let step = self.step;
        std::thread::spawn(move || {
            let mut summary = CleanupSummary {
                dry_run: true,
                ..CleanupSummary::default()
            };
            for a in &plan.actions {
                on_event(CleanupEvent::ItemStarted {
                    item_id: a.item_id.clone(),
                });
                std::thread::sleep(step);
                if a.method.is_trash() {
                    summary.trashed += a.size;
                } else {
                    summary.freed += a.size;
                }
                summary.done += 1;
                on_event(CleanupEvent::ItemFinished {
                    item_id: a.item_id.clone(),
                    freed: a.size,
                    outcome: Outcome::DryRun,
                });
            }
            on_event(CleanupEvent::Finished { summary });
        });
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

    pub fn view(&self, path: &str) -> Result<ViewNode> {
        match self {
            Self::Live(state) => backend::get_view(state, path.to_owned()),
            Self::Mock(m) => m.view(path),
        }
    }

    pub fn category_totals(&self) -> Result<Vec<CategoryTotal>> {
        match self {
            Self::Live(state) => backend::get_category_totals(state),
            Self::Mock(m) => Ok(m.category_totals()),
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

    /// The validated path to preview. Mock paths are not on disk, so the mock has none.
    pub fn quick_look(&self, path: &str) -> Result<std::path::PathBuf> {
        match self {
            Self::Live(state) => backend::quick_look(state, path.to_owned()),
            Self::Mock(_) => Err(Error::NotInSnapshot(path.into())),
        }
    }

    pub fn reveal(&self, path: &str) -> Result<()> {
        match self {
            Self::Live(state) => backend::reveal_in_finder(state, path.to_owned()),
            Self::Mock(_) => Ok(()),
        }
    }

    pub fn full_disk_access(&self) -> bool {
        match self {
            Self::Live(state) => backend::check_permissions(state),
            Self::Mock(m) => m.full_disk_access.load(Ordering::Relaxed),
        }
    }

    pub fn open_fda_settings(&self) -> Result<()> {
        match self {
            Self::Live(_) => backend::open_fda_settings(),
            Self::Mock(_) => Ok(()),
        }
    }

    pub fn onboarding_done(&self) -> bool {
        match self {
            Self::Live(state) => settings::load(&state.config.data_dir).onboarding_done,
            Self::Mock(m) => m.onboarding_done.load(Ordering::Relaxed),
        }
    }

    pub fn set_onboarding_done(&self) -> Result<()> {
        match self {
            Self::Live(state) => {
                let mut s = settings::load(&state.config.data_dir);
                s.onboarding_done = true;
                settings::save(&state.config.data_dir, &s)
            }
            Self::Mock(m) => {
                m.onboarding_done.store(true, Ordering::Relaxed);
                Ok(())
            }
        }
    }

    /// Cleanups only log what they would do (S-08). The mock never changes anything.
    pub fn is_dry_run(&self) -> bool {
        match self {
            Self::Live(state) => state.config.dry_run,
            Self::Mock(_) => true,
        }
    }

    pub fn build_plan(
        &self,
        snapshot_id: &str,
        item_ids: Vec<String>,
        permanent_review: bool,
    ) -> Result<CleanupPlan> {
        match self {
            Self::Live(state) => backend::build_cleanup_plan(
                state,
                snapshot_id.to_owned(),
                item_ids,
                permanent_review,
            ),
            Self::Mock(m) => Ok(m.build_plan(&item_ids, permanent_review)),
        }
    }

    pub fn export_script(&self, plan_id: &str) -> Result<String> {
        match self {
            Self::Live(state) => backend::export_script(state, plan_id),
            Self::Mock(m) => Ok(script::render(
                &m.stored_plan(plan_id)?,
                crate::cleanup::audit::now_ms() as i64 / 1000,
            )),
        }
    }

    /// Returns at once; events arrive on another thread.
    pub fn execute_cleanup(
        &self,
        plan_id: &str,
        on_event: impl FnMut(CleanupEvent) + Send + 'static,
    ) -> Result<()> {
        match self {
            Self::Live(state) => backend::execute_cleanup(state, plan_id.to_owned(), on_event),
            Self::Mock(m) => {
                m.execute(m.stored_plan(plan_id)?, on_event);
                Ok(())
            }
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

    #[test]
    fn view_returns_the_requested_folder() {
        let m = MockData::new(false);
        assert!(
            depth(&m.view(&m.file.view.path).unwrap()) <= 2,
            "Smaller lists its items"
        );
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
    fn mock_home_shows_the_mockup_tiles() {
        let m = MockData::new(false);
        let names: Vec<String> = m
            .view("")
            .unwrap()
            .children
            .into_iter()
            .map(|c| c.name)
            .collect();
        assert_eq!(
            names,
            [
                "Desktop",
                "Library",
                ".ollama",
                ".npm",
                ".android",
                ".gradle",
                "Documents",
                "Pictures",
                "Music",
                ".cache",
                ".vscode",
                "Downloads",
                "3 smaller items"
            ]
        );
    }

    #[test]
    fn mock_smaller_tile_holds_the_rest_of_home() {
        let m = MockData::new(false);
        let smaller = m.view("").unwrap().children.pop().unwrap();
        assert_eq!(smaller.size, 9_800_000_000);
    }

    #[test]
    fn category_totals_sum_to_the_root_size() {
        let m = MockData::new(false);
        let sum: u64 = m.category_totals().iter().map(|t| t.size).sum();
        assert_eq!(sum, m.file.view.size);
    }

    #[test]
    fn empty_path_means_root() {
        let m = MockData::new(false);
        assert_eq!(m.view("").unwrap(), m.view(&m.file.view.path).unwrap());
    }

    #[test]
    fn unknown_path_and_row_are_errors() {
        let s = Source::Mock(MockData::new(false));
        assert!(matches!(s.view("/nowhere"), Err(Error::NotInSnapshot(_))));
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
