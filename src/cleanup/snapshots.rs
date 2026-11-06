//! Thinning and deleting local Time Machine snapshots (PRD FR-SNAP-4, FR-SNAP-5), the one
//! place DiskLens asks for administrator rights (the S-06 exception).
//!
//! Every action is checked in this order, and refused at the first failure: Pro is active;
//! a date has the tmutil form; administrator actions are allowed when the action needs
//! them; the snapshot is in a `tmutil listlocalsnapshots /` read right now (S-12). A dry
//! run stops there. Every action, refused or not, is audit logged.

use std::path::Path;

use super::audit::{self, Audit, Entry};
use crate::platform::{Runner, TMUTIL, is_snapshot_date, list_local_snapshots};

const OSASCRIPT: &str = "/usr/bin/osascript";
/// `tmutil thinlocalsnapshots` urgency: 4 is the highest.
const THIN_URGENCY: &str = "4";
/// NOTE: from the Phase 17 spike (docs/QA.md): whether `tmutil thinlocalsnapshots` needs
/// administrator rights. While true, Thin goes through the same prompt as Delete.
const THIN_NEEDS_ADMIN: bool = true;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SnapshotAction {
    /// Asks macOS to free up to `bytes` of purgeable space on `/`.
    Thin { bytes: u64 },
    /// Deletes the local snapshot with this date.
    Delete { date: String },
}

impl SnapshotAction {
    pub fn needs_admin(&self) -> bool {
        match self {
            Self::Thin { .. } => THIN_NEEDS_ADMIN,
            Self::Delete { .. } => true,
        }
    }

    /// The `tmutil` command, as shown to the user before they confirm.
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

    /// What runs. An action that needs administrator rights goes through one
    /// `osascript … with administrator privileges` call: the system prompt, then nothing
    /// kept. The script text holds only constants and a date `check` has validated, the
    /// single exception to S-05 (PRD FR-SNAP-5).
    pub fn argv(&self) -> Vec<String> {
        let tmutil = self.tmutil_argv();
        if !self.needs_admin() {
            return tmutil;
        }
        let script = format!(
            "do shell script \"{}\" with administrator privileges",
            tmutil.join(" ")
        );
        vec![OSASCRIPT.into(), "-e".into(), script]
    }

    fn method(&self) -> &'static str {
        if self.needs_admin() {
            "privileged_command"
        } else {
            "command"
        }
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
    #[error("Actions that need administrator rights are turned off")]
    AdminOff,
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
    /// The command ran and failed, or the user cancelled the administrator prompt.
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
    let result = check(engine, action).map(|()| perform(engine, action));
    let (status, detail) = match &result {
        Ok(SnapshotOutcome::Ran) => ("command_ran", None),
        Ok(SnapshotOutcome::DryRun) => ("dry_run", None),
        Ok(SnapshotOutcome::Failed(why)) => ("failed", Some(why.clone())),
        Err(refused) => ("skipped", Some(refused.to_string())),
    };
    let entry = Entry {
        ts: audit::now_ms(),
        plan_id: "snapshots".into(),
        item_id: action.item_id(),
        rule: "apfs-snapshots".into(),
        path: "/".into(),
        method: action.method().into(),
        argv: Some(action.argv()),
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
    result
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
    if action.needs_admin() && !engine.allow_admin {
        return Err(Refused::AdminOff);
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

fn perform(engine: &SnapshotEngine, action: &SnapshotAction) -> SnapshotOutcome {
    if engine.dry_run {
        return SnapshotOutcome::DryRun;
    }
    let argv = action.argv();
    let args: Vec<&str> = argv[1..].iter().map(String::as_str).collect();
    match engine.commands.run(Path::new(&argv[0]), &args) {
        Ok(out) if out.success => SnapshotOutcome::Ran,
        Ok(_) => SnapshotOutcome::Failed("Cancelled or failed".into()),
        Err(e) => SnapshotOutcome::Failed(e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::FakeRunner;

    const LIST: &str = "tmutil listlocalsnapshots /";
    const DATE: &str = "2026-10-30-101500";
    const DELETE: &str = "osascript -e do shell script \"/usr/bin/tmutil deletelocalsnapshots 2026-10-30-101500\" with administrator privileges";

    fn probes() -> FakeRunner {
        FakeRunner::default().with(
            LIST,
            true,
            "Snapshots for disk /:\ncom.apple.TimeMachine.2026-10-30-101500.local\n",
        )
    }

    fn commands() -> FakeRunner {
        FakeRunner::default().with(DELETE, true, "")
    }

    fn engine<'a>(probes: &'a FakeRunner, commands: &'a FakeRunner) -> SnapshotEngine<'a> {
        SnapshotEngine {
            probes,
            commands,
            dry_run: false,
            allow_admin: true,
            pro: true,
        }
    }

    fn delete(date: &str) -> SnapshotAction {
        SnapshotAction::Delete { date: date.into() }
    }

    fn calls(runner: &FakeRunner) -> Vec<String> {
        runner.calls.lock().unwrap().clone()
    }

    #[test]
    fn a_listed_snapshot_is_deleted_through_the_administrator_prompt() {
        let (p, c, dir) = (probes(), commands(), tempfile::tempdir().unwrap());
        let mut audit = Audit::open(dir.path()).unwrap();
        run(&engine(&p, &c), &delete(DATE), &mut audit).unwrap();
        assert_eq!(calls(&c), [DELETE]);
    }

    #[test]
    fn a_date_not_in_the_current_list_is_refused() {
        let (p, c, dir) = (probes(), commands(), tempfile::tempdir().unwrap());
        let mut audit = Audit::open(dir.path()).unwrap();
        let result = run(&engine(&p, &c), &delete("2026-10-29-101500"), &mut audit);
        assert_eq!(result, Err(Refused::Gone));
    }

    #[test]
    fn a_refused_date_never_reaches_a_command() {
        let (p, c, dir) = (probes(), commands(), tempfile::tempdir().unwrap());
        let mut audit = Audit::open(dir.path()).unwrap();
        let _ = run(&engine(&p, &c), &delete("2026-10-29-101500"), &mut audit);
        assert_eq!(calls(&c), Vec::<String>::new());
    }

    #[test]
    fn a_malformed_date_is_refused_before_listing() {
        let (p, c, dir) = (probes(), commands(), tempfile::tempdir().unwrap());
        let mut audit = Audit::open(dir.path()).unwrap();
        let result = run(
            &engine(&p, &c),
            &delete("2026-10-30-101500\"; rm"),
            &mut audit,
        );
        assert_eq!(result, Err(Refused::MalformedDate));
    }

    #[test]
    fn a_non_time_machine_snapshot_is_never_acted_on() {
        let (p, c, dir) = (probes(), commands(), tempfile::tempdir().unwrap());
        let mut audit = Audit::open(dir.path()).unwrap();
        let result = run(
            &engine(&p, &c),
            &delete("com.apple.os.update-6058"),
            &mut audit,
        );
        assert_eq!(result, Err(Refused::MalformedDate));
    }

    #[test]
    fn with_the_opt_in_off_no_privileged_command_runs() {
        let (p, c, dir) = (probes(), commands(), tempfile::tempdir().unwrap());
        let mut audit = Audit::open(dir.path()).unwrap();
        let engine = SnapshotEngine {
            allow_admin: false,
            ..engine(&p, &c)
        };
        let _ = run(&engine, &delete(DATE), &mut audit);
        assert_eq!(calls(&c), Vec::<String>::new());
    }

    #[test]
    fn without_pro_the_action_is_refused() {
        let (p, c, dir) = (probes(), commands(), tempfile::tempdir().unwrap());
        let mut audit = Audit::open(dir.path()).unwrap();
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
        let (p, c, dir) = (probes(), commands(), tempfile::tempdir().unwrap());
        let mut audit = Audit::open(dir.path()).unwrap();
        let engine = SnapshotEngine {
            dry_run: true,
            ..engine(&p, &c)
        };
        let _ = run(&engine, &delete(DATE), &mut audit);
        assert_eq!(calls(&c), Vec::<String>::new());
    }

    #[test]
    fn a_dry_run_is_logged_as_a_dry_run() {
        let (p, c, dir) = (probes(), commands(), tempfile::tempdir().unwrap());
        let mut audit = Audit::open(dir.path()).unwrap();
        let engine = SnapshotEngine {
            dry_run: true,
            ..engine(&p, &c)
        };
        let _ = run(&engine, &delete(DATE), &mut audit);
        let logged = audit::read(audit.path()).unwrap();
        assert_eq!(logged[0].result, "dry_run");
    }

    #[test]
    fn a_privileged_action_is_logged_with_its_method_and_command() {
        let (p, c, dir) = (probes(), commands(), tempfile::tempdir().unwrap());
        let mut audit = Audit::open(dir.path()).unwrap();
        run(&engine(&p, &c), &delete(DATE), &mut audit).unwrap();
        let entry = audit::read(audit.path()).unwrap().remove(0);
        assert_eq!(
            (entry.method.as_str(), entry.argv),
            ("privileged_command", Some(delete(DATE).argv()))
        );
    }

    #[test]
    fn a_cancelled_prompt_is_a_failure() {
        let (p, dir) = (probes(), tempfile::tempdir().unwrap());
        let c = FakeRunner::default().with(DELETE, false, "");
        let mut audit = Audit::open(dir.path()).unwrap();
        assert_eq!(
            run(&engine(&p, &c), &delete(DATE), &mut audit),
            Ok(SnapshotOutcome::Failed("Cancelled or failed".into()))
        );
    }

    #[test]
    fn thin_without_local_snapshots_is_refused() {
        let (c, dir) = (commands(), tempfile::tempdir().unwrap());
        let p = FakeRunner::default().with(LIST, true, "Snapshots for disk /:\n");
        let mut audit = Audit::open(dir.path()).unwrap();
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
