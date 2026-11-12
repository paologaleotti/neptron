//! The device backend: DRM/KMS on the launcher's descriptor, SDL3
//! gamepads, evdev keyboards and the SDL3 audio sink.

use std::process::ExitCode;
use std::time::{Duration, Instant};

use neptron::devices::MaybeSink;
use neptron::display::{AUTO, Detected};
use neptron::options::Options;
use neptron::presets::Presets;
use neptron::{Devices, NextRun, Runtime, Shell};
use neptron_audio::{Resampling, Sdl3Sink};
use neptron_backend::{Clock, Display, InputSource};
use neptron_kms::{KmsConfig, KmsDisplay};
use neptron_session::RunRateSource;
use neptron_settings::{Schema, Storage, Target};
use neptron_types::InputEvent;

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
    let global = Storage::new(paths.config())
        .load(&Schema::builtin(), Target::Global)
        .ok();
    let setting = |key: &str| {
        global
            .as_ref()
            .and_then(|l| l.parsed.layer.get(key).map(ToString::to_string))
            .unwrap_or_else(|| AUTO.to_owned())
    };
    let output = Some(setting(neptron_settings::DISPLAY_OUTPUT)).filter(|o| o != AUTO);
    let monitor = setting(neptron_settings::DISPLAY_MONITOR);
    // Before the display opens, the subconnector is not known: the class
    // here only orders the connectors, and the boot reads the class again
    // for the connector in use.
    let detected = Detected::read(&paths.sysfs, output.as_deref().unwrap_or(AUTO), None);
    let presets = Presets::load(&paths.monitors())
        .map(|(p, _)| p)
        .unwrap_or_default();
    // The buffers are made once, at the size of the largest mode the caps
    // allow, so the boot cuts the panel's modes to them.
    let caps = detected.backend_caps(&monitor);
    let config = KmsConfig {
        caps,
        output,
        crt: detected.is_crt(&monitor, &presets),
        buffer_width: caps.max_width,
        buffer_height: caps.max_height,
        writeback: false,
    };
    let mut display = match KmsDisplay::from_env(config) {
        Ok(display) => display,
        Err(e) => {
            eprintln!("neptron: cannot open the display: {e}");
            return ExitCode::FAILURE;
        }
    };
    let output = display.connector_name().to_owned();
    let subconnector = display.subconnector().map(str::to_owned);
    // The kernel reports the subconnector only now: a DAC behind the DP
    // connector, or a digital adapter with no EDID, makes the output
    // analog, and the CRT path must not run with the digital limits.
    let driven = Detected::read(&paths.sysfs, &output, subconnector.as_deref());
    let policy = driven.policy_caps(display.caps());
    display.lower_caps(&policy);
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
        subconnector,
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
