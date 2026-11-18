//! A game that fails at its start: the core refuses the content, asks to
//! stop during the load or before the game shows, or logs an error or
//! sends a message at the launch and asks to stop soon after, as FinalBurn
//! Neo does at its own error screen. A stop after the start window, or
//! after a silent launch, ends a game that ran. The session reports each
//! case with the words of the core, and a failed launch leaves the display
//! and the audio untouched.

use neptron_backend::Clock;
use neptron_safety::Rejection;
use neptron_session::{
    EndReason, LAUNCH_STABLE_FRAMES, LaunchError, START_ERRORS_MAX, START_WINDOW_NS,
};
use neptron_testkit::preset::CAPS_LOW_CLOCK;
use neptron_types::Modeline;

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

/// Runs frames until the session ends, at most `max`, and gives the reason
/// and the time since the launch, in nanoseconds.
fn ended_after(rig: &mut Rig, max: u32) -> (Option<EndReason>, u64) {
    let launched = rig.clock.now_ns();
    let reason = until_ended(rig, max);
    (reason, rig.clock.now_ns() - launched)
}

#[test]
fn a_stop_after_the_start_window_ends_a_game_that_ran() {
    let script = format!(
        "neptron-testcore 3\nload_error {CORE_ERROR}\nat 900 log error late\nat 960 shutdown\n"
    );
    with_script!(
        "a_stop_after_the_start_window_ends_a_game_that_ran",
        &script,
        |dir| {
            let mut rig = Rig::launch(dir, Setup::default());
            let (reason, after_ns) = ended_after(&mut rig, 1_200);
            assert!(after_ns >= START_WINDOW_NS, "{after_ns} ns");
            assert_eq!(reason, Some(EndReason::CoreShutdown));
            assert_eq!(rig.session.start_errors(), [CORE_ERROR]);
            assert_eq!(rig.session.last_error(), Some("late"));
        }
    );
}

#[test]
fn a_message_at_the_launch_makes_a_soon_stop_a_failed_start() {
    with_script!(
        "a_message_at_the_launch_makes_a_soon_stop_a_failed_start",
        "neptron-testcore 3\nat 0 message 600 HDD: game.chd not found\nat 120 shutdown\n",
        |dir| {
            let mut rig = Rig::launch(dir, Setup::default());
            let (reason, after_ns) = ended_after(&mut rig, 200);
            assert!(after_ns < START_WINDOW_NS, "{after_ns} ns");
            assert_eq!(reason, Some(EndReason::FailedStart));
            assert!(rig.session.start_errors().is_empty());
            assert_eq!(rig.session.last_message(), Some("HDD: game.chd not found"));
        }
    );
}

#[test]
fn info_and_warn_lines_at_the_launch_do_not_count() {
    with_script!(
        "info_and_warn_lines_at_the_launch_do_not_count",
        "neptron-testcore 3\nat 1 log info hello\nat 1 log warn slow\nat 1 log debug detail\nat 120 shutdown\n",
        |dir| {
            let mut rig = Rig::launch(dir, Setup::default());
            assert!(rig.session.start_errors().is_empty());
            assert_eq!(until_ended(&mut rig, 200), Some(EndReason::CoreShutdown));
            assert_eq!(rig.session.last_error(), None);
        }
    );
}

#[test]
fn error_lines_in_the_settle_frames_count_up_to_the_cap() {
    let mut script = format!("neptron-testcore 3\nload_error {CORE_ERROR}\n");
    for i in 0..START_ERRORS_MAX + 3 {
        script.push_str(&format!("at 2 log error settle {i}\n"));
    }
    script.push_str("at 120 shutdown\n");
    with_script!(
        "error_lines_in_the_settle_frames_count_up_to_the_cap",
        &script,
        |dir| {
            let mut rig = Rig::launch(dir, Setup::default());
            let errors = rig.session.start_errors();
            assert_eq!(errors.len(), START_ERRORS_MAX);
            assert_eq!(errors[0], CORE_ERROR);
            assert_eq!(errors[1], "settle 0");
            assert_eq!(until_ended(&mut rig, 200), Some(EndReason::FailedStart));
        }
    );
}

#[test]
fn a_noisy_load_keeps_its_error_line_and_counts_what_it_lost() {
    let mut script = "neptron-testcore 3\n".to_owned();
    for i in 0..neptron_libretro::LOG_CAPACITY + 5 {
        script.push_str(&format!("at 0 log debug noise {i}\n"));
    }
    script.push_str(&format!("at 0 log error {CORE_ERROR}\n"));
    with_script!(
        "a_noisy_load_keeps_its_error_line_and_counts_what_it_lost",
        &script,
        |dir| {
            let rig = Rig::launch(dir, Setup::default());
            assert_eq!(rig.session.start_errors(), [CORE_ERROR]);
            assert!(rig.session.start_dropped_logs() >= 6);
        }
    );
}

/// A mode that the inputs refuse after the first frame, kept on screen
/// because the output cannot be turned off: the core runs, and its picture
/// is hidden for hundreds of frames. A stop long after a silent launch
/// still ends a game that ran. A mode cannot be refused before one is set,
/// so the first frame shows the game.
#[test]
fn a_refused_mode_does_not_make_a_long_run_a_failed_start() {
    with_script!(
        "a_refused_mode_does_not_make_a_long_run_a_failed_start",
        "neptron-testcore 1\nat 600 shutdown\n",
        |dir| {
            let mut rig = Rig::launch(
                dir,
                Setup {
                    caps: CAPS_LOW_CLOCK,
                    ..Setup::default()
                },
            );
            rig.run(1);
            for call in 0..64 {
                rig.display.fault_blank(call, "blank refused".to_owned());
            }
            rig.session
                .video_mut()
                .set_veto(Box::new(|mode: &Modeline| {
                    Err(Rejection::NotAPanelMode {
                        mode: *mode,
                        panel_modes: 0,
                    })
                }));
            let mut refused_frames = 0;
            let mut reason = None;
            for _ in 0..700 {
                let report = rig.session.run_frame(None).expect("frame");
                refused_frames += u32::from(rig.session.video().current_refused());
                if report.ended.is_some() {
                    reason = report.ended;
                    break;
                }
            }
            assert!(refused_frames > 500, "{refused_frames} refused frames");
            assert_eq!(reason, Some(EndReason::CoreShutdown));
        }
    );
}
