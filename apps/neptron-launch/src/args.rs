//! The command line of the launcher.

use std::ffi::OsString;
use std::path::PathBuf;

use crate::device::DEFAULT_DEVICE;

/// The usage text.
pub const USAGE: &str =
    "usage: neptron-launch [--device PATH | --no-device] [--power] -- PROGRAM [ARGS...]";

/// What the command line asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    /// The DRM device to hold, `None` with `--no-device` (the desktop
    /// window, where no frontend takes a descriptor).
    pub device: Option<PathBuf>,
    /// True with `--power`: the launcher carries out a reboot or a
    /// power-off that the frontend asks for. Only with a device, so a
    /// desktop run never turns off the computer it runs on.
    pub power: bool,
    /// The frontend program.
    pub program: OsString,
    /// Its arguments.
    pub args: Vec<OsString>,
}

/// The command line does not follow the usage.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{reason}\n{USAGE}")]
pub struct UsageError {
    /// What is wrong.
    pub reason: String,
}

/// Reads the arguments after the program name.
///
/// # Errors
///
/// [`UsageError`].
pub fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Options, UsageError> {
    let mut device = Some(PathBuf::from(DEFAULT_DEVICE));
    let mut power = false;
    let mut args = args.into_iter();
    loop {
        let Some(arg) = args.next() else {
            return Err(usage("no frontend program after --"));
        };
        match arg.to_str() {
            Some("--") => break,
            Some("--no-device") => device = None,
            Some("--power") => power = true,
            Some("--device") => {
                let path = args.next().ok_or_else(|| usage("--device needs a path"))?;
                device = Some(PathBuf::from(path));
            }
            _ => return Err(usage(&format!("unknown argument {arg:?}"))),
        }
    }
    if power && device.is_none() {
        return Err(usage(
            "--power needs a DRM device: a run with --no-device is a desktop run",
        ));
    }
    let program = args
        .next()
        .ok_or_else(|| usage("no frontend program after --"))?;
    Ok(Options {
        device,
        power,
        program,
        args: args.collect(),
    })
}

fn usage(reason: &str) -> UsageError {
    UsageError {
        reason: reason.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_strs(args: &[&str]) -> Result<Options, UsageError> {
        parse(args.iter().map(OsString::from))
    }

    #[test]
    fn the_default_device_is_the_first_card() {
        let options = parse_strs(&["--", "/usr/bin/neptron", "--backend", "kms"]).unwrap();
        assert_eq!(options.device, Some(PathBuf::from("/dev/dri/card0")));
        assert_eq!(options.program, "/usr/bin/neptron");
        assert_eq!(options.args, [OsString::from("--backend"), "kms".into()]);
        assert!(!options.power, "no power control by default");
    }

    #[test]
    fn power_control_needs_a_device() {
        let on = parse_strs(&["--power", "--", "/usr/bin/neptron"]).unwrap();
        assert!(on.power);
        assert_eq!(on.device, Some(PathBuf::from("/dev/dri/card0")));
        for args in [
            &["--power", "--no-device", "--", "neptron"][..],
            &["--no-device", "--power", "--", "neptron"],
        ] {
            let e = parse_strs(args).unwrap_err();
            assert!(e.reason.contains("--power needs a DRM device"), "{e}");
        }
    }

    #[test]
    fn the_device_can_be_named_or_left_out() {
        let named = parse_strs(&["--device", "/dev/dri/card1", "--", "neptron"]).unwrap();
        assert_eq!(named.device, Some(PathBuf::from("/dev/dri/card1")));
        let none = parse_strs(&["--no-device", "--", "neptron"]).unwrap();
        assert_eq!(none.device, None);
    }

    #[test]
    fn a_wrong_command_line_is_a_usage_error() {
        for args in [
            &[][..],
            &["--"],
            &["neptron"],
            &["--device"],
            &["--bogus", "--", "neptron"],
        ] {
            assert!(parse_strs(args).is_err(), "{args:?}");
        }
    }
}
