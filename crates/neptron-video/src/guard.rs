//! The guard: confirm-or-revert, last known good, safe mode and blind
//! recovery, over simulated time.
//!
//! [`Guard`] decides and does no I/O. It changes the display only through
//! the inputs of the engine (`set_monitor`, `set_settings`, `retry`), so
//! every mode it causes is a winner that the validator accepted for the
//! current monitor. What must outlive a boot is plain data,
//! [`GuardRecord`]: the guard gives it out in [`GuardStep::save`], and the
//! app writes it with the temp file, fsync, rename rule.

mod config;
mod machine;
mod record;

pub use config::{DisplayConfig, Health};
pub use machine::{
    ConfirmError, Guard, GuardEvent, GuardStep, Phase, Prompt, PromptId, RevertReason, SafeReason,
};
pub use record::GuardRecord;

/// How long a display change the user asks for stays without a
/// confirmation, in nanoseconds. Then the last confirmed configuration
/// comes back.
pub const CONFIRM_WINDOW_NS: u64 = 10_000_000_000;

/// The longest time that changes with no confirmation can stay, one after
/// the other, in nanoseconds. Each new change gets a new prompt, but its
/// deadline never passes the start of the first change plus this time, so
/// a loop of changes (from the control protocol, or a held key) cannot
/// keep an unconfirmed mode on the tube. Three confirm windows leave time
/// to step through the picture style rows of the Display panel.
pub const PENDING_LIMIT_NS: u64 = 3 * CONFIRM_WINDOW_NS;

/// Boots in a row that never reach a confirmed picture before the next
/// boot starts in safe mode.
pub const SAFE_MODE_FAILED_BOOTS: u32 = 2;

/// How long the confirmed configuration must show a picture before the
/// boot counts as good, in nanoseconds. A frontend that crashes or hangs
/// before this time counts as a failed boot.
pub const GOOD_BOOT_PICTURE_NS: u64 = 10_000_000_000;

/// How long the engine may report no valid picture (the output is off, a
/// mode the inputs refuse is on the display, or no mode is set) before the
/// guard leaves the configuration that causes it, in nanoseconds. The engine walks its
/// candidate list and retries device errors within a few frames, so a
/// shorter limit would revert on a transient state.
pub const NO_PICTURE_LIMIT_NS: u64 = 1_000_000_000;
