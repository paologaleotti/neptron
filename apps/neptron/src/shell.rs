//! The frontend of one run: the game list or one game, the UI over it, the
//! guard on the display, and the settings the menus write.
//!
//! One run is one process: it shows the game list, or it runs one game (a
//! core per process). A launch and the end of a game end the run with a
//! relaunch; the launcher starts the next run at once.

mod boot;
mod frame;
mod guard_events;
mod intents;
mod protocol;
mod view;

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use neptron_audio::Resampling;
use neptron_backend::{AudioSink, Clock, Display, InputSource};
use neptron_input::{Hotkey, Hotkeys, PortStates};
use neptron_session::{RunRateSource, Session};
use neptron_settings::{Level, Value};
use neptron_types::{InputEvent, Modeline, Rotation, SourceGeometry, TvStandard};
use neptron_ui::screens::Screens;
use neptron_ui::{CpuCanvas, Snapshot, UiAction, UiFrame};
use neptron_video::{DisplayConfig, Guard, VideoEngine, VideoStatusText};

use crate::config::Config;
use crate::display::Detected;
use crate::library::Library;
use crate::nav::Navigator;
use crate::outbox::Outbox;
use crate::paths::Paths;
use crate::presets::Presets;
use crate::relaunch::{GameRef, NextRun};

pub use boot::{BootError, Booted};

/// The source of the menus: the 320x240 canvas at 60 Hz, 4:3. The engine
/// picks the menu's mode for it with the same policy and validator as for
/// a game.
pub const MENU_SOURCE: SourceGeometry = SourceGeometry {
    width: 320,
    height: 240,
    refresh_mhz: 60_000,
    aspect_milli: 1_333,
    interlaced: false,
    rotation: Rotation::R0,
};

/// The frame time the loop waits for itself when no flip paces it (the
/// output is off, or the display failed), in nanoseconds: one frame at
/// 60 Hz.
pub const UNPACED_FRAME_NS: u64 = 16_666_667;

/// The lines the control protocol's log keeps.
const LOG_LINES: usize = 64;

/// The display, audio, input and clock of a run.
#[derive(Debug)]
pub struct Devices<D, S, I, C> {
    /// The display.
    pub display: D,
    /// The audio sink.
    pub audio: S,
    /// The input source.
    pub input: I,
    /// The clock. Clones share one time.
    pub clock: C,
}

/// What the backend decides about a run.
pub struct Runtime {
    /// Where the session takes the run rate of the audio.
    pub run_rate: RunRateSource,
    /// The resampler.
    pub resampling: Resampling,
    /// Waits until a time of the clock, in nanoseconds. It paces the loop
    /// while no flip does.
    pub pace: Box<dyn FnMut(u64)>,
    /// Set when the run must end as a quit: SIGTERM, SIGINT, or a closed
    /// window. `None` for a run that only ends by itself.
    pub quit: Option<Arc<AtomicBool>>,
    /// The log of the run off the frame thread; `None` keeps the lines in
    /// the protocol log only.
    pub log: Option<crate::logbook::LogLines>,
    /// The name of the output, for the display panel and the guard record.
    pub output: String,
    /// The `subconnector` property of the output, when the backend reads
    /// one: `VGA` names a DAC behind a DisplayPort connector.
    pub subconnector: Option<String>,
    /// The TV standards of the output, as the backend reads its `TV mode`
    /// property: `None` when it has none (HDMI, VGA, a window). With the
    /// property, every mode of the run comes from a TV profile.
    pub tv_standards: Option<Vec<TvStandard>>,
    /// What the backend could not open, as notices for the player: a run
    /// goes on with no sound or no gamepad.
    pub notices: Vec<String>,
    /// The playback devices the audio backend listed at start, as the
    /// options of the audio output row after `auto`.
    pub audio_outputs: Vec<neptron_audio::OutputDevice>,
    /// True when the launcher carries out a reboot or a power-off
    /// ([`crate::power::POWER_ENV`]). Else the rows of the System panel
    /// are disabled.
    pub power: bool,
}

/// How a run ends.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Start the next run at once with this relaunch file.
    Relaunch(NextRun),
    /// Stop: the user closed the window.
    Quit,
    /// Stop, and ask the launcher to reboot the machine.
    Reboot,
    /// Stop, and ask the launcher to turn the machine off.
    PowerOff,
}

/// The input of a run with the events of the control protocol added: the
/// session polls one source, so the protocol's events must come through
/// it.
#[derive(Debug)]
pub struct ControlInput<I> {
    inner: I,
    queue: Rc<RefCell<VecDeque<(u64, InputEvent)>>>,
    frame: Rc<Cell<u64>>,
}

/// The frontend of one run.
pub struct Shell<D: Display, S: AudioSink, I: InputSource, C: Clock + Clone> {
    paths: Paths,
    config: Config,
    presets: Presets,
    detected: Detected,
    safe: DisplayConfig,
    library: Library,
    guard: Guard,
    stage: Stage<D, S, I, C>,
    clock: C,
    runtime: Runtime,
    screens: Screens,
    ui: UiFrame,
    canvas: CpuCanvas,
    snap: Snapshot,
    nav: Navigator,
    actions: Vec<UiAction>,
    events: Vec<InputEvent>,
    outbox: Outbox,
    pending: Vec<PendingChange>,
    status_text: VideoStatusText,
    shown_mode: Option<Modeline>,
    shown_safe: bool,
    now_ns: u64,
    menu_hotkey: bool,
    /// Panel windows of this run: the boot prompt of a panel and the one
    /// offer of `offer_panel`.
    panel_windows: u8,
    /// `offer_panel` has fired in this run.
    panel_offered: bool,
    picture_noticed: bool,
    next_notice: u32,
    frame: Rc<Cell<u64>>,
    control_queue: Rc<RefCell<VecDeque<(u64, InputEvent)>>>,
    log: VecDeque<String>,
    /// The connector and the monitor of the last known good in the guard
    /// record.
    record_place: Option<crate::record::Place>,
    outcome: Option<Outcome>,
}

/// What the run shows.
enum Stage<D: Display, S: AudioSink, I: InputSource, C: Clock> {
    /// The menus with no game: the app drives the engine and the input.
    List(Box<ListStage<D, S, I>>),
    /// One game: the session drives the engine and the input.
    Game(Box<GameStage<D, S, I, C>>),
    /// The game ended; the run ends with this frame.
    Ended,
}

struct ListStage<D: Display, S: AudioSink, I: InputSource> {
    engine: VideoEngine<D>,
    input: ControlInput<I>,
    audio: S,
    ports: PortStates,
    hotkeys: Hotkeys,
    fired: Vec<Hotkey>,
}

struct GameStage<D: Display, S: AudioSink, I: InputSource, C: Clock> {
    session: Session<D, S, ControlInput<I>, C>,
    reference: GameRef,
    core: String,
}

/// A display setting that is in memory and on screen, and waits for the
/// confirmation before it reaches its file.
#[derive(Debug, Clone, PartialEq)]
struct PendingChange {
    level: Level,
    key: &'static str,
    old: Option<Value>,
    new: Option<Value>,
}

impl<I: InputSource> ControlInput<I> {
    /// `inner` with the events of `queue` added at their frame.
    fn new(
        inner: I,
        queue: Rc<RefCell<VecDeque<(u64, InputEvent)>>>,
        frame: Rc<Cell<u64>>,
    ) -> Self {
        Self {
            inner,
            queue,
            frame,
        }
    }
}

impl<I: InputSource> InputSource for ControlInput<I> {
    fn poll(&mut self, out: &mut Vec<InputEvent>) {
        self.inner.poll(out);
        let frame = self.frame.get();
        let mut queue = self.queue.borrow_mut();
        while queue.front().is_some_and(|(at, _)| *at <= frame) {
            if let Some((_, event)) = queue.pop_front() {
                out.push(event);
            }
        }
    }
}

impl<D: Display, S: AudioSink, I: InputSource, C: Clock + Clone> Shell<D, S, I, C> {
    /// Runs frames until the run ends.
    pub fn run(&mut self) -> Outcome {
        loop {
            if let Some(outcome) = self.step() {
                return outcome;
            }
        }
    }

    /// The time of the last frame, in nanoseconds of the clock.
    pub fn now_ns(&self) -> u64 {
        self.now_ns
    }

    /// The frame counter: frames run since the start.
    pub fn frame(&self) -> u64 {
        self.frame.get()
    }

    /// The snapshot the UI showed last.
    pub fn snapshot(&self) -> &Snapshot {
        &self.snap
    }

    /// The UI frame of the last frame: its scene, its tree, its intents.
    pub fn ui(&self) -> &UiFrame {
        &self.ui
    }

    /// The canvas of the last frame that drew the UI.
    pub fn canvas(&self) -> &CpuCanvas {
        &self.canvas
    }

    /// The guard.
    pub fn guard(&self) -> &Guard {
        &self.guard
    }

    /// The library.
    pub fn library(&self) -> &Library {
        &self.library
    }

    /// The folders of the run.
    pub fn paths(&self) -> &Paths {
        &self.paths
    }

    /// The settings of the run.
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// True while a game runs.
    pub fn in_game(&self) -> bool {
        matches!(self.stage, Stage::Game(_))
    }

    /// The volume of the running game's audio, in percent. `None` with no
    /// game, or after its audio was lost.
    pub fn game_volume_percent(&self) -> Option<u32> {
        match &self.stage {
            Stage::Game(game) => game.session.volume_percent(),
            _ => None,
        }
    }

    /// True while the session of the game is paused under the quick menu.
    pub fn paused(&self) -> bool {
        matches!(&self.stage, Stage::Game(g) if g.session.paused())
    }

    /// The counters of the audio pipeline of the game. `None` in the game
    /// list and after the audio was lost.
    pub fn audio_stats(&self) -> Option<neptron_audio::AudioStats> {
        match &self.stage {
            Stage::Game(game) => game.session.audio_stats().copied(),
            _ => None,
        }
    }

    /// The video engine: the mode on screen and the trace.
    pub fn engine(&self) -> Option<&VideoEngine<D>> {
        match &self.stage {
            Stage::List(list) => Some(&list.engine),
            Stage::Game(game) => Some(game.session.video()),
            Stage::Ended => None,
        }
    }

    /// Ends the run off the frame loop: every queued write reaches its
    /// file. Returns the writes that failed.
    pub fn finish(self) -> Vec<String> {
        let Self { stage, outbox, .. } = self;
        let mut messages = Vec::new();
        if let Stage::Game(game) = stage {
            let report = game.session.quit();
            if let Some(text) = intents::lost_save(&report) {
                messages.push(text);
            }
        }
        messages.extend(outbox.finish());
        messages
    }

    /// Adds a line to the log of the control protocol, and queues it for
    /// the log of the run. The frame thread never writes it itself.
    fn log(&mut self, line: String) {
        if let Some(log) = &self.runtime.log {
            log.send(format!("neptron: {line}"));
        }
        if self.log.len() == LOG_LINES {
            self.log.pop_front();
        }
        self.log.push_back(line);
    }
}
