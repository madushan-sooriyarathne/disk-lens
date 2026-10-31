//! Treemap view model sent to the frontend (PLAN.md section 3).

use serde::{Deserialize, Serialize};

use crate::recommend::build::Analysis;
use crate::recommend::model::Tier;
use crate::rules::category::{self, MEDIA_SHARE};
use crate::rules::model::Rules;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ViewMode {
    Category,
    Cleanup,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ViewKind {
    Dir,
    File,
    /// Several small siblings merged into one tile.
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ViewNode {
    pub id: String,
    pub name: String,
    pub path: String,
    pub size: u64,
    pub category: String,
    pub tier: Option<Tier>,
    pub rule_id: Option<String>,
    pub kind: ViewKind,
    pub children: Vec<ViewNode>,
}

/// What a category owns, for the sidebar (PRD FR-WIN-1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CategoryTotal {
    pub category: String,
    pub size: u64,
    pub folders: u32,
}

// ---------------------------------------------------------------------------
// Building the view from a snapshot
// ---------------------------------------------------------------------------

use std::path::{Component, Path, PathBuf};

use crate::error::{Error, Result};
use crate::scanner::tree::{DirNode, Snapshot};

/// Children smaller than this share of their parent merge into "Other".
const MIN_SHARE_PERMILLE: u64 = 5;
const MAX_CHILDREN: usize = 24;
/// Category when no analysis is available.
const DEFAULT_CATEGORY: &str = "dev_tools";

/// Rules and the current analysis, to fill category, tier and rule per tile.
pub type Annotate<'a> = Option<(&'a Rules, &'a Analysis)>;

/// How a tile is colored.
#[derive(Clone)]
struct Look {
    category: String,
    tier: Option<Tier>,
    rule_id: Option<String>,
}

fn look(snap: &Snapshot, id: u32, ann: Annotate) -> Look {
    let Some((rules, analysis)) = ann else {
        return Look {
            category: DEFAULT_CATEGORY.to_owned(),
            tier: None,
            rule_id: None,
        };
    };
    match analysis.rule_of(snap, id) {
        // Matched folders and everything inside them take the rule's category.
        Some(r) => {
            let rule = &rules.rules[r];
            Look {
                category: rule.category.clone(),
                tier: Some(rule.tier),
                rule_id: Some(rule.id.clone()),
            }
        }
        None => Look {
            category: analysis.categories.of(snap, id).to_owned(),
            tier: None,
            rule_id: None,
        },
    }
}

/// Where a path lands in the snapshot.
enum Target {
    Dir(u32),
    File(u32, usize),
}

fn resolve(snap: &Snapshot, path: &Path) -> Option<Target> {
    let rel = path.strip_prefix(&snap.meta.root).ok()?;
    let mut id = 0u32;
    let mut parts = rel.components().peekable();
    while let Some(part) = parts.next() {
        let Component::Normal(name) = part else {
            return None; // "..", ".", prefixes: never resolve
        };
        let name = name.to_string_lossy();
        let node = &snap.nodes[id as usize];
        if let Some(&child) = node
            .children
            .iter()
            .find(|&&c| snap.nodes[c as usize].name == name)
        {
            id = child;
        } else if parts.peek().is_none()
            && let Some(i) = node.top_files.iter().position(|f| f.name == name)
        {
            return Some(Target::File(id, i));
        } else {
            return None;
        }
    }
    Some(Target::Dir(id))
}

/// The node id of the scanned directory at `path`.
pub fn dir_id(snap: &Snapshot, path: &Path) -> Option<u32> {
    match resolve(snap, path)? {
        Target::Dir(id) => Some(id),
        Target::File(..) => None,
    }
}

/// Absolute path of directory node `id`.
pub fn dir_path(snap: &Snapshot, id: u32) -> PathBuf {
    let mut names = Vec::new();
    let mut cur = id;
    while let Some(parent) = snap.nodes[cur as usize].parent {
        names.push(snap.nodes[cur as usize].name.as_str());
        cur = parent;
    }
    let mut path = PathBuf::from(&snap.meta.root);
    path.extend(names.iter().rev());
    path
}

/// The absolute path for `path` if it is a scanned directory or a listed large file.
pub fn validated_path(snap: &Snapshot, path: &Path) -> Result<PathBuf> {
    match resolve(snap, path) {
        Some(Target::Dir(id)) => Ok(dir_path(snap, id)),
        Some(Target::File(dir, i)) => {
            Ok(dir_path(snap, dir).join(&snap.nodes[dir as usize].top_files[i].name))
        }
        None => Err(Error::NotInSnapshot(path.display().to_string())),
    }
}

/// The folder at `path` plus two levels below it. Mode only changes colors,
/// which the frontend applies.
pub fn build_view(
    snap: &Snapshot,
    path: &Path,
    _mode: ViewMode,
    ann: Annotate,
) -> Result<ViewNode> {
    match resolve(snap, path) {
        Some(Target::Dir(id)) => Ok(dir_view(snap, id, 2, ann)),
        _ => Err(Error::NotInSnapshot(path.display().to_string())),
    }
}

/// Levels below the root searched for claimed folders.
// ponytail: projects nested deeper than this fall back to dev_tools/documents_media;
// raise it, or memoise marker stats across calls, if real trees need more.
const MAX_DEPTH: u32 = 4;

/// Bytes per category, every folder counted once. A folder is claimed by the
/// rule that matched it or by its own marker or category path; what no folder
/// claims is assigned per top-level folder by its media share. One entry per
/// rules category, in rules order.
pub fn category_totals(snap: &Snapshot, rules: &Rules, analysis: &Analysis) -> Vec<CategoryTotal> {
    let mut totals: Vec<CategoryTotal> = rules
        .categories
        .iter()
        .map(|c| CategoryTotal {
            category: c.id.clone(),
            size: 0,
            folders: 0,
        })
        .collect();
    let root = &snap.nodes[0];
    let (mut children_size, mut children_media) = (0u64, 0u64);
    for &child in &root.children {
        let node = &snap.nodes[child as usize];
        let (claimed, claimed_media) = claim(snap, rules, analysis, &mut totals, child, 1);
        let rest = node.size.saturating_sub(claimed);
        if rest > 0 {
            let media = node.media_bytes.saturating_sub(claimed_media);
            add(&mut totals, fallback(rest, media), rest, 1);
        }
        children_size = children_size.saturating_add(node.size);
        children_media = children_media.saturating_add(node.media_bytes);
    }
    let files = root.size.saturating_sub(children_size);
    if files > 0 {
        let media = root.media_bytes.saturating_sub(children_media);
        add(&mut totals, fallback(files, media), files, 0);
    }
    totals
}

/// Adds the claimed folders at or below `id` to `totals` and returns their
/// size and media bytes.
fn claim(
    snap: &Snapshot,
    rules: &Rules,
    analysis: &Analysis,
    totals: &mut [CategoryTotal],
    id: u32,
    depth: u32,
) -> (u64, u64) {
    let node = &snap.nodes[id as usize];
    let category = match analysis.mark_rule(id) {
        Some(r) => Some(rules.rules[r].category.as_str()),
        None => analysis.categories.own(snap, id),
    };
    if let Some(category) = category {
        add(totals, category, node.size, 1);
        return (node.size, node.media_bytes);
    }
    if depth >= MAX_DEPTH {
        return (0, 0);
    }
    node.children.iter().fold((0, 0), |(size, media), &c| {
        let (s, m) = claim(snap, rules, analysis, totals, c, depth + 1);
        (size.saturating_add(s), media.saturating_add(m))
    })
}

fn fallback(size: u64, media: u64) -> &'static str {
    let (num, den) = MEDIA_SHARE;
    if media.saturating_mul(den) >= size.saturating_mul(num) {
        category::MEDIA
    } else {
        category::OTHER
    }
}

fn add(totals: &mut [CategoryTotal], category: &str, size: u64, folders: u32) {
    // Validated rules only name declared categories, so the lookup always hits.
    if let Some(t) = totals.iter_mut().find(|t| t.category == category) {
        t.size = t.size.saturating_add(size);
        t.folders += folders;
    }
}

enum Cand<'a> {
    Dir(&'a DirNode),
    File(&'a str, u64),
}

impl Cand<'_> {
    fn size(&self) -> u64 {
        match self {
            Cand::Dir(n) => n.size,
            Cand::File(_, s) => *s,
        }
    }
    fn name(&self) -> &str {
        match self {
            Cand::Dir(n) => &n.name,
            Cand::File(n, _) => n,
        }
    }
}

fn leaf(id: String, name: &str, path: String, size: u64, kind: ViewKind, look: &Look) -> ViewNode {
    ViewNode {
        id,
        name: name.to_owned(),
        path,
        size,
        category: look.category.clone(),
        tier: look.tier,
        rule_id: look.rule_id.clone(),
        kind,
        children: Vec::new(),
    }
}

fn dir_view(snap: &Snapshot, id: u32, levels: u8, ann: Annotate) -> ViewNode {
    let node = &snap.nodes[id as usize];
    let path = dir_path(snap, id);
    let path_str = path.display().to_string();
    let look = look(snap, id, ann);
    let mut view = leaf(
        id.to_string(),
        &node.name,
        path_str.clone(),
        node.size,
        ViewKind::Dir,
        &look,
    );
    if levels == 0 {
        return view;
    }

    let mut cands: Vec<Cand> = node
        .children
        .iter()
        .map(|&c| Cand::Dir(&snap.nodes[c as usize]))
        .chain(node.top_files.iter().map(|f| Cand::File(&f.name, f.size)))
        .collect();
    cands.sort_by(|a, b| b.size().cmp(&a.size()).then_with(|| a.name().cmp(b.name())));

    let min_size = node.size.saturating_mul(MIN_SHARE_PERMILLE) / 1000;
    let mut shown = 0u64;
    let mut merged_any = false;
    for (rank, cand) in cands.iter().enumerate() {
        if rank >= MAX_CHILDREN || cand.size() < min_size || cand.size() == 0 {
            merged_any = true;
            continue;
        }
        shown += cand.size();
        view.children.push(match cand {
            Cand::Dir(n) => dir_view(snap, n.id, levels - 1, ann),
            Cand::File(name, size) => leaf(
                format!("{id}:f:{name}"),
                name,
                path.join(name).display().to_string(),
                *size,
                ViewKind::File,
                &look,
            ),
        });
    }
    // Everything not shown: pruned siblings, files beyond the top 20, and the folder's own blocks.
    let rest = node.size.saturating_sub(shown);
    if rest > 0 && (merged_any || rest >= min_size.max(1)) {
        view.children.push(leaf(
            format!("{id}:other"),
            "Other",
            path_str,
            rest,
            ViewKind::Other,
            &look,
        ));
    }
    view
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::tree::{NodeFlags, SnapshotMeta, TreeBuilder, VolumeInfo};

    fn snap(build: impl FnOnce(&mut TreeBuilder)) -> Snapshot {
        let mut b = TreeBuilder::default();
        b.push_dir(0, "r".into(), 0, NodeFlags::default());
        build(&mut b);
        let nodes = b.finish();
        Snapshot {
            meta: SnapshotMeta {
                id: "t".into(),
                root: "/r".into(),
                scanned_at: 0,
                duration_ms: 0,
                volume: VolumeInfo {
                    total: 1,
                    available: 1,
                },
                scanned_size: nodes[0].size,
                inaccessible_count: 0,
            },
            nodes,
        }
    }

    fn view(s: &Snapshot, p: &str) -> ViewNode {
        build_view(s, Path::new(p), ViewMode::Category, None).unwrap()
    }

    #[test]
    fn root_plus_two_levels() {
        let s = snap(|b| {
            b.push_dir(1, "a".into(), 0, NodeFlags::default());
            b.push_dir(2, "b".into(), 0, NodeFlags::default());
            b.push_dir(3, "c".into(), 0, NodeFlags::default());
            b.add_file(4, "deep", 1000, 1000, None);
        });
        let v = view(&s, "/r");
        let a = &v.children[0];
        let b = &a.children[0];
        assert_eq!((a.name.as_str(), b.name.as_str()), ("a", "b"));
        assert!(b.children.is_empty(), "third level is cut");
        assert_eq!(b.size, 1000);
    }

    #[test]
    fn small_children_merge_into_other_and_sizes_add_up() {
        let s = snap(|b| {
            b.add_file(1, "big", 100_000, 1, None);
            for i in 0..10 {
                b.add_file(1, &format!("tiny{i}"), 10, 1, None); // 0.01% each
            }
        });
        let v = view(&s, "/r");
        let names: Vec<_> = v.children.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, vec!["big", "Other"]);
        assert_eq!(v.children[1].kind, ViewKind::Other);
        assert_eq!(v.children.iter().map(|c| c.size).sum::<u64>(), v.size);
    }

    #[test]
    fn at_most_24_children_beyond_that_merge() {
        let s = snap(|b| {
            for i in 0..30u64 {
                b.push_dir(1, format!("d{i:02}"), 0, NodeFlags::default());
                b.add_file(2, "f", 1000 + i, 1, None);
            }
        });
        let v = view(&s, "/r");
        assert_eq!(v.children.len(), 25, "24 tiles plus Other");
        assert_eq!(v.children.last().unwrap().kind, ViewKind::Other);
        assert_eq!(v.children.iter().map(|c| c.size).sum::<u64>(), v.size);
    }

    #[test]
    fn files_beyond_top_twenty_land_in_other() {
        let s = snap(|b| {
            for i in 0..30u64 {
                b.add_file(1, &format!("f{i:02}"), 1000, 1, None);
            }
        });
        let v = view(&s, "/r");
        let files = v
            .children
            .iter()
            .filter(|c| c.kind == ViewKind::File)
            .count();
        assert_eq!(files, 20);
        let other = v
            .children
            .iter()
            .find(|c| c.kind == ViewKind::Other)
            .unwrap();
        assert_eq!(other.size, 10_000);
    }

    #[test]
    fn drills_into_a_subfolder() {
        let s = snap(|b| {
            b.push_dir(1, "a".into(), 0, NodeFlags::default());
            b.push_dir(2, "b".into(), 0, NodeFlags::default());
            b.add_file(3, "x", 5000, 1, None);
        });
        let v = view(&s, "/r/a");
        assert_eq!(v.name, "a");
        assert_eq!(v.path, "/r/a");
        assert_eq!(v.children[0].path, "/r/a/b");
        assert_eq!(v.children[0].children[0].path, "/r/a/b/x");
    }

    #[test]
    fn unknown_and_escaping_paths_are_rejected() {
        let s = snap(|b| {
            b.push_dir(1, "a".into(), 0, NodeFlags::default());
        });
        for p in ["/r/nope", "/other", "/r/a/../a", "/r/../etc", "relative"] {
            assert!(
                build_view(&s, Path::new(p), ViewMode::Category, None).is_err(),
                "{p}"
            );
            assert!(validated_path(&s, Path::new(p)).is_err(), "{p}");
        }
    }

    #[test]
    fn validated_path_accepts_dirs_and_listed_files_only() {
        let s = snap(|b| {
            b.push_dir(1, "a".into(), 0, NodeFlags::default());
            b.add_file(2, "big.bin", 5000, 1, None);
        });
        assert_eq!(
            validated_path(&s, Path::new("/r/a/big.bin")).unwrap(),
            PathBuf::from("/r/a/big.bin")
        );
        assert!(validated_path(&s, Path::new("/r/a/unlisted.bin")).is_err());
        assert!(build_view(&s, Path::new("/r/a/big.bin"), ViewMode::Category, None).is_err());
    }
}
