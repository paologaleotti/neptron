//! The command line of the frontend binary.

use std::ffi::OsString;
use std::path::PathBuf;

use crate::paths::Paths;

/// The usage text.
pub const USAGE: &str =
    "usage: neptron [--backend kms|window] [--share DIR] [--state DIR] [--data DIR]
               [--cores DIR] [--run DIR] [--sysfs DIR] [--control-socket PATH | --no-control]";

/// The display backend of a run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    /// DRM/KMS on the descriptor of the launcher (feature `kms`).
    Kms,
    /// The desktop window (feature `window`).
    Window,
}

/// What the command line asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    /// The backend; `None` for the one the build has.
    pub backend: Option<Backend>,
    /// The folders.
    pub paths: Paths,
    /// The control socket; `None` with `--no-control`.
    pub control: Option<PathBuf>,
}

/// The command line does not follow the usage.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{reason}\n{USAGE}")]
pub struct UsageError {
    /// What is wrong.
    pub reason: String,
}

/// Reads the arguments after the program name. `socket_env` is the value
/// of `NEPTRON_CONTROL_SOCKET`, the socket when the command line names
/// none.
///
/// # Errors
///
/// [`UsageError`].
pub fn parse(
    args: impl IntoIterator<Item = OsString>,
    socket_env: Option<OsString>,
) -> Result<Options, UsageError> {
    let mut options = Options {
        backend: None,
        paths: Paths::image(),
        control: Some(
            socket_env
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(neptron_ipc::DEFAULT_SOCKET_PATH)),
        ),
    };
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        let name = arg
            .to_str()
            .ok_or_else(|| usage(format!("unknown argument {arg:?}")))?
            .to_owned();
        if name == "--no-control" {
            options.control = None;
            continue;
        }
        let value = args
            .next()
            .ok_or_else(|| usage(format!("{name} needs a value")))?;
        let path = PathBuf::from(&value);
        let paths = &mut options.paths;
        match name.as_str() {
            "--backend" => {
                options.backend = Some(match value.to_str() {
                    Some("kms") => Backend::Kms,
                    Some("window") => Backend::Window,
                    _ => return Err(usage(format!("unknown backend {value:?}"))),
                });
            }
            "--share" => paths.share = path,
            "--state" => paths.state = path,
            "--data" => paths.data = path,
            "--cores" => paths.cores = path,
            "--run" => paths.run = path,
            "--sysfs" => paths.sysfs = path,
            "--control-socket" => options.control = Some(path),
            _ => return Err(usage(format!("unknown argument {name}"))),
        }
    }
    Ok(options)
}

fn usage(reason: String) -> UsageError {
    UsageError { reason }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_strs(args: &[&str]) -> Result<Options, UsageError> {
        parse(args.iter().map(OsString::from), None)
    }

    #[test]
    fn with_no_argument_the_image_folders_apply() {
        let options = parse_strs(&[]).expect("parse");
        assert_eq!(options.paths, Paths::image());
        assert_eq!(options.backend, None);
        assert_eq!(
            options.control,
            Some(PathBuf::from(neptron_ipc::DEFAULT_SOCKET_PATH))
        );
    }

    #[test]
    fn every_folder_and_the_backend_can_be_named() {
        let options = parse_strs(&[
            "--backend",
            "window",
            "--share",
            "s",
            "--state",
            "t",
            "--data",
            "d",
            "--cores",
            "c",
            "--run",
            "r",
            "--sysfs",
            "y",
            "--no-control",
        ])
        .expect("parse");
        assert_eq!(options.backend, Some(Backend::Window));
        assert_eq!(options.paths.share, PathBuf::from("s"));
        assert_eq!(options.paths.sysfs, PathBuf::from("y"));
        assert_eq!(options.control, None);
    }

    #[test]
    fn the_socket_comes_from_the_environment_or_the_command_line() {
        let env = parse(Vec::<OsString>::new(), Some("/tmp/x.sock".into())).expect("parse");
        assert_eq!(env.control, Some(PathBuf::from("/tmp/x.sock")));
        let named = parse_strs(&["--control-socket", "/tmp/y.sock"]).expect("parse");
        assert_eq!(named.control, Some(PathBuf::from("/tmp/y.sock")));
    }

    #[test]
    fn a_wrong_command_line_is_a_usage_error() {
        for args in [
            &["--backend"][..],
            &["--backend", "fake"],
            &["--bogus", "x"],
        ] {
            assert!(parse_strs(args).is_err(), "{args:?}");
        }
    }
}
