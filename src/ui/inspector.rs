//! Inspector (PRD 13.4.7): the trailing panel that describes what is selected:
//! on Explore a folder, a file or the smaller-items list, on Recommended a row.

use gpui::{
    AnyElement, ClickEvent, Context, FontWeight, Hsla, Role, SharedString, div, prelude::*, px, svg,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::{Disableable, Sizable, Size};

use super::chrome::INSPECTOR_WIDTH;
use super::format::{format_bytes, format_size, group_digits};
use super::model::{AppModel, Page};
use super::recommendations::{checkbox, tier_pill, tilde, tip, toggled};
use super::selection::{self, RowState, within};
use super::shell::{self, CONTROL_HEIGHT};
use super::stats::{compute_stats, largest_folders};
use super::theme::{self, Theme};
use crate::recommend::model::{ItemStatus, MethodView, RecItem, Recommendation};
use crate::rules::model::Rules;
use crate::view::{ViewKind, ViewNode};

pub fn render(model: &AppModel, t: Theme, cx: &mut Context<AppModel>) -> impl IntoElement {
    let panel = div()
        .id("inspector")
        .w(px(INSPECTOR_WIDTH))
        .flex_none()
        .border_l_1()
        .border_color(t.separator)
        .text_size(px(theme::T_BODY))
        .text_color(t.label);
    let body = match model.page {
        _ if model.queue_open => Some(queue(model, t, cx)),
        Page::Recommended => model
            .focused_row
            .as_deref()
            .and_then(|id| model.recommendations.iter().find(|r| r.row_id == id))
            .map(|r| recommendation(model, r, t, cx)),
        _ => model.tile_node().map(|node| match node.kind {
            ViewKind::Smaller => smaller(node, t, cx),
            _ => item(model, node, t, cx),
        }),
    };
    let Some(body) = body else {
        return panel
            .flex()
            .items_center()
            .justify_center()
            .text_color(t.secondary)
            .child("Nothing selected")
            .into_any_element();
    };
    panel
        .flex()
        .flex_col()
        .gap(px(14.))
        .px_4()
        .pt_1()
        .pb_4()
        .overflow_y_scroll()
        .child(body)
        .into_any_element()
}

/// `path` with the scan root shown as `~`.
pub(super) fn display_path(root: &str, path: &str) -> String {
    if path == root {
        return "~".to_owned();
    }
    match path.strip_prefix(root).and_then(|r| r.strip_prefix('/')) {
        Some(rest) if root != "~" => format!("~/{rest}"),
        _ => path.to_owned(),
    }
}

fn header(icon: &str, color: Hsla, name: &str, path: String, t: Theme) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap(px(10.))
        .child(
            svg()
                .path(SharedString::from(format!("icons/ui/{icon}.svg")))
                .size(px(28.))
                .flex_none()
                .text_color(color),
        )
        .child(
            div()
                .min_w_0()
                .child(
                    div()
                        .text_size(px(17.))
                        .line_height(px(21.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .truncate()
                        .child(name.to_owned()),
                )
                .child(
                    div()
                        .font_family(theme::MONO)
                        .text_size(px(theme::T_SMALL))
                        .text_color(t.secondary)
                        .truncate()
                        .child(path),
                ),
        )
}

/// Label and value rows: an 82 pt label column, as in the mockup.
fn facts(rows: Vec<(&'static str, AnyElement)>, t: Theme) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_size(px(12.))
        .children(rows.into_iter().map(move |(label, value)| {
            div()
                .flex()
                .child(
                    div()
                        .w(px(82.))
                        .flex_none()
                        .text_color(t.secondary)
                        .child(label),
                )
                .child(div().min_w_0().font_features(theme::numeric()).child(value))
        }))
}

fn section(label: &'static str, t: Theme) -> gpui::Div {
    div().flex().flex_col().gap(px(6.)).child(
        div()
            .text_size(px(theme::T_SMALL))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(t.secondary)
            .child(label),
    )
}

fn text(s: impl Into<SharedString>, t: Theme) -> AnyElement {
    div()
        .text_size(px(theme::T_SMALL))
        .text_color(t.secondary)
        .child(s.into())
        .into_any_element()
}

/// A folder or a file.
fn item(model: &AppModel, node: &ViewNode, t: Theme, cx: &mut Context<AppModel>) -> AnyElement {
    let is_dir = node.kind == ViewKind::Dir;
    let (_, dot) = theme::category_colors(&t, &node.category);
    let rules = Rules::embedded();
    let rule = node.rule_id.as_deref().and_then(|id| rules.rule(id));

    let ready: Vec<_> = model
        .recommendations
        .iter()
        .flat_map(|r| &r.items)
        .filter(|i| i.status == ItemStatus::Ready && within(&i.path, &node.path))
        .collect();
    let mut rows = vec![
        ("Size", format_bytes(node.size).into_any_element()),
        (
            "Category",
            theme::category_label(&node.category).into_any_element(),
        ),
    ];
    if node.file_count > 0 {
        rows.insert(
            1,
            ("Files", group_digits(node.file_count).into_any_element()),
        );
    }
    if !ready.is_empty() {
        let bytes: u64 = ready.iter().map(|i| i.size).sum();
        let noun = if ready.len() == 1 { "item" } else { "items" };
        rows.push((
            "Cleanable",
            div()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(t.safe_text)
                .child(format!("{} in {} {noun}", format_bytes(bytes), ready.len()))
                .into_any_element(),
        ));
    }

    let about = match rule {
        Some(rule) => section("What it is", t)
            .child(text(rule.description.clone(), t))
            .child(
                div()
                    .pt_1()
                    .text_size(px(theme::T_SMALL))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(t.secondary)
                    .child("What happens if removed"),
            )
            .child(text(rule.regenerates.clone(), t)),
        None if node.system_managed => {
            section("About this folder", t).child(text("Managed by macOS or an app.", t))
        }
        None => section("About this category", t).child(text(
            rules
                .category(&node.category)
                .map(|c| c.description.clone())
                .unwrap_or_default(),
            t,
        )),
    };

    let subfolders = model
        .tile_view
        .as_ref()
        .filter(|v| v.path == node.path)
        .map(|v| largest_folders(v, 5))
        .unwrap_or_default();
    let max = subfolders.first().map_or(1, |f| f.size.max(1));
    let largest = (!subfolders.is_empty()).then(|| {
        section("Largest folders", t).children(subfolders.iter().map(|f| {
            div()
                .flex()
                .items_center()
                .gap_2()
                .text_size(px(12.))
                .child(
                    div()
                        .w(px(108.))
                        .flex_none()
                        .truncate()
                        .child(f.name.clone()),
                )
                .child(
                    div()
                        .flex_1()
                        .h(px(4.))
                        .rounded(px(2.))
                        .bg(t.widget)
                        .overflow_hidden()
                        .child(
                            div()
                                .h_full()
                                .rounded(px(2.))
                                .w(gpui::relative(f.size as f32 / max as f32))
                                .bg(theme::category_colors(&t, &f.category).1),
                        ),
                )
                .child(
                    div()
                        .min_w(px(56.))
                        .flex()
                        .justify_end()
                        .text_color(t.secondary)
                        .font_features(theme::numeric())
                        .child(format_bytes(f.size)),
                )
        }))
    });

    let inside: Vec<_> = model
        .recommendations
        .iter()
        .filter_map(|r| {
            let bytes: u64 = r
                .items
                .iter()
                .filter(|i| within(&i.path, &node.path))
                .map(|i| i.size)
                .sum();
            (bytes > 0).then(|| (r.row_id.clone(), r.label.clone(), bytes, r.size_is_estimate))
        })
        .collect();
    let recommended = (!inside.is_empty()).then(|| {
        section("Recommended inside", t).children(inside.into_iter().map(
            |(row_id, label, bytes, estimate)| {
                theme::focusable(
                    div().id(SharedString::from(format!("inside-{row_id}"))),
                    &t,
                    Role::Button,
                    format!("Show {label}"),
                )
                .flex()
                .items_center()
                .gap_2()
                .text_size(px(12.))
                .rounded(px(4.))
                .cursor_pointer()
                .hover(move |s| s.bg(t.hover))
                .child(div().flex_1().min_w_0().truncate().child(label))
                .child(
                    div()
                        .text_color(t.secondary)
                        .font_features(theme::numeric())
                        .child(format_size(bytes, estimate)),
                )
                .on_click(cx.listener(move |m, _, _, cx| m.show_row(row_id.clone(), cx)))
            },
        ))
    });

    let path = node.path.clone();
    let key = super::explorer::tile_key(node);
    let actions = div()
        .flex()
        .gap_2()
        .pt_2()
        .child(
            Button::new("inspector-reveal")
                .label("Reveal in Finder")
                .with_size(Size::Size(px(CONTROL_HEIGHT)))
                .on_click(cx.listener(move |m, _, _, cx| m.reveal(path.clone(), cx))),
        )
        .child(
            Button::new("inspector-stage")
                .label("Add to Queue")
                .with_size(Size::Size(px(CONTROL_HEIGHT)))
                .disabled(!model.can_stage(node))
                .on_click(cx.listener(move |m, _, _, cx| m.stage_tile(&key, cx))),
        );

    div()
        .flex()
        .flex_col()
        .gap(px(14.))
        .child(header(
            if is_dir { "folder" } else { "doc" },
            dot,
            &node.name,
            display_path(&model.root_path, &node.path),
            t,
        ))
        .child(facts(rows, t))
        .child(about)
        .children(largest)
        .children(recommended)
        .child(actions)
        .into_any_element()
}

/// The items merged into the smaller-items tile, largest first.
fn smaller(node: &ViewNode, t: Theme, cx: &mut Context<AppModel>) -> AnyElement {
    let rows = node.children.iter().map(|c| {
        let row = div()
            .flex()
            .items_center()
            .gap_2()
            .text_size(px(12.))
            .rounded(px(4.))
            .child(
                svg()
                    .path(if c.kind == ViewKind::Dir {
                        "icons/ui/folder.svg"
                    } else {
                        "icons/ui/doc.svg"
                    })
                    .size(px(14.))
                    .flex_none()
                    .text_color(t.secondary),
            )
            .child(div().flex_1().min_w_0().truncate().child(c.name.clone()))
            .child(
                div()
                    .text_color(t.secondary)
                    .font_features(theme::numeric())
                    .child(format_bytes(c.size)),
            );
        if c.kind != ViewKind::Dir {
            return row.into_any_element();
        }
        let path = c.path.clone();
        theme::focusable(
            div().id(SharedString::from(format!("smaller-{}", c.path))),
            &t,
            Role::Button,
            format!("Open {}", c.name),
        )
        .cursor_pointer()
        .hover(move |s| s.bg(t.hover))
        .child(row)
        .on_click(cx.listener(move |m, _, _, cx| m.navigate(path.clone(), cx)))
        .into_any_element()
    });
    div()
        .flex()
        .flex_col()
        .gap(px(14.))
        .child(
            div()
                .child(
                    div()
                        .text_size(px(17.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(node.name.clone()),
                )
                .child(
                    div()
                        .text_color(t.secondary)
                        .font_features(theme::numeric())
                        .child(format!("{} in all", format_bytes(node.size))),
                ),
        )
        .child(section("Items", t).children(rows))
        .into_any_element()
}

/// The staging queue (PRD FR-Q-3): staged items grouped by row, each removable,
/// then the actions on the whole queue.
fn queue(model: &AppModel, t: Theme, cx: &mut Context<AppModel>) -> AnyElement {
    let groups = selection::queue_groups(&model.recommendations, &model.selected);
    let staged = model.meta.as_ref().map_or(0, |meta| {
        compute_stats(meta, &model.recommendations, &model.selected).selected
    });
    let noun = if groups.len() == 1 { "row" } else { "rows" };
    let empty = groups.is_empty();

    let mut list = Vec::with_capacity(groups.len());
    for (r, items) in &groups {
        let method = items
            .first()
            .map(|i| method_words(&i.method))
            .unwrap_or_default();
        let mut group = section_with(r.label.clone(), method, t);
        for i in items {
            let id = i.item_id.clone();
            group = group.child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .h(px(26.))
                    .text_size(px(12.))
                    .child(
                        div()
                            .id(SharedString::from(format!("queued-{}", i.item_id)))
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .font_family(theme::MONO)
                            .tooltip(tip(i.path.clone()))
                            .child(tilde(&i.path, &model.root_path)),
                    )
                    .child(
                        div()
                            .flex_none()
                            .font_features(theme::numeric())
                            .child(format_size(i.size, r.size_is_estimate)),
                    )
                    .child(
                        Button::new(SharedString::from(format!("unqueue-{}", i.item_id)))
                            .ghost()
                            .with_size(Size::Size(px(20.)))
                            .icon(gpui_component::Icon::default().path("icons/ui/x.svg"))
                            .accessibility_label(format!("Remove {} from the queue", i.name))
                            .on_click(cx.listener(move |m, _, _, cx| m.toggle_item(&id, cx))),
                    ),
            );
        }
        list.push(group);
    }

    let actions = div()
        .mt_auto()
        .pt_2()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .flex()
                .gap_2()
                .child(
                    Button::new("queue-copy")
                        .label("Copy as Shell Script")
                        .flex_1()
                        .with_size(Size::Size(px(CONTROL_HEIGHT)))
                        .disabled(empty)
                        .on_click(cx.listener(|m, _, _, cx| m.copy_script(cx))),
                )
                .child(
                    Button::new("queue-export")
                        .label("Export…")
                        .flex_1()
                        .with_size(Size::Size(px(CONTROL_HEIGHT)))
                        .disabled(empty)
                        .on_click(cx.listener(|m, _, _, cx| m.export_script(cx))),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    Button::new("queue-clear")
                        .ghost()
                        .label("Clear Queue")
                        .with_size(Size::Size(px(CONTROL_HEIGHT)))
                        .disabled(empty)
                        .on_click(cx.listener(|m, _, _, cx| m.clear_selection(cx))),
                )
                .child(div().flex_1())
                .child(shell::clean_button("queue-clean", model, staged, cx)),
        );

    div()
        .flex()
        .flex_col()
        .gap(px(12.))
        .min_h_full()
        .child(
            div()
                .flex()
                .items_baseline()
                .justify_between()
                .child(
                    div()
                        .text_size(px(17.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Queue"),
                )
                .child(
                    div()
                        .text_color(t.secondary)
                        .font_features(theme::numeric())
                        .child(format!(
                            "{} {noun} · {}",
                            groups.len(),
                            format_bytes(staged)
                        )),
                ),
        )
        .when(empty, |d| {
            d.child(text(
                "Nothing staged. Check a row or press Space on a tile.",
                t,
            ))
        })
        .children(list)
        .child(actions)
        .into_any_element()
}

/// A section label with a second label on the right.
fn section_with(label: String, right: String, t: Theme) -> gpui::Div {
    div().flex().flex_col().gap(px(2.)).child(
        div()
            .flex()
            .justify_between()
            .gap_2()
            .text_size(px(theme::T_SMALL))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(t.secondary)
            .child(div().min_w_0().truncate().child(label))
            .child(div().flex_none().child(right)),
    )
}

/// Plain-words removal method (PRD 7.5).
fn method_words(m: &MethodView) -> String {
    match m {
        MethodView::Delete => "Deleted permanently".into(),
        MethodView::Trash => "Moved to Trash".into(),
        MethodView::Command { display } => format!("Runs `{display}`"),
    }
}

/// A recommendation row: the v1 drawer content (PRD FR-INS-3, section 7.5).
fn recommendation(
    model: &AppModel,
    r: &Recommendation,
    t: Theme,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let root = model.root_path.as_str();
    let details = model.details.as_ref().filter(|d| d.row_id == r.row_id);
    let mut methods: Vec<String> = Vec::new();
    for i in &r.items {
        let m = method_words(&i.method);
        if !methods.contains(&m) {
            methods.push(m);
        }
    }
    // One item gets the full-width button; several get one Reveal each.
    let single = r.items.len() == 1;

    let about = match details {
        Some(d) => vec![
            section("What it is", t).child(text(d.description.clone(), t)),
            section("What happens if removed", t).child(text(d.regenerates.clone(), t)),
        ],
        None => vec![div().child(text(
            match model.details_error.as_deref() {
                Some(e) => format!("Could not load details: {e}"),
                None => "Loading details…".into(),
            },
            t,
        ))],
    };
    let largest = details
        .filter(|d| !d.largest_subfolders.is_empty())
        .map(|d| {
            section("Largest subfolders", t).children(d.largest_subfolders.iter().map(|f| {
                div()
                    .flex()
                    .gap_2()
                    .text_size(px(12.))
                    .child(
                        div()
                            .id(SharedString::from(format!("sub-{}", f.path)))
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .font_family(theme::MONO)
                            .text_color(t.secondary)
                            .tooltip(tip(f.path.clone()))
                            .child(tilde(&f.path, root)),
                    )
                    .child(
                        div()
                            .flex_none()
                            .font_features(theme::numeric())
                            .child(format_bytes(f.size)),
                    )
            }))
        });
    let mut items = Vec::with_capacity(r.items.len());
    for i in &r.items {
        items.push(rec_item(model, r, i, single, t, cx));
    }
    let reveal = r.items.first().filter(|_| single).map(|i| {
        let path = i.path.clone();
        div().pt_2().child(
            Button::new("inspector-reveal")
                .label("Reveal in Finder")
                .with_size(Size::Size(px(CONTROL_HEIGHT)))
                .on_click(cx.listener(move |m, _, _, cx| m.reveal(path.clone(), cx))),
        )
    });

    div()
        .flex()
        .flex_col()
        .gap(px(14.))
        .child(
            div()
                .flex()
                .flex_col()
                .items_start()
                .gap_2()
                .child(tier_pill(&t, r.tier))
                .child(
                    div()
                        .text_size(px(17.))
                        .line_height(px(21.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(r.label.clone()),
                )
                .child(
                    div()
                        .text_color(t.secondary)
                        .font_features(theme::numeric())
                        .child(format_size(r.size, r.size_is_estimate)),
                ),
        )
        .children(about)
        .child(section("Method", t).children(methods.into_iter().map(|m| text(m, t))))
        .child(section("Items", t).children(items))
        .children(largest)
        .children(reveal)
        .into_any_element()
}

/// One item of a row, with its checkbox and blocked reason.
fn rec_item(
    model: &AppModel,
    r: &Recommendation,
    i: &RecItem,
    single: bool,
    t: Theme,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let blocked = match &i.status {
        ItemStatus::Blocked { reason } => Some(reason.as_str()),
        ItemStatus::Ready => None,
    };
    let state = if model.selected.contains(&i.item_id) {
        RowState::All
    } else {
        RowState::None
    };
    let mut check = checkbox(
        &t,
        SharedString::from(format!("item-{}", i.item_id)),
        state,
        blocked.is_some(),
        16.,
    );
    if blocked.is_none() {
        let id = i.item_id.clone();
        check = theme::focusable(check, &t, Role::CheckBox, format!("Select {}", i.name))
            .aria_toggled(toggled(state))
            .on_click(cx.listener(move |m, _: &ClickEvent, _, cx| m.toggle_item(&id, cx)));
    }
    let sub = div()
        .id(SharedString::from(format!("item-path-{}", i.item_id)))
        .truncate()
        .text_size(px(theme::T_SMALL))
        .tooltip(tip(i.path.clone()));
    let sub = match blocked {
        Some(reason) => sub.text_color(t.review_text).child(reason.to_owned()),
        None => sub
            .font_family(theme::MONO)
            .text_color(t.secondary)
            .child(tilde(&i.path, &model.root_path)),
    };
    let path = i.path.clone();
    let reveal = (!single).then(|| {
        theme::focusable(
            div().id(SharedString::from(format!("reveal-{}", i.item_id))),
            &t,
            Role::Button,
            format!("Reveal {} in Finder", i.name),
        )
        .flex_none()
        .text_size(px(theme::T_SMALL))
        .text_color(t.accent_text)
        .cursor_pointer()
        .child("Reveal")
        .on_click(cx.listener(move |m, _: &ClickEvent, _, cx| m.reveal(path.clone(), cx)))
    });
    div()
        .flex()
        .items_center()
        .gap(px(10.))
        .py_1()
        .text_size(px(12.))
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
                .font_features(theme::numeric())
                .child(format_size(i.size, r.size_is_estimate)),
        )
        .children(reveal)
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_words_read_plainly() {
        assert_eq!(
            method_words(&MethodView::Command {
                display: "brew cleanup".into()
            }),
            "Runs `brew cleanup`"
        );
    }

    #[test]
    fn the_root_reads_as_a_tilde() {
        assert_eq!(display_path("/Users/m", "/Users/m"), "~");
    }

    #[test]
    fn paths_under_the_root_start_with_a_tilde() {
        assert_eq!(display_path("/Users/m", "/Users/m/Desktop"), "~/Desktop");
    }

    #[test]
    fn mock_paths_stay_as_they_are() {
        assert_eq!(display_path("~", "~/Desktop"), "~/Desktop");
    }

    #[test]
    fn look_alike_prefixes_are_not_the_root() {
        assert_eq!(display_path("/Users/m", "/Users/me/x"), "/Users/me/x");
    }
}
