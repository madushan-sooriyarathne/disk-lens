//! Treemap layout, label fitting and keyboard neighbours (PRD 13.4.5).
//! Tile colors live in theme.rs.
//! Pure: layout runs only when the view or the container size changes, never on hover.

use super::theme::GAP_TILE;
use crate::view::ViewNode;

pub const ROOT_LABEL: &str = "~ Home";
/// Below this a tile shows no text (PRD FR-EXP-3).
pub const LABEL_MIN_W: f64 = 90.0;
pub const LABEL_MIN_H: f64 = 44.0;
/// Text inset from a tile's left and right edges.
pub const LABEL_PAD_X: f64 = 10.0;
/// The lock glyph and its gap before a system-managed tile's name.
pub const LOCK_W: f64 = 16.0;
/// The dot and its gap before a cleanable line.
pub const DOT_W: f64 = 11.0;
const ELLIPSIS: char = '\u{2026}';

#[derive(Debug, Clone, PartialEq)]
pub struct Rect {
    /// `ViewNode::id`; look the node up with [`find`].
    pub node_id: String,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// Squarified layout of a view's children in their given order (the view puts
/// the smaller-items tile last), 4 pt apart and flush with the outer edges.
pub fn layout_view(view: &ViewNode, width: f64, height: f64) -> Vec<Rect> {
    if view.children.is_empty() || width <= 0.0 || height <= 0.0 {
        return Vec::new();
    }
    let half = f64::from(GAP_TILE) / 2.0;
    let mut nodes: Vec<Node> = std::iter::once(Node::new(0.0))
        .chain(view.children.iter().map(|c| Node::new(c.size as f64)))
        .collect();
    nodes[0].value = nodes[1..].iter().map(|n| n.value).sum();
    nodes[0].children = (1..nodes.len()).collect();
    // Each tile gives up half a gap on every side, so the box grows by half a gap
    // to keep the outer tiles flush.
    squarify(&mut nodes, 0, -half, -half, width + half, height + half);
    view.children
        .iter()
        .zip(&nodes[1..])
        .map(|(c, n)| {
            let (x0, x1) = shrink(n.x0, n.x1, half);
            let (y0, y1) = shrink(n.y0, n.y1, half);
            let (x0, y0, x1, y1) = (js_round(x0), js_round(y0), js_round(x1), js_round(y1));
            Rect {
                node_id: c.id.clone(),
                x: x0,
                y: y0,
                w: x1 - x0,
                h: y1 - y0,
            }
        })
        .collect()
}

/// Insets an interval by `by` on both ends, collapsing to its middle when too short.
fn shrink(a: f64, b: f64, by: f64) -> (f64, f64) {
    if b - a < 2.0 * by {
        let m = (a + b) / 2.0;
        (m, m)
    } else {
        (a + by, b - by)
    }
}

/// d3 `Node`: arena entry with the fields the treemap needs.
struct Node {
    children: Vec<usize>,
    value: f64,
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
}

impl Node {
    fn new(value: f64) -> Self {
        Self {
            children: Vec::new(),
            value,
            x0: 0.0,
            y0: 0.0,
            x1: 0.0,
            y1: 0.0,
        }
    }
}

/// d3 `treemapSquarify` with the golden ratio.
fn squarify(nodes: &mut [Node], parent: usize, mut x0: f64, mut y0: f64, x1: f64, y1: f64) {
    let ratio = (1.0 + 5f64.sqrt()) / 2.0;
    let kids = nodes[parent].children.clone();
    let n = kids.len();
    let mut value = nodes[parent].value;
    let (mut i0, mut i1) = (0, 0);
    while i0 < n {
        let (dx, dy) = (x1 - x0, y1 - y0);

        // Find the next non-empty node.
        let mut sum_value;
        loop {
            sum_value = nodes[kids[i1]].value;
            i1 += 1;
            if sum_value != 0.0 || i1 >= n {
                break;
            }
        }
        let (mut min_value, mut max_value) = (sum_value, sum_value);
        let alpha = js_max(dy / dx, dx / dy) / (value * ratio);
        let mut beta = sum_value * sum_value * alpha;
        let mut min_ratio = js_max(max_value / beta, beta / min_value);

        // Keep adding nodes while the aspect ratio maintains or improves.
        while i1 < n {
            let node_value = nodes[kids[i1]].value;
            sum_value += node_value;
            if node_value < min_value {
                min_value = node_value;
            }
            if node_value > max_value {
                max_value = node_value;
            }
            beta = sum_value * sum_value * alpha;
            let new_ratio = js_max(max_value / beta, beta / min_value);
            if new_ratio > min_ratio {
                sum_value -= node_value;
                break;
            }
            min_ratio = new_ratio;
            i1 += 1;
        }

        // Position the row.
        let row = &kids[i0..i1];
        if dx < dy {
            let ny0 = y0;
            if value != 0.0 {
                y0 += dy * sum_value / value;
            }
            let row_y1 = if value != 0.0 { y0 } else { y1 };
            lay_row(nodes, row, sum_value, true, x0, ny0, x1, row_y1);
        } else {
            let nx0 = x0;
            if value != 0.0 {
                x0 += dx * sum_value / value;
            }
            let row_x1 = if value != 0.0 { x0 } else { x1 };
            lay_row(nodes, row, sum_value, false, nx0, y0, row_x1, y1);
        }
        value -= sum_value;
        i0 = i1;
    }
}

/// d3 `treemapDice` (`dice`: tiles side by side) and `treemapSlice` (stacked).
#[allow(clippy::too_many_arguments)]
fn lay_row(
    nodes: &mut [Node],
    row: &[usize],
    row_value: f64,
    dice: bool,
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
) {
    let (mut x, mut y) = (x0, y0);
    if dice {
        let k = if row_value != 0.0 {
            (x1 - x0) / row_value
        } else {
            0.0
        };
        for &c in row {
            let n = &mut nodes[c];
            n.y0 = y0;
            n.y1 = y1;
            n.x0 = x;
            x += n.value * k;
            n.x1 = x;
        }
    } else {
        let k = if row_value != 0.0 {
            (y1 - y0) / row_value
        } else {
            0.0
        };
        for &c in row {
            let n = &mut nodes[c];
            n.x0 = x0;
            n.x1 = x1;
            n.y0 = y;
            y += n.value * k;
            n.y1 = y;
        }
    }
}

/// JS `Math.max`: NaN wins, unlike `f64::max`.
fn js_max(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else {
        a.max(b)
    }
}

/// JS `Math.round`: halves round toward +infinity.
fn js_round(x: f64) -> f64 {
    let f = x.floor();
    if x - f >= 0.5 { f + 1.0 } else { f }
}

/// What a tile shows: both lines or nothing (PRD FR-EXP-3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TileLabel {
    pub name: String,
    pub line2: String,
    /// `line2` is the cleanable line rather than the size.
    pub cleanable: bool,
}

/// The label for a `w` x `h` tile, or `None` when it would not fit. The name
/// truncates in the middle; the cleanable line replaces the size when it fits
/// (PRD FR-EXP-5). `name_w` and `line_w` measure text in the name and second-line
/// fonts.
#[allow(clippy::too_many_arguments)]
pub fn tile_label(
    name: &str,
    size_line: &str,
    cleanable_line: Option<&str>,
    has_lock: bool,
    w: f64,
    h: f64,
    name_w: impl Fn(&str) -> f64,
    line_w: impl Fn(&str) -> f64,
) -> Option<TileLabel> {
    if w < LABEL_MIN_W || h < LABEL_MIN_H {
        return None;
    }
    let room = w - 2.0 * LABEL_PAD_X;
    let name = fit_middle(name, room - if has_lock { LOCK_W } else { 0.0 }, name_w);
    if name.is_empty() || line_w(size_line) > room {
        return None;
    }
    let cleanable = cleanable_line.filter(|l| DOT_W + line_w(l) <= room);
    Some(TileLabel {
        name,
        line2: cleanable.unwrap_or(size_line).to_owned(),
        cleanable: cleanable.is_some(),
    })
}

/// `text`, or its start and end around an ellipsis, as wide as fits in `max_w`;
/// empty when not even the ellipsis fits.
pub fn fit_middle(text: &str, max_w: f64, measure: impl Fn(&str) -> f64) -> String {
    if measure(text) <= max_w {
        return text.to_owned();
    }
    let chars: Vec<char> = text.chars().collect();
    if chars.is_empty() {
        return String::new();
    }
    let cut = |keep: usize| -> String {
        let head = keep.div_ceil(2);
        let tail = keep / 2;
        chars[..head]
            .iter()
            .chain(std::iter::once(&ELLIPSIS))
            .chain(&chars[chars.len() - tail..])
            .collect()
    };
    // Widths grow with the kept count, so the longest fit is a binary search.
    let (mut lo, mut hi) = (0usize, chars.len() - 1);
    if measure(&cut(0)) > max_w {
        return String::new();
    }
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        if measure(&cut(mid)) <= max_w {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    cut(lo)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dir {
    Left,
    Right,
    Up,
    Down,
}

/// The tile the selection moves to from `rects[from]` (PRD FR-KB-5): the nearest
/// centre in that direction, sideways distance counting double. `None` at the edge.
pub fn neighbor(rects: &[Rect], from: usize, dir: Dir) -> Option<usize> {
    let centre = |r: &Rect| (r.x + r.w / 2.0, r.y + r.h / 2.0);
    let (fx, fy) = centre(rects.get(from)?);
    rects
        .iter()
        .enumerate()
        .filter(|&(i, _)| i != from)
        .filter_map(|(i, r)| {
            let (cx, cy) = centre(r);
            let (ahead, side) = match dir {
                Dir::Left => (fx - cx, cy - fy),
                Dir::Right => (cx - fx, cy - fy),
                Dir::Up => (fy - cy, cx - fx),
                Dir::Down => (cy - fy, cx - fx),
            };
            (ahead > 0.0).then_some((i, ahead + 2.0 * side.abs()))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}

/// The frame `p` (0 to 1) of the way from `from` to `to`, as `[x, y, w, h]`.
pub fn zoom_frame(from: [f64; 4], to: [f64; 4], p: f64) -> [f64; 4] {
    std::array::from_fn(|i| from[i] + (to[i] - from[i]) * p)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Crumb {
    pub label: String,
    pub path: String,
}

/// Breadcrumb from the scan root down to `path`; the root reads "~ Home".
pub fn crumbs(root_path: &str, path: &str) -> Vec<Crumb> {
    let mut out = vec![Crumb {
        label: ROOT_LABEL.into(),
        path: root_path.into(),
    }];
    let Some(rest) = path
        .strip_prefix(root_path)
        .and_then(|r| r.strip_prefix('/'))
    else {
        return out;
    };
    let mut acc = root_path.to_string();
    for part in rest.split('/') {
        acc = format!("{acc}/{part}");
        out.push(Crumb {
            label: part.into(),
            path: acc.clone(),
        });
    }
    out
}

pub fn parent_path(path: &str) -> String {
    match path.rfind('/') {
        Some(i) if i > 0 => path[..i].into(),
        _ => "/".into(),
    }
}

/// The node with `id` in `view`, at any depth.
pub fn find<'a>(view: &'a ViewNode, id: &str) -> Option<&'a ViewNode> {
    if view.id == id {
        return Some(view);
    }
    view.children.iter().find_map(|c| find(c, id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view::ViewKind;

    fn node(name: &str, size: u64, children: Vec<ViewNode>) -> ViewNode {
        ViewNode {
            id: name.into(),
            name: name.into(),
            path: format!("/r/{name}"),
            size,
            category: "code_projects".into(),
            tier: None,
            rule_id: None,
            kind: if children.is_empty() {
                ViewKind::File
            } else {
                ViewKind::Dir
            },
            file_count: 0,
            cleanable: 0,
            system_managed: false,
            children,
        }
    }

    fn flat(sizes: &[u64]) -> ViewNode {
        let kids = sizes
            .iter()
            .enumerate()
            .map(|(i, &s)| node(&format!("n{i}"), s, vec![]))
            .collect();
        node("r", sizes.iter().sum(), kids)
    }

    fn rect<'a>(rects: &'a [Rect], id: &str) -> &'a Rect {
        rects.iter().find(|r| r.node_id == id).unwrap()
    }

    /// PRD section 10: layout under 16 ms for 400 tiles. Timing is only meaningful
    /// in release builds: `cargo test --release -- --ignored layout_of_400_tiles`.
    #[test]
    #[ignore = "timing; run in release"]
    fn layout_of_400_tiles_takes_under_16_ms() {
        let sizes: Vec<u64> = (0..400u64).map(|i| 1_000 + i * 37 % 911).collect();
        let view = flat(&sizes);
        let fastest = (0..5)
            .map(|_| {
                let started = std::time::Instant::now();
                std::hint::black_box(layout_view(&view, 1100.0, 700.0));
                started.elapsed()
            })
            .min()
            .unwrap();
        assert!(fastest.as_millis() < 16, "layout took {fastest:?}");
    }

    #[test]
    fn areas_follow_sizes() {
        let rects = layout_view(&flat(&[600, 300, 100]), 1000.0, 500.0);
        let total: f64 = rects.iter().map(|r| r.w * r.h).sum();
        let big = rect(&rects, "n0");
        assert!((big.w * big.h / total - 0.6).abs() < 0.05);
    }

    #[test]
    fn tiles_stay_inside_and_never_overlap() {
        let rects = layout_view(&flat(&[50, 40, 30, 20, 10, 5]), 800.0, 600.0);
        for (i, a) in rects.iter().enumerate() {
            assert!(a.x >= 0.0 && a.y >= 0.0 && a.x + a.w <= 800.0 && a.y + a.h <= 600.0);
            for b in &rects[i + 1..] {
                let overlap =
                    a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h;
                assert!(!overlap, "{} overlaps {}", a.node_id, b.node_id);
            }
        }
    }

    #[test]
    fn tiles_touch_the_outer_edges() {
        let rects = layout_view(&flat(&[50, 40, 30]), 800.0, 600.0);
        let right = rects.iter().map(|r| r.x + r.w).fold(0.0, f64::max);
        assert_eq!((rects[0].x, rects[0].y, right), (0.0, 0.0, 800.0));
    }

    #[test]
    fn neighbouring_tiles_are_one_gap_apart() {
        let rects = layout_view(&flat(&[50, 50]), 400.0, 300.0);
        let (a, b) = (rect(&rects, "n0"), rect(&rects, "n1"));
        // Side by side or stacked, depending on the row squarify picks.
        let gap = (b.x - (a.x + a.w)).max(b.y - (a.y + a.h));
        assert_eq!(gap, f64::from(GAP_TILE));
    }

    #[test]
    fn keeps_the_given_order() {
        let rects = layout_view(&flat(&[10, 50, 30]), 600.0, 400.0);
        let ids: Vec<&str> = rects.iter().map(|r| r.node_id.as_str()).collect();
        assert_eq!(ids, ["n0", "n1", "n2"]);
    }

    #[test]
    fn returns_nothing_for_empty_views_or_sizes() {
        assert!(layout_view(&node("r", 0, vec![]), 100.0, 100.0).is_empty());
        assert!(layout_view(&flat(&[1]), 0.0, 100.0).is_empty());
    }

    /// 13 pt semibold runs about 7.5 pt per character; 8 overestimates on purpose.
    fn est(text: &str) -> f64 {
        text.chars().count() as f64 * 8.0
    }

    /// 12 pt second lines.
    fn est_line(text: &str) -> f64 {
        text.chars().count() as f64 * 7.0
    }

    /// Height a label needs: top inset, a 13 pt line and a 12 pt line.
    const LABEL_H: f64 = 8.0 + 17.0 + 16.0;

    /// Small deterministic generator, so the property test needs no dependency.
    fn lcg(seed: &mut u64) -> u64 {
        *seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        *seed >> 33
    }

    #[test]
    fn labels_never_overflow_their_tile_at_the_minimum_window() {
        use super::super::chrome::{INSPECTOR_WIDTH, SIDEBAR_WIDTH, TOOLBAR_HEIGHT};
        let pad = f64::from(super::super::theme::GAP_CONTENT);
        let w = 960.0 - f64::from(SIDEBAR_WIDTH) - f64::from(INSPECTOR_WIDTH) - 2.0 * pad;
        let h = 640.0 - f64::from(TOOLBAR_HEIGHT) - 2.0 * pad;
        let mut seed = 7;
        for _ in 0..200 {
            let n = 1 + lcg(&mut seed) % 13;
            let sizes: Vec<u64> = (0..n).map(|_| 1 + lcg(&mut seed) % 1_000_000).collect();
            let mut view = flat(&sizes);
            for c in &mut view.children {
                let len = 1 + (lcg(&mut seed) % 40) as usize;
                c.name = "W".repeat(len);
                c.system_managed = lcg(&mut seed).is_multiple_of(4);
            }
            for r in layout_view(&view, w, h) {
                let c = view.children.iter().find(|c| c.id == r.node_id).unwrap();
                let lock = if c.system_managed { LOCK_W } else { 0.0 };
                let Some(l) = tile_label(
                    &c.name,
                    "999.9 GB",
                    Some("999.9 GB cleanable"),
                    c.system_managed,
                    r.w,
                    r.h,
                    est,
                    est_line,
                ) else {
                    continue;
                };
                let line = est_line(&l.line2) + if l.cleanable { DOT_W } else { 0.0 };
                assert!(
                    est(&l.name) + lock <= r.w - 2.0 * LABEL_PAD_X,
                    "{l:?} in {r:?}"
                );
                assert!(line <= r.w - 2.0 * LABEL_PAD_X, "{l:?} in {r:?}");
                assert!(LABEL_H <= r.h, "{l:?} in {r:?}");
            }
        }
    }

    #[test]
    fn small_tiles_get_no_label() {
        assert_eq!(
            tile_label("a", "1 GB", None, false, 89.0, 60.0, est, est_line),
            None
        );
    }

    #[test]
    fn short_tiles_get_no_label() {
        assert_eq!(
            tile_label("a", "1 GB", None, false, 200.0, 43.0, est, est_line),
            None
        );
    }

    #[test]
    fn the_cleanable_line_replaces_the_size_when_it_fits() {
        let l = tile_label(
            "a",
            "1 GB",
            Some("1 GB cleanable"),
            false,
            200.0,
            60.0,
            est,
            est_line,
        );
        assert_eq!(l.map(|l| l.line2), Some("1 GB cleanable".to_owned()));
    }

    #[test]
    fn the_size_shows_when_the_cleanable_line_is_too_long() {
        let l = tile_label(
            "a",
            "1 GB",
            Some("1 GB cleanable"),
            false,
            100.0,
            60.0,
            est,
            est_line,
        );
        assert_eq!(l.map(|l| l.line2), Some("1 GB".to_owned()));
    }

    #[test]
    fn long_names_truncate_in_the_middle() {
        assert_eq!(fit_middle("abcdefghij", 6.0 * 8.0, est), "abc\u{2026}ij");
    }

    #[test]
    fn names_that_fit_stay_whole() {
        assert_eq!(fit_middle("abc", 100.0, est), "abc");
    }

    #[test]
    fn no_room_gives_an_empty_name() {
        assert_eq!(fit_middle("abc", 4.0, est), "");
    }

    /// 2x2 grid: n0 top left, n1 top right, n2 bottom left, n3 bottom right.
    fn grid() -> Vec<Rect> {
        [(0.0, 0.0), (100.0, 0.0), (0.0, 100.0), (100.0, 100.0)]
            .into_iter()
            .enumerate()
            .map(|(i, (x, y))| Rect {
                node_id: format!("n{i}"),
                x,
                y,
                w: 96.0,
                h: 96.0,
            })
            .collect()
    }

    #[test]
    fn neighbour_moves_in_each_direction() {
        let g = grid();
        assert_eq!(
            [
                neighbor(&g, 0, Dir::Right),
                neighbor(&g, 0, Dir::Down),
                neighbor(&g, 3, Dir::Left),
                neighbor(&g, 3, Dir::Up),
            ],
            [Some(1), Some(2), Some(2), Some(1)]
        );
    }

    #[test]
    fn neighbour_never_leaves_the_view() {
        let g = grid();
        assert_eq!(
            [neighbor(&g, 0, Dir::Left), neighbor(&g, 0, Dir::Up)],
            [None, None]
        );
    }

    #[test]
    fn neighbour_prefers_the_tile_straight_ahead() {
        // A tall tile on the left; on the right, a far tile in line and a nearer one off to the side.
        let r = |id: &str, x, y, w, h| Rect {
            node_id: id.into(),
            x,
            y,
            w,
            h,
        };
        let rects = vec![
            r("a", 0.0, 100.0, 50.0, 50.0),
            r("off", 60.0, 0.0, 20.0, 20.0),
            r("ahead", 150.0, 100.0, 50.0, 50.0),
        ];
        assert_eq!(neighbor(&rects, 0, Dir::Right), Some(2));
    }

    #[test]
    fn zoom_frame_starts_at_the_tile_and_ends_full() {
        let (from, to) = ([10.0, 20.0, 30.0, 40.0], [0.0, 0.0, 400.0, 300.0]);
        assert_eq!(
            [zoom_frame(from, to, 0.0), zoom_frame(from, to, 1.0)],
            [from, to]
        );
    }

    #[test]
    fn builds_crumbs_from_the_root() {
        let c = |label: &str, path: &str| Crumb {
            label: label.into(),
            path: path.into(),
        };
        assert_eq!(
            crumbs("/Users/m", "/Users/m/Desktop/atlas"),
            vec![
                c("~ Home", "/Users/m"),
                c("Desktop", "/Users/m/Desktop"),
                c("atlas", "/Users/m/Desktop/atlas"),
            ]
        );
    }

    #[test]
    fn finds_parent_paths() {
        assert_eq!(parent_path("/Users/m/Desktop"), "/Users/m");
        assert_eq!(parent_path("/Users"), "/");
    }

    #[test]
    fn finds_nodes_at_any_depth() {
        let view = node("r", 1, vec![node("g", 1, vec![node("a", 1, vec![])])]);
        assert_eq!(find(&view, "a").map(|n| n.name.as_str()), Some("a"));
    }
}
