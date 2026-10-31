//! Recommended page (PRD 13.4.6): tier sections, rows and the queue footer.

use std::collections::HashSet;

use gpui::{
    AnyView, App, ClickEvent, Context, Div, ElementId, FontWeight, Role, ScrollHandle,
    SharedString, Stateful, Toggled, Window, div, linear_color_stop, linear_gradient, prelude::*,
    px, svg,
};
use gpui_component::button::Button;
use gpui_component::{Sizable, Size};

use super::format::{format_bytes, format_size};
use super::keymap;
use super::model::AppModel;
use super::selection::{RowState, blocked_reason, row_state};
use super::shell::CONTROL_HEIGHT;
use super::stats::compute_stats;
use super::theme::{self, MONO, Theme};
use crate::recommend::model::{Recommendation, Tier};

/// Page-only UI state; selection, focus and details live in the model.
#[derive(Default)]
pub struct PanelState {
    /// The list, so j/k can bring the focused row into view.
    pub(super) scroll: ScrollHandle,
}

const TIERS: [Tier; 2] = [Tier::Safe, Tier::Review];
const ROW_HEIGHT: f32 = 52.;
/// Height of the fade at the list's bottom edge (FR-REC-1).
const FADE: f32 = 48.;

/// Rows as the list shows them: safe first, then review, each in the backend's order.
pub(super) fn ordered(recs: &[Recommendation]) -> Vec<&Recommendation> {
    TIERS
        .iter()
        .flat_map(|tier| recs.iter().filter(move |r| r.tier == *tier))
        .collect()
}

/// The list child index of `row_id`: each tier's rows follow its header.
pub(super) fn list_index(recs: &[Recommendation], row_id: &str) -> Option<usize> {
    let mut ix = 0;
    for tier in TIERS {
        let rows: Vec<&Recommendation> = recs.iter().filter(|r| r.tier == tier).collect();
        if rows.is_empty() {
            continue;
        }
        ix += 1;
        for r in rows {
            if r.row_id == row_id {
                return Some(ix);
            }
            ix += 1;
        }
    }
    None
}

pub fn render(
    model: &mut AppModel,
    window: &mut Window,
    cx: &mut Context<AppModel>,
) -> impl IntoElement {
    let t = theme::current(window);
    let page = div()
        .relative()
        .size_full()
        .flex()
        .flex_col()
        .gap(px(theme::GAP_WIDGET));
    let Some(meta) = &model.meta else {
        return page;
    };
    let recs = &model.recommendations;
    let totals = compute_stats(meta, recs, &HashSet::new());
    let chosen = compute_stats(meta, recs, &model.selected);

    let mut list = div()
        .id("recommendations-list")
        .size_full()
        .overflow_y_scroll()
        .track_scroll(&model.panel.scroll)
        .pb(px(FADE));
    if recs.is_empty() {
        list = list.child(
            div()
                .pt_10()
                .flex()
                .justify_center()
                .text_color(t.secondary)
                .child("Nothing to clean up in this scan."),
        );
    }
    let focused = model.focused_row.as_deref();
    let mut first = true;
    for (tier, total) in TIERS
        .into_iter()
        .zip([totals.safe.size, totals.review.size])
    {
        let rows: Vec<&Recommendation> = recs.iter().filter(|r| r.tier == tier).collect();
        if rows.is_empty() {
            continue;
        }
        list = list.child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .px_1()
                .pt(px(if first { 6. } else { 20. }))
                .pb_2()
                .child(tier_pill(&t, tier))
                .child(
                    div()
                        .text_color(t.secondary)
                        .font_features(theme::numeric())
                        .child(format_bytes(total)),
                ),
        );
        first = false;
        let mut previous_on = true;
        for r in rows {
            let on = focused == Some(r.row_id.as_str());
            // Hairlines separate rows, except next to the highlighted one.
            list = list.child(row(&t, r, &model.selected, on, !on && !previous_on, cx));
            previous_on = on;
        }
    }

    let fade = div()
        .absolute()
        .bottom_0()
        .left_0()
        .w_full()
        .h(px(FADE))
        .bg(linear_gradient(
            180.,
            linear_color_stop(t.content.opacity(0.), 0.),
            linear_color_stop(t.content, 1.),
        ));
    let list = div()
        .id("recommendations")
        .relative()
        .flex_1()
        .min_h_0()
        .track_focus(&model.list_focus)
        .key_context(keymap::LIST)
        .on_action(cx.listener(|m, _: &keymap::SelectDown, _, cx| m.move_row(true, cx)))
        .on_action(cx.listener(|m, _: &keymap::SelectUp, _, cx| m.move_row(false, cx)))
        .on_action(cx.listener(|m, _: &keymap::StageSelected, _, cx| m.stage_row(cx)))
        .on_action(cx.listener(|m, _: &keymap::OpenSelected, _, cx| m.open_row(cx)))
        .child(list)
        .child(fade);

    page.child(list)
        .child(footer(&t, chosen.selected, chosen.selected_rows, cx))
}

fn tier_label(tier: Tier) -> &'static str {
    match tier {
        Tier::Safe => "Safe to remove",
        Tier::Review => "Review first",
    }
}

pub(super) fn tier_pill(t: &Theme, tier: Tier) -> Div {
    let (color, fill) = match tier {
        Tier::Safe => (t.safe_text, t.safe_tint),
        Tier::Review => (t.review_text, t.review_tint),
    };
    div()
        .px_2()
        .py_0p5()
        .rounded_md()
        .text_size(px(12.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(color)
        .bg(fill)
        .child(tier_label(tier))
}

/// `/Users/x/.npm` -> `~/.npm` under the scan root.
pub(super) fn tilde(path: &str, root: &str) -> String {
    match path.strip_prefix(root) {
        Some(rest) if rest.is_empty() || rest.starts_with('/') => format!("~{rest}"),
        _ => path.to_owned(),
    }
}

struct Tip(SharedString);

impl Render for Tip {
    fn render(&mut self, window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        // Inverted colors: dark on light windows, light on dark ones.
        let t = theme::current(window);
        div()
            .max_w(px(480.))
            .px_2()
            .py_1()
            .rounded_md()
            .bg(t.label)
            .text_color(t.widget)
            .text_size(px(12.))
            .child(self.0.clone())
    }
}

pub(super) fn tip(text: String) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    let text = SharedString::from(text);
    move |_, cx| cx.new(|_| Tip(text.clone())).into()
}

/// The accessible checked state of a checkbox.
pub(super) fn toggled(state: RowState) -> Toggled {
    match state {
        RowState::All => Toggled::True,
        RowState::Some => Toggled::Mixed,
        RowState::None => Toggled::False,
    }
}

/// Checkbox drawn with divs: filled with a check when all, a bar when some.
pub(super) fn checkbox(
    t: &Theme,
    id: impl Into<ElementId>,
    state: RowState,
    disabled: bool,
    size: f32,
) -> Stateful<Div> {
    let on = state != RowState::None;
    let mut b = div()
        .id(id)
        .flex_none()
        .size(px(size))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(4.))
        .border_1()
        .border_color(if on { t.accent } else { t.neutral_border })
        .bg(if on { t.accent } else { t.content })
        .text_color(gpui::white())
        .text_size(px(size - 4.))
        .font_weight(FontWeight::SEMIBOLD);
    b = match state {
        RowState::All => b.child("✓"),
        RowState::Some => b.child(
            div()
                .w(px(size - 8.))
                .h(px(2.))
                .rounded_full()
                .bg(gpui::white()),
        ),
        RowState::None => b,
    };
    if disabled {
        b.opacity(0.4)
    } else {
        b.cursor_pointer()
    }
}

fn row(
    t: &Theme,
    r: &Recommendation,
    selected: &HashSet<String>,
    on: bool,
    hairline: bool,
    cx: &mut Context<AppModel>,
) -> impl IntoElement {
    let reason = blocked_reason(r);
    let state = row_state(r, selected);
    let paths = r
        .items
        .iter()
        .map(|i| i.path.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let toggle_id = r.row_id.clone();
    let open_id = r.row_id.clone();

    let mut check = checkbox(
        t,
        SharedString::from(format!("check-{}", r.row_id)),
        state,
        reason.is_some(),
        16.,
    );
    if reason.is_none() {
        check = theme::focusable(check, t, Role::CheckBox, format!("Select {}", r.label))
            .aria_toggled(toggled(state))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                // The checkbox stages; only the rest of the row selects it (FR-REC-2).
                cx.stop_propagation();
                this.toggle_row(&toggle_id, cx);
            }));
    }

    let line = theme::focusable(
        div().id(SharedString::from(format!("row-{}", r.row_id))),
        t,
        Role::Button,
        format!("{}, {}", r.label, format_size(r.size, r.size_is_estimate)),
    )
    .flex()
    .items_center()
    .gap_3()
    .h(px(ROW_HEIGHT))
    .px(px(10.))
    .rounded(px(theme::R_CONTROL))
    .cursor_pointer()
    .when(on, |d| d.bg(t.selected))
    .child(check)
    .child(
        div()
            .flex_1()
            .min_w_0()
            .child(
                div()
                    .truncate()
                    .text_color(if reason.is_some() {
                        t.secondary
                    } else {
                        t.label
                    })
                    .child(r.label.clone()),
            )
            .child(
                div()
                    .id(SharedString::from(format!("paths-{}", r.row_id)))
                    .truncate()
                    .font_family(MONO)
                    .text_size(px(theme::T_SMALL))
                    .text_color(t.secondary)
                    .tooltip(tip(paths))
                    .child(r.paths_hint.clone()),
            ),
    )
    .children(reason.map(|reason| {
        div()
            .flex_none()
            .text_size(px(12.))
            .text_color(t.review_text)
            .child(reason.to_owned())
    }))
    .child(
        div()
            .flex_none()
            .w(px(84.))
            .flex()
            .justify_end()
            .font_weight(FontWeight::MEDIUM)
            .child(format_size(r.size, r.size_is_estimate)),
    )
    .child(
        svg()
            .path("icons/ui/chev.svg")
            .size(px(14.))
            .flex_none()
            .text_color(t.tertiary),
    )
    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
        // Clicking a row hands the keyboard to the list.
        window.focus(&this.list_focus, cx);
        this.focus_row(Some(open_id.clone()), cx);
        this.inspector_visible = true;
    }));
    // The hairline sits outside the rounded row so it stays straight.
    div()
        .border_t_1()
        .border_color(if hairline {
            t.separator
        } else {
            gpui::transparent_black()
        })
        .child(line)
}

/// The queue summary and Select Safe Items (FR-REC-4).
fn footer(t: &Theme, size: u64, rows: usize, cx: &mut Context<AppModel>) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .justify_between()
        .pt_1()
        .child(
            div()
                .text_color(t.secondary)
                .font_features(theme::numeric())
                .child(format!(
                    "{} staged · {rows} {}",
                    format_bytes(size),
                    if rows == 1 { "item" } else { "items" }
                )),
        )
        .child(
            Button::new("select-safe")
                .label("Select Safe Items")
                .with_size(Size::Size(px(CONTROL_HEIGHT)))
                .on_click(cx.listener(|m, _, _, cx| m.select_safe(cx))),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(row_id: &str, tier: Tier) -> Recommendation {
        Recommendation {
            row_id: row_id.into(),
            rule_id: row_id.into(),
            label: row_id.into(),
            paths_hint: String::new(),
            tier,
            size: 1,
            size_is_estimate: false,
            items: Vec::new(),
        }
    }

    #[test]
    fn ordered_puts_safe_rows_first() {
        let recs = [rec("r", Tier::Review), rec("s", Tier::Safe)];
        let ids: Vec<&str> = ordered(&recs).iter().map(|r| r.row_id.as_str()).collect();
        assert_eq!(ids, ["s", "r"]);
    }

    #[test]
    fn list_index_counts_both_tier_headers() {
        let recs = [rec("r", Tier::Review), rec("s", Tier::Safe)];
        assert_eq!(list_index(&recs, "r"), Some(3));
    }

    #[test]
    fn list_index_skips_an_empty_tier() {
        let recs = [rec("r", Tier::Review)];
        assert_eq!(list_index(&recs, "r"), Some(1));
    }

    #[test]
    fn tilde_only_replaces_the_scan_root_prefix() {
        assert_eq!(tilde("/Users/x/.npm", "/Users/x"), "~/.npm");
        assert_eq!(tilde("/Users/x", "/Users/x"), "~");
        assert_eq!(tilde("/Users/xy/.npm", "/Users/x"), "/Users/xy/.npm");
        assert_eq!(tilde("/opt/tool", "/Users/x"), "/opt/tool");
    }
}
