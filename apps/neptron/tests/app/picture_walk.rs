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
use neptron::pages;
use neptron::relaunch::GameRef;
use neptron_testkit::{DisplayOptions, FakeDisplay};
use neptron_types::{Control, GpuCaps, Modeline, MonitorKind, MonitorProfile};
use neptron_ui::snapshot::SettingValue;
use neptron_video::{Phase, PictureStyle, RefreshPolicy};

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

pub(crate) fn the_game() -> Option<NextRun> {
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
pub(crate) fn to_video_page(rig: &mut Rig) {
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
        rig.wait_for_picture_answers();
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
    rig.wait_for_picture_answers();
}

/// An option of a row of the snapshot.
#[derive(Debug, Clone)]
pub(crate) struct Shown {
    pub(crate) label: String,
    pub(crate) enabled: bool,
    pub(crate) help: String,
    pub(crate) detail: String,
}

/// The options of the row `label` of the snapshot.
pub(crate) fn options(rig: &mut Rig, label: &str) -> Vec<Shown> {
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
        .map(|o| Shown {
            label: o.label.clone(),
            enabled: row_enabled && o.is_enabled(),
            help: o.help.clone(),
            detail: o.detail.clone(),
        })
        .collect()
}

/// The value in the settings of the option `label` of the row of `key`.
fn value_of(key: &str, label: &str) -> &'static str {
    match key {
        "display.style" => PictureStyle::ALL
            .into_iter()
            .find(|s| pages::mode_label(*s) == label)
            .map(PictureStyle::key),
        _ => RefreshPolicy::ALL
            .into_iter()
            .find(|r| pages::refresh_label(*r) == label)
            .map(RefreshPolicy::key),
    }
    .unwrap_or_else(|| panic!("{key} has the option {label}"))
}

/// What a walk found: the options it chose (with " (resets)" when the
/// pick also set the other row to Auto), the greyed options of enabled
/// rows (activated with every input), and the disabled rows.
#[derive(Debug, Default, PartialEq, Eq)]
struct Walked {
    chosen: Vec<String>,
    greyed: Vec<String>,
    disabled_rows: Vec<String>,
}

/// The notice texts that a picture can show start with one of these.
const PICTURE_NOTICE_STARTS: [&str; 8] = [
    "Soft picture:",
    "Small picture:",
    "The game runs at",
    "Each frame shows",
    "Whole steps",
    "No scanlines:",
    "Picture:",
    "Picture mode",
];

/// The settings file of the game, as it is on disk now.
pub(crate) fn game_file(rig: &Rig) -> String {
    std::fs::read_to_string(rig.paths.config().join("games/nes/Test Game.toml")).unwrap_or_default()
}

/// The global settings file, as it is on disk now.
pub(crate) fn global_file(rig: &Rig) -> String {
    std::fs::read_to_string(rig.paths.config().join("global.toml")).unwrap_or_default()
}

/// The line of `key` with `value` in a settings file.
fn line(key: &str, value: &str) -> String {
    format!("{} = \"{value}\"", key.trim_start_matches("display."))
}

/// The other picture row of `key`, with its Auto value.
fn other_row(key: &str) -> (&'static str, &'static str) {
    pages::other_row(key).expect("a picture row")
}

/// Walks every option of the rows of the Video page on the display of
/// `rig`, each judged as the menu shows it at the moment of the pick. The
/// rows are walked in order, so the options of the Refresh rate row are
/// judged with the Picture mode that the walk picked last (Native where the
/// display gives it), and each pick of a refresh choice is judged with the
/// one before it. A
/// chosen option sets the setting at the game level with no prompt (and
/// the other row to Auto when its help line says so), in memory and on
/// disk; the display takes a mode inside the profile, a picture shows; a
/// new picture that is not exact shows its notice, and a picture that does
/// not change shows none. A greyed option, activated with Accept, Left and
/// Right, changes nothing in memory, on disk or on the engine.
fn walk(mut rig: Rig, key_of_row: &[(&str, &str)]) -> Walked {
    rig.boot(the_game());
    assert!(rig.shell().in_game());
    rig.frames(30);
    let profile = rig.shell().guard().active().monitor.profile().clone();
    let mut walked = Walked::default();
    for &(label, key) in key_of_row {
        to_video_page(&mut rig);
        let row_enabled = rig
            .shell()
            .snapshot()
            .settings
            .rows
            .iter()
            .find(|r| r.label == label)
            .is_some_and(|r| r.is_enabled());
        if !row_enabled {
            rig.focus(label);
            rig.tap(&[Control::A]);
            assert_eq!(
                rig.screen(),
                "quick-menu/video",
                "a disabled row opens nothing"
            );
            walked.disabled_rows.push(label.to_owned());
            continue;
        }
        let count = options(&mut rig, label).len();
        for index in 0..count {
            to_video_page(&mut rig);
            let shown = options(&mut rig, label)[index].clone();
            let option = shown.label.clone();
            let (other, other_auto) = other_row(key);
            let before = rig.shell().config().text(key);
            let other_before = rig.shell().config().text(other);
            let file_before = game_file(&rig);
            let notices = rig.shell().snapshot().notices.len();
            let current = |rig: &mut Rig| {
                rig.shell()
                    .engine()
                    .and_then(|e| e.current())
                    .map(|c| (c.chosen.proposal, c.chosen.notice.is_some()))
            };
            let proposal = current(&mut rig).map(|(p, _)| p);
            let mode_before = rig.display.active_mode();
            rig.focus(label);
            rig.tap(&[Control::A]);
            assert_eq!(rig.screen(), "options");
            rig.focus(&option);
            if !shown.enabled {
                // Left and Right move the cursor by a page, so each input
                // starts on the greyed option again.
                for input in [Control::A, Control::Left, Control::Right, Control::A] {
                    rig.focus(&option);
                    rig.tap(&[input]);
                    rig.frames(2);
                    assert_eq!(rig.screen(), "options", "{label} {option}: greyed");
                }
                rig.frames(30);
                assert_eq!(rig.shell().config().text(key), before, "{label} {option}");
                assert_eq!(game_file(&rig), file_before, "{label} {option}: on disk");
                assert_eq!(
                    current(&mut rig).map(|(p, _)| p),
                    proposal,
                    "{label} {option}"
                );
                assert_eq!(rig.display.active_mode(), mode_before, "{label} {option}");
                rig.tap(&[Control::B]);
                walked.greyed.push(format!("{label}: {option}"));
                continue;
            }
            let resets = shown.detail == pages::reset_tag(other);
            rig.tap(&[Control::A]);
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
            // The stored values, in memory and on disk.
            let value = value_of(key, &option);
            assert_eq!(rig.shell().config().text(key), value, "{label} {option}");
            let other_now = rig.shell().config().text(other);
            if resets {
                assert_eq!(other_now, other_auto, "{label} {option}: the reset");
            } else {
                assert_eq!(other_now, other_before, "{label} {option}: no reset");
            }
            // The writer thread saves the file after the frame.
            let mut lines = Vec::new();
            if before != value {
                lines.push(line(key, value));
            }
            if resets {
                lines.push(line(other, other_auto));
            }
            rig.until(600, "the write of the game settings", |r| {
                let file = game_file(r);
                lines.iter().all(|l| file.contains(l.as_str()))
            });
            let new: Vec<String> = rig
                .shell()
                .snapshot()
                .notices
                .iter()
                .skip(notices)
                .map(|n| n.text.clone())
                .collect();
            let picture_notice = new.iter().any(|t| {
                PICTURE_NOTICE_STARTS
                    .iter()
                    .any(|start| t.starts_with(start))
            });
            match (current(&mut rig), proposal) {
                (Some((now, noticed)), Some(before)) if now != before && noticed => assert!(
                    picture_notice,
                    "{label} {option}: the notice of the new picture: {new:?}"
                ),
                (Some((now, _)), Some(before)) if now == before => assert!(
                    !picture_notice,
                    "{label} {option}: no notice for the same picture: {new:?}"
                ),
                _ => {}
            }
            if resets {
                let named = format!("{} is set to Auto too.", pages::row_label(other));
                assert!(new.contains(&named), "{label} {option}: {new:?}");
                walked.chosen.push(format!("{label}: {option} (resets)"));
            } else {
                walked.chosen.push(format!("{label}: {option}"));
            }
        }
    }
    rig.finish();
    walked
}

/// The options of the picture rows on Settings > Display, judged for every
/// game: plain, not for every game (selectable, with the reason in the help
/// line), and greyed.
#[derive(Debug, Default, PartialEq, Eq)]
struct Judged {
    plain: Vec<String>,
    not_for_every_game: Vec<String>,
    /// Selectable, and a pick also sets the other row to Auto.
    resets: Vec<String>,
    greyed: Vec<String>,
    disabled_rows: Vec<String>,
}

/// Opens Settings > Display from the game list.
pub(crate) fn to_settings_display(rig: &mut Rig) {
    for _ in 0..8 {
        if rig.screen() == "display" {
            rig.wait_for_picture_answers();
            return;
        }
        match rig.screen().as_str() {
            "options" | "notice" | "confirm" => rig.tap(&[Control::B]),
            _ => break,
        }
    }
    rig.focus("Settings");
    rig.tap(&[Control::A]);
    rig.focus("Display");
    rig.tap(&[Control::A]);
    assert_eq!(rig.screen(), "display");
    rig.wait_for_picture_answers();
}

/// The Settings > Display rows of the picture on the display of `rig`, in
/// the game list: each option judged for the reference games. A greyed
/// option changes nothing; the first option that is not for every game and
/// the first plain option other than Auto set the global level, in memory
/// and on disk, after the Keep of the prompt.
fn settings_walk(mut rig: Rig) -> Judged {
    rig.boot(None);
    rig.until(600, "the scan", |r| {
        !r.shell().snapshot().systems.is_empty()
    });
    let mut judged = Judged::default();
    for (label, key) in ROWS {
        to_settings_display(&mut rig);
        let enabled = rig
            .shell()
            .snapshot()
            .settings
            .rows
            .iter()
            .find(|r| r.label == label)
            .is_some_and(|r| r.is_enabled());
        if !enabled {
            judged.disabled_rows.push(label.to_owned());
            continue;
        }
        let shown = options(&mut rig, label);
        for o in &shown {
            let name = format!("{label}: {}", o.label);
            if !o.enabled {
                judged.greyed.push(name);
            } else if o.detail == pages::NOT_ALL_GAMES {
                judged.not_for_every_game.push(name);
            } else if o.detail.ends_with("Auto too") {
                judged.resets.push(name);
            } else {
                judged.plain.push(name);
            }
        }
        let greyed = shown.iter().find(|o| !o.enabled);
        let partial = shown
            .iter()
            .find(|o| o.enabled && o.detail == pages::NOT_ALL_GAMES);
        let plain = shown.iter().skip(1).find(|o| {
            o.enabled && o.detail != pages::NOT_ALL_GAMES && !o.detail.ends_with("Auto too")
        });
        if let Some(o) = greyed {
            let file_before = global_file(&rig);
            let before = rig.shell().config().text(key);
            rig.focus(label);
            rig.tap(&[Control::A]);
            rig.focus(&o.label);
            rig.tap(&[Control::A]);
            rig.frames(30);
            assert_eq!(rig.screen(), "options", "{label} {}: greyed", o.label);
            assert_eq!(rig.shell().config().text(key), before);
            assert_eq!(global_file(&rig), file_before);
            rig.tap(&[Control::B]);
        }
        for o in [partial, plain].into_iter().flatten() {
            to_settings_display(&mut rig);
            rig.focus(label);
            rig.tap(&[Control::A]);
            rig.focus(&o.label);
            rig.tap(&[Control::A]);
            rig.frames(10);
            if rig.screen() == "confirm" {
                rig.focus("Keep");
                rig.tap(&[Control::A]);
                rig.frames(10);
            }
            let value = value_of(key, &o.label);
            assert_eq!(
                rig.shell()
                    .config()
                    .get(neptron_settings::Level::Global, key),
                Some(&neptron_settings::Value::Text(value.to_owned())),
                "{label} {}",
                o.label
            );
            let wanted = line(key, value);
            rig.until(600, "the write of the global settings", |r| {
                global_file(r).contains(&wanted)
            });
        }
    }
    rig.finish();
    judged
}

const ROWS: [(&str, &str); 2] = [
    ("Picture mode", "display.style"),
    ("Refresh rate", "display.refresh_policy"),
];

fn names(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| (*s).to_owned()).collect()
}

#[test]
#[ignore = "tier 2"]
fn every_option_on_a_15_khz_crt_is_safe_or_greyed() {
    let name = test_name(
        module_path!(),
        "every_option_on_a_15_khz_crt_is_safe_or_greyed",
    );
    in_child(&name, |dir| {
        let walked = walk(Rig::new(dir), &ROWS);
        println!("{walked:?}");
        assert_eq!(walked, expected_15_khz());
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
        let walked = walk(rig, &ROWS);
        println!("{walked:?}");
        assert_eq!(walked, expected_pi_dpi());
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
        let walked = walk(rig, &ROWS);
        println!("{walked:?}");
        assert_eq!(walked, expected_panel());
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
        let walked = walk(crate::tv::composite_rig(dir), &ROWS);
        println!("{walked:?}");
        assert_eq!(walked, expected_tv());
    });
}

/// Settings > Display on each rig: the rows of the picture judged for every
/// game.
macro_rules! settings_walks {
    ($($test:ident: $rig:expr, $expected:expr;)*) => {
        $(
            #[test]
            #[ignore = "tier 2"]
            fn $test() {
                let name = test_name(module_path!(), stringify!($test));
                in_child(&name, |dir| {
                    let judged = settings_walk($rig(dir));
                    println!("{judged:?}");
                    assert_eq!(judged, $expected);
                });
            }
        )*
    };
}

fn pi_dpi_rig(dir: &std::path::Path) -> Rig {
    let mut rig = Rig::new(dir);
    rig.display =
        FakeDisplay::with_options(rig.clock.clone(), pi_dpi_caps(), DisplayOptions::default());
    rig
}

fn panel_rig(dir: &std::path::Path) -> Rig {
    let mut rig = Rig::new(dir);
    sysfs_with_panel(&rig.paths.sysfs, PANEL_1080P);
    rig.output = "HDMI-A-1".to_owned();
    rig
}

settings_walks! {
    settings_display_on_a_15_khz_crt_judges_every_game: Rig::new, expected_settings_15_khz();
    settings_display_on_a_pi_dpi_crt_judges_every_game: pi_dpi_rig, expected_settings_pi_dpi();
    settings_display_on_a_flat_panel_judges_every_game: panel_rig, expected_settings_panel();
    settings_display_on_a_pi_composite_tv_judges_every_game: crate::tv::composite_rig, expected_settings_tv();
}

/// A 15 kHz monitor shows no multiple and no double rate for any reference
/// game, and Native for every game but the 480-line one.
fn expected_settings_15_khz() -> Judged {
    Judged {
        plain: names(&[
            "Picture mode: Auto",
            "Picture mode: Sharp lines",
            "Picture mode: Game speed",
            "Picture mode: Fill",
            "Refresh rate: Auto",
            "Refresh rate: 50 Hz",
            "Refresh rate: 60 Hz",
        ]),
        not_for_every_game: names(&["Picture mode: Native"]),
        resets: Vec::new(),
        greyed: names(&[
            "Picture mode: 2x",
            "Picture mode: 3x",
            "Picture mode: 4x",
            "Refresh rate: Double rate",
        ]),
        disabled_rows: Vec::new(),
    }
}

/// The DPI output of a Pi sends no interlace: the interlaced reference game
/// has no exact or native picture there, and no forced rate.
fn expected_settings_pi_dpi() -> Judged {
    Judged {
        plain: names(&[
            "Picture mode: Auto",
            "Picture mode: Game speed",
            "Picture mode: Fill",
            "Refresh rate: Auto",
        ]),
        not_for_every_game: names(&[
            "Picture mode: Sharp lines",
            "Picture mode: Native",
            "Refresh rate: 50 Hz",
            "Refresh rate: 60 Hz",
        ]),
        resets: Vec::new(),
        greyed: names(&[
            "Picture mode: 2x",
            "Picture mode: 3x",
            "Picture mode: 4x",
            "Refresh rate: Double rate",
        ]),
        disabled_rows: Vec::new(),
    }
}

/// A 1080p panel at 60 Hz shows 3x and 4x only for the small games, and
/// holds the speed only of the games near 60 Hz.
fn expected_settings_panel() -> Judged {
    Judged {
        plain: names(&[
            "Picture mode: Auto",
            "Picture mode: Sharp lines",
            "Picture mode: Fill",
            "Picture mode: Native",
            "Picture mode: 2x",
        ]),
        not_for_every_game: names(&[
            "Picture mode: Game speed",
            "Picture mode: 3x",
            "Picture mode: 4x",
        ]),
        resets: Vec::new(),
        greyed: Vec::new(),
        disabled_rows: names(&["Refresh rate"]),
    }
}

/// A TV has one rate and one line count for every game; Native does not
/// hold the 480-line progressive game.
fn expected_settings_tv() -> Judged {
    Judged {
        plain: names(&[
            "Picture mode: Auto",
            "Picture mode: Sharp lines",
            "Picture mode: Fill",
        ]),
        not_for_every_game: names(&["Picture mode: Native"]),
        resets: Vec::new(),
        greyed: names(&[
            "Picture mode: Game speed",
            "Picture mode: 2x",
            "Picture mode: 3x",
            "Picture mode: 4x",
        ]),
        disabled_rows: names(&["Refresh rate"]),
    }
}

/// The control protocol sets one key: a picture mode that the display gives
/// only with the refresh rate at Auto is refused, says so, and changes
/// nothing; the quick menu sets both rows in one write.
#[test]
#[ignore = "tier 2"]
fn a_value_that_needs_the_other_row_at_auto_is_refused_by_the_protocol() {
    let name = test_name(
        module_path!(),
        "a_value_that_needs_the_other_row_at_auto_is_refused_by_the_protocol",
    );
    in_child(&name, |dir| {
        let mut rig = Rig::new(dir);
        let games = rig.paths.config().join("games").join("nes");
        std::fs::create_dir_all(&games).expect("the folder");
        std::fs::write(
            games.join("Test Game.toml"),
            "schema_version = 1\n\n[display]\nrefresh_policy = \"force_50\"\n",
        )
        .expect("the game settings");
        rig.boot(the_game());
        rig.frames(30);
        let file_before = game_file(&rig);
        let refused = rig.shell().set_from_protocol(
            "display.style",
            neptron_settings::Level::Game,
            &serde_json::Value::String("speed".to_owned()),
        );
        let message = refused.expect_err("refused");
        assert!(
            message.contains("only possible with display.refresh_policy = exact"),
            "{message}"
        );
        rig.frames(30);
        assert_eq!(rig.shell().config().text("display.style"), "auto");
        assert_eq!(game_file(&rig), file_before);
        // The quick menu offers it with the reset, and a pick writes both.
        to_video_page(&mut rig);
        let shown = options(&mut rig, "Picture mode");
        let speed = shown
            .iter()
            .find(|o| o.label == "Game speed")
            .expect("speed");
        assert!(speed.enabled);
        assert_eq!(speed.detail, pages::reset_tag("display.refresh_policy"));
        assert_eq!(
            speed.help,
            pages::mode_help(neptron_video::PictureStyle::Speed)
        );
        rig.focus("Picture mode");
        rig.tap(&[Control::A]);
        rig.focus("Game speed");
        rig.tap(&[Control::A]);
        rig.frames(30);
        assert_eq!(rig.shell().config().text("display.style"), "speed");
        assert_eq!(rig.shell().config().text("display.refresh_policy"), "exact");
        rig.until(600, "the write", |r| {
            let file = game_file(r);
            file.contains("style = \"speed\"") && file.contains("refresh_policy = \"exact\"")
        });
        rig.finish();
    });
}

/// A 15 kHz monitor shows one game line on one screen line, at one rate.
fn expected_15_khz() -> Walked {
    Walked {
        chosen: names(&[
            "Picture mode: Auto",
            "Picture mode: Sharp lines",
            "Picture mode: Game speed",
            "Picture mode: Fill",
            "Picture mode: Native",
            "Refresh rate: Auto",
            "Refresh rate: 50 Hz",
            "Refresh rate: 60 Hz",
        ]),
        greyed: names(&[
            "Picture mode: 2x",
            "Picture mode: 3x",
            "Picture mode: 4x",
            "Refresh rate: Double rate",
        ]),
        disabled_rows: Vec::new(),
    }
}

/// The DPI output of a Pi drives the same 15 kHz monitor: the same choices.
fn expected_pi_dpi() -> Walked {
    expected_15_khz()
}

/// A panel draws every picture mode. It keeps its own rates.
fn expected_panel() -> Walked {
    Walked {
        chosen: names(&[
            "Picture mode: Auto",
            "Picture mode: Sharp lines",
            "Picture mode: Game speed",
            "Picture mode: Fill",
            "Picture mode: Native",
            "Picture mode: 2x",
            "Picture mode: 3x",
            "Picture mode: 4x",
        ]),
        greyed: Vec::new(),
        disabled_rows: names(&["Refresh rate"]),
    }
}

/// A TV has one rate and one line count.
fn expected_tv() -> Walked {
    Walked {
        chosen: names(&[
            "Picture mode: Auto",
            "Picture mode: Sharp lines",
            "Picture mode: Fill",
            "Picture mode: Native",
        ]),
        greyed: names(&[
            "Picture mode: Game speed",
            "Picture mode: 2x",
            "Picture mode: 3x",
            "Picture mode: 4x",
        ]),
        disabled_rows: names(&["Refresh rate"]),
    }
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
            std::fs::write(
                &path,
                "schema_version = 1\n\n[display]\nstyle = \"4x\"\nscanlines = \"on\"\n",
            )
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
                    "Picture mode 4x: not possible with this screen and video output. Shown: "
                )),
                "{file}: {shown:?}"
            );
            assert!(
                shown.iter().any(|t| t.contains(
                    "No scanlines: they need each game line on two or more screen lines."
                )),
                "{file}: the scanlines of the picture shown: {shown:?}"
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

/// The control protocol obeys the rule of the menus: in a game, a picture
/// mode or a refresh choice that the display does not give is refused
/// with its reason, and the file on disk and the engine stay as they were.
#[test]
#[ignore = "tier 2"]
fn the_protocol_refuses_a_value_the_display_does_not_give() {
    let name = test_name(
        module_path!(),
        "the_protocol_refuses_a_value_the_display_does_not_give",
    );
    in_children(&name, 2, |dir, stage| {
        let mut rig = if stage == 0 {
            Rig::new(dir)
        } else {
            crate::tv::composite_rig(dir)
        };
        rig.boot(the_game());
        assert!(rig.shell().in_game());
        rig.frames(30);
        let (key, value, reason) = if stage == 0 {
            (
                "display.style",
                "4x",
                "not possible with this screen and video output",
            )
        } else {
            ("display.refresh_policy", "force_50", "a TV has one rate")
        };
        let file_before = game_file(&rig);
        let mode_before = rig.display.active_mode();
        let refused = rig.shell().set_from_protocol(
            key,
            neptron_settings::Level::Game,
            &serde_json::Value::String(value.to_owned()),
        );
        let message = refused.expect_err("refused");
        assert!(message.contains(reason), "{message}");
        rig.frames(30);
        assert_ne!(rig.shell().config().text(key), value);
        assert_eq!(game_file(&rig), file_before);
        assert_eq!(rig.display.active_mode(), mode_before);
        rig.finish();
    });
}

/// On Settings > Display with a stored Double rate that a 15 kHz tube does
/// not take for any game, a picture mode is given only with the refresh
/// rate at Auto: its tag says so, its help line keeps its own text, and a
/// pick writes both keys at the global level, with a notice that names the
/// reset.
#[test]
#[ignore = "tier 2"]
fn a_pick_on_settings_display_that_also_sets_the_refresh_rate() {
    let name = test_name(
        module_path!(),
        "a_pick_on_settings_display_that_also_sets_the_refresh_rate",
    );
    in_child(&name, |dir| {
        let mut rig = Rig::new(dir);
        std::fs::create_dir_all(rig.paths.config()).expect("the folder");
        std::fs::write(
            rig.paths.config().join("global.toml"),
            "schema_version = 1\n\n[display]\nrefresh_policy = \"exact_x2\"\n",
        )
        .expect("the global settings");
        rig.boot(None);
        rig.until(600, "the scan", |r| {
            !r.shell().snapshot().systems.is_empty()
        });
        to_settings_display(&mut rig);
        let shown = options(&mut rig, "Picture mode");
        let sharp = shown
            .iter()
            .find(|o| o.label == "Sharp lines")
            .expect("sharp");
        assert!(sharp.enabled);
        assert_eq!(sharp.detail, pages::reset_tag("display.refresh_policy"));
        assert_eq!(
            sharp.help,
            pages::mode_help(neptron_video::PictureStyle::Sharp)
        );
        let notices = rig.shell().snapshot().notices.len();
        rig.focus("Picture mode");
        rig.tap(&[Control::A]);
        rig.focus("Sharp lines");
        rig.tap(&[Control::A]);
        rig.frames(10);
        if rig.screen() == "confirm" {
            rig.focus("Keep");
            rig.tap(&[Control::A]);
            rig.frames(10);
        }
        assert_eq!(rig.shell().config().text("display.style"), "sharp");
        assert_eq!(rig.shell().config().text("display.refresh_policy"), "exact");
        rig.until(600, "the write of the global settings", |r| {
            let file = global_file(r);
            file.contains("style = \"sharp\"") && file.contains("refresh_policy = \"exact\"")
        });
        let new: Vec<String> = rig
            .shell()
            .snapshot()
            .notices
            .iter()
            .skip(notices)
            .map(|n| n.text.clone())
            .collect();
        assert!(
            new.contains(&"Refresh rate is set to Auto too, for all games.".to_owned()),
            "{new:?}"
        );
        rig.finish();
    });
}

/// A pick of the refresh rate keeps the expert keys of its level: the
/// menu judged the pair with them, so the engine runs that pair. Only a
/// pick of the picture mode removes them.
#[test]
#[ignore = "tier 2"]
fn a_refresh_pick_keeps_the_expert_keys_at_each_level() {
    let name = test_name(
        module_path!(),
        "a_refresh_pick_keeps_the_expert_keys_at_each_level",
    );
    in_child(&name, |dir| {
        use neptron_settings::Level;
        let mut rig = Rig::new(dir);
        let config = rig.paths.config();
        let expert = "schema_version = 1\n\n[display]\nvertical = \"fit\"\n";
        for file in [
            "global.toml",
            "systems/nes.toml",
            "games/nes/Test Game.toml",
        ] {
            let path = config.join(file);
            std::fs::create_dir_all(path.parent().expect("a parent")).expect("the folder");
            std::fs::write(&path, expert).expect("the settings");
        }
        rig.boot(the_game());
        rig.frames(30);
        for level in [Level::Global, Level::System, Level::Game] {
            rig.shell()
                .set_from_protocol(
                    "display.refresh_policy",
                    level,
                    &serde_json::Value::String("exact".to_owned()),
                )
                .expect("the pick");
            rig.frames(10);
            let config = rig.shell().config();
            assert!(config.get(level, "display.refresh_policy").is_some());
            assert!(
                config.get(level, "display.vertical").is_some(),
                "{level:?}: the expert key stays"
            );
            let judged = config.policy().expect("the settings");
            let engine = *rig.shell().engine().expect("the engine").odren().settings();
            assert_eq!(engine, judged, "{level:?}: the engine runs the pair judged");
        }

        // A pick of the picture mode removes the key of its level only.
        rig.shell()
            .set_from_protocol(
                "display.style",
                Level::Game,
                &serde_json::Value::String("sharp".to_owned()),
            )
            .expect("the pick");
        rig.frames(10);
        let config = rig.shell().config();
        assert_eq!(config.get(Level::Game, "display.vertical"), None);
        assert!(config.get(Level::System, "display.vertical").is_some());
        rig.finish();
        for file in ["global.toml", "systems/nes.toml"] {
            let text = std::fs::read_to_string(config_path(&rig, file)).expect("the file");
            assert!(text.contains("vertical = \"fit\""), "{file}: {text}");
            assert!(
                text.contains("refresh_policy = \"exact\""),
                "{file}: {text}"
            );
        }
        assert!(!game_file(&rig).contains("vertical"), "{}", game_file(&rig));
    });
}

fn config_path(rig: &Rig, file: &str) -> std::path::PathBuf {
    rig.paths.config().join(file)
}
