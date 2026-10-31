//! App state and actions: scan phase, explorer path and mode, recommendations,
//! selection and the row in the inspector. Views read the public fields and call the methods.

use std::collections::HashSet;
use std::sync::Arc;
use std::sync::mpsc::{self, TryRecvError};
use std::time::Duration;

use gpui::{AppContext, ClipboardItem, Context, FocusHandle, Subscription, Task, Window};

use super::explorer::{self, ExplorerState};
use super::motion;
use super::recommendations::{self, PanelState};
use super::selection;
use super::source::Source;
use super::stats;
use super::theme;
use super::treemap::{self, Dir};
use crate::cleanup::model::{CleanupEvent, CleanupPlan, CleanupSummary, Outcome};
use crate::error::Error;
use crate::platform;
use crate::recommend::model::{ItemDetails, Recommendation};
use crate::scanner::tree::{ScanEvent, SnapshotMeta};
use crate::update;
use crate::view::{CategoryTotal, ViewKind, ViewNode};

/// How often the UI drains scan events from the scanner thread.
const SCAN_POLL: Duration = Duration::from_millis(100);
/// A snapshot older than this is rescanned on launch (FR-BG-3). A constant, not a setting.
const STALE_AFTER: i64 = 24 * 3_600;
/// How long a toast stays up.
const TOAST: Duration = Duration::from_secs(3);

/// Sidebar pages (PRD FR-WIN-1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Overview,
    Explore,
    Recommended,
}

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
    /// Path of the scan root, as the snapshot reports it.
    pub root_path: String,
    /// Folder the explorer shows.
    pub path: String,
    /// Folders left by Back, nearest last; Forward reopens them.
    pub forward: Vec<String>,
    pub page: Page,
    pub sidebar_visible: bool,
    pub inspector_visible: bool,
    /// Highlight Cleanable (PRD FR-EXP-7): tiles with nothing to clean are dimmed.
    pub highlight_cleanable: bool,
    /// Accessibility display settings, reread whenever the window becomes active.
    pub reduce_motion: bool,
    pub reduce_transparency: bool,
    /// Per-category bytes of the current snapshot, for the sidebar.
    pub category_totals: Vec<CategoryTotal>,
    /// The five largest folders directly under the scan root, for the Overview.
    pub largest_folders: Vec<ViewNode>,
    pub view: Option<ViewNode>,
    /// The selected tile in `view`, by `explorer::tile_key`.
    pub tile: Option<String>,
    /// The selected folder's own view, for the inspector's largest folders.
    pub tile_view: Option<ViewNode>,
    /// `ViewNode::id` under the pointer.
    pub hovered: Option<String>,
    pub recommendations: Vec<Recommendation>,
    /// Selected item IDs. Empty by default (S-09); only ready items.
    pub selected: HashSet<String>,
    /// Row shown in the inspector on Recommended.
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
    /// The treemap, where the Explore keymap applies.
    pub treemap_focus: FocusHandle,
    /// The Recommended list, where j/k/Space/Return apply.
    pub list_focus: FocusHandle,
    pub explorer: ExplorerState,
    pub panel: PanelState,
    pub update_state: UpdateState,
    /// The release behind `UpdateState::Available`, kept for the install.
    pending_update: Option<update::Update>,
    /// Bumped per view request; an older response is dropped when it arrives late.
    view_seq: u64,
    /// Bumped per tile selection; an older folder view is dropped when it arrives late.
    tile_seq: u64,
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

fn is_stale(now: i64, scanned_at: i64) -> bool {
    now.saturating_sub(scanned_at) > STALE_AFTER
}

/// Takes `size` off every node on the way to `path` and drops the node at it, so the treemap
/// matches the disk before the rescan lands.
fn subtract(node: &mut ViewNode, path: &str, size: u64) {
    // The smaller-items tile carries its folder's path; only its children have their own.
    let inside = match node.kind {
        ViewKind::Smaller => node.children.iter().any(|c| is_within(path, &c.path)),
        _ => is_within(path, &node.path),
    };
    if !inside {
        return;
    }
    node.size = node.size.saturating_sub(size);
    node.cleanable = node.cleanable.saturating_sub(size);
    node.children.retain(|c| c.path != path);
    for child in &mut node.children {
        subtract(child, path, size);
    }
}

/// Where Space looks for items: the tile itself, or for the smaller-items tile,
/// which carries its folder's path, the items it merged.
fn stage_paths(node: &ViewNode) -> Vec<&str> {
    match node.kind {
        ViewKind::Smaller => node.children.iter().map(|c| c.path.as_str()).collect(),
        _ => vec![node.path.as_str()],
    }
}

/// The folder above `path`, like `parentOf` in the old view store.
fn parent_of(path: &str) -> &str {
    path.rfind('/')
        .map(|i| &path[..i])
        .filter(|p| !p.is_empty())
        .unwrap_or("/")
}

/// Whether `path` is `dir` or below it.
fn is_within(path: &str, dir: &str) -> bool {
    path == dir
        || path
            .strip_prefix(dir.trim_end_matches('/'))
            .is_some_and(|rest| rest.starts_with('/'))
}

/// `path` and its parents down to `root`, nearest first. A path outside `root` gives just `root`.
fn ancestors<'a>(path: &'a str, root: &'a str) -> Vec<&'a str> {
    if !is_within(path, root) {
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
            // The user may have just granted access or changed a display setting.
            this.reduce_motion = platform::reduce_motion();
            this.reduce_transparency = platform::reduce_transparency();
            this.recheck_access(cx);
            if this.phase == Phase::Ready {
                this.refresh(cx);
            }
        });
        theme::sync_components(window, cx);
        let appearance = cx.observe_window_appearance(window, |_, window, cx| {
            theme::sync_components(window, cx);
            cx.notify();
        });
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
            root_path: String::new(),
            path: String::new(),
            forward: Vec::new(),
            page: Page::Overview,
            sidebar_visible: true,
            inspector_visible: true,
            highlight_cleanable: false,
            reduce_motion: platform::reduce_motion(),
            reduce_transparency: platform::reduce_transparency(),
            category_totals: Vec::new(),
            largest_folders: Vec::new(),
            view: None,
            tile: None,
            tile_view: None,
            hovered: None,
            recommendations: Vec::new(),
            selected: HashSet::new(),
            focused_row: None,
            details: None,
            details_error: None,
            cleanup: None,
            toast: None,
            focus_handle,
            treemap_focus: cx.focus_handle(),
            list_focus: cx.focus_handle(),
            explorer: ExplorerState::default(),
            panel: PanelState::default(),
            update_state: UpdateState::Idle,
            pending_update: None,
            view_seq: 0,
            tile_seq: 0,
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
                let mut view = Err(Error::NotInSnapshot(root.clone()));
                for candidate in &candidates {
                    view = s.view(candidate);
                    if view.is_ok() {
                        break;
                    }
                }
                // The explorer may sit deeper than the root; the Overview always lists the root.
                let largest = stats::largest_folders(&s.view(&root)?, 5);
                Ok::<_, Error>((
                    view?,
                    s.recommendations()?,
                    s.category_totals()?,
                    largest,
                    s.preselected_rows(),
                ))
            },
            move |this, loaded, cx| {
                // The user navigated while this loaded. That newer view request already reads
                // this snapshot, so it sets the path and view; the rest still swaps in here.
                let navigated = this.view_seq != seq;
                let mut stale = false;
                match loaded {
                    Ok((view, recommendations, totals, largest, preselected)) => {
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
                        let inspected = this
                            .focused_row
                            .clone()
                            .filter(|id| recommendations.iter().any(|r| &r.row_id == id));
                        stale = first
                            && is_stale(
                                crate::cleanup::audit::now_ms() as i64 / 1000,
                                meta.scanned_at,
                            );
                        this.root_path = meta.root.clone();
                        if !navigated {
                            // Node ids change with the snapshot; the tile key is its path.
                            let tile = this.tile.take().filter(|k| {
                                view.children.iter().any(|c| explorer::tile_key(c) == *k)
                            });
                            this.path = view.path.clone();
                            this.view = Some(view);
                            this.hovered = None;
                            this.select_tile(tile, cx);
                        }
                        this.meta = Some(meta);
                        this.recommendations = recommendations;
                        this.category_totals = totals;
                        this.largest_folders = largest;
                        // Folders left behind may be gone from the new snapshot.
                        this.forward.clear();
                        this.selected = selected;
                        // The details belong to the old snapshot; reload them for a surviving row.
                        this.focused_row = None;
                        this.details = None;
                        this.details_error = None;
                        if inspected.is_some() {
                            this.focus_row(inspected, cx);
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
                // Only the launch load can start this, so a scan never triggers itself.
                if stale {
                    this.start_scan(cx);
                }
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

    /// Loads the view of `path` and selects the tile `select` in it; the model changes
    /// once it arrives.
    fn load_view(&mut self, path: String, select: Option<String>, cx: &mut Context<Self>) {
        self.view_seq += 1;
        let seq = self.view_seq;
        let wanted = path.clone();
        off_thread(
            &self.source,
            cx,
            move |s| s.view(&wanted),
            move |this, loaded, cx| {
                if this.view_seq != seq {
                    return;
                }
                match loaded {
                    Ok(view) => {
                        this.path = path;
                        this.view = Some(view);
                        this.hovered = None;
                        this.select_tile(select, cx);
                        this.start_zoom(cx);
                    }
                    Err(e) => {
                        this.explorer.clear_zoom();
                        tracing::warn!("could not load {path}: {e}");
                    }
                }
            },
        );
    }

    /// Starts the zoom the last navigation prepared and ends it once it has played.
    fn start_zoom(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.explorer.start_zoom() else {
            return;
        };
        let duration = motion::Transition::new(motion::NAV, self.reduce_motion).duration();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(duration).await;
            this.update(cx, |model, cx| {
                if model.explorer.end_zoom(id) {
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    /// Shows the folder at `path` in the explorer.
    pub fn navigate(&mut self, path: String, cx: &mut Context<Self>) {
        self.forward.clear();
        self.explorer.clear_zoom();
        self.load_view(path, None, cx);
    }

    /// Goes up one folder, remembered for Forward, and selects the folder it left;
    /// does nothing at the scan root.
    pub fn up(&mut self, cx: &mut Context<Self>) {
        if self.path != self.root_path {
            let left = self.path.clone();
            self.forward.push(left.clone());
            if let Some(view) = &self.view {
                self.explorer.prepare_zoom_out(view);
            }
            self.load_view(parent_of(&left).to_owned(), Some(left), cx);
        }
    }

    /// Reopens the folder the last Back left.
    pub fn go_forward(&mut self, cx: &mut Context<Self>) {
        if let Some(path) = self.forward.pop() {
            self.explorer.clear_zoom();
            self.load_view(path, None, cx);
        }
    }

    /// Opens `path` on the Explore page (Overview, largest folders).
    pub fn explore(&mut self, path: String, cx: &mut Context<Self>) {
        self.page = Page::Explore;
        self.navigate(path, cx);
    }

    pub fn set_page(&mut self, page: Page, cx: &mut Context<Self>) {
        self.page = page;
        cx.notify();
    }

    pub fn toggle_sidebar(&mut self, cx: &mut Context<Self>) {
        self.sidebar_visible = !self.sidebar_visible;
        cx.notify();
    }

    pub fn toggle_inspector(&mut self, cx: &mut Context<Self>) {
        self.inspector_visible = !self.inspector_visible;
        cx.notify();
    }

    /// The selected tile's node in the current view.
    pub fn tile_node(&self) -> Option<&ViewNode> {
        self.find_tile(self.tile.as_deref()?)
    }

    fn find_tile(&self, key: &str) -> Option<&ViewNode> {
        self.view
            .as_ref()?
            .children
            .iter()
            .find(|c| explorer::tile_key(c) == key)
    }

    /// Selects a tile and, for a folder, loads its own view for the inspector.
    pub fn select_tile(&mut self, key: Option<String>, cx: &mut Context<Self>) {
        self.tile = key;
        self.tile_view = None;
        self.tile_seq += 1;
        cx.notify();
        let Some(path) = self
            .tile_node()
            .filter(|n| n.kind == ViewKind::Dir)
            .map(|n| n.path.clone())
        else {
            return;
        };
        let seq = self.tile_seq;
        off_thread(
            &self.source,
            cx,
            move |s| s.view(&path),
            move |this, loaded, _| {
                if this.tile_seq != seq {
                    return;
                }
                match loaded {
                    Ok(view) => this.tile_view = Some(view),
                    Err(e) => tracing::warn!("could not load the selected folder: {e}"),
                }
            },
        );
    }

    /// Arrow keys and h/j/k/l: selects the nearest tile in `dir`, or the first tile when
    /// none is selected (PRD FR-KB-5).
    pub fn move_selection(&mut self, dir: Dir, cx: &mut Context<Self>) {
        let Some(view) = &self.view else {
            return;
        };
        let current = self.tile.as_deref().and_then(|k| {
            view.children
                .iter()
                .position(|c| explorer::tile_key(c) == k)
        });
        let next = match current {
            None => Some(0),
            Some(i) => {
                let Some(rects) = self.explorer.rects(view) else {
                    return;
                };
                treemap::neighbor(&rects, i, dir)
            }
        };
        if let Some(key) = next
            .and_then(|i| view.children.get(i))
            .map(explorer::tile_key)
        {
            self.select_tile(Some(key), cx);
        }
    }

    /// Double-click or Return: opens a folder, reveals a file (PRD FR-EXP-8, FR-EXP-9).
    pub fn open_tile(&mut self, key: &str, cx: &mut Context<Self>) {
        let Some(node) = self.find_tile(key) else {
            return;
        };
        let path = node.path.clone();
        match node.kind {
            ViewKind::Dir => {
                self.navigate(path, cx);
                if let Some(view) = &self.view {
                    self.explorer.prepare_zoom_in(view, key);
                }
            }
            ViewKind::File => self.reveal(path, cx),
            ViewKind::Smaller => {}
        }
    }

    /// Whether Space or Add to Queue has anything to stage for `node`.
    pub fn can_stage(&self, node: &ViewNode) -> bool {
        selection::stage_under(&self.recommendations, &self.selected, &stage_paths(node)).is_some()
    }

    /// Space on a tile: stages its ready items, or unstages them (PRD FR-Q-2).
    pub fn stage_tile(&mut self, key: &str, cx: &mut Context<Self>) {
        // The queue is the open plan's input while the sheet is up.
        if self.cleanup.is_some() {
            return;
        }
        let Some(node) = self.find_tile(key) else {
            return;
        };
        let next =
            selection::stage_under(&self.recommendations, &self.selected, &stage_paths(node));
        match next {
            Some(next) => {
                self.selected = next;
                cx.notify();
            }
            None => self.show_toast("Nothing here can be cleaned".into(), cx),
        }
    }

    /// Toggles Quick Look for the selected tile (⌘Y).
    pub fn quick_look(&mut self, _cx: &mut Context<Self>) {
        let Some(path) = self
            .tile_node()
            .filter(|n| n.kind != ViewKind::Smaller)
            .map(|n| n.path.clone())
        else {
            return;
        };
        let shown = self
            .source
            .quick_look(&path)
            .map_err(|e| e.to_string())
            .and_then(|p| platform::quick_look(&p));
        if let Err(e) = shown {
            tracing::warn!("could not Quick Look {path}: {e}");
        }
    }

    /// Dims tiles with nothing to clean (PRD FR-EXP-7).
    pub fn toggle_highlight(&mut self, cx: &mut Context<Self>) {
        self.highlight_cleanable = !self.highlight_cleanable;
        cx.notify();
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

    /// Selects every ready item of `row_id`; already selected items stay.
    pub fn add_row(&mut self, row_id: &str, cx: &mut Context<Self>) {
        if let Some(row) = self.recommendations.iter().find(|r| r.row_id == row_id) {
            self.selected
                .extend(selection::ready_ids(row).into_iter().map(String::from));
            cx.notify();
        }
    }

    pub fn clear_selection(&mut self, cx: &mut Context<Self>) {
        self.selected.clear();
        cx.notify();
    }

    pub fn select_safe(&mut self, cx: &mut Context<Self>) {
        self.selected = selection::select_safe(&self.recommendations, &self.selected);
        cx.notify();
    }

    /// j/k and the arrows on Recommended: focuses the next or previous row in list order, or
    /// the first row when none is focused, and scrolls it into view (PRD FR-REC-5).
    pub fn move_row(&mut self, down: bool, cx: &mut Context<Self>) {
        let order = recommendations::ordered(&self.recommendations);
        let current = self
            .focused_row
            .as_deref()
            .and_then(|id| order.iter().position(|r| r.row_id == id));
        let next = match current {
            None => 0,
            Some(i) if down => (i + 1).min(order.len().saturating_sub(1)),
            Some(i) => i.saturating_sub(1),
        };
        let Some(id) = order.get(next).map(|r| r.row_id.clone()) else {
            return;
        };
        if let Some(ix) = recommendations::list_index(&self.recommendations, &id) {
            self.panel.scroll.scroll_to_item(ix);
        }
        if self.focused_row.as_deref() != Some(id.as_str()) {
            self.focus_row(Some(id), cx);
        }
    }

    /// Space on Recommended: stages or unstages the focused row's ready items.
    pub fn stage_row(&mut self, cx: &mut Context<Self>) {
        if self.cleanup.is_some() {
            return;
        }
        let Some(row) = self
            .focused_row
            .as_deref()
            .and_then(|id| self.recommendations.iter().find(|r| r.row_id == id))
        else {
            return;
        };
        if selection::ready_ids(row).is_empty() {
            self.show_toast("Nothing here can be cleaned".into(), cx);
        } else {
            let id = row.row_id.clone();
            self.toggle_row(&id, cx);
        }
    }

    /// Return on Recommended: shows the focused row in the inspector.
    pub fn open_row(&mut self, cx: &mut Context<Self>) {
        if self.focused_row.is_some() {
            self.inspector_visible = true;
            cx.notify();
        }
    }

    /// "Show Recommendation": opens Recommended with `row_id` in the inspector.
    pub fn show_row(&mut self, row_id: String, cx: &mut Context<Self>) {
        self.page = Page::Recommended;
        self.inspector_visible = true;
        self.focus_row(Some(row_id.clone()), cx);
        if let Some(ix) = recommendations::list_index(&self.recommendations, &row_id) {
            self.panel.scroll.scroll_to_item(ix);
        }
    }

    /// Shows `row_id` in the inspector and loads its details; `None` clears it.
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
        // The plan would be built against a snapshot that is about to be replaced.
        if self.selected.is_empty() || self.cleanup.is_some() || self.scan.is_some() {
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
        if plan.actions.is_empty() || self.scan.is_some() {
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
            CleanupEvent::ItemFinished {
                item_id, outcome, ..
            } => {
                *done += 1;
                *current = None;
                // Deleted or trashed items are gone for sure (FR-BG-6). A command frees an
                // estimate and its folder stays until the rescan, and the rest changed nothing.
                if matches!(outcome, Outcome::Deleted | Outcome::Trashed)
                    && let Some(item) = selection::forget_item(&mut self.recommendations, &item_id)
                {
                    if let Some(view) = &mut self.view {
                        subtract(view, &item.path, item.size);
                    }
                    self.selected.remove(&item_id);
                }
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
        cx.update(gpui_component::init);
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
        cx.update(gpui_component::init);
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

    /// The first folder tile under the root, opened.
    fn open_first_folder(model: &Entity<AppModel>, cx: &mut VisualTestContext) -> String {
        let child = model.read_with(cx, |m, _| {
            m.view
                .as_ref()
                .unwrap()
                .children
                .iter()
                .find(|c| c.kind == crate::view::ViewKind::Dir)
                .unwrap()
                .path
                .clone()
        });
        model.update(cx, |m, cx| m.navigate(child.clone(), cx));
        cx.run_until_parked();
        child
    }

    #[gpui::test]
    fn add_to_queue_selects_the_rows_ready_items(cx: &mut TestAppContext) {
        let (model, cx) = setup(cx, false);
        let (row_id, ready) = model.read_with(cx, |m, _| {
            let row = &m.recommendations[0];
            let ready: HashSet<String> = selection::ready_ids(row)
                .into_iter()
                .map(String::from)
                .collect();
            (row.row_id.clone(), ready)
        });
        model.update(cx, |m, cx| m.add_row(&row_id, cx));
        model.read_with(cx, |m, _| assert_eq!(m.selected, ready));
    }

    #[gpui::test]
    fn clear_queue_empties_the_selection(cx: &mut TestAppContext) {
        let (model, cx) = setup(cx, true);
        model.update(cx, |m, cx| m.clear_selection(cx));
        model.read_with(cx, |m, _| assert!(m.selected.is_empty()));
    }

    #[gpui::test]
    fn forward_reopens_the_folder_back_left(cx: &mut TestAppContext) {
        let (model, cx) = setup(cx, false);
        let child = open_first_folder(&model, cx);
        model.update(cx, |m, cx| m.up(cx));
        cx.run_until_parked();
        model.update(cx, |m, cx| m.go_forward(cx));
        cx.run_until_parked();
        model.read_with(cx, |m, _| assert_eq!(m.path, child));
    }

    #[gpui::test]
    fn opening_a_folder_clears_forward(cx: &mut TestAppContext) {
        let (model, cx) = setup(cx, false);
        open_first_folder(&model, cx);
        model.update(cx, |m, cx| m.up(cx));
        cx.run_until_parked();
        open_first_folder(&model, cx);
        model.read_with(cx, |m, _| assert!(m.forward.is_empty()));
    }

    #[gpui::test]
    fn the_overview_lists_the_five_largest_root_folders(cx: &mut TestAppContext) {
        let (model, cx) = setup(cx, false);
        model.read_with(cx, |m, _| assert_eq!(m.largest_folders.len(), 5));
    }

    #[gpui::test]
    fn largest_folders_stay_on_the_root_while_exploring(cx: &mut TestAppContext) {
        let (model, cx) = setup(cx, false);
        open_first_folder(&model, cx);
        model.update(cx, |m, cx| m.start_scan(cx));
        cx.run_until_parked();
        model.read_with(cx, |m, _| {
            assert_eq!(m.largest_folders[0].path, "~/Desktop");
        });
    }

    #[gpui::test]
    fn explore_opens_the_folder_on_the_explore_page(cx: &mut TestAppContext) {
        let (model, cx) = setup(cx, false);
        model.update(cx, |m, cx| m.explore("~/Desktop".into(), cx));
        cx.run_until_parked();
        model.read_with(cx, |m, _| {
            assert_eq!((m.page, m.path.as_str()), (Page::Explore, "~/Desktop"));
        });
    }

    #[gpui::test]
    fn space_on_a_tile_with_nothing_to_clean_stages_nothing(cx: &mut TestAppContext) {
        let (model, cx) = setup(cx, false);
        model.update(cx, |m, cx| m.stage_tile("~/Documents", cx));
        model.read_with(cx, |m, _| assert!(m.selected.is_empty()));
    }

    #[gpui::test]
    fn space_on_a_tile_with_nothing_to_clean_says_so(cx: &mut TestAppContext) {
        let (model, cx) = setup(cx, false);
        model.update(cx, |m, cx| m.stage_tile("~/Documents", cx));
        model.read_with(cx, |m, _| {
            assert_eq!(m.toast.as_deref(), Some("Nothing here can be cleaned"))
        });
    }

    #[gpui::test]
    fn space_on_a_folder_stages_exactly_its_ready_items(cx: &mut TestAppContext) {
        let (model, cx) = setup(cx, false);
        model.update(cx, |m, cx| m.stage_tile("~/.gradle", cx));
        model.read_with(cx, |m, _| {
            let want: HashSet<String> =
                ["gradle-caches:5".to_owned(), "gradle-caches:12".to_owned()].into();
            assert_eq!(m.selected, want);
        });
    }

    #[gpui::test]
    fn space_again_unstages_the_folder(cx: &mut TestAppContext) {
        let (model, cx) = setup(cx, false);
        model.update(cx, |m, cx| {
            m.stage_tile("~/.gradle", cx);
            m.stage_tile("~/.gradle", cx);
        });
        model.read_with(cx, |m, _| assert!(m.selected.is_empty()));
    }

    #[gpui::test]
    fn space_on_the_smaller_tile_stages_only_what_it_merged(cx: &mut TestAppContext) {
        let (model, cx) = setup(cx, false);
        // Home's smaller tile merges folders with no items; its path is home itself.
        model.update(cx, |m, cx| m.stage_tile("~#smaller", cx));
        model.read_with(cx, |m, _| assert!(m.selected.is_empty()));
    }

    #[gpui::test]
    fn an_arrow_with_nothing_selected_selects_the_largest_tile(cx: &mut TestAppContext) {
        let (model, cx) = setup(cx, false);
        model.update(cx, |m, cx| m.move_selection(Dir::Right, cx));
        model.read_with(cx, |m, _| assert_eq!(m.tile.as_deref(), Some("~/Desktop")));
    }

    #[gpui::test]
    fn an_arrow_moves_the_selection_to_a_neighbour(cx: &mut TestAppContext) {
        let (model, cx) = setup(cx, false);
        model.update(cx, |m, cx| {
            m.explorer.set_size((800, 600));
            m.select_tile(Some("~/Desktop".into()), cx);
            m.move_selection(Dir::Right, cx);
        });
        model.update(cx, |m, _| {
            let view = m.view.clone().unwrap();
            let rects = m.explorer.rects(&view).unwrap();
            let at = |key: &str| {
                let i = view
                    .children
                    .iter()
                    .position(|c| explorer::tile_key(c) == key);
                rects[i.unwrap()].clone()
            };
            let (from, to) = (at("~/Desktop"), at(m.tile.as_deref().unwrap()));
            assert!(to.x >= from.x + from.w, "{to:?} is not right of {from:?}");
        });
    }

    #[gpui::test]
    fn opening_a_folder_zooms_into_it(cx: &mut TestAppContext) {
        let (model, cx) = setup(cx, false);
        model.update(cx, |m, cx| {
            m.explorer.set_size((800, 600));
            m.open_tile("~/Desktop", cx);
        });
        cx.run_until_parked();
        model.read_with(cx, |m, _| assert!(m.explorer.zooming()));
    }

    #[gpui::test]
    fn the_zoom_ends_after_it_has_played(cx: &mut TestAppContext) {
        let (model, cx) = setup(cx, false);
        model.update(cx, |m, cx| {
            m.explorer.set_size((800, 600));
            m.open_tile("~/Desktop", cx);
        });
        cx.run_until_parked();
        cx.executor().advance_clock(Duration::from_secs(1));
        cx.run_until_parked();
        model.read_with(cx, |m, _| assert!(!m.explorer.zooming()));
    }

    #[gpui::test]
    fn a_breadcrumb_jump_does_not_zoom(cx: &mut TestAppContext) {
        let (model, cx) = setup(cx, false);
        model.update(cx, |m, cx| {
            m.explorer.set_size((800, 600));
            m.navigate("~/Desktop".into(), cx);
        });
        cx.run_until_parked();
        model.read_with(cx, |m, _| assert!(!m.explorer.zooming()));
    }

    #[gpui::test]
    fn backspace_at_the_root_does_nothing(cx: &mut TestAppContext) {
        let (model, cx) = setup(cx, false);
        model.update(cx, |m, cx| m.up(cx));
        cx.run_until_parked();
        model.read_with(cx, |m, _| assert_eq!(m.path, m.root_path));
    }

    #[gpui::test]
    fn going_up_selects_the_folder_it_left(cx: &mut TestAppContext) {
        let (model, cx) = setup(cx, false);
        model.update(cx, |m, cx| m.navigate("~/Desktop".into(), cx));
        cx.run_until_parked();
        model.update(cx, |m, cx| m.up(cx));
        cx.run_until_parked();
        model.read_with(cx, |m, _| assert_eq!(m.tile.as_deref(), Some("~/Desktop")));
    }

    #[gpui::test]
    fn selecting_a_folder_loads_its_view_for_the_inspector(cx: &mut TestAppContext) {
        let (model, cx) = setup(cx, false);
        model.update(cx, |m, cx| m.select_tile(Some("~/Desktop".into()), cx));
        cx.run_until_parked();
        model.read_with(cx, |m, _| {
            assert_eq!(
                m.tile_view.as_ref().map(|v| v.path.as_str()),
                Some("~/Desktop")
            )
        });
    }

    #[gpui::test]
    fn the_selected_tile_survives_a_rescan(cx: &mut TestAppContext) {
        let (model, cx) = setup(cx, false);
        model.update(cx, |m, _| m.tile = Some("~/Desktop".into()));
        rescan(&model, cx);
        model.read_with(cx, |m, _| assert_eq!(m.tile.as_deref(), Some("~/Desktop")));
    }

    #[gpui::test]
    fn a_vanished_tile_is_unselected_after_a_rescan(cx: &mut TestAppContext) {
        let (model, cx) = setup(cx, false);
        model.update(cx, |m, _| m.tile = Some("~/gone".into()));
        rescan(&model, cx);
        model.read_with(cx, |m, _| assert_eq!(m.tile, None));
    }

    #[gpui::test]
    fn highlight_cleanable_toggles(cx: &mut TestAppContext) {
        let (model, cx) = setup(cx, false);
        model.update(cx, |m, cx| m.toggle_highlight(cx));
        model.read_with(cx, |m, _| assert!(m.highlight_cleanable));
    }

    #[gpui::test]
    fn the_sidebar_lists_every_category(cx: &mut TestAppContext) {
        let (model, cx) = setup(cx, false);
        model.read_with(cx, |m, _| assert_eq!(m.category_totals.len(), 6));
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

    /// The first row the Recommended list shows.
    fn first_listed_row(model: &Entity<AppModel>, cx: &mut VisualTestContext) -> String {
        model.read_with(cx, |m, _| {
            recommendations::ordered(&m.recommendations)[0]
                .row_id
                .clone()
        })
    }

    #[gpui::test]
    fn j_with_nothing_focused_focuses_the_first_listed_row(cx: &mut TestAppContext) {
        let (model, cx) = setup(cx, false);
        let first = first_listed_row(&model, cx);
        model.update(cx, |m, cx| m.move_row(true, cx));
        assert_eq!(
            model.read_with(cx, |m, _| m.focused_row.clone()),
            Some(first)
        );
    }

    #[gpui::test]
    fn k_moves_to_the_previous_row(cx: &mut TestAppContext) {
        let (model, cx) = setup(cx, false);
        let first = first_listed_row(&model, cx);
        model.update(cx, |m, cx| {
            m.move_row(true, cx);
            m.move_row(true, cx);
            m.move_row(false, cx);
        });
        assert_eq!(
            model.read_with(cx, |m, _| m.focused_row.clone()),
            Some(first)
        );
    }

    #[gpui::test]
    fn space_on_a_row_stages_its_ready_items(cx: &mut TestAppContext) {
        let (model, cx) = setup(cx, false);
        let expected: HashSet<String> = model.update(cx, |m, cx| {
            m.focus_row(Some("npm-cache".into()), cx);
            m.stage_row(cx);
            let row = m.recommendations.iter().find(|r| r.row_id == "npm-cache");
            selection::ready_ids(row.unwrap())
                .into_iter()
                .map(String::from)
                .collect()
        });
        assert_eq!(model.read_with(cx, |m, _| m.selected.clone()), expected);
    }

    #[gpui::test]
    fn return_on_a_row_opens_the_inspector(cx: &mut TestAppContext) {
        let (model, cx) = setup(cx, false);
        model.update(cx, |m, cx| {
            m.inspector_visible = false;
            m.focus_row(Some("npm-cache".into()), cx);
            m.open_row(cx);
        });
        assert!(model.read_with(cx, |m, _| m.inspector_visible));
    }

    #[gpui::test]
    fn show_recommendation_opens_recommended_with_the_row(cx: &mut TestAppContext) {
        let (model, cx) = setup(cx, false);
        model.update(cx, |m, cx| m.show_row("npm-cache".into(), cx));
        assert_eq!(
            model.read_with(cx, |m, _| (m.page, m.focused_row.clone())),
            (Page::Recommended, Some("npm-cache".into()))
        );
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
        cx.update(gpui_component::init);
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
    fn navigation_during_a_swap_is_kept(cx: &mut TestAppContext) {
        let (model, cx) = setup(cx, false);
        model.update(cx, |m, cx| {
            let meta = m.meta.clone().unwrap();
            m.enter(meta, cx);
            m.navigate("~/Desktop".into(), cx);
        });
        cx.run_until_parked();
        assert_eq!(model.read_with(cx, |m, _| m.path.clone()), "~/Desktop");
    }

    #[gpui::test]
    fn the_inspected_row_clears_when_it_is_gone(cx: &mut TestAppContext) {
        let (model, cx) = setup(cx, false);
        model.update(cx, |m, _| m.focused_row = Some("gone-row".into()));
        rescan(&model, cx);
        assert!(model.read_with(cx, |m, _| m.focused_row.is_none()));
    }

    #[gpui::test]
    fn cleanup_is_refused_while_scanning(cx: &mut TestAppContext) {
        cx.update(gpui_component::init);
        let cx = cx.add_empty_window();
        let source = Source::Mock(MockData::new(true).with_step(Duration::from_millis(30)));
        let model = cx.update(|window, cx| cx.new(|cx| AppModel::new(source, window, cx)));
        cx.run_until_parked();
        model.update(cx, |m, cx| {
            m.start_scan(cx);
            m.open_cleanup(cx);
        });
        cx.run_until_parked();
        assert!(model.read_with(cx, |m, _| m.cleanup.is_none()));
    }

    /// A model over a mock whose snapshot is `age_secs` old.
    fn setup_aged(
        cx: &mut TestAppContext,
        age_secs: i64,
    ) -> (Entity<AppModel>, &mut VisualTestContext) {
        cx.update(gpui_component::init);
        let cx = cx.add_empty_window();
        let now = crate::cleanup::audit::now_ms() as i64 / 1000;
        let mock = MockData::new(false)
            .with_step(Duration::from_millis(30))
            .with_scanned_at(now - age_secs);
        let model =
            cx.update(|window, cx| cx.new(|cx| AppModel::new(Source::Mock(mock), window, cx)));
        cx.run_until_parked();
        (model, cx)
    }

    #[gpui::test]
    fn a_stale_snapshot_rescans_on_load(cx: &mut TestAppContext) {
        let (model, cx) = setup_aged(cx, STALE_AFTER + 3_600);
        assert!(model.read_with(cx, |m, _| m.scan.is_some()));
    }

    #[gpui::test]
    fn a_fresh_snapshot_does_not_rescan(cx: &mut TestAppContext) {
        let (model, cx) = setup_aged(cx, 3_600);
        assert!(model.read_with(cx, |m, _| m.scan.is_none()));
    }

    #[test]
    fn exactly_a_day_old_is_not_stale() {
        assert!(!is_stale(STALE_AFTER, 0));
    }

    #[test]
    fn a_day_and_a_second_old_is_stale() {
        assert!(is_stale(STALE_AFTER + 1, 0));
    }

    fn view_node(path: &str, size: u64, children: Vec<ViewNode>) -> ViewNode {
        ViewNode {
            id: path.into(),
            name: path.into(),
            path: path.into(),
            size,
            category: "dev_tools".into(),
            tier: None,
            rule_id: None,
            kind: ViewKind::Dir,
            file_count: 0,
            cleanable: 0,
            system_managed: false,
            children,
        }
    }

    #[test]
    fn subtract_removes_the_exact_node() {
        let mut root = view_node(
            "~",
            10,
            vec![view_node("~/a", 4, vec![]), view_node("~/b", 6, vec![])],
        );
        subtract(&mut root, "~/a", 4);
        assert_eq!(
            root.children
                .iter()
                .map(|c| c.path.as_str())
                .collect::<Vec<_>>(),
            ["~/b"]
        );
    }

    #[test]
    fn subtract_shrinks_the_folders_above() {
        let mut root = view_node(
            "~",
            10,
            vec![view_node("~/a", 4, vec![view_node("~/a/x", 3, vec![])])],
        );
        subtract(&mut root, "~/a/x", 3);
        assert_eq!((root.size, root.children[0].size), (7, 1));
    }

    #[test]
    fn subtract_lowers_cleanable_on_the_way() {
        let mut root = view_node("~", 10, vec![view_node("~/a", 4, vec![])]);
        root.cleanable = 4;
        subtract(&mut root, "~/a", 4);
        assert_eq!(root.cleanable, 0);
    }

    #[test]
    fn subtract_leaves_a_look_alike_sibling_alone() {
        let mut root = view_node("~", 10, vec![view_node("~/ab", 4, vec![])]);
        subtract(&mut root, "~/a", 3);
        assert_eq!(root.children[0].size, 4);
    }

    /// The model with a running cleanup that no event has touched yet.
    fn setup_running(cx: &mut TestAppContext) -> (Entity<AppModel>, &mut VisualTestContext) {
        let (model, cx) = setup(cx, false);
        model.update(cx, |m, _| {
            m.cleanup = Some(Cleanup::Running {
                plan: CleanupPlan {
                    plan_id: "p".into(),
                    snapshot_id: "s".into(),
                    actions: Vec::new(),
                    total: 0,
                    trash_total: 0,
                    permanent_available: false,
                    warnings: Vec::new(),
                },
                done: 0,
                current: None,
            });
        });
        (model, cx)
    }

    /// Finishes the first item of the first row with `outcome`; returns that item.
    fn finish_first_item(
        model: &Entity<AppModel>,
        cx: &mut VisualTestContext,
        outcome: Outcome,
    ) -> crate::recommend::model::RecItem {
        model.update(cx, |m, _| {
            let item = m.recommendations[0].items[0].clone();
            m.selected.insert(item.item_id.clone());
            m.on_cleanup_event(CleanupEvent::ItemFinished {
                item_id: item.item_id.clone(),
                freed: item.size,
                outcome,
            });
            item
        })
    }

    #[gpui::test]
    fn a_trashed_item_leaves_the_recommendations(cx: &mut TestAppContext) {
        let (model, cx) = setup_running(cx);
        let item = finish_first_item(&model, cx, Outcome::Trashed);
        assert!(model.read_with(cx, |m, _| {
            m.recommendations
                .iter()
                .all(|r| r.items.iter().all(|i| i.item_id != item.item_id))
        }));
    }

    #[gpui::test]
    fn a_trashed_item_shrinks_the_view(cx: &mut TestAppContext) {
        let (model, cx) = setup_running(cx);
        let before = model.read_with(cx, |m, _| m.view.as_ref().map(|v| v.size));
        let item = finish_first_item(&model, cx, Outcome::Trashed);
        let after = model.read_with(cx, |m, _| m.view.as_ref().map(|v| v.size));
        assert_eq!(after, before.map(|b| b - item.size));
    }

    #[gpui::test]
    fn a_trashed_item_leaves_the_selection(cx: &mut TestAppContext) {
        let (model, cx) = setup_running(cx);
        let item = finish_first_item(&model, cx, Outcome::Trashed);
        assert!(!model.read_with(cx, |m, _| m.selected.contains(&item.item_id)));
    }

    #[gpui::test]
    fn a_dry_run_item_stays(cx: &mut TestAppContext) {
        let (model, cx) = setup_running(cx);
        let before = model.read_with(cx, |m, _| m.recommendations.clone());
        finish_first_item(&model, cx, Outcome::DryRun);
        assert_eq!(
            model.read_with(cx, |m, _| m.recommendations.clone()),
            before
        );
    }

    #[gpui::test]
    fn a_cancelled_rescan_keeps_the_view(cx: &mut TestAppContext) {
        cx.update(gpui_component::init);
        let cx = cx.add_empty_window();
        let source = Source::Mock(MockData::new(false).with_step(Duration::from_millis(30)));
        let model = cx.update(|window, cx| cx.new(|cx| AppModel::new(source, window, cx)));
        cx.run_until_parked();
        model.update(cx, |m, cx| {
            m.start_scan(cx);
            m.cancel_scan(cx);
        });
        settle(cx, &model, |m| m.scan.is_none());
        assert!(model.read_with(cx, |m, _| m.view.is_some()));
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
