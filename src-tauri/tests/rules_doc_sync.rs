//! `rules/rules.toml` must equal the TOML block in docs/RULES.md section 7.

#[test]
fn rules_toml_matches_rules_md_section_7() {
    let doc = include_str!("../../docs/RULES.md");
    let section = doc
        .split("## 7. Starter rules")
        .nth(1)
        .expect("section 7 present");
    let block = section
        .split("```toml\n")
        .nth(1)
        .and_then(|s| s.split("\n```\n").next())
        .expect("toml block present");
    let file = include_str!("../rules/rules.toml");
    assert_eq!(file.trim_end(), block.trim_end());
}
