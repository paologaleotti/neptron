//! The UI crawler: it walks every menu path with the D-pad, accept and
//! back only, and reads the UI tree of the control protocol. It fails on a
//! dead end, on clipped text of any row, on a row outside the
//! overscan-safe area, on a schema setting it could not reach, and on a
//! setting row whose disabled state does not follow the schema's
//! `applies_to` for the display of the run.
//!
//! It also checks the row rule of the menus: a row that opens a screen has
//! the kind `page` (the arrow), an enabled `page` row opens a screen, and
//! back from a screen gives the focus back to the row that opened it. The
//! control protocol does not carry the kind of a row, so the crawler reads
//! it from the tree of the frame.

use std::collections::BTreeSet;

use neptron::NextRun;
use neptron::pages::{NOT_USED_ON_A_PANEL, RESCAN_LABEL};
use neptron_settings::{DisplayClass, Schema};
use neptron_types::Control;
use neptron_ui::RowKind;
use neptron_ui::theme::SAFE_AREA;

use crate::panel::{PANEL_1080P, sysfs_with_panel};
use crate::rig::{Rig, in_child, test_name};

/// Screens the crawler records and does not enter further: a game list
/// (accept launches), the options of a row (accept changes the value),
/// the grid pattern, and the quick menu's keep prompt.
const LEAVES: [&str; 4] = ["games", "options", "pattern/grid", "quick-menu/keep"];

/// Rows the crawler never accepts: they end the menu or the run, or, for
/// the rescan row, open a notice when the scan ends, which takes the next
/// press of the crawl. `rescan.rs` presses that row.
const NEVER_ACCEPT: [&str; 3] = ["Resume", "Quit game", RESCAN_LABEL];

/// The deepest path from home or the quick menu.
const DEPTH_MAX: usize = 4;

struct Found {
    /// The class of the display of the rig.
    class: DisplayClass,
    screens: BTreeSet<String>,
    labels: BTreeSet<String>,
    /// The labels of the setting rows shown as disabled.
    disabled: BTreeSet<String>,
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
        let expected: BTreeSet<String> = ["Picture style", "Refresh policy"]
            .map(str::to_owned)
            .into();
        assert_eq!(found.disabled, expected);
    });
}

/// Boots `rig` into the list and into a game, crawls both, and checks that
/// every screen and every setting was reached.
fn crawl_all(mut rig: Rig, class: DisplayClass) -> Found {
    rig.boot(None);
    rig.frames(2);
    rig.keep();
    rig.until(600, "the scan", |r| {
        !r.shell().snapshot().systems.is_empty()
    });
    let mut found = Found {
        class,
        screens: BTreeSet::new(),
        labels: BTreeSet::new(),
        disabled: BTreeSet::new(),
    };
    crawl(&mut rig, &mut found, 0);
    rig.finish();

    let game = neptron::relaunch::GameRef {
        system: "nes".to_owned(),
        path: "nes/Test Game.nes".to_owned(),
    };
    rig.boot(Some(NextRun {
        game: Some(game),
        notice: None,
    }));
    rig.frames(30);
    rig.tap(&[Control::Select, Control::Start]);
    rig.frames(1);
    assert_eq!(rig.screen(), "quick-menu");
    crawl(&mut rig, &mut found, 0);
    rig.finish();

    for screen in [
        "home",
        "games",
        "settings",
        "display",
        "audio",
        "library",
        "options",
        "pattern/grid",
        "quick-menu",
        "quick-menu/video",
    ] {
        assert!(
            found.screens.contains(screen),
            "{screen} not reached: {:?}",
            found.screens
        );
    }
    assert!(
        found.labels.contains(RESCAN_LABEL),
        "the rescan row is on no screen"
    );
    for entry in Schema::builtin().entries() {
        assert!(
            found.labels.contains(entry.label),
            "the setting {} ({}) is on no screen",
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
/// with back.
fn crawl(rig: &mut Rig, found: &mut Found, depth: usize) {
    let tree = rig.tree();
    let screen = tree.screen.clone();
    found.screens.insert(screen.clone());
    for row in &tree.rows {
        found.labels.insert(row.label.clone());
        let b = row.bounds;
        let inside = b.x >= SAFE_AREA.x
            && b.y >= SAFE_AREA.y
            && b.x + b.w as i32 <= SAFE_AREA.x + SAFE_AREA.w as i32
            && b.y + b.h as i32 <= SAFE_AREA.y + SAFE_AREA.h as i32;
        assert!(
            inside,
            "{screen}: the row {:?} is outside the safe area",
            row.label
        );
        assert!(
            !row.clipped,
            "{screen}: the row {:?} is clipped: {row:?}",
            row.label
        );
        let schema = Schema::builtin();
        if let Some(entry) = row.setting.as_deref().and_then(|key| schema.get(key)) {
            let disabled = row.value.as_deref() == Some(NOT_USED_ON_A_PANEL);
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
                assert_eq!(
                    kind,
                    Some(RowKind::Page),
                    "{screen}: the row {focused:?} opens {entered} but has no arrow"
                );
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
    }
}
