//! Window layout by phase (PRD section 9) and window-wide keys.

use gpui::{App, Context, KeyBinding, Window, actions, div, prelude::*, px};

use super::model::{AppModel, Cleanup, Phase};
use super::{cleanup, screens, shell, theme};

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
            .text_color(t.label)
            .font_family(theme::SANS)
            .font_features(theme::numeric());
        let full_window = |body: gpui::Div| body.p_4().bg(t.content);
        let body = match self.phase {
            Phase::Loading => full_window(body),
            Phase::Onboarding => full_window(body).child(screens::onboarding(self, window, cx)),
            Phase::Idle => full_window(body).child(screens::idle(self, window, cx)),
            Phase::Scanning => full_window(body).child(screens::scanning(self, window, cx)),
            Phase::Error => full_window(body).child(screens::error(self, window, cx)),
            // The sidebar stays translucent over the blurred window; the rest is opaque.
            Phase::Ready => body
                .when(self.sidebar_visible, |b| {
                    b.child(shell::sidebar(self, t, cx))
                })
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .bg(t.content)
                        .child(shell::toolbar(self, t, cx))
                        .child(
                            div()
                                .flex_1()
                                .min_h_0()
                                .flex()
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .flex()
                                        .flex_col()
                                        .gap(px(theme::GAP_CONTENT))
                                        .px(px(theme::GAP_CONTENT))
                                        .pb(px(theme::GAP_CONTENT))
                                        .pt_1()
                                        .children(screens::update_banner(self, window, cx))
                                        .child(shell::page(self, window, cx)),
                                )
                                .when(self.inspector_visible, |d| d.child(shell::inspector(t))),
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
