//! A game that fails at its start: the core refuses the content, asks to
//! stop during the load or before the game shows, or logs an error at the
//! load and asks to stop later, as FinalBurn Neo does at its own error
//! screen. The session reports each case with the words of the core, and
//! a failed launch leaves the display and the audio untouched.

use neptron_session::{EndReason, LAUNCH_STABLE_FRAMES, LaunchError};

use crate::harness::{Failed, Rig, Setup};
use crate::with_script;

/// The error line of the core in every script here.
const CORE_ERROR: &str = "HDD: game.chd not found";

fn failed(dir: &std::path::Path) -> Failed {
    match Rig::start(dir, Setup::default()) {
        Ok(_) => panic!("the launch did not fail"),
        Err(failed) => *failed,
    }
}

/// Runs frames until the session ends, at most `max`.
fn until_ended(rig: &mut Rig, max: u32) -> Option<EndReason> {
    for _ in 0..max {
        let report = rig.session.run_frame(None).expect("frame");
        if report.ended.is_some() {
            return report.ended;
        }
    }
    None
}

#[test]
fn a_refused_load_gives_the_error_lines_of_the_core() {
    let script = format!("neptron-testcore 2\nload_error {CORE_ERROR}\nload refuse\n");
    with_script!(
        "a_refused_load_gives_the_error_lines_of_the_core",
        &script,
        |dir| {
            let failed = failed(dir);
            let LaunchError::Refused { core_errors } = &failed.error else {
                panic!("a refusal: {:?}", failed.error);
            };
            assert_eq!(core_errors, &[CORE_ERROR]);
            assert!(failed.display.mode_calls().is_empty(), "no mode");
            assert_eq!(failed.audio.written_frames(), 0, "no sound");
        }
    );
}

#[test]
fn a_shutdown_in_the_load_stops_the_launch() {
    with_script!(
        "a_shutdown_in_the_load_stops_the_launch",
        "neptron-testcore 2\nload shutdown\n",
        |dir| {
            let failed = failed(dir);
            let LaunchError::CoreStopped { core_errors } = &failed.error else {
                panic!("a stop: {:?}", failed.error);
            };
            assert!(core_errors.is_empty(), "{core_errors:?}");
            assert!(failed.display.mode_calls().is_empty(), "no mode");
            assert_eq!(failed.audio.written_frames(), 0, "no sound");
        }
    );
}

#[test]
fn a_shutdown_while_the_source_settles_stops_the_launch() {
    let script = format!("neptron-testcore 2\nload_error {CORE_ERROR}\nat 2 shutdown\n");
    with_script!(
        "a_shutdown_while_the_source_settles_stops_the_launch",
        &script,
        |dir| {
            let failed = failed(dir);
            let LaunchError::CoreStopped { core_errors } = &failed.error else {
                panic!("a stop: {:?}", failed.error);
            };
            assert_eq!(core_errors, &[CORE_ERROR]);
            assert!(failed.display.mode_calls().is_empty(), "no mode");
        }
    );
}

#[test]
fn a_shutdown_in_the_first_frame_is_a_failed_start() {
    let script = format!("neptron-testcore 1\nat {LAUNCH_STABLE_FRAMES} shutdown\n");
    with_script!(
        "a_shutdown_in_the_first_frame_is_a_failed_start",
        &script,
        |dir| {
            let mut rig = Rig::launch(dir, Setup::default());
            assert_eq!(until_ended(&mut rig, 1), Some(EndReason::FailedStart));
            assert!(rig.session.start_errors().is_empty());
        }
    );
}

#[test]
fn a_shutdown_after_an_error_at_the_load_is_a_failed_start() {
    let script = format!("neptron-testcore 2\nload_error {CORE_ERROR}\nat 120 shutdown\n");
    with_script!(
        "a_shutdown_after_an_error_at_the_load_is_a_failed_start",
        &script,
        |dir| {
            let mut rig = Rig::launch(dir, Setup::default());
            assert_eq!(until_ended(&mut rig, 200), Some(EndReason::FailedStart));
            assert_eq!(rig.session.start_errors(), [CORE_ERROR]);
            assert!(rig.session.counters().flips > 100, "the game showed");
        }
    );
}
