use std::collections::HashSet;
use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink};
use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use disklens_lib::error::Error;
use disklens_lib::scanner::scan;
use disklens_lib::scanner::tree::{DirNode, Snapshot};

fn no_cancel() -> Arc<AtomicBool> {
    Arc::new(AtomicBool::new(false))
}

fn scan_dir(root: &Path) -> Snapshot {
    scan(root, &no_cancel(), &mut |_| {}).expect("scan")
}

/// Writes a file whose size is a multiple of the 4 KiB APFS block.
fn put(path: &Path, blocks: usize) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, vec![b'x'; blocks * 4096]).unwrap();
}

/// Independent oracle: plain lstat walk, hard links once, symlinks not followed.
fn oracle(root: &Path) -> u64 {
    fn go(dir: &Path, seen: &mut HashSet<(u64, u64)>) -> u64 {
        let md = fs::symlink_metadata(dir).unwrap();
        let mut total = md.blocks() * 512;
        for e in fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            let m = fs::symlink_metadata(&p).unwrap();
            if m.is_dir() {
                total += go(&p, seen);
            } else if m.nlink() > 1 && !seen.insert((m.dev(), m.ino())) {
                continue;
            } else {
                total += m.blocks() * 512;
            }
        }
        total
    }
    go(root, &mut HashSet::new())
}

fn node<'a>(s: &'a Snapshot, name: &str) -> &'a DirNode {
    s.nodes
        .iter()
        .find(|n| n.name == name)
        .unwrap_or_else(|| panic!("no node {name}"))
}

#[test]
fn total_matches_independent_walk() {
    let t = tempfile::tempdir().unwrap();
    put(&t.path().join("a/one.bin"), 3);
    put(&t.path().join("a/deep/two.bin"), 5);
    put(&t.path().join("b/three.bin"), 1);
    put(&t.path().join("top.bin"), 2);
    let s = scan_dir(t.path());
    assert_eq!(s.meta.scanned_size, oracle(t.path()));
    assert_eq!(s.nodes[0].file_count, 4);
    assert_eq!(s.meta.inaccessible_count, 0);
    assert_eq!(s.nodes[0].logical_size, (3 + 5 + 1 + 2) * 4096);
}

#[test]
fn hard_link_counted_once_and_attributed_to_smallest_path() {
    let t = tempfile::tempdir().unwrap();
    put(&t.path().join("a/x.bin"), 4);
    fs::create_dir_all(t.path().join("b")).unwrap();
    fs::hard_link(t.path().join("a/x.bin"), t.path().join("b/y.bin")).unwrap();
    let s = scan_dir(t.path());
    assert_eq!(s.meta.scanned_size, oracle(t.path()));
    let (a, b) = (node(&s, "a"), node(&s, "b"));
    assert_eq!(a.top_files.len(), 1, "owner is a/x.bin, the smallest path");
    assert!(b.top_files.is_empty());
    assert!(a.size >= 4 * 4096 && b.size < 4096);
    assert_eq!(s.nodes[0].file_count, 1);
}

#[test]
fn symlink_is_recorded_but_not_followed() {
    let outside = tempfile::tempdir().unwrap();
    put(&outside.path().join("secret/big.bin"), 64);
    let t = tempfile::tempdir().unwrap();
    put(&t.path().join("real.bin"), 1);
    symlink(outside.path(), t.path().join("link")).unwrap();
    let s = scan_dir(t.path());
    assert_eq!(s.meta.scanned_size, oracle(t.path()));
    assert!(s.meta.scanned_size < 64 * 4096);
    assert!(s.nodes.iter().all(|n| n.name != "secret"));
    assert_eq!(s.nodes[0].file_count, 2, "the link counts as a file");
}

#[test]
fn unreadable_directory_is_inaccessible() {
    let t = tempfile::tempdir().unwrap();
    put(&t.path().join("ok/a.bin"), 1);
    put(&t.path().join("locked/b.bin"), 1);
    let locked = t.path().join("locked");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
    let s = scan_dir(t.path());
    // Restore so the tempdir can be removed.
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
    if unsafe { libc::geteuid() } == 0 {
        return; // root can read anything
    }
    assert_eq!(s.meta.inaccessible_count, 1);
    assert!(node(&s, "locked").flags.inaccessible);
    assert!(!node(&s, "ok").flags.inaccessible);
}

#[test]
fn top_files_are_capped_and_ordered() {
    let t = tempfile::tempdir().unwrap();
    for i in 1..=30 {
        put(&t.path().join(format!("f{i:02}.bin")), i);
    }
    let s = scan_dir(t.path());
    let top = &s.nodes[0].top_files;
    assert_eq!(top.len(), 20);
    assert_eq!(top[0].size, 30 * 4096);
    assert_eq!(top[19].size, 11 * 4096);
    assert_eq!(s.nodes[0].file_count, 30);
}

#[test]
fn cancel_before_start_returns_cancelled() {
    let t = tempfile::tempdir().unwrap();
    put(&t.path().join("a.bin"), 1);
    let cancel = Arc::new(AtomicBool::new(true));
    assert!(matches!(
        scan(t.path(), &cancel, &mut |_| {}),
        Err(Error::Cancelled)
    ));
}

#[test]
fn cancel_mid_walk_stops_within_a_second() {
    let t = tempfile::tempdir().unwrap();
    for d in 0..200 {
        for f in 0..50 {
            fs::create_dir_all(t.path().join(format!("d{d}"))).unwrap();
            fs::write(t.path().join(format!("d{d}/f{f}")), b"x").unwrap();
        }
    }
    let cancel = no_cancel();
    let flag = cancel.clone();
    let handle = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(5));
        flag.store(true, Ordering::Relaxed);
    });
    let started = Instant::now();
    let r = scan(t.path(), &cancel, &mut |_| {});
    handle.join().unwrap();
    assert!(started.elapsed() < Duration::from_secs(1));
    assert!(matches!(r, Err(Error::Cancelled)) || r.is_ok());
}

#[test]
fn progress_is_throttled_to_100ms() {
    let stamps = std::cell::RefCell::new(Vec::new());
    let started = Instant::now();
    scan(Path::new("/usr"), &no_cancel(), &mut |_| {
        stamps.borrow_mut().push(Instant::now());
    })
    .expect("scan /usr");
    let stamps = stamps.into_inner();
    for w in stamps.windows(2) {
        assert!(w[1] - w[0] >= Duration::from_millis(95), "events too close");
    }
    if started.elapsed() > Duration::from_millis(400) {
        assert!(!stamps.is_empty(), "long scan emitted no progress");
    }
}

#[test]
fn other_filesystems_are_not_entered() {
    let t = tempfile::tempdir().unwrap();
    let img = t.path().join("vol.dmg");
    let mnt = t.path().join("root/mnt");
    fs::create_dir_all(&mnt).unwrap();
    put(&t.path().join("root/plain.bin"), 1);
    let made = Command::new("hdiutil")
        .args(["create", "-size", "8m", "-fs", "APFS", "-volname", "dl"])
        .arg(&img)
        .output();
    if !made.is_ok_and(|o| o.status.success()) {
        eprintln!("skipping: hdiutil unavailable");
        return;
    }
    let attached = Command::new("hdiutil")
        .args(["attach", "-nobrowse", "-mountpoint"])
        .arg(&mnt)
        .arg(&img)
        .output();
    if !attached.is_ok_and(|o| o.status.success()) {
        eprintln!("skipping: could not attach image");
        return;
    }
    put(&mnt.join("inside.bin"), 100);
    let s = scan_dir(&t.path().join("root"));
    let _ = Command::new("hdiutil").arg("detach").arg(&mnt).output();
    assert!(node(&s, "mnt").flags.mount);
    assert!(s.nodes.iter().all(|n| n.name != "inside.bin"));
    assert!(s.meta.scanned_size < 100 * 4096, "mount contents leaked in");
}

#[test]
fn sandbox_fixture_smoke() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../.sandbox/home");
    if !root.exists() {
        eprintln!("skipping: run scripts/make-fixture.sh first");
        return;
    }
    let s = scan_dir(&root);
    assert_eq!(s.meta.scanned_size, oracle(&root));
    assert!(
        s.nodes.iter().all(|n| n.name != "secret.txt"),
        "symlink to sandbox/outside must not be followed"
    );
    assert!(s.nodes.iter().any(|n| n.name == "Keychains"));
}
