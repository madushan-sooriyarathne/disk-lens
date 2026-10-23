//! Full-window states and the notice banner (owner: B1). Port of
//! `legacy-web/src/components/{EmptyState,ScanProgress,Banner}.tsx`.

use gpui::{Context, Window, div, prelude::*};

use super::model::AppModel;

/// No scan yet, or a cancelled first scan.
pub fn idle(
    model: &mut AppModel,
    window: &mut Window,
    cx: &mut Context<AppModel>,
) -> impl IntoElement {
    div()
}

pub fn scanning(
    model: &mut AppModel,
    window: &mut Window,
    cx: &mut Context<AppModel>,
) -> impl IntoElement {
    div()
}

pub fn error(
    model: &mut AppModel,
    window: &mut Window,
    cx: &mut Context<AppModel>,
) -> impl IntoElement {
    div()
}

/// Notice above the stat cards: a cancelled rescan, or unreadable folders. Empty otherwise.
pub fn banner(
    model: &mut AppModel,
    window: &mut Window,
    cx: &mut Context<AppModel>,
) -> impl IntoElement {
    div()
}
