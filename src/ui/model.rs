//! App state and actions (owner: A4). Replaces the React stores
//! `legacy-web/src/store/{scan,view,selection}.ts`.
//!
//! Frozen contract: the public fields and method signatures below. Private
//! fields and helpers may be added freely.

use std::collections::HashSet;
use std::sync::Arc;

use gpui::{Context, FocusHandle, Window};

use super::explorer::ExplorerState;
use super::recommendations::PanelState;
use super::source::Source;
use crate::recommend::model::{ItemDetails, Recommendation};
use crate::scanner::tree::SnapshotMeta;
use crate::view::{ViewMode, ViewNode};

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
}

impl AppModel {
    /// Builds the model, focuses it, refreshes recommendations whenever the
    /// window becomes active (blocked states depend on running apps), and
    /// loads the last snapshot in the background.
    pub fn new(source: Source, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        window.focus(&focus_handle, cx);
        Self {
            source: Arc::new(source),
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
        }
    }

    pub fn start_scan(&mut self, cx: &mut Context<Self>) {
        todo!()
    }

    pub fn cancel_scan(&mut self, cx: &mut Context<Self>) {
        todo!()
    }

    /// Shows the folder at `path` in the explorer.
    pub fn navigate(&mut self, path: String, cx: &mut Context<Self>) {
        todo!()
    }

    /// Goes up one folder; does nothing at the scan root.
    pub fn up(&mut self, cx: &mut Context<Self>) {
        todo!()
    }

    pub fn set_mode(&mut self, mode: ViewMode, cx: &mut Context<Self>) {
        todo!()
    }

    pub fn hover(&mut self, node_id: Option<String>, cx: &mut Context<Self>) {
        todo!()
    }

    /// Toggles one item; blocked or unknown items are ignored.
    pub fn toggle_item(&mut self, item_id: &str, cx: &mut Context<Self>) {
        todo!()
    }

    pub fn toggle_row(&mut self, row_id: &str, cx: &mut Context<Self>) {
        todo!()
    }

    pub fn select_safe(&mut self, cx: &mut Context<Self>) {
        todo!()
    }

    /// Opens the detail drawer for `row_id` and loads its details; `None` closes it.
    pub fn focus_row(&mut self, row_id: Option<String>, cx: &mut Context<Self>) {
        todo!()
    }

    /// Re-evaluates blocked states and drops selections that became stale.
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        todo!()
    }

    /// Shows a scanned path in Finder.
    pub fn reveal(&mut self, path: String, cx: &mut Context<Self>) {
        todo!()
    }
}
