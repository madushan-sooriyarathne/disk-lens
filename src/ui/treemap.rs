//! Treemap layout and label helpers (owner: A2). Port of
//! `legacy-web/src/lib/treemap.ts` without the color helpers (those live in theme.rs).
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
    todo!()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Labels {
    pub name: bool,
    pub size: bool,
}

/// The name hides below 60x28 px and the size below 80x44 px, or when it
/// would overlap the name.
pub fn label_visibility(w: f64, h: f64) -> Labels {
    todo!()
}

/// Size label font grows with tile area, from 13 to 22 px.
pub fn size_font_px(area: f64) -> f64 {
    todo!()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Crumb {
    pub label: String,
    pub path: String,
}

/// Breadcrumb from the scan root down to `path`; the root reads "~ Home".
pub fn crumbs(root_path: &str, path: &str) -> Vec<Crumb> {
    todo!()
}

pub fn parent_path(path: &str) -> String {
    todo!()
}

/// The node with `id` in `view`, at any depth.
pub fn find<'a>(view: &'a ViewNode, id: &str) -> Option<&'a ViewNode> {
    todo!()
}
