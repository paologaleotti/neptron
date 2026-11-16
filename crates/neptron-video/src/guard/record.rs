use crate::Monitor;

/// What the guard keeps across boots, apart from the user settings. The app
/// stores it in its own file on the state partition, written as temp file,
/// fsync, rename, and gives it back to [`crate::Guard::boot`].
///
/// A record that cannot be read is [`GuardRecord::default`]: no last known
/// good, no failed boot.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GuardRecord {
    /// The monitor of the last confirmed configuration. It changes only
    /// when the user confirms a change. `None` when nothing was ever
    /// confirmed, or when the app cannot build the stored profile again.
    pub last_known_good: Option<Monitor>,
    /// Boots in a row that ended abnormally before a good picture: a new
    /// boot adds one when it finds [`GuardRecord::run_open`] set. A kept
    /// configuration sets it to 0, and so does a good picture outside safe
    /// mode.
    pub failed_boots: u32,
    /// The next boot starts in safe mode. Set by the blind recovery and by
    /// safe mode after failed boots. Only a kept configuration clears it.
    pub safe_mode: bool,
    /// A new boot started, and its run has not yet ended cleanly or shown
    /// a good picture. A new boot that finds it set counts the run before
    /// as a failed boot.
    pub run_open: bool,
}
