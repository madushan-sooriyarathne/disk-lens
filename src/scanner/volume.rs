//! Volume capacity. Prefers the "important usage" figure Finder shows (it counts
//! purgeable space as available), falling back to `statfs`. The gap between the
//! two is the purgeable total (PRD FR-SNAP-2).

use std::ffi::CString;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_foundation::{
    NSArray, NSNumber, NSString, NSURL, NSURLVolumeAvailableCapacityForImportantUsageKey,
    NSURLVolumeLocalizedNameKey, NSURLVolumeTotalCapacityKey,
};

use super::tree::VolumeInfo;

pub fn volume_info(path: &Path) -> Option<VolumeInfo> {
    ns_volume_info(path).or_else(|| statfs_volume_info(path))
}

fn ns_volume_info(path: &Path) -> Option<VolumeInfo> {
    let ns_path = NSString::from_str(&path.to_string_lossy());
    let url = NSURL::fileURLWithPath(&ns_path);
    // SAFETY: all three statics are valid NSString constants provided by Foundation.
    let (total_key, avail_key, name_key) = unsafe {
        (
            NSURLVolumeTotalCapacityKey,
            NSURLVolumeAvailableCapacityForImportantUsageKey,
            NSURLVolumeLocalizedNameKey,
        )
    };
    let keys = NSArray::from_slice(&[total_key, avail_key, name_key]);
    let values = url.resourceValuesForKeys_error(&keys).ok()?;
    let read = |key| -> Option<u64> {
        let obj: Retained<AnyObject> = values.objectForKey(key)?;
        let n = obj.downcast_ref::<NSNumber>()?;
        u64::try_from(n.longLongValue()).ok()
    };
    let total = read(total_key)?;
    let available = read(avail_key)?;
    let name = values
        .objectForKey(name_key)
        .and_then(|obj| obj.downcast_ref::<NSString>().map(|s| s.to_string()))
        .unwrap_or_default();
    // Without statfs there is no second figure to compare against.
    let free = statfs_volume_info(path).map_or(available, |v| v.available);
    Some(VolumeInfo {
        name,
        purgeable: available.saturating_sub(free),
        ..sanitize(total, available)?
    })
}

fn statfs_volume_info(path: &Path) -> Option<VolumeInfo> {
    let c = CString::new(path.as_os_str().as_bytes()).ok()?;
    let mut s = std::mem::MaybeUninit::<libc::statfs>::uninit();
    // SAFETY: `c` is a valid NUL-terminated path and `s` is a valid out-pointer.
    if unsafe { libc::statfs(c.as_ptr(), s.as_mut_ptr()) } != 0 {
        return None;
    }
    // SAFETY: statfs returned 0, so the struct is initialised.
    let s = unsafe { s.assume_init() };
    from_blocks(s.f_blocks, s.f_bavail, u64::from(s.f_bsize))
}

fn from_blocks(blocks: u64, avail_blocks: u64, block_size: u64) -> Option<VolumeInfo> {
    sanitize(
        blocks.checked_mul(block_size)?,
        avail_blocks.checked_mul(block_size)?,
    )
}

/// Reject nonsense (zero total, available above total).
fn sanitize(total: u64, available: u64) -> Option<VolumeInfo> {
    (total > 0 && available <= total).then(|| VolumeInfo {
        total,
        available,
        ..Default::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_convert_to_bytes() {
        let v = from_blocks(1000, 250, 4096).unwrap();
        assert_eq!((v.total, v.available), (4_096_000, 1_024_000));
    }

    #[test]
    fn rejects_impossible_values() {
        assert!(sanitize(0, 0).is_none());
        assert!(sanitize(10, 11).is_none());
        assert!(from_blocks(u64::MAX, 1, 4096).is_none());
    }

    #[test]
    fn root_volume_reports_capacity() {
        let v = volume_info(Path::new("/")).expect("capacity for /");
        assert!(v.total > 0 && v.available <= v.total);
        let fb = statfs_volume_info(Path::new("/")).expect("statfs for /");
        assert!(fb.total > 0);
    }

    #[test]
    fn root_volume_purgeable_fits_in_available() {
        let v = volume_info(Path::new("/")).expect("capacity for /");
        assert!(v.purgeable <= v.available);
    }

    #[test]
    fn root_volume_has_a_name() {
        let v = volume_info(Path::new("/")).expect("capacity for /");
        assert!(!v.name.is_empty());
    }
}
