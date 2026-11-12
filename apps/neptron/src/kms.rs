//! The device backend: DRM/KMS on the launcher's descriptor, SDL3
//! gamepads, evdev keyboards and the SDL3 audio sink.

use std::process::ExitCode;
use std::time::{Duration, Instant};

use neptron::devices::MaybeSink;
use neptron::display::{AUTO, Detected};
use neptron::options::Options;
use neptron::{Devices, NextRun, Runtime, Shell};
use neptron_audio::{Resampling, Sdl3Sink};
use neptron_backend::{Clock, InputSource};
use neptron_kms::{KmsConfig, KmsDisplay};
use neptron_platform::UNKNOWN_GPU_CAPS;
use neptron_session::RunRateSource;
use neptron_settings::{Schema, Storage, Target};
use neptron_types::InputEvent;

/// The widest scanout buffer, in pixels: the super width of the policy.
const BUFFER_WIDTH_MAX_PX: u16 = 2_560;
/// The tallest scanout buffer, in lines: 576i and the 31 kHz modes of a
/// PC CRT.
const BUFFER_HEIGHT_MAX_PX: u16 = 1_200;

/// The monotonic clock of the device.
#[derive(Debug, Clone, Copy)]
struct Monotonic {
    start: Instant,
}

impl Clock for Monotonic {
    fn now_ns(&self) -> u64 {
        u64::try_from(self.start.elapsed().as_nanos()).unwrap_or(u64::MAX)
    }
}

/// The gamepads and the keyboards as one source.
struct Inputs {
    pads: Option<neptron_input::SdlGamepads>,
    keyboards: neptron_input::EvdevKeyboards,
}

impl InputSource for Inputs {
    fn poll(&mut self, out: &mut Vec<InputEvent>) {
        if let Some(pads) = &mut self.pads {
            pads.poll(out);
        }
        self.keyboards.poll(out);
    }
}

/// Runs the frontend on the DRM device that the launcher holds.
pub fn run(options: &Options, next: Option<NextRun>) -> ExitCode {
    let paths = &options.paths;
    let output = Storage::new(paths.config())
        .load(&Schema::builtin(), Target::Global)
        .ok()
        .and_then(|l| {
            l.parsed
                .layer
                .get(neptron_settings::DISPLAY_OUTPUT)
                .map(ToString::to_string)
        })
        .filter(|o| o != AUTO);
    let detected = Detected::read(&paths.sysfs, output.as_deref().unwrap_or(AUTO));
    let caps = detected.caps.unwrap_or(UNKNOWN_GPU_CAPS);
    let config = KmsConfig {
        caps,
        output,
        crt: true,
        buffer_width: caps.max_width.min(BUFFER_WIDTH_MAX_PX),
        buffer_height: caps.max_height.min(BUFFER_HEIGHT_MAX_PX),
        writeback: false,
    };
    let display = match KmsDisplay::from_env(config) {
        Ok(display) => display,
        Err(e) => {
            eprintln!("neptron: cannot open the display: {e}");
            return ExitCode::FAILURE;
        }
    };
    let output = display.connector_name().to_owned();
    // A missing sound device or gamepad subsystem is a notice: the run
    // still shows the picture, reads the keyboards and reaches its guard,
    // so a box with no usable audio does not restart forever.
    let mut notices = Vec::new();
    let audio = match Sdl3Sink::open() {
        Ok(sink) => MaybeSink::Device(sink),
        Err(e) => {
            eprintln!("neptron: cannot open the audio, the run has no sound: {e}");
            notices.push(format!("No sound: the audio device did not open ({e})."));
            MaybeSink::Silent
        }
    };
    let pads = match neptron_input::SdlGamepads::open() {
        Ok(pads) => Some(pads),
        Err(e) => {
            eprintln!("neptron: cannot open the gamepads, the run reads keyboards only: {e}");
            notices.push(format!("No gamepads: they did not open ({e})."));
            None
        }
    };
    let clock = Monotonic {
        start: Instant::now(),
    };
    let runtime = Runtime {
        run_rate: RunRateSource::ReadBack,
        resampling: Resampling::Sinc,
        pace: Box::new(move |until_ns| {
            let now_ns = clock.now_ns();
            if until_ns > now_ns {
                std::thread::sleep(Duration::from_nanos(until_ns - now_ns));
            }
        }),
        quit: None,
        output,
        notices,
    };
    let devices = Devices {
        display,
        audio,
        input: Inputs {
            pads,
            keyboards: neptron_input::EvdevKeyboards::new(),
        },
        clock,
    };
    let booted = Shell::boot(paths.clone(), devices, runtime, next);
    crate::run_booted(options, booted)
}
