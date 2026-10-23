//! Explorer card: header, breadcrumb, treemap, legend, tooltip and context
//! menu (owner: B2). Port of `legacy-web/src/components/Explorer/*`.

use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

use gpui::{
    AnyElement, Context, FontWeight, Hsla, MouseButton, MouseDownEvent, Pixels, Point,
    SharedString, Window, anchored, canvas, deferred, div, pattern_slash, prelude::*, px,
};

use super::format::format_bytes;
use super::model::AppModel;
use super::selection::row_for_node;
use super::theme::{self, Theme};
use super::treemap::{self, LayoutOptions, Rect};
use crate::view::{ViewKind, ViewMode, ViewNode};

/// Context menu opened by a right click on a tile.
struct Menu {
    /// Pointer position in window coordinates.
    pos: Point<Pixels>,
    node_id: String,
}

/// Layout result for one (view contents, container size) pair.
struct Cached {
    fingerprint: u64,
    size: (u32, u32),
    rects: Rc<Vec<Rect>>,
}

/// Explorer-only UI state: the layout cache, the measured treemap area and the open menu.
#[derive(Default)]
pub struct ExplorerState {
    /// Treemap area in whole pixels, measured by a canvas during prepaint.
    size: Option<(u32, u32)>,
    cached: Option<Cached>,
    menu: Option<Menu>,
}

/// Identity of a view's layout input: node ids and sizes. Tiers, categories and
/// the mode do not move tiles, so they are not part of it.
fn fingerprint(view: &ViewNode) -> u64 {
    fn walk(n: &ViewNode, h: &mut DefaultHasher) {
        n.id.hash(h);
        n.size.hash(h);
        n.children.len().hash(h);
        for c in &n.children {
            walk(c, h);
        }
    }
    let mut h = DefaultHasher::new();
    walk(view, &mut h);
    h.finish()
}

impl ExplorerState {
    /// Records the measured area; true when it changed.
    fn set_size(&mut self, size: (u32, u32)) -> bool {
        let changed = self.size != Some(size);
        self.size = Some(size);
        changed
    }

    /// Rects for `view` at the measured size. Runs the layout only when the view
    /// contents or the size changed since the last call, never on hover.
    fn rects(&mut self, view: &ViewNode) -> Option<Rc<Vec<Rect>>> {
        let size = self.size.filter(|&(w, h)| w > 0 && h > 0)?;
        let fp = fingerprint(view);
        if let Some(c) = &self.cached
            && c.fingerprint == fp
            && c.size == size
        {
            return Some(c.rects.clone());
        }
        let rects = Rc::new(treemap::layout_view(
            view,
            f64::from(size.0),
            f64::from(size.1),
            LayoutOptions::default(),
        ));
        self.cached = Some(Cached {
            fingerprint: fp,
            size,
            rects: rects.clone(),
        });
        Some(rects)
    }
}

/// 45 degree stripes, 2 px lines every 7 px measured across the stripes (PRD 9).
/// The shader lays lines `0.707 * width` thick on a `0.707 * (width + interval)`
/// pitch and works in device pixels, so both are scaled by `sqrt 2` and the
/// window scale factor.
fn hatch(color: Hsla, scale: f32) -> gpui::Background {
    let s = scale * std::f32::consts::SQRT_2;
    pattern_slash(color.opacity(theme::HATCH_OPACITY), 2.0 * s, 5.0 * s)
}

/// What the tooltip shows for a tile (Tooltip.tsx).
struct TooltipView {
    path: SharedString,
    size: String,
    category: &'static str,
    rule: Option<String>,
    theme: Theme,
}

impl Render for TooltipView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme;
        div()
            .max_w(px(420.))
            .rounded_lg()
            .border_1()
            .border_color(t.card_border)
            .bg(t.card)
            .text_color(t.text)
            .font_family(theme::SANS)
            .shadow_lg()
            .px_3()
            .py_2()
            .text_xs()
            .child(
                div()
                    .font_family(theme::MONO)
                    .text_size(px(11.))
                    .text_color(t.muted)
                    .child(self.path.clone()),
            )
            .child(
                div()
                    .mt_1()
                    .flex()
                    .items_baseline()
                    .gap_2()
                    .child(
                        div()
                            .font_family(theme::MONO)
                            .text_size(px(14.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(self.size.clone()),
                    )
                    .child(div().text_color(t.muted).child(self.category)),
            )
            .children(self.rule.clone().map(|r| div().mt_1().child(r)))
    }
}

pub fn render(
    model: &mut AppModel,
    window: &mut Window,
    cx: &mut Context<AppModel>,
) -> impl IntoElement {
    let t = theme::current(window);
    let scale = window.scale_factor();
    let rects = model
        .view
        .as_ref()
        .and_then(|v| model.explorer.rects(v))
        .unwrap_or_default();

    let treemap = div()
        .relative()
        .flex_1()
        .min_h_0()
        .mx_5()
        .child(measure(cx));
    let treemap = match &model.view {
        Some(v) if v.children.is_empty() => treemap.child(
            div()
                .pt_10()
                .text_center()
                .text_size(px(13.))
                .text_color(t.muted)
                .child("Nothing to show here. Press Esc to go back."),
        ),
        Some(v) => {
            let by_id: HashMap<&str, &ViewNode> = {
                let mut m = HashMap::new();
                index(v, &mut m);
                m
            };
            let menu_open = model.explorer.menu.is_some();
            treemap.children(rects.iter().filter_map(|r| {
                let node = by_id.get(r.node_id.as_str())?;
                Some(tile(model, node, r, t, scale, menu_open, cx))
            }))
        }
        None => treemap,
    };

    div()
        .id("explorer")
        .flex_1()
        .min_w_0()
        .min_h_0()
        .flex()
        .flex_col()
        .rounded(px(12.))
        .border_1()
        .border_color(t.card_border)
        .bg(t.card)
        .child(header(model.mode, t, cx))
        .child(breadcrumb(model, t, cx))
        .child(treemap)
        .child(legend(t))
        .children(context_menu(model, t, cx))
}

fn index<'a>(n: &'a ViewNode, out: &mut HashMap<&'a str, &'a ViewNode>) {
    out.insert(n.id.as_str(), n);
    for c in &n.children {
        index(c, out);
    }
}

/// An empty canvas filling the treemap area. Its prepaint bounds are the
/// container size; the model is notified only when they change.
fn measure(cx: &mut Context<AppModel>) -> impl IntoElement {
    let model = cx.entity().downgrade();
    canvas(
        move |bounds, window, cx| {
            let size = (
                f32::from(bounds.size.width).floor() as u32,
                f32::from(bounds.size.height).floor() as u32,
            );
            let known = model.read_with(cx, |m, _| m.explorer.size).ok().flatten();
            if known != Some(size) {
                // Notifying while the window draws is dropped, so update after the frame.
                let model = model.clone();
                window.defer(cx, move |_, cx| {
                    model
                        .update(cx, |m, cx| {
                            if m.explorer.set_size(size) {
                                cx.notify();
                            }
                        })
                        .ok();
                });
            }
        },
        |_, _, _, _| {},
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

fn header(mode: ViewMode, t: Theme, cx: &mut Context<AppModel>) -> impl IntoElement {
    let toggle = [
        (ViewMode::Category, "Category"),
        (ViewMode::Cleanup, "Cleanup"),
    ]
    .into_iter()
    .map(|(m, label)| {
        let active = mode == m;
        div()
            .id(label)
            .cursor_pointer()
            .rounded_md()
            .px_3()
            .py_1()
            .text_size(px(13.))
            .border_1()
            .border_color(if active { t.card_border } else { t.bg })
            .when(active, |d| {
                d.bg(t.card).font_weight(FontWeight::MEDIUM).shadow_sm()
            })
            .when(!active, |d| d.text_color(t.muted))
            .child(label)
            .on_click(cx.listener(move |model, _, _, cx| model.set_mode(m, cx)))
    });
    div()
        .flex()
        .items_start()
        .justify_between()
        .px_5()
        .pt_5()
        .child(
            div()
                .child(
                    div()
                        .text_size(px(15.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Explorer"),
                )
                .child(
                    div()
                        .mt(px(2.))
                        .text_size(px(13.))
                        .text_color(t.muted)
                        .child("Your home folder, sized by space used. Click a tile to open it."),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .id("rescan")
                        .cursor_pointer()
                        .rounded_lg()
                        .px_3()
                        .py_1()
                        .text_size(px(13.))
                        .text_color(t.muted)
                        .hover(move |s| s.bg(t.bg).text_color(t.text))
                        .child("Rescan")
                        .on_click(cx.listener(|model, _, _, cx| model.start_scan(cx))),
                )
                .child(
                    div()
                        .flex()
                        .rounded_lg()
                        .bg(t.bg)
                        .p(px(2.))
                        .children(toggle),
                ),
        )
}

fn breadcrumb(model: &AppModel, t: Theme, cx: &mut Context<AppModel>) -> impl IntoElement {
    let trail = treemap::crumbs(&model.root_path, &model.path);
    let last = trail.len() - 1;
    let items = trail.into_iter().enumerate().flat_map(|(i, c)| {
        let sep = (i > 0).then(|| div().child("/").into_any_element());
        let label = if i == last {
            div().text_color(t.text).child(c.label).into_any_element()
        } else {
            let path = c.path;
            div()
                .id(SharedString::from(path.clone()))
                .cursor_pointer()
                .hover(move |s| s.text_color(t.text).underline())
                .child(c.label)
                .on_click(cx.listener(move |model, _, _, cx| model.navigate(path.clone(), cx)))
                .into_any_element()
        };
        sep.into_iter().chain(Some(label))
    });
    div()
        .mt_3()
        .border_t_1()
        .border_color(t.divider)
        .flex()
        .items_center()
        .justify_between()
        .px_5()
        .py(px(10.))
        .text_size(px(13.))
        .text_color(t.muted)
        .child(div().flex().items_center().gap(px(6.)).children(items))
        .children(
            model
                .view
                .as_ref()
                .map(|v| div().font_family(theme::MONO).child(format_bytes(v.size))),
        )
}

fn legend(t: Theme) -> impl IntoElement {
    div()
        .flex()
        .flex_wrap()
        .gap_x_5()
        .gap_y_1()
        .px_5()
        .pb_4()
        .pt_3()
        .text_size(px(13.))
        .text_color(t.muted)
        .children(theme::LEGEND_ORDER.into_iter().map(|id| {
            div()
                .flex()
                .items_center()
                .gap(px(6.))
                .child(
                    div()
                        .size(px(10.))
                        .rounded(px(3.))
                        .bg(theme::category_colors(id).1),
                )
                .child(theme::category_label(id))
        }))
}

/// One absolutely positioned tile or group. Tiles occlude the mouse, so a
/// group under a tile never sees its clicks, hover or right clicks.
fn tile(
    model: &AppModel,
    node: &ViewNode,
    r: &Rect,
    t: Theme,
    scale: f32,
    menu_open: bool,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let colors = theme::tile_colors(&t, node, model.mode);
    let is_dir = node.kind == ViewKind::Dir;
    let size = format_bytes(node.size);
    let radius = px(if r.group { 10. } else { 8. });
    let show = treemap::label_visibility(r.w, r.h);

    let mut tile = div()
        .id(SharedString::from(node.id.clone()))
        .absolute()
        .left(px(r.x as f32))
        .top(px(r.y as f32))
        .w(px(r.w as f32))
        .h(px(r.h as f32))
        .overflow_hidden()
        .rounded(radius)
        .border_1()
        .border_color(colors.border)
        .bg(colors.fill)
        .occlude()
        .when(is_dir, |d| {
            let path = node.path.clone();
            d.cursor_pointer()
                .on_click(cx.listener(move |m, _, _, cx| m.navigate(path.clone(), cx)))
        })
        .on_mouse_down(MouseButton::Right, {
            let node_id = node.id.clone();
            cx.listener(move |m, e: &MouseDownEvent, _, cx| {
                m.explorer.menu = Some(Menu {
                    pos: e.position,
                    node_id: node_id.clone(),
                });
                cx.notify();
            })
        });

    // The tooltip must not show over the open menu: without a builder gpui drops it.
    if !menu_open {
        let tip_path = SharedString::from(node.path.clone());
        let tip_size = size.clone();
        let tip_category = theme::category_label(&node.category);
        let tip_rule = node.rule_id.as_ref().and_then(|id| {
            model
                .recommendations
                .iter()
                .find(|r| &r.rule_id == id)
                .map(|r| r.label.clone())
        });
        tile = tile.tooltip(move |_, cx| {
            cx.new(|_| TooltipView {
                path: tip_path.clone(),
                size: tip_size.clone(),
                category: tip_category,
                rule: tip_rule.clone(),
                theme: t,
            })
            .into()
        });
    }

    if let Some(color) = colors.hatch {
        tile = tile.child(
            div()
                .absolute()
                .top_0()
                .left_0()
                .right_0()
                .bottom_0()
                .rounded(radius - px(1.))
                .bg(hatch(color, scale)),
        );
    }

    if r.group {
        tile = tile.child(
            div()
                .flex()
                .items_baseline()
                .justify_between()
                .gap_2()
                .px_3()
                .pt_2()
                .text_size(px(13.))
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(node.name.clone()),
                )
                .when(r.w >= 110., |d| {
                    d.child(
                        div()
                            .flex_none()
                            .font_family(theme::MONO)
                            .text_size(px(12.))
                            .text_color(t.muted)
                            .child(size),
                    )
                }),
        );
    } else {
        if show.name {
            tile = tile.child(
                div()
                    .absolute()
                    .left(px(10.))
                    .top(px(8.))
                    .right(px(8.))
                    .truncate()
                    .text_size(px(13.))
                    .font_weight(FontWeight::MEDIUM)
                    .child(node.name.clone()),
            );
        }
        if show.size {
            tile = tile.child(
                div()
                    .absolute()
                    .left(px(10.))
                    .bottom(px(8.))
                    .font_family(theme::MONO)
                    .font_weight(FontWeight::MEDIUM)
                    .text_size(px(treemap::size_font_px(r.w * r.h) as f32))
                    .child(size),
            );
        }
    }
    tile.into_any_element()
}

/// The right-click menu, anchored at the pointer and closed by any outside press.
fn context_menu(model: &AppModel, t: Theme, cx: &mut Context<AppModel>) -> Option<AnyElement> {
    let menu = model.explorer.menu.as_ref()?;
    let node = model
        .view
        .as_ref()
        .and_then(|v| treemap::find(v, &menu.node_id))?;
    let row_id = row_for_node(&model.recommendations, node).map(|r| r.row_id.clone());
    let path = node.path.clone();
    let can_reveal = node.kind != ViewKind::Other;

    let item = |id: &'static str, label: &'static str| {
        div()
            .id(id)
            .cursor_pointer()
            .rounded_md()
            .px_3()
            .py(px(6.))
            .text_size(px(13.))
            .hover(move |s| s.bg(t.bg))
            .child(label)
    };
    let panel = div()
        .occlude()
        .min_w(px(192.))
        .rounded_lg()
        .border_1()
        .border_color(t.card_border)
        .bg(t.card)
        .text_color(t.text)
        .font_family(theme::SANS)
        .shadow_lg()
        .p_1()
        .on_mouse_down_out(cx.listener(|m, _, _, cx| {
            m.explorer.menu = None;
            cx.notify();
        }))
        .when(can_reveal, |d| {
            d.child(
                item("menu-reveal", "Reveal in Finder").on_click(cx.listener(
                    move |m, _, _, cx| {
                        m.explorer.menu = None;
                        m.reveal(path.clone(), cx);
                        cx.notify();
                    },
                )),
            )
        })
        .when_some(row_id, |d, row_id| {
            d.child(
                item("menu-show", "Show recommendation").on_click(cx.listener(
                    move |m, _, _, cx| {
                        m.explorer.menu = None;
                        m.focus_row(Some(row_id.clone()), cx);
                        cx.notify();
                    },
                )),
            )
        });
    Some(
        deferred(
            anchored()
                .position(menu.pos)
                .snap_to_window_with_margin(px(8.))
                .child(panel),
        )
        .priority(1)
        .into_any_element(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leaf(id: &str, size: u64) -> ViewNode {
        ViewNode {
            id: id.into(),
            name: id.into(),
            path: format!("/r/{id}"),
            size,
            category: "dev_tools".into(),
            tier: None,
            rule_id: None,
            kind: ViewKind::Dir,
            children: vec![],
        }
    }

    fn view(sizes: &[u64]) -> ViewNode {
        let mut v = leaf("root", sizes.iter().sum());
        v.children = sizes
            .iter()
            .enumerate()
            .map(|(i, &s)| leaf(&format!("c{i}"), s))
            .collect();
        v
    }

    #[test]
    fn layout_waits_for_a_measured_size() {
        let mut st = ExplorerState::default();
        assert!(st.rects(&view(&[3, 2])).is_none());
        assert!(st.set_size((400, 300)));
        assert!(!st.set_size((400, 300)));
        assert_eq!(st.rects(&view(&[3, 2])).unwrap().len(), 2);
    }

    #[test]
    fn layout_is_cached_until_the_view_or_size_changes() {
        let mut st = ExplorerState::default();
        st.set_size((400, 300));
        let v = view(&[5, 3, 2]);
        let first = st.rects(&v).unwrap();
        // A hover or a mode switch re-renders with the same view: same Rc, no layout.
        assert!(Rc::ptr_eq(&first, &st.rects(&v.clone()).unwrap()));
        let mut retiered = v.clone();
        retiered.children[0].tier = Some(crate::recommend::model::Tier::Safe);
        assert!(Rc::ptr_eq(&first, &st.rects(&retiered).unwrap()));

        let resized = {
            st.set_size((401, 300));
            st.rects(&v).unwrap()
        };
        assert!(!Rc::ptr_eq(&first, &resized));
        let changed = st.rects(&view(&[5, 3, 1])).unwrap();
        assert!(!Rc::ptr_eq(&resized, &changed));
    }
}
