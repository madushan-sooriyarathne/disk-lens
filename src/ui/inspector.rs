//! Inspector (PRD 13.4.7): the trailing panel that describes the selected tile,
//! a folder, a file or the smaller-items list. Phase 11 adds recommendation rows.

use gpui::{AnyElement, Context, FontWeight, Hsla, Role, SharedString, div, prelude::*, px, svg};
use gpui_component::button::Button;
use gpui_component::{Disableable, Sizable, Size};

use super::chrome::INSPECTOR_WIDTH;
use super::format::{format_bytes, format_size, group_digits};
use super::model::AppModel;
use super::selection::within;
use super::shell::CONTROL_HEIGHT;
use super::stats::largest_folders;
use super::theme::{self, Theme};
use crate::recommend::model::ItemStatus;
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
    let Some(node) = model.tile_node() else {
        return panel
            .flex()
            .items_center()
            .justify_center()
            .text_color(t.secondary)
            .child("Nothing selected")
            .into_any_element();
    };
    let body = match node.kind {
        ViewKind::Smaller => smaller(node, t, cx),
        _ => item(model, node, t, cx),
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
                .on_click(cx.listener(move |m, _, _, cx| m.focus_row(Some(row_id.clone()), cx)))
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

#[cfg(test)]
mod tests {
    use super::*;

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
