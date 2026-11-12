//! The command line of the launcher.

use std::ffi::OsString;
use std::path::PathBuf;

use crate::device::DEFAULT_DEVICE;

/// The usage text.
pub const USAGE: &str = "usage: neptron-launch [--device PATH | --no-device] -- PROGRAM [ARGS...]";

/// What the command line asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    /// The DRM device to hold, `None` with `--no-device` (the desktop
    /// window, where no frontend takes a descriptor).
    pub device: Option<PathBuf>,
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
    let mut args = args.into_iter();
    loop {
        let Some(arg) = args.next() else {
            return Err(usage("no frontend program after --"));
        };
        match arg.to_str() {
            Some("--") => break,
            Some("--no-device") => device = None,
            Some("--device") => {
                let path = args.next().ok_or_else(|| usage("--device needs a path"))?;
                device = Some(PathBuf::from(path));
            }
            _ => return Err(usage(&format!("unknown argument {arg:?}"))),
        }
    }
    let program = args
        .next()
        .ok_or_else(|| usage("no frontend program after --"))?;
    Ok(Options {
        device,
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
