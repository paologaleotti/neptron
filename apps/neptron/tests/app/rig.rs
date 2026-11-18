//! A frontend run on the fakes, and the helpers that drive it like a
//! player: buttons, frames and the UI tree of the control protocol.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use neptron::{Booted, Devices, NextRun, Outcome, Paths, Runtime, Shell};
use neptron_audio::Resampling;
use neptron_backend::{Clock, InputSource};
use neptron_input::{
    JoystickCounts, PadInfo, PadRoster, PortRule, Roster, ScriptedPad, ScriptedPads,
};
use neptron_ipc::{Handler, UiTree};
use neptron_session::RunRateSource;
use neptron_testcore::SCRIPT_ENV;
use neptron_testkit::preset::CAPS_LOW_CLOCK;
use neptron_testkit::{FakeAudio, FakeDisplay, ScriptedInput, SimClock};
use neptron_types::Control;

/// A run of the frontend on the fakes.
pub type TestShell = Shell<FakeDisplay, FakeAudio, RigInput, SimClock>;

/// The input of a run: the scripted pads of the run, then the script.
#[derive(Debug, Clone)]
pub struct RigInput {
    pads: ScriptedPads,
    script: ScriptedInput,
}

impl InputSource for RigInput {
    fn poll(&mut self, out: &mut Vec<neptron_types::InputEvent>) {
        self.pads.poll(out);
        self.script.poll(out);
    }
}

impl PadRoster for RigInput {
    fn roster(&self) -> Roster {
        self.pads.roster()
    }

    fn roster_changes(&self) -> u64 {
        self.pads.roster_changes()
    }

    fn take_notices(&self, out: &mut Vec<neptron_input::InputNotice>) {
        self.pads.take_notices(out);
    }

    fn activity(&self) -> neptron_input::PadActivity {
        self.pads.activity()
    }

    fn set_player_maps(&mut self, maps: &neptron_input::PlayerMaps) {
        self.pads.set_player_maps(maps);
    }

    fn capture(&mut self, pad: Option<neptron_input::PadKey>) {
        self.pads.capture(pad);
    }

    fn captured(&self) -> neptron_input::Capture {
        self.pads.captured()
    }

    fn pad_map(&self, pad: neptron_input::PadKey) -> Option<neptron_input::PadMap> {
        self.pads.pad_map(pad)
    }
}

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

/// The playback devices of the fake backend: the text of `/proc/asound`
/// of a PC with an HDA card, whose monitor is on one of three HDMI outputs,
/// and a USB card.
pub fn audio_outputs() -> Vec<neptron_audio::OutputDevice> {
    let absent = "monitor_present\t0\neld_valid\t0\n".to_owned();
    neptron_audio::alsa_outputs(&neptron_audio::AlsaText {
        cards: " 0 [PCH            ]: HDA-Intel - HDA Intel PCH\n 1 [Device         ]: USB-Audio - USB Audio\n".to_owned(),
        pcm: "00-00: ALC221 Analog : ALC221 Analog : playback 1 : capture 1\n00-03: HDMI 0 : HDMI 0 : playback 1\n00-07: HDMI 1 : HDMI 1 : playback 1\n01-00: USB Audio : USB Audio : playback 1\n".to_owned(),
        elds: vec![(0, absent.clone()), (0, absent)],
    })
}

/// The longest real time that [`Rig::scanned`] waits for the game scan:
/// a guard against a hang only, far above what a scan of the test folder
/// takes on a loaded machine.
const SCAN_WAIT_MAX: std::time::Duration = std::time::Duration::from_secs(60);

/// The device rate of the fake audio.
const DEVICE_RATE_HZ: u32 = 48_000;
/// The queue of the fake audio, in frames.
const SINK_CAPACITY_FRAMES: usize = 6_144;

/// The fakes and the folders of a run, kept across the runs of one test,
/// as the launcher keeps the device across the runs of a boot.
pub struct Rig {
    /// What the backend could not open, for the runtime.
    pub notices: Vec<String>,
    /// The name of the output the runtime reports.
    pub output: String,
    /// The TV standards the runtime reports for the output, as the `TV
    /// mode` property of its connector gives them.
    pub tv_standards: Option<Vec<neptron_types::TvStandard>>,
    pub paths: Paths,
    pub clock: SimClock,
    pub display: FakeDisplay,
    pub audio: FakeAudio,
    pub input: ScriptedInput,
    /// The pads of the run. Each run opens them again, as the SDL3 source
    /// of a new process does.
    pub pads: ScriptedPads,
    /// The pads plugged in the machine, in plug order. They stay plugged
    /// across runs.
    pub plugged: Vec<(PadInfo, ScriptedPad)>,
    /// The joysticks with no gamepad mapping plugged in the machine, in
    /// plug order, after the pads. They stay plugged across runs.
    pub joysticks: Vec<(PadInfo, JoystickCounts, ScriptedPad)>,
    pub shell: Option<Box<TestShell>>,
    /// The stop flag of the runtime, as SIGTERM and SIGINT set it.
    pub stop: Arc<AtomicBool>,
    /// Power control, as a launcher with `--power` gives it. On by
    /// default, as on the image.
    pub power: bool,
}

/// Runs `body` in a child process of the test binary with the test core's
/// script variable set, because one process holds one core and setting a
/// variable in the running process is `unsafe`. `test` is the test's path
/// in the binary.
pub fn in_child(test: &str, body: impl FnOnce(&Path)) {
    in_child_with(test, SCRIPT, body);
}

/// [`in_child`] with the test core's script `script`.
pub fn in_child_with(test: &str, script: &str, body: impl FnOnce(&Path)) {
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
    let script_path = dir.join("script.ntc");
    std::fs::write(&script_path, script).expect("the script");
    let script = script_path;
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
    // The child has ended and passed: its folder is not needed. A folder
    // that cannot be removed only leaves files in the temp folder.
    let _ = std::fs::remove_dir_all(&dir);
}

/// The variable that gives a child of [`in_children`] its stage.
const STAGE_ENV: &str = "NEPTRON_TEST_STAGE";

/// Runs `body` in `stages` child processes one after the other, on one
/// folder, each with its stage number: one process holds one core, so a
/// chain of runs through two games needs a process for each run, as the
/// launcher gives it.
pub fn in_children(test: &str, stages: u32, body: impl Fn(&Path, u32)) {
    in_children_with(test, SCRIPT, stages, body);
}

/// [`in_children`] with the test core's script `script`.
pub fn in_children_with(test: &str, script: &str, stages: u32, body: impl Fn(&Path, u32)) {
    if let Some(stage) = std::env::var_os(STAGE_ENV) {
        let stage: u32 = stage
            .to_str()
            .and_then(|s| s.parse().ok())
            .expect("a stage");
        let path = std::env::var_os(SCRIPT_ENV).expect("the script");
        let dir = PathBuf::from(path).parent().expect("the folder").to_owned();
        body(&dir, stage);
        std::fs::write(dir.join(format!("done-{stage}")), test).expect("the marker");
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
    let script_path = dir.join("script.ntc");
    std::fs::write(&script_path, script).expect("the script");
    let script = script_path;
    for stage in 0..stages {
        let status = Command::new(std::env::current_exe().expect("the test binary"))
            .args([
                test,
                "--exact",
                "--include-ignored",
                "--nocapture",
                "--test-threads=1",
            ])
            .env(SCRIPT_ENV, &script)
            .env(STAGE_ENV, stage.to_string())
            .status()
            .expect("start the child");
        assert!(status.success(), "stage {stage} of {test} failed");
        assert!(
            dir.join(format!("done-{stage}")).exists(),
            "stage {stage} ran no test"
        );
    }
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
        let link = paths.cores.join("fceumm_libretro.so");
        if !link.exists() {
            std::os::unix::fs::symlink(core_path(), link).expect("the core link");
        }
        let clock = SimClock::starting_at(1_000_000);
        Self {
            notices: Vec::new(),
            output: "fake-1".to_owned(),
            tv_standards: None,
            display: FakeDisplay::new(clock.clone(), CAPS_LOW_CLOCK),
            audio: FakeAudio::new(clock.clone(), DEVICE_RATE_HZ, SINK_CAPACITY_FRAMES),
            input: ScriptedInput::new(clock.clone(), Vec::new()),
            pads: ScriptedPads::new(PortRule::Menus, &Roster::new()),
            plugged: Vec::new(),
            joysticks: Vec::new(),
            clock,
            paths,
            shell: None,
            stop: Arc::new(AtomicBool::new(false)),
            power: true,
        }
    }

    /// Starts a run with the relaunch file `next`.
    pub fn boot(&mut self, next: Option<NextRun>) {
        if let Some((next, log)) = self.try_boot(next) {
            panic!("the run relaunched at boot: {next:?} {log:?}");
        }
    }

    /// Starts a run with the relaunch file `next`, and gives the relaunch
    /// and the log lines of a run that ends at its boot, as a game run
    /// whose game does not start.
    pub fn try_boot(&mut self, next: Option<NextRun>) -> Option<(NextRun, Vec<String>)> {
        let clock = self.clock.clone();
        let runtime = Runtime {
            run_rate: RunRateSource::ReadBack,
            resampling: Resampling::Polynomial,
            pace: Box::new(move |until_ns| {
                // A time already passed needs no wait.
                let _ = clock.advance_to(until_ns);
            }),
            quit: Some(self.stop.clone()),
            log: None,
            output: self.output.clone(),
            subconnector: None,
            tv_standards: self.tv_standards.clone(),
            notices: self.notices.clone(),
            audio_outputs: audio_outputs(),
            power: self.power,
        };
        let rule = match next.as_ref().and_then(|n| n.game.as_ref()) {
            Some(_) => PortRule::Game,
            None => PortRule::Menus,
        };
        let earlier = next
            .as_ref()
            .map(|n| neptron::relaunch::PadRecord::roster(&n.pads))
            .unwrap_or_default();
        self.pads = ScriptedPads::new(rule, &earlier);
        for (info, pad) in &mut self.plugged {
            *pad = self.pads.plug(info.clone());
        }
        for (info, counts, pad) in &mut self.joysticks {
            *pad = self.pads.plug_joystick(info.clone(), *counts);
        }
        let devices = Devices {
            display: self.display.clone(),
            audio: self.audio.clone(),
            input: RigInput {
                pads: self.pads.clone(),
                script: self.input.clone(),
            },
            clock: self.clock.clone(),
        };
        match Shell::boot(self.paths.clone(), devices, runtime, next).expect("boot") {
            Booted::Ready(shell) => {
                self.shell = Some(shell);
                None
            }
            Booted::Relaunch { next, log } => Some((next, log)),
        }
    }

    /// Plugs a pad into the machine: the run sees it at its next frame,
    /// and every later run finds it at its start.
    pub fn plug(&mut self, info: PadInfo) -> ScriptedPad {
        let pad = self.pads.plug(info.clone());
        self.plugged.push((info, pad));
        pad
    }

    /// Plugs a joystick with no gamepad mapping into the machine, as
    /// [`Rig::plug`] does.
    pub fn plug_joystick(&mut self, info: PadInfo, counts: JoystickCounts) -> ScriptedPad {
        let pad = self.pads.plug_joystick(info.clone(), counts);
        self.joysticks.push((info, counts, pad));
        pad
    }

    /// Unplugs the pad of the run `pad`.
    pub fn unplug(&mut self, pad: ScriptedPad) {
        self.pads.unplug(pad);
        self.plugged.retain(|(_, p)| *p != pad);
        self.joysticks.retain(|(_, _, p)| *p != pad);
    }

    /// Presses raw button `index` of the joystick `pad` for one frame,
    /// then releases it for two frames, as [`Rig::tap`] does.
    pub fn joy_tap(&mut self, pad: ScriptedPad, index: u8) {
        self.pads.set_joy_button(pad, index, true);
        self.frames(1);
        self.pads.set_joy_button(pad, index, false);
        self.frames(2);
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

    /// Runs frames until the game scan of the start has ended and the run
    /// has taken its result, so a scan that ends during a walk through the
    /// menus does not move the focus. The scan thread runs in real time,
    /// so the bound is real time, not a count of frames.
    pub fn scanned(&mut self) {
        let deadline = std::time::Instant::now() + SCAN_WAIT_MAX;
        while self.shell().library().scanning() {
            assert!(
                std::time::Instant::now() < deadline,
                "the scan did not end in {SCAN_WAIT_MAX:?}"
            );
            self.frames(1);
        }
        self.frames(2);
    }

    /// Keeps the open confirmation: a fresh right, then accept, once the
    /// change shows a picture.
    pub fn keep(&mut self) {
        assert!(
            self.shell().guard().pending().is_some(),
            "no confirmation to keep"
        );
        self.frames(2);
        self.tap(&[Control::Right]);
        self.tap(&[Control::A]);
    }

    /// The text of a file of the state folder, empty when it is missing.
    pub fn state_file(&self, path: &str) -> String {
        std::fs::read_to_string(self.paths.state.join(path)).unwrap_or_default()
    }
}
