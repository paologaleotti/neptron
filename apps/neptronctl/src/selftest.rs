//! `neptronctl selftest video`: one test mode per mode family of a monitor
//! profile, each from the policy and approved by `validate`. A dry run is
//! the default; the run on a display needs the confirmation flag with the
//! profile id.

mod args;
#[cfg(all(feature = "kms", target_os = "linux"))]
mod device;
mod error;
mod family;
mod monitor;
// Only the KMS build shows modes on a display; the tests run the display
// part on every host.
#[cfg_attr(not(all(feature = "kms", target_os = "linux")), allow(dead_code))]
mod on_display;
#[cfg_attr(not(all(feature = "kms", target_os = "linux")), allow(dead_code))]
mod pattern;
mod preflight;
mod report;
#[cfg_attr(not(all(feature = "kms", target_os = "linux")), allow(dead_code))]
mod show;
mod summary;
#[cfg_attr(not(all(feature = "kms", target_os = "linux")), allow(dead_code))]
mod timing;
#[cfg_attr(not(all(feature = "kms", target_os = "linux")), allow(dead_code))]
mod verdict;

use std::io::Write;
use std::path::Path;

pub use args::{VideoArgs, parse as parse_video};
pub use error::SelftestError;

use args::CONFIRM_FLAG;
use family::{Outcome, PlannedMode};
use monitor::Setup;

/// Where the process names are read.
const PROC_ROOT: &str = "/proc";

/// Runs `selftest video`. Without the confirmation flag it plans the
/// modes, prints the plan and writes the report, and sets no mode. With
/// it, it checks that the frontend does not hold the display and shows
/// each mode on the DRM device.
///
/// # Errors
///
/// [`SelftestError`]: the profile cannot be read, the run is refused, the
/// device cannot be used, a check fails, or the report or the output
/// cannot be written.
pub fn video(args: &VideoArgs, socket: &Path, out: &mut dyn Write) -> Result<(), SelftestError> {
    let setup = monitor::read(
        &args.profile,
        &args.monitors,
        &args.sysfs,
        args.connector.as_deref(),
    )?;
    let outcomes = family::plan(&setup.profile, &setup.caps);
    let Some(confirmed) = &args.confirmed else {
        return dry_run(args, &setup, &outcomes, out);
    };
    let modes: Vec<PlannedMode> = family::planned(&outcomes).cloned().collect();
    let frontend = preflight::look(socket, Path::new(PROC_ROOT));
    if let Some(reason) = preflight::refusal(confirmed, &setup.profile.id, modes.len(), &frontend) {
        return Err(SelftestError::Refused { reason });
    }
    on_device(args, &setup, &outcomes, &modes, out)
}

fn dry_run(
    args: &VideoArgs,
    setup: &Setup,
    outcomes: &[Outcome],
    out: &mut dyn Write,
) -> Result<(), SelftestError> {
    let plan = report::Report::planned(setup, outcomes, args.seconds_per_mode, true);
    report::write(&args.report, &plan)?;
    let text = format!(
        "Self-test video: dry run, no mode is set.\n{}Each mode shows for {} s.\n\
         Report: {}\n\
         To show the modes, check the ranges above against the monitor's manual, stop the \
         frontend and the launcher, and add:\n  {CONFIRM_FLAG} {}\n",
        summary::plan_text(setup, outcomes),
        args.seconds_per_mode,
        args.report.display(),
        setup.profile.id,
    );
    out.write_all(text.as_bytes())
        .map_err(|e| SelftestError::Output {
            reason: e.to_string(),
        })
}

#[cfg(not(all(feature = "kms", target_os = "linux")))]
fn on_device(
    _args: &VideoArgs,
    _setup: &Setup,
    _outcomes: &[Outcome],
    _modes: &[PlannedMode],
    _out: &mut dyn Write,
) -> Result<(), SelftestError> {
    Err(SelftestError::NoKms)
}

/// The report goes out before the device opens, so a report that cannot
/// be written stops the run before any modeset. The signals are watched
/// before the first modeset. A failure between the two writes the report
/// again as stopped, with the reason.
#[cfg(all(feature = "kms", target_os = "linux"))]
fn on_device(
    args: &VideoArgs,
    setup: &Setup,
    outcomes: &[Outcome],
    modes: &[PlannedMode],
    out: &mut dyn Write,
) -> Result<(), SelftestError> {
    let mut plan = report::Report::planned(setup, outcomes, args.seconds_per_mode, false);
    plan.display.device = Some(args.device.display().to_string());
    plan.modes.clear();
    report::write(&args.report, &plan)?;
    let started = (|| -> Result<_, SelftestError> {
        let stop = device::stop_flag()?;
        let crt = matches!(setup.profile.kind, neptron_types::MonitorKind::Crt { .. });
        let display = device::open(
            &args.device,
            args.connector.clone(),
            crt,
            setup.caps,
            on_display::buffer_size(modes, &setup.caps),
        )?;
        let facts = device::facts(&args.device, &display);
        if let Some(reason) = facts
            .connector
            .as_deref()
            .and_then(|c| preflight::connector_refusal(setup.gpu.connector.as_deref(), c))
        {
            return Err(SelftestError::Refused { reason });
        }
        Ok((stop, display, facts))
    })();
    let (stop, mut display, facts) = match started {
        Ok(started) => started,
        Err(e) => {
            // The error is what the caller must see; a report that cannot
            // be written now only keeps the `running` report of before.
            let _ = report::write(&args.report, &on_display::stopped_before_display(&plan, &e));
            return Err(e);
        }
    };
    let clock = device::Monotonic::new();
    on_display::run(
        &mut display,
        on_display::Inputs {
            setup,
            plan,
            modes,
            display: facts,
            report_path: &args.report,
            clock: &clock,
            stop: &stop,
        },
        out,
    )
}
