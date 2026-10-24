//! A scan root in `target/tmp` (never the real home) with files, a scan, its
//! analysis, and an engine over a fake machine and a fake Trash.

#![allow(dead_code)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, SystemTime};

use disklens_lib::cleanup::audit::{self, Audit, Entry};
use disklens_lib::cleanup::execute::{Engine, execute};
use disklens_lib::cleanup::model::{CleanupEvent, CleanupPlan, CleanupSummary};
use disklens_lib::cleanup::plan::{Request, build};
use disklens_lib::cleanup::revalidate::GROWTH_FLOOR;
use disklens_lib::error::Result;
use disklens_lib::platform::{Facts, FakeRunner, FakeSystem, FakeTrash};
use disklens_lib::recommend::build::{Analysis, analyze};
use disklens_lib::rules::guards::Env;
use disklens_lib::rules::model::Rules;
use disklens_lib::rules::tools::Tools;
use disklens_lib::scanner::{ScanOptions, scan};

pub const MB: u64 = 1_000_000;
const OLD: Duration = Duration::from_secs(90 * 86_400);

pub struct World {
    _t: tempfile::TempDir,
    pub home: PathBuf,
    /// Outside the scan root, for symlinks that point away.
    pub outside: PathBuf,
    pub bin: PathBuf,
    pub logs: PathBuf,
    pub trash: PathBuf,
    pub system: FakeSystem,
    pub runner: FakeRunner,
    pub growth_floor: u64,
    pub analysis: Option<Analysis>,
}

impl World {
    pub fn new() -> Self {
        let t = tempfile::tempdir_in(env!("CARGO_TARGET_TMPDIR")).unwrap();
        let base = fs::canonicalize(t.path()).unwrap();
        let dir = |name: &str| {
            let d = base.join(name);
            fs::create_dir_all(&d).unwrap();
            d
        };
        Self {
            home: dir("home"),
            outside: dir("outside"),
            bin: dir("bin"),
            logs: dir("logs"),
            trash: dir("trash"),
            _t: t,
            system: FakeSystem::default(),
            runner: FakeRunner::default(),
            growth_floor: GROWTH_FLOOR,
            analysis: None,
        }
    }

    /// `mb` megabytes in `rel/data`, all of it last modified long ago.
    pub fn put(&self, rel: &str, mb: u64) {
        let dir = self.home.join(rel);
        fs::create_dir_all(&dir).unwrap();
        let f = dir.join("data");
        fs::write(&f, vec![b'x'; (mb * MB) as usize]).unwrap();
        age(&f);
        for d in Path::new(rel)
            .ancestors()
            .filter(|a| !a.as_os_str().is_empty())
        {
            age(&self.home.join(d));
        }
    }

    /// Adds `bytes` to `rel/more` without making the folder look recently changed.
    pub fn grow(&self, rel: &str, bytes: usize) {
        let f = self.home.join(rel).join("more");
        fs::write(&f, vec![b'x'; bytes]).unwrap();
        age(&f);
        age(&self.home.join(rel));
    }

    /// Writes `rel`, last modified long ago.
    pub fn file(&self, rel: &str, contents: &str) {
        let f = self.home.join(rel);
        fs::write(&f, contents).unwrap();
        age(&f);
        age(f.parent().unwrap());
    }

    pub fn tool(&self, name: &str) {
        let p = self.bin.join(name);
        fs::write(&p, "").unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).unwrap();
    }

    pub fn tools(&self) -> Tools {
        Tools::new(vec![self.bin.clone()], false)
    }

    pub fn set_facts(&self, facts: Facts) {
        *self.system.0.lock().unwrap() = facts;
    }

    pub fn scan(&mut self) {
        let snap = scan(
            &self.home,
            &ScanOptions::from_rules(Rules::embedded()),
            &Arc::new(AtomicBool::new(false)),
            &mut |_| {},
        )
        .unwrap();
        let facts = Facts::default();
        let tools = self.tools();
        let analysis = analyze(Rules::embedded(), &snap, &self.env(&facts, &tools));
        self.analysis = Some(analysis);
    }

    fn env<'a>(&'a self, facts: &'a Facts, tools: &'a Tools) -> Env<'a> {
        Env {
            facts,
            runner: &self.runner,
            tools,
            now: SystemTime::now(),
        }
    }

    pub fn analysis(&self) -> &Analysis {
        self.analysis.as_ref().expect("scan first")
    }

    /// Item IDs of `rule_id`, largest first as the panel lists them.
    pub fn items(&self, rule_id: &str) -> Vec<String> {
        let mut items: Vec<_> = self
            .analysis()
            .items
            .iter()
            .filter(|i| i.id.starts_with(&format!("{rule_id}:")))
            .collect();
        items.sort_by_key(|i| std::cmp::Reverse(i.size));
        items.into_iter().map(|i| i.id.clone()).collect()
    }

    pub fn try_plan(
        &self,
        snapshot_id: &str,
        ids: &[String],
        permanent: bool,
    ) -> Result<CleanupPlan> {
        let facts = self.system.0.lock().unwrap().clone();
        let tools = self.tools();
        build(
            "plan-1".into(),
            Rules::embedded(),
            self.analysis(),
            &self.env(&facts, &tools),
            &Request {
                snapshot_id,
                item_ids: ids,
                permanent_review: permanent,
            },
        )
    }

    pub fn plan(&self, ids: &[String]) -> CleanupPlan {
        self.try_plan(&self.analysis().snapshot_id, ids, false)
            .unwrap()
    }

    pub fn plan_rules(&self, rule_ids: &[&str]) -> CleanupPlan {
        let ids: Vec<String> = rule_ids.iter().flat_map(|r| self.items(r)).collect();
        self.plan(&ids)
    }

    /// Runs `plan` and returns the summary and the audit entries it wrote.
    pub fn execute(&self, plan: &CleanupPlan, dry_run: bool) -> (CleanupSummary, Vec<Entry>) {
        self.execute_with(plan, dry_run, &mut |_| {})
    }

    pub fn execute_with(
        &self,
        plan: &CleanupPlan,
        dry_run: bool,
        on_event: &mut dyn FnMut(CleanupEvent),
    ) -> (CleanupSummary, Vec<Entry>) {
        let tools = self.tools();
        let trash = FakeTrash {
            dir: self.trash.clone(),
        };
        let engine = Engine {
            rules: Rules::embedded(),
            root: &self.home,
            system: &self.system,
            probes: &self.runner,
            commands: &self.runner,
            tools: &tools,
            trash: &trash,
            dry_run,
            growth_floor: self.growth_floor,
        };
        let mut log = Audit::open(&self.logs).unwrap();
        let summary = execute(&engine, plan, &mut log, on_event);
        let entries = audit::read(log.path()).unwrap();
        (summary, entries)
    }

    pub fn exists(&self, rel: &str) -> bool {
        self.home.join(rel).symlink_metadata().is_ok()
    }
}

fn age(p: &Path) {
    fs::File::open(p)
        .unwrap()
        .set_modified(SystemTime::now() - OLD)
        .unwrap();
}
