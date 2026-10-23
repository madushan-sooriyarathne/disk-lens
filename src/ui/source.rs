//! Where the UI gets its data (owner: A4): the real backend, or the mock
//! snapshot when `DISKLENS_MOCK` is set. Replaces `legacy-web/src/lib/{api,mockApi}.ts`.
//!
//! Frozen contract: the public items below. Methods block; the model calls
//! them off the UI thread. `MockData` internals are up to the owner.

use std::sync::Arc;

use crate::error::Result;
use crate::recommend::model::{ItemDetails, Recommendation};
use crate::scanner::tree::{ScanEvent, SnapshotMeta};
use crate::state::AppState;
use crate::view::{ViewMode, ViewNode};

pub enum Source {
    Live(Arc<AppState>),
    Mock(MockData),
}

/// `assets/mock/snapshot.mock.json`, which mirrors `docs/mockup.jpg`.
pub struct MockData {}

impl Source {
    /// `DISKLENS_MOCK=1` serves the mock, `DISKLENS_MOCK=preselect` also selects
    /// the mockup's rows; otherwise the live backend.
    pub fn from_env() -> Self {
        todo!()
    }

    pub fn load_last(&self) -> Option<SnapshotMeta> {
        todo!()
    }

    /// Returns at once; events arrive on another thread.
    pub fn start_scan(&self, on_event: impl FnMut(ScanEvent) + Send + 'static) -> Result<()> {
        todo!()
    }

    pub fn cancel_scan(&self) {
        todo!()
    }

    pub fn view(&self, path: &str, mode: ViewMode) -> Result<ViewNode> {
        todo!()
    }

    pub fn recommendations(&self) -> Result<Vec<Recommendation>> {
        todo!()
    }

    pub fn details(&self, row_id: &str) -> Result<ItemDetails> {
        todo!()
    }

    pub fn reveal(&self, path: &str) -> Result<()> {
        todo!()
    }

    /// Row IDs to select after loading: the mock's `preselected` with
    /// `DISKLENS_MOCK=preselect`, otherwise none (S-09).
    pub fn preselected_rows(&self) -> Vec<String> {
        todo!()
    }
}
