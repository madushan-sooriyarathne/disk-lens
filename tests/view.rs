//! End to end on real scans: scan -> get_view -> drill, without the UI.

use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use std::time::SystemTime;

use disklens_lib::platform::{Facts, FakeRunner};
use disklens_lib::recommend::build::Analysis;
use disklens_lib::recommend::build::analyze;
use disklens_lib::rules::guards::Env;
use disklens_lib::rules::model::Rules;
use disklens_lib::rules::tools::Tools;
use disklens_lib::scanner::tree::Snapshot;
use disklens_lib::scanner::{ScanOptions, scan};
use disklens_lib::view::{
    CategoryTotal, ViewKind, ViewNode, build_view, category_totals, validated_path,
};

fn scanned(root: &Path) -> disklens_lib::scanner::tree::Snapshot {
    scan(
        root,
        &ScanOptions::default(),
        &Arc::new(AtomicBool::new(false)),
        &mut |_| {},
    )
    .unwrap()
}

fn assert_sums(node: &ViewNode) {
    if !node.children.is_empty() {
        let sum: u64 = node.children.iter().map(|c| c.size).sum();
        assert!(sum <= node.size, "{}: children exceed parent", node.path);
        for c in &node.children {
            assert_sums(c);
        }
    }
}

#[test]
fn drill_down_over_a_real_scan() {
    let t = tempfile::tempdir().unwrap();
    let root = fs::canonicalize(t.path()).unwrap();
    for (dir, blocks) in [("apps/web", 40), ("apps/api", 10), ("docs", 5)] {
        fs::create_dir_all(root.join(dir)).unwrap();
        fs::write(root.join(dir).join("f.bin"), vec![0u8; blocks * 4096]).unwrap();
    }
    let snap = scanned(&root);

    let top = build_view(&snap, &root, None).unwrap();
    assert_eq!(top.size, snap.meta.scanned_size);
    assert_eq!(top.children[0].name, "apps", "largest first");
    assert_sums(&top);

    let apps = build_view(&snap, &root.join("apps"), None).unwrap();
    assert_eq!(apps.children[0].name, "web");
    let web = build_view(&snap, &root.join("apps/web"), None).unwrap();
    let file = &web.children[0];
    assert_eq!((file.name.as_str(), file.kind), ("f.bin", ViewKind::File));
    assert_eq!(
        validated_path(&snap, Path::new(&file.path)).unwrap(),
        root.join("apps/web/f.bin")
    );
    assert!(build_view(&snap, &root.join("apps/missing"), None).is_err());
}

#[test]
fn sandbox_view_is_consistent() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(".sandbox/home");
    if !root.exists() {
        eprintln!("skipping: run scripts/make-fixture.sh first");
        return;
    }
    let root = fs::canonicalize(root).unwrap();
    let snap = scanned(&root);
    let v = build_view(&snap, &root, None).unwrap();
    assert_sums(&v);
    assert!(v.children.iter().any(|c| c.name == "Desktop"));
    let desktop = build_view(&snap, &root.join("Desktop"), None).unwrap();
    assert_sums(&desktop);
}

/// A scanned tree of `files`, each `bytes` long, with the embedded rules' analysis.
fn analyzed(files: &[&str], bytes: usize) -> (tempfile::TempDir, Snapshot, Analysis) {
    let t = tempfile::tempdir().unwrap();
    let root = fs::canonicalize(t.path()).unwrap();
    for rel in files {
        let f = root.join(rel);
        fs::create_dir_all(f.parent().unwrap()).unwrap();
        fs::write(f, vec![b'x'; bytes]).unwrap();
    }
    let snap = scanned(&root);
    let facts = Facts::default();
    let runner = FakeRunner::default();
    let tools = Tools::new(vec![], false);
    let env = Env {
        facts: &facts,
        runner: &runner,
        tools: &tools,
        now: SystemTime::now(),
    };
    let analysis = analyze(Rules::embedded(), &snap, &env);
    (t, snap, analysis)
}

const TWO_MB: usize = 2_000_000;

/// The child `name` of the folder at `rel` under the scan root.
fn child(snap: &Snapshot, analysis: &Analysis, rel: &str, name: &str) -> ViewNode {
    let path = Path::new(&snap.meta.root).join(rel);
    let v = build_view(snap, &path, Some((Rules::embedded(), analysis))).unwrap();
    v.children.into_iter().find(|c| c.name == name).unwrap()
}

#[test]
fn cleanable_sums_the_items_inside_a_folder() {
    let (_t, snap, a) = analyzed(&[".npm/_cacache/a", ".npm/_logs/b", ".npm/keep"], TWO_MB);
    let npm = child(&snap, &a, "", ".npm");
    let items: u64 = a.items.iter().map(|i| i.size).sum();
    assert_eq!(npm.cleanable, items);
}

#[test]
fn a_folder_without_items_has_nothing_cleanable() {
    let (_t, snap, a) = analyzed(&[".npm/_cacache/a", "Documents/b"], TWO_MB);
    assert_eq!(child(&snap, &a, "", "Documents").cleanable, 0);
}

#[test]
fn a_protected_folder_is_system_managed() {
    let (_t, snap, a) = analyzed(&[".ssh/id", ".npm/_cacache/a"], TWO_MB);
    assert!(child(&snap, &a, "", ".ssh").system_managed);
}

#[test]
fn a_library_folder_with_nothing_to_clean_is_system_managed() {
    let (_t, snap, a) = analyzed(&["Library/Containers/x/a", "Library/Caches/pip/b"], TWO_MB);
    assert!(child(&snap, &a, "Library", "Containers").system_managed);
}

#[test]
fn a_library_folder_with_items_inside_is_not_system_managed() {
    let (_t, snap, a) = analyzed(&["Library/Containers/x/a", "Library/Caches/pip/b"], TWO_MB);
    assert!(!child(&snap, &a, "Library", "Caches").system_managed);
}

#[test]
fn a_user_folder_is_not_system_managed() {
    let (_t, snap, a) = analyzed(&["Documents/a", "Library/Caches/pip/b"], TWO_MB);
    assert!(!child(&snap, &a, "", "Documents").system_managed);
}

#[test]
fn the_smaller_tile_comes_last_and_lists_the_rest() {
    let files: Vec<String> = (0..15).map(|i| format!("d{i:02}/f")).collect();
    let refs: Vec<&str> = files.iter().map(String::as_str).collect();
    let (_t, snap, a) = analyzed(&refs, TWO_MB);
    let v = build_view(
        &snap,
        Path::new(&snap.meta.root),
        Some((Rules::embedded(), &a)),
    )
    .unwrap();
    let last = v.children.last().unwrap();
    assert_eq!((last.kind, last.children.len()), (ViewKind::Smaller, 3));
}

fn totals_of(files: &[&str]) -> (u64, Vec<CategoryTotal>) {
    let (_t, snap, analysis) = analyzed(files, 4096);
    let rules = Rules::embedded();
    let totals = category_totals(&snap, rules, &analysis);
    (snap.nodes[0].size, totals)
}

fn folders(totals: &[CategoryTotal], id: &str) -> u32 {
    totals.iter().find(|t| t.category == id).unwrap().folders
}

#[test]
fn category_totals_sum_to_the_root_size() {
    let (root_size, totals) = totals_of(&[
        "Documents/a.txt",
        "Library/Android/sdk/a",
        "Developer/x/Cargo.toml",
        "top.txt",
    ]);
    assert_eq!(totals.iter().map(|t| t.size).sum::<u64>(), root_size);
}

#[test]
fn a_rule_matched_folder_counts_once_for_its_category() {
    let (_, totals) = totals_of(&["Library/Android/sdk/a/f", "Library/Android/sdk/b/f"]);
    assert_eq!(folders(&totals, "mobile_sdks"), 1);
}

#[test]
fn a_folder_with_a_project_marker_is_a_code_project() {
    let (_, totals) = totals_of(&["Developer/x/Cargo.toml", "Developer/x/src/main.rs"]);
    assert_eq!(folders(&totals, "code_projects"), 1);
}

#[test]
fn a_project_deeper_than_the_depth_limit_is_not_claimed() {
    let (_, totals) = totals_of(&["a/b/c/d/e/Cargo.toml"]);
    assert_eq!(folders(&totals, "code_projects"), 0);
}
