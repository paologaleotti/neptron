//! A game run continues the boot of the game list run: it opens no boot
//! prompt, shows no revert notice, and a display that was not kept stays
//! on the safe default.

use std::path::Path;

use neptron::NextRun;
use neptron_types::Control;
use neptron_video::{Phase, SafeReason};

use crate::panel::{CONFIRM_WINDOW_FRAMES, PANEL_1080P, sysfs_with_panel};
use crate::rig::{Rig, in_child, test_name};

/// The game of the test core.
fn game() -> NextRun {
    NextRun {
        game: Some(neptron::relaunch::GameRef {
            system: "nes".to_owned(),
            path: "nes/Test Game.nes".to_owned(),
        }),
        notice: None,
    }
}

/// A boot prompt with no answer in the game list, then a game run. Returns
/// the rig in the game run, after some frames.
fn game_after_an_unkept_prompt(dir: &Path, panel: bool) -> Rig {
    let mut rig = Rig::new(dir);
    if panel {
        sysfs_with_panel(&rig.paths.sysfs, PANEL_1080P);
        rig.output = "HDMI-A-1".to_owned();
    }
    rig.boot(None);
    rig.frames(2);
    assert!(matches!(rig.shell().guard().phase(), Phase::Pending(_)));
    rig.frames(CONFIRM_WINDOW_FRAMES);
    assert_eq!(
        rig.shell().guard().phase(),
        Phase::Safe(SafeReason::NothingConfirmed)
    );
    rig.finish();

    let switches = rig.display.mode_switches().len();
    rig.boot(Some(game()));
    assert!(rig.shell().in_game());
    assert!(rig.shell().guard().pending().is_none(), "no boot prompt");
    rig.frames(60);
    assert_eq!(
        rig.shell().guard().phase(),
        Phase::Safe(SafeReason::NothingConfirmed)
    );
    for switch in &rig.display.mode_switches()[switches..] {
        assert!(
            switch.active.line_rate_mhz() < 20_000_000,
            "only the 15 kHz safe default: {switch:?}"
        );
    }
    let notices: Vec<String> = rig
        .shell()
        .snapshot()
        .notices
        .iter()
        .map(|n| n.text.clone())
        .collect();
    assert!(
        !notices.iter().any(|n| n.contains("went back")),
        "{notices:?}"
    );
    rig
}

#[test]
#[ignore = "tier 2"]
fn a_game_on_a_crt_with_nothing_kept_starts_in_the_safe_default_quietly() {
    in_child(
        &test_name(
            module_path!(),
            "a_game_on_a_crt_with_nothing_kept_starts_in_the_safe_default_quietly",
        ),
        |dir| {
            let mut rig = game_after_an_unkept_prompt(dir, false);
            rig.tap(&[Control::Select, Control::Start]);
            rig.frames(2);
            assert_eq!(rig.screen(), "quick-menu");
            rig.finish();
        },
    );
}

#[test]
#[ignore = "tier 2"]
fn a_game_on_an_unkept_panel_never_shows_the_panel_mode() {
    in_child(
        &test_name(
            module_path!(),
            "a_game_on_an_unkept_panel_never_shows_the_panel_mode",
        ),
        |dir| {
            let mut rig = game_after_an_unkept_prompt(dir, true);
            rig.finish();
        },
    );
}
