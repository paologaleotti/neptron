//! The errors of the self-test.

use std::path::PathBuf;

/// Why a self-test cannot plan, start or finish.
#[derive(Debug, thiserror::Error)]
pub enum SelftestError {
    /// A monitor preset cannot be used.
    #[error("the monitor preset {} cannot be used: {reason}", path.display())]
    Preset {
        /// The preset file.
        path: PathBuf,
        /// What is wrong.
        reason: String,
    },
    /// The EDID profile is asked for and there is none.
    #[error("no EDID profile: {reason}; name a preset with --profile")]
    NoEdidProfile {
        /// Why.
        reason: String,
    },
    /// Standard output cannot be written.
    #[error("cannot write to standard output: {reason}")]
    Output {
        /// The error of the system.
        reason: String,
    },
    /// The report cannot be written.
    #[error("cannot write the report {}: {reason}", path.display())]
    Report {
        /// The report file.
        path: PathBuf,
        /// The error of the system.
        reason: String,
    },
}
