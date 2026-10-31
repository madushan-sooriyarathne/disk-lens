//! First launch (PRD 13.4.1): Welcome, Full Disk Access and the first scan, in one
//! card with a page indicator.

use std::sync::{Arc, LazyLock};

use gpui::{
    Context, Div, FontWeight, Image, ImageFormat, IntoElement, ParentElement, Role, Stateful,
    Styled, Window, div, img, prelude::*, px,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::{Disableable, Sizable, Size};

use super::format::{format_bytes, group_digits};
use super::model::{AppModel, Step};
use super::shell::CONTROL_HEIGHT;
use super::theme::{self, Theme};

const CARD_WIDTH: f32 = 520.;
const ICON: f32 = 96.;

static APP_ICON: LazyLock<Arc<Image>> = LazyLock::new(|| {
    Arc::new(Image::from_bytes(
        ImageFormat::Svg,
        include_bytes!("../../assets/icons/icon.svg").to_vec(),
    ))
});

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
        .child(status);
    let skip = button("onboarding-skip", "Continue Without")
        .ghost()
        .on_click(cx.listener(|m, _, _, cx| m.continue_without_access(cx)));
    let open = button("onboarding-open", "Open System Settings")
        .primary()
        .disabled(model.access_confirmed)
        .on_click(cx.listener(|m, _, _, cx| m.open_fda_settings(cx)));
    (content, vec![skip, open])
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
        );
    let cancel = button("onboarding-cancel", "Cancel")
        .on_click(cx.listener(|m, _, _, cx| m.cancel_scan(cx)));
    (content, vec![cancel])
}
