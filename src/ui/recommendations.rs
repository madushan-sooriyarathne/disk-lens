//! Recommendations card: tier sections, rows, footer and the detail drawer
//! (owner: B3). Port of `legacy-web/src/components/Recommendations/*`.

use gpui::{Context, Window, div, prelude::*};

use super::model::AppModel;

/// Panel-only UI state. Fields are up to the owner.
#[derive(Default)]
pub struct PanelState {}

pub fn render(
    model: &mut AppModel,
    window: &mut Window,
    cx: &mut Context<AppModel>,
) -> impl IntoElement {
    div().size_full()
}
