//! Full-window states (onboarding, idle, scanning, error) and the notice banner.

use std::time::Duration;

use gpui::{
    Animation, AnimationExt, Context, Div, FontWeight, Hsla, Role, Stateful, Window, div,
    ease_in_out, prelude::*, px, relative,
};

use super::format::{format_bytes, group_digits};
use super::model::{AppModel, UpdateState};
use super::theme::{self, Theme};

const CARD_WIDTH: f32 = 480.;

/// Centered 480 px card (`m-auto w-[480px] rounded-xl border p-6`).
fn centered(t: &Theme, content: Div) -> Div {
    div()
        .flex_1()
        .h_full()
        .flex()
        .items_center()
        .justify_center()
        .child(
            content
                .w(px(CARD_WIDTH))
                .p_6()
                .rounded(px(12.))
                .border_1()
                .border_color(t.card_border)
                .bg(t.card),
        )
}

fn title(text: &'static str) -> Div {
    div()
        .text_size(px(15.))
        .font_weight(FontWeight::SEMIBOLD)
        .child(text)
}

/// The Tailwind `mt-5 rounded-lg px-4 py-1.5 text-[13px] font-medium` button.
fn button(t: &Theme, id: &'static str, label: &'static str) -> Stateful<Div> {
    theme::focusable(div().id(id), t, Role::Button, label)
        .mt_5()
        .px_4()
        .py(px(6.))
        .rounded_lg()
        .text_size(px(13.))
        .font_weight(FontWeight::MEDIUM)
        .cursor_pointer()
        .child(label)
}

fn message(
    t: Theme,
    heading: &'static str,
    body: String,
    body_color: Hsla,
    action: &'static str,
    cx: &mut Context<AppModel>,
) -> Div {
    let go = button(&t, "scan", action)
        .bg(t.text)
        .text_color(t.card)
        .on_click(cx.listener(|this, _, _, cx| this.start_scan(cx)));
    let body = div()
        .mt(px(6.))
        .text_size(px(13.))
        .text_color(body_color)
        .child(body);
    centered(
        &t,
        div()
            .child(title(heading))
            .child(body)
            .child(div().flex().child(go)),
    )
}

/// Full Disk Access is missing (US-01, PRD 7.8). The user can continue without it.
pub fn onboarding(
    _model: &mut AppModel,
    window: &mut Window,
    cx: &mut Context<AppModel>,
) -> impl IntoElement {
    let t = theme::current(window);
    let bg = t.bg;
    let open = button(&t, "open-fda", "Open System Settings")
        .bg(t.text)
        .text_color(t.card)
        .on_click(cx.listener(|this, _, _, cx| this.open_fda_settings(cx)));
    let skip = button(&t, "skip-fda", "Continue without access")
        .border_1()
        .border_color(t.card_border)
        .hover(move |s| s.bg(bg))
        .on_click(cx.listener(|this, _, _, cx| this.continue_without_access(cx)));
    let body = div()
        .mt(px(6.))
        .text_size(px(13.))
        .text_color(t.muted)
        .child(
            "macOS hides folders such as Mail, Safari and other apps' data until you grant Full Disk Access. \
             Without it, those folders count as unreadable and their sizes are missing. \
             DiskLens only reads folder sizes and never asks for an administrator password.",
        )
        .child(
            div()
                .mt_2()
                .child("In System Settings, turn on DiskLens under Privacy & Security, Full Disk Access, then come back."),
        );
    centered(
        &t,
        div()
            .child(title("Allow Full Disk Access"))
            .child(body)
            .child(div().flex().gap_2().child(open).child(skip)),
    )
}

/// No scan yet, or a cancelled first scan.
pub fn idle(
    model: &mut AppModel,
    window: &mut Window,
    cx: &mut Context<AppModel>,
) -> impl IntoElement {
    let t = theme::current(window);
    let body = model.error.clone().unwrap_or_else(|| {
        "DiskLens reads folder sizes only. It never opens your files and removes nothing until you choose."
            .into()
    });
    message(
        t,
        "Scan your home folder",
        body,
        t.muted,
        "Scan home folder",
        cx,
    )
}

pub fn scanning(
    model: &mut AppModel,
    window: &mut Window,
    cx: &mut Context<AppModel>,
) -> impl IntoElement {
    let t = theme::current(window);
    let p = model.scan.clone().unwrap_or_default();
    let (_, bar) = theme::category_colors(&t, "code_projects");
    let stat = |label: &'static str, value: String| {
        div()
            .flex_1()
            .child(div().text_size(px(13.)).text_color(t.muted).child(label))
            .child(
                div()
                    .font_family(theme::MONO)
                    .text_size(px(22.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(value),
            )
    };
    let current = if p.current.is_empty() {
        "Starting…".to_string()
    } else {
        p.current.clone()
    };
    // `slide`: the third-width bar runs from -100% to 300% of its own width.
    let slider = div()
        .h_full()
        .w(relative(1. / 3.))
        .rounded_full()
        .bg(bar)
        .with_animation(
            "scan-slide",
            Animation::new(Duration::from_millis(1200))
                .repeat()
                .with_easing(ease_in_out),
            |bar, d| bar.ml(relative((-1. + 4. * d) / 3.)),
        );
    let cancel = button(&t, "cancel", "Cancel")
        .border_1()
        .border_color(t.card_border)
        .hover(|s| s.bg(t.bg))
        .on_click(cx.listener(|this, _, _, cx| this.cancel_scan(cx)));
    centered(
        &t,
        div()
            .child(title("Scanning…"))
            .child(
                div()
                    .mt(px(4.))
                    .h(px(4.))
                    .overflow_hidden()
                    .rounded_full()
                    .bg(t.bg)
                    .child(slider),
            )
            .child(
                div()
                    .mt_5()
                    .flex()
                    .gap_4()
                    .child(stat("Files counted", group_digits(p.files)))
                    .child(stat("Size so far", format_bytes(p.bytes))),
            )
            .child(
                div()
                    .mt_4()
                    .truncate()
                    .font_family(theme::MONO)
                    .text_xs()
                    .text_color(t.muted)
                    .child(current),
            )
            .child(div().flex().child(cancel)),
    )
}

pub fn error(
    model: &mut AppModel,
    window: &mut Window,
    cx: &mut Context<AppModel>,
) -> impl IntoElement {
    let t = theme::current(window);
    let body = model
        .error
        .clone()
        .unwrap_or_else(|| "Something went wrong.".into());
    message(t, "Scan failed", body, t.error, "Try again", cx)
}

/// Notice above the stat cards about unreadable folders, with the way to grant access. Empty otherwise.
pub fn banner(
    model: &mut AppModel,
    window: &mut Window,
    cx: &mut Context<AppModel>,
) -> impl IntoElement {
    let t = theme::current(window);
    let unreadable = model.meta.as_ref().map_or(0, |m| m.inaccessible_count);
    let Some(text) = unreadable_notice(unreadable) else {
        return div();
    };
    let banner = div()
        .flex()
        .items_center()
        .gap_3()
        .px_4()
        .py_2()
        .rounded_lg()
        .border_1()
        .border_color(t.review)
        .bg(t.review_fill)
        .text_size(px(13.))
        .child(div().flex_1().child(text));
    if model.full_disk_access {
        return banner;
    }
    banner.child(
        theme::focusable(
            div().id("grant-fda"),
            &t,
            Role::Button,
            "Grant Full Disk Access",
        )
        .flex_none()
        .px_3()
        .py_1()
        .rounded_lg()
        .border_1()
        .border_color(t.review)
        .bg(t.card)
        .font_weight(FontWeight::MEDIUM)
        .cursor_pointer()
        .child("Grant Full Disk Access")
        .on_click(cx.listener(|this, _, _, cx| this.open_fda_settings(cx))),
    )
}

/// Button label and what a click does.
type UpdateAction = (&'static str, fn(&mut AppModel, &mut Context<AppModel>));

/// Update notice under the banner: an available release, the install, and the restart. Empty when idle.
pub fn update_banner(
    model: &mut AppModel,
    window: &mut Window,
    cx: &mut Context<AppModel>,
) -> Option<Div> {
    let t = theme::current(window);
    let can_install = model.can_install_update();
    // (text, button label, action); no button while installing.
    let (text, button): (String, Option<UpdateAction>) = match &model.update_state {
        UpdateState::Idle => return None,
        UpdateState::Available { version } if can_install => (
            format!("DiskLens {version} is available"),
            Some(("Install", AppModel::install_update)),
        ),
        UpdateState::Available { version } => (
            format!(
                "DiskLens {version} is available. Install after the current scan or cleanup finishes."
            ),
            None,
        ),
        UpdateState::Installing => ("Installing update…".into(), None),
        UpdateState::Ready if can_install => (
            "Restart to finish the update".into(),
            Some(("Restart", |_, cx| cx.restart())),
        ),
        UpdateState::Ready => (
            "Restart to finish the update after the current scan or cleanup finishes.".into(),
            None,
        ),
        UpdateState::Failed(message) => (
            format!("Update failed: {message}"),
            Some(("Dismiss", AppModel::dismiss_update)),
        ),
    };
    let banner = div()
        .flex()
        .items_center()
        .gap_3()
        .px_4()
        .py_2()
        .rounded_lg()
        .border_1()
        .border_color(t.review)
        .bg(t.review_fill)
        .text_size(px(13.))
        .child(div().flex_1().child(text));
    let Some((label, action)) = button else {
        return Some(banner);
    };
    Some(
        banner.child(
            theme::focusable(div().id("update-action"), &t, Role::Button, label)
                .flex_none()
                .px_3()
                .py_1()
                .rounded_lg()
                .border_1()
                .border_color(t.review)
                .bg(t.card)
                .font_weight(FontWeight::MEDIUM)
                .cursor_pointer()
                .child(label)
                .on_click(cx.listener(move |this, _, _, cx| action(this, cx))),
        ),
    )
}

fn unreadable_notice(n: u32) -> Option<String> {
    (n > 0).then(|| {
        let noun = if n == 1 { "folder" } else { "folders" };
        format!("{n} {noun} couldn't be read. Granting Full Disk Access lets DiskLens see them.")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unreadable_notice_pluralizes() {
        assert_eq!(unreadable_notice(0), None);
        assert!(
            unreadable_notice(1)
                .unwrap()
                .starts_with("1 folder couldn't")
        );
        assert!(
            unreadable_notice(3)
                .unwrap()
                .starts_with("3 folders couldn't")
        );
    }
}
