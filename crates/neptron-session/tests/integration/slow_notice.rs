//! The speed that the session reports for the notice of a game that the
//! machine cannot keep at full speed, and the sound that is off below half
//! speed.

use neptron_session::LATE_HOLD_FRAMES;

use crate::harness::{Rig, Setup};
use crate::pacing::{RUN_FRAMES, SCRIPT, interval_ns, run_with_cost};
use crate::with_script;

/// A core at `permille` of the interval reports `100 / x` percent within
/// 1 below 98%, else nothing; the sound is off only below half speed.
fn reports(test: &str, permille: u64) {
    with_script!(test, SCRIPT, |dir| {
        let mut rig = Rig::launch(dir, Setup::default());
        rig.run(30);
        run_with_cost(&mut rig, permille, 0, RUN_FRAMES);
        let speed = 100_000 / permille;
        match rig.session.slow_speed_percent() {
            Some(percent) => {
                assert!(speed < 98, "{percent}% at {permille}");
                assert!(u64::from(percent).abs_diff(speed) <= 1, "{percent}%");
            }
            None => assert!(speed >= 98, "none at {permille}"),
        }
        assert_eq!(rig.session.sound_too_slow(), speed < 50);
    });
}

#[test]
fn a_fast_core_reports_no_speed() {
    reports("a_fast_core_reports_no_speed", 900);
}

#[test]
fn a_core_two_percent_too_slow_reports_no_speed() {
    reports("a_core_two_percent_too_slow_reports_no_speed", 1_020);
}

#[test]
fn a_core_four_percent_too_slow_reports_96_percent() {
    reports("a_core_four_percent_too_slow_reports_96_percent", 1_040);
}

#[test]
fn a_core_at_1_3_intervals_reports_76_percent() {
    reports("a_core_at_1_3_intervals_reports_76_percent", 1_300);
}

#[test]
fn a_core_at_2_5_intervals_reports_40_percent_and_no_sound() {
    reports(
        "a_core_at_2_5_intervals_reports_40_percent_and_no_sound",
        2_500,
    );
}

#[test]
fn a_core_that_jitters_at_the_limit_reports_no_speed() {
    with_script!(
        "a_core_that_jitters_at_the_limit_reports_no_speed",
        SCRIPT,
        |dir| {
            let mut rig = Rig::launch(dir, Setup::default());
            rig.run(30);
            run_with_cost(&mut rig, 1_000, 50, RUN_FRAMES);
            assert_eq!(rig.session.slow_speed_percent(), None, "above 98%");
        }
    );
}

/// One long frame of a fast core never gives a speed.
#[test]
fn a_single_late_frame_reports_no_speed() {
    with_script!("a_single_late_frame_reports_no_speed", SCRIPT, |dir| {
        let mut rig = Rig::launch(dir, Setup::default());
        rig.run(30);
        let interval = interval_ns(&rig, 1);
        rig.cost.set(interval * 3 / 2, 0, 0);
        rig.run(1);
        rig.cost.set(0, 0, 0);
        for _ in 0..4 * LATE_HOLD_FRAMES {
            rig.session.run_frame(None).unwrap();
            assert_eq!(rig.session.slow_speed_percent(), None);
        }
    });
}
