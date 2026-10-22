//! Files with more than one hard link count once, attributed to the
//! lexicographically smallest path seen, so results do not depend on thread order.

use std::collections::HashMap;
use std::path::PathBuf;

pub struct LinkedFile {
    pub parent: u32,
    pub name: String,
    pub size: u64,
    pub logical: u64,
}

struct Owner {
    path: PathBuf,
    file: LinkedFile,
}

#[derive(Default)]
pub struct HardLinks {
    by_inode: HashMap<(u64, u64), Owner>,
}

impl HardLinks {
    #[allow(clippy::too_many_arguments)]
    pub fn record(
        &mut self,
        dev: u64,
        ino: u64,
        path: PathBuf,
        parent: u32,
        name: String,
        size: u64,
        logical: u64,
    ) {
        let file = LinkedFile {
            parent,
            name,
            size,
            logical,
        };
        match self.by_inode.entry((dev, ino)) {
            std::collections::hash_map::Entry::Vacant(v) => {
                v.insert(Owner { path, file });
            }
            std::collections::hash_map::Entry::Occupied(mut o) => {
                if path < o.get().path {
                    o.insert(Owner { path, file });
                }
            }
        }
    }

    /// One entry per inode, in a deterministic order.
    pub fn into_credits(self) -> Vec<LinkedFile> {
        let mut owners: Vec<Owner> = self.by_inode.into_values().collect();
        owners.sort_by(|a, b| a.path.cmp(&b.path));
        owners.into_iter().map(|o| o.file).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(h: &mut HardLinks, path: &str, parent: u32) {
        h.record(1, 7, path.into(), parent, path.into(), 4096, 10);
    }

    #[test]
    fn counted_once_attributed_to_smallest_path() {
        let mut a = HardLinks::default();
        rec(&mut a, "/r/b", 2);
        rec(&mut a, "/r/a", 1);
        let mut b = HardLinks::default();
        rec(&mut b, "/r/a", 1);
        rec(&mut b, "/r/b", 2);
        for h in [a, b] {
            let c = h.into_credits();
            assert_eq!(c.len(), 1);
            assert_eq!(c[0].parent, 1);
        }
    }

    #[test]
    fn distinct_inodes_both_counted() {
        let mut h = HardLinks::default();
        h.record(1, 1, "/a".into(), 0, "a".into(), 1, 1);
        h.record(1, 2, "/b".into(), 0, "b".into(), 1, 1);
        assert_eq!(h.into_credits().len(), 2);
    }
}
