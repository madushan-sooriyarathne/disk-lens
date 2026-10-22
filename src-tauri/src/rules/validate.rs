//! Semantic checks serde cannot express (RULES.md section 3). Unknown fields,
//! guard names, providers, tiers and methods are already rejected while parsing.

use std::collections::HashSet;

use super::model::{Method, Rules};
use super::{compile, expand};

const SCHEMA_VERSION: u32 = 1;
const PLACEHOLDERS: [&str; 3] = ["{path}", "{name}", "{main_repo}"];
/// Any absolute root works for checking that globs compile.
const CHECK_ROOT: &str = "/h";

/// Every problem found, empty when the rules are valid.
pub fn validate(r: &Rules) -> Vec<String> {
    let mut errors = Vec::new();
    if r.schema_version != SCHEMA_VERSION {
        errors.push(format!("unsupported schema_version {}", r.schema_version));
    }

    let mut category_ids = HashSet::new();
    for c in &r.categories {
        if !category_ids.insert(c.id.as_str()) {
            errors.push(format!("duplicate category id `{}`", c.id));
        }
        for p in &c.paths {
            if !p.starts_with("~/") {
                errors.push(format!(
                    "category `{}`: path `{p}` must start with ~/",
                    c.id
                ));
            }
        }
    }

    for p in &r.protect {
        check_glob(&mut errors, "protect", &p.path);
        if !p.path.starts_with("~/") {
            errors.push(format!("protect: path `{}` must start with ~/", p.path));
        }
    }

    let mut rule_ids = HashSet::new();
    for rule in &r.rules {
        let id = &rule.id;
        let at = format!("rule `{id}`");
        if !rule_ids.insert(id.as_str()) {
            errors.push(format!("duplicate rule id `{id}`"));
        }
        if id.is_empty()
            || !id
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        {
            errors.push(format!("{at}: id must be kebab-case"));
        }
        if !category_ids.contains(rule.category.as_str()) {
            errors.push(format!("{at}: unknown category `{}`", rule.category));
        }
        if rule.paths.is_empty() {
            errors.push(format!("{at}: paths is empty"));
        }
        for p in &rule.paths {
            // Anchored at the scan root, so a rule can never reach outside it.
            if !p.starts_with("~/") {
                errors.push(format!("{at}: path `{p}` must start with ~/"));
            }
            check_glob(&mut errors, &at, p);
        }
        for p in &rule.exclude {
            check_glob(&mut errors, &at, p);
        }
        match (rule.method, &rule.command) {
            (Method::Command, None) => errors.push(format!("{at}: method command needs `command`")),
            (Method::Command, Some(argv)) => {
                if argv.is_empty() || argv[0].is_empty() {
                    errors.push(format!("{at}: command is empty"));
                }
                for arg in argv {
                    check_placeholders(&mut errors, &at, arg);
                }
            }
            (_, Some(_)) => errors.push(format!(
                "{at}: `command` is only allowed with method command"
            )),
            (_, None) => {}
        }
        if rule.fallback_method.is_some() && rule.guards.requires_tool.is_none() {
            errors.push(format!("{at}: fallback_method needs guards.requires_tool"));
        }
        if rule.row_label.is_some() && rule.split_rows_by.is_none() {
            errors.push(format!("{at}: row_label needs split_rows_by"));
        }
        if rule.min_size_mb == 0 {
            errors.push(format!("{at}: min_size_mb must be at least 1"));
        }
    }
    errors
}

fn check_glob(errors: &mut Vec<String>, at: &str, pattern: &str) {
    if let Err(e) = compile(&expand(pattern, CHECK_ROOT)) {
        errors.push(format!("{at}: glob `{pattern}` does not compile: {e}"));
    }
}

fn check_placeholders(errors: &mut Vec<String>, at: &str, arg: &str) {
    let mut rest = arg;
    while let Some(start) = rest.find('{') {
        let Some(len) = rest[start..].find('}') else {
            errors.push(format!("{at}: unclosed placeholder in `{arg}`"));
            return;
        };
        let token = &rest[start..start + len + 1];
        if !PLACEHOLDERS.contains(&token) {
            errors.push(format!("{at}: unknown placeholder `{token}`"));
        }
        rest = &rest[start + len + 1..];
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::model::Rules;

    const BASE: &str = r##"
schema_version = 1
media_extensions = ["jpg"]

[[category]]
id = "package_caches"
label = "Package caches"
fill = "#000000"
border = "#000000"
paths = ["~/.npm"]

[[protect]]
path = "~/.ssh"
reason = "keys"

[[rule]]
id = "npm-cache"
label = "npm cache"
description = "d"
regenerates = "r"
category = "package_caches"
tier = "safe"
paths = ["~/.npm/_cacache"]
method = "delete"
"##;

    /// `BASE` plus `extra` appended to the end (inside the last `[[rule]]`).
    fn parse_with(extra: &str) -> Result<Rules, String> {
        Rules::parse(&format!("{BASE}{extra}\n"))
    }

    fn rejects(extra: &str, needle: &str) {
        let err = parse_with(extra).expect_err(extra);
        assert!(
            err.contains(needle),
            "`{extra}`: expected `{needle}` in: {err}"
        );
    }

    const SECOND_RULE: &str = r#"
[[rule]]
id = "other"
label = "l"
description = "d"
regenerates = "r"
category = "package_caches"
tier = "review"
paths = ["~/x"]
method = "trash"
"#;

    #[test]
    fn base_is_valid() {
        let r = parse_with("").unwrap();
        assert_eq!(r.rules.len(), 1);
        assert!(parse_with(SECOND_RULE).is_ok());
    }

    #[test]
    fn rejects_unknown_fields_everywhere() {
        rejects("colour = \"red\"", "unknown field `colour`");
        let top = BASE.replace("schema_version = 1", "schema_version = 1\nextra = 1");
        assert!(
            Rules::parse(&top)
                .unwrap_err()
                .contains("unknown field `extra`")
        );
        let cat = BASE.replace("label = \"Package caches\"", "label = \"P\"\nicon = \"x\"");
        assert!(
            Rules::parse(&cat)
                .unwrap_err()
                .contains("unknown field `icon`")
        );
        let prot = BASE.replace("reason = \"keys\"", "reason = \"keys\"\nwhy = 1");
        assert!(
            Rules::parse(&prot)
                .unwrap_err()
                .contains("unknown field `why`")
        );
    }

    #[test]
    fn rejects_malformed_toml() {
        assert!(Rules::parse("schema_version = ").is_err());
        assert!(Rules::parse("[[rule]\nid = 1").is_err());
        assert!(Rules::parse("").unwrap_err().contains("missing field"));
    }

    #[test]
    fn rejects_missing_required_field() {
        let no_label = BASE.replace("label = \"npm cache\"\n", "");
        assert!(
            Rules::parse(&no_label)
                .unwrap_err()
                .contains("missing field `label`")
        );
    }

    #[test]
    fn rejects_duplicate_ids() {
        let dup = SECOND_RULE.replace("id = \"other\"", "id = \"npm-cache\"");
        rejects(&dup, "duplicate rule id `npm-cache`");
    }

    #[test]
    fn rejects_unknown_category() {
        let r = BASE.replace("category = \"package_caches\"", "category = \"nope\"");
        assert!(
            Rules::parse(&r)
                .unwrap_err()
                .contains("unknown category `nope`")
        );
    }

    #[test]
    fn rejects_unknown_tier_and_method() {
        let never = BASE.replace("tier = \"safe\"", "tier = \"never\"");
        assert!(
            Rules::parse(&never)
                .unwrap_err()
                .contains("unknown variant `never`")
        );
        let shred = BASE.replace("method = \"delete\"", "method = \"shred\"");
        assert!(
            Rules::parse(&shred)
                .unwrap_err()
                .contains("unknown variant `shred`")
        );
    }

    #[test]
    fn rejects_command_mismatch() {
        let missing = BASE.replace("method = \"delete\"", "method = \"command\"");
        assert!(
            Rules::parse(&missing)
                .unwrap_err()
                .contains("needs `command`")
        );
        rejects("command = [\"rm\"]", "only allowed with method command");
        let empty = BASE.replace("method = \"delete\"", "method = \"command\"\ncommand = []");
        assert!(
            Rules::parse(&empty)
                .unwrap_err()
                .contains("command is empty")
        );
    }

    #[test]
    fn rejects_unknown_placeholder() {
        let r = BASE.replace(
            "method = \"delete\"",
            "method = \"command\"\ncommand = [\"x\", \"{home}\"]",
        );
        assert!(
            Rules::parse(&r)
                .unwrap_err()
                .contains("unknown placeholder `{home}`")
        );
        let ok = BASE.replace(
            "method = \"delete\"",
            "method = \"command\"\ncommand = [\"git\", \"-C\", \"{main_repo}\", \"x\", \"{path}\"]",
        );
        assert!(Rules::parse(&ok).is_ok());
    }

    #[test]
    fn rejects_unknown_guard() {
        rejects(
            "guards = { quiet_hours = 1 }",
            "unknown field `quiet_hours`",
        );
    }

    #[test]
    fn rejects_unknown_provider() {
        rejects("provider = \"npm\"", "unknown variant `npm`");
    }

    #[test]
    fn rejects_bad_globs() {
        let r = BASE.replace("[\"~/.npm/_cacache\"]", "[\"~/.npm/[abc\"]");
        assert!(Rules::parse(&r).unwrap_err().contains("does not compile"));
        rejects("exclude = [\"~/{a,b\"]", "does not compile");
        let p = BASE.replace("path = \"~/.ssh\"", "path = \"~/[x\"");
        assert!(Rules::parse(&p).unwrap_err().contains("does not compile"));
    }

    #[test]
    fn rejects_unanchored_rule_paths() {
        let r = BASE.replace("[\"~/.npm/_cacache\"]", "[\"/etc/x\"]");
        assert!(Rules::parse(&r).unwrap_err().contains("must start with ~/"));
        let r = BASE.replace("[\"~/.npm/_cacache\"]", "[\"**/cache\"]");
        assert!(Rules::parse(&r).unwrap_err().contains("must start with ~/"));
    }

    #[test]
    fn rejects_fallback_without_requires_tool() {
        rejects("fallback_method = \"delete\"", "needs guards.requires_tool");
        rejects("fallback_method = \"command\"", "unknown variant `command`");
        let ok = BASE.replace(
            "method = \"delete\"",
            "method = \"command\"\ncommand = [\"brew\"]\nfallback_method = \"delete\"\nguards = { requires_tool = \"brew\" }",
        );
        assert!(Rules::parse(&ok).is_ok());
    }

    #[test]
    fn rejects_bad_split_and_label() {
        rejects("split_rows_by = \"owner\"", "unknown variant `owner`");
        rejects("row_label = \"{project}\"", "row_label needs split_rows_by");
    }

    #[test]
    fn rejects_bad_ids_and_sizes() {
        let r = BASE.replace("id = \"npm-cache\"", "id = \"Npm_Cache\"");
        assert!(Rules::parse(&r).unwrap_err().contains("kebab-case"));
        rejects("min_size_mb = 0", "min_size_mb must be at least 1");
        let v = BASE.replace("schema_version = 1", "schema_version = 2");
        assert!(Rules::parse(&v).unwrap_err().contains("schema_version 2"));
    }

    #[test]
    fn reports_every_error() {
        let r = BASE
            .replace("category = \"package_caches\"", "category = \"nope\"")
            .replace("[\"~/.npm/_cacache\"]", "[\"/abs\"]");
        let err = Rules::parse(&r).unwrap_err();
        assert_eq!(err.lines().count(), 2, "{err}");
    }
}
