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
    /// The run on a display must not start.
    #[error("the self-test does not start: {reason}")]
    Refused {
        /// Why.
        reason: String,
    },
    /// This build cannot drive a display.
    #[cfg_attr(all(feature = "kms", target_os = "linux"), allow(dead_code))]
    #[error("this build of neptronctl has no KMS backend; build it with the kms feature for Linux")]
    NoKms,
    /// The DRM device cannot be used.
    #[cfg_attr(not(all(feature = "kms", target_os = "linux")), allow(dead_code))]
    #[error("cannot use the DRM device {}: {reason}", path.display())]
    Device {
        /// The device node.
        path: PathBuf,
        /// What is wrong.
        reason: String,
    },
    /// A stop signal handler cannot be installed.
    #[cfg_attr(not(all(feature = "kms", target_os = "linux")), allow(dead_code))]
    #[error("cannot watch the stop signals: {reason}")]
    Signal {
        /// The error of the system.
        reason: String,
    },
    /// The self-test ran, and a check failed.
    #[error("the self-test failed; the report {} says why", report.display())]
    Failed {
        /// The report file.
        report: PathBuf,
    },
}
