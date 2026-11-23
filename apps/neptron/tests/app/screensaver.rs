//! The screensaver: it starts after the set time with no input in the
//! menus and in a paused game, never in a running game or under a dialog
//! with a timeout, and the input that wakes it does nothing else.

use neptron_backend::Clock;
use neptron_settings::Level;
use neptron_types::Control;
use neptron_ui::screensaver::SCREENSAVER_SCREEN;
use serde_json::Value;

use crate::pause::in_game;
use crate::rig::{Rig, in_child, test_name};

const MINUTE_NS: u64 = 60_000_000_000;

/// The default time of the setting.
const DEFAULT_NS: u64 = 5 * MINUTE_NS;

/// The most the start may come after the set time: a few frames.
const LATE_MAX_NS: u64 = 100_000_000;

/// Frames that cover a little more than `ns` at the 60 Hz of the fakes.
fn frames_for(ns: u64) -> u32 {
    u32::try_from(ns / 16_000_000 + 600).expect("a frame count")
}

/// True while the snapshot shows the screensaver.
fn shows(rig: &mut Rig) -> bool {
    rig.shell().snapshot().screensaver.is_some()
}

/// Runs frames until the screensaver shows, and checks that it starts
/// `timeout_ns` after `since_ns`, not before and only a few frames after.
fn idle_until_it_shows(rig: &mut Rig, since_ns: u64, timeout_ns: u64) {
    rig.until(frames_for(timeout_ns), "the screensaver", |r| {
        let now = r.shell().now_ns();
        let shown = shows(r);
        assert!(
            !shown || now - since_ns >= timeout_ns,
            "it shows {} ns after the input",
            now - since_ns
        );
        shown
    });
    let late = rig.shell().now_ns() - since_ns - timeout_ns;
    assert!(late < LATE_MAX_NS, "{late} ns late");
    assert_eq!(rig.screen(), SCREENSAVER_SCREEN);
}

fn set(rig: &mut Rig, key: &str, value: Value) {
    rig.shell()
        .set_from_protocol(key, Level::Global, &value)
        .expect("the change");
    rig.frames(2);
}

#[test]
#[ignore = "tier 2"]
fn in_the_menus_it_shows_at_the_set_time_and_the_wake_does_nothing_else() {
    let name = test_name(
        module_path!(),
        "in_the_menus_it_shows_at_the_set_time_and_the_wake_does_nothing_else",
    );
    in_child(&name, |dir| {
        let mut rig = Rig::new(dir);
        rig.boot(None);
        rig.scanned();
        rig.tap(&[Control::Down]);
        let last_press = rig.clock.now_ns();
        rig.tap(&[Control::Up]);
        let screen = rig.screen();
        let focus = rig.focused();
        idle_until_it_shows(&mut rig, last_press, DEFAULT_NS);
        rig.frames(120);
        assert!(shows(&mut rig), "it stays with no input");

        // An accept on the focused row only wakes the screensaver: the row
        // does not open.
        rig.tap(&[Control::A]);
        assert!(!shows(&mut rig));
        assert_eq!(rig.screen(), screen);
        assert_eq!(rig.focused(), focus);
        rig.frames(60);
        assert_eq!(rig.screen(), screen, "the release leaks nothing");

        // A direction held across the wake gives no move and no repeat.
        rig.tap(&[Control::Down]);
        let last_press = rig.clock.now_ns();
        rig.tap(&[Control::Up]);
        assert_eq!(rig.focused(), focus);
        idle_until_it_shows(&mut rig, last_press, DEFAULT_NS);
        rig.hold(&[Control::Down]);
        rig.frames(60);
        assert!(!shows(&mut rig));
        rig.release(&[Control::Down]);
        rig.frames(2);
        assert_eq!(rig.focused(), focus, "no move and no repeat");

        // The next press acts as always.
        rig.tap(&[Control::A]);
        assert_ne!(rig.screen(), screen, "a second press opens the row");
        rig.finish();
    });
}

#[test]
#[ignore = "tier 2"]
fn off_never_shows_and_a_confirm_holds_it_back() {
    let name = test_name(
        module_path!(),
        "off_never_shows_and_a_confirm_holds_it_back",
    );
    in_child(&name, |dir| {
        let mut rig = Rig::new(dir);
        rig.boot(None);
        rig.scanned();
        set(&mut rig, "display.screensaver", Value::Bool(false));
        for _ in 0..frames_for(2 * DEFAULT_NS) {
            rig.frames(1);
            assert!(!shows(&mut rig));
        }
        set(&mut rig, "display.screensaver", Value::Bool(true));
        set(
            &mut rig,
            "display.screensaver_minutes",
            Value::String("1".to_owned()),
        );
        assert!(rig.state_file("config/global.toml").contains("screensaver"));

        set(
            &mut rig,
            "display.monitor",
            Value::String("arcade_31".to_owned()),
        );
        assert_eq!(rig.screen(), "confirm");
        while rig.shell().guard().pending().is_some() {
            rig.frames(1);
            assert!(!shows(&mut rig), "the confirm and its timeout stay");
        }
        let ended = rig.shell().now_ns();
        idle_until_it_shows(&mut rig, ended, MINUTE_NS);
        rig.finish();
    });
}

#[test]
#[ignore = "tier 2"]
fn a_running_game_never_shows_it_and_a_paused_game_does() {
    let name = test_name(
        module_path!(),
        "a_running_game_never_shows_it_and_a_paused_game_does",
    );
    in_child(&name, |dir| {
        let mut rig = in_game(dir);
        set(
            &mut rig,
            "display.screensaver_minutes",
            Value::String("1".to_owned()),
        );
        for _ in 0..frames_for(2 * MINUTE_NS) {
            rig.frames(1);
            assert!(!shows(&mut rig));
        }
        assert_eq!(rig.screen(), "game");

        rig.open_menu();
        assert!(rig.shell().paused());
        assert_eq!(rig.screen(), "quick-menu");
        rig.tap(&[Control::Down]);
        let last_press = rig.clock.now_ns();
        rig.tap(&[Control::Up]);
        let focus = rig.focused();
        idle_until_it_shows(&mut rig, last_press, MINUTE_NS);
        rig.frames(2);
        let last = rig.display.frames().last().expect("a frame").clone();
        assert!(last.scene.ui.is_some());
        assert!(last.scene.game.is_none(), "no frozen game picture");

        // Accept on Resume only wakes the screensaver: the game stays
        // paused under the menu.
        rig.tap(&[Control::A]);
        assert!(!shows(&mut rig));
        assert!(rig.shell().paused());
        assert_eq!(rig.screen(), "quick-menu");
        assert_eq!(rig.focused(), focus);
        rig.frames(2);
        let last = rig.display.frames().last().expect("a frame").clone();
        assert!(last.scene.game.is_some(), "the game picture is back");
        rig.finish();
    });
}

#[test]
#[ignore = "tier 2"]
fn a_held_direction_holds_it_off() {
    let name = test_name(module_path!(), "a_held_direction_holds_it_off");
    in_child(&name, |dir| {
        let mut rig = Rig::new(dir);
        rig.boot(None);
        rig.scanned();
        set(
            &mut rig,
            "display.screensaver_minutes",
            Value::String("1".to_owned()),
        );
        // The key repeat makes actions with no new event.
        rig.hold(&[Control::Down]);
        for _ in 0..frames_for(2 * MINUTE_NS) {
            rig.frames(1);
            assert!(!shows(&mut rig));
        }
        rig.release(&[Control::Down]);
        let last_repeat = rig.clock.now_ns() - neptron::nav::REPEAT_INTERVAL_NS;
        idle_until_it_shows(&mut rig, last_repeat, MINUTE_NS);
        rig.finish();
    });
}
