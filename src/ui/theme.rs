//! Colors, fonts and tile colors. Category colors and labels come from `rules.toml`.

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use gpui::{
    App, FontFeatures, Hsla, Role, SharedString, StatefulInteractiveElement, Styled, Window,
    WindowAppearance, px, rgb, rgba,
};

use crate::recommend::model::Tier;
use crate::rules::model::Rules;
use crate::view::ViewNode;

pub const SANS: &str = ".SystemUIFont";
/// SF Mono on macOS; GPUI resolves CoreText's hidden system name (checked in Phase 8).
pub const MONO: &str = ".AppleSystemUIFontMonospaced";
/// Stripe opacity on disposable tiles; PRD 13.5 caps it at 20%.
pub const STRIPE_ALPHA: f32 = 0.2;
/// How much of the category fill a disposable or restorable tile keeps.
pub const TEX_MIX: f32 = 0.65;
/// Opacity of Kept and System-managed tiles under Highlight cleanable.
pub const DIM: f32 = 0.45;

// Type scale (pt): large number and page title semibold, the rest regular
// except the headline.
pub const T_LARGE: f32 = 28.;
pub const T_TITLE: f32 = 20.;
pub const T_HEAD: f32 = 13.;
pub const T_BODY: f32 = 13.;
pub const T_SMALL: f32 = 11.;

pub const R_WIDGET: f32 = 14.;
pub const R_TILE: f32 = 10.;
pub const R_CONTROL: f32 = 8.;

/// 4 pt grid: around the content and between widgets, inside widgets, between tiles.
pub const GAP_CONTENT: f32 = 16.;
pub const GAP_WIDGET: f32 = 12.;
pub const GAP_TILE: f32 = 4.;
/// Legend order follows the mockup.
pub const LEGEND_ORDER: [&str; 6] = [
    "documents_media",
    "dev_tools",
    "code_projects",
    "mobile_sdks",
    "ai_models",
    "package_caches",
];

/// v2 tokens (PRD 13.5), named after `docs/mockup-v2/mockup.html`. Text shades
/// carry their WCAG ratio in the tests; `*_tint` are the tinted fills under them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Theme {
    /// Translucent over the blurred window background.
    pub sidebar: Hsla,
    /// The sidebar under Reduce Transparency.
    pub sidebar_solid: Hsla,
    pub content: Hsla,
    pub widget: Hsla,
    pub hover: Hsla,
    pub selected: Hsla,
    pub separator: Hsla,
    pub scrim: Hsla,
    pub popover: Hsla,
    pub label: Hsla,
    pub secondary: Hsla,
    /// Secondary text on the sidebar: the mockup's shade measures 4.2:1 there in light mode.
    pub sidebar_secondary: Hsla,
    /// Size lines on tiles: passes AA on every category fill and stripe pixel.
    pub tile_secondary: Hsla,
    /// Icons and placeholders only, never body text.
    pub tertiary: Hsla,
    pub accent: Hsla,
    pub accent_text: Hsla,
    /// Fills, dots and stripes.
    pub safe: Hsla,
    pub safe_text: Hsla,
    pub safe_tint: Hsla,
    pub review: Hsla,
    pub review_text: Hsla,
    pub review_tint: Hsla,
    /// Clean up button: the system green fails AA under white text, so a darker one.
    pub clean: Hsla,
    pub clean_text: Hsla,
    pub danger: Hsla,
    pub neutral: Hsla,
    pub neutral_border: Hsla,
    pub free: Hsla,
    /// Picks the `*_dark` category colors from rules.toml.
    pub dark: bool,
}

impl Theme {
    pub fn light() -> Self {
        let c = |hex: u32| Hsla::from(rgb(hex));
        let a = |hex: u32| Hsla::from(rgba(hex));
        Self {
            sidebar: a(0xeaeaeed1),
            sidebar_solid: c(0xeaeaee),
            content: c(0xffffff),
            widget: c(0xf5f5f7),
            hover: a(0x0000000d),
            selected: a(0x00000014),
            separator: a(0x0000001a),
            scrim: a(0x0000002e),
            popover: c(0xffffff),
            label: c(0x1d1d1f),
            secondary: c(0x6e6e73),
            sidebar_secondary: c(0x636366),
            tile_secondary: c(0x55555a),
            tertiary: c(0x8e8e93),
            accent: c(0x007aff),
            accent_text: c(0x0062cc),
            safe: c(0x34c759),
            safe_text: c(0x1d7a35),
            safe_tint: c(0xe5f6e9),
            review: c(0xff9500),
            review_text: c(0xa34f00),
            review_tint: c(0xfff1de),
            clean: c(0x1e7a34),
            clean_text: c(0xffffff),
            danger: c(0xd70015),
            neutral: c(0xececef),
            neutral_border: c(0xb4b4ba),
            free: c(0xe8e8ed),
            dark: false,
        }
    }

    pub fn dark() -> Self {
        let c = |hex: u32| Hsla::from(rgb(hex));
        let a = |hex: u32| Hsla::from(rgba(hex));
        Self {
            sidebar: a(0x2c2c30cc),
            sidebar_solid: c(0x2c2c30),
            content: c(0x1e1e1e),
            widget: c(0x2a2a2c),
            hover: a(0xffffff0f),
            selected: a(0xffffff1a),
            separator: a(0xffffff1a),
            scrim: a(0x00000066),
            popover: c(0x2c2c2e),
            label: c(0xf5f5f7),
            secondary: c(0xa1a1a6),
            sidebar_secondary: c(0xa1a1a6),
            tile_secondary: c(0xc2c2c7),
            tertiary: c(0x7c7c80),
            accent: c(0x0a84ff),
            accent_text: c(0x409cff),
            safe: c(0x30d158),
            safe_text: c(0x30d158),
            safe_tint: c(0x1c3323),
            review: c(0xff9f0a),
            review_text: c(0xff9f0a),
            review_tint: c(0x3a2a12),
            clean: c(0x1e7a34),
            clean_text: c(0xffffff),
            danger: c(0xff453a),
            neutral: c(0x2e2e32),
            neutral_border: c(0x5a5a60),
            free: c(0x3a3a3e),
            dark: true,
        }
    }
}

/// Tabular figures, set once on the window so every number lines up in columns.
pub fn numeric() -> FontFeatures {
    FontFeatures(Arc::new(vec![("tnum".into(), 1)]))
}

/// The theme for the window's appearance (PRD section 9: dark mode).
pub fn current(window: &Window) -> Theme {
    match window.appearance() {
        WindowAppearance::Dark | WindowAppearance::VibrantDark => Theme::dark(),
        WindowAppearance::Light | WindowAppearance::VibrantLight => Theme::light(),
    }
}

/// Drives gpui-component's theme from these tokens, so its controls match the
/// hand-built views. Called at startup and on every appearance change: `change`
/// reloads the component defaults for the mode, then ours overwrite them.
pub fn sync_components(window: &mut Window, cx: &mut App) {
    use gpui_component::{Theme as Kit, ThemeMode};
    let t = current(window);
    let mode = if t.dark {
        ThemeMode::Dark
    } else {
        ThemeMode::Light
    };
    Kit::change(mode, Some(window), cx);
    Kit::update(cx, |k| {
        let c = &mut k.colors;
        c.background = t.content;
        c.foreground = t.label;
        c.muted = t.widget;
        c.muted_foreground = t.secondary;
        c.border = t.separator;
        c.input = t.separator;
        c.ring = t.accent;
        c.caret = t.accent;
        c.selection = t.accent.opacity(0.25);
        c.accent = t.hover;
        c.accent_foreground = t.label;
        c.secondary = t.widget;
        c.secondary_hover = t.hover;
        c.secondary_active = t.selected;
        c.secondary_foreground = t.label;
        c.primary = t.clean;
        c.primary_hover = t.clean;
        c.primary_active = t.clean;
        c.primary_foreground = t.clean_text;
        c.button_primary = t.clean;
        c.button_primary_hover = t.clean;
        c.button_primary_active = t.clean;
        c.button_primary_foreground = t.clean_text;
        c.button = t.selected;
        c.button_hover = t.hover;
        c.button_active = t.selected;
        c.button_foreground = t.label;
        c.danger = t.danger;
        c.popover = t.popover;
        c.popover_foreground = t.label;
        c.list_hover = t.hover;
        c.list_active = t.selected;
        c.sidebar = t.sidebar;
        c.title_bar = t.content;
        k.font_family = SANS.into();
        k.font_size = px(T_BODY);
        k.mono_font_family = MONO.into();
        k.radius = px(R_CONTROL);
        k.radius_lg = px(R_WIDGET);
    });
}

/// Makes a clickable element a Tab stop that VoiceOver names (PRD 10). Enter
/// and Space click a focused element; the ring shows only for keyboard focus.
pub fn focusable<E: StatefulInteractiveElement>(
    el: E,
    t: &Theme,
    role: Role,
    label: impl Into<SharedString>,
) -> E {
    let ring = t.accent;
    el.tab_index(0)
        .role(role)
        .aria_label(label)
        .focus_visible(move |s| s.border_1().border_color(ring))
}

const FALLBACK_CATEGORY: &str = "dev_tools";

/// Label, light (fill, border) and dark (fill, border).
type CategoryEntry = (&'static str, (Hsla, Hsla), (Hsla, Hsla));

/// `#RRGGBB` from `rules.toml`; validation guarantees the format, so black never shows.
fn parse_hex(hex: &str) -> Hsla {
    let v = u32::from_str_radix(hex.trim_start_matches('#'), 16).unwrap_or(0);
    rgb(v).into()
}

/// Category id to its label and colors, parsed once from the embedded rules.
fn category(id: &str) -> &'static CategoryEntry {
    static MAP: OnceLock<HashMap<&'static str, CategoryEntry>> = OnceLock::new();
    let map = MAP.get_or_init(|| {
        Rules::embedded()
            .categories
            .iter()
            .map(|c| {
                let entry = (
                    c.label.as_str(),
                    (parse_hex(&c.fill), parse_hex(&c.border)),
                    (parse_hex(&c.fill_dark), parse_hex(&c.border_dark)),
                );
                (c.id.as_str(), entry)
            })
            .collect()
    });
    map.get(id).unwrap_or_else(|| &map[FALLBACK_CATEGORY])
}

/// `(fill, border)` of a category in theme `t`; unknown ids get `dev_tools`.
pub fn category_colors(t: &Theme, id: &str) -> (Hsla, Hsla) {
    let (_, light, dark) = category(id);
    if t.dark { *dark } else { *light }
}

pub fn category_label(id: &str) -> &'static str {
    category(id).0
}

/// What kind of space a treemap tile is (PRD 13.5, Texture).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Texture {
    /// A ready safe-tier item: caches and build output.
    Disposable,
    /// A review-tier item, or a safe one that is blocked right now.
    Restorable,
    /// User data and projects.
    Kept,
    /// FR-EXP-6: protected, or managed by an app.
    System,
    /// FR-EXP-2: the smaller-items tile.
    Smaller,
}

/// `over` laid on `under` at `alpha`, as one opaque color.
pub fn mix(under: Hsla, over: Hsla, alpha: f32) -> Hsla {
    let (u, o) = (under.to_rgb(), over.to_rgb());
    let m = |u: f32, o: f32| u + (o - u) * alpha;
    gpui::Rgba {
        r: m(u.r, o.r),
        g: m(u.g, o.g),
        b: m(u.b, o.b),
        a: 1.,
    }
    .into()
}

/// A tile's fill and, for disposable space, its stripe color. Textured fills are
/// opaque mixes, so Reduce Transparency leaves them as they are.
pub fn tile_fill(t: &Theme, category: &str, texture: Texture) -> (Hsla, Option<Hsla>) {
    let (fill, border) = category_colors(t, category);
    match texture {
        Texture::Kept => (fill, None),
        Texture::Restorable => (mix(t.content, fill, TEX_MIX), None),
        Texture::Disposable => (mix(t.content, fill, TEX_MIX), Some(border)),
        Texture::System | Texture::Smaller => (t.neutral, None),
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TileColors {
    pub fill: Hsla,
    pub border: Hsla,
    /// Tier color for the hatch overlay, when a recommendation matches.
    pub hatch: Option<Hsla>,
}

pub fn tile_colors(theme: &Theme, node: &ViewNode) -> TileColors {
    let hatch = node.tier.map(|t| match t {
        Tier::Safe => theme.safe,
        Tier::Review => theme.review,
    });
    let (fill, border) = category_colors(theme, &node.category);
    TileColors {
        fill,
        border: hatch.unwrap_or(border),
        hatch,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(v: u32) -> Hsla {
        rgb(v).into()
    }

    #[test]
    fn category_colors_match_prd() {
        let table = [
            ("code_projects", 0xE3E0F7, 0x7B6FE0),
            ("ai_models", 0xF6DDE5, 0xD75F86),
            ("mobile_sdks", 0xD5EBF2, 0x3A9BBA),
            ("package_caches", 0xDBE4F5, 0x5A82D1),
            ("documents_media", 0xF7E3D4, 0xDD8A4F),
            ("dev_tools", 0xEBE5D7, 0xA6926A),
        ];
        let got: Vec<_> = table
            .iter()
            .map(|(id, ..)| category_colors(&Theme::light(), id))
            .collect();
        let want: Vec<_> = table.iter().map(|&(_, f, b)| (hex(f), hex(b))).collect();
        assert_eq!(got, want);
    }

    /// WCAG 2 contrast ratio between two opaque colors.
    fn contrast(a: Hsla, b: Hsla) -> f32 {
        fn luminance(c: Hsla) -> f32 {
            let c = c.to_rgb();
            let lin = |v: f32| {
                if v <= 0.040_45 {
                    v / 12.92
                } else {
                    ((v + 0.055) / 1.055).powf(2.4)
                }
            };
            0.2126 * lin(c.r) + 0.7152 * lin(c.g) + 0.0722 * lin(c.b)
        }
        let (la, lb) = (luminance(a), luminance(b));
        (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
    }

    /// `over` painted at `alpha` on `under`, in sRGB like the GPU blends.
    /// Every surface a tile label sits on, as the treemap draws it: each
    /// texture's fill and, under stripes, the darkest stripe pixel.
    fn tile_surfaces(t: &Theme) -> Vec<Hsla> {
        use Texture::*;
        LEGEND_ORDER
            .iter()
            .flat_map(|id| {
                [Disposable, Restorable, Kept, System, Smaller]
                    .into_iter()
                    .flat_map(move |tex| {
                        let (fill, stripe) = tile_fill(t, id, tex);
                        std::iter::once(fill).chain(stripe.map(|s| mix(fill, s, STRIPE_ALPHA)))
                    })
            })
            .collect()
    }

    /// Every text-on-fill pair the v2 screens use, with a name for the failure message.
    fn pairs(t: &Theme) -> Vec<(String, Hsla, Hsla)> {
        let mut p: Vec<(String, Hsla, Hsla)> = [
            ("label/content", t.label, t.content),
            ("label/widget", t.label, t.widget),
            ("label/popover", t.label, t.popover),
            ("secondary/content", t.secondary, t.content),
            ("secondary/widget", t.secondary, t.widget),
            ("accent_text/content", t.accent_text, t.content),
            ("accent_text/review_tint", t.accent_text, t.review_tint),
            ("label/sidebar_solid", t.label, t.sidebar_solid),
            (
                "sidebar_secondary/sidebar_solid",
                t.sidebar_secondary,
                t.sidebar_solid,
            ),
            ("safe_text/content", t.safe_text, t.content),
            ("safe_text/safe_tint", t.safe_text, t.safe_tint),
            ("review_text/content", t.review_text, t.content),
            ("review_text/review_tint", t.review_text, t.review_tint),
            ("clean_text/clean", t.clean_text, t.clean),
            ("danger/content", t.danger, t.content),
        ]
        .into_iter()
        .map(|(n, a, b)| (n.to_owned(), a, b))
        .collect();
        for (i, s) in tile_surfaces(t).into_iter().enumerate() {
            p.push((format!("label/tile surface {i}"), t.label, s));
            p.push((
                format!("tile_secondary/tile surface {i}"),
                t.tile_secondary,
                s,
            ));
        }
        p
    }

    const AA: f32 = 4.5;

    fn failing(t: &Theme) -> Vec<String> {
        pairs(t)
            .into_iter()
            .filter(|(_, fg, bg)| contrast(*fg, *bg) < AA)
            .map(|(n, fg, bg)| format!("{n}: {:.2}", contrast(fg, bg)))
            .collect()
    }

    #[test]
    fn light_text_pairs_pass_aa() {
        assert_eq!(failing(&Theme::light()), Vec::<String>::new());
    }

    #[test]
    fn dark_text_pairs_pass_aa() {
        assert_eq!(failing(&Theme::dark()), Vec::<String>::new());
    }

    #[test]
    fn dark_mode_uses_the_dark_category_colors() {
        assert_eq!(
            category_colors(&Theme::dark(), "ai_models"),
            (hex(0x4A2C38), hex(0xEE8AAE))
        );
    }

    #[test]
    fn labels_and_fallback() {
        assert_eq!(category_label("ai_models"), "AI models");
        assert_eq!(category_label("nope"), category_label("dev_tools"));
        let t = Theme::light();
        assert_eq!(
            category_colors(&t, "nope"),
            category_colors(&t, "dev_tools")
        );
    }

    #[test]
    fn legend_covers_rule_categories() {
        let mut legend = LEGEND_ORDER.to_vec();
        let mut ids: Vec<&str> = Rules::embedded()
            .categories
            .iter()
            .map(|c| c.id.as_str())
            .collect();
        legend.sort();
        ids.sort();
        assert_eq!(legend, ids);
    }
}
