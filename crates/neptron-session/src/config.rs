use std::path::Path;

use neptron_audio::Resampling;
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
    /// missing.
    pub save_dir: &'a Path,
    /// Core option values (key, value) from the settings. A key the core
    /// does not declare, or a value it does not list, is skipped and
    /// counted.
    pub core_options: &'a [(String, String)],
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
        }
    }
}
