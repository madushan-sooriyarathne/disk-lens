//! Colors, fonts and tile colors (owner: A3). Port of `legacy-web/src/styles.css`
//! tokens, `legacy-web/src/lib/categories.ts`, and `tileColors` from
//! `legacy-web/src/lib/treemap.ts`. Category colors come from `rules.toml`.

use gpui::{App, Hsla, Window};

use crate::view::{ViewMode, ViewNode};

pub const SANS: &str = "Geist";
pub const MONO: &str = "Geist Mono";
/// Hatch overlay opacity on matched tiles (PRD section 9).
pub const HATCH_OPACITY: f32 = 0.35;
/// Legend order follows the mockup.
pub const LEGEND_ORDER: [&str; 6] = [
    "documents_media",
    "dev_tools",
    "code_projects",
    "mobile_sdks",
    "ai_models",
    "package_caches",
];

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Theme {
    pub bg: Hsla,
    pub card: Hsla,
    pub card_border: Hsla,
    pub text: Hsla,
    pub muted: Hsla,
    pub divider: Hsla,
    pub safe: Hsla,
    pub review: Hsla,
    pub safe_fill: Hsla,
    pub review_fill: Hsla,
}

impl Theme {
    pub fn light() -> Self {
        todo!()
    }
}

/// The theme for the window's appearance. Light only until Phase 5 adds dark.
pub fn current(window: &Window) -> Theme {
    Theme::light()
}

/// `(fill, border)` of a category; unknown ids get `dev_tools`.
pub fn category_colors(id: &str) -> (Hsla, Hsla) {
    todo!()
}

pub fn category_label(id: &str) -> &'static str {
    todo!()
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TileColors {
    pub fill: Hsla,
    pub border: Hsla,
    /// Tier color for the hatch overlay, when a recommendation matches.
    pub hatch: Option<Hsla>,
}

pub fn tile_colors(theme: &Theme, node: &ViewNode, mode: ViewMode) -> TileColors {
    todo!()
}

/// Registers the embedded Geist and Geist Mono fonts.
pub fn load_fonts(cx: &App) {}
