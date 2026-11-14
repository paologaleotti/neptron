//! The device backend: DRM/KMS on the launcher's descriptor, SDL3
//! gamepads, evdev keyboards and the SDL3 audio sink.

use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use neptron::devices::{audio_output_setting, open_audio};
use neptron::display::{AUTO, Detected};
use neptron::display_wait::{Look, NO_DISPLAY_EXIT_CODE, auto_output, wait_line, wait_with_looks};
use neptron::logbook::LogLines;
use neptron::options::Options;
use neptron::presets::Presets;
use neptron::{Devices, NextRun, Runtime, Shell};
use neptron_audio::{Resampling, Sdl3Outputs};
use neptron_backend::{Clock, Display, InputSource};
use neptron_kms::{ChoiceError, KmsConfig, KmsDisplay, KmsError};
use neptron_platform::Matrix;
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
pub fn run(
    options: &Options,
    next: Option<NextRun>,
    stop: Arc<AtomicBool>,
    log: LogLines,
) -> ExitCode {
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
    let mut output = Some(setting(neptron_settings::DISPLAY_OUTPUT)).filter(|o| o != AUTO);
    let monitor = setting(neptron_settings::DISPLAY_MONITOR);
    let clock = Monotonic {
        start: Instant::now(),
    };
    // A named output is used whatever its status, so only `auto` waits.
    if output.is_none() {
        // The table is built in, so a failure is a build defect: logged
        // once, and the wait then has no composite to fall back on.
        let matrix = Matrix::builtin()
            .inspect_err(|e| crate::say!("neptron: {e}; no composite fallback"))
            .ok();
        wait_for_a_display(&paths.sysfs, matrix.as_ref(), &clock, &stop);
        if stop.load(Ordering::Relaxed) {
            return ExitCode::SUCCESS;
        }
        // A composite connector never reports a display, so the
        // connector choice never takes it: with no display that answers,
        // the composite connector of a TV encoder is opened by name.
        output = neptron_platform::detect(&paths.sysfs)
            .ok()
            .and_then(|p| auto_output(&p, matrix.as_ref()));
        if let Some(name) = &output {
            crate::say!("neptron: no display answers; the output is {name}");
        }
    }
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
            crate::say!("neptron: cannot open the display: {e}");
            return match e {
                KmsError::Choice(ChoiceError::NoneConnected { .. }) => {
                    ExitCode::from(NO_DISPLAY_EXIT_CODE)
                }
                _ => ExitCode::FAILURE,
            };
        }
    };
    let output = display.connector_name().to_owned();
    let subconnector = display.subconnector().map(str::to_owned);
    let tv_standards = display.tv_standards();
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
    // The device opens here, once per process, before the first frame:
    // every launch and every return to the game list is a new process, so
    // a new output takes effect at the next run.
    let opened = open_audio(
        Sdl3Outputs::start().map_err(|e| e.to_string()),
        &audio_output_setting(paths),
    );
    crate::say!("neptron: {}", opened.log);
    for notice in &opened.notices {
        crate::say!("neptron: {notice}");
    }
    notices.extend(opened.notices);
    let audio = opened.sink;
    let pads = match neptron_input::SdlGamepads::open() {
        Ok(pads) => Some(pads),
        Err(e) => {
            crate::say!("neptron: cannot open the gamepads, the run reads keyboards only: {e}");
            notices.push(format!("No gamepads: they did not open ({e})."));
            None
        }
    };
    let failed = std::env::var_os(neptron::power::POWER_FAILED_ENV);
    if let Some(text) = neptron::power::failed_notice(failed.as_deref()) {
        crate::say!("neptron: {text}");
        notices.push(text.to_owned());
    }
    let runtime = Runtime {
        run_rate: RunRateSource::ReadBack,
        resampling: Resampling::Sinc,
        pace: Box::new(move |until_ns| {
            let now_ns = clock.now_ns();
            if until_ns > now_ns {
                std::thread::sleep(Duration::from_nanos(until_ns - now_ns));
            }
        }),
        quit: Some(stop),
        log: Some(log),
        output,
        subconnector,
        tv_standards,
        notices,
        audio_outputs: opened.outputs,
        power: neptron::power::control_from(std::env::var_os(neptron::power::POWER_ENV).as_deref()),
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

/// Waits a bounded time for a connector that qualifies, and logs how the
/// wait ended. A stop request ends the wait.
fn wait_for_a_display(
    sysfs: &std::path::Path,
    matrix: Option<&Matrix>,
    clock: &Monotonic,
    stop: &AtomicBool,
) {
    let pause = |until_ns: u64| {
        let now_ns = clock.now_ns();
        if until_ns > now_ns {
            std::thread::sleep(Duration::from_nanos(until_ns - now_ns));
        }
    };
    let waited = wait_with_looks(
        clock,
        pause,
        || stop.load(Ordering::Relaxed),
        || Look::at(&neptron_platform::detect(sysfs).unwrap_or_default(), matrix),
    );
    if let Some(line) = wait_line(waited) {
        crate::say!("neptron: {line}");
    }
}
