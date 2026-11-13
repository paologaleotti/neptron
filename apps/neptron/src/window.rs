//! The desktop window backend: the SDL3 window, the keyboard as a pad and
//! the SDL3 audio sink, in real time.

use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use neptron::devices::MaybeSink;
use neptron::options::Options;
use neptron::{Devices, NextRun, Runtime, Shell};
use neptron_audio::{Resampling, Sdl3Sink};
use neptron_backend::{Clock, InputSource};
use neptron_session::RunRateSource;
use neptron_types::InputEvent;
use neptron_window::KeyboardPad;

/// The keyboard of the window, which also reports a closed window.
struct WindowInput {
    pad: KeyboardPad,
    quit: Arc<AtomicBool>,
}

impl InputSource for WindowInput {
    fn poll(&mut self, out: &mut Vec<InputEvent>) {
        self.pad.poll(out);
        if self.pad.quit_requested() {
            self.quit.store(true, Ordering::Relaxed);
        }
    }
}

/// Runs the frontend in a desktop window.
pub fn run(options: &Options, next: Option<NextRun>, quit: Arc<AtomicBool>) -> ExitCode {
    let (display, pad) = match neptron_window::open("NEPTRON") {
        Ok(window) => window,
        Err(e) => {
            eprintln!("neptron: cannot open the window: {e}");
            return ExitCode::FAILURE;
        }
    };
    let mut notices = Vec::new();
    let audio = match Sdl3Sink::open() {
        Ok(sink) => MaybeSink::Device(sink),
        Err(e) => {
            eprintln!("neptron: cannot open the audio, the run has no sound: {e}");
            notices.push(format!("No sound: the audio device did not open ({e})."));
            MaybeSink::Silent
        }
    };
    let clock = display.clock();
    let runtime = Runtime {
        run_rate: RunRateSource::Measured,
        resampling: Resampling::Sinc,
        pace: Box::new(move |until_ns| {
            let now_ns = clock.now_ns();
            if until_ns > now_ns {
                std::thread::sleep(Duration::from_nanos(until_ns - now_ns));
            }
        }),
        quit: Some(quit.clone()),
        output: "window".to_owned(),
        subconnector: None,
        notices,
    };
    let devices = Devices {
        display,
        audio,
        input: WindowInput { pad, quit },
        clock,
    };
    let booted = Shell::boot(options.paths.clone(), devices, runtime, next);
    crate::run_booted(options, booted)
}
