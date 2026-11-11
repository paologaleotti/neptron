//! Standalone hardware spike (WP 1.1). It shows 240p, 480i and 480p test
//! modes for a named preset and prints what the driver programmed.
//!
//! Every mode passes `neptron_safety::validate` before it reaches the
//! kernel, and the original display state comes back at the end. See the
//! README for the commands and the safety behaviour.

#![forbid(unsafe_code)]

mod args;
mod caps;
mod json;
#[cfg_attr(
    not(all(target_os = "linux", feature = "kms")),
    allow(
        dead_code,
        reason = "only the Linux KMS run uses it; the tests cover it on every host"
    )
)]
mod kernel_mode;
mod modes;
mod plan;
mod presets;
mod report;
#[cfg_attr(
    not(all(target_os = "linux", feature = "kms")),
    allow(
        dead_code,
        reason = "only the Linux KMS run uses it; the tests cover it on every host"
    )
)]
mod timing;

use std::path::Path;
use std::process::ExitCode;
use std::time::Duration;

use args::{CONFIRM_FLAG, Command, Options, USAGE};
use plan::Planned;
use presets::Preset;
use report::Report;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args_os()
        .skip(1)
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    match args::parse(&args) {
        Err(e) => {
            eprintln!("kms-spike: {e}\n\n{USAGE}");
            ExitCode::from(EXIT_USAGE)
        }
        Ok(Command::Help) => {
            print!("{USAGE}");
            ExitCode::SUCCESS
        }
        Ok(Command::ListPresets) => {
            for p in presets::all() {
                println!("{}", p.id);
            }
            ExitCode::SUCCESS
        }
        Ok(Command::ListConnectors { device }) => list_connectors(&device),
        Ok(Command::Run(options)) => run(&options, &args),
    }
}

/// Exit code for a command line or a build that cannot run the request.
const EXIT_USAGE: u8 = 2;
/// Time to open the device, probe, make the buffers, commit and restore,
/// in seconds.
const SETUP_ALLOWANCE_S: u64 = 15;
/// How long the supervisor waits past the worker's deadline before it kills
/// the worker, in seconds.
const KILL_MARGIN_S: u64 = 10;

fn run(o: &Options, args: &[String]) -> ExitCode {
    let Some(preset) = presets::find(&o.preset) else {
        eprintln!(
            "kms-spike: no preset {:?}; --list-presets prints the names",
            o.preset
        );
        return ExitCode::from(EXIT_USAGE);
    };
    let planned = plan::plan(preset, &o.modes, o.width, &o.allow);
    if o.worker {
        return worker(o, preset, &planned);
    }
    print!("{}", plan::describe_preset(preset));
    println!("{}", plan::describe_caps(&o.allow.caps()));
    for p in &planned {
        print!("{}", plan::describe_planned(p, &o.allow));
    }
    let valid = planned.iter().filter(|p| p.valid().is_some()).count();
    if !o.confirmed {
        println!(
            "\nDRY RUN: no mode was set. Check that the ranges above match the monitor, \
             then add {CONFIRM_FLAG} to show the {valid} valid mode(s), {} s each \
             (the run stops after {} s at most).",
            o.seconds,
            hard_limit(o).as_secs()
        );
        return finish_dry_run(o, preset, &planned);
    }
    if valid == 0 {
        println!("\nNo mode passed validation, so there is nothing to show.");
        return finish_dry_run(o, preset, &planned);
    }
    confirmed_run(o, args)
}

fn finish_dry_run(o: &Options, preset: &Preset, planned: &[Planned]) -> ExitCode {
    let report = Report::new(preset.id, &o.allow, o.width, o.seconds, false, planned);
    print!("\n{}", report.summary());
    if write_report(&report, &o.report) {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// Writes the report and says where. False when it cannot.
fn write_report(report: &Report, path: &Path) -> bool {
    match report.write(path) {
        Ok(()) => {
            println!("report written to {}", path.display());
            true
        }
        Err(e) => {
            eprintln!("kms-spike: cannot write the report {}: {e}", path.display());
            false
        }
    }
}

/// The time the worker may use: every mode plus the setup.
fn worker_limit(o: &Options) -> Duration {
    let modes = u64::try_from(o.modes.len()).unwrap_or(u64::MAX);
    Duration::from_secs(
        modes
            .saturating_mul(u64::from(o.seconds))
            .saturating_add(SETUP_ALLOWANCE_S),
    )
}

/// The time after which the supervisor kills the worker.
fn hard_limit(o: &Options) -> Duration {
    worker_limit(o) + Duration::from_secs(KILL_MARGIN_S)
}

fn list_connectors(_: &Path) -> ExitCode {
    no_kms()
}

fn confirmed_run(_: &Options, _: &[String]) -> ExitCode {
    no_kms()
}

fn worker(_: &Options, _: &Preset, _: &[Planned]) -> ExitCode {
    no_kms()
}

fn no_kms() -> ExitCode {
    eprintln!(
        "kms-spike: this build cannot set modes. DRM/KMS needs Linux and the kms feature: \
         cargo build -p kms-spike --features kms. The dry run works everywhere."
    );
    ExitCode::from(EXIT_USAGE)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options(modes: usize, seconds: u32) -> Options {
        match args::parse(&["ntsc".to_owned()]).unwrap() {
            Command::Run(mut o) => {
                o.modes.truncate(modes);
                o.seconds = seconds;
                o
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn the_limits_cover_every_mode_plus_setup_and_margin() {
        assert_eq!(worker_limit(&options(3, 10)), Duration::from_secs(45));
        assert_eq!(hard_limit(&options(3, 10)), Duration::from_secs(55));
        assert_eq!(hard_limit(&options(1, 1)), Duration::from_secs(26));
    }
}
