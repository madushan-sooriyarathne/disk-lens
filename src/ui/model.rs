//! App state and actions: scan phase, explorer path and mode, recommendations,
//! selection and the open drawer. Views read the public fields and call the methods.

use std::collections::HashSet;
use std::sync::Arc;
use std::sync::mpsc::{self, TryRecvError};
use std::time::Duration;

use gpui::{AppContext, ClipboardItem, Context, FocusHandle, Subscription, Task, Window};

use super::explorer::ExplorerState;
use super::recommendations::PanelState;
use super::selection;
use super::source::Source;
use crate::cleanup::model::{CleanupEvent, CleanupPlan, CleanupSummary};
use crate::error::Error;
use crate::recommend::model::{ItemDetails, Recommendation};
use crate::scanner::tree::{ScanEvent, SnapshotMeta};
use crate::update;
use crate::view::{ViewMode, ViewNode};

/// How often the UI drains scan events from the scanner thread.
const SCAN_POLL: Duration = Duration::from_millis(100);
/// How long a toast stays up.
const TOAST: Duration = Duration::from_secs(3);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Loading,
    /// Full Disk Access is missing and the user has not moved past it yet (US-01).
    Onboarding,
    Idle,
    Scanning,
    Ready,
    Error,
}

/// The cleanup dialog (PRD section 7.6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cleanup {
    /// Waiting for the plan.
    Planning,
    Confirm {
        plan: CleanupPlan,
        /// "Delete permanently instead of moving to Trash".
        permanent: bool,
    },
    Running {
        plan: CleanupPlan,
        /// Items finished so far.
        done: usize,
        /// Item being cleaned.
        current: Option<String>,
    },
    Results {
        summary: CleanupSummary,
    },
}

/// Where the in-app update stands (banner in `screens.rs`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum UpdateState {
    #[default]
    Idle,
    Available {
        version: String,
    },
    Installing,
    /// Installed on disk; the running app is still the old build until it restarts.
    Ready,
    Failed(String),
}

/// Counters of the running scan.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScanProgress {
    pub files: u64,
    pub bytes: u64,
    pub current: String,
}

pub struct AppModel {
    pub source: Arc<Source>,
    pub phase: Phase,
    pub meta: Option<SnapshotMeta>,
    /// The running scan, if any. Independent of `phase`: a rescan keeps the results on screen.
    pub scan: Option<ScanProgress>,
    /// Whether the app can read every folder (PRD 7.8); rechecked on window focus.
    pub full_disk_access: bool,
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
    /// Why the details of `focused_row` could not be loaded.
    pub details_error: Option<String>,
    /// Open cleanup dialog, if any.
    pub cleanup: Option<Cleanup>,
    /// Short notice at the bottom of the window ("Script copied").
    pub toast: Option<String>,
    /// Focused at startup so window-wide keys (root.rs) dispatch.
    pub focus_handle: FocusHandle,
    pub explorer: ExplorerState,
    pub panel: PanelState,
    pub update_state: UpdateState,
    /// The release behind `UpdateState::Available`, kept for the install.
    pending_update: Option<update::Update>,
    /// Bumped per view request; an older response is dropped when it arrives late.
    view_seq: u64,
    /// Polls the running scan's events; dropped when a new scan starts.
    scan_task: Option<Task<()>>,
    /// Polls the running cleanup's events.
    cleanup_task: Option<Task<()>>,
    /// Bumped per toast, so an older toast's timer does not hide a newer one.
    toast_seq: u64,
    _activation: Subscription,
    /// Repaints when macOS switches between light and dark.
    _appearance: Subscription,
    /// Refocuses the window root when the focused element disappears.
    _focus_lost: Subscription,
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

/// Looks for a newer release once. A missing release or no network is normal, so failures
/// only reach the log.
fn check_for_update(cx: &mut Context<AppModel>) {
    let task = cx.background_spawn(async { update::check() });
    cx.spawn(async move |this, cx| match task.await {
        Ok(Some(found)) => {
            this.update(cx, |model, cx| {
                model.update_state = UpdateState::Available {
                    version: found.version.clone(),
                };
                model.pending_update = Some(found);
                cx.notify();
            })
            .ok();
        }
        Ok(None) => {}
        Err(e) => tracing::warn!("{e}"),
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

/// `path` and its parents down to `root`, nearest first. A path outside `root` gives just `root`.
fn ancestors<'a>(path: &'a str, root: &'a str) -> Vec<&'a str> {
    let inside = path == root
        || path
            .strip_prefix(root.trim_end_matches('/'))
            .is_some_and(|rest| rest.starts_with('/'));
    if !inside {
        return vec![root];
    }
    let mut chain = vec![path];
    let mut current = path;
    while current != root {
        let parent = parent_of(current);
        // A root with a trailing slash is never reached by trimming; stop at the top.
        if parent == current {
            chain.push(root);
            break;
        }
        chain.push(parent);
        current = parent;
    }
    chain
}

impl AppModel {
    /// Builds the model, focuses it, refreshes recommendations whenever the
    /// window becomes active (blocked states depend on running apps), and
    /// loads the last snapshot in the background.
    pub fn new(source: Source, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        window.focus(&focus_handle, cx);
        let activation = cx.observe_window_activation(window, |this, window, cx| {
            if !window.is_window_active() {
                return;
            }
            // The user may have just granted access in System Settings.
            this.recheck_access(cx);
            if this.phase == Phase::Ready {
                this.refresh(cx);
            }
        });
        let appearance = cx.observe_window_appearance(window, |_, _, cx| cx.notify());
        // A focused tile or row vanishes on drill-down or rescan. Window-wide keys
        // (Esc, Tab) dispatch from the focused element, so focus must go back to the root.
        let focus_lost = cx.on_focus_lost(window, |this, window, cx| {
            window.focus(&this.focus_handle, cx);
        });
        let source = Arc::new(source);
        let (is_mock, is_sandboxed) = match source.as_ref() {
            Source::Live(state) => (false, state.config.scan_root.is_some()),
            Source::Mock(_) => (true, false),
        };
        if update::is_enabled(is_mock, is_sandboxed) {
            check_for_update(cx);
        }
        // Onboarding first when access is missing and was never dismissed.
        off_thread(
            &source,
            cx,
            |s| (s.full_disk_access(), s.onboarding_done()),
            |this, (granted, done), cx| {
                this.full_disk_access = granted;
                if granted || done {
                    this.load_last(cx);
                } else {
                    this.phase = Phase::Onboarding;
                }
            },
        );
        Self {
            source,
            phase: Phase::Loading,
            meta: None,
            scan: None,
            full_disk_access: false,
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
            details_error: None,
            cleanup: None,
            toast: None,
            focus_handle,
            explorer: ExplorerState::default(),
            panel: PanelState::default(),
            update_state: UpdateState::Idle,
            pending_update: None,
            view_seq: 0,
            scan_task: None,
            cleanup_task: None,
            toast_seq: 0,
            _activation: activation,
            _appearance: appearance,
            _focus_lost: focus_lost,
        }
    }

    /// Last snapshot -> Ready, none -> Idle.
    fn load_last(&mut self, cx: &mut Context<Self>) {
        self.phase = Phase::Loading;
        off_thread(
            &self.source,
            cx,
            |s| s.load_last(),
            |this, meta, cx| match meta {
                Some(meta) => this.enter(meta, cx),
                None => this.phase = Phase::Idle,
            },
        );
    }

    /// Installing or restarting under a running scan or cleanup would cut it short.
    pub fn can_install_update(&self) -> bool {
        self.scan.is_none() && self.cleanup.is_none()
    }

    pub fn install_update(&mut self, cx: &mut Context<Self>) {
        if !self.can_install_update() {
            return;
        }
        let Some(found) = self.pending_update.take() else {
            return;
        };
        self.update_state = UpdateState::Installing;
        cx.notify();
        let task = cx.background_spawn(async move { update::install(&found) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |model, cx| {
                model.update_state = match result {
                    Ok(()) => UpdateState::Ready,
                    Err(e) => {
                        tracing::warn!("{e}");
                        UpdateState::Failed(e.to_string())
                    }
                };
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn dismiss_update(&mut self, cx: &mut Context<Self>) {
        self.update_state = UpdateState::Idle;
        cx.notify();
    }

    /// Updates `full_disk_access`, and leaves onboarding once access is granted.
    pub fn recheck_access(&mut self, cx: &mut Context<Self>) {
        off_thread(
            &self.source,
            cx,
            |s| s.full_disk_access(),
            |this, granted, cx| {
                this.full_disk_access = granted;
                if granted && this.phase == Phase::Onboarding {
                    this.load_last(cx);
                }
            },
        );
    }

    /// Opens the Full Disk Access pane; the activation recheck notices a grant.
    pub fn open_fda_settings(&mut self, cx: &mut Context<Self>) {
        off_thread(
            &self.source,
            cx,
            |s| s.open_fda_settings(),
            |this, opened, cx| {
                if let Err(e) = opened {
                    tracing::warn!("could not open System Settings: {e}");
                    this.show_toast("Could not open System Settings".into(), cx);
                }
            },
        );
    }

    /// Leaves onboarding without access and remembers that (US-01).
    pub fn continue_without_access(&mut self, cx: &mut Context<Self>) {
        off_thread(
            &self.source,
            cx,
            |s| s.set_onboarding_done(),
            |this, saved, cx| {
                // Not saving only means onboarding shows again next launch.
                if let Err(e) = saved {
                    tracing::warn!("could not save settings: {e}");
                }
                this.load_last(cx);
            },
        );
    }

    /// Shows a snapshot. The first one opens at the root with the default selection; a later one
    /// keeps the folder (or its nearest surviving ancestor), the selection and the drawer.
    fn enter(&mut self, meta: SnapshotMeta, cx: &mut Context<Self>) {
        self.view_seq += 1;
        let seq = self.view_seq;
        let mode = self.mode;
        let first = self.meta.is_none();
        let candidates: Vec<String> = if first {
            vec![meta.root.clone()]
        } else {
            ancestors(&self.path, &meta.root)
                .into_iter()
                .map(String::from)
                .collect()
        };
        let root = meta.root.clone();
        off_thread(
            &self.source,
            cx,
            move |s| {
                let mut view = Err(Error::NotInSnapshot(root));
                for candidate in &candidates {
                    view = s.view(candidate, mode);
                    if view.is_ok() {
                        break;
                    }
                }
                Ok::<_, Error>((view?, s.recommendations()?, s.preselected_rows()))
            },
            move |this, loaded, cx| {
                if this.view_seq != seq {
                    // The user navigated while this loaded; load again for where they are now.
                    this.enter(meta, cx);
                    return;
                }
                match loaded {
                    Ok((view, recommendations, preselected)) => {
                        let selected = if first {
                            let mut selected = HashSet::new();
                            for row in &recommendations {
                                if preselected.contains(&row.row_id) {
                                    selected.extend(
                                        selection::ready_ids(row).into_iter().map(String::from),
                                    );
                                }
                            }
                            selected
                        } else {
                            let (kept, dropped) = selection::carry_over(
                                &this.recommendations,
                                &this.selected,
                                &recommendations,
                            );
                            if dropped > 0 {
                                let noun = if dropped == 1 { "item" } else { "items" };
                                this.show_toast(
                                    format!("{dropped} selected {noun} no longer found"),
                                    cx,
                                );
                            }
                            kept
                        };
                        let drawer = this
                            .focused_row
                            .clone()
                            .filter(|id| recommendations.iter().any(|r| &r.row_id == id));
                        this.root_path = meta.root.clone();
                        this.path = view.path.clone();
                        this.meta = Some(meta);
                        this.view = Some(view);
                        this.hovered = None;
                        this.recommendations = recommendations;
                        this.selected = selected;
                        // The details belong to the old snapshot; reload them for a surviving row.
                        this.focused_row = None;
                        this.details = None;
                        this.details_error = None;
                        if drawer.is_some() {
                            this.focus_row(drawer, cx);
                        }
                        this.error = None;
                        this.phase = Phase::Ready;
                    }
                    Err(e) => {
                        if first {
                            this.phase = Phase::Error;
                        }
                        this.error = Some(e.to_string());
                    }
                }
                this.scan = None;
                cx.notify();
            },
        );
    }

    pub fn start_scan(&mut self, cx: &mut Context<Self>) {
        if self.scan.is_some() {
            return;
        }
        self.scan = Some(ScanProgress::default());
        self.error = None;
        // With a snapshot on screen the scan runs behind it; the explorer keeps working.
        if self.meta.is_none() {
            self.phase = Phase::Scanning;
        }
        cx.notify();
        let (tx, rx) = mpsc::channel();
        let started = self.source.start_scan(move |event| {
            // The receiver is gone once a newer scan replaced this one.
            tx.send(event).ok();
        });
        if let Err(e) = started {
            self.scan = None;
            if self.meta.is_none() {
                self.phase = Phase::Error;
            }
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
                if let Some(scan) = &mut self.scan {
                    *scan = ScanProgress {
                        files,
                        bytes,
                        current,
                    };
                }
                false
            }
            ScanEvent::Finished { meta } => {
                self.enter(meta, cx);
                true
            }
            ScanEvent::Failed { message } => {
                self.scan = None;
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

    pub fn cancel_scan(&mut self, _cx: &mut Context<Self>) {
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
        self.details_error = None;
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
                    Err(e) => {
                        tracing::warn!("could not load details of {row_id}: {e}");
                        this.details_error = Some(e.to_string());
                    }
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

impl AppModel {
    fn snapshot_id(&self) -> String {
        self.meta.as_ref().map(|m| m.id.clone()).unwrap_or_default()
    }

    fn selected_ids(&self) -> Vec<String> {
        let mut ids: Vec<String> = self.selected.iter().cloned().collect();
        ids.sort();
        ids
    }

    /// "Clean selected": builds the plan and opens the confirm dialog.
    pub fn open_cleanup(&mut self, cx: &mut Context<Self>) {
        if self.selected.is_empty() || self.cleanup.is_some() {
            return;
        }
        self.request_plan(false, cx);
    }

    /// Rebuilds the plan with or without permanent deletion of review-tier trash items.
    pub fn set_permanent(&mut self, permanent: bool, cx: &mut Context<Self>) {
        if matches!(self.cleanup, Some(Cleanup::Confirm { .. })) {
            self.request_plan(permanent, cx);
        }
    }

    fn request_plan(&mut self, permanent: bool, cx: &mut Context<Self>) {
        self.cleanup = Some(Cleanup::Planning);
        cx.notify();
        let (snapshot, ids) = (self.snapshot_id(), self.selected_ids());
        off_thread(
            &self.source,
            cx,
            move |s| s.build_plan(&snapshot, ids, permanent),
            move |this, built, cx| {
                // Cancelled while the plan was being built.
                if this.cleanup != Some(Cleanup::Planning) {
                    return;
                }
                match built {
                    Ok(plan) => this.cleanup = Some(Cleanup::Confirm { plan, permanent }),
                    Err(e) => {
                        this.cleanup = None;
                        this.show_toast(format!("Could not plan the cleanup: {e}"), cx);
                    }
                }
            },
        );
    }

    /// The confirm button: runs the plan with per-item progress.
    pub fn confirm_cleanup(&mut self, cx: &mut Context<Self>) {
        let Some(Cleanup::Confirm { plan, .. }) = self.cleanup.clone() else {
            return;
        };
        if plan.actions.is_empty() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        let started = self.source.execute_cleanup(&plan.plan_id, move |event| {
            tx.send(event).ok();
        });
        if let Err(e) = started {
            self.cleanup = None;
            self.show_toast(format!("Could not start the cleanup: {e}"), cx);
            return;
        }
        self.cleanup = Some(Cleanup::Running {
            plan,
            done: 0,
            current: None,
        });
        cx.notify();
        self.cleanup_task = Some(cx.spawn(async move |this, cx| {
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
                        done |= model.on_cleanup_event(event);
                    }
                    if gone && !done {
                        model.cleanup = Some(Cleanup::Results {
                            summary: CleanupSummary::default(),
                        });
                        model.show_toast("The cleanup stopped unexpectedly".into(), cx);
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

    /// Applies one cleanup event; true when it ended the cleanup.
    fn on_cleanup_event(&mut self, event: CleanupEvent) -> bool {
        let Some(Cleanup::Running { done, current, .. }) = &mut self.cleanup else {
            return matches!(event, CleanupEvent::Finished { .. });
        };
        match event {
            CleanupEvent::ItemStarted { item_id } => {
                *current = Some(item_id);
                false
            }
            CleanupEvent::ItemFinished { .. } => {
                *done += 1;
                *current = None;
                false
            }
            CleanupEvent::Finished { summary } => {
                self.cleanup = Some(Cleanup::Results { summary });
                true
            }
        }
    }

    /// Cancel in the confirm dialog, or Done in the results. After a cleanup
    /// the selection is cleared and the scan root is rescanned, so sizes and
    /// recommendations reflect the disk (PRD section 7.6, step 8).
    pub fn close_cleanup(&mut self, cx: &mut Context<Self>) {
        match self.cleanup {
            Some(Cleanup::Running { .. }) | None => {}
            Some(Cleanup::Results { .. }) => {
                self.cleanup = None;
                self.selected.clear();
                self.start_scan(cx);
            }
            Some(Cleanup::Planning | Cleanup::Confirm { .. }) => {
                self.cleanup = None;
                cx.notify();
            }
        }
    }

    /// "Copy script": the selection's plan as a bash script on the clipboard.
    pub fn copy_script(&mut self, cx: &mut Context<Self>) {
        if self.selected.is_empty() {
            return;
        }
        let (snapshot, ids) = (self.snapshot_id(), self.selected_ids());
        off_thread(
            &self.source,
            cx,
            move |s| {
                let plan = s.build_plan(&snapshot, ids, false)?;
                s.export_script(&plan.plan_id)
            },
            |this, script, cx| match script {
                Ok(script) => {
                    cx.write_to_clipboard(ClipboardItem::new_string(script));
                    this.show_toast("Cleanup script copied to the clipboard".into(), cx);
                }
                Err(e) => this.show_toast(format!("Could not export the script: {e}"), cx),
            },
        );
    }

    fn show_toast(&mut self, text: String, cx: &mut Context<Self>) {
        self.toast_seq += 1;
        let seq = self.toast_seq;
        self.toast = Some(text);
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(TOAST).await;
            this.update(cx, |model, cx| {
                if model.toast_seq == seq {
                    model.toast = None;
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::Ordering;

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

    #[test]
    fn ancestors_walk_up_to_the_root() {
        assert_eq!(ancestors("~/a/b", "~"), ["~/a/b", "~/a", "~"]);
    }

    #[test]
    fn ancestors_of_the_root_is_the_root() {
        assert_eq!(ancestors("/Users/a", "/Users/a"), ["/Users/a"]);
    }

    #[test]
    fn ancestors_of_a_path_outside_the_root_is_the_root() {
        assert_eq!(ancestors("/Users/ab/x", "/Users/a"), ["/Users/a"]);
    }

    #[test]
    fn ancestors_under_the_filesystem_root_end_at_it() {
        assert_eq!(ancestors("/a/b", "/"), ["/a/b", "/a", "/"]);
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

    /// A model over the mock, started without Full Disk Access.
    fn setup_without_access(cx: &mut TestAppContext) -> (Entity<AppModel>, &mut VisualTestContext) {
        let cx = cx.add_empty_window();
        let mock = MockData::new(false).with_step(Duration::ZERO);
        mock.full_disk_access.store(false, Ordering::Relaxed);
        let source = Source::Mock(mock);
        let model = cx.update(|window, cx| cx.new(|cx| AppModel::new(source, window, cx)));
        cx.run_until_parked();
        (model, cx)
    }

    fn grant_access(model: &Entity<AppModel>, cx: &mut VisualTestContext) {
        model.read_with(cx, |m, _| match m.source.as_ref() {
            Source::Mock(mock) => mock.full_disk_access.store(true, Ordering::Relaxed),
            Source::Live(_) => unreachable!("tests use the mock"),
        });
    }

    #[gpui::test]
    fn no_install_while_scanning(cx: &mut TestAppContext) {
        let (model, cx) = setup(cx, false);
        model.update(cx, |m, _| m.scan = Some(ScanProgress::default()));
        assert!(!model.read_with(cx, |m, _| m.can_install_update()));
    }

    #[gpui::test]
    fn no_install_while_a_cleanup_dialog_is_open(cx: &mut TestAppContext) {
        let (model, cx) = setup(cx, false);
        model.update(cx, |m, _| m.cleanup = Some(Cleanup::Planning));
        assert!(!model.read_with(cx, |m, _| m.can_install_update()));
    }

    #[gpui::test]
    fn install_is_allowed_when_idle(cx: &mut TestAppContext) {
        let (model, cx) = setup(cx, false);
        assert!(model.read_with(cx, |m, _| m.can_install_update()));
    }

    #[gpui::test]
    fn dismissing_a_failed_update_goes_idle(cx: &mut TestAppContext) {
        let (model, cx) = setup(cx, false);
        model.update(cx, |m, cx| {
            m.update_state = UpdateState::Failed("offline".into());
            m.dismiss_update(cx);
        });
        assert_eq!(
            model.read_with(cx, |m, _| m.update_state.clone()),
            UpdateState::Idle
        );
    }

    #[gpui::test]
    fn missing_access_starts_with_onboarding(cx: &mut TestAppContext) {
        let (model, cx) = setup_without_access(cx);
        assert_eq!(model.read_with(cx, |m, _| m.phase), Phase::Onboarding);
    }

    #[gpui::test]
    fn continue_without_access_reaches_ready(cx: &mut TestAppContext) {
        let (model, cx) = setup_without_access(cx);
        model.update(cx, |m, cx| m.continue_without_access(cx));
        cx.run_until_parked();
        assert_eq!(model.read_with(cx, |m, _| m.phase), Phase::Ready);
    }

    #[gpui::test]
    fn continue_without_access_is_remembered(cx: &mut TestAppContext) {
        let (model, cx) = setup_without_access(cx);
        model.update(cx, |m, cx| m.continue_without_access(cx));
        cx.run_until_parked();
        assert!(model.read_with(cx, |m, _| m.source.onboarding_done()));
    }

    #[gpui::test]
    fn granting_access_leaves_onboarding_on_recheck(cx: &mut TestAppContext) {
        let (model, cx) = setup_without_access(cx);
        grant_access(&model, cx);
        model.update(cx, |m, cx| m.recheck_access(cx));
        cx.run_until_parked();
        assert_eq!(model.read_with(cx, |m, _| m.phase), Phase::Ready);
    }

    #[gpui::test]
    fn recheck_without_a_grant_stays_on_onboarding(cx: &mut TestAppContext) {
        let (model, cx) = setup_without_access(cx);
        model.update(cx, |m, cx| m.recheck_access(cx));
        cx.run_until_parked();
        assert_eq!(model.read_with(cx, |m, _| m.phase), Phase::Onboarding);
    }

    #[gpui::test]
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
    fn failed_details_load_reports_the_error(cx: &mut TestAppContext) {
        let (model, cx) = setup(cx, false);
        model.update(cx, |m, cx| m.focus_row(Some("nope".into()), cx));
        cx.run_until_parked();
        model.read_with(cx, |m, _| {
            assert_eq!(
                m.details_error.as_deref(),
                Some("no recommendation nope in the current scan")
            );
        });
    }

    #[gpui::test]
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
    fn toggle_row_selects_then_clears(cx: &mut TestAppContext) {
        let (model, cx) = setup(cx, false);
        model.update(cx, |m, cx| m.toggle_row("npm-cache", cx));
        model.read_with(cx, |m, _| assert!(!m.selected.is_empty()));
        model.update(cx, |m, cx| m.toggle_row("npm-cache", cx));
        model.read_with(cx, |m, _| assert!(m.selected.is_empty()));
    }

    #[gpui::test]
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
            assert_eq!(m.phase, Phase::Ready);
            assert!(m.scan.is_some());
        });
        settle(cx, &model, |m| m.scan.is_none());
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
        settle(cx, &model, |m| m.scan.is_none());
        model.read_with(cx, |m, _| {
            // A snapshot was already loaded, so the explorer comes back.
            assert_eq!(m.phase, Phase::Ready);
            assert_eq!(m.error.as_deref(), Some("Scan cancelled"));
        });
    }

    /// Rescans and waits until the new snapshot is shown.
    fn rescan(model: &Entity<AppModel>, cx: &mut VisualTestContext) {
        model.update(cx, |m, cx| m.start_scan(cx));
        settle(cx, model, |m| m.scan.is_none());
    }

    /// Adds a ready item that the mock's next snapshot does not have, and selects it.
    fn select_a_ghost(model: &Entity<AppModel>, cx: &mut VisualTestContext) {
        model.update(cx, |m, _| {
            let mut ghost = m.recommendations[0].items[0].clone();
            ghost.item_id = "ghost".into();
            ghost.path = "~/ghost".into();
            m.recommendations[0].items.push(ghost);
            m.selected.insert("ghost".into());
        });
    }

    #[gpui::test]
    fn selection_survives_a_swap_by_path(cx: &mut TestAppContext) {
        let (model, cx) = setup(cx, false);
        let real_id = model.update(cx, |m, _| {
            let item = &mut m.recommendations[0].items[0];
            let real = std::mem::replace(&mut item.item_id, "old-id".into());
            m.selected.insert("old-id".into());
            real
        });
        rescan(&model, cx);
        assert!(model.read_with(cx, |m, _| m.selected.contains(&real_id)));
    }

    #[gpui::test]
    fn a_vanished_item_is_dropped(cx: &mut TestAppContext) {
        let (model, cx) = setup(cx, false);
        select_a_ghost(&model, cx);
        rescan(&model, cx);
        assert!(!model.read_with(cx, |m, _| m.selected.contains("ghost")));
    }

    #[gpui::test]
    fn a_vanished_item_is_counted_in_a_toast(cx: &mut TestAppContext) {
        let (model, cx) = setup(cx, false);
        select_a_ghost(&model, cx);
        rescan(&model, cx);
        assert_eq!(
            model.read_with(cx, |m, _| m.toast.clone()).as_deref(),
            Some("1 selected item no longer found")
        );
    }

    #[gpui::test]
    fn explorer_path_falls_back_to_an_ancestor(cx: &mut TestAppContext) {
        let (model, cx) = setup(cx, false);
        model.update(cx, |m, _| m.path = "~/Desktop/vanished".into());
        rescan(&model, cx);
        assert_eq!(model.read_with(cx, |m, _| m.path.clone()), "~/Desktop");
    }

    #[gpui::test]
    fn drawer_closes_when_its_row_is_gone(cx: &mut TestAppContext) {
        let (model, cx) = setup(cx, false);
        model.update(cx, |m, _| m.focused_row = Some("gone-row".into()));
        rescan(&model, cx);
        assert!(model.read_with(cx, |m, _| m.focused_row.is_none()));
    }

    #[gpui::test]
    fn open_cleanup_without_a_selection_does_nothing(cx: &mut TestAppContext) {
        let (model, cx) = setup(cx, false);
        model.update(cx, |m, cx| m.open_cleanup(cx));
        cx.run_until_parked();
        model.read_with(cx, |m, _| assert!(m.cleanup.is_none()));
    }

    #[gpui::test]
    fn open_cleanup_plans_the_selection(cx: &mut TestAppContext) {
        let (model, cx) = setup(cx, true);
        model.update(cx, |m, cx| m.open_cleanup(cx));
        cx.run_until_parked();
        model.read_with(cx, |m, _| {
            let Some(Cleanup::Confirm { plan, .. }) = &m.cleanup else {
                panic!("expected the confirm dialog, got {:?}", m.cleanup);
            };
            let ids: HashSet<String> = plan.actions.iter().map(|a| a.item_id.clone()).collect();
            assert_eq!(ids, m.selected);
        });
    }

    #[gpui::test]
    fn confirmed_cleanup_ends_in_results(cx: &mut TestAppContext) {
        let (model, cx) = setup(cx, true);
        model.update(cx, |m, cx| m.open_cleanup(cx));
        cx.run_until_parked();
        model.update(cx, |m, cx| m.confirm_cleanup(cx));
        settle(cx, &model, |m| {
            matches!(m.cleanup, Some(Cleanup::Results { .. }))
        });
        model.read_with(cx, |m, _| {
            let Some(Cleanup::Results { summary }) = &m.cleanup else {
                unreachable!()
            };
            assert!(summary.dry_run);
        });
    }

    #[gpui::test]
    fn closing_the_results_clears_the_selection_and_rescans(cx: &mut TestAppContext) {
        let (model, cx) = setup(cx, true);
        model.update(cx, |m, cx| m.open_cleanup(cx));
        cx.run_until_parked();
        model.update(cx, |m, cx| m.confirm_cleanup(cx));
        settle(cx, &model, |m| {
            matches!(m.cleanup, Some(Cleanup::Results { .. }))
        });
        model.update(cx, |m, cx| m.close_cleanup(cx));
        model.read_with(cx, |m, _| {
            assert_eq!(
                (m.phase, m.scan.is_some(), m.selected.is_empty()),
                (Phase::Ready, true, true)
            );
        });
    }

    #[gpui::test]
    fn a_rescan_never_leaves_ready(cx: &mut TestAppContext) {
        let (model, cx) = setup(cx, false);
        model.update(cx, |m, cx| m.start_scan(cx));
        settle(cx, &model, |m| {
            assert_eq!(m.phase, Phase::Ready);
            m.scan.is_none()
        });
    }

    #[gpui::test]
    fn rescan_shows_progress_in_scan(cx: &mut TestAppContext) {
        let (model, cx) = setup(cx, false);
        model.update(cx, |m, cx| m.start_scan(cx));
        assert!(model.read_with(cx, |m, _| m.scan.is_some()));
    }

    #[gpui::test]
    fn cancel_closes_the_confirm_dialog(cx: &mut TestAppContext) {
        let (model, cx) = setup(cx, true);
        model.update(cx, |m, cx| m.open_cleanup(cx));
        cx.run_until_parked();
        model.update(cx, |m, cx| m.close_cleanup(cx));
        model.read_with(cx, |m, _| assert!(m.cleanup.is_none()));
    }

    #[gpui::test]
    fn permanent_toggle_replans_review_trash_as_delete(cx: &mut TestAppContext) {
        let (model, cx) = setup(cx, false);
        model.update(cx, |m, cx| {
            let row = m
                .recommendations
                .iter()
                .find(|r| {
                    r.tier == Tier::Review
                        && r.items
                            .iter()
                            .any(|i| i.method == crate::recommend::model::MethodView::Trash)
                })
                .cloned()
                .expect("the mock has a review-tier trash row");
            m.toggle_row(&row.row_id, cx);
            m.open_cleanup(cx);
        });
        cx.run_until_parked();
        model.update(cx, |m, cx| m.set_permanent(true, cx));
        cx.run_until_parked();
        model.read_with(cx, |m, _| {
            let Some(Cleanup::Confirm { plan, permanent }) = &m.cleanup else {
                panic!("expected the confirm dialog, got {:?}", m.cleanup);
            };
            assert_eq!((*permanent, plan.trash_total), (true, 0));
        });
    }

    #[gpui::test]
    fn copy_script_puts_the_script_on_the_clipboard(cx: &mut TestAppContext) {
        let (model, cx) = setup(cx, true);
        model.update(cx, |m, cx| m.copy_script(cx));
        cx.run_until_parked();
        let text = cx
            .read_from_clipboard()
            .and_then(|c| c.text())
            .unwrap_or_default();
        assert!(
            text.starts_with("#!/bin/bash\nset -euo pipefail\n"),
            "{text}"
        );
    }
}
