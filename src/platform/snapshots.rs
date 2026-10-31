//! Local Time Machine snapshots of the boot volume (PRD 13.10.5). The cleanup results
//! need only their count, to say why freed space did not show up (PRD 13.4.8).

use std::io;
use std::path::Path;

use super::Runner;

const TMUTIL: &str = "/usr/bin/tmutil";
const PREFIX: &str = "com.apple.TimeMachine.";

/// How many local Time Machine snapshots `/` has, from `tmutil listlocalsnapshots /`.
/// Listing needs no administrator rights.
pub fn local_snapshot_count(runner: &dyn Runner) -> io::Result<usize> {
    let out = runner.run(Path::new(TMUTIL), &["listlocalsnapshots", "/"])?;
    if !out.success {
        return Err(io::Error::other("tmutil listlocalsnapshots failed"));
    }
    Ok(out
        .stdout
        .lines()
        .filter(|l| l.trim_start().starts_with(PREFIX))
        .count())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::FakeRunner;

    const COMMAND: &str = "tmutil listlocalsnapshots /";

    #[test]
    fn counts_the_snapshot_lines_under_the_header() {
        let runner = FakeRunner::default().with(
            COMMAND,
            true,
            "Snapshots for disk /:\ncom.apple.TimeMachine.2026-10-30-101500.local\ncom.apple.TimeMachine.2026-10-31-101500.local\n",
        );
        assert_eq!(local_snapshot_count(&runner).unwrap(), 2);
    }

    #[test]
    fn no_snapshots_count_zero() {
        let runner = FakeRunner::default().with(COMMAND, true, "Snapshots for disk /:\n");
        assert_eq!(local_snapshot_count(&runner).unwrap(), 0);
    }

    #[test]
    fn a_failed_listing_is_an_error() {
        let runner = FakeRunner::default().with(COMMAND, false, "");
        assert!(local_snapshot_count(&runner).is_err());
    }
}
