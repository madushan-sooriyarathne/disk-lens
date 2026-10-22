//! Files with more than one hard link count once, attributed to the
//! lexicographically smallest path.
//!
//! The walk is sorted by file name and visits parents before children, which is
//! exactly path order. So the first sighting of an inode is its smallest path and
//! no paths need to be stored (millions of hard links in pnpm stores would
//! otherwise cost gigabytes).

use std::collections::HashSet;

#[derive(Default)]
pub struct HardLinks {
    seen: HashSet<(u64, u64)>,
}

impl HardLinks {
    /// True the first time this `(dev, ino)` is seen; later links must not be counted.
    pub fn first_sighting(&mut self, dev: u64, ino: u64) -> bool {
        self.seen.insert((dev, ino))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_first_sighting_counts() {
        let mut h = HardLinks::default();
        assert!(h.first_sighting(1, 7));
        assert!(!h.first_sighting(1, 7));
        assert!(h.first_sighting(1, 8));
        assert!(
            h.first_sighting(2, 7),
            "same inode number on another device is distinct"
        );
    }
}
