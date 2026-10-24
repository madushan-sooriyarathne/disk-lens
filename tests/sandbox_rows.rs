//! The fixture sandbox (`scripts/make-fixture.sh`) yields the expected rows,
//! tiers, statuses and totals. Read-only: scans the sandbox and runs `git status`
//! in its worktrees.

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, SystemTime};

use disklens_lib::platform::{Facts, FakeRunner, ProcessRunner, Runner};
use disklens_lib::recommend::build::analyze;
use disklens_lib::recommend::model::{ItemStatus, MethodView, Recommendation, Tier};
use disklens_lib::rules::guards::Env;
use disklens_lib::rules::model::Rules;
use disklens_lib::rules::tools::Tools;
use disklens_lib::scanner::tree::Snapshot;
use disklens_lib::scanner::{ScanOptions, scan};
use disklens_lib::view::{ViewMode, ViewNode, build_view};

const FIXTURE_VERSION: &str = "3";
const MB: u64 = 1_000_000;

fn sandbox() -> Option<PathBuf> {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join(".sandbox");
    if !base.join("home").exists() {
        eprintln!("skipping: run scripts/make-fixture.sh first");
        return None;
    }
    let version = fs::read_to_string(base.join("fixture-version")).unwrap_or_default();
    assert_eq!(
        version.trim(),
        FIXTURE_VERSION,
        "stale fixture: run scripts/make-fixture.sh"
    );
    Some(fs::canonicalize(base.join("home")).unwrap())
}

fn scanned(home: &Path) -> Snapshot {
    scan(
        home,
        &ScanOptions::from_rules(Rules::embedded()),
        &Arc::new(AtomicBool::new(false)),
        &mut |_| {},
    )
    .unwrap()
}

fn rows(snap: &Snapshot, tools: &Tools, runner: &dyn Runner) -> Vec<Recommendation> {
    let rules = Rules::embedded();
    let facts = Facts::default();
    let env = Env {
        facts: &facts,
        runner,
        tools,
        now: SystemTime::now(),
    };
    analyze(rules, snap, &env).recommendations(rules, &env)
}

fn by_id(rows: &[Recommendation]) -> BTreeMap<&str, &Recommendation> {
    rows.iter().map(|r| (r.row_id.as_str(), r)).collect()
}

#[test]
fn sandbox_rows_tiers_statuses_and_totals() {
    let Some(home) = sandbox() else { return };
    let snap = scanned(&home);
    let real_home = std::env::var("HOME").unwrap();
    // Sandbox mode: only git resolves. Real git answers the worktree checks.
    let tools = Tools::discover(Path::new(&real_home), true);
    let runner = ProcessRunner {
        timeout: Duration::from_secs(30),
        home: real_home,
    };
    let rows = rows(&snap, &tools, &runner);
    let ids = by_id(&rows);

    // Hidden in sandbox mode: pnpm, ollama, xcrun and docker do not resolve.
    let expected: [(&str, Tier, usize); 23] = [
        ("app-caches", Tier::Safe, 1),
        ("npm-cache", Tier::Safe, 3),
        ("gradle-caches", Tier::Safe, 3),
        ("next-turbo-build", Tier::Safe, 3),
        ("yarn-cache", Tier::Safe, 2),
        ("uv-puppeteer", Tier::Safe, 2),
        ("cargo-registry", Tier::Safe, 2),
        ("bun-cache", Tier::Safe, 1),
        ("xcode-derived-data", Tier::Safe, 1),
        ("playwright-browsers", Tier::Safe, 1),
        ("pip-cache", Tier::Safe, 1),
        ("homebrew-cache", Tier::Safe, 1),
        ("cocoapods-cache", Tier::Safe, 1),
        ("go-build-cache", Tier::Safe, 1),
        ("android-sdk-emulators", Tier::Review, 3),
        ("agent-worktrees:atlas", Tier::Review, 4),
        ("huggingface-cache", Tier::Review, 1),
        ("lmstudio-models", Tier::Review, 1),
        ("node-modules-idle", Tier::Review, 1),
        ("xcode-archives", Tier::Review, 1),
        ("ios-device-support", Tier::Review, 1),
        ("ios-backups", Tier::Review, 1),
        ("trash-bin", Tier::Review, 1),
    ];
    let got: Vec<&str> = ids.keys().copied().collect();
    let mut want: Vec<&str> = expected.iter().map(|(id, ..)| *id).collect();
    want.sort();
    assert_eq!(got, want);
    for (id, tier, count) in &expected {
        let r = ids[id];
        assert_eq!(r.tier, *tier, "{id}");
        assert_eq!(r.items.len(), *count, "{id}");
        assert_eq!(r.size, r.items.iter().map(|i| i.size).sum::<u64>(), "{id}");
        assert!(
            r.items.iter().all(|i| i.size >= MB),
            "{id}: items under 1 MB are dropped"
        );
    }

    // Safe first, then review; sizes never increase within a tier.
    let first_review = rows.iter().position(|r| r.tier == Tier::Review).unwrap();
    assert!(rows[..first_review].iter().all(|r| r.tier == Tier::Safe));
    assert!(rows[first_review..].iter().all(|r| r.tier == Tier::Review));
    for pair in rows.windows(2) {
        if pair[0].tier == pair[1].tier {
            assert!(
                pair[0].size >= pair[1].size,
                "{} before {}",
                pair[0].row_id,
                pair[1].row_id
            );
        }
    }
    assert_eq!(rows[0].row_id, "app-caches", "the 50 MB app cache leads");
    assert!(ids["app-caches"].size >= 50 * MB);

    // Everything is ready except two worktrees: one with uncommitted changes and
    // one with a commit on no branch. Pushed and local-branch worktrees are ready.
    let mut blocked: Vec<(&str, &ItemStatus)> = rows
        .iter()
        .flat_map(|r| r.items.iter())
        .filter(|i| i.status != ItemStatus::Ready)
        .map(|i| (i.name.as_str(), &i.status))
        .collect();
    blocked.sort_by_key(|(name, _)| *name);
    assert_eq!(
        blocked,
        [
            (
                "wt-detached",
                &ItemStatus::Blocked {
                    reason: "Has commits on no branch".into()
                }
            ),
            (
                "wt-dirty",
                &ItemStatus::Blocked {
                    reason: "Has uncommitted changes".into()
                }
            ),
        ]
    );

    // Sandbox runs can only ever run git; brew falls back to delete.
    for r in &rows {
        for i in &r.items {
            if let MethodView::Command { display } = &i.method {
                assert!(
                    display.starts_with("git -C ~/Desktop/atlas worktree remove"),
                    "{display}"
                );
            }
        }
    }
    assert_eq!(ids["homebrew-cache"].items[0].method, MethodView::Delete);
    assert_eq!(ids["agent-worktrees:atlas"].label, "atlas worktrees");
    assert_eq!(ids["npm-cache"].paths_hint.matches("~/.npm/").count(), 3);

    // Totals: tier sums equal the sum of their rows, and no path is in two rows.
    let safe: u64 = rows
        .iter()
        .filter(|r| r.tier == Tier::Safe)
        .map(|r| r.size)
        .sum();
    let all: u64 = rows.iter().map(|r| r.size).sum();
    let mut paths: Vec<&str> = rows
        .iter()
        .flat_map(|r| r.items.iter().map(|i| i.path.as_str()))
        .collect();
    let n = paths.len();
    paths.sort();
    paths.dedup();
    assert_eq!(paths.len(), n);
    assert!(safe > 60 * MB && all > safe);
}

#[test]
fn view_tiles_carry_tier_and_category() {
    let Some(home) = sandbox() else { return };
    let snap = scanned(&home);
    let rules = Rules::embedded();
    let tools = Tools::new(vec![], true);
    let facts = Facts::default();
    let runner = FakeRunner::default();
    let env = Env {
        facts: &facts,
        runner: &runner,
        tools: &tools,
        now: SystemTime::now(),
    };
    let a = analyze(rules, &snap, &env);
    let v = build_view(
        &snap,
        &home.join(".npm"),
        ViewMode::Cleanup,
        Some((rules, &a)),
    )
    .unwrap();
    let find =
        |v: &ViewNode, name: &str| v.children.iter().find(|c| c.name == name).cloned().unwrap();
    assert_eq!(v.category, "package_caches");
    assert_eq!(v.tier, None);
    let cacache = find(&v, "_cacache");
    assert_eq!(cacache.tier, Some(Tier::Safe));
    assert_eq!(cacache.rule_id.as_deref(), Some("npm-cache"));
    // Inside a matched folder, tiles inherit its tier.
    let inner = find(&cacache, "content-v2");
    assert_eq!(inner.rule_id.as_deref(), Some("npm-cache"));

    let desktop = build_view(
        &snap,
        &home.join("Desktop"),
        ViewMode::Category,
        Some((rules, &a)),
    )
    .unwrap();
    assert_eq!(find(&desktop, "atlas").category, "code_projects");
    assert_eq!(find(&desktop, "idle-proj").category, "code_projects");
    let idle_nm = find(&find(&desktop, "idle-proj"), "node_modules");
    assert_eq!(idle_nm.tier, Some(Tier::Review));
}

#[test]
fn providers_on_the_fixture_with_all_tools() {
    let Some(home) = sandbox() else { return };
    let snap = scanned(&home);
    let t = tempfile::tempdir().unwrap();
    for name in ["git", "pnpm", "brew", "ollama", "xcrun", "docker"] {
        let p = t.path().join(name);
        fs::write(&p, "").unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).unwrap();
    }
    let tools = Tools::new(vec![t.path().to_path_buf()], false);
    let runner = FakeRunner::default()
        .with(
            "xcrun simctl list devices unavailable -j",
            true,
            r#"{"devices":{"com.apple.CoreSimulator.SimRuntime.iOS-14-5":[{"udid":"AAAA-BBBB","name":"iPhone 12","isAvailable":false}]}}"#,
        )
        .with(
            "docker system df --format json",
            true,
            r#"{"Type":"Images","Reclaimable":"1.5GB (60%)"}
{"Type":"Local Volumes","Reclaimable":"9GB (90%)"}
{"Type":"Build Cache","Reclaimable":"300MB"}"#,
        );
    let rows = rows(&snap, &tools, &runner);
    let ids = by_id(&rows);

    let ollama = ids["ollama-models"];
    let models: Vec<&str> = ollama.items.iter().map(|i| i.name.as_str()).collect();
    assert_eq!(
        models,
        ["llama3:8b", "qwen:0.5b"],
        "llama3:latest shares every blob"
    );
    assert_eq!(
        ollama.items[0].method,
        MethodView::Command {
            display: "ollama rm llama3:8b".into()
        }
    );
    assert_eq!(
        ollama.items[0].status,
        ItemStatus::Blocked {
            reason: "Start Ollama to clean".into()
        }
    );

    let sims = ids["simulators-unavailable"];
    assert_eq!(sims.items[0].name, "iPhone 12 (iOS 14.5)");
    assert!(sims.items[0].path.ends_with("Devices/AAAA-BBBB"));

    let docker = ids["docker-unused"];
    assert_eq!(docker.size, 1_800_000_000);
    assert!(docker.size_is_estimate);
    assert_eq!(
        docker.items[0].status,
        ItemStatus::Blocked {
            reason: "Start Docker Desktop to clean".into()
        }
    );

    assert_eq!(
        ids["pnpm-store"].items[0].method,
        MethodView::Command {
            display: "pnpm store prune".into()
        }
    );
}
