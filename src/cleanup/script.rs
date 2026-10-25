//! Script export (PRD section 7.7), from the same plan execution runs.
//!
//! Every path and argument is a single-quoted literal. The script defines no
//! variables; Finder's delete takes the path as an `osascript` argument, so no
//! path is ever spliced into AppleScript source.

use std::borrow::Cow;
use std::collections::HashSet;

use shell_escape::unix::escape;

use super::model::{CleanupPlan, PlannedMethod};
use crate::ui::format::format_bytes;

/// Finder delete of the path given as the first argument.
const FINDER_DELETE: [&str; 6] = [
    "-e",
    "on run argv",
    "-e",
    "tell application \"Finder\" to delete (POSIX file (item 1 of argv) as alias)",
    "-e",
    "end run",
];

/// The script for `plan`, generated at `generated_at` (Unix seconds).
pub fn render(plan: &CleanupPlan, generated_at: i64) -> String {
    let mut out = String::new();
    let count = plan.actions.len();
    out.push_str("#!/bin/bash\nset -euo pipefail\n\n");
    out.push_str(&format!(
        "# DiskLens {} cleanup script\n# Generated {}\n# Total {} in {count} {}\n",
        env!("CARGO_PKG_VERSION"),
        utc(generated_at),
        format_bytes(plan.total),
        if count == 1 { "item" } else { "items" },
    ));
    let mut ran: HashSet<&[String]> = HashSet::new();
    for a in &plan.actions {
        out.push_str(&format!("\n# Item {}\n", a.item_id));
        let size = format_bytes(a.size);
        let (says, run): (String, Option<String>) = match &a.method {
            PlannedMethod::Delete => (
                format!("Deleting {} ({size})", a.path),
                Some(line(&["rm", "-rf", "--", &a.path])),
            ),
            PlannedMethod::DeleteContents => (
                format!("Deleting the contents of {} ({size})", a.path),
                Some(line(&[
                    "find",
                    &a.path,
                    "-mindepth",
                    "1",
                    "-maxdepth",
                    "1",
                    "-exec",
                    "rm",
                    "-rf",
                    "--",
                    "{}",
                    "+",
                ])),
            ),
            PlannedMethod::Trash => (
                format!("Moving {} to the Trash ({size})", a.path),
                Some(line(
                    &[
                        &["/usr/bin/osascript"],
                        &FINDER_DELETE[..],
                        &[a.path.as_str()],
                    ]
                    .concat(),
                )),
            ),
            PlannedMethod::TrashContents => (
                format!("Moving the contents of {} to the Trash ({size})", a.path),
                Some(line(
                    &[
                        &["find", a.path.as_str(), "-mindepth", "1", "-maxdepth", "1"],
                        &["-exec", "/usr/bin/osascript"][..],
                        &FINDER_DELETE[..],
                        &["{}", ";"][..],
                    ]
                    .concat(),
                )),
            ),
            PlannedMethod::Command { argv } if !ran.insert(argv) => {
                (format!("{} is covered by the command above", a.path), None)
            }
            PlannedMethod::Command { argv } => {
                let args: Vec<&str> = argv.iter().map(String::as_str).collect();
                (format!("Cleaning {} ({size})", a.path), Some(line(&args)))
            }
        };
        out.push_str(&line(&["echo", &says]));
        out.push('\n');
        if let Some(run) = run {
            out.push_str(&run);
            out.push('\n');
        }
    }
    out
}

/// Words joined with spaces, each one shell-quoted when it needs to be.
fn line(words: &[&str]) -> String {
    words
        .iter()
        .map(|w| escape(Cow::Borrowed(w)).into_owned())
        .collect::<Vec<_>>()
        .join(" ")
}

/// `2026-10-24 09:05 UTC` (days-to-civil from Howard Hinnant's date algorithms).
fn utc(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02} UTC",
        rem / 3600,
        rem % 3600 / 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_utc_dates() {
        assert_eq!(utc(1_792_833_900), "2026-10-24 09:25 UTC");
    }

    #[test]
    fn formats_the_epoch() {
        assert_eq!(utc(0), "1970-01-01 00:00 UTC");
    }

    #[test]
    fn quotes_words_that_need_it() {
        assert_eq!(
            line(&["rm", "-rf", "--", "/h/it's here"]),
            r"rm -rf -- '/h/it'\''s here'"
        );
    }
}
