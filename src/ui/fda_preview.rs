//! A drawn copy of System Settings › Privacy & Security › Full Disk Access (macOS 26)
//! with DiskLens switched on, for the onboarding step (PRD FR-ONB-3a). It follows the
//! app's appearance. Other rows are neutral placeholders, so no other app is shown.

use std::sync::Arc;
use std::time::Duration;

use gpui::{
    Animation, AnimationExt, AnyElement, Div, FontWeight, Hsla, Image, IntoElement, ParentElement,
    Styled, div, img, px, rgb, rgba,
};

use super::motion;
use super::theme::{self, Theme};

pub const WIDTH: f32 = 488.;
const ROW: f32 = 40.;
const ICON: f32 = 20.;
const TOGGLE_W: f32 = 36.;
const TOGGLE_H: f32 = 20.;
/// The DiskLens toggle switches on, holds, and starts again.
const LOOP: Duration = Duration::from_millis(3200);

/// System Settings' own colors, read off the macOS 26 pane.
#[derive(Clone, Copy)]
struct Palette {
    window: Hsla,
    group: Hsla,
    capsule: Hsla,
    capsule_border: Hsla,
    separator: Hsla,
    label: Hsla,
    secondary: Hsla,
    placeholder: Hsla,
    off_track: Hsla,
    off_knob: Hsla,
    off_mark: Hsla,
    on_track: Hsla,
    on_knob: Hsla,
    highlight: Hsla,
}

fn palette(t: &Theme) -> Palette {
    let c = |hex: u32| -> Hsla { rgb(hex).into() };
    let a = |hex: u32| -> Hsla { rgba(hex).into() };
    if t.dark {
        Palette {
            window: c(0x1e1e1f),
            group: c(0x262628),
            capsule: c(0x2a2a2c),
            capsule_border: a(0xffffff1f),
            separator: a(0xffffff14),
            label: c(0xf2f2f7),
            secondary: c(0x9a9a9f),
            placeholder: a(0xffffff24),
            off_track: c(0x3a3a3c),
            off_knob: c(0xe5e5ea),
            off_mark: c(0x8e8e93),
            on_track: c(0x2f7cf6),
            on_knob: c(0xdfe9ff),
            highlight: a(0x2f7cf62e),
        }
    } else {
        Palette {
            window: c(0xf0f0f2),
            group: c(0xffffff),
            capsule: c(0xffffff),
            capsule_border: a(0x0000001a),
            separator: a(0x00000014),
            label: c(0x1d1d1f),
            secondary: c(0x6e6e73),
            placeholder: a(0x0000001a),
            off_track: c(0xe3e3e8),
            off_knob: c(0xffffff),
            off_mark: c(0xaeaeb2),
            on_track: c(0x0a7aff),
            on_knob: c(0xffffff),
            highlight: a(0x0a7aff1f),
        }
    }
}

/// Placeholder rows around DiskLens: icon tint, name width, toggle on.
const ABOVE: [(u32, f32, bool); 2] = [(0x8e8e93, 92., false), (0x5e5ce6, 128., true)];
const BELOW: [(u32, f32, bool); 2] = [(0x30b0c7, 104., false), (0xff9f0a, 76., true)];

pub fn render(t: &Theme, icon: Arc<Image>, reduce_motion: bool) -> Div {
    let p = palette(t);
    let header = div()
        .flex()
        .items_center()
        .gap_3()
        .child(
            div()
                .flex()
                .items_center()
                .h(px(28.))
                .px(px(10.))
                .gap(px(10.))
                .rounded_full()
                .bg(p.capsule)
                .border_1()
                .border_color(p.capsule_border)
                .text_color(p.secondary)
                .child("‹")
                .child(div().w(px(1.)).h(px(14.)).bg(p.capsule_border))
                .child("›"),
        )
        .child(
            div()
                .text_size(px(15.))
                .font_weight(FontWeight::BOLD)
                .text_color(p.label)
                .child("Full Disk Access"),
        );
    let placeholder = |(tint, name, on): (u32, f32, bool)| {
        row(
            &p,
            div()
                .size(px(ICON))
                .rounded(px(5.))
                .bg(Hsla::from(rgb(tint)).opacity(0.85))
                .into_any_element(),
            div()
                .w(px(name))
                .h(px(8.))
                .rounded_full()
                .bg(p.placeholder)
                .into_any_element(),
            toggle(&p, if on { 1. } else { 0. }).into_any_element(),
        )
    };
    let disklens_toggle = if reduce_motion {
        toggle(&p, 1.).into_any_element()
    } else {
        div()
            .child(toggle(&p, 0.))
            .with_animation(
                "fda-preview-toggle",
                Animation::new(LOOP).repeat(),
                move |_, d| {
                    div().child(toggle(&p, motion::ease(((d - 0.25) / 0.15).clamp(0., 1.))))
                },
            )
            .into_any_element()
    };
    let disklens = row(
        &p,
        img(icon).size(px(ICON)).into_any_element(),
        div()
            .text_color(p.label)
            .font_weight(FontWeight::MEDIUM)
            .child("DiskLens.app")
            .into_any_element(),
        disklens_toggle,
    )
    .bg(p.highlight);
    let group = div()
        .flex()
        .flex_col()
        .rounded(px(12.))
        .bg(p.group)
        .overflow_hidden()
        .child(
            div()
                .px(px(14.))
                .py(px(10.))
                .text_size(px(12.))
                .line_height(px(16.))
                .text_color(p.secondary)
                .child(
                    "Allow the applications below to access data like Mail, Messages, Safari, \
                     Home, Time Machine backups, and certain administrative settings for all \
                     users on this Mac.",
                ),
        )
        .children(ABOVE.map(placeholder))
        .child(disklens)
        .children(BELOW.map(placeholder));
    div()
        .w(px(WIDTH))
        .flex()
        .flex_col()
        .gap(px(14.))
        .p(px(16.))
        .rounded(px(16.))
        .bg(p.window)
        .border_1()
        .border_color(p.capsule_border)
        .shadow_lg()
        .text_size(px(13.))
        .font_family(theme::SANS)
        .child(header)
        .child(group)
}

/// One list row: icon, name and toggle, with the separator above it.
fn row(p: &Palette, icon: AnyElement, name: AnyElement, toggle: AnyElement) -> Div {
    div()
        .h(px(ROW))
        .px(px(14.))
        .flex()
        .items_center()
        .gap(px(10.))
        .border_t_1()
        .border_color(p.separator)
        .child(icon)
        .child(div().flex_1().child(name))
        .child(toggle)
}

/// The macOS 26 switch: off shows a ring on the right, on shows a bar on the left.
fn toggle(p: &Palette, on: f32) -> Div {
    let track = theme::mix(p.off_track, p.on_track, on);
    let knob = theme::mix(p.off_knob, p.on_knob, on);
    let knob_w = TOGGLE_W - TOGGLE_H + 4.;
    div()
        .relative()
        .w(px(TOGGLE_W))
        .h(px(TOGGLE_H))
        .rounded_full()
        .bg(track)
        .child(
            // The "I" on, the "O" off, under the knob's free side.
            div()
                .absolute()
                .top(px(TOGGLE_H / 2. - 4.))
                .left(px(7.))
                .w(px(1.5))
                .h(px(8.))
                .rounded_full()
                .bg(knob)
                .opacity(on),
        )
        .child(
            div()
                .absolute()
                .top(px(TOGGLE_H / 2. - 3.))
                .right(px(6.))
                .size(px(6.))
                .rounded_full()
                .border_1()
                .border_color(p.off_mark)
                .opacity(1. - on),
        )
        .child(
            div()
                .absolute()
                .top(px(2.))
                .left(px(2. + on * (TOGGLE_W - 4. - knob_w)))
                .w(px(knob_w))
                .h(px(TOGGLE_H - 4.))
                .rounded_full()
                .bg(knob),
        )
}
