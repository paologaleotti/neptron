//! The UI crawler: it walks every menu path with the D-pad, accept and
//! back only, and reads the UI tree of the control protocol. It fails on a
//! dead end, on clipped text of any row, on a row outside the
//! overscan-safe area, on a schema setting it could not reach, and on a
//! setting row whose disabled state does not follow the schema's
//! `applies_to` for the display of the run.

use std::collections::BTreeSet;

use neptron::NextRun;
use neptron::pages::NOT_USED_ON_A_PANEL;
use neptron_settings::{DisplayClass, Schema};
use neptron_types::Control;
use neptron_ui::theme::SAFE_AREA;

use crate::rig::{Rig, in_child, test_name};

/// Screens the crawler records and does not enter further: a game list
/// (accept launches), the options of a row (accept changes the value),
/// the grid pattern, and the quick menu's keep prompt.
const LEAVES: [&str; 4] = ["games", "options", "pattern/grid", "quick-menu/keep"];

/// Rows the crawler never accepts: they end the menu or the run.
const NEVER_ACCEPT: [&str; 2] = ["Resume", "Quit game"];

/// The class of the display of the rig: the 15 kHz safe start, a CRT.
const CLASS: DisplayClass = DisplayClass::Crt;

/// The deepest path from home or the quick menu.
const DEPTH_MAX: usize = 4;

#[derive(Default)]
struct Found {
    screens: BTreeSet<String>,
    labels: BTreeSet<String>,
}

#[test]
#[ignore = "tier 2"]
fn every_screen_and_every_setting_is_reachable() {
    in_child(
        &test_name(
            module_path!(),
            "every_screen_and_every_setting_is_reachable",
        ),
        |dir| {
            let mut rig = Rig::new(dir);
            rig.boot(None);
            rig.frames(2);
            rig.keep();
            rig.until(600, "the scan", |r| {
                !r.shell().snapshot().systems.is_empty()
            });
            let mut found = Found::default();
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
                "display",
                "options",
                "pattern/grid",
                "quick-menu",
            ] {
                assert!(
                    found.screens.contains(screen),
                    "{screen} not reached: {:?}",
                    found.screens
                );
            }
            for entry in Schema::builtin().entries() {
                assert!(
                    found.labels.contains(entry.label),
                    "the setting {} ({}) is on no screen",
                    entry.key,
                    entry.label
                );
            }
        },
    );
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
            assert_eq!(
                row.value.as_deref() == Some(NOT_USED_ON_A_PANEL),
                !entry.applies_to.applies(CLASS),
                "{screen}: the row {:?} does not follow applies_to: {row:?}",
                row.label
            );
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
            rig.tap(&[Control::A]);
            let entered = rig.screen();
            if entered != screen {
                crawl(rig, found, depth + 1);
                rig.tap(&[Control::B]);
                assert_eq!(
                    rig.screen(),
                    screen,
                    "back from {entered} did not return to {screen}: a dead end"
                );
                rig.focus(&focused);
            }
        }
        rig.tap(&[Control::Down]);
    }
}
