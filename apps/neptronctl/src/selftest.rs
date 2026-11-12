//! `neptronctl selftest video`: one test mode per mode family of a monitor
//! profile, each from the policy and approved by `validate`.

mod args;
mod error;
mod family;
mod monitor;
mod summary;

use std::io::Write;

pub use args::{VideoArgs, parse as parse_video};
pub use error::SelftestError;

/// Runs `selftest video`: plans the modes and prints the plan.
///
/// # Errors
///
/// [`SelftestError`] when the profile cannot be read or the output cannot
/// be written.
pub fn video(args: &VideoArgs, out: &mut dyn Write) -> Result<(), SelftestError> {
    let setup = monitor::read(
        &args.profile,
        &args.monitors,
        &args.sysfs,
        args.connector.as_deref(),
    )?;
    let outcomes = family::plan(&setup.profile, &setup.caps);
    let text = format!(
        "Self-test video: dry run, no mode is set.\n{}",
        summary::plan_text(&setup, &outcomes)
    );
    out.write_all(text.as_bytes())
        .map_err(|e| SelftestError::Output {
            reason: e.to_string(),
        })
}
