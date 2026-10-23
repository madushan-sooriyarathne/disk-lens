//! Explorer card: header, breadcrumb, treemap, legend, tooltip and context
//! menu (owner: B2). Port of `legacy-web/src/components/Explorer/*`.

use gpui::{Context, Window, div, prelude::*};

use super::model::AppModel;

/// Explorer-only UI state (layout cache, container size, open menu). Fields are up to the owner.
#[derive(Default)]
pub struct ExplorerState {}

pub fn render(
    model: &mut AppModel,
    window: &mut Window,
    cx: &mut Context<AppModel>,
) -> impl IntoElement {
    div().flex_1()
}
