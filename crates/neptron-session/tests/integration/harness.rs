//! A session on the fakes: the built test core through the real host, the
//! fake display, the fake audio, scripted input and the simulated clock.
//!
//! The test core reads its script from an environment variable, and one
//! process holds one core. Setting a variable in a running process is
//! `unsafe`, so each test runs its body in a child process of the test
//! binary, made with the variable set: [`in_child`].

use std::path::{Path, PathBuf};
use std::process::Command;

use neptron_audio::Resampling;
use neptron_backend::{AudioSink, Clock};
use neptron_session::{Devices, GameLaunch, RunRateSource, Session, SessionOptions, VideoSetup};
use neptron_testcore::{PixelFormat as CoreFormat, SCRIPT_ENV, Size, expected_hash, hash};
use neptron_testkit::preset::{CAPS_LOW_CLOCK, monitor_preset};
use neptron_testkit::{
    AudioOptions, DisplayOptions, FakeAudio, FakeDisplay, ScriptedInput, SimClock,
};
use neptron_types::{Control, Frame, GpuCaps, PixelFormat};
use neptron_video::{Monitor, PolicySettings, ScanlineSettings};

/// The session type of the tests.
pub type TestSession = Session<FakeDisplay, FakeAudio, ScriptedInput, SimClock>;

/// The device rate of the fake audio.
pub const DEVICE_RATE_HZ: u32 = 48_000;
/// The queue of the fake audio: 128 ms, as the SDL3 sink keeps.
pub const SINK_CAPACITY_FRAMES: usize = 6_144;
/// The time the simulated clock starts at.
pub const START_NS: u64 = 1_000_000;

/// Runs `body` in a child process that has the test core's script
/// variable set to a file holding `script`. In the child, `body` gets the
/// folder of the script. `test` is the test's path in the binary.
pub fn in_child(test: &str, script: &str, body: impl FnOnce(&Path)) {
    if let Some(path) = std::env::var_os(SCRIPT_ENV) {
        let dir = PathBuf::from(path)
            .parent()
            .expect("the script folder")
            .to_owned();
        body(&dir);
        std::fs::write(dir.join(DONE_MARKER), test).expect("write the marker");
        return;
    }
    let dir = temp_dir(test);
    let path = dir.join("game.ntc");
    std::fs::write(&path, script).expect("write the script");
    let status = Command::new(std::env::current_exe().expect("the test binary"))
        .args([
            test,
            "--exact",
            "--include-ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(SCRIPT_ENV, &path)
        .status()
        .expect("start the child");
    assert!(status.success(), "the child process of {test} failed");
    // A name that matches no test makes the child pass with no test run.
    assert!(
        dir.join(DONE_MARKER).exists(),
        "the child process ran no test named {test}"
    );
}

/// The file a child writes when its body has returned.
const DONE_MARKER: &str = "done";

/// The test's path in the binary, from `module_path!()` and the function
/// name: the binary's own name is not part of it.
pub fn test_name(module: &str, function: &str) -> String {
    let module = module.split_once("::").map_or("", |(_, rest)| rest);
    format!("{module}::{function}")
}

/// Runs the body in a child process with the script set, as
/// [`in_child`] does, for the test function it is called in.
#[macro_export]
macro_rules! with_script {
    ($function:expr, $script:expr, $body:expr) => {
        $crate::harness::in_child(
            &$crate::harness::test_name(module_path!(), $function),
            $script,
            $body,
        )
    };
}

/// A fresh folder for a test.
fn temp_dir(test: &str) -> PathBuf {
    let name = test.replace("::", "-");
    let dir =
        std::env::temp_dir().join(format!("neptron-session-it-{}-{name}", std::process::id()));
    // A leftover of an earlier run with the same process id; absence is
    // fine.
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

/// The built test core, next to the test binary, as the host's own test
/// finds it.
fn core_path() -> PathBuf {
    let exe = std::env::current_exe().expect("test binary path");
    let deps = exe.parent().expect("deps directory");
    let name = if cfg!(target_os = "macos") {
        "libneptron_testcore.dylib"
    } else {
        "libneptron_testcore.so"
    };
    [
        deps.join(name),
        deps.parent().expect("profile directory").join(name),
    ]
    .into_iter()
    .find(|p| p.exists())
    .unwrap_or_else(|| panic!("no {name} next to {}", exe.display()))
}

/// What a test can change about its session.
#[derive(Debug, Clone)]
pub struct Setup {
    pub monitor: &'static str,
    pub caps: GpuCaps,
    pub policy: PolicySettings,
    pub scanlines: ScanlineSettings,
    pub display: DisplayOptions,
    pub device_rate_mhz: Option<u64>,
    pub resampling: Resampling,
    pub run_rate: RunRateSource,
    pub start_hold_ns: u64,
    pub core_options: Vec<(String, String)>,
    pub volume_percent: u32,
    pub keep_samples: bool,
}

impl Default for Setup {
    fn default() -> Self {
        Self {
            monitor: "generic_15",
            caps: CAPS_LOW_CLOCK,
            policy: PolicySettings::default(),
            scanlines: ScanlineSettings::OFF,
            display: DisplayOptions::default(),
            device_rate_mhz: None,
            resampling: Resampling::Polynomial,
            run_rate: RunRateSource::ReadBack,
            start_hold_ns: 0,
            core_options: Vec::new(),
            volume_percent: 100,
            keep_samples: false,
        }
    }
}

/// A launched session and clones of its fakes.
pub struct Rig {
    pub clock: SimClock,
    pub display: FakeDisplay,
    pub audio: FakeAudio,
    pub input: ScriptedInput,
    pub session: TestSession,
}

/// Launches the test core with the script in `dir` and `setup` on `audio`,
/// a sink of the test's own type, made on the session's clock with a
/// clone of the display.
pub fn launch_on<S: AudioSink>(
    dir: &Path,
    setup: Setup,
    audio: impl FnOnce(&SimClock, &FakeDisplay) -> S,
) -> (Session<FakeDisplay, S, ScriptedInput, SimClock>, SimClock) {
    let clock = SimClock::starting_at(START_NS);
    let display = FakeDisplay::with_options(clock.clone(), setup.caps, setup.display);
    let save_dir = dir.join("saves");
    let content = dir.join("game.ntc");
    let extensions = ["ntc".to_owned()];
    let core = core_path();
    let system_dir = dir.join("bios");
    let game = GameLaunch {
        core: &core,
        content: Some(&content),
        extensions: &extensions,
        system_dir: &system_dir,
        save_dir: &save_dir,
        core_options: &setup.core_options,
    };
    let sink = audio(&clock, &display);
    let video = VideoSetup {
        display,
        monitor: Monitor::Profile(monitor_preset(setup.monitor).expect("monitor preset")),
        policy: setup.policy,
        scanlines: setup.scanlines,
    };
    let devices = Devices {
        audio: sink,
        input: ScriptedInput::new(clock.clone(), Vec::new()),
        clock: clock.clone(),
    };
    let options = SessionOptions {
        resampling: setup.resampling,
        run_rate: setup.run_rate,
        ..SessionOptions::default()
    };
    let session = Session::launch(&game, video, devices, options).expect("launch");
    (session, clock)
}

impl Rig {
    /// Launches the test core with the script in `dir`.
    pub fn launch(dir: &Path, setup: Setup) -> Self {
        let clock = SimClock::starting_at(START_NS);
        let display = FakeDisplay::with_options(clock.clone(), setup.caps, setup.display);
        let audio = FakeAudio::with_options(
            clock.clone(),
            DEVICE_RATE_HZ,
            AudioOptions {
                capacity_frames: SINK_CAPACITY_FRAMES,
                device_rate_mhz: setup.device_rate_mhz,
                keep_samples: setup.keep_samples,
            },
        );
        let input = ScriptedInput::new(clock.clone(), Vec::new());
        let save_dir = dir.join("saves");
        let content = dir.join("game.ntc");
        let extensions = ["ntc".to_owned()];
        let core = core_path();
        let system_dir = dir.join("bios");
        let game = GameLaunch {
            core: &core,
            content: Some(&content),
            extensions: &extensions,
            system_dir: &system_dir,
            save_dir: &save_dir,
            core_options: &setup.core_options,
        };
        let video = VideoSetup {
            display: display.clone(),
            monitor: Monitor::Profile(monitor_preset(setup.monitor).expect("monitor preset")),
            policy: setup.policy,
            scanlines: setup.scanlines,
        };
        let devices = Devices {
            audio: audio.clone(),
            input: input.clone(),
            clock: clock.clone(),
        };
        let options = SessionOptions {
            resampling: setup.resampling,
            run_rate: setup.run_rate,
            start_hold_ns: setup.start_hold_ns,
            volume_percent: setup.volume_percent,
        };
        let session = Session::launch(&game, video, devices, options).expect("launch");
        Self {
            clock,
            display,
            audio,
            input,
            session,
        }
    }

    /// Runs `n` frames with no UI.
    pub fn run(&mut self, n: u64) {
        for _ in 0..n {
            self.session.run_frame(None).expect("frame");
        }
    }

    /// The test core's frame number of the last core frame: settle frames
    /// and loop frames both run the core.
    pub fn core_frame(&self) -> u64 {
        let c = self.session.counters();
        c.settle_frames + c.core_frames - 1
    }

    /// Presses `controls` on port 0 now, so the next frame polls them.
    pub fn press(&self, controls: &[Control]) {
        let now = self.clock.now_ns();
        for control in controls {
            self.input.press(now, 0, *control);
        }
    }

    /// Releases `controls` on port 0 now.
    pub fn release(&self, controls: &[Control]) {
        let now = self.clock.now_ns();
        for control in controls {
            self.input.release(now, 0, *control);
        }
    }
}

/// The hash of the session's last game frame, as the test core hashes it.
pub fn frame_hash(frame: &Frame<'_>) -> u64 {
    let size = Size {
        width: u32::from(frame.width),
        height: u32::from(frame.height),
    };
    hash(frame.data, frame.pitch, size, core_format(frame.format)).expect("hash")
}

/// The hash of test core frame `n` with `buttons` held, at the size of
/// `frame`.
pub fn expected(frame: &Frame<'_>, n: u64, buttons: u16) -> u64 {
    let size = Size {
        width: u32::from(frame.width),
        height: u32::from(frame.height),
    };
    expected_hash(size, core_format(frame.format), n, buttons).expect("expected hash")
}

fn core_format(format: PixelFormat) -> CoreFormat {
    match format {
        PixelFormat::Xrgb8888 => CoreFormat::Xrgb8888,
        PixelFormat::Rgb565 => CoreFormat::Rgb565,
        PixelFormat::Xrgb1555 => CoreFormat::Rgb1555,
    }
}
