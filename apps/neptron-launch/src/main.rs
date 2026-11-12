//! The launcher. It holds the DRM descriptor, starts the frontend, passes
//! the descriptor in `NEPTRON_DRM_FD` and respawns the frontend with a
//! backoff.

#![forbid(unsafe_code)]

mod args;
mod backoff;
mod device;
mod supervise;

use std::ffi::OsString;
use std::io;
use std::process::{Command, ExitCode};
use std::time::{Duration, Instant};

use backoff::Backoff;
use device::{DRM_FD_ENV, Device};
use supervise::{Frontend, Time, supervise};

fn main() -> ExitCode {
    let options = match args::parse(std::env::args_os().skip(1)) {
        Ok(options) => options,
        Err(e) => {
            eprintln!("neptron-launch: {e}");
            return ExitCode::from(2);
        }
    };
    let device = match options.device.as_deref().map(Device::open).transpose() {
        Ok(device) => device,
        Err(e) => {
            eprintln!("neptron-launch: {e}");
            return ExitCode::FAILURE;
        }
    };
    let mut frontend = Process {
        program: options.program,
        args: options.args,
        fd: device.as_ref().map(Device::fd),
    };
    let mut time = MonotonicTime {
        start: Instant::now(),
    };
    match supervise(&mut frontend, &mut time, &mut Backoff::new()) {
        Ok(runs) => {
            eprintln!("neptron-launch: the frontend quit after {runs} runs");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("neptron-launch: {e}");
            ExitCode::FAILURE
        }
    }
}

/// The frontend as a child process with the descriptor number in its
/// environment.
struct Process {
    program: OsString,
    args: Vec<OsString>,
    fd: Option<i32>,
}

impl Frontend for Process {
    fn run(&mut self) -> io::Result<Option<i32>> {
        let mut command = Command::new(&self.program);
        command.args(&self.args);
        if let Some(fd) = self.fd {
            command.env(DRM_FD_ENV, fd.to_string());
        }
        Ok(command.status()?.code())
    }
}

struct MonotonicTime {
    start: Instant,
}

impl Time for MonotonicTime {
    fn now_ns(&self) -> u64 {
        u64::try_from(self.start.elapsed().as_nanos()).unwrap_or(u64::MAX)
    }

    fn wait_ns(&mut self, ns: u64) {
        std::thread::sleep(Duration::from_nanos(ns));
    }
}

#[cfg(test)]
mod tests {
    use std::os::fd::AsRawFd;

    use super::*;
    use crate::backoff::RELAUNCH_EXIT_CODE;

    fn shell(script: &str, fd: Option<i32>) -> Process {
        Process {
            program: "/bin/sh".into(),
            args: vec!["-c".into(), script.into()],
            fd,
        }
    }

    #[test]
    fn a_child_reports_its_exit_code() {
        let mut child = shell(&format!("exit {RELAUNCH_EXIT_CODE}"), None);
        assert_eq!(child.run().expect("run"), Some(RELAUNCH_EXIT_CODE));
        assert_eq!(shell("exit 0", None).run().expect("run"), Some(0));
        assert_eq!(shell("kill -9 $$", None).run().expect("run"), None);
    }

    #[test]
    fn a_child_gets_the_descriptor_under_its_number() {
        let path =
            std::env::temp_dir().join(format!("neptron-launch-child-{}", std::process::id()));
        std::fs::write(&path, b"held").expect("write");
        let file = std::fs::File::open(&path).expect("open");
        device::inheritable(&file).expect("inheritable");
        let script = format!(
            "test \"${DRM_FD_ENV}\" = {fd} && test \"$(cat <&{fd})\" = held",
            fd = file.as_raw_fd()
        );
        let code = shell(&script, Some(file.as_raw_fd())).run().expect("run");
        assert_eq!(
            code,
            Some(0),
            "the child reads the descriptor of the launcher"
        );
        // The file is the test's own; a failed removal leaves a temp file.
        let _ = std::fs::remove_file(&path);
    }
}
