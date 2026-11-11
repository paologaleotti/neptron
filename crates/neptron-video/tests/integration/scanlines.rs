//! Scanlines in the scene: the settings, the mode and the scale decide
//! them, through `neptron_types::scanlines_for`.

use std::cell::RefCell;
use std::rc::Rc;

use neptron_backend::{Display, DisplayError};
use neptron_safety::ValidatedMode;
use neptron_testkit::preset::{CAPS_LOW_CLOCK, monitor_preset};
use neptron_testkit::{FakeDisplay, SimClock};
use neptron_types::{
    FlipEvent, GpuCaps, Modeline, ScanlineKind, ScanlineMode, Scanlines, Scene, SourceGeometry,
};
use neptron_video::{
    Directive, Monitor, PictureStyle, PolicySettings, ScanlineSettings, VideoEngine,
};

use crate::harness::{SNES, frame, generic_15, native_width};

/// The fake display, and the scanlines of every scene it gets.
struct Recording {
    inner: FakeDisplay,
    scanlines: Rc<RefCell<Vec<Scanlines>>>,
}

impl Display for Recording {
    fn caps(&self) -> &GpuCaps {
        self.inner.caps()
    }

    fn set_mode(&mut self, mode: &ValidatedMode) -> Result<Modeline, DisplayError> {
        self.inner.set_mode(mode)
    }

    fn present(&mut self, scene: &Scene<'_>) -> Result<(), DisplayError> {
        self.scanlines.borrow_mut().push(scene.scanlines);
        self.inner.present(scene)
    }

    fn wait_flip(&mut self) -> Result<FlipEvent, DisplayError> {
        self.inner.wait_flip()
    }
}

struct Rig {
    engine: VideoEngine<Recording>,
    fake: FakeDisplay,
    scanlines: Rc<RefCell<Vec<Scanlines>>>,
}

impl Rig {
    fn new(monitor: Monitor, settings: PolicySettings) -> Self {
        let fake = FakeDisplay::new(SimClock::new(), CAPS_LOW_CLOCK);
        let scanlines = Rc::new(RefCell::new(Vec::new()));
        let display = Recording {
            inner: fake.clone(),
            scanlines: Rc::clone(&scanlines),
        };
        Self {
            engine: VideoEngine::new(display, monitor, settings),
            fake,
            scanlines,
        }
    }

    /// One core frame; the scanlines of its scene.
    fn frame(&mut self, source: SourceGeometry) -> (Directive, Scanlines) {
        let directive = self.engine.begin_frame(source);
        self.engine.present(Some(frame(&source)), None).unwrap();
        self.engine.wait_flip().unwrap();
        (directive, *self.scanlines.borrow().last().unwrap())
    }

    /// The vertical multiple of the placement on screen.
    fn vertical_multiple(&self) -> Option<u16> {
        self.engine
            .current()
            .unwrap()
            .chosen
            .proposal
            .placement
            .scale
            .y
    }
}

fn pc_31_120() -> Monitor {
    Monitor::Profile(monitor_preset("pc_31_120").unwrap())
}

fn x2() -> PolicySettings {
    PolicySettings {
        style: PictureStyle::X2,
        ..native_width()
    }
}

fn set(mode: ScanlineMode, strength_percent: u8) -> ScanlineSettings {
    ScanlineSettings {
        mode,
        strength_percent,
    }
}

fn black(strength_percent: u8) -> Scanlines {
    Scanlines {
        kind: ScanlineKind::Black,
        strength_percent,
    }
}

#[test]
fn auto_draws_black_lines_on_a_line_doubled_31khz_mode() {
    let mut rig = Rig::new(pc_31_120(), x2());
    rig.engine.set_scanlines(set(ScanlineMode::Auto, 60));
    let (_, scanlines) = rig.frame(SNES);
    assert_eq!(
        rig.vertical_multiple(),
        Some(2),
        "the 2x style doubles the lines"
    );
    assert!(!rig.engine.current().unwrap().read_back.interlace);
    assert_eq!(scanlines, black(60));
}

#[test]
fn auto_draws_none_on_a_15khz_native_mode() {
    let mut rig = Rig::new(generic_15(), native_width());
    rig.engine.set_scanlines(set(ScanlineMode::Auto, 100));
    assert_eq!(rig.frame(SNES).1, Scanlines::NONE);
}

#[test]
fn off_draws_none() {
    let mut rig = Rig::new(pc_31_120(), x2());
    rig.engine.set_scanlines(set(ScanlineMode::Off, 100));
    assert_eq!(rig.frame(SNES).1, Scanlines::NONE);
}

#[test]
fn the_engine_draws_none_until_the_session_passes_the_settings() {
    let mut rig = Rig::new(pc_31_120(), x2());
    assert_eq!(rig.frame(SNES).1, Scanlines::NONE);
}

#[test]
fn a_scanline_change_takes_effect_on_the_next_frame_with_no_modeset() {
    let mut rig = Rig::new(pc_31_120(), x2());
    rig.frame(SNES);
    let calls = rig.fake.mode_calls().len();
    rig.engine.set_scanlines(set(ScanlineMode::On, 100));
    assert_eq!(rig.frame(SNES), (Directive::Steady, black(100)));
    rig.engine.set_scanlines(set(ScanlineMode::On, 25));
    assert_eq!(rig.frame(SNES), (Directive::Steady, black(25)));
    assert_eq!(rig.fake.mode_calls().len(), calls);
    assert!(!rig.engine.muted());
}

#[test]
fn the_preview_of_a_style_row_takes_the_same_scanlines() {
    let mut rig = Rig::new(pc_31_120(), native_width());
    rig.engine.set_scanlines(set(ScanlineMode::Auto, 75));
    rig.frame(SNES);
    let rows = rig.engine.style_rows(&SNES).unwrap();
    let row = rows.iter().find(|r| r.style == PictureStyle::X2).unwrap();
    let proposal = row.plan.considered[row.winner.unwrap()].outcome.unwrap();
    let preview = rig
        .engine
        .preview_scene(&proposal, &SNES, Some(frame(&SNES)), None);
    let mut run = Rig::new(pc_31_120(), x2());
    run.engine.set_scanlines(set(ScanlineMode::Auto, 75));
    let (_, scanlines) = run.frame(SNES);
    assert_eq!(preview.scanlines, scanlines);
    assert_eq!(
        preview.dest,
        run.engine.current().unwrap().chosen.proposal.placement.dest
    );
}
