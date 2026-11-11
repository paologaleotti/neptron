use std::fmt::Display;

/// An error of the window backend when it opens.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum WindowError {
    /// An SDL3 call failed.
    #[error("SDL3 failed in {operation}: {reason}")]
    Sdl {
        /// The operation that failed, for example `window`.
        operation: &'static str,
        /// What SDL3 reported.
        reason: String,
    },
}

impl WindowError {
    pub(crate) fn sdl(operation: &'static str, reason: impl Display) -> Self {
        WindowError::Sdl {
            operation,
            reason: reason.to_string(),
        }
    }
}
