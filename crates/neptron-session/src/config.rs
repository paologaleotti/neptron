use std::path::{Path, PathBuf};

use neptron_audio::Resampling;
use neptron_backend::ContextOffer;

use crate::{InternalScale, Machine};
use neptron_video::{Monitor, PolicySettings, ScanlineSettings};

/// The game to launch: the core, the content and the folders.
#[derive(Debug, Clone, Copy)]
pub struct GameLaunch<'a> {
    /// The core library.
    pub core: &'a Path,
    /// The content: a plain file or a `.zip` file. `None` for a core that
    /// runs with no content.
    pub content: Option<&'a Path>,
    /// The extensions that the system accepts, without dots.
    pub extensions: &'a [String],
    /// The BIOS folder.
    pub system_dir: &'a Path,
    /// The save folder of the system. The session makes it when it is
    /// missing. The core gets it as its save directory.
    pub save_dir: &'a Path,
    /// The folders of the game below its system's ROM folder, in order:
    /// empty for a game at the top of the system folder. The save file is
    /// `<save_dir>/<folders>/<content stem>.srm`, so two games of one name
    /// in two folders keep two saves. The session makes the folders and
    /// follows no link (see `neptron_libretro::save_folder`).
    pub save_folders: &'a [String],
    /// Before the save followed the folders, a game in a folder used the
    /// flat save `<save_dir>/<content stem>.srm`. When this is true, the
    /// game is in a folder, it has no save file of its own yet, and the
    /// flat save exists, the launch copies the flat save to the game's own
    /// path first, and never changes or removes the flat save. The caller
    /// sets it only when no game of the same name is at the top of the
    /// system folder: the flat save is that game's save.
    pub adopt_flat_save: bool,
    /// Core option values (key, value): the defaults of the core data,
    /// then the values from the settings, so a later pair for a key wins.
    /// The host holds them and applies each one when the core declares its
    /// key, also during the load. A key the core does not declare by the
    /// end of the load, or a value it does not list, is counted in
    /// `Counters::refused_options`; `Host::held_option_report` names them.
    pub core_options: &'a [(String, String)],
    /// Every frame of the system is progressive (the system data key
    /// `always_progressive`): the line count of a frame does not make it
    /// interlaced.
    pub always_progressive: bool,
    /// The core option that sets the internal resolution of a GL core, from
    /// the core data. The session holds its value for the `k` of
    /// [`internal_scale`](crate::internal_scale) with the core options.
    /// `None` for a core with no such option.
    pub internal_scale: Option<InternalScale<'a>>,
    /// The discs after the first of a group of disc files, in order. The
    /// launch adds them with the disc control interface of the core after
    /// the load (`docs/spec/disc-games.md`, section 9). Empty for every
    /// other game.
    pub more_discs: &'a [PathBuf],
    /// The name of the save file with no extension, in place of the stem
    /// of the content: the name of a disc game with no disc tag, so that
    /// every disc of a game shares one save, which holds its memory card.
    /// `None` takes the stem of the content.
    pub save_name: Option<&'a str>,
    /// The core gets the save folder of the game, which follows
    /// `save_folders`, as its save directory, in place of `save_dir`: a
    /// disc core that writes its own memory card files puts them with the
    /// saves of the game.
    pub core_saves_in_game_folder: bool,
}

/// The display and the resolved video settings.
#[derive(Debug, Clone)]
pub struct VideoSetup<D> {
    /// The display backend.
    pub display: D,
    /// The monitor.
    pub monitor: Monitor,
    /// From `display.style` and `display.refresh_policy`.
    pub policy: PolicySettings,
    /// From `display.scanlines` and `display.scanline_strength`. The engine
    /// draws no scanlines until it gets them, so they are part of the
    /// launch.
    pub scanlines: ScanlineSettings,
    /// The context types that the display offers to a GL core, as the app
    /// keeps them from an earlier probe of the same display. `None` asks
    /// the display, which probes its GPU once per process: a cost that a
    /// software core should not pay at each launch.
    pub core_offer: Option<ContextOffer>,
}

/// The audio, input and time of the session.
#[derive(Debug, Clone)]
pub struct Devices<S, I, C> {
    /// The audio sink.
    pub audio: S,
    /// The input source. The session polls it once per frame.
    pub input: I,
    /// The clock of the hotkey holds and the save RAM timer.
    pub clock: C,
}

/// Choices of the session that come from the backend and the settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionOptions {
    /// The resampler of the audio pipeline.
    pub resampling: Resampling,
    /// Where the run rate for audio rate control comes from.
    pub run_rate: RunRateSource,
    /// From `input.menu_hold_start_seconds`, in nanoseconds. 0 is off.
    pub start_hold_ns: u64,
    /// From `audio.volume`, in percent: 100 is the core's own level, 0 is
    /// silence. The pipeline starts at it with no ramp.
    pub volume_percent: u32,
    /// The machine, for the internal resolution of a GL core.
    pub machine: Machine,
}

/// Where the session takes the run rate (core frames per second of real
/// time) for audio rate control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunRateSource {
    /// The field rate of the mode read back, over the flips per core frame.
    /// For KMS: the display flips at the rate of the mode.
    ReadBack,
    /// The flip rate measured from the flip times, over the flips per core
    /// frame. For the desktop window, which flips at the desktop rate.
    Measured,
}

impl Default for SessionOptions {
    fn default() -> Self {
        Self {
            resampling: Resampling::Sinc,
            run_rate: RunRateSource::ReadBack,
            start_hold_ns: 0,
            volume_percent: neptron_audio::VOLUME_MAX_PERCENT,
            machine: Machine::Pc,
        }
    }
}
