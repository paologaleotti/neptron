//! Folder stamps: the modification time of each folder that a scan read,
//! so that a later refresh reads again only the folders that changed.
//!
//! A folder's modification time changes when an entry in it is made,
//! removed or renamed. It does not change when the content of a file in it
//! changes, and it does not change for an entry in a subfolder. The
//! refresh therefore checks every folder on its own.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// The age, in nanoseconds, that a folder time must have at the scan
/// before a refresh trusts it.
///
/// A file system keeps times at a coarse step: exFAT at 10 ms, ext4 at the
/// kernel tick (up to 10 ms), FAT at 2 s. An entry made in the same step
/// after the scan read the folder leaves the time as it was. A time older
/// than the largest step cannot hide such an entry, so the window is 2 s.
pub const MTIME_TRUST_WINDOW_NS: u64 = 2_000_000_000;

/// A modification time: seconds and nanoseconds from the Unix epoch.
/// Times before the epoch have negative seconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Mtime {
    /// Whole seconds from the Unix epoch.
    pub secs: i64,
    /// Nanoseconds in the second, below 1,000,000,000.
    pub nanos: u32,
}

/// What a scan saw of one folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderStamp {
    /// Path relative to `share/roms/`, as in a game path: the system folder
    /// first, then the subfolders, with `/` between the parts.
    pub path: String,
    /// The modification time before the scan read the folder, or `None`
    /// when a refresh must read the folder again: the time was too new to
    /// trust (see [`MTIME_TRUST_WINDOW_NS`]), or the folder or one of its
    /// entries could not be read.
    pub mtime: Option<Mtime>,
}

impl Mtime {
    /// The time of `time`. A time out of the range of `i64` seconds is
    /// clamped, which no file system gives.
    pub fn of(time: SystemTime) -> Self {
        match time.duration_since(UNIX_EPOCH) {
            Ok(after) => Self::from_parts(after, 1),
            Err(e) => Self::from_parts(e.duration(), -1),
        }
    }

    fn from_parts(d: Duration, sign: i64) -> Self {
        let secs = i64::try_from(d.as_secs()).unwrap_or(i64::MAX);
        if sign > 0 || d.subsec_nanos() == 0 {
            return Self {
                secs: secs.saturating_mul(sign),
                nanos: d.subsec_nanos(),
            };
        }
        Self {
            secs: -secs - 1,
            nanos: 1_000_000_000 - d.subsec_nanos(),
        }
    }

    /// Nanoseconds from the epoch, as a wide integer, for a difference.
    fn as_nanos(self) -> i128 {
        i128::from(self.secs) * 1_000_000_000 + i128::from(self.nanos)
    }
}

/// The time to keep for a folder whose modification time was `mtime` when
/// the scan read it at `read_at`: `mtime` when it is at least
/// [`MTIME_TRUST_WINDOW_NS`] older than `read_at`, else `None`. A folder
/// time after `read_at` (a clock set back) is not trusted either.
pub fn trusted(mtime: Mtime, read_at: Mtime) -> Option<Mtime> {
    let age = read_at.as_nanos() - mtime.as_nanos();
    (age >= i128::from(MTIME_TRUST_WINDOW_NS)).then_some(mtime)
}

/// The folder part of a game path: everything before the last `/`.
pub(crate) fn parent(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(dir, _)| dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(secs: i64, nanos: u32) -> Mtime {
        Mtime { secs, nanos }
    }

    #[test]
    fn a_time_converts_on_both_sides_of_the_epoch() {
        let t = UNIX_EPOCH + Duration::new(5, 250);
        assert_eq!(Mtime::of(t), at(5, 250));
        let before = UNIX_EPOCH - Duration::new(1, 250_000_000);
        assert_eq!(Mtime::of(before), at(-2, 750_000_000));
        assert_eq!(Mtime::of(UNIX_EPOCH - Duration::from_secs(3)), at(-3, 0));
        assert!(Mtime::of(before) < Mtime::of(UNIX_EPOCH));
    }

    #[test]
    fn only_a_time_older_than_the_window_is_trusted() {
        let mtime = at(100, 500);
        assert_eq!(trusted(mtime, at(102, 500)), Some(mtime));
        assert_eq!(trusted(mtime, at(102, 499)), None);
        assert_eq!(trusted(mtime, at(100, 500)), None);
        assert_eq!(trusted(mtime, at(50, 0)), None, "a clock set back");
    }

    #[test]
    fn the_parent_of_a_game_path_is_its_folder() {
        assert_eq!(parent("nes/sub/a.nes"), "nes/sub");
        assert_eq!(parent("nes/a.nes"), "nes");
        assert_eq!(parent("a.nes"), "");
    }
}
