//! First launch (PRD 13.4.1): Welcome, Full Disk Access and the first scan, in one
//! card with a page indicator.

use std::path::PathBuf;
use std::sync::{Arc, LazyLock};
use std::time::Duration;

use gpui::{
    Animation, AnimationExt, Context, CursorStyle, Div, FontWeight, Image, ImageFormat,
    IntoElement, MouseButton, ParentElement, Role, Stateful, Styled, Window, div, img, prelude::*,
    px, relative,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::{Disableable, Sizable, Size};

use super::format::{format_bytes, group_digits};
use super::model::{AppModel, Step};
use super::motion;
use super::shell::CONTROL_HEIGHT;
use super::theme::{self, Theme};
use crate::platform::drag;

const CARD_WIDTH: f32 = 520.;
const ICON: f32 = 96.;

static APP_ICON: LazyLock<Arc<Image>> = LazyLock::new(|| {
    Arc::new(Image::from_bytes(
        ImageFormat::Svg,
        include_bytes!("../../assets/icons/icon.svg").to_vec(),
    ))
});

const TOGGLE_W: f32 = 34.;
const TOGGLE_H: f32 = 20.;
const TOGGLE_LOOP: Duration = Duration::from_millis(2400);
const DRAG_ICON: f32 = 40.;

/// The bundle to drag; none under `cargo run`, where only the Finder link shows.
static BUNDLE: LazyLock<Option<PathBuf>> = LazyLock::new(drag::app_bundle);

const POINTS: [&str; 3] = [
    "See what fills your disk, folder by folder.",
    "Find space that is safe to free: caches and build output that rebuild on their own.",
    "Nothing is removed without your confirmation, and most items go to the Trash.",
];

pub fn render(
    model: &mut AppModel,
    step: Step,
    window: &mut Window,
    cx: &mut Context<AppModel>,
) -> impl IntoElement {
    let t = theme::current(window);
    let (content, buttons) = match step {
        Step::Welcome => welcome(&t, cx),
        Step::Access => access(&t, model, cx),
        Step::FirstScan => first_scan(&t, model, cx),
    };
    div()
        .flex_1()
        .h_full()
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .w(px(CARD_WIDTH))
                .p(px(24.))
                .flex()
                .flex_col()
                .gap(px(20.))
                .rounded(px(theme::R_WIDGET))
                .bg(t.widget)
                .text_size(px(theme::T_BODY))
                .child(indicator(&t, step))
                .child(content)
                .child(div().flex().justify_end().gap_2().children(buttons)),
        )
}

/// Three dots, the current one in the accent color.
fn indicator(t: &Theme, step: Step) -> Stateful<Div> {
    let current = match step {
        Step::Welcome => 0,
        Step::Access => 1,
        Step::FirstScan => 2,
    };
    div()
        .flex()
        .justify_center()
        .gap(px(6.))
        .id("page-indicator")
        .role(Role::Group)
        .aria_label(format!("Step {} of 3", current + 1))
        .children((0..3).map(|i| {
            div()
                .size(px(6.))
                .rounded_full()
                .bg(if i == current { t.accent } else { t.tertiary })
        }))
}

fn title(text: &'static str) -> Div {
    div()
        .text_size(px(theme::T_TITLE))
        .font_weight(FontWeight::SEMIBOLD)
        .child(text)
}

fn button(id: &'static str, label: &'static str) -> Button {
    Button::new(id)
        .label(label)
        .with_size(Size::Size(px(CONTROL_HEIGHT)))
}

fn welcome(t: &Theme, cx: &mut Context<AppModel>) -> (Div, Vec<Button>) {
    let points = div().flex().flex_col().gap_2().children(POINTS.map(|p| {
        div()
            .flex()
            .gap_2()
            .child(div().text_color(t.accent).child("•"))
            .child(div().text_color(t.secondary).child(p))
    }));
    let content = div()
        .flex()
        .flex_col()
        .items_center()
        .gap_4()
        .child(img(APP_ICON.clone()).size(px(ICON)))
        .child(title("Welcome to DiskLens"))
        .child(points);
    let go = button("onboarding-continue", "Continue")
        .primary()
        .on_click(cx.listener(|m, _, _, cx| m.onboarding_continue(cx)));
    (content, vec![go])
}

fn access(t: &Theme, model: &AppModel, cx: &mut Context<AppModel>) -> (Div, Vec<Button>) {
    let status = if model.access_confirmed {
        div()
            .text_color(t.safe_text)
            .font_weight(FontWeight::MEDIUM)
            .child("✓ Full Disk Access is on")
    } else {
        div()
            .text_color(t.secondary)
            .child("Turn on DiskLens in the list. This window moves on by itself.")
    };
    let content = div()
        .flex()
        .flex_col()
        .gap_3()
        .child(title("Allow Full Disk Access"))
        .child(div().text_color(t.secondary).child(
            "macOS hides some folders, such as Mail and other apps' data, until you allow \
             Full Disk Access. DiskLens only measures folder sizes and never opens your files.",
        ))
        .child(status)
        .child(drag_card(t, model, cx));
    let skip = button("onboarding-skip", "Continue Without")
        .ghost()
        .on_click(cx.listener(|m, _, _, cx| m.continue_without_access(cx)));
    let open = button("onboarding-open", "Open System Settings")
        .primary()
        .disabled(model.access_confirmed)
        .on_click(cx.listener(|m, _, window, cx| m.open_fda_settings(window, cx)));
    (content, vec![skip, open])
}

/// The toggle switching on, and the app to drag into the list when it is missing.
fn drag_card(t: &Theme, model: &AppModel, cx: &mut Context<AppModel>) -> Div {
    let (off, lit, knob) = (t.tertiary, t.safe, t.widget);
    let toggle = move |on: f32| {
        div()
            .w(px(TOGGLE_W))
            .h(px(TOGGLE_H))
            .rounded_full()
            .bg(theme::mix(off, lit, on))
            .child(
                div()
                    .mt(px(2.))
                    .ml(px(2. + on * (TOGGLE_W - TOGGLE_H)))
                    .size(px(TOGGLE_H - 4.))
                    .rounded_full()
                    .bg(knob),
            )
    };
    let toggle = if model.reduce_motion {
        toggle(1.).into_any_element()
    } else {
        div()
            .child(toggle(0.))
            .with_animation(
                "fda-toggle",
                Animation::new(TOGGLE_LOOP).repeat(),
                move |_, d| {
                    // Off, switching on, then on for most of the loop.
                    let on = motion::ease(((d - 0.2) / 0.2).clamp(0., 1.));
                    div().child(toggle(on))
                },
            )
            .into_any_element()
    };
    let row = div().flex().items_center().gap_3().child(toggle).child(
        div()
            .flex_1()
            .text_color(t.secondary)
            .child("Don't see DiskLens? Drag this icon into the list."),
    );
    let row = if BUNDLE.is_some() {
        row.child(
            div()
                .id("drag-app")
                .cursor(CursorStyle::OpenHand)
                .role(Role::Image)
                .aria_label("DiskLens app icon, drag it into the Full Disk Access list")
                .child(img(APP_ICON.clone()).size(px(DRAG_ICON)))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|m, _, window, _| m.drag_app(window)),
                ),
        )
    } else {
        row
    };
    div()
        .flex()
        .flex_col()
        .gap_2()
        .p(px(theme::GAP_WIDGET))
        .rounded(px(theme::R_CONTROL))
        .bg(t.content)
        .child(row)
        .child(
            div().flex().child(
                Button::new("reveal-app")
                    .link()
                    .label("Show DiskLens in Finder")
                    .on_click(cx.listener(|m, _, _, cx| m.reveal_app(cx))),
            ),
        )
}

fn first_scan(t: &Theme, model: &AppModel, cx: &mut Context<AppModel>) -> (Div, Vec<Button>) {
    let p = model.scan.clone().unwrap_or_default();
    let content = div()
        .flex()
        .flex_col()
        .gap_3()
        .child(title("Scanning your home folder"))
        .child(div().text_color(t.secondary).child(format!(
            "{} files · {}",
            group_digits(p.files),
            format_bytes(p.bytes)
        )))
        .child(
            div()
                .truncate()
                .font_family(theme::MONO)
                .text_size(px(theme::T_SMALL))
                .text_color(t.secondary)
                .child(if p.current.is_empty() {
                    "Starting…".to_string()
                } else {
                    p.current
                }),
        )
        .child(category_rows(t, &p.categories));
    let cancel = button("onboarding-cancel", "Cancel")
        .on_click(cx.listener(|m, _, _, cx| m.cancel_scan(cx)));
    (content, vec![cancel])
}

/// The six categories with their bytes so far, filling in as the scan finds them
/// (PRD FR-ONB-4). `bytes` is in rules order; rows follow the sidebar's order.
pub(super) fn category_rows(t: &Theme, bytes: &[u64]) -> Div {
    let ids = &crate::rules::model::Rules::embedded().categories;
    let of = |id: &str| {
        ids.iter()
            .position(|c| c.id == id)
            .and_then(|i| bytes.get(i))
            .copied()
            .unwrap_or(0)
    };
    let total = bytes.iter().sum::<u64>().max(1);
    div()
        .flex()
        .flex_col()
        .gap_2()
        .children(theme::LEGEND_ORDER.map(|id| {
            let size = of(id);
            let (_, color) = theme::category_colors(t, id);
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(div().size(px(8.)).rounded_full().bg(color))
                        .child(div().flex_1().child(theme::category_label(id)))
                        .child(div().text_color(t.secondary).child(format_bytes(size))),
                )
                .child(
                    div().h(px(4.)).rounded_full().bg(t.content).child(
                        div()
                            .h_full()
                            .rounded_full()
                            .bg(color)
                            .w(relative(size as f32 / total as f32)),
                    ),
                )
        }))
}
