//! Colors, fonts and tile colors. Category colors and labels come from `rules.toml`.

use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::OnceLock;

use gpui::{App, Hsla, Window, WindowAppearance, rgb};

use crate::recommend::model::Tier;
use crate::rules::model::Rules;
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
    pub error: Hsla,
    /// Picks the `*_dark` category colors from rules.toml.
    pub dark: bool,
}

impl Theme {
    pub fn light() -> Self {
        let c = |hex: u32| Hsla::from(rgb(hex));
        Self {
            bg: c(0xf7f7f8),
            card: c(0xffffff),
            card_border: c(0xe6e6e9),
            text: c(0x18181b),
            muted: c(0x6b6b73),
            divider: c(0xececef),
            safe: c(0x22a06b),
            review: c(0xe2a03f),
            safe_fill: c(0xdff3ea),
            review_fill: c(0xfbefd9),
            error: c(0xdc2626),
            dark: false,
        }
    }

    pub fn dark() -> Self {
        let c = |hex: u32| Hsla::from(rgb(hex));
        Self {
            bg: c(0x111113),
            card: c(0x1c1c1f),
            card_border: c(0x2e2e33),
            text: c(0xededef),
            muted: c(0xa1a1aa),
            divider: c(0x27272a),
            safe: c(0x3fbf86),
            review: c(0xe8b04f),
            safe_fill: c(0x173226),
            review_fill: c(0x3a2d14),
            error: c(0xf87171),
            dark: true,
        }
    }
}

/// The theme for the window's appearance (PRD section 9: dark mode).
pub fn current(window: &Window) -> Theme {
    match window.appearance() {
        WindowAppearance::Dark | WindowAppearance::VibrantDark => Theme::dark(),
        WindowAppearance::Light | WindowAppearance::VibrantLight => Theme::light(),
    }
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

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TileColors {
    pub fill: Hsla,
    pub border: Hsla,
    /// Tier color for the hatch overlay, when a recommendation matches.
    pub hatch: Option<Hsla>,
}

pub fn tile_colors(theme: &Theme, node: &ViewNode, mode: ViewMode) -> TileColors {
    let hatch = node.tier.map(|t| match t {
        Tier::Safe => theme.safe,
        Tier::Review => theme.review,
    });
    match mode {
        ViewMode::Cleanup => {
            let (fill, border) = category_colors(theme, FALLBACK_CATEGORY);
            let tint = node.tier.map(|t| match t {
                Tier::Safe => theme.safe_fill,
                Tier::Review => theme.review_fill,
            });
            TileColors {
                fill: tint.unwrap_or(fill),
                border: hatch.unwrap_or(border),
                hatch,
            }
        }
        ViewMode::Category => {
            let (fill, border) = category_colors(theme, &node.category);
            TileColors {
                fill,
                border: hatch.unwrap_or(border),
                hatch,
            }
        }
    }
}

/// Registers the embedded Geist and Geist Mono fonts.
pub fn load_fonts(cx: &App) {
    let fonts = [
        &include_bytes!("../../assets/fonts/Geist-Regular.ttf")[..],
        &include_bytes!("../../assets/fonts/Geist-Medium.ttf")[..],
        &include_bytes!("../../assets/fonts/Geist-SemiBold.ttf")[..],
        &include_bytes!("../../assets/fonts/GeistMono-Regular.ttf")[..],
        &include_bytes!("../../assets/fonts/GeistMono-Medium.ttf")[..],
        &include_bytes!("../../assets/fonts/GeistMono-SemiBold.ttf")[..],
    ];
    if let Err(e) = cx
        .text_system()
        .add_fonts(fonts.into_iter().map(Cow::Borrowed).collect())
    {
        tracing::warn!("failed to load Geist fonts: {e:#}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view::ViewKind;

    fn hex(v: u32) -> Hsla {
        rgb(v).into()
    }

    fn node(category: &str, tier: Option<Tier>) -> ViewNode {
        ViewNode {
            id: "n".into(),
            name: "n".into(),
            path: "/n".into(),
            size: 1,
            category: category.into(),
            tier,
            rule_id: None,
            kind: ViewKind::Dir,
            children: vec![],
        }
    }

    #[test]
    fn category_colors_match_prd() {
        let table = [
            ("code_projects", 0xE6E4FB, 0x8E84E8),
            ("ai_models", 0xFBDDE6, 0xE58AA7),
            ("mobile_sdks", 0xD9EEF4, 0x6FB7CC),
            ("package_caches", 0xDCE5F7, 0x7C9BD8),
            ("documents_media", 0xFCE6D8, 0xEFA57A),
            ("dev_tools", 0xEEEEF0, 0xC4C4C9),
        ];
        for (id, fill, border) in table {
            assert_eq!(
                category_colors(&Theme::light(), id),
                (hex(fill), hex(border)),
                "{id}"
            );
        }
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

    /// The lowest contrast of `fg` against every category fill in `t`.
    fn worst_on_category_fills(t: &Theme, fg: Hsla) -> f32 {
        LEGEND_ORDER
            .iter()
            .map(|id| contrast(fg, category_colors(t, id).0))
            .fold(f32::MAX, f32::min)
    }

    const AA: f32 = 4.5;

    #[test]
    fn light_text_is_aa_on_bg() {
        let t = Theme::light();
        assert!(contrast(t.text, t.bg) >= AA);
    }

    #[test]
    fn light_muted_is_aa_on_card() {
        let t = Theme::light();
        assert!(contrast(t.muted, t.card) >= AA);
    }

    #[test]
    fn light_muted_is_aa_on_bg() {
        let t = Theme::light();
        assert!(contrast(t.muted, t.bg) >= AA);
    }

    #[test]
    fn dark_text_is_aa_on_bg() {
        let t = Theme::dark();
        assert!(contrast(t.text, t.bg) >= AA);
    }

    #[test]
    fn dark_muted_is_aa_on_card() {
        let t = Theme::dark();
        assert!(contrast(t.muted, t.card) >= AA);
    }

    #[test]
    fn dark_muted_is_aa_on_bg() {
        let t = Theme::dark();
        assert!(contrast(t.muted, t.bg) >= AA);
    }

    #[test]
    fn light_text_is_aa_on_every_category_fill() {
        let t = Theme::light();
        assert!(worst_on_category_fills(&t, t.text) >= AA);
    }

    #[test]
    fn dark_text_is_aa_on_every_category_fill() {
        let t = Theme::dark();
        assert!(worst_on_category_fills(&t, t.text) >= AA);
    }

    #[test]
    fn dark_error_is_aa_on_card() {
        let t = Theme::dark();
        assert!(contrast(t.error, t.card) >= AA);
    }

    #[test]
    fn dark_mode_uses_the_dark_category_colors() {
        assert_eq!(
            category_colors(&Theme::dark(), "ai_models"),
            (hex(0x45232F), hex(0xE58AA7))
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

    #[test]
    fn tile_colors_by_mode_and_tier() {
        let t = Theme::light();
        let (dev_fill, dev_border) = category_colors(&t, "dev_tools");
        let (ai_fill, ai_border) = category_colors(&t, "ai_models");
        let parts = |c: TileColors| (c.fill, c.border, c.hatch);

        let c = tile_colors(&t, &node("ai_models", None), ViewMode::Cleanup);
        assert_eq!(parts(c), (dev_fill, dev_border, None));
        let c = tile_colors(&t, &node("ai_models", Some(Tier::Safe)), ViewMode::Cleanup);
        assert_eq!(parts(c), (t.safe_fill, t.safe, Some(t.safe)));
        let c = tile_colors(
            &t,
            &node("ai_models", Some(Tier::Review)),
            ViewMode::Cleanup,
        );
        assert_eq!(parts(c), (t.review_fill, t.review, Some(t.review)));

        let c = tile_colors(&t, &node("ai_models", None), ViewMode::Category);
        assert_eq!(parts(c), (ai_fill, ai_border, None));
        let c = tile_colors(
            &t,
            &node("ai_models", Some(Tier::Review)),
            ViewMode::Category,
        );
        assert_eq!(parts(c), (ai_fill, t.review, Some(t.review)));
    }
}
