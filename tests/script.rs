//! Script export (PRD section 7.7): shape, quoting, and agreement with what a
//! dry run of the same plan logs.

mod support;

use std::borrow::Cow;

use disklens_lib::cleanup::model::{CleanupPlan, PlannedMethod};
use disklens_lib::cleanup::script::render;
use shell_escape::unix::escape;
use support::World;

const GENERATED: i64 = 1_792_833_900;

fn world() -> World {
    let mut w = World::new();
    w.tool("brew");
    w.put(".npm/_cacache", 2);
    w.put(".Trash/old", 2);
    w.put(".cache/huggingface/hub", 2);
    w.put("Library/Developer/Xcode/Archives/2026-01-01", 2);
    w.put("Library/Caches/Homebrew/downloads", 2);
    w.scan();
    w
}

fn plan(w: &World) -> CleanupPlan {
    w.plan_rules(&[
        "npm-cache",
        "trash-bin",
        "huggingface-cache",
        "xcode-archives",
        "homebrew-cache",
    ])
}

fn quoted(path: &str) -> String {
    escape(Cow::Borrowed(path)).into_owned()
}

/// The command lines of each step, keyed by the item ID comment above them.
fn steps(script: &str) -> Vec<(String, Vec<String>)> {
    let mut out: Vec<(String, Vec<String>)> = Vec::new();
    for line in script.lines() {
        match (line.strip_prefix("# Item "), out.last_mut()) {
            (Some(id), _) => out.push((id.to_owned(), Vec::new())),
            (None, Some((_, lines))) if !line.is_empty() && !line.starts_with("echo ") => {
                lines.push(line.to_owned());
            }
            _ => {}
        }
    }
    out
}

#[test]
fn starts_with_bash_and_strict_mode() {
    let w = world();
    assert!(render(&plan(&w), GENERATED).starts_with("#!/bin/bash\nset -euo pipefail\n"));
}

#[test]
fn header_names_version_date_total_and_count() {
    let w = world();
    let p = plan(&w);
    let script = render(&p, GENERATED);
    let expected = format!(
        "# DiskLens {} cleanup script\n# Generated 2026-10-24 09:25 UTC\n# Total 10.0 MB in 5 items\n",
        env!("CARGO_PKG_VERSION")
    );
    assert!(script.contains(&expected), "{script}");
}

#[test]
fn delete_uses_rm_with_end_of_options() {
    let w = world();
    let script = render(&plan(&w), GENERATED);
    let path = w.home.join(".npm/_cacache").display().to_string();
    assert!(script.contains(&format!("\nrm -rf -- {}\n", quoted(&path))));
}

#[test]
fn contents_delete_uses_find() {
    let w = world();
    let script = render(&plan(&w), GENERATED);
    let path = w.home.join(".Trash").display().to_string();
    assert!(script.contains(&format!(
        "\nfind {} -mindepth 1 -maxdepth 1 -exec rm -rf -- '{{}}' +\n",
        quoted(&path)
    )));
}

#[test]
fn trash_passes_the_path_to_finder_as_an_argument() {
    let w = world();
    let script = render(&plan(&w), GENERATED);
    let path = w.home.join(".cache/huggingface/hub").display().to_string();
    assert!(script.contains(&format!(
        "\n/usr/bin/osascript -e 'on run argv' -e 'tell application \"Finder\" to delete (POSIX file (item 1 of argv) as alias)' -e 'end run' {}\n",
        quoted(&path)
    )));
}

#[test]
fn paths_with_spaces_and_quotes_are_single_quoted() {
    let mut w = World::new();
    w.put("it's mine/node_modules", 2);
    w.file("it's mine/package.json", "{}");
    w.scan();
    let script = render(&w.plan_rules(&["node-modules-idle"]), GENERATED);
    let path = w.home.join("it's mine/node_modules").display().to_string();
    let escaped = path.replace('\'', r"'\''");
    assert!(
        script.contains(&format!("\nrm -rf -- '{escaped}'\n")),
        "{script}"
    );
}

#[test]
fn script_defines_no_variables() {
    let w = world();
    let script = render(&plan(&w), GENERATED);
    assert!(
        !script
            .lines()
            .any(|l| l.contains('$') || l.contains('=') && !l.contains("--prune=all"))
    );
}

#[test]
fn dry_run_audit_matches_the_script_step_by_step() {
    let w = world();
    let p = plan(&w);
    let script = render(&p, GENERATED);
    let (_, entries) = w.execute(&p, true);
    let audited: Vec<(String, String)> = entries
        .iter()
        .map(|e| (e.item_id.clone(), e.method.clone()))
        .collect();
    let scripted: Vec<(String, String)> = steps(&script)
        .into_iter()
        .zip(&p.actions)
        .map(|((id, lines), a)| {
            let method = match (&a.method, lines.first().map(String::as_str)) {
                (PlannedMethod::Delete, Some(l)) if l.starts_with("rm -rf -- ") => "delete",
                (PlannedMethod::DeleteContents, Some(l)) if l.contains("-exec rm -rf") => {
                    "delete_contents"
                }
                (PlannedMethod::Trash, Some(l)) if l.starts_with("/usr/bin/osascript") => "trash",
                (PlannedMethod::TrashContents, Some(l))
                    if l.contains("-exec /usr/bin/osascript") =>
                {
                    "trash_contents"
                }
                (PlannedMethod::Command { argv }, Some(l)) if l.starts_with(&quoted(&argv[0])) => {
                    "command"
                }
                _ => "mismatch",
            };
            let path_ok = lines.iter().any(|l| l.contains(&quoted(&a.path)))
                || matches!(a.method, PlannedMethod::Command { .. });
            (
                id,
                if path_ok {
                    method.to_owned()
                } else {
                    "wrong path".into()
                },
            )
        })
        .collect();
    assert_eq!(scripted, audited);
}
