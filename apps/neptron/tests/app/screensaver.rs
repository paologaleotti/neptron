//! The screensaver: it starts after the set time with no input in the
//! menus and in a paused game, never in a running game or under a dialog
//! with a timeout, and the input that wakes it does nothing else.

use neptron_backend::Clock;
use neptron_settings::Level;
use neptron_types::Control;
use neptron_ui::screensaver::{SCREENSAVER_SCREEN, pose};
use neptron_ui::{UI_HEIGHT, UI_WIDTH};
use serde_json::Value;

use crate::pause::in_game;
use crate::players::pad;
use crate::remapping::{PAD_ANSWERS, answer, map_buttons, open_pad_page, review};
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

/// Presses `control` for the frames until the screensaver sees the press
/// (`seen_frames`: 1 in the game list, 2 in a game, where the UI reads the
/// events of the frame before), and returns the time of that frame, then
/// releases it, as [`Rig::tap`] does.
fn press(rig: &mut Rig, control: Control, seen_frames: u32) -> u64 {
    rig.hold(&[control]);
    rig.frames(seen_frames);
    let seen = rig.shell().now_ns();
    rig.release(&[control]);
    rig.frames(3 - seen_frames);
    seen
}

/// Runs `n` frames under the screensaver and checks that each one shows
/// the next step, a new pose, and draws the canvas once.
fn each_frame_is_one_step(rig: &mut Rig, n: u32) {
    let mut step = rig.shell().snapshot().screensaver.expect("the screensaver");
    let mut paints = rig.shell().canvas().paints();
    for _ in 0..n {
        rig.frames(1);
        let next = rig.shell().snapshot().screensaver.expect("the screensaver");
        assert_eq!(next, step + 1, "one step per frame");
        let (a, b) = (
            pose(step, UI_WIDTH, UI_HEIGHT),
            pose(next, UI_WIDTH, UI_HEIGHT),
        );
        assert_eq!(a.x.abs_diff(b.x), 1, "{a:?} {b:?}");
        assert_eq!(a.y.abs_diff(b.y), 1, "{a:?} {b:?}");
        let now = rig.shell().canvas().paints();
        assert_eq!(now, paints + 1, "the canvas is drawn once per frame");
        (step, paints) = (next, now);
    }
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
        let last_press = press(&mut rig, Control::Up, 1);
        let screen = rig.screen();
        let focus = rig.focused();
        idle_until_it_shows(&mut rig, last_press, DEFAULT_NS);
        each_frame_is_one_step(&mut rig, 600);
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
        let last_press = press(&mut rig, Control::Up, 1);
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

        // Close to the time with no input, a confirm with its timeout
        // opens: it holds the screensaver back past the minute.
        let on = rig.shell().now_ns();
        rig.until(frames_for(MINUTE_NS), "59 s", |r| {
            assert!(!shows(r));
            r.shell().now_ns() - on >= 59_000_000_000
        });
        set(
            &mut rig,
            "display.monitor",
            Value::String("arcade_31".to_owned()),
        );
        assert_eq!(rig.screen(), "confirm");
        let mut last_confirm = rig.shell().now_ns();
        while rig.shell().guard().pending().is_some() {
            last_confirm = rig.shell().now_ns();
            rig.frames(1);
            assert!(!shows(&mut rig), "the confirm and its timeout stay");
        }
        assert!(last_confirm - on > MINUTE_NS + 10_000_000_000);
        idle_until_it_shows(&mut rig, last_confirm, MINUTE_NS);
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
        let last_press = press(&mut rig, Control::Up, 2);
        let focus = rig.focused();
        idle_until_it_shows(&mut rig, last_press, MINUTE_NS);
        each_frame_is_one_step(&mut rig, 300);
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

#[test]
#[ignore = "tier 2"]
fn a_guided_mapping_holds_it_off_and_the_time_counts_from_its_end() {
    let name = test_name(
        module_path!(),
        "a_guided_mapping_holds_it_off_and_the_time_counts_from_its_end",
    );
    in_child(&name, |dir| {
        let mut rig = Rig::new(dir);
        rig.boot(None);
        let one = rig.plug(pad(1));
        rig.frames(2);
        open_pad_page(&mut rig, "Player 1");
        set(
            &mut rig,
            "display.screensaver_minutes",
            Value::String("1".to_owned()),
        );
        map_buttons(&mut rig);
        let start = rig.shell().now_ns();
        // Each answer comes before the idle timeout of the mapping, and
        // the whole mapping takes longer than the screensaver time.
        for raw in PAD_ANSWERS {
            let asked = rig.shell().now_ns();
            rig.until(frames_for(4 * 1_000_000_000), "the next answer", |r| {
                assert!(!shows(r), "the mapping stays");
                r.shell().now_ns() - asked >= 3_500_000_000
            });
            answer(&mut rig, one, raw);
        }
        assert!(review(&mut rig).is_some());
        let mut last_mapping = rig.shell().now_ns();
        while rig.shell().snapshot().pad_mapping.is_some() {
            last_mapping = rig.shell().now_ns();
            rig.frames(1);
            assert!(!shows(&mut rig), "the review and its timeout stay");
        }
        assert!(last_mapping - start > MINUTE_NS);
        idle_until_it_shows(&mut rig, last_mapping, MINUTE_NS);
        rig.finish();
    });
}
