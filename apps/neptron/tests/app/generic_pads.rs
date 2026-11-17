//! A pad with no gamepad mapping: the generic layout, its notice and the
//! button that opens the Input panel; and the notice of a pad that sends
//! no input.

use neptron_backend::Clock;
use neptron_input::{DeviceIdentity, HatState, JoystickCounts, PadInfo, SILENT_PAD_AFTER_NS};
use neptron_types::Control;
use neptron_ui::snapshot::Notice;

use crate::players::{launch_from_the_list, pad, rows};
use crate::rig::{Rig, in_child, test_name};

/// The text of the notice of the generic layout of [`joystick`].
const GENERIC_TEXT: &str = "USB Joystick has no known layout, so it uses a generic one. Some buttons can be in other places.";

/// A PS3-type pad with no mapping: 13 buttons, a hat and 4 axes.
const FULL: JoystickCounts = JoystickCounts {
    buttons: 13,
    hats: 1,
    axes: 4,
};

fn joystick() -> PadInfo {
    PadInfo {
        identity: DeviceIdentity::from_parts(&[b"joystick"]),
        path: DeviceIdentity::from_parts(&[b"/dev/input/event9"]),
        name: "USB Joystick".to_owned(),
    }
}

fn notices(rig: &mut Rig) -> Vec<Notice> {
    rig.shell().snapshot().notices.clone()
}

/// Raw buttons of the PS3 order.
const CIRCLE: u8 = 2;
const SELECT: u8 = 8;
const START: u8 = 9;

#[test]
#[ignore = "tier 2"]
fn a_pad_with_no_mapping_plays_tells_once_and_opens_its_input_row() {
    let name = test_name(
        module_path!(),
        "a_pad_with_no_mapping_plays_tells_once_and_opens_its_input_row",
    );
    in_child(&name, |dir| {
        let mut rig = Rig::new(dir);
        rig.boot(None);
        rig.plug(pad(1));
        rig.frames(2);
        let stick = rig.plug_joystick(joystick(), FULL);
        rig.frames(2);
        let shown = notices(&mut rig);
        assert_eq!(shown.len(), 1, "{shown:?}");
        assert_eq!(shown[0].text, GENERIC_TEXT);
        assert_eq!(shown[0].choice.as_deref(), Some("Set up this controller"));
        assert_eq!(rig.screen(), "notice");

        // The joystick answers with its own hat and buttons: right to the
        // second button, Circle (A) to choose it.
        rig.pads.set_joy_hat(stick, HatState(HatState::RIGHT));
        rig.frames(1);
        rig.pads.set_joy_hat(stick, HatState::default());
        rig.frames(2);
        rig.joy_tap(stick, CIRCLE);
        rig.frames(2);
        assert_eq!(rig.screen(), "input");
        assert_eq!(rig.focused().as_deref(), Some("Player 2"));
        assert_eq!(
            rows(&mut rig)[..2],
            [
                ("Player 1".to_owned(), "Pad 1".to_owned()),
                (
                    "Player 2".to_owned(),
                    "USB Joystick (generic layout)".to_owned()
                ),
            ]
        );
        assert!(notices(&mut rig).is_empty());

        // The next run of the boot does not tell again, and in the game
        // the joystick plays on its port and opens the quick menu.
        rig.tap(&[Control::B]);
        rig.tap(&[Control::B]);
        assert_eq!(rig.screen(), "home");
        let next = launch_from_the_list(&mut rig);
        rig.finish();
        rig.boot(Some(next));
        assert!(rig.shell().in_game());
        rig.frames(30);
        assert!(notices(&mut rig).is_empty(), "told once");
        let stick = rig.joysticks[0].2;
        rig.pads.set_joy_button(stick, CIRCLE, true);
        rig.frames(1);
        let ports = rig.shell().ports().expect("ports");
        assert!(ports.is_held(1, Control::A), "Circle is A of player 2");
        rig.pads.set_joy_button(stick, CIRCLE, false);
        rig.frames(1);
        rig.pads.set_joy_button(stick, SELECT, true);
        rig.pads.set_joy_button(stick, START, true);
        rig.frames(1);
        rig.pads.set_joy_button(stick, START, false);
        rig.pads.set_joy_button(stick, SELECT, false);
        rig.frames(3);
        assert_eq!(rig.screen(), "quick-menu");
        rig.finish();
    });
}

#[test]
#[ignore = "tier 2"]
fn a_joystick_that_is_not_a_gamepad_is_told_once() {
    let name = test_name(
        module_path!(),
        "a_joystick_that_is_not_a_gamepad_is_told_once",
    );
    in_child(&name, |dir| {
        let mut rig = Rig::new(dir);
        rig.boot(None);
        let one_button = JoystickCounts {
            buttons: 1,
            hats: 0,
            axes: 2,
        };
        let mut info = joystick();
        info.name = "Volume Knob".to_owned();
        rig.plug_joystick(info, one_button);
        rig.frames(2);
        let texts: Vec<_> = notices(&mut rig)
            .into_iter()
            .map(|n| (n.text, n.choice))
            .collect();
        assert_eq!(
            texts,
            [(
                "Volume Knob is not used: it does not look like a gamepad.".to_owned(),
                None
            )]
        );
        assert!(rig.shell().roster().is_empty());
        rig.tap(&[Control::A]);
        assert!(notices(&mut rig).is_empty());
        rig.finish();
        rig.boot(None);
        rig.frames(5);
        assert!(
            notices(&mut rig).is_empty(),
            "the next run does not tell again"
        );
        rig.finish();
    });
}

#[test]
#[ignore = "tier 2"]
fn a_pad_that_sends_nothing_while_the_keyboard_plays_is_told_once() {
    let name = test_name(
        module_path!(),
        "a_pad_that_sends_nothing_while_the_keyboard_plays_is_told_once",
    );
    in_child(&name, |dir| {
        let mut rig = Rig::new(dir);
        rig.boot(None);
        rig.plug(pad(1));
        let used = rig.plug(pad(2));
        rig.frames(2);
        let plugged_ns = rig.clock.now_ns();
        rig.pads.set(used, neptron_input::PadButton::South, true);
        rig.frames(1);
        rig.pads.set(used, neptron_input::PadButton::South, false);
        for _ in 0..5 {
            rig.tap(&[Control::Down]);
            rig.tap(&[Control::Up]);
        }
        rig.until(5_000, "the silent pad notice", |r| !notices(r).is_empty());
        assert!(rig.clock.now_ns() - plugged_ns >= SILENT_PAD_AFTER_NS);
        let texts: Vec<_> = notices(&mut rig).into_iter().map(|n| n.text).collect();
        assert_eq!(
            texts,
            ["Pad 1 is connected but sends no input. Try another USB port or a USB hub."]
        );
        rig.tap(&[Control::A]);
        for _ in 0..10 {
            rig.tap(&[Control::Down]);
        }
        let later = rig.clock.now_ns() + 2 * SILENT_PAD_AFTER_NS;
        rig.until(10_000, "more time", |r| r.clock.now_ns() >= later);
        assert!(notices(&mut rig).is_empty(), "once per plug");
        rig.finish();
        rig.boot(None);
        for _ in 0..10 {
            rig.tap(&[Control::Down]);
        }
        let later = rig.clock.now_ns() + 2 * SILENT_PAD_AFTER_NS;
        rig.until(10_000, "more time", |r| r.clock.now_ns() >= later);
        assert!(notices(&mut rig).is_empty(), "not in the next run either");
        rig.finish();
    });
}

#[test]
#[ignore = "tier 2"]
fn a_notice_of_a_game_run_that_nobody_saw_shows_in_the_next_run() {
    let name = test_name(
        module_path!(),
        "a_notice_of_a_game_run_that_nobody_saw_shows_in_the_next_run",
    );
    in_child(&name, |dir| {
        let mut rig = Rig::new(dir);
        rig.plug(pad(1));
        rig.boot(Some(neptron::NextRun {
            game: Some(neptron::relaunch::GameRef {
                system: "nes".to_owned(),
                path: "nes/Test Game.nes".to_owned(),
            }),
            notice: None,
            pads: Vec::new(),
        }));
        assert!(rig.shell().in_game());
        rig.frames(30);
        rig.plug_joystick(joystick(), FULL);
        rig.frames(5);
        assert_eq!(rig.screen(), "game", "a notice waits for the menu");
        assert_eq!(notices(&mut rig).len(), 1);
        // The run ends with the notice unseen, as a crash or a power
        // request ends it.
        rig.finish();
        rig.boot(None);
        rig.frames(5);
        let texts: Vec<_> = notices(&mut rig).into_iter().map(|n| n.text).collect();
        assert_eq!(texts, [GENERIC_TEXT], "the notice was never seen");
        rig.tap(&[Control::A]);
        rig.finish();
        rig.boot(None);
        rig.frames(5);
        assert!(notices(&mut rig).is_empty(), "seen once, told once");
        rig.finish();
    });
}
