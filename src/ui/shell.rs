//! Window shell (PRD 13.4.3): sidebar, unified toolbar and the inspector slot
//! around the page content.

use gpui::{
    AnyElement, Context, FontWeight, Hsla, Role, SharedString, Window, div, prelude::*, px, svg,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::{Disableable, Selectable, Sizable, Size};

use super::chrome::{SIDEBAR_WIDTH, TOOLBAR_HEIGHT};
use super::explorer;
use super::format::format_bytes;
use super::model::{AppModel, Page};
use super::screens::unreadable_notice;
use super::stats::compute_stats;
use super::theme::{self, Theme};
use crate::cleanup::audit::now_ms;

/// Room for the traffic lights when the sidebar is hidden.
const TRAFFIC_LIGHTS_WIDTH: f32 = 78.;
pub(super) const CONTROL_HEIGHT: f32 = 28.;

fn icon(name: &str, color: Hsla) -> impl IntoElement {
    svg()
        .path(SharedString::from(format!("icons/ui/{name}.svg")))
        .size(px(16.))
        .flex_none()
        .text_color(color)
}

fn icon_button(id: &'static str, name: &str, label: &'static str, t: &Theme) -> Button {
    Button::new(id)
        .ghost()
        .with_size(Size::Size(px(CONTROL_HEIGHT)))
        .icon(gpui_component::Icon::default().path(format!("icons/ui/{name}.svg")))
        .text_color(t.secondary)
        .accessibility_label(label)
        .tooltip(label)
}

pub fn sidebar(model: &AppModel, t: Theme, cx: &mut Context<AppModel>) -> impl IntoElement {
    let ready_rows = model.recommendations.len();
    let nav = [
        (Page::Overview, "Overview", "grid", None),
        (Page::Explore, "Explore", "map", None),
        (
            Page::Recommended,
            "Recommended",
            "sparkle",
            Some(ready_rows),
        ),
    ]
    .into_iter()
    .map(|(page, label, name, count)| {
        let on = model.page == page;
        theme::focusable(div().id(label), &t, Role::Button, label)
            .flex()
            .items_center()
            .gap_2()
            .h(px(CONTROL_HEIGHT))
            .px_2()
            .rounded(px(theme::R_CONTROL))
            .font_weight(FontWeight::MEDIUM)
            .cursor_pointer()
            .when(on, |d| d.bg(t.selected))
            .when(!on, |d| d.hover(move |s| s.bg(t.hover)))
            .child(icon(name, t.accent))
            .child(label)
            .children(count.map(|n| {
                div()
                    .ml_auto()
                    .text_size(px(theme::T_SMALL))
                    .text_color(t.sidebar_secondary)
                    .child(n.to_string())
            }))
            .on_click(cx.listener(move |m, _, _, cx| m.set_page(page, cx)))
    });

    let mut totals = model.category_totals.clone();
    totals.sort_by_key(|c| std::cmp::Reverse(c.size));
    let categories = totals.into_iter().map(|c| {
        let (_, dot) = theme::category_colors(&t, &c.category);
        let noun = if c.folders == 1 { "folder" } else { "folders" };
        div()
            .flex()
            .items_center()
            .gap_2()
            .px_2()
            .py_1()
            .child(div().size(px(10.)).flex_none().rounded_full().bg(dot))
            .child(
                div()
                    .min_w_0()
                    .child(div().truncate().child(theme::category_label(&c.category)))
                    .child(
                        div()
                            .text_size(px(theme::T_SMALL))
                            .text_color(t.sidebar_secondary)
                            .child(format!("{} {noun}", c.folders)),
                    ),
            )
            .child(
                div()
                    .ml_auto()
                    .flex_none()
                    .text_size(px(12.))
                    .text_color(t.sidebar_secondary)
                    .child(format_bytes(c.size)),
            )
    });

    div()
        .w(px(SIDEBAR_WIDTH))
        .flex_none()
        .flex()
        .flex_col()
        .gap(px(2.))
        .pt(px(TOOLBAR_HEIGHT))
        .px(px(10.))
        .pb(px(10.))
        .bg(if model.reduce_transparency {
            t.sidebar_solid
        } else {
            t.sidebar
        })
        .border_r_1()
        .border_color(t.separator)
        .text_size(px(theme::T_BODY))
        .children(nav)
        .child(
            div()
                .pt_4()
                .pb(px(6.))
                .px_2()
                .text_size(px(theme::T_SMALL))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(t.sidebar_secondary)
                .child("Categories"),
        )
        .children(categories)
        .child(div().mt_auto().children(footer_notice(model, t, cx)))
}

/// Full Disk Access and unreadable folders (FR-WIN-5), in place of the v1 banner.
fn footer_notice(model: &AppModel, t: Theme, cx: &mut Context<AppModel>) -> Option<AnyElement> {
    let unreadable = model.meta.as_ref().map_or(0, |m| m.inaccessible_count);
    let missing = !model.full_disk_access;
    let text = unreadable_notice(unreadable).or_else(|| {
        missing.then(|| "Some folders can't be read until access is granted.".to_owned())
    })?;
    let notice = div()
        .rounded(px(theme::R_CONTROL))
        .bg(t.review_tint)
        .p(px(10.))
        .text_size(px(theme::T_SMALL))
        .line_height(px(15.))
        .child(
            div()
                .text_size(px(12.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(t.review_text)
                .child(if missing {
                    "Full Disk Access is off"
                } else {
                    "Some folders were skipped"
                }),
        )
        .child(text);
    if !missing {
        return Some(notice.into_any_element());
    }
    Some(
        notice
            .child(
                theme::focusable(
                    div().id("grant-fda"),
                    &t,
                    Role::Button,
                    "Grant Full Disk Access",
                )
                .mt(px(6.))
                .text_color(t.accent_text)
                .font_weight(FontWeight::MEDIUM)
                .cursor_pointer()
                .child("Grant Access…")
                .on_click(cx.listener(|m, _, _, cx| m.open_fda_settings(cx))),
            )
            .into_any_element(),
    )
}

pub fn toolbar(model: &AppModel, t: Theme, cx: &mut Context<AppModel>) -> impl IntoElement {
    let head = match model.page {
        Page::Explore => explorer::crumbs(model, t, cx).into_any_element(),
        page => div()
            .text_size(px(15.))
            .font_weight(FontWeight::SEMIBOLD)
            .child(match page {
                Page::Overview => "Overview",
                _ => "Recommended",
            })
            .into_any_element(),
    };
    let nav = (model.page == Page::Explore).then(|| {
        div()
            .flex()
            .child(
                icon_button("back", "back", "Back", &t)
                    .disabled(model.path == model.root_path)
                    .on_click(cx.listener(|m, _, _, cx| m.up(cx))),
            )
            .child(
                icon_button("forward", "fwd", "Forward", &t)
                    .disabled(model.forward.is_empty())
                    .on_click(cx.listener(|m, _, _, cx| m.go_forward(cx))),
            )
    });

    let scanning = model.scan.is_some();
    let status = explorer::status_label(
        model.scan.as_ref(),
        model.error.as_deref(),
        model.meta.as_ref().map(|m| m.scanned_at),
        now_ms() as i64 / 1000,
    );
    let status_color = if !scanning && model.error.is_some() {
        t.review_text
    } else {
        t.secondary
    };
    // Cancel while updating, Retry after a failure, else Rescan (FR-BG-2, FR-BG-7).
    let scan_button = match (scanning, model.error.is_some()) {
        (true, _) => Button::new("cancel-scan")
            .label("Cancel")
            .small()
            .on_click(cx.listener(|m, _, _, cx| m.cancel_scan(cx))),
        (false, true) => Button::new("retry-scan")
            .label("Retry")
            .small()
            .on_click(cx.listener(|m, _, _, cx| m.start_scan(cx))),
        (false, false) => icon_button("rescan", "rescan", "Rescan", &t)
            .on_click(cx.listener(|m, _, _, cx| m.start_scan(cx))),
    };

    let staged = model
        .meta
        .as_ref()
        .map(|meta| compute_stats(meta, &model.recommendations, &model.selected).selected)
        .unwrap_or(0);
    let queue_label = if model.selected.is_empty() {
        String::new()
    } else {
        format!("{} · {}", model.selected.len(), format_bytes(staged))
    };
    let clean_label = if staged == 0 {
        "Clean up".to_owned()
    } else {
        format!("Clean up {}", format_bytes(staged))
    };
    let clean = Button::new("clean-up")
        .primary()
        .with_size(Size::Size(px(CONTROL_HEIGHT)))
        .label(clean_label)
        .disabled(model.selected.is_empty() || scanning)
        .on_click(cx.listener(|m, _, _, cx| m.open_cleanup(cx)));
    // FR-BG-5: a plan built now would belong to the snapshot about to be replaced.
    let clean = if scanning {
        clean.tooltip("Available when the update finishes")
    } else {
        clean
    };

    div()
        .h(px(TOOLBAR_HEIGHT))
        .flex_none()
        .flex()
        .items_center()
        .gap(px(6.))
        .pl(px(if model.sidebar_visible {
            10.
        } else {
            TRAFFIC_LIGHTS_WIDTH
        }))
        .pr(px(12.))
        .text_size(px(theme::T_BODY))
        .child(
            icon_button("toggle-sidebar", "sidebar", "Toggle Sidebar", &t)
                .on_click(cx.listener(|m, _, _, cx| m.toggle_sidebar(cx))),
        )
        .children(nav)
        .child(div().ml(px(6.)).min_w_0().child(head))
        .child(div().flex_1())
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .mr(px(6.))
                .text_size(px(12.))
                .text_color(status_color)
                .children(status)
                .child(scan_button),
        )
        .child(
            // Only Explore has an inspector until Phase 11 adds Recommended.
            icon_button("toggle-inspector", "inspector", "Inspector", &t)
                .selected(model.inspector_visible && model.page == Page::Explore)
                .disabled(model.page != Page::Explore)
                .on_click(cx.listener(|m, _, _, cx| m.toggle_inspector(cx))),
        )
        .child(
            Button::new("queue")
                .ghost()
                .with_size(Size::Size(px(CONTROL_HEIGHT)))
                .icon(gpui_component::Icon::default().path("icons/ui/tray.svg"))
                .label(queue_label)
                .accessibility_label("Queue")
                .tooltip("Queue")
                // The queue panel arrives with the inspector content in Phase 11.
                .on_click(cx.listener(|m, _, _, cx| m.toggle_inspector(cx))),
        )
        .child(clean)
}

/// The page under the toolbar.
pub fn page(model: &mut AppModel, window: &mut Window, cx: &mut Context<AppModel>) -> AnyElement {
    match model.page {
        Page::Overview => super::overview::render(model, window, cx).into_any_element(),
        Page::Explore => explorer::render(model, window, cx).into_any_element(),
        Page::Recommended => super::recommendations::render(model, window, cx).into_any_element(),
    }
}
