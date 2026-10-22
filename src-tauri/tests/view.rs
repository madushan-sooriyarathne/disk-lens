//! End to end on real scans: scan -> get_view -> drill, without the UI.

use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use disklens_lib::scanner::{ScanOptions, scan};
use disklens_lib::view::{ViewKind, ViewMode, ViewNode, build_view, validated_path};

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

    let top = build_view(&snap, &root, ViewMode::Category).unwrap();
    assert_eq!(top.size, snap.meta.scanned_size);
    assert_eq!(top.children[0].name, "apps", "largest first");
    assert_sums(&top);

    let apps = build_view(&snap, &root.join("apps"), ViewMode::Category).unwrap();
    assert_eq!(apps.children[0].name, "web");
    let file = &apps.children[0].children[0];
    assert_eq!((file.name.as_str(), file.kind), ("f.bin", ViewKind::File));
    assert_eq!(
        validated_path(&snap, Path::new(&file.path)).unwrap(),
        root.join("apps/web/f.bin")
    );
    assert!(build_view(&snap, &root.join("apps/missing"), ViewMode::Category).is_err());
}

#[test]
fn sandbox_view_is_consistent() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../.sandbox/home");
    if !root.exists() {
        eprintln!("skipping: run scripts/make-fixture.sh first");
        return;
    }
    let root = fs::canonicalize(root).unwrap();
    let snap = scanned(&root);
    let v = build_view(&snap, &root, ViewMode::Category).unwrap();
    assert_sums(&v);
    assert!(v.children.iter().any(|c| c.name == "Desktop"));
    let desktop = build_view(&snap, &root.join("Desktop"), ViewMode::Cleanup).unwrap();
    assert_sums(&desktop);
}
