//! `neptronctl selftest video`: one test mode per mode family of a monitor
//! profile, each from the policy and approved by `validate`.

mod args;
mod error;
mod family;
mod monitor;
// Only the KMS build shows modes on a display; the tests run the display
// part on every host.
#[cfg_attr(not(all(feature = "kms", target_os = "linux")), allow(dead_code))]
mod pattern;
mod report;
#[cfg_attr(not(all(feature = "kms", target_os = "linux")), allow(dead_code))]
mod show;
mod summary;
#[cfg_attr(not(all(feature = "kms", target_os = "linux")), allow(dead_code))]
mod timing;
#[cfg_attr(not(all(feature = "kms", target_os = "linux")), allow(dead_code))]
mod verdict;

use std::io::Write;

pub use args::{VideoArgs, parse as parse_video};
pub use error::SelftestError;

/// The display time of each mode, in seconds.
pub const SECONDS_PER_MODE_DEFAULT_S: u32 = 10;

/// Runs `selftest video`: plans the modes, prints the plan and writes the
/// report.
///
/// # Errors
///
/// [`SelftestError`] when the profile cannot be read, or the report or the
/// output cannot be written.
pub fn video(args: &VideoArgs, out: &mut dyn Write) -> Result<(), SelftestError> {
    let setup = monitor::read(
        &args.profile,
        &args.monitors,
        &args.sysfs,
        args.connector.as_deref(),
    )?;
    let outcomes = family::plan(&setup.profile, &setup.caps);
    let plan = report::Report::planned(&setup, &outcomes, SECONDS_PER_MODE_DEFAULT_S, true);
    report::write(&args.report, &plan)?;
    let text = format!(
        "Self-test video: dry run, no mode is set.\n{}Report: {}\n",
        summary::plan_text(&setup, &outcomes),
        args.report.display()
    );
    out.write_all(text.as_bytes())
        .map_err(|e| SelftestError::Output {
            reason: e.to_string(),
        })
}
