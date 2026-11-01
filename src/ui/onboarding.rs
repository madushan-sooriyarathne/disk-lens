//! First launch (PRD 13.4.1): Welcome, Full Disk Access and the first scan. Content
//! sits centred on the window over a warm gradient with film grain, with Back and
//! the page dots at the bottom.

use std::sync::{Arc, LazyLock};

use gpui::{
    Animation, AnimationExt, App, Context, CursorStyle, Div, FontWeight, Hsla, Image, ImageFormat,
    IntoElement, MouseButton, ParentElement, Role, Stateful, Styled, Window, div, img,
    linear_color_stop, linear_gradient, prelude::*, px, relative, rgba, svg,
};

use super::format::{format_bytes, group_digits};
use super::model::{AppModel, Step};
use super::theme::{self, Theme};
use super::{fda_preview, motion};

/// Width of the centred column.
const COLUMN: f32 = 488.;
const APP_ICON_SIZE: f32 = 104.;
const DISPLAY: f32 = 34.;
const PILL_H: f32 = 40.;
const NOISE_TILE: f32 = 128.;

/// `scripts/make-icon.sh`. A PNG, because GPUI rasterizes an SVG of this size on its
/// first draw and the icon would pop in.
static APP_ICON: LazyLock<Arc<Image>> = LazyLock::new(|| {
    Arc::new(Image::from_bytes(
        ImageFormat::Png,
        include_bytes!("../../assets/icons/icon-256.png").to_vec(),
    ))
});

/// `scripts/make-noise.py`; drawn at half size so each grain is one device pixel.
static NOISE: LazyLock<Arc<Image>> = LazyLock::new(|| {
    Arc::new(Image::from_bytes(
        ImageFormat::Png,
        include_bytes!("../../assets/onboarding/noise.png").to_vec(),
    ))
});

/// (icon, title, line) for the Welcome step (PRD FR-ONB-2).
const FEATURES: [(&str, &str, &str); 3] = [
    (
        "grid",
        "See what fills your disk",
        "Every large folder, named and explained, one level at a time.",
    ),
    (
        "sparkle",
        "Find space that is safe to free",
        "Caches and build output that rebuild on their own, gathered in one place.",
    ),
    (
        "tray",
        "Nothing goes without your say",
        "You confirm every cleanup, and most items go to the Trash first.",
    ),
];

/// Starts decoding the images, so they are ready by the first onboarding frame.
pub fn preload(window: &mut Window, cx: &mut App) {
    APP_ICON.clone().get_render_image(window, cx);
    NOISE.clone().get_render_image(window, cx);
}

pub fn render(
    model: &mut AppModel,
    step: Step,
    window: &mut Window,
    cx: &mut Context<AppModel>,
) -> impl IntoElement {
    let t = theme::current(window);
    let viewport = window.viewport_size();
    let content = match step {
        Step::Welcome => welcome(&t, cx),
        Step::Access => access(&t, model, f32::from(viewport.height), cx),
        Step::FirstScan => first_scan(&t, model, cx),
    };
    div()
        .relative()
        .size_full()
        .overflow_hidden()
        .bg(t.content)
        .text_size(px(theme::T_BODY))
        .child(backdrop(
            &t,
            f32::from(viewport.width),
            f32::from(viewport.height),
        ))
        .child(
            div()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .child(step_transition(model, content)),
        )
        .child(footer(&t, model, step, cx))
}

/// How far a step travels as it enters.
const SLIDE: f32 = 24.;

/// The step slides in from the side it comes from (the right going forward, the
/// left going back) as it fades in; under Reduce Motion it only fades.
fn step_transition(model: &AppModel, content: Div) -> impl IntoElement {
    let transition = motion::Transition::new(motion::NAV, model.reduce_motion);
    let moves = matches!(transition, motion::Transition::Move { .. });
    let from = if model.went_back { -SLIDE } else { SLIDE };
    div()
        .relative()
        .w(px(COLUMN))
        .child(content)
        .with_animation(
            ("onboarding-step", model.step_seq),
            Animation::new(transition.duration()).with_easing(motion::ease),
            move |d, p| {
                let d = d.opacity(p);
                if moves {
                    d.left(px((1. - p) * from))
                } else {
                    d
                }
            },
        )
}

/// A warm glow from the top over the window color, and film grain over both.
fn backdrop(t: &Theme, width: f32, height: f32) -> Div {
    let glow: Hsla = if t.dark {
        rgba(0xff5a2e38).into()
    } else {
        rgba(0xff6a3d2e).into()
    };
    let tiles = ((width / NOISE_TILE).ceil() * (height / NOISE_TILE).ceil()) as usize;
    div()
        .absolute()
        .inset_0()
        .child(div().absolute().inset_0().bg(linear_gradient(
            180.,
            linear_color_stop(glow, 0.),
            linear_color_stop(glow.opacity(0.), 0.6),
        )))
        .child(
            div()
                .absolute()
                .inset_0()
                .flex()
                .flex_wrap()
                .opacity(if t.dark { 0.06 } else { 0.05 })
                .children((0..tiles).map(|_| img(NOISE.clone()).size(px(NOISE_TILE)))),
        )
}

const STEPS: [(Step, &str); 3] = [
    (Step::Welcome, "Welcome"),
    (Step::Access, "Full Disk Access"),
    (Step::FirstScan, "First scan"),
];

/// Back on the left (not on Welcome), the page dots on the right, both within the
/// centered column.
fn footer(t: &Theme, model: &AppModel, step: Step, cx: &mut Context<AppModel>) -> Div {
    let back = (step != Step::Welcome).then(|| {
        let (color, hover) = (t.secondary, t.label);
        theme::focusable(div().id("onboarding-back"), t, Role::Button, "Back")
            .flex()
            .items_center()
            .gap(px(4.))
            .px_2()
            .py_1()
            .rounded(px(theme::R_CONTROL))
            .text_color(color)
            .cursor_pointer()
            .hover(move |s| s.text_color(hover))
            .child(
                svg()
                    .path("icons/ui/back.svg")
                    .size(px(12.))
                    .text_color(color),
            )
            .child("Back")
            .on_click(cx.listener(|m, _, _, cx| m.onboarding_back(cx)))
    });
    // Negative margins cancel the hit padding, so the chevron and the last dot line
    // up with the column's edges.
    div()
        .absolute()
        .bottom(px(24.))
        .left_0()
        .right_0()
        .flex()
        .justify_center()
        .child(
            div()
                .w(px(COLUMN))
                .h(px(28.))
                .flex()
                .items_center()
                .justify_between()
                .child(div().ml(px(-8.)).children(back))
                .child(div().mr(px(-4.)).child(indicator(t, model, step, cx))),
        )
}

/// Three dots, the current one in the accent color; a reached step's dot goes to it.
fn indicator(t: &Theme, model: &AppModel, step: Step, cx: &mut Context<AppModel>) -> Stateful<Div> {
    let current = STEPS.iter().position(|&(s, _)| s == step).unwrap_or(0);
    div()
        .id("page-indicator")
        .role(Role::Group)
        .aria_label(format!("Step {} of 3", current + 1))
        .flex()
        .items_center()
        .children(STEPS.iter().enumerate().map(|(i, &(to, name))| {
            let active = model.can_go(to);
            let dim = t.tertiary.opacity(0.5);
            let hover = t.tertiary;
            // The hit area pads the dot, which stays 6 pt high.
            div()
                .id(("onboarding-dot", i))
                .role(Role::Button)
                .aria_label(format!("Step {}: {name}", i + 1))
                .px(px(4.))
                .py(px(6.))
                .group("dot")
                .child(
                    div()
                        .h(px(6.))
                        .w(px(if i == current { 18. } else { 6. }))
                        .rounded_full()
                        .bg(if i == current { t.accent } else { dim })
                        .when(active, |d| d.group_hover("dot", move |s| s.bg(hover))),
                )
                .when(active, |d| {
                    d.cursor_pointer()
                        .on_click(cx.listener(move |m, _, _, cx| m.onboarding_go(to, cx)))
                })
        }))
}

fn display(text: &'static str) -> Div {
    div()
        .text_size(px(DISPLAY))
        .line_height(px(40.))
        .font_weight(FontWeight::BOLD)
        .child(text)
}

fn lead(t: &Theme, text: &'static str) -> Div {
    div()
        .text_size(px(15.))
        .line_height(px(21.))
        .text_color(t.secondary)
        .text_center()
        .child(text)
}

/// A full-width capsule button; Tab reaches it and Return presses it.
fn pill(t: &Theme, id: &'static str, label: &'static str, fill: Hsla, text: Hsla) -> Stateful<Div> {
    theme::focusable(div().id(id), t, Role::Button, label)
        .h(px(PILL_H))
        .w_full()
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .bg(fill)
        .text_color(text)
        .text_size(px(14.))
        .font_weight(FontWeight::SEMIBOLD)
        .cursor_pointer()
        .hover(move |s| s.bg(fill.opacity(0.88)))
        .child(label)
}

/// A quiet text button under the main action.
fn text_button(t: &Theme, id: &'static str, label: &'static str) -> Stateful<Div> {
    let color = t.secondary;
    let hover = t.label;
    theme::focusable(div().id(id), t, Role::Button, label)
        .px_2()
        .py_1()
        .rounded(px(theme::R_CONTROL))
        .text_color(color)
        .cursor_pointer()
        .hover(move |s| s.text_color(hover))
        .child(label)
}

fn welcome(t: &Theme, cx: &mut Context<AppModel>) -> Div {
    let tints = [t.accent, t.safe, t.review];
    let features = div()
        .flex()
        .flex_col()
        .gap(px(18.))
        .children(
            FEATURES
                .iter()
                .zip(tints)
                .map(|(&(icon, title, line), tint)| {
                    div()
                        .flex()
                        .gap(px(14.))
                        .child(
                            div()
                                .flex_none()
                                .size(px(36.))
                                .rounded(px(10.))
                                .bg(tint.opacity(0.14))
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(
                                    svg()
                                        .path(format!("icons/ui/{icon}.svg"))
                                        .size(px(18.))
                                        .text_color(tint),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(2.))
                                .child(div().font_weight(FontWeight::SEMIBOLD).child(title))
                                .child(div().text_color(t.secondary).child(line)),
                        )
                }),
        );
    div()
        .flex()
        .flex_col()
        .items_center()
        .child(img(APP_ICON.clone()).size(px(APP_ICON_SIZE)))
        .child(div().mt(px(20.)).child(display("Welcome to DiskLens")))
        .child(div().mt(px(8.)).child(lead(
            t,
            "A clear picture of your disk, and a safe way to give space back.",
        )))
        .child(div().mt(px(36.)).w(px(420.)).child(features))
        .child(
            div().mt(px(40.)).w(px(280.)).child(
                pill(
                    t,
                    "onboarding-continue",
                    "Continue",
                    t.accent,
                    gpui::white(),
                )
                .on_click(cx.listener(|m, _, _, cx| m.onboarding_continue(cx))),
            ),
        )
}

/// Height the step needs besides the System Settings pane, so the pane shrinks
/// first in a short window (down to the minimum, 640 pt).
const ACCESS_REST: f32 = 420.;
const HERO_MAX: f32 = 260.;
const HERO_MIN: f32 = 220.;

fn access(t: &Theme, model: &AppModel, height: f32, cx: &mut Context<AppModel>) -> Div {
    // The pane fades into the window below the DiskLens row.
    let hero_h = (height - ACCESS_REST).clamp(HERO_MIN, HERO_MAX);
    let fade = div()
        .absolute()
        .left_0()
        .right_0()
        .bottom_0()
        .h(px((hero_h - fda_preview::DISKLENS_BOTTOM).max(40.)))
        .bg(linear_gradient(
            180.,
            linear_color_stop(t.content.opacity(0.), 0.),
            linear_color_stop(t.content, 1.),
        ));
    let hero = div()
        .relative()
        .h(px(hero_h))
        .overflow_hidden()
        .flex()
        .justify_center()
        .child(fda_preview::render(
            t,
            APP_ICON.clone(),
            model.reduce_motion,
        ))
        .child(fade);
    let action = if model.access_confirmed {
        div()
            .h(px(PILL_H))
            .w_full()
            .flex()
            .items_center()
            .justify_center()
            .rounded_full()
            .bg(t.safe_tint)
            .text_color(t.safe_text)
            .text_size(px(14.))
            .font_weight(FontWeight::SEMIBOLD)
            .child("✓  Full Disk Access is on")
            .into_any_element()
    } else {
        pill(
            t,
            "onboarding-open",
            "Open System Settings",
            t.accent,
            gpui::white(),
        )
        .on_click(cx.listener(|m, _, _, cx| m.open_fda_settings(cx)))
        .into_any_element()
    };
    div()
        .flex()
        .flex_col()
        .items_center()
        .child(hero)
        .child(div().mt(px(8.)).child(display("Allow Full Disk Access")))
        .child(div().mt(px(8.)).child(lead(
            t,
            "macOS keeps some folders, like Mail and other apps' data, out of reach until you \
             allow it. DiskLens only measures sizes and never opens your files.",
        )))
        .child(div().mt(px(24.)).w_full().child(drag_row(t, cx)))
        .child(div().mt(px(16.)).w_full().child(action))
        .child(
            div().mt(px(10.)).child(
                text_button(t, "onboarding-skip", "Continue without access")
                    .on_click(cx.listener(|m, _, _, cx| m.continue_without_access(cx))),
            ),
        )
}

/// The app to drag into the list when System Settings does not show it.
fn drag_row(t: &Theme, cx: &mut Context<AppModel>) -> Div {
    let icon = div()
        .id("drag-app")
        .flex_none()
        .cursor(CursorStyle::OpenHand)
        .role(Role::Image)
        .aria_label("DiskLens app icon, drag it into the Full Disk Access list")
        .child(img(APP_ICON.clone()).size(px(44.)))
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|m, _, window, _| m.drag_app(window)),
        );
    let (fill, text) = (t.label, t.content);
    let finder = theme::focusable(
        div().id("reveal-app"),
        t,
        Role::Button,
        "Show DiskLens in Finder",
    )
    .flex_none()
    .h(px(30.))
    .px(px(14.))
    .flex()
    .items_center()
    .rounded_full()
    .bg(fill)
    .text_color(text)
    .font_weight(FontWeight::MEDIUM)
    .cursor_pointer()
    .hover(move |s| s.bg(fill.opacity(0.85)))
    .child("Show in Finder")
    .on_click(cx.listener(|m, _, _, cx| m.reveal_app(cx)));
    div()
        .flex()
        .items_center()
        .gap(px(12.))
        .p(px(12.))
        .rounded(px(theme::R_WIDGET))
        .bg(t.popover)
        .border_1()
        .border_color(t.separator)
        .shadow_sm()
        .child(icon)
        .child(
            div()
                .flex_1()
                .flex()
                .flex_col()
                .gap(px(2.))
                .child(div().font_weight(FontWeight::SEMIBOLD).child("DiskLens"))
                .child(
                    div()
                        .text_size(px(theme::T_SMALL + 1.))
                        .text_color(t.secondary)
                        .child("Not in the list? Drag this icon into it."),
                ),
        )
        .child(finder)
}

fn first_scan(t: &Theme, model: &AppModel, cx: &mut Context<AppModel>) -> Div {
    let p = model.scan.clone().unwrap_or_default();
    let current = if p.current.is_empty() {
        "Starting…".to_string()
    } else {
        p.current
    };
    div()
        .flex()
        .flex_col()
        .items_center()
        .child(
            div()
                .text_size(px(theme::T_SMALL))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(t.tertiary)
                .child("SCANNING YOUR HOME FOLDER"),
        )
        .child(
            div()
                .mt(px(10.))
                .text_size(px(56.))
                .line_height(px(62.))
                .font_weight(FontWeight::BOLD)
                .child(format_bytes(p.bytes)),
        )
        .child(
            div()
                .mt(px(2.))
                .text_size(px(15.))
                .text_color(t.secondary)
                .child(format!("{} files", group_digits(p.files))),
        )
        .child(
            div()
                .mt(px(32.))
                .w_full()
                .child(stacked_bar(t, &p.categories)),
        )
        .child(div().mt(px(20.)).w_full().child(legend(t, &p.categories)))
        .child(
            div()
                .mt(px(28.))
                .w_full()
                .truncate()
                .text_center()
                .font_family(theme::MONO)
                .text_size(px(theme::T_SMALL))
                .text_color(t.tertiary)
                .child(current),
        )
        .child(
            div().mt(px(20.)).child(
                text_button(t, "onboarding-cancel", "Cancel")
                    .on_click(cx.listener(|m, _, _, cx| m.cancel_scan(cx))),
            ),
        )
}

/// `bytes` (rules order) as `(id, bytes)` in the sidebar's order.
fn by_category(bytes: &[u64]) -> [(&'static str, u64); 6] {
    let ids = &crate::rules::model::Rules::embedded().categories;
    theme::LEGEND_ORDER.map(|id| {
        let size = ids
            .iter()
            .position(|c| c.id == id)
            .and_then(|i| bytes.get(i))
            .copied()
            .unwrap_or(0);
        (id, size)
    })
}

/// One bar split by category, growing as the scan finds space.
fn stacked_bar(t: &Theme, bytes: &[u64]) -> Div {
    let total = bytes.iter().sum::<u64>().max(1) as f32;
    div()
        .h(px(10.))
        .w_full()
        .flex()
        .gap(px(2.))
        .rounded_full()
        .overflow_hidden()
        .bg(t.separator)
        .children(
            by_category(bytes)
                .into_iter()
                .filter(|&(_, b)| b > 0)
                .map(|(id, b)| {
                    div()
                        .h_full()
                        .w(relative(b as f32 / total))
                        .bg(theme::category_colors(t, id).1)
                }),
        )
}

/// The six categories in a 3 by 2 grid: dot, label, size.
fn legend(t: &Theme, bytes: &[u64]) -> Div {
    div()
        .grid()
        .grid_cols(3)
        .gap_x(px(16.))
        .gap_y(px(10.))
        .children(by_category(bytes).map(|(id, size)| {
            div()
                .flex()
                .items_center()
                .gap(px(6.))
                .child(
                    div()
                        .flex_none()
                        .size(px(8.))
                        .rounded_full()
                        .bg(theme::category_colors(t, id).1),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_color(t.secondary)
                        .child(theme::category_label(id)),
                )
                .child(
                    div()
                        .font_weight(FontWeight::MEDIUM)
                        .child(format_bytes(size)),
                )
        }))
}

/// The six categories with their bytes so far, for the scanning screen (PRD FR-ONB-4).
pub(super) fn category_rows(t: &Theme, bytes: &[u64]) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_3()
        .child(stacked_bar(t, bytes))
        .child(legend(t, bytes))
}
