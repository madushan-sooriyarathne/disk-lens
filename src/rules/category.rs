//! Category resolution for treemap colors (RULES.md section 6).
//!
//! Walking up from a node, the first folder that holds a project marker or
//! equals a category path decides. Otherwise the node's own media share picks
//! `documents_media` or `dev_tools`. Markers are checked on disk lazily and
//! memoised per node, so only folders that are actually shown cost a `stat`.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

use super::model::Rules;
use crate::scanner::tree::Snapshot;
use crate::view::{dir_id, dir_path};

pub const MEDIA: &str = "documents_media";
pub const OTHER: &str = "dev_tools";
/// Media fallback threshold: at least 6/10 of the node's bytes.
const MEDIA_SHARE: (u64, u64) = (6, 10);

pub struct Categories {
    ids: Vec<String>,
    /// `(category, marker names)` for categories that declare markers.
    markers: Vec<(usize, Vec<String>)>,
    /// Nodes that equal a category path.
    by_node: HashMap<u32, usize>,
    media: usize,
    other: usize,
    /// Own hit per node: `Some(category)` when the folder itself decides.
    own: Mutex<HashMap<u32, Option<usize>>>,
}

impl Categories {
    pub fn new(rules: &Rules, snap: &Snapshot) -> Self {
        let index = |id: &str| {
            rules
                .categories
                .iter()
                .position(|c| c.id == id)
                .expect("validated rules declare fallback categories")
        };
        let mut by_node = HashMap::new();
        for (i, c) in rules.categories.iter().enumerate() {
            for p in &c.paths {
                let rel = p.strip_prefix("~/").unwrap_or(p);
                if let Some(id) = dir_id(snap, &Path::new(&snap.meta.root).join(rel)) {
                    by_node.entry(id).or_insert(i);
                }
            }
        }
        Self {
            ids: rules.categories.iter().map(|c| c.id.clone()).collect(),
            markers: rules
                .categories
                .iter()
                .enumerate()
                .filter(|(_, c)| !c.markers.is_empty())
                .map(|(i, c)| (i, c.markers.clone()))
                .collect(),
            by_node,
            media: index(MEDIA),
            other: index(OTHER),
            own: Mutex::new(HashMap::new()),
        }
    }

    /// Category id of directory node `id`.
    pub fn of(&self, snap: &Snapshot, id: u32) -> &str {
        let mut cur = Some(id);
        while let Some(n) = cur {
            if let Some(c) = self.own_hit(snap, n) {
                return &self.ids[c];
            }
            cur = snap.nodes[n as usize].parent;
        }
        let node = &snap.nodes[id as usize];
        let (num, den) = MEDIA_SHARE;
        if node.size > 0 && node.media_bytes * den >= node.size * num {
            &self.ids[self.media]
        } else {
            &self.ids[self.other]
        }
    }

    fn own_hit(&self, snap: &Snapshot, id: u32) -> Option<usize> {
        if let Some(&c) = self.by_node.get(&id) {
            return Some(c);
        }
        // The scan root is skipped: a dotfiles repository in the home folder
        // would otherwise turn every unclaimed folder into a code project.
        if id == 0 {
            return None;
        }
        let mut own = self.own.lock().expect("category memo");
        *own.entry(id).or_insert_with(|| {
            let dir = dir_path(snap, id);
            self.markers
                .iter()
                .find(|(_, names)| names.iter().any(|m| dir.join(m).symlink_metadata().is_ok()))
                .map(|&(c, _)| c)
        })
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;

    use super::*;
    use crate::scanner::{ScanOptions, scan};

    fn put(root: &Path, rel: &str, blocks: usize) {
        let p = root.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, vec![b'x'; blocks * 4096]).unwrap();
    }

    fn setup(files: &[(&str, usize)]) -> (tempfile::TempDir, Snapshot, Categories) {
        let t = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(t.path()).unwrap();
        for (rel, blocks) in files {
            put(&root, rel, *blocks);
        }
        let rules = Rules::embedded();
        let snap = scan(
            &root,
            &ScanOptions::from_rules(rules),
            &Arc::new(AtomicBool::new(false)),
            &mut |_| {},
        )
        .unwrap();
        let cats = Categories::new(rules, &snap);
        (t, snap, cats)
    }

    fn cat<'a>(snap: &Snapshot, cats: &'a Categories, rel: &str) -> &'a str {
        let id = dir_id(snap, &Path::new(&snap.meta.root).join(rel)).expect(rel);
        cats.of(snap, id)
    }

    #[test]
    fn project_marker_claims_the_folder_and_everything_below() {
        let (_t, s, c) = setup(&[
            ("Desktop/app/package.json", 1),
            ("Desktop/app/src/deep/a.png", 10),
            ("Desktop/rs/Cargo.toml", 1),
            ("Desktop/loose/notes.txt", 1),
        ]);
        assert_eq!(cat(&s, &c, "Desktop/app"), "code_projects");
        assert_eq!(cat(&s, &c, "Desktop/app/src/deep"), "code_projects");
        assert_eq!(cat(&s, &c, "Desktop/rs"), "code_projects");
        assert_eq!(cat(&s, &c, "Desktop/loose"), OTHER);
        // Desktop itself has no marker; the fallback looks at its own bytes,
        // most of which are the project's png.
        assert_eq!(cat(&s, &c, "Desktop"), MEDIA);
    }

    #[test]
    fn category_paths_and_nearest_hit_wins() {
        let (_t, s, c) = setup(&[
            ("Library/Caches/Yarn/a", 1),
            ("Library/Developer/Xcode/b", 1),
            (".ollama/models/c", 1),
            ("Documents/work/app/package.json", 1),
            ("Documents/letters/a.txt", 1),
            ("Library/Containers/com.docker.docker/Data/d", 1),
        ]);
        assert_eq!(cat(&s, &c, "Library/Caches/Yarn"), "package_caches");
        assert_eq!(cat(&s, &c, "Library/Developer/Xcode"), "mobile_sdks");
        assert_eq!(cat(&s, &c, ".ollama/models"), "ai_models");
        // A project inside Documents is a project; other Documents folders are media.
        assert_eq!(cat(&s, &c, "Documents/work/app"), "code_projects");
        assert_eq!(cat(&s, &c, "Documents/letters"), MEDIA);
        assert_eq!(
            cat(&s, &c, "Library/Containers/com.docker.docker/Data"),
            OTHER
        );
        assert_eq!(cat(&s, &c, "Library"), OTHER);
    }

    #[test]
    fn media_fallback_at_sixty_percent() {
        let (_t, s, c) = setup(&[
            ("Desktop/shots/a.png", 6),
            ("Desktop/shots/b.txt", 4),
            ("Desktop/mixed/a.png", 5),
            ("Desktop/mixed/b.txt", 5),
        ]);
        assert_eq!(cat(&s, &c, "Desktop/shots"), MEDIA);
        assert_eq!(cat(&s, &c, "Desktop/mixed"), OTHER);
    }

    #[test]
    fn sixty_percent_boundary_is_inclusive() {
        let (_t, mut s, c) = setup(&[("Desktop/x/a.txt", 1)]);
        let id = dir_id(&s, &Path::new(&s.meta.root).join("Desktop/x")).unwrap() as usize;
        s.nodes[id].size = 1000;
        s.nodes[id].media_bytes = 600;
        assert_eq!(c.of(&s, id as u32), MEDIA);
        s.nodes[id].media_bytes = 599;
        assert_eq!(c.of(&s, id as u32), OTHER);
        s.nodes[id].size = 0;
        s.nodes[id].media_bytes = 0;
        assert_eq!(c.of(&s, id as u32), OTHER);
    }

    #[test]
    fn root_markers_are_ignored_and_results_memoised() {
        let (t, s, c) = setup(&[(".git/HEAD", 1), ("Desktop/a/b.txt", 1)]);
        assert_eq!(cat(&s, &c, "Desktop/a"), OTHER);
        // Memoised: a marker added after the first lookup does not change the answer.
        fs::write(
            fs::canonicalize(t.path()).unwrap().join("Desktop/a/go.mod"),
            "",
        )
        .unwrap();
        assert_eq!(cat(&s, &c, "Desktop/a"), OTHER);
    }
}
