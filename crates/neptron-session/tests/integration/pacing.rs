//! The pacing of the loop: the input delay of a fast core, and a core that
//! needs more than one flip interval. A [`FrameCost`] gives each frame its
//! time on the simulated clock.
//!
//! [`FrameCost`]: crate::harness::FrameCost

use neptron_backend::Clock;
use neptron_session::{FLIPLESS_FRAMES_LIMIT, LATE_HOLD_FRAMES, SessionError};
use neptron_testkit::preset::CAPS_LOW_CLOCK;
use neptron_testkit::{DEFAULT_FLIP_TIMEOUT_NS, FlipFault};
use neptron_types::Control;
use neptron_video::{Outcome, PictureStyle, PolicySettings, RefreshPolicy};

use crate::harness::{Rig, Setup};
use crate::with_script;

pub const SCRIPT: &str = "\
neptron-testcore 1
pixel_format rgb565
geometry 128 112 1333
timing 60000 48000
";

/// Frames of each run with a cost.
pub const RUN_FRAMES: u64 = 600;
/// Frames at the start of a run that the rate does not count: the entry
/// into late pacing.
const ENTRY_FRAMES: usize = 20;
/// The largest error of a presented rate against `1 / x`.
const RATE_TOLERANCE: f64 = 0.005;

/// The flip interval of the mode on screen, in nanoseconds, as the
/// session computes it: the blanks of one flip over the field rate.
pub fn interval_ns(rig: &Rig, blanks_per_flip: u64) -> u64 {
    let rate = rig.display.active_mode().expect("a mode").field_rate_mhz();
    blanks_per_flip * 1_000_000_000_000 / rate
}

/// What a run with a cost showed.
#[derive(Debug)]
pub struct Measure {
    /// Flips per blank after the entry, from the flip events.
    pub presented_rate: f64,
    /// Changes from wait pacing to late pacing, and back.
    pub entries: u32,
    pub exits: u32,
    /// The last frame left its flip queued.
    pub ends_deferred: bool,
    /// The input delay two frames after each input change, in
    /// nanoseconds.
    pub delays: Vec<u64>,
}

/// Runs [`RUN_FRAMES`] frames with each frame's work at `permille` of the
/// interval, plus or minus `jitter_permille`, and an input change every 25
/// frames.
pub fn run_with_cost(rig: &mut Rig, permille: u64, jitter_permille: u64, frames: u64) -> Measure {
    let interval = interval_ns(rig, 1);
    rig.cost.set(
        interval * permille / 1_000,
        interval * jitter_permille / 1_000,
        0x5EED,
    );
    let flips_before = rig.display.flips().len();
    let mut late = false;
    let (mut entries, mut exits) = (0, 0);
    let mut delays = Vec::new();
    let mut held = false;
    for frame in 0..frames {
        if frame % 25 == 0 {
            if held {
                rig.release(&[Control::A]);
            } else {
                rig.press(&[Control::A]);
            }
            held = !held;
        }
        let report = rig.session.run_frame(None).expect("frame");
        assert!(report.display_error.is_none(), "{report:?}");
        if report.deferred != late {
            if report.deferred {
                entries += 1;
            } else {
                exits += 1;
            }
            late = report.deferred;
        }
        if frame % 25 == 2 {
            delays.push(rig.session.counters().input_to_flip_ns);
        }
    }
    let flips = rig.display.flips();
    let counted = &flips[flips_before + ENTRY_FRAMES..];
    let first = counted.first().expect("flips").sequence;
    let last = counted.last().expect("flips").sequence;
    Measure {
        presented_rate: (counted.len() - 1) as f64 / (last - first) as f64,
        entries,
        exits,
        ends_deferred: late,
        delays,
    }
}

/// The checks of every run: each core frame is shown, once, and the
/// session counts each repeated blank as a missed field.
fn assert_every_frame_shown(rig: &Rig, measure: &Measure) {
    let counters = *rig.session.counters();
    assert_eq!(counters.flipless_frames, 0, "no frame dropped");
    assert_eq!(
        rig.display.dropped_frames().len(),
        usize::from(measure.ends_deferred),
        "only the flip of the last frame can still be queued"
    );
    assert_eq!(counters.missed_fields, rig.display.repeated_vblanks());
    let hashes = rig.display.frame_hashes();
    assert!(
        hashes.windows(2).all(|pair| pair[0] != pair[1]),
        "each flip shows a new frame"
    );
}

/// The core of a fast game runs at the flip event and its frame flips at
/// the next blank: the delay from the poll to the flip is one interval.
#[test]
fn the_input_delay_of_a_fast_core_is_one_flip_interval() {
    with_script!(
        "the_input_delay_of_a_fast_core_is_one_flip_interval",
        SCRIPT,
        |dir| {
            let mut rig = Rig::launch(dir, Setup::default());
            rig.run(30);
            assert_eq!(rig.session.counters().input_to_flip_ns, 0, "no input yet");
            for change in [Rig::press, Rig::release] {
                change(&rig, &[Control::A]);
                rig.run(1);
                let flips = rig.display.flip_times();
                let interval = flips[flips.len() - 1] - flips[flips.len() - 2];
                assert_eq!(rig.session.counters().input_to_flip_ns, interval);
                rig.run(10);
                assert_eq!(
                    rig.session.counters().input_to_flip_ns,
                    interval,
                    "frames with no input keep the last delay"
                );
            }
        }
    );
}

/// A core that fits in the interval stays in wait pacing: every frame
/// flips at the next blank, one interval after its poll, as with no cost.
fn fits(test: &str, permille: u64) {
    with_script!(test, SCRIPT, |dir| {
        let mut rig = Rig::launch(dir, Setup::default());
        rig.run(30);
        let measure = run_with_cost(&mut rig, permille, 0, RUN_FRAMES);
        assert_every_frame_shown(&rig, &measure);
        assert_eq!((measure.entries, measure.exits), (0, 0), "{measure:?}");
        assert_eq!(rig.session.counters().late_frames, 0);
        assert_eq!(rig.session.counters().missed_fields, 0);
        assert_eq!(measure.presented_rate, 1.0);
        let interval = interval_ns(&rig, 1);
        assert!(!measure.delays.is_empty());
        for delay in &measure.delays {
            assert!(delay.abs_diff(interval) <= 1, "delay {delay} ns");
        }
    });
}

#[test]
fn a_core_at_half_the_interval_waits_for_each_flip() {
    fits("a_core_at_half_the_interval_waits_for_each_flip", 500);
}

#[test]
fn a_core_at_nine_tenths_of_the_interval_waits_for_each_flip() {
    fits(
        "a_core_at_nine_tenths_of_the_interval_waits_for_each_flip",
        900,
    );
}

#[test]
fn a_core_at_0_95_of_the_interval_waits_for_each_flip() {
    fits("a_core_at_0_95_of_the_interval_waits_for_each_flip", 950);
}

#[test]
fn a_core_at_0_99_of_the_interval_waits_for_each_flip() {
    fits("a_core_at_0_99_of_the_interval_waits_for_each_flip", 990);
}

/// A core that needs more than one interval goes to late pacing at its
/// first frame and stays there. The presented rate is `1 / x` of the flip
/// rate, the input delay is `x` to `x + 1` intervals, and no frame is
/// dropped.
fn late(test: &str, permille: u64) {
    with_script!(test, SCRIPT, |dir| {
        let mut rig = Rig::launch(dir, Setup::default());
        rig.run(30);
        let measure = run_with_cost(&mut rig, permille, 0, RUN_FRAMES);
        assert_every_frame_shown(&rig, &measure);
        assert_eq!((measure.entries, measure.exits), (1, 0), "{measure:?}");
        assert_eq!(rig.session.counters().late_frames, RUN_FRAMES);
        let expected = 1_000.0 / permille as f64;
        assert!(
            (measure.presented_rate - expected).abs() <= RATE_TOLERANCE,
            "presented rate {} for {expected}",
            measure.presented_rate
        );
        let interval = interval_ns(&rig, 1);
        let low = interval * permille / 1_000;
        let high = low + interval;
        assert!(measure.delays.len() > 10);
        for delay in &measure.delays[1..] {
            assert!((low..=high).contains(delay), "delay {delay} ns");
        }
    });
}

#[test]
fn a_core_two_percent_too_slow_loses_two_percent() {
    late("a_core_two_percent_too_slow_loses_two_percent", 1_020);
}

#[test]
fn a_core_four_percent_too_slow_loses_four_percent() {
    late("a_core_four_percent_too_slow_loses_four_percent", 1_040);
}

#[test]
fn a_core_at_one_point_three_intervals_runs_at_its_own_rate() {
    late(
        "a_core_at_one_point_three_intervals_runs_at_its_own_rate",
        1_300,
    );
}

#[test]
fn a_core_at_two_intervals_runs_at_half_rate() {
    late("a_core_at_two_intervals_runs_at_half_rate", 2_000);
}

#[test]
fn a_core_at_two_and_a_half_intervals_runs_at_two_fifths() {
    late(
        "a_core_at_two_and_a_half_intervals_runs_at_two_fifths",
        2_500,
    );
}

/// A cost that jitters around one interval enters late pacing once and
/// never goes back: no hold of frames that all fit comes.
#[test]
fn a_core_that_jitters_at_the_limit_stays_in_late_pacing() {
    with_script!(
        "a_core_that_jitters_at_the_limit_stays_in_late_pacing",
        SCRIPT,
        |dir| {
            let mut rig = Rig::launch(dir, Setup::default());
            rig.run(30);
            let measure = run_with_cost(&mut rig, 1_000, 50, RUN_FRAMES);
            assert_every_frame_shown(&rig, &measure);
            assert_eq!((measure.entries, measure.exits), (1, 0), "{measure:?}");
            assert!(measure.presented_rate >= 0.96, "{measure:?}");
        }
    );
}

/// One late frame of a fast core: late pacing for the late frame and
/// [`LATE_HOLD_FRAMES`] frames that fit, one missed field, then wait
/// pacing with an input delay of one interval again.
#[test]
fn a_single_late_frame_returns_to_wait_pacing() {
    with_script!(
        "a_single_late_frame_returns_to_wait_pacing",
        SCRIPT,
        |dir| {
            let mut rig = Rig::launch(dir, Setup::default());
            rig.run(30);
            let interval = interval_ns(&rig, 1);
            rig.cost.set(interval * 3 / 2, 0, 0);
            assert!(rig.session.run_frame(None).unwrap().deferred);
            rig.cost.set(0, 0, 0);
            let mut deferred = 1;
            for _ in 0..4 * LATE_HOLD_FRAMES {
                deferred += u32::from(rig.session.run_frame(None).unwrap().deferred);
            }
            assert_eq!(deferred, LATE_HOLD_FRAMES + 1);
            assert_eq!(rig.session.counters().missed_fields, 1);
            assert_eq!(
                rig.session.counters().late_frames,
                u64::from(LATE_HOLD_FRAMES + 1)
            );
            rig.press(&[Control::A]);
            rig.run(1);
            let flips = rig.display.flip_times();
            let last = flips[flips.len() - 1] - flips[flips.len() - 2];
            assert_eq!(rig.session.counters().input_to_flip_ns, last);
            assert!(rig.display.dropped_frames().is_empty());
        }
    );
}

/// The quick menu opens in late pacing: the frame that opens it waits for
/// the flip before and for its own, the menu runs in wait pacing, and the
/// game goes back to late pacing after the resume.
#[test]
fn the_quick_menu_waits_for_every_flip() {
    with_script!("the_quick_menu_waits_for_every_flip", SCRIPT, |dir| {
        let mut rig = Rig::launch(dir, Setup::default());
        rig.run(30);
        let interval = interval_ns(&rig, 1);
        rig.cost.set(interval * 13 / 10, 0, 0);
        rig.run(60);
        assert!(rig.session.counters().late_frames > 0);
        rig.open_menu();
        assert!(rig.session.paused());
        let missed = rig.session.counters().missed_fields;
        for _ in 0..30 {
            let report = rig.session.run_frame(None).unwrap();
            assert!(!report.deferred);
            assert_eq!(report.flips, 1);
        }
        assert!(rig.display.dropped_frames().is_empty(), "no flip queued");
        assert_eq!(rig.session.counters().missed_fields, missed);
        rig.session.resume();
        let report = rig.session.run_frame(None).unwrap();
        assert!(report.deferred, "the late core is late again");
        assert_eq!(rig.session.counters().flipless_frames, 0);
    });
}

/// A new core rate in late pacing: the frame of the modeset waits for the
/// flip before, so the display takes the mode, and waits for its own.
#[test]
fn a_mode_change_in_late_pacing_waits_for_the_flip_before() {
    let script = format!("{SCRIPT}at 200 av_info 128 112 1333 320 240 50007 48000\n");
    with_script!(
        "a_mode_change_in_late_pacing_waits_for_the_flip_before",
        &script,
        |dir| {
            let mut rig = Rig::launch(dir, Setup::default());
            rig.run(30);
            let interval = interval_ns(&rig, 1);
            rig.cost.set(interval * 13 / 10, 0, 0);
            let mut modesets = 0;
            let mut late_after = 0;
            for _ in 0..400 {
                let report = rig.session.run_frame(None).unwrap();
                assert!(report.display_error.is_none(), "{report:?}");
                if let Some(outcome) = &report.modeset {
                    assert_eq!(*outcome, Outcome::Modeset);
                    assert!(!report.deferred, "the modeset frame waits");
                    modesets += 1;
                } else if modesets > 0 && report.deferred {
                    late_after += 1;
                }
            }
            assert_eq!(modesets, 1);
            assert!(late_after > 0, "late pacing again in the new mode");
            assert_eq!(rig.display.mode_calls().len(), 2);
            assert!(rig.display.mode_calls().iter().all(|m| m.result.is_ok()));
            assert_eq!(rig.session.counters().flipless_frames, 0);
        }
    );
}

/// An interlaced mode on a driver that flips once per frame: the interval
/// is two fields, every flip lands on a blank that starts a frame, so the
/// field order stays, and the rate is `1 / x` of the frame rate.
#[test]
fn an_interlaced_mode_keeps_its_field_order_in_late_pacing() {
    let script = "\
neptron-testcore 1
pixel_format rgb565
geometry 256 448 1333
timing 60000 48000
";
    with_script!(
        "an_interlaced_mode_keeps_its_field_order_in_late_pacing",
        script,
        |dir| {
            let mut rig = Rig::launch(dir, Setup::default());
            const { assert!(!CAPS_LOW_CLOCK.flip_per_field) };
            rig.run(30);
            assert!(rig.display.active_mode().unwrap().interlace);
            let frame_ns = interval_ns(&rig, 2);
            rig.cost.set(frame_ns * 13 / 10, 0, 0);
            let flips_before = rig.display.flips().len();
            rig.run(RUN_FRAMES);
            assert!(rig.session.counters().late_frames > 0);
            let flips = rig.display.flips();
            let parity = flips[0].sequence % 2;
            assert!(
                flips.iter().all(|f| f.sequence % 2 == parity),
                "field order"
            );
            let counted = &flips[flips_before + ENTRY_FRAMES..];
            let span = counted.last().unwrap().sequence - counted[0].sequence;
            let rate = 2.0 * (counted.len() - 1) as f64 / span as f64;
            assert!((rate - 1.0 / 1.3).abs() <= RATE_TOLERANCE, "rate {rate}");
            assert_eq!(
                rig.session.counters().missed_fields,
                rig.display.repeated_vblanks()
            );
            assert_eq!(rig.session.counters().flipless_frames, 0);
        }
    );
}

/// Double refresh with a core at 1.3 fields: the first flip of each core
/// frame is waited for, the second is left queued, and the core keeps its
/// own rate of one frame per two fields. Wait pacing alone gives one frame
/// per three fields.
#[test]
fn double_refresh_in_late_pacing_keeps_the_game_rate() {
    let script = "\
neptron-testcore 1
pixel_format rgb565
geometry 256 224 1333
max_size 320 240
timing 60099 32040
";
    with_script!(
        "double_refresh_in_late_pacing_keeps_the_game_rate",
        script,
        |dir| {
            let setup = Setup {
                monitor: "pc_31_120",
                policy: PolicySettings {
                    style: PictureStyle::Native,
                    refresh: RefreshPolicy::ExactX2,
                    ..PolicySettings::default()
                },
                ..Setup::default()
            };
            let mut rig = Rig::launch(dir, setup);
            rig.run(30);
            let field_ns = interval_ns(&rig, 1);
            rig.cost.set(field_ns * 13 / 10, 0, 0);
            let before = *rig.session.counters();
            let flips_before = rig.display.flips().len();
            for frame in 0..RUN_FRAMES {
                let report = rig.session.run_frame(None).unwrap();
                assert!(report.deferred);
                let flips = if frame == 0 { 1 } else { 2 };
                assert_eq!(report.flips, flips, "the flip before and the first flip");
            }
            let counters = *rig.session.counters();
            assert_eq!(counters.core_frames - before.core_frames, RUN_FRAMES);
            assert_eq!(counters.flipless_frames, 0);
            let flips = rig.display.flips();
            let counted = &flips[flips_before + 2 * ENTRY_FRAMES..];
            let span = counted.last().unwrap().sequence - counted[0].sequence;
            assert_eq!(span, counted.len() as u64 - 1, "a flip on every field");
            let hashes = rig.display.frame_hashes();
            let paired = hashes.len() - hashes.len() % 2;
            assert!(
                hashes[paired - 40..paired].chunks(2).all(|p| p[0] == p[1]),
                "each core frame shows on two flips"
            );
        }
    );
}

/// A display that stops flipping in late pacing: the wait for the queued
/// flip is bounded, each frame after it is flipless, and the session ends
/// after [`FLIPLESS_FRAMES_LIMIT`] frames, as in wait pacing.
#[test]
fn a_display_that_stops_flipping_in_late_pacing_ends_the_session() {
    with_script!(
        "a_display_that_stops_flipping_in_late_pacing_ends_the_session",
        SCRIPT,
        |dir| {
            let mut rig = Rig::launch(dir, Setup::default());
            rig.run(30);
            let interval = interval_ns(&rig, 1);
            let cost = interval * 13 / 10;
            rig.cost.set(cost, 0, 0);
            rig.run(30);
            let first = rig.display.next_present_index();
            for index in first..first + 100 {
                rig.display.fault_flip(index, FlipFault::Lost);
            }
            assert!(rig.session.run_frame(None).unwrap().deferred);
            let start_ns = rig.clock.now_ns();
            let mut frames = 0;
            let error = loop {
                frames += 1;
                match rig.session.run_frame(None) {
                    Ok(report) => assert_eq!(report.flips, 0),
                    Err(error) => break error,
                }
                assert!(frames <= FLIPLESS_FRAMES_LIMIT, "no end");
            };
            assert_eq!(frames, FLIPLESS_FRAMES_LIMIT);
            assert!(matches!(error, SessionError::DisplayStalled { .. }));
            let waited = rig.clock.now_ns() - start_ns;
            let bound = u64::from(FLIPLESS_FRAMES_LIMIT) * (DEFAULT_FLIP_TIMEOUT_NS + cost);
            assert!(waited <= bound, "{waited} ns");
        }
    );
}

/// The frame in which the core asks to stop waits for its flip: no flip
/// stays queued after the last frame.
#[test]
fn the_last_frame_of_a_late_core_waits_for_its_flip() {
    let script = format!("{SCRIPT}at 100 shutdown\n");
    with_script!(
        "the_last_frame_of_a_late_core_waits_for_its_flip",
        &script,
        |dir| {
            let mut rig = Rig::launch(dir, Setup::default());
            rig.run(10);
            let interval = interval_ns(&rig, 1);
            rig.cost.set(interval * 13 / 10, 0, 0);
            let report = loop {
                let report = rig.session.run_frame(None).unwrap();
                if report.ended.is_some() {
                    break report;
                }
                assert!(report.deferred);
            };
            assert!(!report.deferred);
            assert!(rig.display.dropped_frames().is_empty(), "no flip queued");
        }
    );
}
