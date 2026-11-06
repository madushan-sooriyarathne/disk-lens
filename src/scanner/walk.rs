//! Parallel, read-only directory walk. Uses `lstat` metadata only: symlinks are
//! never followed, other filesystems are never entered, and file contents are
//! never opened (so iCloud placeholders are not downloaded).

use std::os::macos::fs::MetadataExt as MacMeta;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use jwalk::{ClientState, DirEntry, WalkDirGeneric};

use super::ScanOptions;
use super::tree::{DirNode, LockfileAt, NodeFlags, TreeBuilder};
use crate::error::{Error, Result};

/// `SF_DATALESS`: the file's data lives in the cloud and is not on disk.
const SF_DATALESS: u32 = 0x4000_0000;
const PROGRESS_INTERVAL: Duration = Duration::from_millis(100);

#[derive(Debug, Clone)]
pub struct Progress {
    pub files: u64,
    pub bytes: u64,
    pub current: String,
    /// Bytes per category so far, in rules order (rough; see `category::Live`).
    pub categories: Vec<u64>,
}

pub struct WalkResult {
    pub nodes: Vec<DirNode>,
    pub lockfiles: Vec<LockfileAt>,
    pub inaccessible_count: u32,
}

/// What the parallel `process_read_dir` callback learned about one entry.
#[derive(Debug, Default, Clone)]
struct EntryStat {
    allocated: u64,
    logical: u64,
    dev: u64,
    ino: u64,
    nlink: u64,
    mount: bool,
    valid: bool,
    /// Category the containing folder decided, if any.
    category: Option<u8>,
}

#[derive(Debug, Default, Clone)]
struct Client;

impl ClientState for Client {
    /// Category the folder being read inherits or decides; passed down to its children.
    type ReadDirState = Option<u8>;
    type DirEntryState = EntryStat;
}

/// Dataless entries are placeholders; they occupy no local space.
pub fn is_dataless(st_flags: u32) -> bool {
    st_flags & SF_DATALESS != 0
}

/// Allocated bytes of an entry, never counting dataless placeholders.
pub(crate) fn allocated_bytes(md: &std::fs::Metadata) -> u64 {
    if is_dataless(md.st_flags()) {
        0
    } else {
        md.blocks() * 512
    }
}

pub fn walk(
    root: &Path,
    options: &ScanOptions,
    cancel: &Arc<AtomicBool>,
    on_progress: &mut dyn FnMut(&Progress),
) -> Result<WalkResult> {
    let root_md = std::fs::symlink_metadata(root)
        .map_err(|e| Error::Root(format!("{}: {e}", root.display())))?;
    if !root_md.is_dir() {
        return Err(Error::Root(format!(
            "{} is not a directory",
            root.display()
        )));
    }
    let root_dev = root_md.dev();

    let cb_cancel = cancel.clone();
    let live = options.live.clone();
    let cb_root = root.to_path_buf();
    let walker = WalkDirGeneric::<Client>::new(root)
        .follow_links(false)
        .skip_hidden(false)
        .sort(true) // path order: the first sighting of a hard link is its smallest path
        .process_read_dir(move |_depth, path, category, children| {
            if cb_cancel.load(Ordering::Relaxed) {
                children.clear();
                return;
            }
            if !live.is_empty() {
                let names = children
                    .iter()
                    .flatten()
                    .filter_map(|c| c.file_name.to_str());
                *category = live.dir(&cb_root, path, names, *category);
            }
            let category = *category;
            for child in children.iter_mut().flatten() {
                let md = match std::fs::symlink_metadata(child.path()) {
                    Ok(md) => md,
                    Err(_) => {
                        // Vanished or unreadable: skip it, and do not descend.
                        child.read_children = None;
                        continue;
                    }
                };
                let mount = child.file_type.is_dir() && md.dev() != root_dev;
                child.client_state = EntryStat {
                    allocated: allocated_bytes(&md),
                    logical: if is_dataless(md.st_flags()) {
                        0
                    } else {
                        md.len()
                    },
                    dev: md.dev(),
                    ino: md.ino(),
                    nlink: md.nlink(),
                    mount,
                    valid: true,
                    category,
                };
                if mount {
                    child.read_children = None; // never cross into another filesystem
                }
            }
        });

    let mut tree = TreeBuilder::with_media(options.media.clone());
    let live = &options.live;
    let mut categories = vec![0u64; live.len()];
    let mut inaccessible = 0u32;
    let (mut files, mut bytes) = (0u64, 0u64);
    let mut last_emit = Instant::now();
    let mut emitted = false;

    let root_name = root.file_name().map_or_else(
        || root.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    );
    tree.push_dir(
        0,
        root_name,
        allocated_bytes(&root_md),
        NodeFlags::default(),
    );

    for entry in walker {
        if cancel.load(Ordering::Relaxed) {
            return Err(Error::Cancelled);
        }
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let depth = entry.depth;
        if depth == 0 {
            // The root directory itself; a failed read shows up on its read_children.
            if read_failed(&entry) {
                tree.mark_inaccessible(0);
                inaccessible += 1;
            }
            continue;
        }
        let st = &entry.client_state;
        if !st.valid {
            continue;
        }
        let name = entry.file_name.to_string_lossy();
        if let Some(total) = categories.get_mut(usize::from(live.entry(
            st.category,
            !entry.file_type.is_dir() && options.media.is_media(&name),
        ))) {
            *total += st.allocated;
        }
        if entry.file_type.is_dir() {
            let flags = NodeFlags {
                mount: st.mount,
                ..NodeFlags::default()
            };
            tree.push_dir(depth, name.into_owned(), st.allocated, flags);
            bytes += st.allocated;
            if read_failed(&entry) {
                tree.mark_inaccessible(depth);
                inaccessible += 1;
            }
        } else {
            files += 1;
            bytes += st.allocated;
            let link = (st.nlink > 1).then_some((st.dev, st.ino));
            tree.add_file(depth, &name, st.allocated, st.logical, link);
        }
        if last_emit.elapsed() >= PROGRESS_INTERVAL {
            last_emit = Instant::now();
            emitted = true;
            on_progress(&Progress {
                files,
                bytes,
                current: entry.parent_path().display().to_string(),
                categories: categories.clone(),
            });
        }
    }

    // The callback stops descending once cancelled, so the loop can end early without noticing.
    if cancel.load(Ordering::Relaxed) {
        return Err(Error::Cancelled);
    }
    // A walk shorter than one interval still reports its counts once.
    if !emitted || last_emit.elapsed() >= PROGRESS_INTERVAL {
        on_progress(&Progress {
            files,
            bytes,
            current: root.display().to_string(),
            categories,
        });
    }
    Ok(WalkResult {
        lockfiles: tree.take_lockfiles(),
        nodes: tree.finish(),
        inaccessible_count: inaccessible,
    })
}

fn read_failed(entry: &DirEntry<Client>) -> bool {
    entry
        .read_children
        .as_ref()
        .is_some_and(|r| r.error().is_some())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dataless_flag() {
        assert!(is_dataless(0x4000_0000));
        assert!(is_dataless(0x4000_0000 | 0x20));
        assert!(!is_dataless(0));
        assert!(!is_dataless(0x20));
    }
}
