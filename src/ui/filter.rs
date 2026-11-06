//! The palette's filter grammar (PRD FR-KB-3): `size>1GB`, `size<100MB`,
//! `tier:safe`, `tier:review`, `category:<id>` and `idle>30d`, separated by spaces.
//! Every term must hold. Explore dims the tiles that don't match and Recommended
//! hides the rows that don't.

use std::fmt;

use crate::recommend::model::{Recommendation, Tier};
use crate::rules::model::Rules;
use crate::view::ViewNode;

#[derive(Debug, Clone, PartialEq)]
pub struct Filter {
    terms: Vec<Term>,
    /// The terms as typed, for the toolbar token.
    text: String,
}

#[derive(Debug, Clone, PartialEq)]
enum Term {
    SizeAbove(u64),
    SizeBelow(u64),
    Tier(Tier),
    Category(String),
    IdleAbove(u32),
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FilterError {
    #[error("Type a filter, such as size>1GB or tier:safe")]
    Empty,
    #[error("Unknown filter term: {0}")]
    Unknown(String),
    #[error("Not a size: {0} (use B, KB, MB, GB or TB)")]
    Size(String),
    #[error("Not a number of days: {0} (use idle>30d)")]
    Days(String),
    #[error("Unknown tier: {0} (use safe or review)")]
    Tier(String),
    #[error("Unknown category: {0}")]
    Category(String),
}

/// Decimal units, as `format_bytes` shows them.
const UNITS: [(&str, u64); 5] = [
    ("TB", 1_000_000_000_000),
    ("GB", 1_000_000_000),
    ("MB", 1_000_000),
    ("KB", 1_000),
    ("B", 1),
];

impl Filter {
    pub fn parse(text: &str, rules: &Rules) -> Result<Self, FilterError> {
        let words: Vec<&str> = text.split_whitespace().collect();
        if words.is_empty() {
            return Err(FilterError::Empty);
        }
        let terms = words
            .iter()
            .map(|w| term(w, rules))
            .collect::<Result<_, _>>()?;
        Ok(Self {
            terms,
            text: words.join(" "),
        })
    }

    /// `idle` is the idle age of the recommended item at the tile, if any.
    pub fn matches_tile(&self, node: &ViewNode, idle: Option<u32>) -> bool {
        self.matches(node.size, node.tier, &node.category, idle)
    }

    /// `category` is the category of the row's rule. A row is as idle as its
    /// most idle item.
    pub fn matches_row(&self, row: &Recommendation, category: &str) -> bool {
        let idle = row.items.iter().filter_map(|i| i.idle_days).max();
        self.matches(row.size, Some(row.tier), category, idle)
    }

    fn matches(&self, size: u64, tier: Option<Tier>, category: &str, idle: Option<u32>) -> bool {
        self.terms.iter().all(|t| match t {
            Term::SizeAbove(n) => size > *n,
            Term::SizeBelow(n) => size < *n,
            Term::Tier(want) => tier == Some(*want),
            Term::Category(id) => category == id,
            // Without an age (no project evidence) nothing counts as idle.
            Term::IdleAbove(days) => idle.is_some_and(|d| d > *days),
        })
    }
}

impl fmt::Display for Filter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text)
    }
}

fn term(word: &str, rules: &Rules) -> Result<Term, FilterError> {
    let lower = word.to_ascii_lowercase();
    if let Some(v) = lower.strip_prefix("size>") {
        return size(v, word).map(Term::SizeAbove);
    }
    if let Some(v) = lower.strip_prefix("size<") {
        return size(v, word).map(Term::SizeBelow);
    }
    if let Some(v) = lower.strip_prefix("idle>") {
        return v
            .strip_suffix('d')
            .and_then(|n| n.parse().ok())
            .map(Term::IdleAbove)
            .ok_or_else(|| FilterError::Days(word.into()));
    }
    if let Some(v) = lower.strip_prefix("tier:") {
        return match v {
            "safe" => Ok(Term::Tier(Tier::Safe)),
            "review" => Ok(Term::Tier(Tier::Review)),
            _ => Err(FilterError::Tier(word.into())),
        };
    }
    if let Some(v) = lower.strip_prefix("category:") {
        return rules
            .categories
            .iter()
            .find(|c| c.id == v)
            .map(|c| Term::Category(c.id.clone()))
            .ok_or_else(|| FilterError::Category(word.into()));
    }
    Err(FilterError::Unknown(word.into()))
}

/// `1.5gb` -> 1.5 GB in bytes. A bare number is bytes.
fn size(v: &str, word: &str) -> Result<u64, FilterError> {
    let err = || FilterError::Size(word.into());
    let upper = v.to_ascii_uppercase();
    let (number, unit) = UNITS
        .iter()
        .find_map(|&(u, n)| upper.strip_suffix(u).map(|num| (num, n)))
        .unwrap_or((upper.as_str(), 1));
    let value: f64 = number.trim().parse().map_err(|_| err())?;
    if !value.is_finite() || value < 0.0 {
        return Err(err());
    }
    // Sizes past u64 saturate, which still means "larger than everything".
    Ok((value * unit as f64) as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recommend::model::{ItemStatus, MethodView, RecItem};
    use crate::view::ViewKind;

    fn parse(s: &str) -> Result<Filter, FilterError> {
        Filter::parse(s, Rules::embedded())
    }

    fn tile(size: u64, tier: Option<Tier>, category: &str) -> ViewNode {
        ViewNode {
            id: "1".into(),
            name: "t".into(),
            path: "/r/t".into(),
            size,
            category: category.into(),
            tier,
            rule_id: None,
            kind: ViewKind::Dir,
            file_count: 0,
            cleanable: 0,
            system_managed: false,
            children: Vec::new(),
        }
    }

    fn row(size: u64, tier: Tier, idle: &[Option<u32>]) -> Recommendation {
        Recommendation {
            row_id: "r".into(),
            rule_id: "r".into(),
            label: "r".into(),
            paths_hint: String::new(),
            tier,
            size,
            size_is_estimate: false,
            items: idle
                .iter()
                .map(|&idle_days| RecItem {
                    item_id: "i".into(),
                    path: "/i".into(),
                    name: "i".into(),
                    size: 1,
                    status: ItemStatus::Ready,
                    method: MethodView::Delete,
                    idle_days,
                    active_work: None,
                    rehydrate: None,
                })
                .collect(),
        }
    }

    const GB: u64 = 1_000_000_000;

    #[test]
    fn size_above_compares_in_decimal_units() {
        let f = parse("size>1GB").unwrap();
        assert!(f.matches_tile(&tile(GB + 1, None, "x"), None));
    }

    #[test]
    fn size_above_is_strict() {
        let f = parse("size>1GB").unwrap();
        assert!(!f.matches_tile(&tile(GB, None, "x"), None));
    }

    #[test]
    fn size_below_accepts_lowercase_and_decimals() {
        let f = parse("size<1.5gb").unwrap();
        assert!(f.matches_tile(&tile(GB, None, "x"), None));
    }

    #[test]
    fn size_without_a_unit_is_bytes() {
        let f = parse("size<10").unwrap();
        assert!(!f.matches_tile(&tile(10, None, "x"), None));
    }

    #[test]
    fn tier_matches_only_that_tier() {
        let f = parse("tier:review").unwrap();
        assert!(!f.matches_tile(&tile(1, Some(Tier::Safe), "x"), None));
    }

    #[test]
    fn tier_never_matches_a_tile_without_one() {
        let f = parse("tier:safe").unwrap();
        assert!(!f.matches_tile(&tile(1, None, "x"), None));
    }

    #[test]
    fn category_matches_a_rules_category() {
        let f = parse("category:ai_models").unwrap();
        assert!(f.matches_tile(&tile(1, None, "ai_models"), None));
    }

    #[test]
    fn idle_needs_an_age() {
        let f = parse("idle>30d").unwrap();
        assert!(!f.matches_tile(&tile(1, None, "x"), None));
    }

    #[test]
    fn idle_compares_whole_days() {
        let f = parse("idle>30d").unwrap();
        assert!(f.matches_tile(&tile(1, None, "x"), Some(31)));
    }

    #[test]
    fn a_row_is_as_idle_as_its_most_idle_item() {
        let f = parse("idle>30d").unwrap();
        assert!(f.matches_row(&row(1, Tier::Review, &[Some(2), None, Some(45)]), "x"));
    }

    #[test]
    fn every_term_must_hold() {
        let f = parse("size>1GB tier:safe").unwrap();
        assert!(!f.matches_row(&row(2 * GB, Tier::Review, &[]), "x"));
    }

    #[test]
    fn the_token_shows_the_terms_as_typed() {
        assert_eq!(
            parse("  size>1GB   tier:safe ").unwrap().to_string(),
            "size>1GB tier:safe"
        );
    }

    #[test]
    fn an_empty_filter_is_refused() {
        assert_eq!(parse("  "), Err(FilterError::Empty));
    }

    #[test]
    fn an_unknown_term_is_named() {
        assert_eq!(
            parse("color:red"),
            Err(FilterError::Unknown("color:red".into()))
        );
    }

    #[test]
    fn a_bad_unit_is_refused() {
        assert_eq!(parse("size>1PB"), Err(FilterError::Size("size>1PB".into())));
    }

    #[test]
    fn a_missing_number_is_refused() {
        assert_eq!(parse("size>GB"), Err(FilterError::Size("size>GB".into())));
    }

    #[test]
    fn a_negative_size_is_refused() {
        assert_eq!(
            parse("size>-1GB"),
            Err(FilterError::Size("size>-1GB".into()))
        );
    }

    #[test]
    fn idle_needs_days() {
        assert_eq!(parse("idle>30"), Err(FilterError::Days("idle>30".into())));
    }

    #[test]
    fn an_unknown_tier_is_refused() {
        assert_eq!(parse("tier:all"), Err(FilterError::Tier("tier:all".into())));
    }

    #[test]
    fn an_unknown_category_is_refused() {
        assert_eq!(
            parse("category:music"),
            Err(FilterError::Category("category:music".into()))
        );
    }
}
