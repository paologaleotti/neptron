//! A wait that times out while the flip is still in flight, as on a real
//! backend with a bounded wait.

use std::cell::Cell;
use std::rc::Rc;

use neptron_backend::{Display, DisplayError};
use neptron_safety::ValidatedMode;
use neptron_testkit::preset::CAPS_LOW_CLOCK;
use neptron_testkit::{FakeDisplay, SimClock};
use neptron_types::{FlipEvent, GpuCaps, Modeline, Scene};
use neptron_video::{Directive, Outcome, VideoEngine, VideoError};

use crate::harness::{MD_H32, MD_H40, frame, generic_15, native_width};

/// The fake display, with waits that fail with a timeout and leave the
/// flip queued.
struct Stalling {
    inner: FakeDisplay,
    stalls: Rc<Cell<u32>>,
}

impl Display for Stalling {
    fn caps(&self) -> &GpuCaps {
        self.inner.caps()
    }

    fn set_mode(&mut self, mode: &ValidatedMode) -> Result<Modeline, DisplayError> {
        self.inner.set_mode(mode)
    }

    fn present(&mut self, scene: &Scene<'_>) -> Result<(), DisplayError> {
        self.inner.present(scene)
    }

    fn wait_flip(&mut self) -> Result<FlipEvent, DisplayError> {
        if self.stalls.get() > 0 {
            self.stalls.set(self.stalls.get() - 1);
            return Err(DisplayError::Device {
                operation: "wait_flip",
                reason: "timeout".to_owned(),
            });
        }
        self.inner.wait_flip()
    }
}

fn engine() -> (VideoEngine<Stalling>, FakeDisplay, Rc<Cell<u32>>) {
    let fake = FakeDisplay::new(SimClock::new(), CAPS_LOW_CLOCK);
    let stalls = Rc::new(Cell::new(0));
    let display = Stalling {
        inner: fake.clone(),
        stalls: Rc::clone(&stalls),
    };
    (
        VideoEngine::new(display, generic_15(), native_width()),
        fake,
        stalls,
    )
}

#[test]
fn a_timed_out_wait_keeps_the_flip_queued_and_the_next_wait_gets_it() {
    let (mut engine, fake, stalls) = engine();
    engine.begin_frame(MD_H40);
    engine.present(Some(frame(&MD_H40)), None).unwrap();
    stalls.set(1);
    assert!(matches!(engine.wait_flip(), Err(VideoError::WaitFlip(_))));
    let event = engine.wait_flip().unwrap();
    assert_eq!(fake.flips().last(), Some(&event));
}

#[test]
fn a_timed_out_wait_before_a_modeset_sets_no_mode_and_blocks_nothing() {
    let (mut engine, fake, stalls) = engine();
    engine.begin_frame(MD_H40);
    engine.present(Some(frame(&MD_H40)), None).unwrap();
    engine.wait_flip().unwrap();
    engine.begin_frame(MD_H32);
    engine.begin_frame(MD_H32);
    engine.present(Some(frame(&MD_H32)), None).unwrap();
    stalls.set(1);
    assert_eq!(engine.begin_frame(MD_H32), Directive::Modeset);
    assert!(matches!(
        engine.present(Some(frame(&MD_H32)), None),
        Err(VideoError::PendingFlip(_))
    ));
    assert_eq!(
        fake.mode_calls().len(),
        1,
        "no set_mode while the flip is queued"
    );
    assert_eq!(engine.begin_frame(MD_H32), Directive::Modeset);
    let presented = engine.present(Some(frame(&MD_H32)), None).unwrap();
    assert!(presented.drained_flip.is_some());
    assert_eq!(presented.modeset, Some(Outcome::Modeset));
    engine.wait_flip().unwrap();
    assert!(fake.mode_calls().iter().all(|c| c.result.is_ok()));
    assert_eq!(fake.mode_switches().len(), 2);
}

#[test]
fn a_flip_whose_wait_fails_twice_counts_as_lost() {
    let (mut engine, _fake, stalls) = engine();
    engine.begin_frame(MD_H40);
    engine.present(Some(frame(&MD_H40)), None).unwrap();
    stalls.set(2);
    assert!(engine.wait_flip().is_err());
    assert!(engine.wait_flip().is_err());
    assert_eq!(engine.wait_flip(), Err(VideoError::NoFlipPending));
}
