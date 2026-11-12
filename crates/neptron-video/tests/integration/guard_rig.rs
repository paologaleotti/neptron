//! The guard and the engine on the fake display and the simulated clock.
//!
//! After every frame the rig checks the safety rule of the guard: a mode on
//! the wire passes the validator for the engine's current monitor, unless
//! the engine reports it as refused (`current_refused`), which the guard
//! then acts on.

use neptron_backend::Clock;
use neptron_platform::{EdidProfile, TimingFormula};
use neptron_safety::validate;
use neptron_testkit::preset::{CAPS_LOW_CLOCK, monitor_preset};
use neptron_testkit::{FakeDisplay, SimClock};
use neptron_types::{Modeline, SourceGeometry};
use neptron_video::{
    DisplayConfig, Guard, GuardEvent, GuardRecord, GuardStep, Monitor, PictureStyle,
    PolicySettings, Prompt, RevertReason, VideoEngine, VideoError,
};

use crate::harness::{SNES, frame, generic_15, mode_for, native_width};

/// One frame at 60 Hz, for frames with no flip to move the clock.
pub const FRAME_NS: u64 = 16_666_667;

pub fn pc_31_120() -> Monitor {
    Monitor::Profile(monitor_preset("pc_31_120").unwrap())
}

/// A multisync arcade monitor: 15, 25 and 31 kHz.
pub fn arcade_15_25_31() -> Monitor {
    Monitor::Profile(monitor_preset("arcade_15_25_31").unwrap())
}

/// A monitor that accepts no mode of the GPU: its highest pixel clock is
/// below every mode of the policy.
pub fn dead_edid() -> Monitor {
    Monitor::Edid(EdidProfile {
        profile: monitor_preset("pc_31_120").unwrap(),
        pclock_max_hz: 2_000_000,
        formula: TimingFormula::DefaultGtf,
    })
}

pub fn config(monitor: Monitor) -> DisplayConfig {
    DisplayConfig {
        monitor,
        settings: native_width(),
    }
}

pub fn crt_15() -> DisplayConfig {
    config(generic_15())
}

pub fn crt_31() -> DisplayConfig {
    config(pc_31_120())
}

/// `config` with another picture style.
pub fn with_style(config: DisplayConfig, style: PictureStyle) -> DisplayConfig {
    DisplayConfig {
        settings: PolicySettings {
            style,
            ..config.settings
        },
        ..config
    }
}

/// The mode the policy gives the test source under `config`.
pub fn mode_of(config: &DisplayConfig) -> Modeline {
    mode_for(config.monitor.clone(), config.settings, SNES)
}

/// The revert of a change the user asked for: the settings files never
/// held it, so the app writes nothing.
pub fn reverted(reason: RevertReason) -> GuardEvent {
    GuardEvent::Reverted {
        reason,
        rewrite: None,
    }
}

/// A record whose last known good is `monitor`.
pub fn confirmed(monitor: Monitor) -> GuardRecord {
    GuardRecord {
        last_known_good: Some(monitor),
        ..GuardRecord::default()
    }
}

pub struct GuardRig {
    pub clock: SimClock,
    pub fake: FakeDisplay,
    pub engine: VideoEngine<FakeDisplay>,
    pub guard: Guard,
    /// Every event, with the time it came.
    pub events: Vec<(u64, GuardEvent)>,
    /// Every record the guard asked to write, in order.
    pub saves: Vec<GuardRecord>,
    /// Errors of `present` and `wait_flip`, in order.
    pub errors: Vec<VideoError>,
    /// The source of every frame.
    pub source: SourceGeometry,
}

impl GuardRig {
    /// A boot: the engine starts on the safe default, and the guard gives
    /// it the configuration it chooses.
    pub fn boot(record: GuardRecord, wanted: DisplayConfig, safe: DisplayConfig) -> Self {
        let clock = SimClock::new();
        let fake = FakeDisplay::new(clock.clone(), CAPS_LOW_CLOCK);
        let mut engine = VideoEngine::new(fake.clone(), safe.monitor.clone(), safe.settings);
        let (guard, step) = Guard::boot(record, wanted, safe, clock.now_ns(), &mut engine);
        let mut rig = Self {
            clock,
            fake,
            engine,
            guard,
            events: Vec::new(),
            saves: Vec::new(),
            errors: Vec::new(),
            source: SNES,
        };
        rig.take(step);
        rig
    }

    /// A boot whose configuration is confirmed, after one frame on it.
    pub fn confirmed_on(wanted: DisplayConfig, safe: DisplayConfig) -> Self {
        let mut rig = Self::boot(confirmed(wanted.monitor.clone()), wanted, safe);
        rig.frame();
        rig
    }

    pub fn now(&self) -> u64 {
        self.clock.now_ns()
    }

    /// Records what a guard call asks of the app.
    pub fn take(&mut self, step: GuardStep) {
        let now = self.now();
        self.events.extend(step.event.map(|e| (now, e)));
        self.saves.extend(step.save);
    }

    /// One core frame of the test source with one flip, then the guard's
    /// tick. A frame with no flip moves the clock by one frame.
    pub fn frame(&mut self) {
        let start = self.now();
        let source = self.source;
        self.engine.begin_frame(source);
        let result = self
            .engine
            .present(Some(frame(&source)), None)
            .and_then(|_| self.engine.wait_flip().map(|_| ()));
        if let Err(error) = result {
            self.errors.push(error);
        }
        if self.now() == start {
            self.clock.advance(FRAME_NS);
        }
        self.check_wire();
        let step = self.guard.tick(self.now(), &mut self.engine);
        self.take(step);
    }

    /// Frames until the clock reaches `time_ns`.
    pub fn run_until(&mut self, time_ns: u64) {
        while self.now() < time_ns {
            self.frame();
        }
    }

    /// Frames while one more frame ends well before `time_ns`.
    pub fn run_before(&mut self, time_ns: u64) {
        while self.now() + 2 * FRAME_NS < time_ns {
            self.frame();
        }
    }

    /// Frames for `duration_ns` from now.
    pub fn run_for(&mut self, duration_ns: u64) {
        self.run_until(self.now() + duration_ns);
    }

    /// The user asks for `config`; returns the prompt it opened.
    pub fn propose(&mut self, config: DisplayConfig) -> Prompt {
        let step = self.guard.propose(config, self.now(), &mut self.engine);
        let event = step.event.clone();
        self.take(step);
        match event {
            Some(GuardEvent::PromptOpened(prompt)) => prompt,
            other => panic!("no prompt: {other:?}"),
        }
    }

    /// The user holds the blind recovery combination.
    pub fn recover(&mut self) {
        let step = self.guard.recover(&mut self.engine);
        self.take(step);
    }

    /// The mode on the wire.
    pub fn wire(&self) -> Option<Modeline> {
        self.fake.active_mode()
    }

    /// The events with no time.
    pub fn event_list(&self) -> Vec<GuardEvent> {
        self.events.iter().map(|(_, e)| e.clone()).collect()
    }

    /// The time of the first event equal to `event`.
    pub fn time_of(&self, event: GuardEvent) -> Option<u64> {
        self.events
            .iter()
            .find(|(_, e)| *e == event)
            .map(|(t, _)| *t)
    }

    fn check_wire(&self) {
        let Some(mode) = self.wire() else {
            return;
        };
        if self.engine.current_refused() {
            return;
        }
        let odren = self.engine.odren();
        assert!(
            validate(&mode, odren.monitor().profile(), odren.caps()).is_ok(),
            "frame {}: a mode the current monitor refuses is on the wire, and the engine does not \
             say so: {mode:?}",
            odren.frame()
        );
    }
}
