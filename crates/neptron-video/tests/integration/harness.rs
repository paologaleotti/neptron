//! The engine on a fake display, and the sources the tests use.
//!
//! The tests do not assert which mode the policy picks for a source. They
//! assert what the engine does with the policy's answer: when it sets a
//! mode, how often, in which order, and what the trace says.

use neptron_testkit::preset::{CAPS_LOW_CLOCK, monitor_preset};
use neptron_testkit::{FakeDisplay, SimClock};
use neptron_types::{Frame, Modeline, PixelFormat, Rotation, SourceGeometry};
use neptron_video::{
    Directive, HorizontalStrategy, Monitor, PolicySettings, VideoEngine, VideoError,
};

/// The largest frame of the tests: 512x448 in RGB565.
static PIXELS: [u8; 512 * 448 * 2] = [0; 512 * 448 * 2];

/// Mega Drive H32: 256x224 at 59.922 Hz.
pub const MD_H32: SourceGeometry = source(256, 224, 59_922);
/// Mega Drive H40: 320x224 at 59.922 Hz.
pub const MD_H40: SourceGeometry = source(320, 224, 59_922);
/// SNES: 256x224 at 60.099 Hz.
pub const SNES: SourceGeometry = source(256, 224, 60_099);
/// SNES pseudo hires: 512x224 at 60.099 Hz.
pub const SNES_HIRES: SourceGeometry = source(512, 224, 60_099);
/// SNES interlaced hires: 512x448 at 60.099 Hz.
pub const SNES_448I: SourceGeometry = SourceGeometry {
    interlaced: true,
    ..source(512, 448, 60_099)
};
/// SNES at 239 lines (overscan mode).
pub const SNES_239: SourceGeometry = source(256, 239, 60_099);
/// The SNES geometry at the PAL rate: only the rate differs from `SNES`.
pub const SNES_PAL: SourceGeometry = source(256, 224, 50_007);
/// A PAL game: 256x239 at 50.007 Hz.
pub const PAL_239: SourceGeometry = source(256, 239, 50_007);

pub const fn source(width: u16, height: u16, refresh_mhz: u32) -> SourceGeometry {
    SourceGeometry {
        width,
        height,
        refresh_mhz,
        aspect_milli: 1_333,
        interlaced: false,
        rotation: Rotation::R0,
    }
}

/// A frame of `source`'s size.
pub fn frame(source: &SourceGeometry) -> Frame<'static> {
    Frame {
        data: &PIXELS,
        width: source.width,
        height: source.height,
        pitch: usize::from(source.width) * 2,
        format: PixelFormat::Rgb565,
    }
}

pub fn generic_15() -> Monitor {
    Monitor::Profile(monitor_preset("generic_15").unwrap())
}

/// The fixed super width, under which a width change needs no modeset.
pub fn fixed_width() -> PolicySettings {
    PolicySettings {
        horizontal: Some(HorizontalStrategy::FixedSuperWidth { width: 2_560 }),
        ..PolicySettings::default()
    }
}

/// The native width, under which a width change is a modeset.
pub fn native_width() -> PolicySettings {
    PolicySettings {
        horizontal: Some(HorizontalStrategy::Native),
        ..PolicySettings::default()
    }
}

pub struct Rig {
    pub fake: FakeDisplay,
    pub engine: VideoEngine<FakeDisplay>,
}

/// What one core frame did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Step {
    pub directive: Directive,
    /// The engine asked for silence after `begin_frame`.
    pub muted_before_present: bool,
    /// `set_mode` calls during `present`.
    pub mode_calls: usize,
    /// The engine still asked for silence after the flip.
    pub muted_after_flip: bool,
}

impl Rig {
    pub fn new(monitor: Monitor, settings: PolicySettings) -> Self {
        let clock = SimClock::new();
        let fake = FakeDisplay::new(clock.clone(), CAPS_LOW_CLOCK);
        let engine = VideoEngine::new(fake.clone(), monitor, settings);
        Self { fake, engine }
    }

    pub fn generic_15(settings: PolicySettings) -> Self {
        Self::new(generic_15(), settings)
    }

    /// One core frame with one flip: begin, present, wait.
    pub fn frame(&mut self, source: SourceGeometry) -> Step {
        self.try_frame(source)
            .unwrap_or_else(|e| panic!("frame {}: {e}", self.engine.odren().frame()))
    }

    pub fn try_frame(&mut self, source: SourceGeometry) -> Result<Step, VideoError> {
        let calls = self.fake.mode_calls().len();
        let directive = self.engine.begin_frame(source);
        let muted_before_present = self.engine.muted();
        assert_eq!(
            self.fake.mode_calls().len(),
            calls,
            "begin_frame makes no display call"
        );
        self.engine.present(Some(frame(&source)), None)?;
        let mode_calls = self.fake.mode_calls().len() - calls;
        self.engine.wait_flip()?;
        Ok(Step {
            directive,
            muted_before_present,
            mode_calls,
            muted_after_flip: self.engine.muted(),
        })
    }

    /// The modes the display took, in order.
    pub fn modes(&self) -> Vec<Modeline> {
        self.fake.mode_switches().iter().map(|s| s.active).collect()
    }

    /// The mode the engine set last.
    pub fn current_mode(&self) -> Modeline {
        *self.engine.current().unwrap().chosen.token.mode()
    }
}

/// The winner of `source` on a fresh engine: the mode the policy and the
/// validator give, whatever it is.
pub fn mode_for(monitor: Monitor, settings: PolicySettings, source: SourceGeometry) -> Modeline {
    let mut rig = Rig::new(monitor, settings);
    rig.frame(source);
    rig.current_mode()
}
