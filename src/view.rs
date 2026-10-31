//! Treemap view model sent to the frontend (PLAN.md section 3).

use serde::{Deserialize, Serialize};

use crate::recommend::build::Analysis;
use crate::recommend::model::Tier;
use crate::rules::category::{self, MEDIA_SHARE};
use crate::rules::model::Rules;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ViewKind {
    Dir,
    File,
    /// The children beyond the largest, merged into one tile (PRD FR-EXP-2). It
    /// carries its folder's path; its own children are the merged items.
    Smaller,
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
    #[serde(default)]
    pub file_count: u64,
    /// Bytes of recommended items at or below this node (PRD FR-EXP-5).
    #[serde(default)]
    pub cleanable: u64,
    /// Protected, or a `~/Library` folder with nothing to recommend (PRD FR-EXP-6).
    #[serde(default)]
    pub system_managed: bool,
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

use crate::cleanup::protected::Protected;
use crate::error::{Error, Result};
use crate::scanner::tree::Snapshot;

/// Children smaller than this share of their parent merge into the smaller-items tile.
const MIN_SHARE_PERMILLE: u64 = 10;
const MAX_TILES: usize = 12;
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

/// The folder at `path` and its direct children (PRD FR-EXP-1).
pub fn build_view(snap: &Snapshot, path: &Path, ann: Annotate) -> Result<ViewNode> {
    match resolve(snap, path) {
        Some(Target::Dir(id)) => Ok(dir_view(snap, id, ann)),
        _ => Err(Error::NotInSnapshot(path.display().to_string())),
    }
}

/// At most 12 children of 1% or more of `parent`, largest first, then one tile
/// for the rest: the merged items plus whatever the folder holds that no child
/// lists (its own blocks, files beyond the top 20). Shared with the mock.
pub fn merge_smaller(parent: &ViewNode, mut children: Vec<ViewNode>) -> Vec<ViewNode> {
    children.sort_by(|a, b| b.size.cmp(&a.size).then_with(|| a.name.cmp(&b.name)));
    let min = parent.size.saturating_mul(MIN_SHARE_PERMILLE) / 1000;
    let mut shown = Vec::new();
    let mut merged = Vec::new();
    // A child that is already a smaller-items tile stands for leftovers, which the
    // parent's size accounts for.
    for c in children
        .into_iter()
        .filter(|c| c.size > 0 && c.kind != ViewKind::Smaller)
    {
        if shown.len() < MAX_TILES && c.size >= min {
            shown.push(c);
        } else {
            merged.push(c);
        }
    }
    let rest = parent
        .size
        .saturating_sub(shown.iter().map(|c| c.size).sum());
    if rest > 0 && (!merged.is_empty() || rest >= min.max(1)) {
        let name = match merged.len() {
            0 => "Other files".to_owned(),
            1 => "1 smaller item".to_owned(),
            n => format!("{n} smaller items"),
        };
        shown.push(ViewNode {
            id: format!("{}:smaller", parent.id),
            name,
            path: parent.path.clone(),
            size: rest,
            category: parent.category.clone(),
            tier: None,
            rule_id: None,
            kind: ViewKind::Smaller,
            file_count: 0,
            cleanable: merged.iter().map(|c| c.cleanable).sum(),
            system_managed: false,
            children: merged,
        });
    }
    shown
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

/// What the analysis adds per node: cleanable bytes and the system-managed test.
struct Context<'a> {
    analysis: &'a Analysis,
    protected: Protected,
    library: PathBuf,
}

impl Context<'_> {
    fn cleanable(&self, path: &Path) -> u64 {
        // ponytail: scans every item per tile; index items by node if views get slow.
        self.analysis
            .items
            .iter()
            .filter(|i| i.path.starts_with(path))
            .map(|i| i.size)
            .sum()
    }

    fn system_managed(&self, path: &Path, cleanable: u64) -> bool {
        self.protected.is_protected(path)
            || (cleanable == 0 && path.starts_with(&self.library) && path != self.library)
    }
}

fn leaf(id: String, name: &str, path: &Path, size: u64, kind: ViewKind, look: &Look) -> ViewNode {
    ViewNode {
        id,
        name: name.to_owned(),
        path: path.display().to_string(),
        size,
        category: look.category.clone(),
        tier: look.tier,
        rule_id: look.rule_id.clone(),
        kind,
        file_count: 0,
        cleanable: 0,
        system_managed: false,
        children: Vec::new(),
    }
}

fn dir_leaf(
    snap: &Snapshot,
    id: u32,
    path: &Path,
    ann: Annotate,
    ctx: Option<&Context>,
) -> ViewNode {
    let node = &snap.nodes[id as usize];
    let mut view = leaf(
        id.to_string(),
        &node.name,
        path,
        node.size,
        ViewKind::Dir,
        &look(snap, id, ann),
    );
    view.file_count = node.file_count;
    if let Some(ctx) = ctx {
        view.cleanable = ctx.cleanable(path);
        view.system_managed = ctx.system_managed(path, view.cleanable);
    }
    view
}

fn dir_view(snap: &Snapshot, id: u32, ann: Annotate) -> ViewNode {
    let ctx = ann.map(|(rules, analysis)| {
        let protect: Vec<String> = rules.protect.iter().map(|p| p.path.clone()).collect();
        Context {
            analysis,
            protected: Protected::new(&snap.meta.root, &protect),
            library: Path::new(&snap.meta.root).join("Library"),
        }
    });
    let node = &snap.nodes[id as usize];
    let path = dir_path(snap, id);
    let mut view = dir_leaf(snap, id, &path, ann, ctx.as_ref());
    // Listed files take their folder's look: a matched folder's files are matched too.
    let look = look(snap, id, ann);
    let children = node
        .children
        .iter()
        .map(|&c| {
            let child = path.join(&snap.nodes[c as usize].name);
            dir_leaf(snap, c, &child, ann, ctx.as_ref())
        })
        .chain(node.top_files.iter().map(|f| {
            let file = path.join(&f.name);
            let mut v = leaf(
                format!("{id}:f:{}", f.name),
                &f.name,
                &file,
                f.size,
                ViewKind::File,
                &look,
            );
            v.cleanable = ctx.as_ref().map_or(0, |c| c.cleanable(&file));
            v
        }))
        .collect();
    view.children = merge_smaller(&view, children);
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
                    ..Default::default()
                },
                scanned_size: nodes[0].size,
                inaccessible_count: 0,
            },
            nodes,
        }
    }

    fn view(s: &Snapshot, p: &str) -> ViewNode {
        build_view(s, Path::new(p), None).unwrap()
    }

    fn names(v: &ViewNode) -> Vec<&str> {
        v.children.iter().map(|c| c.name.as_str()).collect()
    }

    fn node(name: &str, size: u64) -> ViewNode {
        ViewNode {
            id: name.into(),
            name: name.into(),
            path: format!("/r/{name}"),
            size,
            category: DEFAULT_CATEGORY.into(),
            tier: None,
            rule_id: None,
            kind: ViewKind::Dir,
            file_count: 0,
            cleanable: 0,
            system_managed: false,
            children: Vec::new(),
        }
    }

    #[test]
    fn shows_one_level() {
        let s = snap(|b| {
            b.push_dir(1, "a".into(), 0, NodeFlags::default());
            b.push_dir(2, "b".into(), 0, NodeFlags::default());
            b.add_file(3, "deep", 1000, 1000, None);
        });
        assert!(view(&s, "/r").children[0].children.is_empty());
    }

    #[test]
    fn at_most_12_tiles_then_one_smaller_tile() {
        let s = snap(|b| {
            for i in 0..20u64 {
                b.push_dir(1, format!("d{i:02}"), 0, NodeFlags::default());
                b.add_file(2, "f", 1000 + i, 1, None);
            }
        });
        let v = view(&s, "/r");
        assert_eq!(v.children.len(), 13);
    }

    #[test]
    fn the_smaller_tile_lists_what_it_merged() {
        let s = snap(|b| {
            for i in 0..20u64 {
                b.push_dir(1, format!("d{i:02}"), 0, NodeFlags::default());
                b.add_file(2, "f", 1000 + i, 1, None);
            }
        });
        let smaller = view(&s, "/r").children.pop().unwrap();
        assert_eq!(
            (smaller.kind, smaller.name.as_str(), smaller.children.len()),
            (ViewKind::Smaller, "8 smaller items", 8)
        );
    }

    #[test]
    fn tiles_and_the_smaller_tile_add_up_to_the_folder() {
        let s = snap(|b| {
            for i in 0..20u64 {
                b.push_dir(1, format!("d{i:02}"), 0, NodeFlags::default());
                b.add_file(2, "f", 1000 + i, 1, None);
            }
        });
        let v = view(&s, "/r");
        assert_eq!(v.children.iter().map(|c| c.size).sum::<u64>(), v.size);
    }

    #[test]
    fn children_under_one_percent_merge() {
        let s = snap(|b| {
            b.add_file(1, "big", 100_000, 1, None);
            b.add_file(1, "tiny", 900, 1, None); // 0.9%
        });
        assert_eq!(names(&view(&s, "/r")), ["big", "1 smaller item"]);
    }

    #[test]
    fn leftover_bytes_alone_read_as_other_files() {
        let parent = node("r", 100);
        let merged = merge_smaller(&parent, vec![node("a", 60), node("b", 30)]);
        assert_eq!(merged[2].name, "Other files");
    }

    #[test]
    fn a_mock_leftover_tile_counts_as_bytes_not_an_item() {
        let parent = node("r", 100);
        let leftover = ViewNode {
            kind: ViewKind::Smaller,
            ..node("Other", 10)
        };
        let merged = merge_smaller(&parent, vec![node("a", 90), leftover]);
        assert_eq!(
            names(&ViewNode {
                children: merged,
                ..parent
            }),
            ["a", "Other files"]
        );
    }

    #[test]
    fn files_beyond_top_twenty_count_in_the_smaller_tile() {
        let s = snap(|b| {
            for i in 0..30u64 {
                b.add_file(1, &format!("f{i:02}"), 1000, 1, None);
            }
        });
        let smaller = view(&s, "/r").children.pop().unwrap();
        assert_eq!(smaller.size, 18_000, "8 listed plus 10 unlisted files");
    }

    #[test]
    fn drills_into_a_subfolder() {
        let s = snap(|b| {
            b.push_dir(1, "a".into(), 0, NodeFlags::default());
            b.push_dir(2, "b".into(), 0, NodeFlags::default());
            b.add_file(3, "x", 5000, 1, None);
        });
        let v = view(&s, "/r/a");
        assert_eq!(
            (v.path.as_str(), v.children[0].path.as_str()),
            ("/r/a", "/r/a/b")
        );
    }

    #[test]
    fn unknown_and_escaping_paths_are_rejected() {
        let s = snap(|b| {
            b.push_dir(1, "a".into(), 0, NodeFlags::default());
        });
        for p in ["/r/nope", "/other", "/r/a/../a", "/r/../etc", "relative"] {
            assert!(build_view(&s, Path::new(p), None).is_err(), "{p}");
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
        assert!(build_view(&s, Path::new("/r/a/big.bin"), None).is_err());
    }
}
