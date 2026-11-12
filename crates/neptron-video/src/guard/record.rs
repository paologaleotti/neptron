use crate::Monitor;

/// What the guard keeps across boots, apart from the user settings. The app
/// stores it in its own file on the state partition, written as temp file,
/// fsync, rename, and gives it back to [`crate::Guard::boot`].
///
/// A record that cannot be read is [`GuardRecord::default`]: no last known
/// good, no failed boot. The next boot then asks for a confirmation, and
/// the safe default comes back when nobody confirms.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GuardRecord {
    /// The monitor of the last confirmed configuration. It changes only
    /// when the user confirms a change. `None` when nothing was ever
    /// confirmed, or when the app cannot build the stored profile again.
    pub last_known_good: Option<Monitor>,
    /// Boots in a row that did not reach a confirmed picture. Each boot
    /// adds one when it starts. A confirmation or a good boot sets it to 0.
    pub failed_boots: u32,
    /// The next boot starts in safe mode. Set by the blind recovery and by
    /// safe mode after failed boots. Only a confirmation clears it.
    pub safe_mode: bool,
}
