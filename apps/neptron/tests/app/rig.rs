//! A frontend run on the fakes, and the helpers that drive it like a
//! player: buttons, frames and the UI tree of the control protocol.

use std::path::{Path, PathBuf};
use std::process::Command;

use neptron::{Booted, Devices, NextRun, Outcome, Paths, Runtime, Shell};
use neptron_audio::Resampling;
use neptron_backend::Clock;
use neptron_ipc::{Handler, UiTree};
use neptron_session::RunRateSource;
use neptron_testcore::SCRIPT_ENV;
use neptron_testkit::preset::CAPS_LOW_CLOCK;
use neptron_testkit::{FakeAudio, FakeDisplay, ScriptedInput, SimClock};
use neptron_types::Control;

/// A run of the frontend on the fakes.
pub type TestShell = Shell<FakeDisplay, FakeAudio, ScriptedInput, SimClock>;

/// The script of the test core: a NES-like source that changes its height
/// at core frame 300, so the engine switches the mode.
pub const SCRIPT: &str = "\
neptron-testcore 1
pixel_format rgb565
geometry 256 224 1333
max_size 320 240
timing 60099 48000
save_ram 64
at 300 geometry 256 240 1333
at 310 sram_write 0 7
";

/// The device rate of the fake audio.
const DEVICE_RATE_HZ: u32 = 48_000;
/// The queue of the fake audio, in frames.
const SINK_CAPACITY_FRAMES: usize = 6_144;

/// The fakes and the folders of a run, kept across the runs of one test,
/// as the launcher keeps the device across the runs of a boot.
pub struct Rig {
    pub paths: Paths,
    pub clock: SimClock,
    pub display: FakeDisplay,
    pub audio: FakeAudio,
    pub input: ScriptedInput,
    pub shell: Option<Box<TestShell>>,
}

/// Runs `body` in a child process of the test binary with the test core's
/// script variable set, because one process holds one core and setting a
/// variable in the running process is `unsafe`. `test` is the test's path
/// in the binary.
pub fn in_child(test: &str, body: impl FnOnce(&Path)) {
    if let Some(path) = std::env::var_os(SCRIPT_ENV) {
        let dir = PathBuf::from(path).parent().expect("the folder").to_owned();
        body(&dir);
        std::fs::write(dir.join("done"), test).expect("the marker");
        return;
    }
    let dir = std::env::temp_dir().join(format!(
        "neptron-app-{}-{}",
        std::process::id(),
        test.replace("::", "-")
    ));
    // A leftover of an earlier run with the same process id; absence is fine.
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("the folder");
    let script = dir.join("script.ntc");
    std::fs::write(&script, SCRIPT).expect("the script");
    let status = Command::new(std::env::current_exe().expect("the test binary"))
        .args([
            test,
            "--exact",
            "--include-ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(SCRIPT_ENV, &script)
        .status()
        .expect("start the child");
    assert!(status.success(), "the child process of {test} failed");
    assert!(
        dir.join("done").exists(),
        "the child ran no test named {test}"
    );
}

/// The test's path in the binary.
pub fn test_name(module: &str, function: &str) -> String {
    let module = module.split_once("::").map_or("", |(_, rest)| rest);
    format!("{module}::{function}")
}

/// The built test core, next to the test binary.
fn core_path() -> PathBuf {
    let exe = std::env::current_exe().expect("the test binary");
    let deps = exe.parent().expect("the deps folder");
    let name = if cfg!(target_os = "macos") {
        "libneptron_testcore.dylib"
    } else {
        "libneptron_testcore.so"
    };
    [deps.join(name), deps.parent().expect("profile").join(name)]
        .into_iter()
        .find(|p| p.exists())
        .unwrap_or_else(|| panic!("no {name} next to {}", exe.display()))
}

impl Rig {
    /// Folders under `root` with one NES game, the shipped presets and
    /// skin, and the test core under the library name of the NES core.
    pub fn new(root: &Path) -> Self {
        let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let paths = Paths {
            share: root.join("share"),
            state: root.join("state"),
            data,
            cores: root.join("cores"),
            run: root.join("run"),
            sysfs: root.join("sys"),
        };
        let nes = paths.roms().join("nes");
        std::fs::create_dir_all(&nes).expect("roms");
        std::fs::write(nes.join("Test Game.nes"), b"test core content").expect("a game");
        std::fs::create_dir_all(&paths.cores).expect("cores");
        std::os::unix::fs::symlink(core_path(), paths.cores.join("fceumm_libretro.so"))
            .expect("the core link");
        let clock = SimClock::starting_at(1_000_000);
        Self {
            display: FakeDisplay::new(clock.clone(), CAPS_LOW_CLOCK),
            audio: FakeAudio::new(clock.clone(), DEVICE_RATE_HZ, SINK_CAPACITY_FRAMES),
            input: ScriptedInput::new(clock.clone(), Vec::new()),
            clock,
            paths,
            shell: None,
        }
    }

    /// Starts a run with the relaunch file `next`.
    pub fn boot(&mut self, next: Option<NextRun>) {
        let clock = self.clock.clone();
        let runtime = Runtime {
            run_rate: RunRateSource::ReadBack,
            resampling: Resampling::Polynomial,
            pace: Box::new(move |until_ns| {
                // A time already passed needs no wait.
                let _ = clock.advance_to(until_ns);
            }),
            quit: None,
            output: "fake".to_owned(),
        };
        let devices = Devices {
            display: self.display.clone(),
            audio: self.audio.clone(),
            input: self.input.clone(),
            clock: self.clock.clone(),
        };
        match Shell::boot(self.paths.clone(), devices, runtime, next).expect("boot") {
            Booted::Ready(shell) => self.shell = Some(shell),
            Booted::Relaunch(next) => panic!("the run relaunched at boot: {next:?}"),
        }
    }

    /// The run.
    pub fn shell(&mut self) -> &mut TestShell {
        self.shell.as_mut().expect("a run")
    }

    /// Runs `n` frames. Panics when the run ends.
    pub fn frames(&mut self, n: u32) {
        for _ in 0..n {
            if let Some(outcome) = self.shell().step() {
                panic!("the run ended: {outcome:?}");
            }
        }
    }

    /// Runs frames until the run ends, at most `max`.
    pub fn until_end(&mut self, max: u32) -> Outcome {
        for _ in 0..max {
            if let Some(outcome) = self.shell().step() {
                return outcome;
            }
        }
        panic!("the run did not end in {max} frames");
    }

    /// Runs frames until `done` holds, at most `max`.
    pub fn until(&mut self, max: u32, what: &str, done: impl Fn(&mut Self) -> bool) {
        for _ in 0..max {
            if done(self) {
                return;
            }
            self.frames(1);
        }
        panic!("{what} did not happen in {max} frames");
    }

    /// Ends the run: every write reaches its file.
    pub fn finish(&mut self) {
        let shell = self.shell.take().expect("a run");
        let messages = shell.finish();
        assert!(messages.is_empty(), "{messages:?}");
    }

    /// Presses `controls` on port 0 for one frame, then releases them for
    /// two frames. In a game the session polls the input in its frame, and
    /// the UI reads those events in the next one, so the UI shows a press
    /// two frames after it.
    pub fn tap(&mut self, controls: &[Control]) {
        self.hold(controls);
        self.frames(1);
        self.release(controls);
        self.frames(2);
    }

    /// Presses `controls` on port 0 now.
    pub fn hold(&mut self, controls: &[Control]) {
        let now = self.clock.now_ns();
        for control in controls {
            self.input.press(now, 0, *control);
        }
    }

    /// Releases `controls` on port 0 now.
    pub fn release(&mut self, controls: &[Control]) {
        let now = self.clock.now_ns();
        for control in controls {
            self.input.release(now, 0, *control);
        }
    }

    /// The UI tree, as the control protocol gives it.
    pub fn tree(&mut self) -> UiTree {
        self.shell().ui_tree().expect("ui_tree")
    }

    /// The screen on show.
    pub fn screen(&mut self) -> String {
        self.tree().screen
    }

    /// The label of the focused row.
    pub fn focused(&mut self) -> Option<String> {
        self.tree().focused
    }

    /// Moves the focus to the row with `label`: to the top first, then
    /// down.
    pub fn focus(&mut self, label: &str) {
        for _ in 0..32 {
            let before = self.focused();
            if before.as_deref() == Some(label) {
                return;
            }
            self.tap(&[Control::Up]);
            if self.focused() == before {
                break;
            }
        }
        for _ in 0..32 {
            if self.focused().as_deref() == Some(label) {
                return;
            }
            self.tap(&[Control::Down]);
        }
        panic!("no row {label:?} on {}: {:?}", self.screen(), self.tree());
    }

    /// Keeps the open confirmation: the dialog moves its focus only after
    /// 60 quiet frames, then right and accept.
    pub fn keep(&mut self) {
        self.frames(61);
        self.tap(&[Control::Right]);
        self.tap(&[Control::A]);
    }

    /// The text of a file of the state folder, empty when it is missing.
    pub fn state_file(&self, path: &str) -> String {
        std::fs::read_to_string(self.paths.state.join(path)).unwrap_or_default()
    }
}
