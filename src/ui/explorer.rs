//! Explore page (PRD 13.4.5): one level of tiles with semantic textures, the
//! texture key, tooltip and context menu. The breadcrumb is drawn in the
//! toolbar from `crumbs`.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

use gpui::{
    AnyElement, Context, Font, FontWeight, Hsla, MouseButton, MouseDownEvent, Pixels, Point, Role,
    SharedString, TextRun, Window, anchored, canvas, deferred, div, pattern_slash, prelude::*, px,
    svg,
};

use super::format::{format_age, format_bytes, group_digits};
use super::model::{AppModel, ScanProgress};
use super::selection::{self, row_for_node};
use super::theme::{self, Texture, Theme};
use super::treemap::{self, Rect};
use crate::recommend::model::{ItemStatus, Recommendation, Tier};
use crate::rules::model::Rules;
use crate::view::{ViewKind, ViewNode};

const MANAGED: &str = "Managed by macOS or an app";

/// Context menu opened by a right click on a tile.
struct Menu {
    /// Pointer position in window coordinates.
    pos: Point<Pixels>,
    key: String,
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

impl ExplorerState {
    /// Closes the context menu; true if one was open (Esc closes it before going up).
    pub fn close_menu(&mut self) -> bool {
        self.menu.take().is_some()
    }

    /// Records the measured area; true when it changed.
    fn set_size(&mut self, size: (u32, u32)) -> bool {
        let changed = self.size != Some(size);
        self.size = Some(size);
        changed
    }

    /// Rects for `view` at the measured size, in the order of `view.children`. Runs
    /// the layout only when the view contents or the size changed, never on hover.
    pub(super) fn rects(&mut self, view: &ViewNode) -> Option<Rc<Vec<Rect>>> {
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
        ));
        self.cached = Some(Cached {
            fingerprint: fp,
            size,
            rects: rects.clone(),
        });
        Some(rects)
    }
}

/// Identity of a view's layout input: node ids and sizes. Tiers and categories
/// do not move tiles, so they are not part of it.
fn fingerprint(view: &ViewNode) -> u64 {
    let mut h = DefaultHasher::new();
    view.id.hash(&mut h);
    for c in &view.children {
        c.id.hash(&mut h);
        c.size.hash(&mut h);
    }
    h.finish()
}

/// What identifies a tile across snapshots, whose node ids change: its path, and
/// for the smaller-items tile, which shares its folder's path, a suffix.
pub fn tile_key(node: &ViewNode) -> String {
    match node.kind {
        ViewKind::Smaller => format!("{}#smaller", node.path),
        _ => node.path.clone(),
    }
}

/// The kind of space a tile is (PRD 13.5, Texture). Children of a matched
/// folder carry its tier, so a tile inside an item is textured like the item;
/// a folder that only contains items keeps its own look.
pub(super) fn texture(node: &ViewNode, recs: &[Recommendation]) -> Texture {
    if node.kind == ViewKind::Smaller {
        return Texture::Smaller;
    }
    if node.system_managed {
        return Texture::System;
    }
    match node.tier {
        Some(Tier::Review) => Texture::Restorable,
        Some(Tier::Safe) => {
            let mut covering = recs
                .iter()
                .flat_map(|r| &r.items)
                .filter(|i| selection::within(&node.path, &i.path))
                .peekable();
            // Disposable only while the item can be cleaned now; a blocked one waits.
            if covering.peek().is_none() || covering.any(|i| i.status == ItemStatus::Ready) {
                Texture::Disposable
            } else {
                Texture::Restorable
            }
        }
        None => Texture::Kept,
    }
}

/// 45 degree stripes, 2 px lines every 7 px measured across the stripes (PRD 13.5).
/// The shader lays lines `0.707 * width` thick on a `0.707 * (width + interval)`
/// pitch and works in device pixels, so both are scaled by `sqrt 2` and the
/// window scale factor.
fn hatch(color: Hsla, scale: f32) -> gpui::Background {
    let s = scale * std::f32::consts::SQRT_2;
    pattern_slash(color.opacity(theme::STRIPE_ALPHA), 2.0 * s, 5.0 * s)
}

/// One line on what a tile is: managed by the system, its rule's description,
/// else its category's.
fn tile_description(node: &ViewNode) -> Option<&'static str> {
    if node.system_managed {
        return Some(MANAGED);
    }
    let rules = Rules::embedded();
    node.rule_id
        .as_deref()
        .and_then(|id| rules.rule(id))
        .map(|r| r.description.as_str())
        .or_else(|| {
            rules
                .category(&node.category)
                .map(|c| c.description.as_str())
        })
}

/// What the tooltip shows for a tile.
struct TooltipView {
    name: SharedString,
    size: String,
    category: &'static str,
    description: Option<&'static str>,
    theme: Theme,
}

impl Render for TooltipView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme;
        div()
            .max_w(px(320.))
            .rounded(px(theme::R_CONTROL))
            .border_1()
            .border_color(t.separator)
            .bg(t.popover)
            .text_color(t.label)
            .font_family(theme::SANS)
            .shadow_lg()
            .px_3()
            .py_2()
            .text_size(px(theme::T_SMALL))
            .child(
                div()
                    .text_size(px(theme::T_BODY))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(self.name.clone()),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .text_color(t.secondary)
                    .child(self.size.clone())
                    .child(self.category),
            )
            .children(self.description.map(|d| div().mt_1().child(d)))
    }
}

pub fn render(
    model: &mut AppModel,
    window: &mut Window,
    cx: &mut Context<AppModel>,
) -> impl IntoElement {
    let t = theme::current(window);
    let rects = model
        .view
        .as_ref()
        .and_then(|v| model.explorer.rects(v))
        .unwrap_or_default();

    let treemap = div()
        .id("treemap")
        .relative()
        .flex_1()
        .min_h_0()
        .child(measure(cx));
    let treemap = match &model.view {
        Some(v) if v.children.is_empty() => treemap.child(
            div()
                .pt_10()
                .text_center()
                .text_size(px(theme::T_BODY))
                .text_color(t.secondary)
                .child("Nothing to show here. Press Esc to go back."),
        ),
        Some(v) => {
            let menu_open = model.explorer.menu.is_some();
            treemap.children(
                v.children
                    .iter()
                    .zip(rects.iter())
                    .map(|(node, r)| tile(model, node, r, t, window, menu_open, cx)),
            )
        }
        None => treemap,
    };

    div()
        .flex_1()
        .min_w_0()
        .min_h_0()
        .flex()
        .flex_col()
        .gap(px(theme::GAP_CONTENT))
        .child(treemap)
        .child(texture_key(t, model.highlight_cleanable))
        .children(context_menu(model, t, cx))
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

/// What the toolbar says next to the scan button: progress, the last failure, or the
/// snapshot age ("Scanned 2 h ago", US-13).
pub(super) fn status_label(
    scan: Option<&ScanProgress>,
    error: Option<&str>,
    scanned_at: Option<i64>,
    now: i64,
) -> Option<String> {
    if let Some(scan) = scan {
        return Some(format!("Updating… {} files", group_digits(scan.files)));
    }
    if let Some(error) = error {
        return Some(error.to_owned());
    }
    scanned_at.map(|at| format!("Scanned {}", format_age(now, at)))
}

/// The path from the scan root to the open folder; earlier parts are links.
pub(super) fn crumbs(model: &AppModel, t: Theme, cx: &mut Context<AppModel>) -> impl IntoElement {
    let trail = treemap::crumbs(&model.root_path, &model.path);
    let last = trail.len() - 1;
    let items = trail.into_iter().enumerate().flat_map(|(i, c)| {
        let sep = (i > 0).then(|| div().text_color(t.tertiary).child("/").into_any_element());
        let label = if i == last {
            div()
                .text_color(t.label)
                .font_weight(FontWeight::SEMIBOLD)
                .child(c.label)
                .into_any_element()
        } else {
            let path = c.path;
            theme::focusable(
                div().id(SharedString::from(path.clone())),
                &t,
                Role::Link,
                format!("Go to {}", c.label),
            )
            .cursor_pointer()
            .hover(move |s| s.text_color(t.label).underline())
            .child(c.label)
            .on_click(cx.listener(move |model, _, _, cx| model.navigate(path.clone(), cx)))
            .into_any_element()
        };
        sep.into_iter().chain(Some(label))
    });
    div()
        .flex()
        .items_center()
        .gap(px(4.))
        .text_size(px(theme::T_BODY))
        .text_color(t.secondary)
        .children(items)
}

/// One line under the treemap naming each surface (PRD 13.5, Texture).
fn texture_key(t: Theme, highlight: bool) -> impl IntoElement {
    let swatch = |category: &str, texture: Texture, label: &'static str| {
        let (fill, stripe) = theme::tile_fill(&t, category, texture);
        div()
            .flex()
            .items_center()
            .gap(px(6.))
            .child(
                div()
                    .relative()
                    .w(px(18.))
                    .h(px(12.))
                    .rounded(px(3.))
                    .overflow_hidden()
                    .bg(fill)
                    .when(texture == Texture::Smaller, |d| {
                        d.border_1().border_dashed().border_color(t.neutral_border)
                    })
                    .children(stripe.map(|s| div().absolute().inset_0().bg(hatch(s, 1.)))),
            )
            .child(label)
    };
    div()
        .flex()
        .flex_wrap()
        .items_center()
        .gap_x_4()
        .gap_y_1()
        .text_size(px(theme::T_SMALL))
        .text_color(t.secondary)
        .when(highlight, |d| {
            d.child(
                div()
                    .text_color(t.label)
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Highlight Cleanable is on"),
            )
        })
        .child(swatch("package_caches", Texture::Disposable, "Disposable"))
        .child(swatch("ai_models", Texture::Restorable, "Restorable"))
        .child(swatch("code_projects", Texture::Kept, "Your data"))
        .child(swatch("dev_tools", Texture::System, "Managed by macOS"))
        .child(swatch("dev_tools", Texture::Smaller, "Smaller items"))
}

/// Width of `text` in the system font at `size` and `weight`.
fn text_width(window: &Window, text: &str, size: f32, weight: FontWeight) -> f64 {
    let font = Font {
        weight,
        features: theme::numeric(),
        ..gpui::font(theme::SANS)
    };
    let run = TextRun {
        len: text.len(),
        font,
        color: gpui::black(),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let line = window
        .text_system()
        .layout_line(text, px(size), &[run], None);
    f64::from(f32::from(line.width))
}

/// One absolutely positioned tile.
fn tile(
    model: &AppModel,
    node: &ViewNode,
    r: &Rect,
    t: Theme,
    window: &Window,
    menu_open: bool,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let texture = texture(node, &model.recommendations);
    let (fill, stripe) = theme::tile_fill(&t, &node.category, texture);
    let key = tile_key(node);
    let selected = model.tile.as_deref() == Some(key.as_str());
    let size = format_bytes(node.size);
    let has_lock = texture == Texture::System;
    // Only a tile that is not itself cleanable space points at what is inside it.
    let cleanable = (matches!(texture, Texture::Kept | Texture::System) && node.cleanable > 0)
        .then(|| format!("{} cleanable", format_bytes(node.cleanable)));
    let label = treemap::tile_label(
        &node.name,
        &size,
        cleanable.as_deref(),
        has_lock,
        r.w,
        r.h,
        |s| text_width(window, s, theme::T_BODY, FontWeight::SEMIBOLD),
        |s| text_width(window, s, 12., FontWeight::NORMAL),
    );
    let dim = model.highlight_cleanable
        && matches!(texture, Texture::Kept | Texture::System)
        && node.cleanable == 0;
    let aria = format!(
        "{}, {size}, {}",
        node.name,
        if has_lock {
            MANAGED
        } else {
            theme::category_label(&node.category)
        }
    );

    let mut tile = theme::focusable(
        div().id(SharedString::from(key.clone())),
        &t,
        Role::Button,
        aria,
    )
    .absolute()
    .left(px(r.x as f32))
    .top(px(r.y as f32))
    .w(px(r.w as f32))
    .h(px(r.h as f32))
    .overflow_hidden()
    .rounded(px(theme::R_TILE))
    .bg(fill)
    .cursor_pointer()
    .when(texture == Texture::Smaller, |d| {
        d.border_1().border_dashed().border_color(t.neutral_border)
    })
    .when(dim, |d| d.opacity(theme::DIM))
    .occlude()
    .on_click({
        let key = key.clone();
        cx.listener(move |m, e: &gpui::ClickEvent, _, cx| {
            if e.click_count() >= 2 {
                m.open_tile(&key, cx);
            } else {
                m.select_tile(Some(key.clone()), cx);
            }
        })
    })
    .on_mouse_down(MouseButton::Right, {
        let key = key.clone();
        cx.listener(move |m, e: &MouseDownEvent, _, cx| {
            m.explorer.menu = Some(Menu {
                pos: e.position,
                key: key.clone(),
            });
            m.select_tile(Some(key.clone()), cx);
        })
    });

    // The tooltip must not show over the open menu: without a builder gpui drops it.
    if !menu_open {
        let name = SharedString::from(node.name.clone());
        let tip_size = size.clone();
        let category = theme::category_label(&node.category);
        let description = tile_description(node);
        tile = tile.tooltip(move |_, cx| {
            cx.new(|_| TooltipView {
                name: name.clone(),
                size: tip_size.clone(),
                category,
                description,
                theme: t,
            })
            .into()
        });
    }

    if let Some(color) = stripe {
        tile = tile.child(
            div()
                .absolute()
                .inset_0()
                .bg(hatch(color, window.scale_factor())),
        );
    }

    if let Some(label) = label {
        let line2 = if label.cleanable {
            div()
                .flex()
                .items_center()
                .gap(px(5.))
                .text_color(t.label)
                .child(div().size(px(6.)).rounded_full().bg(t.safe))
                .child(label.line2)
        } else {
            div().text_color(t.tile_secondary).child(label.line2)
        };
        tile = tile.child(
            div()
                .absolute()
                .left(px(treemap::LABEL_PAD_X as f32))
                .top(px(8.))
                .flex()
                .flex_col()
                .text_color(t.label)
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(4.))
                        .text_size(px(theme::T_BODY))
                        .line_height(px(17.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .when(has_lock, |d| {
                            d.child(
                                svg()
                                    .path("icons/ui/lock.svg")
                                    .size(px(12.))
                                    .flex_none()
                                    .text_color(t.tile_secondary),
                            )
                        })
                        .child(label.name),
                )
                .child(
                    line2
                        .text_size(px(12.))
                        .line_height(px(16.))
                        .font_features(theme::numeric()),
                ),
        );
    }
    if selected {
        // Drawn on top, so the ring stays above the stripes.
        tile = tile.child(
            div()
                .absolute()
                .inset_0()
                .rounded(px(theme::R_TILE))
                .border_2()
                .border_color(t.accent),
        );
    }
    tile.into_any_element()
}

/// The right-click menu, anchored at the pointer and closed by any outside press.
fn context_menu(model: &AppModel, t: Theme, cx: &mut Context<AppModel>) -> Option<AnyElement> {
    let menu = model.explorer.menu.as_ref()?;
    let node = model.tile_node()?;
    let row_id = row_for_node(&model.recommendations, node).map(|r| r.row_id.clone());
    let key = menu.key.clone();
    let can_stage = model.can_stage(node);
    let is_item = node.kind != ViewKind::Smaller;
    let path = node.path.clone();

    let item = |id: &'static str, label: &'static str| {
        div()
            .id(id)
            .cursor_pointer()
            .rounded(px(5.))
            .px_2()
            .py(px(3.))
            .text_size(px(theme::T_BODY))
            .hover(move |s| s.bg(t.accent).text_color(gpui::white()))
            .child(label)
    };
    let panel = div()
        .occlude()
        .min_w(px(192.))
        .rounded(px(theme::R_CONTROL))
        .border_1()
        .border_color(t.separator)
        .bg(t.popover)
        .text_color(t.label)
        .font_family(theme::SANS)
        .shadow_lg()
        .p_1()
        .on_mouse_down_out(cx.listener(|m, _, _, cx| {
            m.explorer.menu = None;
            cx.notify();
        }))
        .when(can_stage, |d| {
            d.child(
                item("menu-stage", "Add to Queue").on_click(cx.listener(move |m, _, _, cx| {
                    m.explorer.menu = None;
                    m.stage_tile(&key, cx);
                })),
            )
        })
        .when(is_item, |d| {
            d.child(
                item("menu-reveal", "Reveal in Finder").on_click(cx.listener(
                    move |m, _, _, cx| {
                        m.explorer.menu = None;
                        m.reveal(path.clone(), cx);
                        cx.notify();
                    },
                )),
            )
            .child(
                item("menu-quick-look", "Quick Look").on_click(cx.listener(|m, _, _, cx| {
                    m.explorer.menu = None;
                    m.quick_look(cx);
                })),
            )
        })
        .when_some(row_id, |d, row_id| {
            d.child(
                item("menu-show", "Show Recommendation").on_click(cx.listener(
                    move |m, _, _, cx| {
                        m.explorer.menu = None;
                        m.focus_row(Some(row_id.clone()), cx);
                    },
                )),
            )
        })
        .child(
            item("menu-info", "Get Info").on_click(cx.listener(|m, _, _, cx| {
                m.explorer.menu = None;
                m.inspector_visible = true;
                cx.notify();
            })),
        );
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
    use crate::recommend::model::{MethodView, RecItem};

    #[test]
    fn status_while_scanning_counts_files() {
        let scan = ScanProgress {
            files: 1_234,
            ..ScanProgress::default()
        };
        assert_eq!(
            status_label(Some(&scan), Some("old"), Some(0), 0).as_deref(),
            Some("Updating… 1,234 files")
        );
    }

    #[test]
    fn status_after_a_failure_is_the_message() {
        assert_eq!(
            status_label(None, Some("Scan cancelled"), Some(0), 0).as_deref(),
            Some("Scan cancelled")
        );
    }

    #[test]
    fn status_otherwise_is_the_snapshot_age() {
        assert_eq!(
            status_label(None, None, Some(0), 2 * 3_600).as_deref(),
            Some("Scanned 2 h ago")
        );
    }

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
            file_count: 0,
            cleanable: 0,
            system_managed: false,
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

    /// A safe row with one item at `path`.
    fn row(path: &str, status: ItemStatus) -> Recommendation {
        Recommendation {
            row_id: "r".into(),
            rule_id: "npm-cache".into(),
            label: "npm cache".into(),
            paths_hint: String::new(),
            tier: Tier::Safe,
            size: 1,
            size_is_estimate: false,
            items: vec![RecItem {
                item_id: "i".into(),
                path: path.into(),
                name: "i".into(),
                size: 1,
                status,
                method: MethodView::Delete,
            }],
        }
    }

    fn tiered(tier: Tier) -> ViewNode {
        ViewNode {
            tier: Some(tier),
            ..leaf("cache", 1)
        }
    }

    #[test]
    fn a_ready_safe_item_is_disposable() {
        let recs = [row("/r/cache", ItemStatus::Ready)];
        assert_eq!(texture(&tiered(Tier::Safe), &recs), Texture::Disposable);
    }

    #[test]
    fn a_blocked_safe_item_is_restorable() {
        let blocked = ItemStatus::Blocked {
            reason: "busy".into(),
        };
        let recs = [row("/r/cache", blocked)];
        assert_eq!(texture(&tiered(Tier::Safe), &recs), Texture::Restorable);
    }

    #[test]
    fn a_review_item_is_restorable() {
        assert_eq!(texture(&tiered(Tier::Review), &[]), Texture::Restorable);
    }

    #[test]
    fn a_folder_that_only_contains_items_is_kept() {
        let folder = ViewNode {
            cleanable: 5,
            ..leaf("projects", 10)
        };
        assert_eq!(texture(&folder, &[]), Texture::Kept);
    }

    #[test]
    fn a_system_managed_folder_is_system() {
        let folder = ViewNode {
            system_managed: true,
            ..leaf("Containers", 10)
        };
        assert_eq!(texture(&folder, &[]), Texture::System);
    }

    #[test]
    fn the_smaller_tile_is_smaller() {
        let smaller = ViewNode {
            kind: ViewKind::Smaller,
            ..leaf("rest", 10)
        };
        assert_eq!(texture(&smaller, &[]), Texture::Smaller);
    }

    #[test]
    fn the_smaller_tile_key_differs_from_its_folder() {
        let smaller = ViewNode {
            kind: ViewKind::Smaller,
            ..leaf("rest", 10)
        };
        assert_ne!(tile_key(&smaller), smaller.path);
    }

    #[test]
    fn matched_tile_describes_its_rule() {
        let mut node = leaf("_cacache", 1);
        node.rule_id = Some("npm-cache".into());
        assert_eq!(
            tile_description(&node),
            Some("Package tarballs and metadata npm keeps to speed up installs.")
        );
    }

    #[test]
    fn unmatched_tile_describes_its_category() {
        assert_eq!(
            tile_description(&leaf("misc", 1)),
            Some("Developer tool settings, app data and anything not in another category.")
        );
    }

    #[test]
    fn a_managed_tile_says_so() {
        let node = ViewNode {
            system_managed: true,
            ..leaf("Containers", 1)
        };
        assert_eq!(tile_description(&node), Some(MANAGED));
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
        // A hover or a retier re-renders with the same layout input: same Rc, no layout.
        let mut retiered = v.clone();
        retiered.children[0].tier = Some(Tier::Safe);
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
