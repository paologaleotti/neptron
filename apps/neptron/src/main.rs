//! The frontend binary. It picks the backend, starts a run, and ends it
//! with a relaunch or a quit. The wiring is in the library of this crate.

#![forbid(unsafe_code)]

#[cfg(all(feature = "kms", target_os = "linux"))]
mod kms;
#[cfg(feature = "window")]
mod window;

use std::process::ExitCode;

use neptron::options::{self, Backend, Options};
use neptron::{Booted, NextRun, Outcome, Paths, RELAUNCH_EXIT_CODE, relaunch};

fn main() -> ExitCode {
    let options = match options::parse(
        std::env::args_os().skip(1),
        std::env::var_os(neptron_ipc::SOCKET_PATH_ENV),
    ) {
        Ok(options) => options,
        Err(e) => {
            say!("neptron: {e}");
            return ExitCode::from(2);
        }
    };
    // Before any SDL code starts: SDL leaves a signal alone when it has a
    // handler already.
    let stop = match neptron::stop::stop_flag() {
        Ok(stop) => stop,
        Err(e) => {
            say!("neptron: cannot watch SIGTERM and SIGINT: {e}");
            return ExitCode::FAILURE;
        }
    };
    // The log of the run leaves the frame thread through a bounded queue;
    // a writer thread writes it to standard error.
    let (log, log_writer) = neptron::logbook::to_stderr();
    // On a bench the binary can come from another folder than the image's
    // own; the log says which one runs, with which data.
    let program = std::env::current_exe()
        .map_or_else(|e| format!("unknown ({e})"), |p| p.display().to_string());
    log.send(format!(
        "neptron: {program} starts; data {}, cores {}, state {}, share {}",
        options.paths.data.display(),
        options.paths.cores.display(),
        options.paths.state.display(),
        options.paths.share.display()
    ));
    let code = match options.backend.or(DEFAULT_BACKEND) {
        #[cfg(all(feature = "kms", target_os = "linux"))]
        Some(Backend::Kms) => kms::run(&options, take_next(&options.paths), stop, log),
        #[cfg(feature = "window")]
        Some(Backend::Window) => window::run(&options, take_next(&options.paths), stop, log),
        other => {
            drop((stop, log));
            say!(
                "neptron: this build has no {} backend; build it with the feature `window` or `kms`",
                match other {
                    Some(Backend::Kms) => "kms",
                    Some(Backend::Window) => "window",
                    None => "display",
                }
            );
            ExitCode::FAILURE
        }
    };
    // Every sending side is gone with the run; the writer ends after the
    // queued lines.
    log_writer.finish();
    code
}

/// Writes a line to standard error. A failed write loses the line: the
/// frontend must not panic or stop over its log, and `eprintln!` panics on
/// a write error.
macro_rules! say {
    ($($arg:tt)*) => {{
        use std::io::Write as _;
        // A line that cannot be written is lost; see above.
        let _ = writeln!(std::io::stderr(), $($arg)*);
    }};
}
use say;

/// The backend of a run with no `--backend`: KMS on the device, the window
/// on a desktop build.
const DEFAULT_BACKEND: Option<Backend> = if cfg!(feature = "kms") {
    Some(Backend::Kms)
} else if cfg!(feature = "window") {
    Some(Backend::Window)
} else {
    None
};

/// Takes the relaunch file of the run before. A file that cannot be taken
/// makes this start a new boot.
#[cfg_attr(not(any(feature = "kms", feature = "window")), allow(dead_code))]
fn take_next(paths: &Paths) -> Option<NextRun> {
    relaunch::take(&paths.relaunch())
        .inspect_err(|e| say!("neptron: {e}; this start is a new boot"))
        .ok()
        .flatten()
}

/// Ends a run: what the boot gave, or the outcome of the frame loop, with
/// the messages of the last writes.
#[cfg_attr(not(any(feature = "kms", feature = "window")), allow(dead_code))]
fn end(paths: &Paths, outcome: Outcome, messages: Vec<String>) -> ExitCode {
    for message in messages {
        say!("neptron: {message}");
    }
    if let Some(code) = neptron::power::exit_code(&outcome) {
        return ExitCode::from(code);
    }
    match outcome {
        Outcome::Relaunch(next) => relaunch(paths, &next),
        Outcome::Quit | Outcome::Reboot | Outcome::PowerOff => ExitCode::SUCCESS,
    }
}

/// Writes the relaunch file and asks the launcher for the next run.
#[cfg_attr(not(any(feature = "kms", feature = "window")), allow(dead_code))]
fn relaunch(paths: &Paths, next: &NextRun) -> ExitCode {
    match relaunch::write(&paths.relaunch(), next) {
        Ok(()) => ExitCode::from(RELAUNCH_EXIT_CODE),
        Err(e) => {
            say!("neptron: cannot write the relaunch file: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Runs a booted frontend with the control socket of `options`.
#[cfg_attr(not(any(feature = "kms", feature = "window")), allow(dead_code))]
fn run_booted<D, S, I, C>(
    options: &Options,
    booted: Result<Booted<D, S, I, C>, neptron::BootError>,
) -> ExitCode
where
    D: neptron_backend::Display,
    S: neptron_backend::AudioSink,
    I: neptron_backend::InputSource,
    C: neptron_backend::Clock + Clone,
{
    let mut shell = match booted {
        Ok(Booted::Ready(shell)) => shell,
        Ok(Booted::Relaunch(next)) => return relaunch(&options.paths, &next),
        Err(e) => {
            say!("neptron: the frontend cannot start: {e}");
            return ExitCode::FAILURE;
        }
    };
    let control = options.control.as_deref().and_then(|path| {
        neptron::control::ControlServer::start(path)
            .inspect_err(|e| say!("neptron: {e}; the run has no control socket"))
            .ok()
    });
    let (server, mut port) = match control {
        Some((server, port)) => (Some(server), Some(port)),
        None => (None, None),
    };
    let outcome = neptron::run(&mut shell, port.as_mut());
    if let (Some(server), Some(port)) = (server, port) {
        server.stop(port);
    }
    let messages = shell.finish();
    end(&options.paths, outcome, messages)
}
