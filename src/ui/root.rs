//! Window layout by phase (PRD section 9) and window-wide keys.

use gpui::{App, Context, KeyBinding, Window, actions, div, prelude::*, px};

use super::model::{AppModel, Cleanup, Phase};
use super::{cleanup, explorer, recommendations, screens, stat_cards, theme};

actions!(disklens, [Back, FocusNext, FocusPrev]);

const CONTEXT: &str = "DiskLens";

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("escape", Back, Some(CONTEXT)),
        KeyBinding::new("tab", FocusNext, Some(CONTEXT)),
        KeyBinding::new("shift-tab", FocusPrev, Some(CONTEXT)),
    ]);
}

impl Render for AppModel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme::current(window);
        let body = div()
            .relative()
            .size_full()
            .flex()
            .p_4()
            .bg(t.bg)
            .text_color(t.text)
            .font_family(theme::SANS);
        let body = match self.phase {
            Phase::Loading => body,
            Phase::Onboarding => body.child(screens::onboarding(self, window, cx)),
            Phase::Idle => body.child(screens::idle(self, window, cx)),
            Phase::Scanning => body.child(screens::scanning(self, window, cx)),
            Phase::Error => body.child(screens::error(self, window, cx)),
            Phase::Ready => body
                .flex_col()
                .gap_4()
                .child(screens::banner(self, window, cx))
                .child(stat_cards::render(self, window, cx))
                .child(
                    div()
                        .flex()
                        .flex_1()
                        .min_h_0()
                        .gap_4()
                        .child(explorer::render(self, window, cx))
                        .child(
                            div()
                                .w(px(390.))
                                .flex_none()
                                .child(recommendations::render(self, window, cx)),
                        ),
                ),
        };
        let dialog = cleanup::render(self, window, cx);
        let toast = cleanup::toast(self, window);
        body.children(dialog)
            .children(toast)
            .key_context(CONTEXT)
            .track_focus(&self.focus_handle)
            .on_action(|_: &FocusNext, window, cx| window.focus_next(cx))
            .on_action(|_: &FocusPrev, window, cx| window.focus_prev(cx))
            // Esc closes the cleanup dialog (not while it runs), then the context
            // menu, then the detail drawer, otherwise goes up one folder.
            .on_action(cx.listener(|this, _: &Back, _, cx| {
                if let Some(stage) = &this.cleanup {
                    if !matches!(stage, Cleanup::Running { .. }) {
                        this.close_cleanup(cx);
                    }
                } else if this.explorer.close_menu() {
                    cx.notify();
                } else if this.focused_row.is_some() {
                    this.focus_row(None, cx);
                } else {
                    this.up(cx);
                }
            }))
    }
}
