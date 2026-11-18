//! A game that the core does not start or stops: the core refuses the
//! file, stops during the load, stops in the first frame, logs an error at
//! the load and stops soon after at its own error screen (as FinalBurn Neo
//! does for Killer Instinct with no disk image), or ends a game that ran.
//! Each case ends in the game list with a notice that says why, with the
//! display mode and the audio of the list as before, and the frontend
//! never ends.

use neptron::relaunch::GameRef;
use neptron::{NextRun, Outcome};
use neptron_video::Phase;

use crate::rig::{Rig, in_child_with, test_name};

/// The error lines of FinalBurn Neo for Killer Instinct with no disk
/// image, from a core trace.
const FBNEO_ERRORS: &str = "load_error HDD: kinst.chd not found\n";

/// What the game list run shows, to compare with the list after the game.
#[derive(Debug, PartialEq)]
struct ListState {
    mode: String,
    audio_frames_per_30: u64,
}

fn game() -> NextRun {
    NextRun {
        pads: Vec::new(),
        game: Some(GameRef {
            system: "nes".to_owned(),
            path: "nes/Test Game.nes".to_owned(),
        }),
        notice: None,
    }
}

/// Runs the list, and gives its mode and the audio it writes in 30 frames.
fn list_state(rig: &mut Rig) -> ListState {
    rig.frames(2);
    assert_eq!(rig.shell().guard().phase(), Phase::Confirmed);
    let mode = rig
        .shell()
        .engine()
        .expect("the engine of the list")
        .current()
        .expect("a mode")
        .read_back;
    let before = rig.audio.written_frames();
    rig.frames(30);
    ListState {
        mode: format!("{mode:?}"),
        audio_frames_per_30: rig.audio.written_frames() - before,
    }
}

/// The first run of the list, as the player sees it before the launch.
fn first_list(rig: &mut Rig) -> ListState {
    rig.boot(None);
    let state = list_state(rig);
    rig.finish();
    state
}

/// The run of the list after the game: the notice shows, and the list is
/// as before.
fn back_in_list(rig: &mut Rig, next: NextRun, before: &ListState, notice: &str) {
    assert_eq!(next.game, None, "back to the list");
    assert_eq!(next.notice.as_deref(), Some(notice));
    rig.boot(Some(next));
    let after = list_state(rig);
    let shown: Vec<String> = rig
        .shell()
        .snapshot()
        .notices
        .iter()
        .map(|n| n.text.clone())
        .collect();
    assert_eq!(shown, [notice]);
    assert_eq!(rig.screen(), "notice");
    assert_eq!(&after, before, "the mode and the audio of the list");
    rig.finish();
}

/// A launch that fails in the game run: the run relaunches at its boot.
fn launch_fails(script: &str, test: &str, notice: &'static str, core_lines: &'static [&str]) {
    in_child_with(&test_name(module_path!(), test), script, |dir| {
        let mut rig = Rig::new(dir);
        let before = first_list(&mut rig);
        let switches = rig.display.mode_switches().len();
        let (next, log) = rig.try_boot(Some(game())).expect("the game does not start");
        assert_eq!(
            rig.display.mode_switches().len(),
            switches,
            "the game run set no mode"
        );
        let mut expected = vec![notice.to_owned()];
        expected.extend(
            core_lines
                .iter()
                .map(|l| format!("the core logged at the launch: {l}")),
        );
        assert_eq!(log, expected);
        back_in_list(&mut rig, next, &before, notice);
    });
}

/// A game that stops in the game run after its launch.
fn game_stops(script: &str, test: &str, notice: &'static str, core_lines: &'static [&str]) {
    in_child_with(&test_name(module_path!(), test), script, |dir| {
        let mut rig = Rig::new(dir);
        let before = first_list(&mut rig);
        rig.boot(Some(game()));
        let Outcome::Relaunch(next) = rig.until_end(1_200) else {
            panic!("the game run relaunches");
        };
        let log = rig.shell().log_lines(16);
        for line in core_lines {
            let line = format!("the core logged at the launch: {line}");
            assert!(log.contains(&line), "{line} in {log:?}");
        }
        assert!(log.contains(&notice.to_owned()), "{log:?}");
        rig.finish();
        back_in_list(&mut rig, next, &before, notice);
    });
}

#[test]
#[ignore = "tier 2"]
fn a_refused_game_file_returns_to_the_list_with_a_notice() {
    launch_fails(
        &format!("neptron-testcore 2\n{FBNEO_ERRORS}load refuse\n"),
        "a_refused_game_file_returns_to_the_list_with_a_notice",
        "Test Game did not start: the core refused the game file. The core says: \"HDD: kinst.chd not found\". Check that the file is complete and is for this system.",
        &["HDD: kinst.chd not found"],
    );
}

#[test]
#[ignore = "tier 2"]
fn a_shutdown_in_the_load_returns_to_the_list_with_a_notice() {
    launch_fails(
        "neptron-testcore 2\nload shutdown\n",
        "a_shutdown_in_the_load_returns_to_the_list_with_a_notice",
        "Test Game did not start: the core stopped it at the start. Check that the game has all its files.",
        &[],
    );
}

#[test]
#[ignore = "tier 2"]
fn a_shutdown_in_the_first_frame_returns_to_the_list_with_a_notice() {
    game_stops(
        &format!(
            "neptron-testcore 1\nat {} shutdown\n",
            neptron_session::LAUNCH_STABLE_FRAMES
        ),
        "a_shutdown_in_the_first_frame_returns_to_the_list_with_a_notice",
        "Test Game did not start: the core stopped it at the start. Check that the game has all its files.",
        &[],
    );
}

#[test]
#[ignore = "tier 2"]
fn an_error_screen_of_the_core_returns_to_the_list_with_a_notice() {
    game_stops(
        &format!("neptron-testcore 2\n{FBNEO_ERRORS}at 60 shutdown\n"),
        "an_error_screen_of_the_core_returns_to_the_list_with_a_notice",
        "Test Game did not start: the core stopped it at the start. The core says: \"HDD: kinst.chd not found\". Check that the game has all its files.",
        &["HDD: kinst.chd not found"],
    );
}

/// A core that loads in silence, logs an error line in the first loop
/// frame and stops in that frame: S2 quotes that line.
#[test]
#[ignore = "tier 2"]
fn an_error_line_in_the_frame_of_the_stop_reaches_the_notice() {
    let frame = neptron_session::LAUNCH_STABLE_FRAMES;
    game_stops(
        &format!(
            "neptron-testcore 3\nat {frame} log error Driver init failed\nat {frame} shutdown\n"
        ),
        "an_error_line_in_the_frame_of_the_stop_reaches_the_notice",
        "Test Game did not start: the core stopped it at the start. The core says: \"Driver init failed\". Check that the game has all its files.",
        &[],
    );
}

/// A core that ends a game that ran, with a silent launch: the list shows
/// S3 with the last message of the core.
#[test]
#[ignore = "tier 2"]
fn a_core_that_ends_a_game_returns_to_the_list_with_a_notice() {
    game_stops(
        "neptron-testcore 3\nat 880 message 60 Game over\nat 900 shutdown\n",
        "a_core_that_ends_a_game_returns_to_the_list_with_a_notice",
        "Test Game stopped: the core ended the game. The core says: \"Game over\".",
        &[],
    );
}

/// The same with no word of the core: S3 with no quote.
#[test]
#[ignore = "tier 2"]
fn a_silent_core_that_ends_a_game_still_gives_a_notice() {
    game_stops(
        "neptron-testcore 1\nat 900 shutdown\n",
        "a_silent_core_that_ends_a_game_still_gives_a_notice",
        "Test Game stopped: the core ended the game.",
        &[],
    );
}

/// A noisy launch whose log queue overflows: the error line still reaches
/// the notice and the log, and the log says that lines were lost.
#[test]
#[ignore = "tier 2"]
fn a_noisy_launch_keeps_its_error_line_and_says_what_it_lost() {
    let mut script = "neptron-testcore 3\n".to_owned();
    for i in 0..80 {
        script.push_str(&format!("at 0 log debug noise {i}\n"));
    }
    script.push_str("at 0 log error HDD: kinst.chd not found\nat 60 shutdown\n");
    let test = "a_noisy_launch_keeps_its_error_line_and_says_what_it_lost";
    in_child_with(&test_name(module_path!(), test), &script, |dir| {
        let mut rig = Rig::new(dir);
        rig.boot(Some(game()));
        let Outcome::Relaunch(next) = rig.until_end(600) else {
            panic!("the game run relaunches");
        };
        let log = rig.shell().log_lines(16);
        assert!(
            log.iter().any(|l| l.starts_with("the core log lost ")
                && l.ends_with("the first 8 error lines of the launch are kept")),
            "{log:?}"
        );
        assert!(
            log.contains(&"the core logged at the launch: HDD: kinst.chd not found".to_owned()),
            "{log:?}"
        );
        assert_eq!(
            next.notice.as_deref(),
            Some(
                "Test Game did not start: the core stopped it at the start. The core says: \"HDD: kinst.chd not found\". Check that the game has all its files."
            )
        );
        rig.finish();
    });
}
