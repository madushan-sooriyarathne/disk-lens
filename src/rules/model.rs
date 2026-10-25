//! Typed form of `rules/rules.toml` (RULES.md sections 3 and 7). Unknown fields,
//! guards, providers, tiers and methods are rejected by serde.

use serde::de::Error as _;
use serde::{Deserialize, Deserializer};

use crate::recommend::model::Tier;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rules {
    pub schema_version: u32,
    pub media_extensions: Vec<String>,
    #[serde(rename = "category")]
    pub categories: Vec<Category>,
    #[serde(rename = "protect", default)]
    pub protect: Vec<Protect>,
    #[serde(rename = "rule")]
    pub rules: Vec<Rule>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Category {
    pub id: String,
    pub label: String,
    /// One line for nodes no rule explains (PRD goal G2).
    pub description: String,
    pub fill: String,
    pub border: String,
    #[serde(default)]
    pub markers: Vec<String>,
    #[serde(default)]
    pub paths: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Protect {
    pub path: String,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Target {
    #[default]
    #[serde(rename = "self")]
    Itself,
    Contents,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Method {
    Delete,
    Trash,
    Command,
}

/// `fallback_method` may only name a file operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FallbackMethod {
    Delete,
    Trash,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Provider {
    Ollama,
    GitWorktrees,
    SimctlUnavailable,
    Docker,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SplitBy {
    Project,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Guards {
    // match-time
    pub sibling_exists: Vec<String>,
    pub project_idle_days: Option<u32>,
    pub requires_tool: Option<String>,
    // blocking
    pub quiet_minutes: Option<u32>,
    pub process_not_running: Vec<String>,
    pub app_not_running: Vec<String>,
    pub owner_app_not_running: bool,
    pub requires_running: Vec<String>,
    // safety
    pub git_clean: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    pub id: String,
    pub label: String,
    pub description: String,
    pub regenerates: String,
    pub category: String,
    #[serde(deserialize_with = "tier")]
    pub tier: Tier,
    pub paths: Vec<String>,
    #[serde(default)]
    pub exclude: Vec<String>,
    #[serde(default)]
    pub target: Target,
    pub method: Method,
    pub command: Option<Vec<String>>,
    pub fallback_method: Option<FallbackMethod>,
    pub provider: Option<Provider>,
    #[serde(default)]
    pub guards: Guards,
    #[serde(default = "yes")]
    pub allow_permanent: bool,
    #[serde(default)]
    pub size_is_estimate: bool,
    #[serde(default = "one")]
    pub min_size_mb: u64,
    #[serde(default)]
    pub priority: i32,
    pub split_rows_by: Option<SplitBy>,
    pub row_label: Option<String>,
}

/// `rules.toml` spells tiers in lowercase; `never` is declared with `[[protect]]` instead.
fn tier<'de, D: Deserializer<'de>>(d: D) -> Result<Tier, D::Error> {
    match String::deserialize(d)?.as_str() {
        "safe" => Ok(Tier::Safe),
        "review" => Ok(Tier::Review),
        other => Err(D::Error::unknown_variant(other, &["safe", "review"])),
    }
}

fn yes() -> bool {
    true
}

fn one() -> u64 {
    1
}

impl Rules {
    pub fn rule(&self, id: &str) -> Option<&Rule> {
        self.rules.iter().find(|r| r.id == id)
    }

    pub fn category(&self, id: &str) -> Option<&Category> {
        self.categories.iter().find(|c| c.id == id)
    }
}
