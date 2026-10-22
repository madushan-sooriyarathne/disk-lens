//! Parsing and the embedded rule set.

use std::sync::OnceLock;

use super::model::Rules;
use super::validate::validate;

const EMBEDDED: &str = include_str!("../../rules/rules.toml");

impl Rules {
    /// Parses and validates; the error lists every problem found.
    pub fn parse(text: &str) -> Result<Rules, String> {
        let rules: Rules = toml::from_str(text).map_err(|e| e.to_string())?;
        let errors = validate(&rules);
        if errors.is_empty() {
            Ok(rules)
        } else {
            Err(errors.join("\n"))
        }
    }

    /// The rules bundled into the binary. Tests keep them valid, so a failure here
    /// is a build defect and stops the app at startup.
    pub fn embedded() -> &'static Rules {
        static RULES: OnceLock<Rules> = OnceLock::new();
        RULES.get_or_init(|| {
            Rules::parse(EMBEDDED).unwrap_or_else(|e| panic!("invalid embedded rules.toml:\n{e}"))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recommend::model::Tier;
    use crate::rules::model::{FallbackMethod, Method, Provider, SplitBy, Target};

    #[test]
    fn embedded_rules_are_valid() {
        let r = Rules::embedded();
        assert_eq!(r.schema_version, 1);
        assert_eq!(r.categories.len(), 6);
        assert_eq!(r.protect.len(), 19);
        assert_eq!(r.rules.len(), 27);
        assert!(r.media_extensions.iter().any(|e| e == "heic"));
    }

    #[test]
    fn defaults_and_typed_fields() {
        let r = Rules::embedded();
        let npm = r.rule("npm-cache").unwrap();
        assert_eq!(npm.tier, Tier::Safe);
        assert_eq!(npm.target, Target::Itself);
        assert_eq!(npm.method, Method::Delete);
        assert!(npm.allow_permanent);
        assert_eq!((npm.min_size_mb, npm.priority), (1, 0));
        assert_eq!(npm.guards.quiet_minutes, Some(10));

        let brew = r.rule("homebrew-cache").unwrap();
        assert_eq!(brew.fallback_method, Some(FallbackMethod::Delete));
        assert_eq!(brew.target, Target::Contents);

        let caches = r.rule("app-caches").unwrap();
        assert_eq!((caches.min_size_mb, caches.priority), (50, -10));
        assert!(caches.guards.owner_app_not_running);

        let wt = r.rule("agent-worktrees").unwrap();
        assert_eq!(wt.provider, Some(Provider::GitWorktrees));
        assert_eq!(wt.split_rows_by, Some(SplitBy::Project));
        assert!(wt.guards.git_clean);

        assert!(!r.rule("ios-backups").unwrap().allow_permanent);
        assert_eq!(r.rule("trash-bin").unwrap().tier, Tier::Review);
    }
}
