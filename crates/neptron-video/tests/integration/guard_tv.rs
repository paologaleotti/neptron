//! The guard on a TV output: a connector with the `TV mode` property,
//! which takes only the modes of a TV profile. A change of the standard is
//! a change of the monitor: confirm or revert, last known good and the
//! blind recovery work on it as on any monitor.

use neptron_testkit::{DisplayOptions, FakeDisplay, SimClock};
use neptron_types::{Modeline, TV_525_240P, TV_625_288P, TvStandard};
use neptron_video::{
    CONFIRM_WINDOW_NS, DisplayConfig, Guard, GuardEvent, GuardRecord, Monitor, Phase,
    PolicySettings, RevertReason, SafeReason, VideoEngine,
};

use crate::guard_rig::{FRAME_NS, reverted};
use crate::harness::{SNES, frame};
use neptron_testkit::preset::CAPS_LOW_CLOCK;

fn tv(standard: TvStandard) -> DisplayConfig {
    DisplayConfig {
        monitor: Monitor::Profile(standard.profile()),
        settings: PolicySettings::default(),
    }
}

struct TvRig {
    clock: SimClock,
    fake: FakeDisplay,
    engine: VideoEngine<FakeDisplay>,
    guard: Guard,
    events: Vec<GuardEvent>,
    saves: Vec<GuardRecord>,
}

impl TvRig {
    /// A boot on a connector that offers every standard, with NTSC as the
    /// safe default.
    fn boot(record: GuardRecord, wanted: DisplayConfig) -> Self {
        let clock = SimClock::new();
        let fake = FakeDisplay::with_options(
            clock.clone(),
            CAPS_LOW_CLOCK,
            DisplayOptions {
                tv_standards: Some(TvStandard::ALL.to_vec()),
                ..DisplayOptions::default()
            },
        );
        let safe = tv(TvStandard::Ntsc);
        let mut engine = VideoEngine::new(fake.clone(), safe.monitor.clone(), safe.settings);
        let (guard, step) = Guard::boot(record, wanted, safe, 0, &mut engine);
        let mut rig = Self {
            clock,
            fake,
            engine,
            guard,
            events: Vec::new(),
            saves: Vec::new(),
        };
        rig.take(step);
        rig.frame();
        rig
    }

    fn take(&mut self, step: neptron_video::GuardStep) {
        self.events.extend(step.event);
        self.saves.extend(step.save);
    }

    fn now(&self) -> u64 {
        neptron_backend::Clock::now_ns(&self.clock)
    }

    fn frame(&mut self) {
        let start = self.now();
        self.engine.begin_frame(SNES);
        let shown = self
            .engine
            .present(Some(frame(&SNES)), None)
            .and_then(|_| self.engine.wait_flip().map(|_| ()));
        assert!(shown.is_ok(), "{shown:?}");
        if self.now() == start {
            self.clock.advance(FRAME_NS);
        }
        let step = self.guard.tick(self.now(), &mut self.engine);
        self.take(step);
    }

    fn run_for(&mut self, duration_ns: u64) {
        let end = self.now() + duration_ns;
        while self.now() < end {
            self.frame();
        }
    }

    fn wire(&self) -> (Modeline, TvStandard) {
        (
            self.fake.active_mode().expect("a mode"),
            self.fake.active_tv_standard().expect("a standard"),
        )
    }

    fn assert_only_tv_modes(&self) {
        for call in self.fake.mode_calls() {
            let standard = call.tv_standard.expect("a standard");
            assert!(standard.modes().contains(&call.requested), "{call:?}");
            assert!(call.result.is_ok(), "{call:?}");
        }
    }
}

fn confirmed(standard: TvStandard) -> GuardRecord {
    GuardRecord {
        last_known_good: Some(tv(standard).monitor),
        ..GuardRecord::default()
    }
}

#[test]
fn a_new_standard_opens_the_prompt_and_the_timeout_reverts_it() {
    let mut rig = TvRig::boot(confirmed(TvStandard::Ntsc), tv(TvStandard::Ntsc));
    assert_eq!(rig.guard.phase(), Phase::Confirmed);
    assert_eq!(rig.wire(), (TV_525_240P, TvStandard::Ntsc));
    let step = rig
        .guard
        .propose(tv(TvStandard::Pal), rig.now(), &mut rig.engine);
    assert!(matches!(step.event, Some(GuardEvent::PromptOpened(_))));
    rig.take(step);
    rig.frame();
    assert_eq!(rig.wire(), (TV_625_288P, TvStandard::Pal));
    rig.run_for(CONFIRM_WINDOW_NS + 2 * FRAME_NS);
    assert!(rig.events.contains(&reverted(RevertReason::Timeout)));
    assert_eq!(rig.wire(), (TV_525_240P, TvStandard::Ntsc));
    rig.assert_only_tv_modes();
}

#[test]
fn a_standard_alone_is_a_modeset_with_the_same_lines() {
    let mut rig = TvRig::boot(confirmed(TvStandard::Ntsc), tv(TvStandard::Ntsc));
    let before = rig.fake.mode_calls().len();
    let step = rig
        .guard
        .propose(tv(TvStandard::Pal60), rig.now(), &mut rig.engine);
    rig.take(step);
    rig.frame();
    assert_eq!(rig.wire(), (TV_525_240P, TvStandard::Pal60));
    assert_eq!(rig.fake.mode_calls().len(), before + 1, "one modeset");
}

#[test]
fn a_kept_standard_is_the_last_known_good() {
    let mut rig = TvRig::boot(confirmed(TvStandard::Ntsc), tv(TvStandard::Ntsc));
    let step = rig
        .guard
        .propose(tv(TvStandard::Pal), rig.now(), &mut rig.engine);
    let Some(GuardEvent::PromptOpened(prompt)) = step.event.clone() else {
        panic!("no prompt: {step:?}");
    };
    rig.take(step);
    rig.run_for(2 * FRAME_NS);
    let step = rig
        .guard
        .confirm(prompt.id, rig.now(), &rig.engine)
        .expect("kept");
    rig.take(step);
    assert_eq!(rig.guard.phase(), Phase::Confirmed);
    assert_eq!(
        rig.saves.last().and_then(|r| r.last_known_good.clone()),
        Some(tv(TvStandard::Pal).monitor)
    );
    rig.run_for(CONFIRM_WINDOW_NS + 2 * FRAME_NS);
    assert_eq!(rig.wire(), (TV_625_288P, TvStandard::Pal));
    rig.assert_only_tv_modes();
}

#[test]
fn the_blind_recovery_on_a_tv_gives_ntsc() {
    let mut rig = TvRig::boot(confirmed(TvStandard::Pal), tv(TvStandard::Pal));
    assert_eq!(rig.wire(), (TV_625_288P, TvStandard::Pal));
    let step = rig.guard.recover(&mut rig.engine);
    rig.take(step);
    rig.frame();
    assert_eq!(rig.guard.phase(), Phase::Safe(SafeReason::Recovery));
    assert_eq!(rig.wire(), (TV_525_240P, TvStandard::Ntsc));
    rig.assert_only_tv_modes();
}
