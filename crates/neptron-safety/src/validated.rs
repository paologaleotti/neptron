use neptron_types::{GpuCaps, Modeline, MonitorProfile};

use crate::Rejection;

/// A mode that passed `validate`. It is the only input that can program a
/// display.
///
/// The field is private and the type has no `Default`, no public
/// constructor and no `Deserialize`, so `validate` is the only way to make
/// one:
///
/// ```compile_fail,E0277
/// let mode: neptron_safety::ValidatedMode = Default::default();
/// ```
///
/// ```compile_fail,E0451
/// fn forge(mode: neptron_types::Modeline) -> neptron_safety::ValidatedMode {
///     neptron_safety::ValidatedMode { mode }
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ValidatedMode {
    mode: Modeline,
}

impl ValidatedMode {
    /// The modeline that passed the checks.
    pub fn mode(&self) -> &Modeline {
        &self.mode
    }
}

/// Checks `mode` against the monitor profile, the GPU capabilities and the
/// hard rails, with integer maths only. It judges the modeline it gets and
/// changes nothing.
///
/// # Errors
///
/// Returns the first [`Rejection`] it finds, with the value and the limit.
///
/// # Panics
///
/// The checks are not implemented yet, so every call panics.
pub fn validate(
    mode: &Modeline,
    profile: &MonitorProfile,
    caps: &GpuCaps,
) -> Result<ValidatedMode, Rejection> {
    todo!("validate {mode:?} for profile {} on {caps:?}", profile.id)
}
