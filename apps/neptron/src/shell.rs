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
mod mapping;
mod picture;
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
use neptron_input::{PadRoster, Roster};
use neptron_session::{RunRateSource, Session};
use neptron_settings::{Level, Value};
use neptron_types::{InputEvent, Modeline, Rotation, SourceGeometry, TvStandard};
use neptron_ui::screens::Screens;
use neptron_ui::{SceneCanvas, Snapshot, UiAction, UiFrame};
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
pub struct Shell<D: Display, S: AudioSink, I: InputSource + PadRoster, C: Clock + Clone> {
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
    canvas: SceneCanvas,
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
    picture_notice: PictureNotice,
    /// The picks of this run at the game level that also set the other
    /// picture row to Auto: (the key picked, the other key). A keep that
    /// moves the pick to another level moves both keys.
    game_resets: Vec<(&'static str, &'static str)>,
    /// The keys whose move to another level the gate of that level refused
    /// in this batch of intents: their value stays at the game level.
    refused_keeps: Vec<&'static str>,
    /// The one-time notices shown already, as the notices file holds them.
    shown_notices: crate::notices::ShownNotices,
    /// The pad notices of this power cycle and the silent pad rule.
    pad_notes: crate::pad_notes::PadNotes,
    /// The controller layouts of the player, as the file holds them.
    pad_maps: crate::pad_maps::PadMapsFile,
    /// The guided mapping that runs, in the game list.
    mapping: Option<mapping::MappingRun>,
    /// The id of the last guided mapping.
    next_mapping: u32,
    /// The notices of the input source of the frame, a buffer made at the
    /// start.
    input_notices: Vec<neptron_input::InputNotice>,
    next_notice: u32,
    /// The notice that asks whether to start an arcade game that cannot be
    /// checked, and that game.
    unchecked_launch: Option<(neptron_ui::snapshot::NoticeId, neptron_ui::snapshot::GameId)>,
    frame: Rc<Cell<u64>>,
    control_queue: Rc<RefCell<VecDeque<(u64, InputEvent)>>>,
    log: VecDeque<String>,
    /// The connector and the monitor of this run, the place of the guard
    /// record.
    here: crate::record::Place,
    /// The last known good of every place in the guard record, as the
    /// last write left them.
    places: Vec<crate::record::PlaceEntry>,
    /// The format of the share partition that the storage script offers
    /// at this boot.
    share_offer: Option<crate::storage::FormatOffer>,
    format_request: FormatRequest,
    /// The reader of the network status, `None` when its thread did not
    /// start.
    network: Option<crate::network::NetworkWatch>,
    /// The last network status the reader gave, `None` before its first,
    /// `Unknown` when the reader did not start.
    network_info: Option<crate::network::NetworkInfo>,
    /// The time of the next read while the Network panel is open, in
    /// nanoseconds of the clock.
    network_next_ns: u64,
    /// True when the Network panel was the screen on show in the last
    /// frame.
    network_shown: bool,
    /// The reads asked for since the start.
    network_requests: u64,
    /// The builds of the Network page since the start.
    network_builds: u64,
    /// The roster counter of the input source at the last check.
    pad_changes: u64,
    outcome: Option<Outcome>,
}

/// Where the format request of this run is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FormatRequest {
    /// None was asked for.
    None,
    /// The writer thread has the request file; the reboot waits for its
    /// result.
    Waiting,
    /// The request file was not written: nothing is formatted, and the row
    /// stays disabled in this run.
    Failed,
    /// The request file is written, but the run took no reboot: the next
    /// start formats, and the row stays disabled in this run.
    Written,
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

/// When the picture notice of a game shows.
#[derive(Debug, Clone, Copy, PartialEq)]
enum PictureNotice {
    /// At the first picture of the game.
    Launch,
    /// At the first picture after a change of the picture mode or the
    /// refresh choice in the game whose proposal differs from the one with
    /// the hash `before` (`proposal_hash`), within `frames` more frames.
    AfterChange {
        /// The hash of the proposal before the change.
        before: u64,
        /// The frames left to wait for a new picture.
        frames: u32,
    },
    /// Shown; nothing waits.
    Shown,
}

/// The frames that a change of the picture mode or the refresh choice
/// waits for a new picture before the notice stops waiting: a modeset waits
/// `DEBOUNCE_FRAMES` frames for a steady source, and a change that gives
/// the same picture must not leave a notice waiting for a later, unrelated
/// one.
const PICTURE_CHANGE_WAIT_FRAMES: u32 = 4 * neptron_video::DEBOUNCE_FRAMES;

impl PictureNotice {
    /// The state after `change` (a change of the picture mode or refresh
    /// choice) with the current proposal `current`.
    fn after_change(current: u64) -> Self {
        Self::AfterChange {
            before: current,
            frames: PICTURE_CHANGE_WAIT_FRAMES,
        }
    }

    /// One frame with the proposal `current` on screen: the next state,
    /// and whether the notice shows now.
    fn step(self, current: u64) -> (Self, bool) {
        match self {
            Self::Shown => (Self::Shown, false),
            Self::Launch => (Self::Shown, true),
            Self::AfterChange { before, .. } if before != current => (Self::Shown, true),
            Self::AfterChange { frames: 0, .. } => (Self::Shown, false),
            Self::AfterChange { before, frames } => (
                Self::AfterChange {
                    before,
                    frames: frames - 1,
                },
                false,
            ),
        }
    }
}

/// The hash of a proposal, to tell a new picture from the one before a
/// change.
fn proposal_hash(proposal: &neptron_video::Proposal) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::hash::DefaultHasher::new();
    proposal.hash(&mut hasher);
    hasher.finish()
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

impl<I: PadRoster> PadRoster for ControlInput<I> {
    fn roster(&self) -> Roster {
        self.inner.roster()
    }

    fn roster_changes(&self) -> u64 {
        self.inner.roster_changes()
    }

    fn take_notices(&self, out: &mut Vec<neptron_input::InputNotice>) {
        self.inner.take_notices(out);
    }

    fn activity(&self) -> neptron_input::PadActivity {
        self.inner.activity()
    }

    fn set_player_maps(&mut self, maps: &neptron_input::PlayerMaps) {
        self.inner.set_player_maps(maps);
    }

    fn capture(&mut self, pad: Option<neptron_input::PadKey>) {
        self.inner.capture(pad);
    }

    fn captured(&self) -> neptron_input::Capture {
        self.inner.captured()
    }

    fn pad_map(&self, pad: neptron_input::PadKey) -> Option<neptron_input::PadMap> {
        self.inner.pad_map(pad)
    }
}

impl<D: Display, S: AudioSink, I: InputSource + PadRoster, C: Clock + Clone> Shell<D, S, I, C> {
    /// Runs frames until the run ends.
    pub fn run(&mut self) -> Outcome {
        loop {
            if let Some(outcome) = self.step() {
                return outcome;
            }
        }
    }

    /// The pad on each port of the input source now.
    pub fn roster(&self) -> Roster {
        match &self.stage {
            Stage::List(list) => list.input.roster(),
            Stage::Game(game) => game.session.input().roster(),
            Stage::Ended => Roster::new(),
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

    /// The canvas of the UI: the pixels of the last scene drawn, and how
    /// many scenes it drew.
    pub fn canvas(&self) -> &SceneCanvas {
        &self.canvas
    }

    /// The guard.
    pub fn guard(&self) -> &Guard {
        &self.guard
    }

    /// The reads of the network status asked for, and the builds of the
    /// Network page, since the start: they stay still while the panel is
    /// closed.
    pub fn network_counts(&self) -> (u64, u64) {
        (self.network_requests, self.network_builds)
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

    /// The RetroPad state of each port after the last frame: what the
    /// menus read, and in a game what the core reads. `None` after the end.
    pub fn ports(&self) -> Option<&PortStates> {
        match &self.stage {
            Stage::List(list) => Some(&list.ports),
            Stage::Game(game) => Some(game.session.ports()),
            Stage::Ended => None,
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
    /// file. Returns the writes that failed. The end is clean, so the
    /// guard record clears its run marker first.
    pub fn finish(mut self) -> Vec<String> {
        if let Some(record) = self.guard.close_run().save {
            self.save_record(record);
        }
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

#[cfg(test)]
mod picture_notice_tests {
    use super::*;

    /// After a change that gives the same picture, the notice stops
    /// waiting, so a later picture of another cause does not show it.
    #[test]
    fn a_change_with_the_same_picture_stops_waiting() {
        let mut state = PictureNotice::after_change(7);
        for _ in 0..PICTURE_CHANGE_WAIT_FRAMES {
            let (next, show) = state.step(7);
            assert!(!show);
            state = next;
        }
        let (next, show) = state.step(7);
        assert!(!show);
        assert_eq!(next, PictureNotice::Shown);
        assert_eq!(next.step(8), (PictureNotice::Shown, false));
    }

    #[test]
    fn a_new_picture_after_a_change_shows_the_notice_once() {
        let (next, show) = PictureNotice::after_change(7).step(8);
        assert!(show);
        assert_eq!(next.step(9), (PictureNotice::Shown, false));
        assert_eq!(PictureNotice::Launch.step(1), (PictureNotice::Shown, true));
    }
}
