//! The four stat cards (PRD section 7.2).

use gpui::{Context, Div, FontWeight, Hsla, Window, div, prelude::*, px};

use super::format::format_bytes;
use super::model::AppModel;
use super::stats::compute_stats;
use super::theme::{self, Theme};

fn items(n: usize) -> String {
    format!("{n} {}", if n == 1 { "item" } else { "items" })
}

/// Label, optional status dot, value, and secondary text (`green` for the delta).
fn card(
    t: &Theme,
    label: &'static str,
    dot: Option<Hsla>,
    value: String,
    secondary: Option<(String, bool)>,
) -> Div {
    let mut head = div()
        .flex()
        .items_center()
        .gap(px(6.))
        .text_size(px(13.))
        .text_color(t.secondary);
    if let Some(color) = dot {
        head = head.child(div().size(px(6.)).rounded_full().bg(color));
    }
    head = head.child(label);
    let mut row = div().mt(px(6.)).flex().items_baseline().gap_2p5().child(
        div()
            .font_family(theme::MONO)
            .text_size(px(30.))
            .line_height(px(36.))
            .font_weight(FontWeight::SEMIBOLD)
            .child(value),
    );
    if let Some((text, green)) = secondary {
        row = row.child(
            div()
                .font_family(theme::MONO)
                .text_size(px(13.))
                .text_color(if green { t.safe_text } else { t.secondary })
                .child(text),
        );
    }
    div().flex_1().px_6().py_5().child(head).child(row)
}

pub fn render(
    model: &mut AppModel,
    window: &mut Window,
    _cx: &mut Context<AppModel>,
) -> impl IntoElement {
    let t = theme::current(window);
    let Some(meta) = &model.meta else {
        return div();
    };
    let s = compute_stats(meta, &model.recommendations, &model.selected);
    let cards = [
        card(
            &t,
            "Free after cleanup",
            None,
            format_bytes(s.free_after_cleanup),
            (s.selected > 0).then(|| (format!("+{}", format_bytes(s.selected)), true)),
        ),
        card(
            &t,
            "Safe to remove",
            Some(t.safe),
            format_bytes(s.safe.size),
            Some((items(s.safe.count), false)),
        ),
        card(
            &t,
            "Needs review",
            Some(t.review),
            format_bytes(s.review.size),
            Some((items(s.review.count), false)),
        ),
        card(
            &t,
            "Not scanned",
            None,
            format_bytes(s.not_scanned),
            Some(("system & apps".into(), false)),
        ),
    ];
    let mut row = div()
        .flex()
        .rounded(px(12.))
        .border_1()
        .border_color(t.separator)
        .bg(t.widget);
    for (i, c) in cards.into_iter().enumerate() {
        // `divide-x`: a 1 px divider on the left of every card but the first.
        row = row.child(if i > 0 {
            c.border_l_1().border_color(t.separator)
        } else {
            c
        });
    }
    row
}
