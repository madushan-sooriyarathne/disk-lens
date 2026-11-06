//! APFS snapshots of the boot volume (PRD 13.10.5). Local Time Machine snapshots are the
//! only ones DiskLens acts on (S-12); the others are listed read-only.

use std::io;
use std::path::Path;

use super::Runner;

pub const TMUTIL: &str = "/usr/bin/tmutil";
const DISKUTIL: &str = "/usr/sbin/diskutil";
const PREFIX: &str = "com.apple.TimeMachine.";
const SUFFIX: &str = ".local";

/// One local Time Machine snapshot, named `com.apple.TimeMachine.<date>.local`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalSnapshot {
    /// `YYYY-MM-DD-HHMMSS`, always passing [`is_snapshot_date`].
    pub date: String,
}

impl LocalSnapshot {
    /// Seconds since the Unix epoch, reading the date as UTC; good enough for an age in days.
    pub fn epoch_secs(&self) -> u64 {
        let n = |r: std::ops::Range<usize>| self.date[r].parse::<i64>().unwrap_or(0);
        let days = days_from_civil(n(0..4), n(5..7), n(8..10));
        let secs = days * 86_400 + n(11..13) * 3600 + n(13..15) * 60 + n(15..17);
        u64::try_from(secs).unwrap_or(0)
    }
}

/// `^\d{4}-\d{2}-\d{2}-\d{6}$`: the only form a date may have before it reaches a command
/// (PRD FR-SNAP-5).
pub fn is_snapshot_date(date: &str) -> bool {
    let b = date.as_bytes();
    b.len() == 17
        && b.iter().enumerate().all(|(i, c)| match i {
            4 | 7 | 10 => *c == b'-',
            _ => c.is_ascii_digit(),
        })
}

/// Local Time Machine snapshots of `/`, from `tmutil listlocalsnapshots /`. Listing needs
/// no administrator rights.
pub fn list_local_snapshots(runner: &dyn Runner) -> io::Result<Vec<LocalSnapshot>> {
    let out = runner.run(Path::new(TMUTIL), &["listlocalsnapshots", "/"])?;
    if !out.success {
        return Err(io::Error::other("tmutil listlocalsnapshots failed"));
    }
    Ok(out
        .stdout
        .lines()
        .filter_map(|l| l.trim().strip_prefix(PREFIX)?.strip_suffix(SUFFIX))
        .filter(|d| is_snapshot_date(d))
        .map(|d| LocalSnapshot { date: d.to_owned() })
        .collect())
}

/// How many local Time Machine snapshots `/` has.
pub fn local_snapshot_count(runner: &dyn Runner) -> io::Result<usize> {
    Ok(list_local_snapshots(runner)?.len())
}

/// Names of the other APFS snapshots of `/` (system and update snapshots), from
/// `diskutil apfs listSnapshots /`. They are shown as managed by macOS and never acted on.
pub fn other_snapshots(runner: &dyn Runner) -> io::Result<Vec<String>> {
    let out = runner.run(Path::new(DISKUTIL), &["apfs", "listSnapshots", "/"])?;
    if !out.success {
        return Err(io::Error::other("diskutil apfs listSnapshots failed"));
    }
    Ok(out
        .stdout
        .lines()
        .filter_map(|l| l.trim().strip_prefix("Name:"))
        .map(str::trim)
        .filter(|n| !n.is_empty() && !n.starts_with(PREFIX))
        .map(str::to_owned)
        .collect())
}

/// Days since 1970-01-01 of a proleptic Gregorian date (Howard Hinnant's algorithm).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::FakeRunner;

    const COMMAND: &str = "tmutil listlocalsnapshots /";
    const DISKUTIL_COMMAND: &str = "diskutil apfs listSnapshots /";

    fn listing(stdout: &str) -> FakeRunner {
        FakeRunner::default().with(COMMAND, true, stdout)
    }

    #[test]
    fn counts_the_snapshot_lines_under_the_header() {
        let runner = listing(
            "Snapshots for disk /:\ncom.apple.TimeMachine.2026-10-30-101500.local\ncom.apple.TimeMachine.2026-10-31-101500.local\n",
        );
        assert_eq!(local_snapshot_count(&runner).unwrap(), 2);
    }

    #[test]
    fn no_snapshots_count_zero() {
        assert_eq!(
            local_snapshot_count(&listing("Snapshots for disk /:\n")).unwrap(),
            0
        );
    }

    #[test]
    fn a_failed_listing_is_an_error() {
        let runner = FakeRunner::default().with(COMMAND, false, "");
        assert!(local_snapshot_count(&runner).is_err());
    }

    #[test]
    fn lists_the_dates_of_time_machine_snapshots() {
        let runner = listing(
            "Snapshots for disk /:\ncom.apple.TimeMachine.2026-10-30-101500.local\ncom.apple.TimeMachine.2026-10-31-091000.local\n",
        );
        let dates: Vec<_> = list_local_snapshots(&runner)
            .unwrap()
            .into_iter()
            .map(|s| s.date)
            .collect();
        assert_eq!(dates, ["2026-10-30-101500", "2026-10-31-091000"]);
    }

    #[test]
    fn a_listed_name_with_a_malformed_date_is_left_out() {
        let runner = listing("com.apple.TimeMachine.2026-10-30-1015; rm -rf ~.local\n");
        assert_eq!(list_local_snapshots(&runner).unwrap(), []);
    }

    #[test]
    fn a_snapshot_date_has_the_tmutil_form() {
        assert!(is_snapshot_date("2026-10-30-101500"));
    }

    #[test]
    fn a_date_with_a_short_field_is_not_a_snapshot_date() {
        assert!(!is_snapshot_date("2026-1-30-1015000"));
    }

    #[test]
    fn a_date_with_shell_text_is_not_a_snapshot_date() {
        assert!(!is_snapshot_date("2026-10-30-101500; rm"));
    }

    #[test]
    fn letters_are_not_a_snapshot_date() {
        assert!(!is_snapshot_date("abcd-ef-gh-ijklmn"));
    }

    #[test]
    fn the_age_comes_from_the_date() {
        let snap = LocalSnapshot {
            date: "2026-11-06-120000".into(),
        };
        assert_eq!(snap.epoch_secs(), 1_793_966_400);
    }

    #[test]
    fn other_snapshots_are_the_named_non_time_machine_ones() {
        let runner = FakeRunner::default().with(
            DISKUTIL_COMMAND,
            true,
            "Snapshot for disk3s1s1 (2 found)\n|\n+-- 12EF\n    Name:        com.apple.os.update-6058\n    Purgeable:   No\n+-- 34AB\n    Name:        com.apple.TimeMachine.2026-10-30-101500.local\n",
        );
        assert_eq!(
            other_snapshots(&runner).unwrap(),
            ["com.apple.os.update-6058"]
        );
    }
}
