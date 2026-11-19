//! Every option of the Picture mode and Refresh rate rows of the quick
//! menu, in a running game, on each display class the rig models: the
//! 15 kHz CRT of the safe start, a CRT on the GPIO DPI output of a
//! Raspberry Pi (no interlace, 165 MHz), a 1080p flat panel on HDMI, and a
//! TV on the composite output of a Pi. An enabled option changes the
//! setting at the game level with no prompt, the mode the display takes
//! lies inside the profile, a picture shows, and the notice of a new
//! picture that is not exact shows after the change. A greyed option
//! changes nothing, whatever the input.

use neptron::NextRun;
use neptron::relaunch::GameRef;
use neptron_testkit::{DisplayOptions, FakeDisplay};
use neptron_types::{Control, GpuCaps, Modeline, MonitorKind, MonitorProfile};
use neptron_ui::snapshot::SettingValue;
use neptron_video::Phase;

use crate::panel::{PANEL_1080P, sysfs_with_panel};
use crate::rig::{Rig, in_child, in_children, test_name};

/// The limits of the GPIO DPI output of a Raspberry Pi, as the shipped
/// hardware matrix gives them (`pi_vc4_dpi`): the data path a Pi uses.
fn pi_dpi_caps() -> GpuCaps {
    let matrix = neptron_platform::Matrix::builtin().expect("the shipped matrix");
    matrix
        .entries()
        .iter()
        .find(|e| e.id == "pi_vc4_dpi")
        .map(|e| e.caps)
        .expect("pi_vc4_dpi is in data/hardware/matrix.toml")
}

fn the_game() -> Option<NextRun> {
    Some(NextRun {
        pads: Vec::new(),
        game: Some(GameRef {
            system: "nes".to_owned(),
            path: "nes/Test Game.nes".to_owned(),
        }),
        notice: None,
    })
}

/// The mode lies inside the profile: a range of a CRT holds its line rate
/// and its field rate, a panel lists it, a TV standard has it.
fn inside(profile: &MonitorProfile, mode: &Modeline) -> bool {
    match &profile.kind {
        MonitorKind::Crt { ranges } => {
            let (line, field) = (mode.line_rate_mhz(), mode.field_rate_mhz());
            ranges.iter().any(|r| {
                (r.h_min_mhz..=r.h_max_mhz).contains(&line)
                    && (r.v_min_mhz..=r.v_max_mhz).contains(&field)
            })
        }
        MonitorKind::FlatPanel { modes } => modes.contains(mode),
        MonitorKind::Tv { standard } => standard
            .modes()
            .iter()
            .any(|m| m.hactive == mode.hactive && m.vactive == mode.vactive),
    }
}

/// Dismisses the notices on show, then goes to the Video page of the
/// quick menu.
fn to_video_page(rig: &mut Rig) {
    for _ in 0..8 {
        if rig.screen() != "notice" {
            break;
        }
        rig.tap(&[Control::A]);
    }
    if rig.screen() == "options" {
        rig.tap(&[Control::B]);
    }
    if rig.screen() == "quick-menu/video" {
        return;
    }
    if rig.screen() != "quick-menu" {
        rig.open_menu();
        rig.frames(1);
        for _ in 0..8 {
            if rig.screen() != "notice" {
                break;
            }
            rig.tap(&[Control::A]);
        }
    }
    assert_eq!(rig.screen(), "quick-menu", "{:?}", rig.tree());
    rig.focus("Video");
    rig.tap(&[Control::A]);
    assert_eq!(rig.screen(), "quick-menu/video");
}

/// The options of the row `label` of the snapshot: label and enabled.
fn options(rig: &mut Rig, label: &str) -> Vec<(String, bool)> {
    let row = rig
        .shell()
        .snapshot()
        .settings
        .rows
        .iter()
        .find(|r| r.label == label)
        .map(|r| (r.value.clone(), r.is_enabled()));
    let Some((SettingValue::Choice { options, .. }, row_enabled)) = row else {
        panic!("the row {label}: {row:?}");
    };
    options
        .iter()
        .map(|o| (o.label.clone(), row_enabled && o.is_enabled()))
        .collect()
}

/// Walks every option of the rows of the Video page on the display of
/// `rig`, and returns how many options it chose and how many were greyed.
fn walk(mut rig: Rig, key_of_row: &[(&str, &str)]) -> (usize, usize) {
    rig.boot(the_game());
    assert!(rig.shell().in_game());
    rig.frames(30);
    let profile = rig.shell().guard().active().monitor.profile().clone();
    let (mut chosen, mut greyed) = (0, 0);
    for &(label, key) in key_of_row {
        to_video_page(&mut rig);
        let all = options(&mut rig, label);
        for (option, enabled) in all {
            to_video_page(&mut rig);
            let before = rig.shell().config().text(key);
            let notices = rig.shell().snapshot().notices.len();
            let proposal = rig
                .shell()
                .engine()
                .and_then(|e| e.current())
                .map(|c| c.chosen.proposal);
            rig.focus(label);
            let row_enabled = rig
                .shell()
                .snapshot()
                .settings
                .rows
                .iter()
                .find(|r| r.label == label)
                .is_some_and(|r| r.is_enabled());
            rig.tap(&[Control::A]);
            if !row_enabled {
                assert_eq!(
                    rig.screen(),
                    "quick-menu/video",
                    "a disabled row opens nothing"
                );
                greyed += 1;
                continue;
            }
            assert_eq!(rig.screen(), "options");
            rig.focus(&option);
            rig.tap(&[Control::A]);
            if !enabled {
                // Accept, Left and Right on a greyed option change nothing.
                rig.tap(&[Control::Left]);
                rig.tap(&[Control::Right]);
                assert_eq!(rig.screen(), "options", "{label} {option}: greyed");
                assert_eq!(rig.shell().config().text(key), before, "{label} {option}");
                rig.tap(&[Control::B]);
                greyed += 1;
                continue;
            }
            rig.frames(30);
            assert_eq!(
                rig.shell().guard().phase(),
                Phase::Confirmed,
                "{label} {option}"
            );
            let mode = rig.display.active_mode().expect("a mode on screen");
            assert!(inside(&profile, &mode), "{label} {option}: {mode:?}");
            for switch in rig.display.mode_switches() {
                assert!(
                    inside(&profile, &switch.active),
                    "{label} {option}: {switch:?}"
                );
            }
            let frame = rig.display.frames().last().expect("a frame").scene;
            assert!(frame.game.is_some(), "{label} {option}: a picture");
            let current = rig
                .shell()
                .engine()
                .and_then(|e| e.current())
                .map(|c| (c.chosen.proposal, c.chosen.notice.is_some()));
            if let (Some((now, noticed)), Some(before)) = (current, proposal)
                && now != before
                && noticed
            {
                assert!(
                    rig.shell().snapshot().notices.len() > notices,
                    "{label} {option}: the notice of the new picture"
                );
            }
            chosen += 1;
        }
    }
    rig.finish();
    (chosen, greyed)
}

const ROWS: [(&str, &str); 2] = [
    ("Picture mode", "display.style"),
    ("Refresh rate", "display.refresh_policy"),
];

#[test]
#[ignore = "tier 2"]
fn every_option_on_a_15_khz_crt_is_safe_or_greyed() {
    let name = test_name(
        module_path!(),
        "every_option_on_a_15_khz_crt_is_safe_or_greyed",
    );
    in_child(&name, |dir| {
        let (chosen, greyed) = walk(Rig::new(dir), &ROWS);
        assert!(chosen >= 5 && greyed >= 3, "{chosen} {greyed}");
    });
}

#[test]
#[ignore = "tier 2"]
fn every_option_on_a_pi_dpi_crt_is_safe_or_greyed() {
    let name = test_name(
        module_path!(),
        "every_option_on_a_pi_dpi_crt_is_safe_or_greyed",
    );
    in_child(&name, |dir| {
        let mut rig = Rig::new(dir);
        rig.display =
            FakeDisplay::with_options(rig.clock.clone(), pi_dpi_caps(), DisplayOptions::default());
        let (chosen, greyed) = walk(rig, &ROWS);
        assert!(chosen >= 1 && greyed >= 3, "{chosen} {greyed}");
    });
}

#[test]
#[ignore = "tier 2"]
fn every_option_on_a_flat_panel_is_safe_or_greyed() {
    let name = test_name(
        module_path!(),
        "every_option_on_a_flat_panel_is_safe_or_greyed",
    );
    in_child(&name, |dir| {
        let mut rig = Rig::new(dir);
        sysfs_with_panel(&rig.paths.sysfs, PANEL_1080P);
        rig.output = "HDMI-A-1".to_owned();
        let (chosen, greyed) = walk(rig, &ROWS);
        assert!(
            chosen >= 8,
            "every picture mode fits a 1080p panel: {chosen} {greyed}"
        );
    });
}

#[test]
#[ignore = "tier 2"]
fn every_option_on_a_pi_composite_tv_is_safe_or_greyed() {
    let name = test_name(
        module_path!(),
        "every_option_on_a_pi_composite_tv_is_safe_or_greyed",
    );
    in_child(&name, |dir| {
        let (chosen, greyed) = walk(crate::tv::composite_rig(dir), &ROWS);
        assert!(chosen >= 4 && greyed >= 4, "{chosen} {greyed}");
    });
}

/// A stored picture mode that the display does not give (here 4x on a
/// 15 kHz tube), at each settings level: the game runs with the winner of
/// its plan, inside the profile, and the notice names what was asked, why,
/// and what shows.
#[test]
#[ignore = "tier 2"]
fn a_stored_mode_the_display_does_not_give_shows_a_safe_picture_and_says_so() {
    let name = test_name(
        module_path!(),
        "a_stored_mode_the_display_does_not_give_shows_a_safe_picture_and_says_so",
    );
    // One process hosts one core, so each level runs in a child of its own.
    in_children(&name, 3, |dir, stage| {
        let files = [
            "global.toml",
            "systems/nes.toml",
            "games/nes/Test Game.toml",
        ];
        {
            let file = files[stage as usize];
            let root = dir.join(format!("level-{stage}"));
            std::fs::create_dir_all(&root).expect("a folder");
            let mut rig = Rig::new(&root);
            let path = rig.paths.config().join(file);
            std::fs::create_dir_all(path.parent().expect("a parent")).expect("the folder");
            std::fs::write(&path, "schema_version = 1\n\n[display]\nstyle = \"4x\"\n")
                .expect("the settings");
            rig.boot(the_game());
            assert!(rig.shell().in_game());
            rig.frames(30);
            let profile = rig.shell().guard().active().monitor.profile().clone();
            let mode = rig.display.active_mode().expect("a mode on screen");
            assert!(inside(&profile, &mode), "{file}: {mode:?}");
            let frame = rig.display.frames().last().expect("a frame").scene;
            assert!(frame.game.is_some(), "{file}: a picture");
            let shown: Vec<String> = rig
                .shell()
                .snapshot()
                .notices
                .iter()
                .map(|n| n.text.clone())
                .collect();
            assert!(
                shown.iter().any(|t| t.starts_with(
                    "Picture mode 4x is not possible on this screen: Needs another monitor. Shown: "
                )),
                "{file}: {shown:?}"
            );
            rig.finish();
        }
    });
}

/// The picture mode set in the quick menu removes the expert keys that it
/// covers at the game level, in the same write; a value at another level
/// stays.
#[test]
#[ignore = "tier 2"]
fn a_picture_mode_set_at_a_level_removes_the_expert_keys_of_that_level() {
    let name = test_name(
        module_path!(),
        "a_picture_mode_set_at_a_level_removes_the_expert_keys_of_that_level",
    );
    in_child(&name, |dir| {
        let mut rig = Rig::new(dir);
        let games = rig.paths.config().join("games").join("nes");
        std::fs::create_dir_all(&games).expect("the folder");
        std::fs::write(
            games.join("Test Game.toml"),
            "schema_version = 1\n\n[display]\nvertical = \"fit\"\npanel_scale = \"integer\"\n",
        )
        .expect("the game settings");
        std::fs::write(
            rig.paths.config().join("global.toml"),
            "schema_version = 1\n\n[display]\nvertical = \"fit\"\n",
        )
        .expect("the global settings");
        rig.boot(the_game());
        rig.frames(30);
        to_video_page(&mut rig);
        rig.focus("Picture mode");
        rig.tap(&[Control::A]);
        rig.focus("Sharp lines");
        rig.tap(&[Control::A]);
        rig.frames(10);
        let config = rig.shell().config();
        assert_eq!(config.text("display.style"), "sharp");
        let source = |key| config.resolved(key).map(|r| r.source);
        assert_eq!(
            source("display.vertical"),
            Some(neptron_settings::Source::Global),
            "the game value is gone, the global one stays"
        );
        assert_eq!(
            source("display.panel_scale"),
            Some(neptron_settings::Source::Default)
        );
        rig.finish();
        let file = rig.state_file("config/games/nes/Test Game.toml");
        assert!(file.contains("style = \"sharp\""), "{file}");
        assert!(!file.contains("vertical"), "{file}");
        assert!(!file.contains("panel_scale"), "{file}");
    });
}
