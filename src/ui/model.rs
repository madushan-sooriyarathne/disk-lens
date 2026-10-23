//! App state and actions (owner: A4). Replaces the React stores
//! `legacy-web/src/store/{scan,view,selection}.ts`.
//!
//! Frozen contract: the public fields and method signatures below. Private
//! fields and helpers may be added freely.

use std::collections::HashSet;
use std::sync::Arc;
use std::sync::mpsc::{self, TryRecvError};
use std::time::Duration;

use gpui::{AppContext, Context, FocusHandle, Subscription, Task, Window};

use super::explorer::ExplorerState;
use super::recommendations::PanelState;
use super::selection;
use super::source::Source;
use crate::error::Error;
use crate::recommend::model::{ItemDetails, Recommendation};
use crate::scanner::tree::{ScanEvent, SnapshotMeta};
use crate::view::{ViewMode, ViewNode};

/// How often the UI drains scan events from the scanner thread.
const SCAN_POLL: Duration = Duration::from_millis(100);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Loading,
    Idle,
    Scanning,
    Ready,
    Error,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Progress {
    pub files: u64,
    pub bytes: u64,
    pub current: String,
}

pub struct AppModel {
    pub source: Arc<Source>,
    pub phase: Phase,
    pub meta: Option<SnapshotMeta>,
    pub progress: Progress,
    /// Last scan failure or cancel notice; shown by the idle, error and banner views.
    pub error: Option<String>,
    pub mode: ViewMode,
    /// Path of the scan root, as the snapshot reports it.
    pub root_path: String,
    /// Folder the explorer shows.
    pub path: String,
    pub view: Option<ViewNode>,
    /// `ViewNode::id` under the pointer.
    pub hovered: Option<String>,
    pub recommendations: Vec<Recommendation>,
    /// Selected item IDs. Empty by default (S-09); only ready items.
    pub selected: HashSet<String>,
    /// Row whose detail drawer is open.
    pub focused_row: Option<String>,
    /// Details of `focused_row`, once loaded.
    pub details: Option<ItemDetails>,
    /// Focused at startup so window-wide keys (root.rs) dispatch.
    pub focus_handle: FocusHandle,
    pub explorer: ExplorerState,
    pub panel: PanelState,
    /// Bumped per view request; an older response is dropped when it arrives late.
    view_seq: u64,
    /// Polls the running scan's events; dropped when a new scan starts.
    scan_task: Option<Task<()>>,
    _activation: Subscription,
}

/// Runs `work` against the source off the UI thread, then hands the result to
/// `then` on it and repaints. Does nothing if the model is gone by then.
fn off_thread<R: Send + 'static>(
    source: &Arc<Source>,
    cx: &mut Context<AppModel>,
    work: impl FnOnce(&Source) -> R + Send + 'static,
    then: impl FnOnce(&mut AppModel, R, &mut Context<AppModel>) + 'static,
) {
    let source = source.clone();
    let task = cx.background_spawn(async move { work(&source) });
    cx.spawn(async move |this, cx| {
        let result = task.await;
        this.update(cx, |model, cx| {
            then(model, result, cx);
            cx.notify();
        })
        .ok();
    })
    .detach();
}

/// The folder above `path`, like `parentOf` in the old view store.
fn parent_of(path: &str) -> &str {
    path.rfind('/')
        .map(|i| &path[..i])
        .filter(|p| !p.is_empty())
        .unwrap_or("/")
}

impl AppModel {
    /// Builds the model, focuses it, refreshes recommendations whenever the
    /// window becomes active (blocked states depend on running apps), and
    /// loads the last snapshot in the background.
    pub fn new(source: Source, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        window.focus(&focus_handle, cx);
        let activation = cx.observe_window_activation(window, |this, window, cx| {
            if window.is_window_active() && this.phase == Phase::Ready {
                this.refresh(cx);
            }
        });
        let source = Arc::new(source);
        // Last snapshot -> Ready, none -> Idle.
        off_thread(
            &source,
            cx,
            |s| s.load_last(),
            |this, meta, cx| match meta {
                Some(meta) => this.enter(meta, cx),
                None => this.phase = Phase::Idle,
            },
        );
        Self {
            source,
            phase: Phase::Loading,
            meta: None,
            progress: Progress::default(),
            error: None,
            mode: ViewMode::Category,
            root_path: String::new(),
            path: String::new(),
            view: None,
            hovered: None,
            recommendations: Vec::new(),
            selected: HashSet::new(),
            focused_row: None,
            details: None,
            focus_handle,
            explorer: ExplorerState::default(),
            panel: PanelState::default(),
            view_seq: 0,
            scan_task: None,
            _activation: activation,
        }
    }

    /// Shows a snapshot: the root folder, fresh rows and the default selection.
    fn enter(&mut self, meta: SnapshotMeta, cx: &mut Context<Self>) {
        self.view_seq += 1;
        let seq = self.view_seq;
        let mode = self.mode;
        let root = meta.root.clone();
        off_thread(
            &self.source,
            cx,
            move |s| {
                Ok::<_, Error>((
                    s.view(&root, mode)?,
                    s.recommendations()?,
                    s.preselected_rows(),
                ))
            },
            move |this, loaded, cx| {
                if this.view_seq != seq {
                    return;
                }
                match loaded {
                    Ok((view, recommendations, preselected)) => {
                        let mut selected = HashSet::new();
                        for row in &recommendations {
                            if preselected.contains(&row.row_id) {
                                selected.extend(
                                    selection::ready_ids(row).into_iter().map(String::from),
                                );
                            }
                        }
                        this.root_path = meta.root.clone();
                        this.path = meta.root.clone();
                        this.meta = Some(meta);
                        this.view = Some(view);
                        this.hovered = None;
                        this.recommendations = recommendations;
                        this.selected = selected;
                        this.focused_row = None;
                        this.details = None;
                        this.error = None;
                        this.phase = Phase::Ready;
                    }
                    Err(e) => {
                        this.phase = Phase::Error;
                        this.error = Some(e.to_string());
                    }
                }
                cx.notify();
            },
        );
    }

    pub fn start_scan(&mut self, cx: &mut Context<Self>) {
        self.phase = Phase::Scanning;
        self.progress = Progress::default();
        self.error = None;
        cx.notify();
        let (tx, rx) = mpsc::channel();
        let started = self.source.start_scan(move |event| {
            // The receiver is gone once a newer scan replaced this one.
            tx.send(event).ok();
        });
        if let Err(e) = started {
            self.phase = Phase::Error;
            self.error = Some(e.to_string());
            return;
        }
        self.scan_task = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(SCAN_POLL).await;
                let mut events = Vec::new();
                let mut gone = false;
                loop {
                    match rx.try_recv() {
                        Ok(event) => events.push(event),
                        Err(TryRecvError::Empty) => break,
                        Err(TryRecvError::Disconnected) => {
                            gone = true;
                            break;
                        }
                    }
                }
                let done = this.update(cx, |model, cx| {
                    let mut done = false;
                    for event in events {
                        done |= model.on_scan_event(event, cx);
                    }
                    if gone && !done {
                        model.on_scan_event(
                            ScanEvent::Failed {
                                message: "Scan stopped unexpectedly".into(),
                            },
                            cx,
                        );
                        done = true;
                    }
                    cx.notify();
                    done
                });
                if done.unwrap_or(true) {
                    break;
                }
            }
        }));
    }

    /// Applies one scanner event; true when it ended the scan.
    fn on_scan_event(&mut self, event: ScanEvent, cx: &mut Context<Self>) -> bool {
        match event {
            ScanEvent::Progress {
                files,
                bytes,
                current,
            } => {
                self.progress = Progress {
                    files,
                    bytes,
                    current,
                };
                false
            }
            ScanEvent::Finished { meta } => {
                self.enter(meta, cx);
                true
            }
            ScanEvent::Failed { message } => {
                self.phase = if self.meta.is_some() {
                    Phase::Ready
                } else {
                    Phase::Idle
                };
                self.error = Some(message);
                true
            }
        }
    }

    pub fn cancel_scan(&mut self, cx: &mut Context<Self>) {
        self.source.cancel_scan();
    }

    /// Loads the view of `path` in `mode`; the model changes once it arrives.
    fn load_view(&mut self, path: String, mode: ViewMode, cx: &mut Context<Self>) {
        self.view_seq += 1;
        let seq = self.view_seq;
        let wanted = path.clone();
        off_thread(
            &self.source,
            cx,
            move |s| s.view(&wanted, mode),
            move |this, loaded, _| {
                if this.view_seq != seq {
                    return;
                }
                match loaded {
                    Ok(view) => {
                        this.path = path;
                        this.mode = mode;
                        this.view = Some(view);
                        this.hovered = None;
                    }
                    Err(e) => tracing::warn!("could not load {path}: {e}"),
                }
            },
        );
    }

    /// Shows the folder at `path` in the explorer.
    pub fn navigate(&mut self, path: String, cx: &mut Context<Self>) {
        self.load_view(path, self.mode, cx);
    }

    /// Goes up one folder; does nothing at the scan root.
    pub fn up(&mut self, cx: &mut Context<Self>) {
        if self.path != self.root_path {
            self.navigate(parent_of(&self.path).to_owned(), cx);
        }
    }

    pub fn set_mode(&mut self, mode: ViewMode, cx: &mut Context<Self>) {
        self.load_view(self.path.clone(), mode, cx);
    }

    pub fn hover(&mut self, node_id: Option<String>, cx: &mut Context<Self>) {
        if self.hovered != node_id {
            self.hovered = node_id;
            cx.notify();
        }
    }

    /// Toggles one item; blocked or unknown items are ignored.
    pub fn toggle_item(&mut self, item_id: &str, cx: &mut Context<Self>) {
        let ready = self
            .recommendations
            .iter()
            .any(|r| selection::ready_ids(r).contains(&item_id));
        if !ready {
            return;
        }
        if !self.selected.remove(item_id) {
            self.selected.insert(item_id.to_owned());
        }
        cx.notify();
    }

    pub fn toggle_row(&mut self, row_id: &str, cx: &mut Context<Self>) {
        if let Some(row) = self.recommendations.iter().find(|r| r.row_id == row_id) {
            self.selected = selection::toggle_row(row, &self.selected);
            cx.notify();
        }
    }

    pub fn select_safe(&mut self, cx: &mut Context<Self>) {
        self.selected = selection::select_safe(&self.recommendations, &self.selected);
        cx.notify();
    }

    /// Opens the detail drawer for `row_id` and loads its details; `None` closes it.
    pub fn focus_row(&mut self, row_id: Option<String>, cx: &mut Context<Self>) {
        self.focused_row = row_id.clone();
        self.details = None;
        cx.notify();
        let Some(row_id) = row_id else { return };
        let wanted = row_id.clone();
        off_thread(
            &self.source,
            cx,
            move |s| s.details(&wanted),
            move |this, loaded, _| {
                // Another row may have been focused while this one loaded.
                if this.focused_row.as_deref() != Some(row_id.as_str()) {
                    return;
                }
                match loaded {
                    Ok(details) => this.details = Some(details),
                    Err(e) => tracing::warn!("could not load details of {row_id}: {e}"),
                }
            },
        );
    }

    /// Re-evaluates blocked states and drops selections that became stale.
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        off_thread(
            &self.source,
            cx,
            |s| s.recommendations(),
            |this, loaded, _| match loaded {
                Ok(recommendations) => {
                    this.selected = selection::prune_selection(&recommendations, &this.selected);
                    this.recommendations = recommendations;
                }
                Err(e) => tracing::warn!("could not refresh recommendations: {e}"),
            },
        );
    }

    /// Shows a scanned path in Finder.
    pub fn reveal(&mut self, path: String, cx: &mut Context<Self>) {
        off_thread(
            &self.source,
            cx,
            move |s| s.reveal(&path).map_err(|e| format!("{path}: {e}")),
            |_, result, _| {
                if let Err(e) = result {
                    tracing::warn!("could not reveal {e}");
                }
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use gpui::{Entity, TestAppContext, VisualTestContext};

    use super::*;
    use crate::recommend::model::{ItemStatus, Tier};
    use crate::ui::source::MockData;

    /// An app model over the mock, with scan steps that take no time.
    fn setup(
        cx: &mut TestAppContext,
        preselect: bool,
    ) -> (Entity<AppModel>, &mut VisualTestContext) {
        let cx = cx.add_empty_window();
        let source = Source::Mock(MockData::new(preselect).with_step(Duration::ZERO));
        let model = cx.update(|window, cx| cx.new(|cx| AppModel::new(source, window, cx)));
        cx.run_until_parked();
        (model, cx)
    }

    /// Runs tasks, scanner thread and the poll clock until `done` holds.
    fn settle(
        cx: &mut VisualTestContext,
        model: &Entity<AppModel>,
        done: impl Fn(&AppModel) -> bool,
    ) {
        for _ in 0..500 {
            cx.run_until_parked();
            if model.read_with(cx, |m, _| done(m)) {
                return;
            }
            std::thread::sleep(Duration::from_millis(2));
            cx.executor().advance_clock(SCAN_POLL);
        }
        panic!("model did not settle");
    }

    #[test]
    fn parent_of_matches_the_old_store() {
        assert_eq!(parent_of("~/Library/Caches"), "~/Library");
        assert_eq!(parent_of("/Users/a"), "/Users");
        assert_eq!(parent_of("/Users"), "/");
        assert_eq!(parent_of("~"), "/");
    }

    #[gpui::test]
    fn init_reaches_ready_with_an_empty_selection(cx: &mut TestAppContext) {
        let (model, cx) = setup(cx, false);
        model.read_with(cx, |m, _| {
            assert_eq!(m.phase, Phase::Ready);
            assert!(m.meta.is_some());
            assert_eq!(m.path, m.root_path);
            assert!(m.view.is_some());
            assert_eq!(m.recommendations.len(), 12);
            // S-09: nothing is selected by default.
            assert!(m.selected.is_empty());
            assert!(m.error.is_none());
        });
    }

    #[gpui::test]
    #[ignore = "needs ui::selection (A1)"]
    fn preselect_selects_the_mockup_rows(cx: &mut TestAppContext) {
        let (model, cx) = setup(cx, true);
        model.read_with(cx, |m, _| {
            let want: HashSet<String> = m
                .recommendations
                .iter()
                .filter(|r| m.source.preselected_rows().contains(&r.row_id))
                .flat_map(|r| r.items.iter().map(|i| i.item_id.clone()))
                .collect();
            assert!(!want.is_empty());
            assert_eq!(m.selected, want);
        });
    }

    #[gpui::test]
    fn navigate_and_up_stop_at_the_root(cx: &mut TestAppContext) {
        let (model, cx) = setup(cx, false);
        let (root, child) = model.read_with(cx, |m, _| {
            let child = m
                .view
                .as_ref()
                .unwrap()
                .children
                .iter()
                .find(|c| c.kind == crate::view::ViewKind::Dir)
                .unwrap()
                .path
                .clone();
            (m.root_path.clone(), child)
        });
        model.update(cx, |m, cx| m.navigate(child.clone(), cx));
        cx.run_until_parked();
        model.read_with(cx, |m, _| {
            assert_eq!(m.path, child);
            assert_eq!(m.view.as_ref().unwrap().path, child);
        });
        model.update(cx, |m, cx| m.up(cx));
        cx.run_until_parked();
        model.read_with(cx, |m, _| assert_eq!(m.path, root));
        // At the root, `up` does nothing.
        model.update(cx, |m, cx| m.up(cx));
        cx.run_until_parked();
        model.read_with(cx, |m, _| {
            assert_eq!(m.path, root);
            assert_eq!(m.view.as_ref().unwrap().path, root);
        });
    }

    #[gpui::test]
    fn set_mode_reloads_the_same_folder(cx: &mut TestAppContext) {
        let (model, cx) = setup(cx, false);
        model.update(cx, |m, cx| m.set_mode(ViewMode::Cleanup, cx));
        cx.run_until_parked();
        model.read_with(cx, |m, _| {
            assert_eq!(m.mode, ViewMode::Cleanup);
            assert_eq!(m.path, m.root_path);
        });
    }

    #[gpui::test]
    fn hover_only_sets_the_id(cx: &mut TestAppContext) {
        let (model, cx) = setup(cx, false);
        model.update(cx, |m, cx| m.hover(Some("n1".into()), cx));
        model.read_with(cx, |m, _| assert_eq!(m.hovered.as_deref(), Some("n1")));
        model.update(cx, |m, cx| m.hover(None, cx));
        model.read_with(cx, |m, _| assert!(m.hovered.is_none()));
    }

    #[gpui::test]
    fn focus_row_loads_details_and_none_closes(cx: &mut TestAppContext) {
        let (model, cx) = setup(cx, false);
        model.update(cx, |m, cx| m.focus_row(Some("npm-cache".into()), cx));
        model.read_with(cx, |m, _| assert!(m.details.is_none()));
        cx.run_until_parked();
        model.read_with(cx, |m, _| {
            assert_eq!(m.focused_row.as_deref(), Some("npm-cache"));
            assert_eq!(m.details.as_ref().unwrap().row_id, "npm-cache");
        });
        // A late answer for a row that is no longer focused is dropped.
        model.update(cx, |m, cx| {
            m.focus_row(Some("ollama-models".into()), cx);
            m.focus_row(None, cx);
        });
        cx.run_until_parked();
        model.read_with(cx, |m, _| {
            assert!(m.focused_row.is_none());
            assert!(m.details.is_none());
        });
    }

    #[gpui::test]
    #[ignore = "needs ui::selection (A1)"]
    fn toggle_item_ignores_blocked_and_unknown_ids(cx: &mut TestAppContext) {
        let (model, cx) = setup(cx, false);
        let (ready, blocked) = model.update(cx, |m, cx| {
            let item = &mut m.recommendations[0].items[0];
            let ready = item.item_id.clone();
            let mut blocked = item.clone();
            blocked.item_id = "blocked-item".into();
            blocked.status = ItemStatus::Blocked {
                reason: "running".into(),
            };
            m.recommendations[0].items.push(blocked);
            m.toggle_item("blocked-item", cx);
            m.toggle_item("no-such-item", cx);
            assert!(m.selected.is_empty());
            m.toggle_item(&ready, cx);
            assert!(m.selected.contains(&ready));
            m.toggle_item(&ready, cx);
            (ready, "blocked-item")
        });
        model.read_with(cx, |m, _| {
            assert!(!m.selected.contains(&ready));
            assert!(!m.selected.contains(blocked));
        });
    }

    #[gpui::test]
    #[ignore = "needs ui::selection (A1)"]
    fn toggle_row_selects_then_clears(cx: &mut TestAppContext) {
        let (model, cx) = setup(cx, false);
        model.update(cx, |m, cx| m.toggle_row("npm-cache", cx));
        model.read_with(cx, |m, _| assert!(!m.selected.is_empty()));
        model.update(cx, |m, cx| m.toggle_row("npm-cache", cx));
        model.read_with(cx, |m, _| assert!(m.selected.is_empty()));
    }

    #[gpui::test]
    #[ignore = "needs ui::selection (A1)"]
    fn select_safe_skips_blocked_items(cx: &mut TestAppContext) {
        let (model, cx) = setup(cx, false);
        let blocked_id = model.update(cx, |m, cx| {
            let row = m
                .recommendations
                .iter_mut()
                .find(|r| r.tier == Tier::Safe)
                .unwrap();
            row.items[0].status = ItemStatus::Blocked {
                reason: "Xcode is running".into(),
            };
            let id = row.items[0].item_id.clone();
            m.select_safe(cx);
            id
        });
        model.read_with(cx, |m, _| {
            assert!(!m.selected.is_empty());
            assert!(!m.selected.contains(&blocked_id));
            // Review items stay out.
            let review: HashSet<_> = m
                .recommendations
                .iter()
                .filter(|r| r.tier == Tier::Review)
                .flat_map(|r| r.items.iter().map(|i| i.item_id.clone()))
                .collect();
            assert!(m.selected.is_disjoint(&review));
        });
    }

    #[gpui::test]
    #[ignore = "needs ui::selection (A1)"]
    fn refresh_prunes_ids_that_disappeared(cx: &mut TestAppContext) {
        let (model, cx) = setup(cx, false);
        let kept = model.update(cx, |m, cx| {
            let kept = m.recommendations[0].items[0].item_id.clone();
            m.selected.insert(kept.clone());
            m.selected.insert("vanished".into());
            m.refresh(cx);
            kept
        });
        cx.run_until_parked();
        model.read_with(cx, |m, _| {
            assert!(m.selected.contains(&kept));
            assert!(!m.selected.contains("vanished"));
        });
    }

    #[gpui::test]
    fn a_mock_scan_reaches_ready(cx: &mut TestAppContext) {
        let (model, cx) = setup(cx, false);
        model.update(cx, |m, cx| {
            m.selected.insert("stale".into());
            m.start_scan(cx);
            assert_eq!(m.phase, Phase::Scanning);
        });
        settle(cx, &model, |m| m.phase != Phase::Scanning);
        model.read_with(cx, |m, _| {
            assert_eq!(m.phase, Phase::Ready);
            assert!(m.error.is_none());
            // A fresh scan starts from an empty selection.
            assert!(m.selected.is_empty());
            assert_eq!(m.path, m.root_path);
        });
    }

    #[gpui::test]
    fn a_cancelled_scan_reports_the_notice(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        // Slow enough that the cancel lands before the first step ends.
        let source = Source::Mock(MockData::new(false).with_step(Duration::from_millis(30)));
        let model = cx.update(|window, cx| cx.new(|cx| AppModel::new(source, window, cx)));
        cx.run_until_parked();
        model.update(cx, |m, cx| {
            m.start_scan(cx);
            m.cancel_scan(cx);
        });
        settle(cx, &model, |m| m.phase != Phase::Scanning);
        model.read_with(cx, |m, _| {
            // A snapshot was already loaded, so the explorer comes back.
            assert_eq!(m.phase, Phase::Ready);
            assert_eq!(m.error.as_deref(), Some("Scan cancelled"));
        });
    }
}
