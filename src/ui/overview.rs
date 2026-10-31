//! Overview page (PRD 13.4.4): Disk, Safe to clean, Worth a look, Largest folders.

use gpui::{
    Context, Div, FontWeight, Hsla, Role, SharedString, Stateful, Window, div, prelude::*, px,
    relative, svg,
};
use gpui_component::button::Button;
use gpui_component::{Disableable, Sizable, Size};

use super::format::format_bytes;
use super::model::{AppModel, Page};
use super::shell::CONTROL_HEIGHT;
use super::stats::{Segment, Stats, TierStat, capacity_segments, compute_stats};
use super::theme::{self, Theme};
use crate::view::ViewNode;

/// Purgeable totals below this are noise and stay hidden.
pub(super) const PURGEABLE_MIN: u64 = 1_000_000_000;
const SAFE_LINE: &str = "Caches and build output that rebuild on their own";
const REVIEW_LINE: &str = "Models, SDKs and backups you may still need";

pub fn render(
    model: &mut AppModel,
    window: &mut Window,
    cx: &mut Context<AppModel>,
) -> impl IntoElement {
    let t = theme::current(window);
    let Some(meta) = &model.meta else {
        return div();
    };
    let s = compute_stats(meta, &model.recommendations, &model.selected);
    let name = volume_name(&meta.volume.name);
    let total = meta.volume.total;

    let safe_button = Button::new("select-safe")
        .label("Select Safe Items")
        .with_size(Size::Size(px(CONTROL_HEIGHT)))
        .disabled(s.safe.items == 0)
        .on_click(cx.listener(|m, _, _, cx| m.select_safe(cx)));
    let review_button = Button::new("review")
        .label("Review")
        .with_size(Size::Size(px(CONTROL_HEIGHT)))
        .on_click(cx.listener(|m, _, _, cx| m.set_page(Page::Recommended, cx)));

    div()
        .grid()
        .grid_cols(3)
        .gap(px(theme::GAP_CONTENT))
        .child(
            disk(&t, name, total, &s, model)
                .col_span(2)
                .aria_label(disk_sentence(name, total, &s)),
        )
        .child(
            tier_widget(&t, "Safe to clean", t.safe_text, s.safe, SAFE_LINE)
                .aria_label(tier_sentence("Safe to clean", s.safe, SAFE_LINE))
                .child(div().mt_auto().flex().child(safe_button)),
        )
        .child(
            tier_widget(&t, "Worth a look", t.review_text, s.review, REVIEW_LINE)
                .aria_label(tier_sentence("Worth a look", s.review, REVIEW_LINE))
                .child(div().mt_auto().flex().child(review_button)),
        )
        .child(
            largest(&t, &model.largest_folders, cx)
                .col_span(2)
                .aria_label(largest_sentence(&model.largest_folders)),
        )
}

fn volume_name(name: &str) -> &str {
    if name.is_empty() { "Disk" } else { name }
}

fn items(n: usize) -> String {
    format!("{n} {}", if n == 1 { "item" } else { "items" })
}

fn widget(t: &Theme, id: &'static str, title: impl Into<SharedString>) -> Stateful<Div> {
    div()
        .id(id)
        .role(Role::Group)
        .flex()
        .flex_col()
        .gap(px(theme::GAP_WIDGET))
        .p(px(16.))
        .rounded(px(theme::R_WIDGET))
        .bg(t.widget)
        .text_size(px(theme::T_BODY))
        .child(
            div()
                .text_size(px(theme::T_HEAD))
                .font_weight(FontWeight::SEMIBOLD)
                .child(title.into()),
        )
}

fn big(text: String, color: Hsla) -> Div {
    div()
        .text_size(px(theme::T_LARGE))
        .line_height(px(31.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(color)
        .child(text)
}

fn disk(t: &Theme, name: &str, total: u64, s: &Stats, model: &AppModel) -> Stateful<Div> {
    let segments = capacity_segments(s, &model.category_totals);
    let sum: u64 = segments.iter().map(|(_, b)| b).sum();
    let color = |seg: &Segment| match seg {
        Segment::Category(id) => theme::category_colors(t, id).1,
        Segment::System => t.neutral_border,
        Segment::Free => t.free,
    };
    let bar = div()
        .flex()
        .gap(px(2.))
        .h(px(14.))
        .rounded(px(7.))
        .overflow_hidden()
        .children(segments.iter().map(|(seg, bytes)| {
            div()
                .h_full()
                .flex_basis(relative(*bytes as f32 / sum.max(1) as f32))
                .bg(color(seg))
        }));
    let dot = |c: Hsla, label: &'static str| {
        div()
            .flex()
            .items_center()
            .gap(px(6.))
            .child(div().size(px(8.)).rounded_full().bg(c))
            .child(label)
    };
    let legend = div()
        .flex()
        .flex_wrap()
        .gap_x_4()
        .gap_y(px(6.))
        .text_size(px(theme::T_SMALL))
        .text_color(t.secondary)
        .children(
            theme::LEGEND_ORDER
                .iter()
                .map(|id| dot(theme::category_colors(t, id).1, theme::category_label(id))),
        )
        .child(dot(t.neutral_border, "System and apps"))
        .child(dot(t.free, "Free"));

    widget(t, "disk", name.to_owned())
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .flex()
                        .items_baseline()
                        .gap(px(10.))
                        .child(big(format_bytes(s.available), t.label))
                        .child(
                            div()
                                .text_color(t.secondary)
                                .child(format!("available of {}", format_bytes(total))),
                        ),
                )
                .when(s.selected > 0, |d| {
                    d.child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(t.safe_text)
                            .child(format!("+{} after cleanup", format_bytes(s.selected))),
                    )
                }),
        )
        .child(bar)
        .child(legend)
        .when(s.purgeable >= PURGEABLE_MIN, |d| {
            d.child(
                div()
                    .text_size(px(theme::T_SMALL))
                    .text_color(t.secondary)
                    .child(purgeable_line(s.purgeable)),
            )
        })
}

fn tier_widget(
    t: &Theme,
    title: &'static str,
    color: Hsla,
    stat: TierStat,
    line: &'static str,
) -> Stateful<Div> {
    widget(t, title, title)
        .child(big(format_bytes(stat.size), color))
        .child(
            div()
                .mt(px(-6.))
                .text_color(t.secondary)
                .child(format!("{} · {line}", items(stat.items))),
        )
}

fn largest(t: &Theme, folders: &[ViewNode], cx: &mut Context<AppModel>) -> Stateful<Div> {
    let max = folders.first().map_or(1, |f| f.size.max(1));
    let rows = folders.iter().map(|f| {
        let size = format_bytes(f.size);
        let path = f.path.clone();
        theme::focusable(
            div().id(SharedString::from(format!("largest-{}", f.path))),
            t,
            Role::Button,
            format!("{}, {size}, open in Explore", f.name),
        )
        .flex()
        .items_center()
        .gap(px(12.))
        .rounded(px(theme::R_CONTROL))
        .cursor_pointer()
        .child(
            div()
                .w(px(140.))
                .flex_none()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    svg()
                        .path("icons/ui/folder.svg")
                        .size(px(14.))
                        .flex_none()
                        .text_color(t.secondary),
                )
                .child(div().min_w_0().truncate().child(f.name.clone())),
        )
        .child(
            div()
                .flex_1()
                .h(px(8.))
                .rounded(px(4.))
                .bg(t.content)
                .overflow_hidden()
                .child(
                    div()
                        .h_full()
                        .rounded(px(4.))
                        .w(relative(f.size as f32 / max as f32))
                        .bg(theme::category_colors(t, &f.category).1),
                ),
        )
        .child(
            div()
                .w(px(70.))
                .flex_none()
                .flex()
                .justify_end()
                .text_color(t.secondary)
                .child(size),
        )
        .on_click(cx.listener(move |m, _, _, cx| m.explore(path.clone(), cx)))
    });
    widget(t, "largest", "Largest folders").child(
        div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .children(rows)
            .when(folders.is_empty(), |d| {
                d.child(div().text_color(t.secondary).child("No folders yet"))
            }),
    )
}

fn purgeable_line(purgeable: u64) -> String {
    format!(
        "Up to {} of this is purgeable, mostly local Time Machine snapshots.",
        format_bytes(purgeable)
    )
}

/// VoiceOver reads each widget as one sentence (PLAN.md Phase 9).
fn disk_sentence(name: &str, total: u64, s: &Stats) -> String {
    let mut out = format!(
        "{name}: {} available of {}.",
        format_bytes(s.available),
        format_bytes(total)
    );
    if s.selected > 0 {
        out += &format!(" {} more after cleanup.", format_bytes(s.selected));
    }
    if s.purgeable >= PURGEABLE_MIN {
        out += " ";
        out += &purgeable_line(s.purgeable);
    }
    out
}

fn tier_sentence(title: &str, stat: TierStat, line: &str) -> String {
    format!(
        "{title}: {} in {}. {line}.",
        format_bytes(stat.size),
        items(stat.items)
    )
}

fn largest_sentence(folders: &[ViewNode]) -> String {
    if folders.is_empty() {
        return "Largest folders: none yet.".to_owned();
    }
    let list: Vec<String> = folders
        .iter()
        .map(|f| format!("{} {}", f.name, format_bytes(f.size)))
        .collect();
    format!("Largest folders: {}.", list.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view::ViewKind;

    const GB: u64 = 1_000_000_000;

    fn stats(selected: u64, purgeable: u64) -> Stats {
        Stats {
            available: 39_500_000_000,
            purgeable,
            selected,
            ..Default::default()
        }
    }

    fn folder(name: &str, size: u64) -> ViewNode {
        ViewNode {
            id: name.into(),
            name: name.into(),
            path: format!("~/{name}"),
            size,
            category: "dev_tools".into(),
            tier: None,
            rule_id: None,
            kind: ViewKind::Dir,
            file_count: 0,
            cleanable: 0,
            system_managed: false,
            children: Vec::new(),
        }
    }

    #[test]
    fn disk_sentence_reads_free_space_delta_and_purgeable() {
        let s = stats(52_500_000_000, 4_200_000_000);
        assert_eq!(
            disk_sentence("Macintosh HD", 462_500_000_000, &s),
            "Macintosh HD: 39.5 GB available of 463 GB. 52.5 GB more after cleanup. \
             Up to 4.2 GB of this is purgeable, mostly local Time Machine snapshots."
        );
    }

    #[test]
    fn disk_sentence_leaves_out_hidden_lines() {
        assert_eq!(
            disk_sentence("Macintosh HD", 462_500_000_000, &stats(0, GB - 1)),
            "Macintosh HD: 39.5 GB available of 463 GB."
        );
    }

    #[test]
    fn tier_sentence_reads_total_count_and_line() {
        let stat = TierStat {
            size: 41_700_000_000,
            items: 8,
        };
        assert_eq!(
            tier_sentence("Safe to clean", stat, SAFE_LINE),
            "Safe to clean: 41.7 GB in 8 items. Caches and build output that rebuild on their own."
        );
    }

    #[test]
    fn largest_sentence_lists_each_folder() {
        let folders = [
            folder("Desktop", 41_500_000_000),
            folder("Library", 17_100_000_000),
        ];
        assert_eq!(
            largest_sentence(&folders),
            "Largest folders: Desktop 41.5 GB, Library 17.1 GB."
        );
    }

    #[test]
    fn unnamed_volume_reads_as_disk() {
        assert_eq!(volume_name(""), "Disk");
    }
}
