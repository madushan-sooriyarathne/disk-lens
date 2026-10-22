//! Rules engine: `rules.toml` model, validation, categories and matching (RULES.md).

pub mod category;
pub mod load;
pub mod model;
pub mod validate;

use globset::{Glob, GlobBuilder};

/// Expands a leading `~` to `root`, escaped so characters in the root are literal.
pub fn expand(pattern: &str, root: &str) -> String {
    let root = globset::escape(root).replace('\\', "\\\\");
    match pattern.strip_prefix('~') {
        Some(rest) if rest.is_empty() || rest.starts_with('/') => format!("{root}{rest}"),
        _ => pattern.to_owned(),
    }
}

/// Compiles a rule glob: `*` never crosses `/`, `**` spans whole segments.
pub fn compile(pattern: &str) -> Result<Glob, globset::Error> {
    GlobBuilder::new(pattern).literal_separator(true).build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_tilde_and_escapes_root() {
        assert_eq!(expand("~/.npm", "/Users/x"), "/Users/x/.npm");
        assert_eq!(expand("~", "/Users/x"), "/Users/x");
        assert_eq!(
            expand("**/node_modules/**", "/Users/x"),
            "**/node_modules/**"
        );
        assert_eq!(expand("~x/a", "/Users/x"), "~x/a");
        assert_eq!(expand("~/a", "/tmp/[w]*"), "/tmp/[[]w[]][*]/a");
    }

    #[test]
    fn escaped_root_matches_literally() {
        let g = compile(&expand("~/a/*", "/tmp/we*rd{x}"))
            .unwrap()
            .compile_matcher();
        assert!(g.is_match("/tmp/we*rd{x}/a/b"));
        assert!(!g.is_match("/tmp/weXXrdx/a/b"));
    }
}
