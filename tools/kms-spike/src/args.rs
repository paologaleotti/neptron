//! The command line, parsed by hand: the dependency rules allow no
//! argument parser crate.

use std::fmt;
use std::path::PathBuf;

use crate::caps::{Allowances, CONSERVATIVE_CAPS};
use crate::modes::TestMode;

/// What the user asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Print the usage text.
    Help,
    /// Print the names of the embedded presets.
    ListPresets,
    /// List the connectors of a DRM device. No modeset.
    ListConnectors {
        /// The DRM device node.
        device: PathBuf,
    },
    /// Plan, validate and (with the confirmation) show the test modes.
    Run(Options),
}

/// The options of a run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    /// The preset name. There is no default.
    pub preset: String,
    /// The test modes, in the order they are shown.
    pub modes: Vec<TestMode>,
    /// Active pixels per line.
    pub width: u16,
    /// How long each mode shows, in seconds.
    pub seconds: u32,
    /// What the user allows beyond the conservative GPU values.
    pub allow: Allowances,
    /// The DRM device node.
    pub device: PathBuf,
    /// The connector name, for example `VGA-1`.
    pub connector: Option<String>,
    /// Where the JSON report goes.
    pub report: PathBuf,
    /// The user confirms that the preset matches the monitor. Without it the
    /// run is a dry run.
    pub confirmed: bool,
    /// The process is the worker that the supervisor starts.
    pub worker: bool,
}

/// A command line that cannot be parsed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArgsError {
    /// An option this program does not know.
    Unknown(String),
    /// An option without its value.
    MissingValue(&'static str),
    /// A value that is not valid for its option.
    BadValue {
        /// The option.
        option: &'static str,
        /// The value given.
        value: String,
        /// What the option accepts.
        expected: &'static str,
    },
    /// The same option twice.
    Repeated(&'static str),
    /// No preset is named.
    NoPreset,
    /// Two preset names.
    ExtraArgument(String),
    /// `--worker` without the confirmation flag. The worker sets modes, so
    /// it never runs unconfirmed.
    WorkerWithoutConfirmation,
}

/// The default DRM device node.
pub const DEFAULT_DEVICE: &str = "/dev/dri/card0";
/// The default report file, in the working directory.
pub const DEFAULT_REPORT: &str = "kms-spike-report.json";
/// The longest time a mode shows, in seconds.
pub const SECONDS_MAX: u32 = 10;
/// The default active width in pixels.
pub const DEFAULT_WIDTH_PX: u16 = 640;
/// The narrowest active width in pixels.
pub const WIDTH_MIN_PX: u16 = 64;
/// The widest active width in pixels.
pub const WIDTH_MAX_PX: u16 = 3840;
/// The lowest pixel clock a user can allow, in hertz.
pub const PCLOCK_ALLOW_MIN_HZ: u32 = 1_000_000;
/// The confirmation flag.
pub const CONFIRM_FLAG: &str = "--i-have-checked-the-monitor";
/// The flag that marks the worker process.
pub const WORKER_FLAG: &str = "--worker";

/// The usage text.
pub const USAGE: &str = "\
usage:
  kms-spike <PRESET> [options]       plan and validate the test modes (dry run)
  kms-spike <PRESET> [options] --i-have-checked-the-monitor
                                     show each valid mode on the display
  kms-spike --list-presets           print the embedded preset names
  kms-spike --list-connectors [--device PATH]
                                     list the connectors, set no mode
  kms-spike --help

options:
  --modes LIST           comma-separated subset of 240p,480i,480p (default: all)
  --width PX             active pixels per line, 64 to 3840 (default: 640)
  --seconds N            time per mode, 1 to 10 (default: 10)
  --allow-low-pclock HZ  allow pixel clocks down to HZ, below the 25 MHz of an
                         unknown GPU (at least 1000000)
  --allow-interlace      allow interlaced modes, off for an unknown GPU
  --device PATH          DRM device node (default: /dev/dri/card0)
  --connector NAME       connector to use, for example VGA-1 or DP-1
  --report PATH          JSON report file (default: kms-spike-report.json)
";

/// Parses the arguments after the program name.
pub fn parse(args: &[String]) -> Result<Command, ArgsError> {
    let mut p = Parser::default();
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        p.take(arg, &mut it)?;
    }
    p.finish()
}

#[derive(Default)]
struct Parser {
    help: bool,
    list_presets: bool,
    list_connectors: bool,
    preset: Option<String>,
    modes: Option<Vec<TestMode>>,
    width: Option<u16>,
    seconds: Option<u32>,
    pclock_min_hz: Option<u32>,
    interlace: bool,
    device: Option<PathBuf>,
    connector: Option<String>,
    report: Option<PathBuf>,
    confirmed: bool,
    worker: bool,
}

impl Parser {
    fn take<'a>(
        &mut self,
        arg: &str,
        rest: &mut impl Iterator<Item = &'a String>,
    ) -> Result<(), ArgsError> {
        let mut value =
            |option: &'static str| rest.next().cloned().ok_or(ArgsError::MissingValue(option));
        match arg {
            "--help" | "-h" => self.help = true,
            "--list-presets" => self.list_presets = true,
            "--list-connectors" => self.list_connectors = true,
            "--allow-interlace" => set_flag(&mut self.interlace, "--allow-interlace")?,
            CONFIRM_FLAG => set_flag(&mut self.confirmed, CONFIRM_FLAG)?,
            WORKER_FLAG => set_flag(&mut self.worker, WORKER_FLAG)?,
            "--modes" => set(&mut self.modes, "--modes", parse_modes(&value("--modes")?)?)?,
            "--width" => {
                let v = value("--width")?;
                set(&mut self.width, "--width", parse_width(&v)?)?;
            }
            "--seconds" => {
                let v = value("--seconds")?;
                set(&mut self.seconds, "--seconds", parse_seconds(&v)?)?;
            }
            "--allow-low-pclock" => {
                let v = value("--allow-low-pclock")?;
                set(
                    &mut self.pclock_min_hz,
                    "--allow-low-pclock",
                    parse_pclock(&v)?,
                )?;
            }
            "--device" => set(
                &mut self.device,
                "--device",
                PathBuf::from(value("--device")?),
            )?,
            "--connector" => set(&mut self.connector, "--connector", value("--connector")?)?,
            "--report" => set(
                &mut self.report,
                "--report",
                PathBuf::from(value("--report")?),
            )?,
            other if other.starts_with('-') => return Err(ArgsError::Unknown(other.to_owned())),
            name => {
                if self.preset.is_some() {
                    return Err(ArgsError::ExtraArgument(name.to_owned()));
                }
                self.preset = Some(name.to_owned());
            }
        }
        Ok(())
    }

    fn finish(self) -> Result<Command, ArgsError> {
        let device = self.device.unwrap_or_else(|| PathBuf::from(DEFAULT_DEVICE));
        if self.help {
            return Ok(Command::Help);
        }
        if self.list_presets {
            return Ok(Command::ListPresets);
        }
        if self.list_connectors {
            return Ok(Command::ListConnectors { device });
        }
        let preset = self.preset.ok_or(ArgsError::NoPreset)?;
        if self.worker && !self.confirmed {
            return Err(ArgsError::WorkerWithoutConfirmation);
        }
        Ok(Command::Run(Options {
            preset,
            modes: self.modes.unwrap_or_else(|| TestMode::ALL.to_vec()),
            width: self.width.unwrap_or(DEFAULT_WIDTH_PX),
            seconds: self.seconds.unwrap_or(SECONDS_MAX),
            allow: Allowances {
                pclock_min_hz: self.pclock_min_hz,
                interlace: self.interlace,
            },
            device,
            connector: self.connector,
            report: self.report.unwrap_or_else(|| PathBuf::from(DEFAULT_REPORT)),
            confirmed: self.confirmed,
            worker: self.worker,
        }))
    }
}

fn set<T>(slot: &mut Option<T>, option: &'static str, value: T) -> Result<(), ArgsError> {
    if slot.is_some() {
        return Err(ArgsError::Repeated(option));
    }
    *slot = Some(value);
    Ok(())
}

fn set_flag(slot: &mut bool, option: &'static str) -> Result<(), ArgsError> {
    if *slot {
        return Err(ArgsError::Repeated(option));
    }
    *slot = true;
    Ok(())
}

fn parse_modes(value: &str) -> Result<Vec<TestMode>, ArgsError> {
    let bad = || ArgsError::BadValue {
        option: "--modes",
        value: value.to_owned(),
        expected: "a comma-separated subset of 240p,480i,480p, each once",
    };
    let mut modes = Vec::new();
    for name in value.split(',') {
        let mode = TestMode::parse(name).ok_or_else(bad)?;
        if modes.contains(&mode) {
            return Err(bad());
        }
        modes.push(mode);
    }
    Ok(modes)
}

fn parse_width(value: &str) -> Result<u16, ArgsError> {
    value
        .parse::<u16>()
        .ok()
        .filter(|w| (WIDTH_MIN_PX..=WIDTH_MAX_PX).contains(w))
        .ok_or_else(|| ArgsError::BadValue {
            option: "--width",
            value: value.to_owned(),
            expected: "a whole number of pixels from 64 to 3840",
        })
}

fn parse_seconds(value: &str) -> Result<u32, ArgsError> {
    value
        .parse::<u32>()
        .ok()
        .filter(|s| (1..=SECONDS_MAX).contains(s))
        .ok_or_else(|| ArgsError::BadValue {
            option: "--seconds",
            value: value.to_owned(),
            expected: "a whole number of seconds from 1 to 10",
        })
}

fn parse_pclock(value: &str) -> Result<u32, ArgsError> {
    value
        .parse::<u32>()
        .ok()
        .filter(|hz| (PCLOCK_ALLOW_MIN_HZ..CONSERVATIVE_CAPS.pclock_min_hz).contains(hz))
        .ok_or_else(|| ArgsError::BadValue {
            option: "--allow-low-pclock",
            value: value.to_owned(),
            expected: "a pixel clock in hertz from 1000000 to 24999999",
        })
}

impl fmt::Display for ArgsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ArgsError::Unknown(o) => write!(f, "unknown option {o}"),
            ArgsError::MissingValue(o) => write!(f, "{o} needs a value"),
            ArgsError::BadValue {
                option,
                value,
                expected,
            } => write!(f, "{option} {value:?}: expected {expected}"),
            ArgsError::Repeated(o) => write!(f, "{o} is given twice"),
            ArgsError::NoPreset => {
                f.write_str("name a monitor preset; there is no default (see --list-presets)")
            }
            ArgsError::ExtraArgument(a) => write!(f, "unexpected argument {a:?}: name one preset"),
            ArgsError::WorkerWithoutConfirmation => write!(
                f,
                "{WORKER_FLAG} is internal and needs {CONFIRM_FLAG}; run without {WORKER_FLAG}"
            ),
        }
    }
}

impl std::error::Error for ArgsError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(args: &[&str]) -> Result<Command, ArgsError> {
        parse(&args.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>())
    }

    fn options(args: &[&str]) -> Options {
        match run(args).unwrap() {
            Command::Run(o) => o,
            other => panic!("not a run: {other:?}"),
        }
    }

    #[test]
    fn a_preset_alone_is_a_dry_run_with_the_defaults() {
        let o = options(&["ntsc"]);
        assert_eq!(o.preset, "ntsc");
        assert_eq!(o.modes, TestMode::ALL);
        assert_eq!(o.width, 640);
        assert_eq!(o.seconds, 10);
        assert_eq!(o.allow, Allowances::default());
        assert_eq!(o.device, PathBuf::from("/dev/dri/card0"));
        assert_eq!(o.connector, None);
        assert_eq!(o.report, PathBuf::from("kms-spike-report.json"));
        assert!(!o.confirmed);
        assert!(!o.worker);
    }

    #[test]
    fn every_option_is_read() {
        let o = options(&[
            "--modes",
            "480i,240p",
            "pal",
            "--width",
            "320",
            "--seconds",
            "3",
            "--allow-low-pclock",
            "6500000",
            "--allow-interlace",
            "--device",
            "/dev/dri/card1",
            "--connector",
            "VGA-1",
            "--report",
            "/tmp/r.json",
            "--i-have-checked-the-monitor",
            "--worker",
        ]);
        assert_eq!(o.preset, "pal");
        assert_eq!(o.modes, [TestMode::I480, TestMode::P240]);
        assert_eq!(o.width, 320);
        assert_eq!(o.seconds, 3);
        assert_eq!(o.allow.pclock_min_hz, Some(6_500_000));
        assert!(o.allow.interlace);
        assert_eq!(o.device, PathBuf::from("/dev/dri/card1"));
        assert_eq!(o.connector.as_deref(), Some("VGA-1"));
        assert_eq!(o.report, PathBuf::from("/tmp/r.json"));
        assert!(o.confirmed);
        assert!(o.worker);
    }

    #[test]
    fn the_worker_flag_needs_the_confirmation() {
        assert_eq!(
            run(&["pc_31_120", "--worker"]),
            Err(ArgsError::WorkerWithoutConfirmation)
        );
        assert!(run(&["pc_31_120", "--worker", "--i-have-checked-the-monitor"]).is_ok());
    }

    #[test]
    fn there_is_no_default_preset() {
        assert_eq!(run(&[]), Err(ArgsError::NoPreset));
        assert_eq!(
            run(&["--i-have-checked-the-monitor"]),
            Err(ArgsError::NoPreset)
        );
    }

    #[test]
    fn listing_commands_need_no_preset() {
        assert_eq!(run(&["--help"]), Ok(Command::Help));
        assert_eq!(run(&["--list-presets"]), Ok(Command::ListPresets));
        assert_eq!(
            run(&["--list-connectors", "--device", "/dev/dri/card1"]),
            Ok(Command::ListConnectors {
                device: PathBuf::from("/dev/dri/card1")
            })
        );
    }

    #[test]
    fn values_out_of_range_are_refused() {
        for args in [
            ["ntsc", "--seconds", "0"],
            ["ntsc", "--seconds", "11"],
            ["ntsc", "--width", "63"],
            ["ntsc", "--width", "3841"],
            ["ntsc", "--allow-low-pclock", "999999"],
            ["ntsc", "--allow-low-pclock", "25000000"],
            ["ntsc", "--modes", "240p,240p"],
            ["ntsc", "--modes", "576i"],
            ["ntsc", "--modes", ""],
            ["ntsc", "--width", "-5"],
        ] {
            assert!(
                matches!(run(&args), Err(ArgsError::BadValue { .. })),
                "{args:?}"
            );
        }
    }

    #[test]
    fn malformed_lines_are_refused() {
        assert_eq!(
            run(&["ntsc", "--width"]),
            Err(ArgsError::MissingValue("--width"))
        );
        assert_eq!(
            run(&["ntsc", "--fast"]),
            Err(ArgsError::Unknown("--fast".to_owned()))
        );
        assert_eq!(
            run(&["ntsc", "pal"]),
            Err(ArgsError::ExtraArgument("pal".to_owned()))
        );
        assert_eq!(
            run(&["ntsc", "--allow-interlace", "--allow-interlace"]),
            Err(ArgsError::Repeated("--allow-interlace"))
        );
        assert_eq!(
            run(&["ntsc", "--width", "320", "--width", "640"]),
            Err(ArgsError::Repeated("--width"))
        );
    }

    #[test]
    fn errors_say_what_is_wrong() {
        let e = run(&["ntsc", "--seconds", "60"]).unwrap_err().to_string();
        assert!(e.contains("--seconds") && e.contains("1 to 10"), "{e}");
        assert!(ArgsError::NoPreset.to_string().contains("no default"));
    }
}
