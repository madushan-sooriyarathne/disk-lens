//! Treemap layout and label helpers, ported from d3's squarified treemap.
//! Tile colors live in theme.rs.
//! Pure: called only when the view or the container size changes, never on hover.

use crate::view::ViewNode;

pub const GROUP_HEADER: f64 = 30.0;
pub const GROUP_PAD: f64 = 8.0;
pub const TILE_GAP: f64 = 6.0;
pub const ROOT_LABEL: &str = "~ Home";

#[derive(Debug, Clone, PartialEq)]
pub struct Rect {
    /// `ViewNode::id`; look the node up with [`find`].
    pub node_id: String,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    /// 1 = top-level group or tile, 2 = tile inside a group.
    pub depth: u8,
    /// True when the rect is a group that draws a header and holds tiles.
    pub group: bool,
    pub parent_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayoutOptions {
    pub header: f64,
    pub group_pad: f64,
    pub gap: f64,
}

impl Default for LayoutOptions {
    fn default() -> Self {
        Self {
            header: GROUP_HEADER,
            group_pad: GROUP_PAD,
            gap: TILE_GAP,
        }
    }
}

/// Squarified layout of a view (root plus two levels), matching d3's
/// `treemapSquarify` with the paddings and rounding of the TS version.
pub fn layout_view(view: &ViewNode, width: f64, height: f64, opts: LayoutOptions) -> Vec<Rect> {
    if view.children.is_empty() || width <= 0.0 || height <= 0.0 {
        return Vec::new();
    }
    let mut nodes = Vec::new();
    build(view, None, 0, &mut nodes);
    sort_children(&mut nodes, 0);
    nodes[0].x1 = width;
    nodes[0].y1 = height;
    position(&mut nodes, 0, &opts);
    for n in &mut nodes {
        n.x0 = js_round(n.x0);
        n.y0 = js_round(n.y0);
        n.x1 = js_round(n.x1);
        n.y1 = js_round(n.y1);
    }

    // Breadth-first, like d3's `descendants()`.
    let mut out = Vec::new();
    let mut queue = std::collections::VecDeque::from([0usize]);
    while let Some(i) = queue.pop_front() {
        let n = &nodes[i];
        queue.extend(n.children.iter().copied());
        if n.depth == 0 || n.depth > 2 {
            continue;
        }
        out.push(Rect {
            node_id: n.data.id.clone(),
            x: n.x0,
            y: n.y0,
            w: n.x1 - n.x0,
            h: n.y1 - n.y0,
            depth: n.depth as u8,
            group: n.depth == 1 && !n.children.is_empty(),
            parent_id: n
                .parent
                .filter(|&p| nodes[p].depth > 0)
                .map(|p| nodes[p].data.id.clone()),
        });
    }
    out
}

/// d3 `Node`: arena entry with the fields the treemap needs.
struct Node<'a> {
    data: &'a ViewNode,
    parent: Option<usize>,
    depth: usize,
    children: Vec<usize>,
    value: f64,
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
}

/// `hierarchy(...).sum(n => n.children.length === 0 ? n.size : 0)`; returns the node's index.
fn build<'a>(
    data: &'a ViewNode,
    parent: Option<usize>,
    depth: usize,
    nodes: &mut Vec<Node<'a>>,
) -> usize {
    let i = nodes.len();
    nodes.push(Node {
        data,
        parent,
        depth,
        children: Vec::new(),
        value: 0.0,
        x0: 0.0,
        y0: 0.0,
        x1: 0.0,
        y1: 0.0,
    });
    let children: Vec<usize> = data
        .children
        .iter()
        .map(|c| build(c, Some(i), depth + 1, nodes))
        .collect();
    let own = if children.is_empty() {
        data.size as f64
    } else {
        0.0
    };
    nodes[i].value = own + children.iter().map(|&c| nodes[c].value).sum::<f64>();
    nodes[i].children = children;
    i
}

/// `.sort((a, b) => (b.value - a.value) || a.data.name.localeCompare(b.data.name))`.
// Deviation: plain string order instead of ICU `localeCompare`; only matters for
// equal sizes whose names differ in case or punctuation.
fn sort_children(nodes: &mut [Node], i: usize) {
    let mut kids = std::mem::take(&mut nodes[i].children);
    kids.sort_by(|&a, &b| {
        nodes[b]
            .value
            .total_cmp(&nodes[a].value)
            .then_with(|| nodes[a].data.name.cmp(&nodes[b].data.name))
    });
    for &k in &kids {
        sort_children(nodes, k);
    }
    nodes[i].children = kids;
}

/// d3 `positionNode` (applied top-down); the padding stack is `0` for the root
/// and `gap / 2` below it.
fn position(nodes: &mut [Node], i: usize, opts: &LayoutOptions) {
    let p = if nodes[i].depth == 0 {
        0.0
    } else {
        opts.gap / 2.0
    };
    let (mut x0, mut y0, mut x1, mut y1) = (
        nodes[i].x0 + p,
        nodes[i].y0 + p,
        nodes[i].x1 - p,
        nodes[i].y1 - p,
    );
    if x1 < x0 {
        x0 = (x0 + x1) / 2.0;
        x1 = x0;
    }
    if y1 < y0 {
        y0 = (y0 + y1) / 2.0;
        y1 = y0;
    }
    nodes[i].x0 = x0;
    nodes[i].y0 = y0;
    nodes[i].x1 = x1;
    nodes[i].y1 = y1;
    if nodes[i].children.is_empty() {
        return;
    }
    let p = opts.gap / 2.0;
    let group = nodes[i].depth == 1;
    let (top, side, bottom) = if group {
        (opts.header, opts.group_pad, opts.group_pad)
    } else {
        (0.0, 0.0, 0.0)
    };
    x0 += side - p;
    y0 += top - p;
    x1 -= side - p;
    y1 -= bottom - p;
    if x1 < x0 {
        x0 = (x0 + x1) / 2.0;
        x1 = x0;
    }
    if y1 < y0 {
        y0 = (y0 + y1) / 2.0;
        y1 = y0;
    }
    squarify(nodes, i, x0, y0, x1, y1);
    for k in nodes[i].children.clone() {
        position(nodes, k, opts);
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Labels {
    pub name: bool,
    pub size: bool,
}

/// The name hides below 60x28 px and the size below 80x44 px, or when it
/// would overlap the name.
pub fn label_visibility(w: f64, h: f64) -> Labels {
    let name = w >= 60.0 && h >= NAME_BAND;
    let room = !name || h >= NAME_BAND + SIZE_BAND;
    Labels {
        name,
        size: w >= 80.0 && h >= 44.0 && room,
    }
}

const NAME_BAND: f64 = 28.0; // top inset plus one line of name
const SIZE_BAND: f64 = 21.0; // 13 px size label plus its bottom inset
const MIN_FONT: f64 = 13.0;
const MAX_FONT: f64 = 22.0;
const SMALL_AREA: f64 = 80.0 * 44.0;
const LARGE_AREA: f64 = 200.0 * 200.0;

/// Size label font grows with tile area, from 13 to 22 px.
pub fn size_font_px(area: f64) -> f64 {
    let t = (area.sqrt() - SMALL_AREA.sqrt()) / (LARGE_AREA.sqrt() - SMALL_AREA.sqrt());
    js_round(MIN_FONT + t.clamp(0.0, 1.0) * (MAX_FONT - MIN_FONT))
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

    /// PRD section 10: layout under 16 ms for 400 tiles. Timing is only meaningful
    /// in release builds: `cargo test --release -- --ignored layout_of_400_tiles`.
    #[test]
    #[ignore = "timing; run in release"]
    fn layout_of_400_tiles_takes_under_16_ms() {
        let groups = (0..20u64)
            .map(|g| {
                let kids = (0..20u64)
                    .map(|k| node(&format!("g{g}k{k}"), 1_000 + g * 37 + k * 11, vec![]))
                    .collect();
                node(&format!("g{g}"), 0, kids)
            })
            .map(|mut g| {
                g.size = g.children.iter().map(|c| c.size).sum();
                g
            })
            .collect::<Vec<_>>();
        let view = node("r", groups.iter().map(|g| g.size).sum(), groups);
        let fastest = (0..5)
            .map(|_| {
                let started = std::time::Instant::now();
                std::hint::black_box(layout_view(&view, 1100.0, 700.0, LayoutOptions::default()));
                started.elapsed()
            })
            .min()
            .unwrap();
        assert!(fastest.as_millis() < 16, "layout took {fastest:?}");
    }

    fn rect<'a>(rects: &'a [Rect], id: &str) -> &'a Rect {
        rects.iter().find(|r| r.node_id == id).unwrap()
    }

    type Golden = (
        &'static str,
        f64,
        f64,
        f64,
        f64,
        u8,
        bool,
        Option<&'static str>,
    );

    fn assert_golden(rects: &[Rect], golden: &[Golden]) {
        let got: Vec<_> = rects
            .iter()
            .map(|r| {
                (
                    r.node_id.as_str(),
                    r.x,
                    r.y,
                    r.w,
                    r.h,
                    r.depth,
                    r.group,
                    r.parent_id.as_deref(),
                )
            })
            .collect();
        assert_eq!(got, golden);
    }

    #[test]
    fn areas_are_proportional_to_sizes_without_padding() {
        let opts = LayoutOptions {
            gap: 0.0,
            ..Default::default()
        };
        let rects = layout_view(&flat(&[600, 300, 100]), 1000.0, 500.0, opts);
        let total: f64 = rects.iter().map(|r| r.w * r.h).sum();
        assert!((total - 500_000.0).abs() < 500.0, "total {total}");
        let big = rect(&rects, "n0");
        assert!((big.w * big.h / total - 0.6).abs() < 0.05);
    }

    #[test]
    fn keeps_every_rect_inside_the_bounds_and_rects_apart() {
        let rects = layout_view(
            &flat(&[50, 40, 30, 20, 10, 5]),
            800.0,
            600.0,
            Default::default(),
        );
        for r in &rects {
            assert!(r.x >= 0.0 && r.y >= 0.0);
            assert!(r.x + r.w <= 800.0 && r.y + r.h <= 600.0);
        }
        for (i, a) in rects.iter().enumerate() {
            for b in &rects[i + 1..] {
                let overlap =
                    a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h;
                assert!(!overlap, "{} overlaps {}", a.node_id, b.node_id);
            }
        }
    }

    #[test]
    fn nests_tiles_inside_group_rects_below_the_header() {
        let view = node(
            "r",
            100,
            vec![node(
                "g",
                100,
                vec![node("a", 60, vec![]), node("b", 40, vec![])],
            )],
        );
        let opts = LayoutOptions {
            header: 30.0,
            group_pad: 8.0,
            ..Default::default()
        };
        let rects = layout_view(&view, 400.0, 300.0, opts);
        let g = rect(&rects, "g");
        let a = rect(&rects, "a");
        assert!(g.group);
        assert_eq!(a.parent_id.as_deref(), Some("g"));
        assert!(a.y >= g.y + 30.0);
        assert!(a.x >= g.x + 8.0);
    }

    fn mock_view() -> ViewNode {
        fn truncate(mut n: ViewNode, levels: usize) -> ViewNode {
            n.children = if levels == 0 {
                vec![]
            } else {
                n.children
                    .into_iter()
                    .map(|c| truncate(c, levels - 1))
                    .collect()
            };
            n
        }
        let json: serde_json::Value =
            serde_json::from_str(include_str!("../../assets/mock/snapshot.mock.json")).unwrap();
        truncate(serde_json::from_value(json["view"].clone()).unwrap(), 2)
    }

    /// Golden numbers come from d3-hierarchy running the TS `layoutView` on the
    /// mock snapshot truncated to two levels, 760x560.
    #[test]
    fn lays_out_the_mock_view() {
        let view = mock_view();
        let rects = layout_view(&view, 760.0, 560.0, Default::default());
        assert_eq!(
            rects.iter().filter(|r| r.depth == 1).count(),
            view.children.len()
        );
        assert!(rects.len() > 30);
        assert_golden(&rects, MOCK_GOLDEN);
    }

    #[test]
    fn returns_nothing_for_empty_views_or_sizes() {
        assert!(layout_view(&node("r", 0, vec![]), 100.0, 100.0, Default::default()).is_empty());
        assert!(layout_view(&flat(&[1]), 0.0, 100.0, Default::default()).is_empty());
    }

    #[test]
    fn lays_out_400_tiles_in_under_16_ms() {
        let groups = (0..20)
            .map(|g| {
                let tiles = (0..20)
                    .map(|t| node(&format!("t{g}-{t}"), 50 + t, vec![]))
                    .collect();
                node(&format!("g{g}"), 1000, tiles)
            })
            .collect();
        let view = node("r", 20_000, groups);
        layout_view(&view, 760.0, 560.0, Default::default()); // warm up
        let mut runs: Vec<_> = (0..15)
            .map(|_| {
                let t0 = std::time::Instant::now();
                layout_view(&view, 760.0, 560.0, Default::default());
                t0.elapsed().as_secs_f64() * 1000.0
            })
            .collect();
        runs.sort_by(f64::total_cmp);
        assert!(runs[7] < 16.0, "median {} ms", runs[7]);
    }

    /// d3-hierarchy output for the same tree, 500x400, default options. Covers a
    /// zero-size leaf, equal sizes (name tie-break), a single-child group and
    /// plain tiles next to groups.
    #[test]
    fn matches_d3_on_a_nested_tree() {
        let t = |n, s| node(n, s, vec![]);
        let view = node(
            "r",
            0,
            vec![
                node(
                    "docs",
                    300,
                    vec![t("a", 120), t("b", 90), t("c", 90), t("zero", 0)],
                ),
                node("lib", 250, vec![t("x", 100), t("y", 100), t("z", 50)]),
                t("cache", 200),
                node("big", 150, vec![t("one", 150)]),
                t("tie2", 20),
                t("tie1", 20),
                t("tiny", 3),
            ],
        );
        assert_golden(
            &layout_view(&view, 500.0, 400.0, Default::default()),
            SMALL_GOLDEN,
        );
    }

    const SMALL_GOLDEN: &[Golden] = &[
        ("docs", 0.0, 0.0, 289.0, 215.0, 1, true, None),
        ("lib", 0.0, 221.0, 289.0, 179.0, 1, true, None),
        ("cache", 295.0, 0.0, 205.0, 201.0, 1, false, None),
        ("big", 295.0, 207.0, 158.0, 193.0, 1, true, None),
        ("tie1", 459.0, 207.0, 41.0, 86.0, 1, false, None),
        ("tie2", 459.0, 299.0, 41.0, 87.0, 1, false, None),
        ("tiny", 459.0, 392.0, 41.0, 8.0, 1, false, None),
        ("a", 8.0, 30.0, 189.0, 99.0, 2, false, Some("docs")),
        ("b", 8.0, 135.0, 189.0, 72.0, 2, false, Some("docs")),
        ("c", 203.0, 30.0, 78.0, 177.0, 2, false, Some("docs")),
        ("zero", 203.0, 210.0, 78.0, 0.0, 2, false, Some("docs")),
        ("x", 8.0, 251.0, 217.0, 68.0, 2, false, Some("lib")),
        ("y", 8.0, 325.0, 217.0, 67.0, 2, false, Some("lib")),
        ("z", 231.0, 251.0, 50.0, 141.0, 2, false, Some("lib")),
        ("one", 303.0, 237.0, 142.0, 155.0, 2, false, Some("big")),
    ];

    const MOCK_GOLDEN: &[Golden] = &[
        ("1", 0.0, 0.0, 327.0, 395.0, 1, true, None),
        ("19", 0.0, 401.0, 327.0, 159.0, 1, true, None),
        ("9", 333.0, 0.0, 234.0, 217.0, 1, true, None),
        ("13", 573.0, 0.0, 187.0, 217.0, 1, false, None),
        ("14", 333.0, 223.0, 175.0, 123.0, 1, true, None),
        ("16", 333.0, 352.0, 175.0, 111.0, 1, true, None),
        ("23", 333.0, 469.0, 175.0, 91.0, 1, false, None),
        ("24", 514.0, 223.0, 87.0, 133.0, 1, true, None),
        ("28", 607.0, 223.0, 74.0, 133.0, 1, false, None),
        ("32", 687.0, 223.0, 73.0, 133.0, 1, true, None),
        ("29", 514.0, 362.0, 139.0, 65.0, 1, false, None),
        ("27", 514.0, 433.0, 139.0, 60.0, 1, false, None),
        ("30", 514.0, 499.0, 139.0, 61.0, 1, false, None),
        ("37", 659.0, 362.0, 49.0, 156.0, 1, false, None),
        ("31", 714.0, 362.0, 46.0, 156.0, 1, false, None),
        ("36", 659.0, 524.0, 101.0, 36.0, 1, false, None),
        ("2", 8.0, 30.0, 206.0, 211.0, 2, false, Some("1")),
        ("3", 220.0, 30.0, 99.0, 211.0, 2, false, Some("1")),
        ("8", 8.0, 247.0, 153.0, 67.0, 2, false, Some("1")),
        ("5", 8.0, 320.0, 153.0, 67.0, 2, false, Some("1")),
        ("4", 167.0, 247.0, 104.0, 69.0, 2, false, Some("1")),
        ("6", 167.0, 322.0, 104.0, 65.0, 2, false, Some("1")),
        ("7", 277.0, 247.0, 42.0, 140.0, 2, false, Some("1")),
        ("20", 8.0, 431.0, 177.0, 121.0, 2, false, Some("19")),
        ("21", 191.0, 431.0, 115.0, 121.0, 2, false, Some("19")),
        ("22", 312.0, 431.0, 7.0, 121.0, 2, false, Some("19")),
        ("10", 341.0, 30.0, 167.0, 113.0, 2, false, Some("9")),
        ("11", 341.0, 149.0, 167.0, 60.0, 2, false, Some("9")),
        ("12", 514.0, 30.0, 45.0, 179.0, 2, false, Some("9")),
        ("15", 341.0, 253.0, 159.0, 85.0, 2, false, Some("14")),
        ("17", 341.0, 382.0, 147.0, 73.0, 2, false, Some("16")),
        ("18", 494.0, 382.0, 6.0, 73.0, 2, false, Some("16")),
        ("25", 522.0, 253.0, 71.0, 75.0, 2, false, Some("24")),
        ("26", 522.0, 334.0, 71.0, 14.0, 2, false, Some("24")),
        ("33", 695.0, 253.0, 57.0, 47.0, 2, false, Some("32")),
        ("34", 695.0, 306.0, 41.0, 42.0, 2, false, Some("32")),
        ("35", 742.0, 306.0, 10.0, 42.0, 2, false, Some("32")),
    ];

    #[test]
    fn hides_name_below_60x28_and_size_below_80x44() {
        let l = |name, size| Labels { name, size };
        assert_eq!(label_visibility(59.0, 40.0), l(false, false));
        assert_eq!(label_visibility(60.0, 28.0), l(true, false));
        assert_eq!(label_visibility(80.0, 43.0), l(true, false));
        assert_eq!(label_visibility(80.0, 48.0), l(true, false)); // would overlap the name
        assert_eq!(label_visibility(80.0, 49.0), l(true, true));
        assert_eq!(label_visibility(70.0, 60.0), l(true, false));
    }

    #[test]
    fn scales_the_size_font_from_13_to_22_px_with_area() {
        assert_eq!(size_font_px(80.0 * 44.0), 13.0);
        assert_eq!(size_font_px(1.0), 13.0);
        assert_eq!(size_font_px(1e7), 22.0);
        let mid = size_font_px(120.0 * 120.0);
        assert!(mid > 13.0 && mid < 22.0);
        assert!(size_font_px(150.0 * 150.0) >= mid);
    }

    #[test]
    fn builds_crumbs_from_the_root() {
        let c = |label: &str, path: &str| Crumb {
            label: label.into(),
            path: path.into(),
        };
        assert_eq!(
            crumbs("/Users/m", "/Users/m"),
            vec![c("~ Home", "/Users/m")]
        );
        assert_eq!(
            crumbs("/Users/m", "/Users/m/Desktop/atlas"),
            vec![
                c("~ Home", "/Users/m"),
                c("Desktop", "/Users/m/Desktop"),
                c("atlas", "/Users/m/Desktop/atlas"),
            ]
        );
        let labels: Vec<_> = crumbs("~", "~/Desktop")
            .into_iter()
            .map(|c| c.label)
            .collect();
        assert_eq!(labels, ["~ Home", "Desktop"]);
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
        assert!(find(&view, "nope").is_none());
    }
}
