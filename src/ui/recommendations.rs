//! Recommendations card: tier sections, rows, footer and the detail drawer
//! (owner: B3). Port of `legacy-web/src/components/Recommendations/*`.

use std::collections::HashSet;
use std::time::Duration;

use gpui::{
    Animation, AnimationExt as _, AnyView, App, ClickEvent, Context, Div, ElementId, FontWeight,
    SharedString, Stateful, Window, div, ease_out_quint, prelude::*, px, rgb,
};

use super::format::{format_bytes, format_size};
use super::model::AppModel;
use super::selection::{RowState, blocked_reason, row_state};
use super::stats::compute_stats;
use super::theme::{self, MONO, Theme};
use crate::recommend::model::{ItemDetails, ItemStatus, MethodView, RecItem, Recommendation, Tier};

/// Panel-only UI state. The panel keeps none: selection, focus and details
/// live in the model.
#[derive(Default)]
pub struct PanelState {}

const DISABLED_HINT: &str = "Available once the cleanup engine ships";

pub fn render(
    model: &mut AppModel,
    window: &mut Window,
    cx: &mut Context<AppModel>,
) -> impl IntoElement {
    let t = theme::current(window);
    let card = div()
        .relative()
        .size_full()
        .flex()
        .flex_col()
        .overflow_hidden()
        .rounded(px(12.))
        .border_1()
        .border_color(t.card_border)
        .bg(t.card);
    let Some(meta) = &model.meta else {
        return card;
    };
    let recs = &model.recommendations;
    let totals = compute_stats(meta, recs, &HashSet::new());
    let chosen = compute_stats(meta, recs, &model.selected);

    let mut body = div()
        .id("recommendations-body")
        .flex_1()
        .min_h_0()
        .overflow_y_scroll()
        .border_t_1()
        .border_color(t.divider);
    if recs.is_empty() {
        body = body.child(
            div()
                .px_5()
                .py_8()
                .flex()
                .justify_center()
                .text_size(px(13.))
                .text_color(t.muted)
                .child("Nothing to clean up in this scan."),
        );
    }
    for (tier, total) in [
        (Tier::Safe, totals.safe.size),
        (Tier::Review, totals.review.size),
    ] {
        let rows: Vec<&Recommendation> = recs.iter().filter(|r| r.tier == tier).collect();
        if rows.is_empty() {
            continue;
        }
        let last = rows.len() - 1;
        body = body.child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .px_5()
                .py_2()
                .border_b_1()
                .border_color(t.divider)
                .child(tier_pill(&t, tier))
                .child(
                    div()
                        .font_family(MONO)
                        .text_size(px(13.))
                        .text_color(t.muted)
                        .child(format_bytes(total)),
                ),
        );
        for (i, r) in rows.into_iter().enumerate() {
            body = body.child(row(&t, r, &model.selected, i == last, cx));
        }
    }

    let card = card
        .child(
            div()
                .px_5()
                .pt_5()
                .pb_3()
                .child(
                    div()
                        .text_size(px(15.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Recommendations"),
                )
                .child(
                    div()
                        .mt_0p5()
                        .text_size(px(13.))
                        .text_color(t.muted)
                        .child("Select items to build a cleanup script"),
                ),
        )
        .child(body)
        .child(footer(&t, chosen.selected, chosen.selected_rows, cx));

    match model
        .focused_row
        .as_ref()
        .and_then(|id| recs.iter().find(|r| &r.row_id == id))
    {
        Some(r) => card.child(drawer(
            &t,
            r,
            model.details.as_ref().filter(|d| d.row_id == r.row_id),
            &model.selected,
            &meta.root,
            cx,
        )),
        None => card,
    }
}

fn tier_label(tier: Tier) -> &'static str {
    match tier {
        Tier::Safe => "Safe to remove",
        Tier::Review => "Review first",
    }
}

fn tier_pill(t: &Theme, tier: Tier) -> Div {
    let (color, fill) = match tier {
        Tier::Safe => (t.safe, t.safe_fill),
        Tier::Review => (t.review, t.review_fill),
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

/// Plain-words removal method (DetailDrawer.tsx `methodWords`).
fn method_words(m: &MethodView) -> String {
    match m {
        MethodView::Delete => "Deleted permanently".into(),
        MethodView::Trash => "Moved to Trash".into(),
        MethodView::Command { display } => format!("Runs `{display}`"),
    }
}

/// `/Users/x/.npm` -> `~/.npm` under the scan root.
fn tilde(path: &str, root: &str) -> String {
    match path.strip_prefix(root) {
        Some(rest) if rest.is_empty() || rest.starts_with('/') => format!("~{rest}"),
        _ => path.to_owned(),
    }
}

fn blocked_of(item: &RecItem) -> Option<&str> {
    match &item.status {
        ItemStatus::Blocked { reason } => Some(reason),
        ItemStatus::Ready => None,
    }
}

struct Tip(SharedString);

impl Render for Tip {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .max_w(px(480.))
            .px_2()
            .py_1()
            .rounded_md()
            .bg(rgb(0x18181b))
            .text_color(rgb(0xffffff))
            .text_size(px(12.))
            .child(self.0.clone())
    }
}

fn tip(text: String) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    let text = SharedString::from(text);
    move |_, cx| cx.new(|_| Tip(text.clone())).into()
}

/// Checkbox drawn with divs: filled with a check when all, a bar when some.
fn checkbox(
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
        .border_color(if on { t.text } else { t.muted })
        .bg(if on { t.text } else { t.card })
        .text_color(t.card)
        .text_size(px(size - 4.))
        .font_weight(FontWeight::SEMIBOLD);
    b = match state {
        RowState::All => b.child("✓"),
        RowState::Some => b.child(div().w(px(size - 8.)).h(px(2.)).rounded_full().bg(t.card)),
        RowState::None => b,
    };
    if disabled {
        b.opacity(0.4)
    } else {
        b.cursor_pointer()
    }
}

/// A small muted glyph or word button with a hover background.
fn ghost_button(t: &Theme, id: impl Into<ElementId>, label: &'static str) -> Stateful<Div> {
    let (bg, text) = (t.bg, t.text);
    div()
        .id(id)
        .flex_none()
        .px_1p5()
        .py_0p5()
        .rounded_md()
        .cursor_pointer()
        .text_color(t.muted)
        .hover(move |s| s.bg(bg).text_color(text))
        .child(label)
}

fn row(
    t: &Theme,
    r: &Recommendation,
    selected: &HashSet<String>,
    last: bool,
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
        18.,
    );
    if reason.is_none() {
        check = check.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
            this.toggle_row(&toggle_id, cx);
        }));
    }

    let sub = div()
        .id(SharedString::from(format!("paths-{}", r.row_id)))
        .truncate()
        .text_size(px(12.))
        .line_height(px(16.));
    let sub = match reason {
        Some(reason) => sub.text_color(t.review).child(reason.to_owned()),
        None => sub
            .font_family(MONO)
            .text_color(t.muted)
            .tooltip(tip(paths))
            .child(r.paths_hint.clone()),
    };

    let mut line = div()
        .flex()
        .items_center()
        .gap_3()
        .px_5()
        .py_3()
        .border_color(t.divider)
        .child(check)
        .child(
            div()
                .flex_1()
                .min_w_0()
                .child(
                    div()
                        .truncate()
                        .text_size(px(15.))
                        .line_height(px(20.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(if reason.is_some() { t.muted } else { t.text })
                        .child(r.label.clone()),
                )
                .child(sub),
        )
        .child(
            div()
                .flex_none()
                .font_family(MONO)
                .text_size(px(15.))
                .font_weight(FontWeight::MEDIUM)
                .child(format_size(r.size, r.size_is_estimate)),
        )
        .child(
            ghost_button(t, SharedString::from(format!("open-{}", r.row_id)), "›")
                .text_size(px(18.))
                .line_height(px(18.))
                .mr(px(-8.))
                .tooltip(tip(format!("Details for {}", r.label)))
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.focus_row(Some(open_id.clone()), cx);
                })),
        );
    if !last {
        line = line.border_b_1();
    }
    line
}

fn footer(t: &Theme, size: u64, rows: usize, cx: &mut Context<AppModel>) -> impl IntoElement {
    let secondary = |id: &'static str, label: &'static str| {
        div()
            .id(id)
            .px_3()
            .py_1p5()
            .rounded_lg()
            .border_1()
            .border_color(t.card_border)
            .text_size(px(13.))
            .font_weight(FontWeight::MEDIUM)
            .child(label)
    };
    let bg = t.bg;
    div()
        .px_5()
        .py_4()
        .border_t_1()
        .border_color(t.divider)
        .child(
            div()
                .flex()
                .items_center()
                .gap_1()
                .text_size(px(13.))
                .text_color(t.muted)
                .child(
                    div()
                        .font_family(MONO)
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(t.text)
                        .child(format_bytes(size)),
                )
                .child(format!(
                    "selected · {rows} {}",
                    if rows == 1 { "item" } else { "items" }
                )),
        )
        .child(
            div()
                .mt_3()
                .flex()
                .gap_2()
                .child(
                    secondary("select-safe", "Select safe")
                        .cursor_pointer()
                        .hover(move |s| s.bg(bg))
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.select_safe(cx))),
                )
                // Script export and cleanup arrive with the cleanup engine (Phase 4).
                .child(
                    secondary("copy-script", "Copy script")
                        .opacity(0.4)
                        .tooltip(tip(DISABLED_HINT.into())),
                )
                .child(
                    div()
                        .id("clean-selected")
                        .ml_auto()
                        .px_3()
                        .py_1p5()
                        .rounded_lg()
                        .bg(t.text)
                        .text_color(t.card)
                        .text_size(px(13.))
                        .font_weight(FontWeight::MEDIUM)
                        .opacity(0.4)
                        .tooltip(tip(DISABLED_HINT.into()))
                        .child("Clean selected"),
                ),
        )
}

fn section(t: &Theme, title: &'static str, body: impl IntoElement) -> impl IntoElement {
    div()
        .mt_5()
        .child(
            div()
                .text_size(px(12.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(t.muted)
                .child(title.to_uppercase()),
        )
        .child(
            div()
                .mt_1p5()
                .text_size(px(13.))
                .line_height(px(20.))
                .child(body),
        )
}

/// Slides over the recommendations card (PRD section 7.5).
fn drawer(
    t: &Theme,
    r: &Recommendation,
    details: Option<&ItemDetails>,
    selected: &HashSet<String>,
    root: &str,
    cx: &mut Context<AppModel>,
) -> impl IntoElement {
    let mut methods: Vec<String> = Vec::new();
    for i in &r.items {
        let m = method_words(&i.method);
        if !methods.contains(&m) {
            methods.push(m);
        }
    }

    let mut list = div().flex().flex_col();
    for i in &r.items {
        list = list.child(item(t, r, i, selected, root, cx));
    }
    let mut content = div()
        .id("drawer-body")
        .flex_1()
        .min_h_0()
        .overflow_y_scroll()
        .px_5()
        .pb_5();
    content = match details {
        Some(d) => content
            .child(section(t, "What it is", d.description.clone()))
            .child(section(t, "What happens if removed", d.regenerates.clone())),
        None => content.child(
            div()
                .mt_4()
                .text_size(px(13.))
                .text_color(t.muted)
                .child("Loading details…"),
        ),
    };
    content = content
        .child(section(
            t,
            "How it is removed",
            div().children(methods.into_iter().map(|m| div().child(m))),
        ))
        .child(section(t, "Items", list));
    if let Some(d) = details.filter(|d| !d.largest_subfolders.is_empty()) {
        content = content.child(section(
            t,
            "Largest subfolders",
            div().children(d.largest_subfolders.iter().map(|f| {
                div()
                    .flex()
                    .gap_2()
                    .py_0p5()
                    .text_size(px(12.))
                    .child(
                        div()
                            .id(SharedString::from(format!("sub-{}", f.path)))
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .font_family(MONO)
                            .text_color(t.muted)
                            .tooltip(tip(f.path.clone()))
                            .child(tilde(&f.path, root)),
                    )
                    .child(
                        div()
                            .flex_none()
                            .font_family(MONO)
                            .child(format_bytes(f.size)),
                    )
            })),
        ));
    }

    div()
        .absolute()
        .top_0()
        .bottom_0()
        .w_full()
        .flex()
        .flex_col()
        .bg(t.card)
        .occlude()
        .child(
            div()
                .flex()
                .items_start()
                .gap_2()
                .px_5()
                .pt_5()
                .pb_4()
                .border_b_1()
                .border_color(t.divider)
                .child(
                    ghost_button(t, "drawer-close", "←")
                        .text_size(px(16.))
                        .line_height(px(18.))
                        .ml(px(-6.))
                        .tooltip(tip("Back to recommendations".into()))
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.focus_row(None, cx);
                        })),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .child(
                            div()
                                .text_size(px(15.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(r.label.clone()),
                        )
                        .child(
                            div()
                                .mt_1()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(tier_pill(t, r.tier))
                                .child(
                                    div()
                                        .font_family(MONO)
                                        .text_size(px(13.))
                                        .child(format_size(r.size, r.size_is_estimate)),
                                ),
                        ),
                ),
        )
        .child(content)
        .with_animation(
            SharedString::from(format!("drawer-slide-{}", r.row_id)),
            Animation::new(Duration::from_millis(220)).with_easing(ease_out_quint()),
            |el, delta| el.left(px((1.0 - delta) * 56.)).opacity(delta),
        )
}

fn item(
    t: &Theme,
    r: &Recommendation,
    i: &RecItem,
    selected: &HashSet<String>,
    root: &str,
    cx: &mut Context<AppModel>,
) -> impl IntoElement {
    let blocked = blocked_of(i);
    let state = if selected.contains(&i.item_id) {
        RowState::All
    } else {
        RowState::None
    };
    let mut check = checkbox(
        t,
        SharedString::from(format!("item-{}", i.item_id)),
        state,
        blocked.is_some(),
        16.,
    );
    if blocked.is_none() {
        let id = i.item_id.clone();
        check = check.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
            this.toggle_item(&id, cx);
        }));
    }
    let path = i.path.clone();
    let sub = div()
        .id(SharedString::from(format!("item-path-{}", i.item_id)))
        .truncate()
        .text_size(px(12.))
        .tooltip(tip(i.path.clone()));
    let sub = match blocked {
        Some(reason) => sub.text_color(t.review).child(reason.to_owned()),
        None => sub
            .font_family(MONO)
            .text_color(t.muted)
            .child(tilde(&i.path, root)),
    };
    div()
        .flex()
        .items_center()
        .gap_2p5()
        .py_1p5()
        .child(check)
        .child(
            div()
                .flex_1()
                .min_w_0()
                .child(div().truncate().child(i.name.clone()))
                .child(sub),
        )
        .child(
            div()
                .flex_none()
                .font_family(MONO)
                .text_size(px(13.))
                .child(format_size(i.size, r.size_is_estimate)),
        )
        .child(
            ghost_button(
                t,
                SharedString::from(format!("reveal-{}", i.item_id)),
                "Reveal",
            )
            .text_size(px(12.))
            .tooltip(tip("Reveal in Finder".into()))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.reveal(path.clone(), cx);
            })),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_words_read_plainly() {
        assert_eq!(method_words(&MethodView::Delete), "Deleted permanently");
        assert_eq!(method_words(&MethodView::Trash), "Moved to Trash");
        assert_eq!(
            method_words(&MethodView::Command {
                display: "brew cleanup".into()
            }),
            "Runs `brew cleanup`"
        );
    }

    #[test]
    fn tilde_only_replaces_the_scan_root_prefix() {
        assert_eq!(tilde("/Users/x/.npm", "/Users/x"), "~/.npm");
        assert_eq!(tilde("/Users/x", "/Users/x"), "~");
        assert_eq!(tilde("/Users/xy/.npm", "/Users/x"), "/Users/xy/.npm");
        assert_eq!(tilde("/opt/tool", "/Users/x"), "/opt/tool");
    }
}
