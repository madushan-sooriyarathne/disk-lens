//! Thinning and deleting local Time Machine snapshots (PRD FR-SNAP-4, FR-SNAP-5), the one
//! place DiskLens may ask for administrator rights (the S-06 exception).
//!
//! Every action is checked first, and refused at the first failure: Pro is active; a date
//! has the tmutil form; the snapshot is in a `tmutil listlocalsnapshots /` read right now
//! (S-12). A dry run stops there. Then `tmutil` runs directly: on macOS 26 neither action
//! needs administrator rights (Phase 17 spike, docs/QA.md). Only when that fails, and only
//! with the opt-in on, it is retried once through the administrator prompt, after the
//! listing is read again. Every command that runs, and every refusal, is audit logged.

use std::path::Path;

use super::audit::{self, Audit, Entry};
use crate::platform::{Runner, TMUTIL, is_snapshot_date, list_local_snapshots};

const OSASCRIPT: &str = "/usr/bin/osascript";
/// `tmutil thinlocalsnapshots` urgency: 4 is the highest.
const THIN_URGENCY: &str = "4";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SnapshotAction {
    /// Asks macOS to free up to `bytes` of purgeable space on `/`.
    Thin { bytes: u64 },
    /// Deletes the local snapshot with this date.
    Delete { date: String },
}

impl SnapshotAction {
    /// The `tmutil` command: shown before Confirm, and what runs first.
    pub fn tmutil_argv(&self) -> Vec<String> {
        let args: Vec<String> = match self {
            Self::Thin { bytes } => vec![
                "thinlocalsnapshots".into(),
                "/".into(),
                bytes.to_string(),
                THIN_URGENCY.into(),
            ],
            Self::Delete { date } => vec!["deletelocalsnapshots".into(), date.clone()],
        };
        std::iter::once(TMUTIL.to_owned()).chain(args).collect()
    }

    /// The retry with administrator rights: one `osascript … with administrator
    /// privileges` call, the system prompt, then nothing kept. The script text holds only
    /// constants and a date `check` has validated, the single exception to S-05
    /// (PRD FR-SNAP-5).
    pub fn privileged_argv(&self) -> Vec<String> {
        let script = format!(
            "do shell script \"{}\" with administrator privileges",
            self.tmutil_argv().join(" ")
        );
        vec![OSASCRIPT.into(), "-e".into(), script]
    }

    fn item_id(&self) -> String {
        match self {
            Self::Thin { .. } => "thin".into(),
            Self::Delete { date } => date.clone(),
        }
    }
}

/// Why an action did not run.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Refused {
    #[error("Snapshot actions need DiskLens Pro")]
    NotPro,
    #[error("That is not a snapshot date")]
    MalformedDate,
    #[error("That snapshot no longer exists")]
    Gone,
    #[error("There are no local snapshots to thin")]
    NoSnapshots,
    #[error("Could not list local snapshots: {0}")]
    ListFailed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SnapshotOutcome {
    Ran,
    /// Checked and logged; nothing ran (S-08).
    DryRun,
    /// `tmutil` failed and the administrator opt-in is off, so nothing was retried.
    NeedsAdmin,
    /// The command failed, or the user cancelled the administrator prompt.
    Failed(String),
}

/// What a snapshot action touches. Tests pass fakes for both runners.
pub struct SnapshotEngine<'a> {
    /// Lists the snapshots right before acting.
    pub probes: &'a dyn Runner,
    /// Runs the action, with the 10 minute timeout (S-05).
    pub commands: &'a dyn Runner,
    pub dry_run: bool,
    /// The Settings opt-in "Allow actions that need administrator rights".
    pub allow_admin: bool,
    pub pro: bool,
}

pub fn run(
    engine: &SnapshotEngine,
    action: &SnapshotAction,
    audit: &mut Audit,
) -> Result<SnapshotOutcome, Refused> {
    let direct = action.tmutil_argv();
    if let Err(refused) = check(engine, action) {
        log(audit, engine, action, &direct, Err(&refused));
        return Err(refused);
    }
    if engine.dry_run {
        log(audit, engine, action, &direct, Ok(&SnapshotOutcome::DryRun));
        return Ok(SnapshotOutcome::DryRun);
    }
    let first = execute(engine.commands, &direct);
    log(audit, engine, action, &direct, Ok(&first));
    if !matches!(first, SnapshotOutcome::Failed(_)) {
        return Ok(first);
    }
    if !engine.allow_admin {
        return Ok(SnapshotOutcome::NeedsAdmin);
    }
    let privileged = action.privileged_argv();
    // The failed attempt may have raced a change; act only on what is listed now.
    if let Err(refused) = check(engine, action) {
        log(audit, engine, action, &privileged, Err(&refused));
        return Err(refused);
    }
    let second = execute(engine.commands, &privileged);
    log(audit, engine, action, &privileged, Ok(&second));
    Ok(second)
}

fn check(engine: &SnapshotEngine, action: &SnapshotAction) -> Result<(), Refused> {
    if !engine.pro {
        return Err(Refused::NotPro);
    }
    if let SnapshotAction::Delete { date } = action
        && !is_snapshot_date(date)
    {
        return Err(Refused::MalformedDate);
    }
    let listed =
        list_local_snapshots(engine.probes).map_err(|e| Refused::ListFailed(e.to_string()))?;
    match action {
        SnapshotAction::Delete { date } if !listed.iter().any(|s| &s.date == date) => {
            Err(Refused::Gone)
        }
        SnapshotAction::Thin { .. } if listed.is_empty() => Err(Refused::NoSnapshots),
        _ => Ok(()),
    }
}

fn execute(runner: &dyn Runner, argv: &[String]) -> SnapshotOutcome {
    let args: Vec<&str> = argv[1..].iter().map(String::as_str).collect();
    match runner.run(Path::new(&argv[0]), &args) {
        Ok(out) if out.success => SnapshotOutcome::Ran,
        Ok(_) => SnapshotOutcome::Failed("Cancelled or failed".into()),
        Err(e) => SnapshotOutcome::Failed(e.to_string()),
    }
}

fn log(
    audit: &mut Audit,
    engine: &SnapshotEngine,
    action: &SnapshotAction,
    argv: &[String],
    result: Result<&SnapshotOutcome, &Refused>,
) {
    let (status, detail) = match result {
        Ok(SnapshotOutcome::Ran) => ("command_ran", None),
        Ok(SnapshotOutcome::DryRun) => ("dry_run", None),
        Ok(SnapshotOutcome::NeedsAdmin) => ("failed", Some("needs administrator rights".into())),
        Ok(SnapshotOutcome::Failed(why)) => ("failed", Some(why.clone())),
        Err(refused) => ("skipped", Some(refused.to_string())),
    };
    let method = if argv.first().is_some_and(|p| p == OSASCRIPT) {
        "privileged_command"
    } else {
        "command"
    };
    let entry = Entry {
        ts: audit::now_ms(),
        plan_id: "snapshots".into(),
        item_id: action.item_id(),
        rule: "apfs-snapshots".into(),
        path: "/".into(),
        method: method.into(),
        argv: Some(argv.to_vec()),
        bytes: 0,
        result: status.into(),
        detail,
        dry_run: engine.dry_run,
    };
    if let Err(e) = audit.record(&entry) {
        tracing::error!(
            "could not write {} for {entry:?}: {e}",
            audit.path().display()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::FakeRunner;

    const LIST: &str = "tmutil listlocalsnapshots /";
    const DATE: &str = "2026-10-30-101500";
    const DIRECT: &str = "tmutil deletelocalsnapshots 2026-10-30-101500";
    const PRIVILEGED: &str = "osascript -e do shell script \"/usr/bin/tmutil deletelocalsnapshots 2026-10-30-101500\" with administrator privileges";

    fn probes() -> FakeRunner {
        FakeRunner::default().with(
            LIST,
            true,
            "Snapshots for disk /:\ncom.apple.TimeMachine.2026-10-30-101500.local\n",
        )
    }

    /// `tmutil` works without administrator rights, as on macOS 26.
    fn commands() -> FakeRunner {
        FakeRunner::default().with(DIRECT, true, "")
    }

    /// `tmutil` fails without administrator rights; the prompt succeeds.
    fn needs_admin() -> FakeRunner {
        FakeRunner::default()
            .with(DIRECT, false, "")
            .with(PRIVILEGED, true, "")
    }

    fn engine<'a>(probes: &'a FakeRunner, commands: &'a FakeRunner) -> SnapshotEngine<'a> {
        SnapshotEngine {
            probes,
            commands,
            dry_run: false,
            allow_admin: false,
            pro: true,
        }
    }

    fn delete(date: &str) -> SnapshotAction {
        SnapshotAction::Delete { date: date.into() }
    }

    fn calls(runner: &FakeRunner) -> Vec<String> {
        runner.calls.lock().unwrap().clone()
    }

    fn audit() -> (tempfile::TempDir, Audit) {
        let dir = tempfile::tempdir().unwrap();
        let audit = Audit::open(dir.path()).unwrap();
        (dir, audit)
    }

    #[test]
    fn a_listed_snapshot_is_deleted_without_a_prompt() {
        let (p, c, (_d, mut audit)) = (probes(), commands(), audit());
        run(&engine(&p, &c), &delete(DATE), &mut audit).unwrap();
        assert_eq!(calls(&c), [DIRECT]);
    }

    #[test]
    fn a_failed_delete_with_the_opt_in_off_asks_for_it() {
        let (p, c, (_d, mut audit)) = (probes(), needs_admin(), audit());
        assert_eq!(
            run(&engine(&p, &c), &delete(DATE), &mut audit),
            Ok(SnapshotOutcome::NeedsAdmin)
        );
    }

    #[test]
    fn with_the_opt_in_off_no_privileged_command_runs() {
        let (p, c, (_d, mut audit)) = (probes(), needs_admin(), audit());
        let _ = run(&engine(&p, &c), &delete(DATE), &mut audit);
        assert_eq!(calls(&c), [DIRECT]);
    }

    #[test]
    fn with_the_opt_in_on_a_failed_delete_is_retried_through_the_prompt() {
        let (p, c, (_d, mut audit)) = (probes(), needs_admin(), audit());
        let engine = SnapshotEngine {
            allow_admin: true,
            ..engine(&p, &c)
        };
        let _ = run(&engine, &delete(DATE), &mut audit);
        assert_eq!(calls(&c), [DIRECT, PRIVILEGED]);
    }

    #[test]
    fn the_retry_reads_the_listing_again() {
        let (p, c, (_d, mut audit)) = (probes(), needs_admin(), audit());
        let engine = SnapshotEngine {
            allow_admin: true,
            ..engine(&p, &c)
        };
        let _ = run(&engine, &delete(DATE), &mut audit);
        assert_eq!(calls(&p), [LIST, LIST]);
    }

    #[test]
    fn the_privileged_retry_is_logged_with_its_method_and_command() {
        let (p, c, (_d, mut audit)) = (probes(), needs_admin(), audit());
        let engine = SnapshotEngine {
            allow_admin: true,
            ..engine(&p, &c)
        };
        let _ = run(&engine, &delete(DATE), &mut audit);
        let entry = audit::read(audit.path()).unwrap().remove(1);
        assert_eq!(
            (entry.method.as_str(), entry.argv),
            ("privileged_command", Some(delete(DATE).privileged_argv()))
        );
    }

    #[test]
    fn a_cancelled_prompt_is_a_failure() {
        let (p, (_d, mut audit)) = (probes(), audit());
        let c = FakeRunner::default()
            .with(DIRECT, false, "")
            .with(PRIVILEGED, false, "");
        let engine = SnapshotEngine {
            allow_admin: true,
            ..engine(&p, &c)
        };
        assert_eq!(
            run(&engine, &delete(DATE), &mut audit),
            Ok(SnapshotOutcome::Failed("Cancelled or failed".into()))
        );
    }

    #[test]
    fn a_date_not_in_the_current_list_is_refused() {
        let (p, c, (_d, mut audit)) = (probes(), commands(), audit());
        let result = run(&engine(&p, &c), &delete("2026-10-29-101500"), &mut audit);
        assert_eq!(result, Err(Refused::Gone));
    }

    #[test]
    fn a_refused_date_never_reaches_a_command() {
        let (p, c, (_d, mut audit)) = (probes(), commands(), audit());
        let _ = run(&engine(&p, &c), &delete("2026-10-29-101500"), &mut audit);
        assert_eq!(calls(&c), Vec::<String>::new());
    }

    #[test]
    fn a_malformed_date_is_refused() {
        let (p, c, (_d, mut audit)) = (probes(), commands(), audit());
        let result = run(
            &engine(&p, &c),
            &delete("2026-10-30-101500\"; rm"),
            &mut audit,
        );
        assert_eq!(result, Err(Refused::MalformedDate));
    }

    #[test]
    fn a_non_time_machine_snapshot_is_never_acted_on() {
        let (p, c, (_d, mut audit)) = (probes(), commands(), audit());
        let result = run(
            &engine(&p, &c),
            &delete("com.apple.os.update-6058"),
            &mut audit,
        );
        assert_eq!(result, Err(Refused::MalformedDate));
    }

    #[test]
    fn without_pro_the_action_is_refused() {
        let (p, c, (_d, mut audit)) = (probes(), commands(), audit());
        let engine = SnapshotEngine {
            pro: false,
            ..engine(&p, &c)
        };
        assert_eq!(
            run(&engine, &delete(DATE), &mut audit),
            Err(Refused::NotPro)
        );
    }

    #[test]
    fn a_dry_run_never_invokes_the_runner() {
        let (p, c, (_d, mut audit)) = (probes(), commands(), audit());
        let engine = SnapshotEngine {
            dry_run: true,
            ..engine(&p, &c)
        };
        let _ = run(&engine, &delete(DATE), &mut audit);
        assert_eq!(calls(&c), Vec::<String>::new());
    }

    #[test]
    fn a_dry_run_is_logged_as_a_dry_run() {
        let (p, c, (_d, mut audit)) = (probes(), commands(), audit());
        let engine = SnapshotEngine {
            dry_run: true,
            ..engine(&p, &c)
        };
        let _ = run(&engine, &delete(DATE), &mut audit);
        let logged = audit::read(audit.path()).unwrap();
        assert_eq!(logged[0].result, "dry_run");
    }

    #[test]
    fn a_direct_delete_is_logged_as_a_command() {
        let (p, c, (_d, mut audit)) = (probes(), commands(), audit());
        run(&engine(&p, &c), &delete(DATE), &mut audit).unwrap();
        let entry = audit::read(audit.path()).unwrap().remove(0);
        assert_eq!(
            (entry.method.as_str(), entry.result.as_str()),
            ("command", "command_ran")
        );
    }

    #[test]
    fn thin_without_local_snapshots_is_refused() {
        let (c, (_d, mut audit)) = (commands(), audit());
        let p = FakeRunner::default().with(LIST, true, "Snapshots for disk /:\n");
        let result = run(
            &engine(&p, &c),
            &SnapshotAction::Thin { bytes: 1 },
            &mut audit,
        );
        assert_eq!(result, Err(Refused::NoSnapshots));
    }

    #[test]
    fn thin_asks_for_the_bytes_at_the_highest_urgency() {
        assert_eq!(
            SnapshotAction::Thin { bytes: 5_000 }.tmutil_argv(),
            ["/usr/bin/tmutil", "thinlocalsnapshots", "/", "5000", "4"]
        );
    }
}
