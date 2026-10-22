//! Table-driven rule matching over real scans (RULES.md section 8): a match and a
//! non-match for every rule, overlaps, protection and subtree consumption.

use std::collections::HashMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, SystemTime};

use disklens_lib::platform::{Facts, FakeRunner};
use disklens_lib::rules::guards::Env;
use disklens_lib::rules::matcher::Matcher;
use disklens_lib::rules::model::Rules;
use disklens_lib::rules::tools::Tools;
use disklens_lib::scanner::{ScanOptions, scan};
use disklens_lib::view::dir_path;

const ALL_TOOLS: [&str; 6] = ["git", "pnpm", "brew", "ollama", "xcrun", "docker"];
const OLD: u64 = 90 * 86_400;

struct Fixture {
    _tmp: tempfile::TempDir,
    home: PathBuf,
    bin: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let base = fs::canonicalize(tmp.path()).unwrap();
        let home = base.join("home");
        let bin = base.join("bin");
        fs::create_dir_all(&home).unwrap();
        fs::create_dir_all(&bin).unwrap();
        for t in ALL_TOOLS {
            fs::write(bin.join(t), "#!/bin/sh\n").unwrap();
            fs::set_permissions(bin.join(t), fs::Permissions::from_mode(0o755)).unwrap();
        }
        Self {
            _tmp: tmp,
            home,
            bin,
        }
    }

    /// A folder holding one 4 KiB file.
    fn dir(&self, rel: &str) {
        let d = self.home.join(rel);
        fs::create_dir_all(&d).unwrap();
        fs::write(d.join("f"), vec![b'x'; 4096]).unwrap();
    }

    /// A file last modified `age` seconds ago.
    fn file(&self, rel: &str, age: u64) {
        let p = self.home.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, "{}").unwrap();
        let when = SystemTime::now() - Duration::from_secs(age);
        fs::File::options()
            .write(true)
            .open(&p)
            .unwrap()
            .set_modified(when)
            .unwrap();
    }

    /// Relative path of every match -> rule id.
    fn matches(&self, tools: &Tools) -> HashMap<String, String> {
        let rules = Rules::embedded();
        let snap = scan(
            &self.home,
            &ScanOptions::from_rules(rules),
            &Arc::new(AtomicBool::new(false)),
            &mut |_| {},
        )
        .unwrap();
        let facts = Facts::default();
        let runner = FakeRunner::default();
        let env = Env {
            facts: &facts,
            runner: &runner,
            tools,
            now: SystemTime::now(),
        };
        let matcher = Matcher::new(rules, &snap.meta.root);
        matcher
            .run(rules, &snap, &env)
            .into_iter()
            .map(|m| {
                let path = dir_path(&snap, m.node);
                let rel = path.strip_prefix(&self.home).unwrap();
                (rel.display().to_string(), rules.rules[m.rule].id.clone())
            })
            .collect()
    }

    fn all_tools(&self) -> Tools {
        Tools::new(vec![self.bin.clone()], false)
    }
}

/// `(rule id, paths it must match, paths it must not match)`.
const TABLE: &[(&str, &[&str], &[&str])] = &[
    (
        "npm-cache",
        &[".npm/_cacache", ".npm/_npx", ".npm/_logs"],
        &[".npm/other", ".npmx/_cacache"],
    ),
    (
        "pnpm-store",
        &["Library/pnpm/store"],
        &["Library/pnpm/other"],
    ),
    (
        "bun-cache",
        &[".bun/install/cache"],
        &[".bun/install/other", ".bun/cache"],
    ),
    (
        "yarn-cache",
        &["Library/Caches/Yarn", ".yarn/berry/cache"],
        &[".yarn/cache"],
    ),
    (
        "next-turbo-build",
        &[
            "Desktop/web/.next",
            "Desktop/web/.turbo",
            "Desktop/mono/.turbo",
            "Desktop/deep/a/b/.next",
        ],
        &[
            "Desktop/bare/.next",
            "Library/App/.next",
            "Desktop/web/node_modules/pkg/.next",
            "Desktop/web/.nextx",
        ],
    ),
    (
        "gradle-caches",
        &[".gradle/caches", ".gradle/wrapper/dists", ".gradle/daemon"],
        &[".gradle/wrapper/other", ".gradle/native"],
    ),
    (
        "xcode-derived-data",
        &["Library/Developer/Xcode/DerivedData"],
        &["Library/Developer/Xcode/DerivedData.bak"],
    ),
    (
        "uv-puppeteer",
        &[".cache/uv", ".cache/puppeteer"],
        &[".cache/pip"],
    ),
    (
        "playwright-browsers",
        &["Library/Caches/ms-playwright"],
        &["Library/Caches/ms-playwright-go"],
    ),
    (
        "pip-cache",
        &["Library/Caches/pip"],
        &["Library/Caches/pipx"],
    ),
    (
        "homebrew-cache",
        &["Library/Caches/Homebrew"],
        &["Library/Caches/HomebrewX"],
    ),
    (
        "cocoapods-cache",
        &["Library/Caches/CocoaPods"],
        &["Library/Caches/CocoaPods-old"],
    ),
    ("go-build-cache", &["Library/Caches/go-build"], &["go/pkg"]),
    (
        "cargo-registry",
        &[".cargo/registry/cache", ".cargo/registry/src"],
        &[".cargo/registry/index", ".cargo/bin"],
    ),
    (
        "app-caches",
        &[
            "Library/Caches/com.example.app",
            "Library/Caches/ms-playwright-go",
            "Library/Caches/pipx",
        ],
        &[
            "Library/Caches",
            "Library/Caches/com.apple.Safari",
            "Library/Caches/CloudKit",
            "Library/Caches/GeoServices",
            "Library/Caches/familycircled",
            "Library/Caches/PassKit",
            "Library/Caches/Yarn",
            "Library/Caches/Homebrew",
            "Library/Caches/com.example.app/nested",
        ],
    ),
    (
        "android-sdk-emulators",
        &["Library/Android/sdk", ".android/avd", ".android/cache"],
        &["Library/Android/other", ".android/prefs"],
    ),
    ("ollama-models", &[".ollama/models"], &[".ollama/history"]),
    (
        "huggingface-cache",
        &[".cache/huggingface/hub"],
        &[".cache/huggingface/datasets"],
    ),
    (
        "lmstudio-models",
        &[".lmstudio/models", ".cache/lm-studio/models"],
        &[".lmstudio/conversations"],
    ),
    (
        "node-modules-idle",
        &["Desktop/idle/node_modules", "Desktop/web-idle/node_modules"],
        &[
            "Desktop/active/node_modules",
            "Desktop/active/node_modules/dep/node_modules",
            "Desktop/nopkg/node_modules",
            "Library/proj/node_modules",
            ".nvm/versions/node/lib/node_modules",
            ".vscode/extensions/x/node_modules",
        ],
    ),
    (
        "agent-worktrees",
        &["Desktop/atlas/.claude/worktrees", "Desktop/foo/.worktrees"],
        &["Library/app/.worktrees", "Desktop/atlas/claude/worktrees"],
    ),
    (
        "xcode-archives",
        &["Library/Developer/Xcode/Archives"],
        &["Library/Developer/Xcode/Archive"],
    ),
    (
        "ios-device-support",
        &[
            "Library/Developer/Xcode/iOS DeviceSupport",
            "Library/Developer/Xcode/watchOS DeviceSupport",
            "Library/Developer/Xcode/tvOS DeviceSupport",
            "Library/Developer/Xcode/visionOS DeviceSupport",
        ],
        &["Library/Developer/Xcode/macOS DeviceSupport"],
    ),
    (
        "simulators-unavailable",
        &["Library/Developer/CoreSimulator/Devices"],
        &["Library/Developer/CoreSimulator/Caches"],
    ),
    (
        "docker-unused",
        &["Library/Containers/com.docker.docker/Data/vms"],
        &["Library/Containers/com.docker.docker/Data/log"],
    ),
    (
        "ios-backups",
        &["Library/Application Support/MobileSync/Backup/0000-1111"],
        &["Library/Application Support/MobileSync/Backup"],
    ),
    ("trash-bin", &[".Trash"], &[".Trash2"]),
];

/// Files that make the guards in TABLE pass or fail as intended.
fn guard_files(f: &Fixture) {
    for p in [
        "Desktop/web/package.json",
        "Desktop/active/package.json",
        "Desktop/web/node_modules/pkg/package.json",
    ] {
        f.file(p, 0);
    }
    f.file("Desktop/mono/turbo.json", 0);
    for p in [
        "Desktop/deep/a/b/package.json",
        "Library/App/package.json",
        "Desktop/idle/package.json",
        "Desktop/web-idle/package.json",
        "Desktop/web-idle/package-lock.json",
        "Desktop/active/node_modules/dep/package.json",
        "Library/proj/package.json",
        ".nvm/versions/node/lib/package.json",
        ".vscode/extensions/x/package.json",
    ] {
        f.file(p, OLD);
    }
}

#[test]
fn every_rule_matches_and_misses_as_specified() {
    let ids: Vec<&str> = Rules::embedded()
        .rules
        .iter()
        .map(|r| r.id.as_str())
        .collect();
    let covered: Vec<&str> = TABLE.iter().map(|(id, _, _)| *id).collect();
    assert_eq!(covered, ids, "TABLE covers every rule, in file order");

    let f = Fixture::new();
    for (_, yes, no) in TABLE {
        for p in yes.iter().chain(no.iter()) {
            f.dir(p);
        }
    }
    guard_files(&f);
    let got = f.matches(&f.all_tools());

    for (id, yes, no) in TABLE {
        for p in *yes {
            assert_eq!(
                got.get(*p).map(String::as_str),
                Some(*id),
                "{p} should match {id}"
            );
        }
        for p in *no {
            assert_ne!(
                got.get(*p).map(String::as_str),
                Some(*id),
                "{p} must not match {id}"
            );
        }
    }
}

#[test]
fn specific_rules_beat_app_caches() {
    let f = Fixture::new();
    for p in [
        "Library/Caches/Yarn",
        "Library/Caches/Homebrew",
        "Library/Caches/pip",
        "Library/Caches/ms-playwright",
        "Library/Caches/go-build",
        "Library/Caches/CocoaPods",
        "Library/Caches/com.other",
    ] {
        f.dir(p);
    }
    let got = f.matches(&f.all_tools());
    assert_eq!(got["Library/Caches/Yarn"], "yarn-cache");
    assert_eq!(got["Library/Caches/Homebrew"], "homebrew-cache");
    assert_eq!(got["Library/Caches/pip"], "pip-cache");
    assert_eq!(got["Library/Caches/ms-playwright"], "playwright-browsers");
    assert_eq!(got["Library/Caches/go-build"], "go-build-cache");
    assert_eq!(got["Library/Caches/CocoaPods"], "cocoapods-cache");
    assert_eq!(got["Library/Caches/com.other"], "app-caches");
    assert_eq!(got.len(), 7);
}

#[test]
fn matched_subtrees_are_consumed() {
    let f = Fixture::new();
    f.dir(".npm/_cacache/x/node_modules");
    f.file(".npm/_cacache/x/package.json", OLD);
    f.dir("Desktop/idle/node_modules/a/.next");
    f.file("Desktop/idle/package.json", OLD);
    f.file("Desktop/idle/node_modules/a/package.json", 0);
    f.dir(".Trash/proj/node_modules");
    f.file(".Trash/proj/package.json", OLD);
    let got = f.matches(&f.all_tools());
    let mut keys: Vec<_> = got.keys().map(String::as_str).collect();
    keys.sort();
    assert_eq!(
        keys,
        [".Trash", ".npm/_cacache", "Desktop/idle/node_modules"]
    );
}

#[test]
fn protected_paths_stop_the_walk() {
    let f = Fixture::new();
    for p in [
        ".ssh/proj/node_modules",
        "Pictures/Photos Library.photoslibrary/proj/node_modules",
        "Library/Containers/com.apple.Notes/Data/Library/Caches/x",
        "Library/Mobile Documents/proj/.next",
    ] {
        f.dir(p);
    }
    for p in [
        ".ssh/proj/package.json",
        "Pictures/Photos Library.photoslibrary/proj/package.json",
        "Library/Mobile Documents/proj/package.json",
    ] {
        f.file(p, OLD);
    }
    assert!(f.matches(&f.all_tools()).is_empty());
}

#[test]
fn inaccessible_folders_never_match() {
    let f = Fixture::new();
    f.dir(".cache/uv");
    f.dir(".cache/puppeteer");
    let locked = f.home.join(".cache/uv");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
    let got = f.matches(&f.all_tools());
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(
        got.get(".cache/puppeteer").map(String::as_str),
        Some("uv-puppeteer")
    );
    assert!(!got.contains_key(".cache/uv"));
}

#[test]
fn missing_tools_hide_rules_unless_a_fallback_exists() {
    let f = Fixture::new();
    for p in [
        "Library/pnpm/store",
        "Library/Caches/Homebrew",
        ".ollama/models",
        "Library/Developer/CoreSimulator/Devices",
        "Library/Containers/com.docker.docker/Data/vms",
        "Desktop/atlas/.claude/worktrees",
    ] {
        f.dir(p);
    }
    let none = f.matches(&Tools::new(vec![], false));
    let mut keys: Vec<_> = none.keys().map(String::as_str).collect();
    keys.sort();
    assert_eq!(
        keys,
        ["Library/Caches/Homebrew"],
        "only the brew rule has a fallback"
    );

    // Sandbox mode resolves git only, even when the other tools exist.
    let sandbox = f.matches(&Tools::new(vec![f.bin.clone()], true));
    let mut keys: Vec<_> = sandbox.keys().map(String::as_str).collect();
    keys.sort();
    assert_eq!(
        keys,
        ["Desktop/atlas/.claude/worktrees", "Library/Caches/Homebrew"]
    );
}

#[test]
fn idle_is_measured_from_the_newest_evidence() {
    let f = Fixture::new();
    f.dir("Desktop/p/node_modules");
    f.file("Desktop/p/package.json", OLD);
    f.file("Desktop/p/yarn.lock", 86_400);
    assert!(
        f.matches(&f.all_tools()).is_empty(),
        "fresh lockfile keeps it active"
    );
    f.file("Desktop/p/yarn.lock", OLD);
    assert_eq!(
        f.matches(&f.all_tools())["Desktop/p/node_modules"],
        "node-modules-idle"
    );
}

#[test]
fn scan_root_with_glob_characters_is_literal() {
    let tmp = tempfile::tempdir().unwrap();
    let home = fs::canonicalize(tmp.path()).unwrap().join("we[ir]d*{home}");
    fs::create_dir_all(home.join(".npm/_cacache")).unwrap();
    fs::write(home.join(".npm/_cacache/f"), vec![0u8; 4096]).unwrap();
    let rules = Rules::embedded();
    let snap = scan(
        &home,
        &ScanOptions::from_rules(rules),
        &Arc::new(AtomicBool::new(false)),
        &mut |_| {},
    )
    .unwrap();
    let facts = Facts::default();
    let env = Env {
        facts: &facts,
        runner: &FakeRunner::default(),
        tools: &Tools::new(vec![], false),
        now: SystemTime::now(),
    };
    let got = Matcher::new(rules, &snap.meta.root).run(rules, &snap, &env);
    assert_eq!(got.len(), 1);
    assert_eq!(rules.rules[got[0].rule].id, "npm-cache");
    assert!(Path::new(&snap.meta.root).ends_with("we[ir]d*{home}"));
}
