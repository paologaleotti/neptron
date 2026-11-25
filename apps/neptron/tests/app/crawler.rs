//! The UI crawler: it walks every menu path with the D-pad, accept and
//! back only, and reads the UI tree of the control protocol. It fails on a
//! dead end, on clipped text of any row, on a row outside the
//! overscan-safe area, on a schema setting it could not reach, and on a
//! setting row whose disabled state does not follow the schema's
//! `applies_to` for the display of the run. A TV setting must show on a
//! TV and nowhere else.
//!
//! It also checks the row rule of the menus: a row that opens a screen has
//! the kind `page` (the arrow), an enabled `page` row opens a screen, and
//! back from a screen gives the focus back to the row that opened it. The
//! control protocol does not carry the kind of a row, so the crawler reads
//! it from the tree of the frame.

use std::collections::BTreeSet;

use neptron::NextRun;
use neptron::pages::{
    FORMAT_SHARE_LABEL, NOT_USED_ON_A_PANEL, NOT_USED_ON_A_TV, REBOOT_LABEL, RESCAN_LABEL,
    SHUT_DOWN_LABEL,
};
use neptron_settings::{DisplayClass, Schema};
use neptron_types::Control;
use neptron_ui::{Form, RowKind, Size};

use crate::panel::{PANEL_1080P, sysfs_with_panel};
use crate::rig::{Rig, in_child, test_name};

/// Screens the crawler records and does not enter further: a game list
/// (accept launches), the options of a row (accept changes the value),
/// the grid pattern, and the quick menu's keep prompt.
const LEAVES: [&str; 4] = ["games", "options", "pattern/grid", "quick-menu/keep"];

/// Rows the crawler never accepts: they end the menu or the run, or, for
/// the rescan row, open a notice when the scan ends, which takes the next
/// press of the crawl. `rescan.rs` presses that row. `Reboot` and `Shut
/// down` open a confirm question that is no screen of the stack, and a
/// confirm ends the run; `system.rs` presses them, and the row that asks
/// for the format of the share partition.
const NEVER_ACCEPT: [&str; 6] = [
    "Resume",
    "Quit game",
    RESCAN_LABEL,
    REBOOT_LABEL,
    SHUT_DOWN_LABEL,
    FORMAT_SHARE_LABEL,
];

/// Action rows that open a dialog over the screen, which back closes:
/// the guided mapping of a controller.
const OPENS_A_DIALOG: [&str; 1] = [neptron::pad_page::MAP_BUTTONS_LABEL];

/// The deepest path from home or the quick menu.
const DEPTH_MAX: usize = 4;

struct Found {
    /// The class of the display of the rig.
    class: DisplayClass,
    screens: BTreeSet<String>,
    labels: BTreeSet<String>,
    /// The labels of the setting rows shown as disabled.
    disabled: BTreeSet<String>,
    /// The forms of the canvas the crawl saw.
    forms: BTreeSet<String>,
}

#[test]
#[ignore = "tier 2"]
fn every_screen_and_every_setting_is_reachable() {
    let name = test_name(
        module_path!(),
        "every_screen_and_every_setting_is_reachable",
    );
    in_child(&name, |dir| {
        // The 15 kHz safe start: a CRT.
        let found = crawl_all(Rig::new(dir), DisplayClass::Crt);
        assert_eq!(found.disabled, BTreeSet::new());
        assert_eq!(found.forms, [format!("{:?}", Form::Landscape)].into());
    });
}

#[test]
#[ignore = "tier 2"]
fn on_a_flat_panel_the_rows_it_ignores_are_disabled() {
    let name = test_name(
        module_path!(),
        "on_a_flat_panel_the_rows_it_ignores_are_disabled",
    );
    in_child(&name, |dir| {
        let mut rig = Rig::new(dir);
        sysfs_with_panel(&rig.paths.sysfs, PANEL_1080P);
        rig.output = "HDMI-A-1".to_owned();
        let found = crawl_all(rig, DisplayClass::FlatPanel);
        let expected: BTreeSet<String> = ["Refresh rate"].map(str::to_owned).into();
        assert_eq!(found.disabled, expected, "the picture mode applies");
    });
}

#[test]
#[ignore = "tier 2"]
fn on_a_tv_the_tv_rows_show_and_the_monitor_and_refresh_rows_are_disabled() {
    let name = test_name(
        module_path!(),
        "on_a_tv_the_tv_rows_show_and_the_monitor_and_refresh_rows_are_disabled",
    );
    in_child(&name, |dir| {
        let found = crawl_all(crate::tv::composite_rig(dir), DisplayClass::Tv);
        let expected: BTreeSet<String> = ["Monitor profile", "Refresh rate"]
            .map(str::to_owned)
            .into();
        assert_eq!(found.disabled, expected);
    });
}

/// Writes the screen orientation `degrees` to the global settings of
/// `rig`.
pub(crate) fn set_orientation(rig: &Rig, degrees: &str) {
    let config = rig.paths.config();
    std::fs::create_dir_all(&config).expect("the config folder");
    std::fs::write(
        config.join("global.toml"),
        format!("schema_version = 1\n\n[display]\norientation = \"{degrees}\"\n"),
    )
    .expect("the settings");
}

#[test]
#[ignore = "tier 2"]
fn on_a_turned_crt_every_screen_and_every_setting_is_reachable_in_portrait() {
    let name = test_name(
        module_path!(),
        "on_a_turned_crt_every_screen_and_every_setting_is_reachable_in_portrait",
    );
    in_child(&name, |dir| {
        let rig = Rig::new(dir);
        set_orientation(&rig, "90");
        let found = crawl_all(rig, DisplayClass::Crt);
        assert_eq!(found.disabled, BTreeSet::new());
        assert_eq!(found.forms, [format!("{:?}", Form::Portrait)].into());
    });
}

#[test]
#[ignore = "tier 2"]
fn on_a_turned_flat_panel_every_screen_and_every_setting_is_reachable_in_portrait() {
    let name = test_name(
        module_path!(),
        "on_a_turned_flat_panel_every_screen_and_every_setting_is_reachable_in_portrait",
    );
    in_child(&name, |dir| {
        let mut rig = Rig::new(dir);
        sysfs_with_panel(&rig.paths.sysfs, PANEL_1080P);
        rig.output = "HDMI-A-1".to_owned();
        set_orientation(&rig, "270");
        let found = crawl_all(rig, DisplayClass::FlatPanel);
        let expected: BTreeSet<String> = ["Refresh rate"].map(str::to_owned).into();
        assert_eq!(found.disabled, expected);
        assert_eq!(found.forms, [format!("{:?}", Form::Portrait)].into());
    });
}

#[test]
#[ignore = "tier 2"]
fn with_a_format_offer_the_row_is_found_and_never_accepted() {
    let name = test_name(
        module_path!(),
        "with_a_format_offer_the_row_is_found_and_never_accepted",
    );
    in_child(&name, |dir| {
        let mut rig = Rig::new(dir);
        crate::system::storage_status(&rig, crate::system::OFFER);
        rig.boot(None);
        rig.frames(2);
        for _ in 0..8 {
            if rig.screen() != "notice" {
                break;
            }
            rig.tap(&[Control::A]);
        }
        rig.until(600, "the scan", |r| {
            !r.shell().snapshot().systems.is_empty()
        });
        let mut found = Found {
            class: DisplayClass::Crt,
            screens: BTreeSet::new(),
            labels: BTreeSet::new(),
            disabled: BTreeSet::new(),
            forms: BTreeSet::new(),
        };
        // The frames of the crawl panic when the run ends.
        crawl(&mut rig, &mut found, 0);
        rig.finish();
        assert!(
            found.labels.contains(FORMAT_SHARE_LABEL),
            "the format row is on no screen: {:?}",
            found.labels
        );
        assert!(!rig.paths.share_format_request().exists());
    });
}

/// Boots `rig` into the list and into a game, crawls both, and checks that
/// every screen and every setting was reached.
fn crawl_all(mut rig: Rig, class: DisplayClass) -> Found {
    // A pad, so the page of a controller and its guided mapping are on
    // the path.
    rig.plug(crate::players::pad(1));
    rig.boot(None);
    rig.frames(2);
    rig.until(600, "the scan", |r| {
        !r.shell().snapshot().systems.is_empty()
    });
    let mut found = Found {
        class,
        screens: BTreeSet::new(),
        labels: BTreeSet::new(),
        disabled: BTreeSet::new(),
        forms: BTreeSet::new(),
    };
    crawl(&mut rig, &mut found, 0);
    rig.finish();

    let game = neptron::relaunch::GameRef {
        system: "nes".to_owned(),
        path: "nes/Test Game.nes".to_owned(),
    };
    rig.boot(Some(NextRun {
        pads: Vec::new(),
        game: Some(game),
        notice: None,
    }));
    rig.frames(30);
    rig.open_menu();
    rig.frames(1);
    // A horizontal game across a turned 15 kHz screen shows interlaced,
    // and its one-time notice waits for the menu.
    crate::tv::dismiss_notices(&mut rig);
    assert_eq!(rig.screen(), "quick-menu");
    crawl(&mut rig, &mut found, 0);
    rig.finish();

    for screen in [
        "home",
        "games",
        "settings",
        "display",
        "audio",
        "input",
        "library",
        "network",
        "system",
        "options",
        "pattern/grid",
        "quick-menu",
        "quick-menu/video",
        "pad-mapping",
    ] {
        assert!(
            found.screens.contains(screen),
            "{screen} not reached: {:?}",
            found.screens
        );
    }
    for label in [RESCAN_LABEL, REBOOT_LABEL, SHUT_DOWN_LABEL] {
        assert!(
            found.labels.contains(label),
            "the row {label} is on no screen"
        );
    }
    for entry in Schema::builtin().entries() {
        assert_eq!(
            found.labels.contains(entry.label),
            entry.shown(class),
            "the setting {} ({}) is on a screen only where it shows",
            entry.key,
            entry.label
        );
    }
    found
}

/// The kind of the focused row, from the tree of the last frame.
fn focused_kind(rig: &mut Rig) -> Option<RowKind> {
    rig.shell().ui().tree.focused().and_then(|r| r.kind)
}

/// Records the screen on show, then enters each of its rows and comes back
/// with back. The rows that scroll into view are recorded as the focus
/// moves down.
fn crawl(rig: &mut Rig, found: &mut Found, depth: usize) {
    let screen = record(rig, found);
    if depth == DEPTH_MAX || LEAVES.contains(&screen.as_str()) {
        return;
    }
    let mut visited = BTreeSet::new();
    for _ in 0..32 {
        let Some(focused) = rig.focused() else {
            return;
        };
        if !visited.insert(focused.clone()) {
            return;
        }
        if !NEVER_ACCEPT.contains(&focused.as_str()) {
            let kind = focused_kind(rig);
            rig.tap(&[Control::A]);
            let entered = rig.screen();
            if entered != screen {
                if !OPENS_A_DIALOG.contains(&focused.as_str()) {
                    assert_eq!(
                        kind,
                        Some(RowKind::Page),
                        "{screen}: the row {focused:?} opens {entered} but has no arrow"
                    );
                }
                crawl(rig, found, depth + 1);
                rig.tap(&[Control::B]);
                assert_eq!(
                    rig.screen(),
                    screen,
                    "back from {entered} did not return to {screen}: a dead end"
                );
                assert_eq!(
                    rig.focused().as_deref(),
                    Some(focused.as_str()),
                    "back from {entered} did not give the focus back to its row"
                );
            } else {
                assert_ne!(
                    kind,
                    Some(RowKind::Page),
                    "{screen}: the row {focused:?} has an arrow and opens nothing"
                );
            }
        }
        rig.tap(&[Control::Down]);
        record(rig, found);
    }
}

/// Records the screen on show and its visible rows, and checks each row:
/// inside the safe area of the canvas of the tree, not clipped, and
/// disabled exactly where its setting does not apply. Returns the screen.
fn record(rig: &mut Rig, found: &mut Found) -> String {
    let tree = rig.tree();
    let canvas = tree.canvas.expect("the tree names its canvas");
    let form = Size::new(canvas.width, canvas.height)
        .map(Size::form)
        .expect("the canvas has the size of a form");
    found.forms.insert(format!("{form:?}"));
    let safe = form.safe_area();
    let screen = tree.screen.clone();
    found.screens.insert(screen.clone());
    for row in &tree.rows {
        found.labels.insert(row.label.clone());
        let b = row.bounds;
        let inside = b.x >= safe.x
            && b.y >= safe.y
            && b.x + b.w as i32 <= safe.x + safe.w as i32
            && b.y + b.h as i32 <= safe.y + safe.h as i32;
        assert!(
            inside,
            "{screen} ({form:?}): the row {:?} is outside the safe area",
            row.label
        );
        assert!(
            !row.clipped,
            "{screen} ({form:?}): the row {:?} is clipped: {row:?}",
            row.label
        );
        let schema = Schema::builtin();
        if let Some(entry) = row.setting.as_deref().and_then(|key| schema.get(key)) {
            let disabled =
                [Some(NOT_USED_ON_A_PANEL), Some(NOT_USED_ON_A_TV)].contains(&row.value.as_deref());
            assert_eq!(
                disabled,
                !entry.applies_to.applies(found.class),
                "{screen}: the row {:?} does not follow applies_to: {row:?}",
                row.label
            );
            if disabled {
                found.disabled.insert(row.label.clone());
            }
        }
    }
    screen
}
