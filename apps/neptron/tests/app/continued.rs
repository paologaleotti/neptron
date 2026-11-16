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

/// Writes `monitor` as `display.monitor` to the global settings.
pub(crate) fn set_monitor(rig: &Rig, monitor: &str) {
    let config = rig.paths.config();
    std::fs::create_dir_all(&config).expect("the config folder");
    std::fs::write(
        config.join("global.toml"),
        format!("schema_version = 1\n\n[display]\nmonitor = \"{monitor}\"\n"),
    )
    .expect("the settings");
}

/// A boot prompt with no answer in the game list, then a game run. Returns
/// the rig in the game run, after some frames. The prompt is for a 31 kHz
/// preset that nobody kept, on a display with no EDID or on a panel, where
/// a named preset is not trusted.
fn game_after_an_unkept_prompt(dir: &Path, panel: bool) -> Rig {
    let mut rig = Rig::new(dir);
    if panel {
        sysfs_with_panel(&rig.paths.sysfs, PANEL_1080P);
        rig.output = "HDMI-A-1".to_owned();
        set_monitor(&rig, "pc_31_120");
    } else {
        set_monitor(&rig, "arcade_31");
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
    let guard = rig.shell().guard();
    assert_eq!(
        guard.active().monitor,
        guard.safe().monitor,
        "the revert wrote the safe default to the settings"
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
    assert!(!notices.iter().any(|n| n.contains("undone")), "{notices:?}");
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

/// A return to the game list is a relaunch of the same boot: it never
/// opens the boot prompt again, also when the run before ended while the
/// prompt was open, and it counts no boot.
#[test]
#[ignore = "tier 2"]
fn a_return_to_the_list_after_an_unanswered_prompt_opens_no_prompt() {
    let dir = std::env::temp_dir().join(format!("neptron-app-return-{}", std::process::id()));
    // A leftover of an earlier run with the same process id; absence is fine.
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("the folder");
    let mut rig = Rig::new(&dir);
    set_monitor(&rig, "arcade_31");
    rig.boot(None);
    rig.frames(2);
    assert!(matches!(rig.shell().guard().phase(), Phase::Pending(_)));
    rig.finish();
    let counted = rig.state_file("video/guard.toml");

    for _ in 0..3 {
        rig.boot(Some(NextRun {
            game: None,
            notice: None,
        }));
        rig.frames(2);
        assert!(
            rig.shell().guard().pending().is_none(),
            "no boot prompt on a return"
        );
        assert_eq!(
            rig.shell().guard().phase(),
            Phase::Safe(SafeReason::NothingConfirmed)
        );
        assert_ne!(rig.screen(), "confirm");
        rig.finish();
        assert_eq!(
            rig.state_file("video/guard.toml"),
            counted,
            "a return counts no boot"
        );
    }
}
