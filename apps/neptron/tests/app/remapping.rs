//! The guided mapping of a controller: a generic pad and a known pad
//! mapped from the menus, then playing with the new map; skip by hold,
//! cancel from the keyboard, the timeout, the reset, and the map in the
//! settings folder across a relaunch and a new start.

use neptron_backend::Clock;
use neptron_input::{DeviceIdentity, HatState, JoystickCounts, PadButton, PadInfo, ScriptedPad};
use neptron_types::Control;
use neptron_ui::snapshot::{MappingStage, PadMapping};

use crate::players::{launch_from_the_list, pad, rows};
use crate::rig::{Rig, in_child, test_name};

/// The file of the controller layouts under the state folder.
const PAD_MAPS: &str = "config/input/pads.toml";

/// A pad with no mapping: 12 buttons, a hat and 2 axes.
const STICK: JoystickCounts = JoystickCounts {
    buttons: 12,
    hats: 1,
    axes: 2,
};

fn joystick() -> PadInfo {
    PadInfo {
        identity: DeviceIdentity::from_parts(&[b"remap joystick"]),
        path: DeviceIdentity::from_parts(&[b"/dev/input/event7"]),
        name: "USB Joystick".to_owned(),
    }
}

/// One answer to a prompt of the guided mapping.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Raw {
    /// A raw button of a joystick.
    Button(u8),
    /// A hat direction of a joystick.
    Hat(u8),
    /// An axis of a joystick to a value, then back to 0.
    Axis(u8, i16),
    /// A button of the standard layout of a scripted gamepad.
    Pad(PadButton),
    /// A raw button of a joystick held until the step is skipped.
    Skip(u8),
    /// A button of a scripted gamepad held until the step is skipped.
    SkipPad(PadButton),
}

/// The step of the prompt on show, from 1, or `None` with no prompt.
pub(crate) fn step(rig: &mut Rig) -> Option<u32> {
    match &rig.shell().snapshot().pad_mapping {
        Some(PadMapping {
            stage: MappingStage::Prompt { step, .. },
            ..
        }) => Some(*step),
        _ => None,
    }
}

/// The review on show: its message and whether it offers Keep.
pub(crate) fn review(rig: &mut Rig) -> Option<(String, bool)> {
    match &rig.shell().snapshot().pad_mapping {
        Some(PadMapping {
            stage: MappingStage::Review { message, can_keep },
            ..
        }) => Some((message.clone(), *can_keep)),
        _ => None,
    }
}

fn set(rig: &mut Rig, pad: ScriptedPad, raw: Raw, down: bool) {
    match raw {
        Raw::Button(index) | Raw::Skip(index) => rig.pads.set_joy_button(pad, index, down),
        Raw::Hat(bits) => {
            let hat = if down {
                HatState(bits)
            } else {
                HatState::default()
            };
            rig.pads.set_joy_hat(pad, hat);
        }
        Raw::Axis(index, value) => rig
            .pads
            .set_joy_axis(pad, index, if down { value } else { 0 }),
        Raw::Pad(button) | Raw::SkipPad(button) => rig.pads.set(pad, button, down),
    }
}

/// Answers the prompt on show with `raw`, and checks that the mapping
/// moves on.
pub(crate) fn answer(rig: &mut Rig, pad: ScriptedPad, raw: Raw) {
    let before = step(rig);
    assert!(before.is_some(), "no prompt for {raw:?}");
    set(rig, pad, raw, true);
    match raw {
        Raw::Skip(_) | Raw::SkipPad(_) => {
            rig.until(300, "the skip", |r| step(r) != before);
        }
        _ => rig.frames(2),
    }
    set(rig, pad, raw, false);
    rig.frames(2);
    let after = step(rig);
    assert!(
        after != before || review(rig).is_some(),
        "{raw:?} did not move the mapping on from {before:?}"
    );
}

/// Opens the page of the controller of the row `row` of the Input panel,
/// from home, with the keyboard.
pub(crate) fn open_pad_page(rig: &mut Rig, row: &str) {
    rig.scanned();
    rig.focus("Settings");
    rig.tap(&[Control::A]);
    rig.focus("Input");
    rig.tap(&[Control::A]);
    rig.focus(row);
    rig.tap(&[Control::A]);
    rig.frames(2);
    assert!(
        rows(rig).iter().any(|(l, _)| l == "Controller"),
        "{:?}",
        rig.tree()
    );
}

/// Starts the mapping from the page on show, with the keyboard.
pub(crate) fn map_buttons(rig: &mut Rig) {
    rig.focus("Map buttons");
    rig.tap(&[Control::A]);
    rig.frames(2);
    assert_eq!(rig.screen(), "pad-mapping");
}

/// The text of a row of the screen on show.
fn row(rig: &mut Rig, label: &str) -> Option<String> {
    rows(rig)
        .into_iter()
        .find(|(l, _)| l == label)
        .map(|(_, v)| v)
}

/// The answers of a full mapping of [`STICK`]: the hat for the D-pad,
/// button 0 as A and 3 as B (not their places in the generic layout), the
/// other buttons in order, the left stick on the axes, and the right
/// stick, which it lacks, skipped by a hold.
const STICK_ANSWERS: [Raw; 20] = [
    Raw::Hat(HatState::UP),
    Raw::Hat(HatState::DOWN),
    Raw::Hat(HatState::LEFT),
    Raw::Hat(HatState::RIGHT),
    Raw::Button(0),
    Raw::Button(3),
    Raw::Button(9),
    Raw::Button(8),
    Raw::Button(1),
    Raw::Button(2),
    Raw::Button(4),
    Raw::Button(5),
    Raw::Button(6),
    Raw::Button(7),
    Raw::Button(10),
    Raw::Button(11),
    Raw::Axis(0, 32_767),
    Raw::Axis(1, 32_767),
    Raw::Skip(4),
    Raw::Skip(4),
];

#[test]
#[ignore = "tier 2"]
fn a_generic_pad_is_mapped_from_its_notice_and_plays_with_its_map_after_a_relaunch_and_a_restart() {
    let name = test_name(
        module_path!(),
        "a_generic_pad_is_mapped_from_its_notice_and_plays_with_its_map_after_a_relaunch_and_a_restart",
    );
    in_child(&name, |dir| {
        let mut rig = Rig::new(dir);
        rig.boot(None);
        rig.plug(pad(1));
        rig.frames(2);
        let stick = rig.plug_joystick(joystick(), STICK);
        rig.frames(2);
        assert_eq!(rig.screen(), "notice");
        // `Set up this controller`, from the keyboard.
        rig.tap(&[Control::Right]);
        rig.tap(&[Control::A]);
        rig.frames(2);
        assert_eq!(rig.screen(), "pad-mapping", "the button starts the mapping");
        assert_eq!(step(&mut rig), Some(1));
        for raw in STICK_ANSWERS {
            answer(&mut rig, stick, raw);
        }
        let (message, can_keep) = review(&mut rig).expect("the review");
        assert!(can_keep, "{message}");
        assert_eq!(
            message,
            "18 of 20 controls are set. Skipped: Right stick X, Right stick Y."
        );
        assert_eq!(rig.screen(), "pad-mapping/review");

        // The new map drives the review: right on the hat, then A on
        // button 0, which the generic layout reads as Y.
        rig.pads.set_joy_hat(stick, HatState(HatState::RIGHT));
        rig.frames(2);
        rig.pads.set_joy_hat(stick, HatState::default());
        rig.frames(2);
        rig.joy_tap(stick, 0);
        rig.frames(2);
        assert!(rig.shell().snapshot().pad_mapping.is_none());
        rig.frames(2);
        rig.pads.set_joy_button(stick, 0, true);
        rig.frames(2);
        let ports = rig.shell().ports().expect("ports");
        assert!(
            ports.is_held(1, Control::A),
            "button 0 is A of player 2 now"
        );
        rig.pads.set_joy_button(stick, 0, false);
        rig.frames(2);

        // Back to home from the keyboard, then a game: the next run reads
        // the file, and the map holds on the port of the pad.
        for _ in 0..4 {
            rig.tap(&[Control::B]);
        }
        let next = launch_from_the_list(&mut rig);
        rig.finish();
        let file = rig.state_file(PAD_MAPS);
        assert!(file.contains("a = \"b0\""), "{file}");
        assert!(file.contains("up = \"h0up\""), "{file}");
        assert!(
            !file.contains("right_x"),
            "a skipped control has no key: {file}"
        );
        rig.boot(Some(next));
        assert!(rig.shell().in_game());
        rig.frames(30);
        let stick = rig.joysticks[0].2;
        rig.pads.set_joy_button(stick, 0, true);
        rig.frames(2);
        let ports = rig.shell().ports().expect("ports");
        assert!(ports.is_held(1, Control::A), "the game reads the map");
        assert!(!ports.is_held(1, Control::Y));
        rig.pads.set_joy_button(stick, 0, false);
        rig.frames(2);
        rig.finish();

        // A new start of the machine: no relaunch file, the same file.
        rig.boot(None);
        rig.frames(2);
        open_pad_page(&mut rig, "Player 2");
        assert_eq!(row(&mut rig, "Layout").as_deref(), Some("Yours"));
        assert_eq!(row(&mut rig, "A").as_deref(), Some("Button 0"));
        assert!(
            rig.shell().snapshot().notices.is_empty(),
            "a pad with a layout of the player gets no generic notice"
        );
        rig.finish();
    });
}

/// The answers for a scripted gamepad: the D-pad, A and B swapped against
/// the standard layout, Start, Select, the face and shoulder buttons, and
/// a hold for the triggers and the sticks, which it lacks.
pub(crate) const PAD_ANSWERS: [Raw; 20] = [
    Raw::Pad(PadButton::DPadUp),
    Raw::Pad(PadButton::DPadDown),
    Raw::Pad(PadButton::DPadLeft),
    Raw::Pad(PadButton::DPadRight),
    Raw::Pad(PadButton::South),
    Raw::Pad(PadButton::East),
    Raw::Pad(PadButton::Start),
    Raw::Pad(PadButton::Back),
    Raw::Pad(PadButton::West),
    Raw::Pad(PadButton::North),
    Raw::Pad(PadButton::LeftShoulder),
    Raw::Pad(PadButton::RightShoulder),
    Raw::SkipPad(PadButton::North),
    Raw::SkipPad(PadButton::North),
    Raw::Pad(PadButton::LeftStick),
    Raw::Pad(PadButton::RightStick),
    Raw::SkipPad(PadButton::North),
    Raw::SkipPad(PadButton::North),
    Raw::SkipPad(PadButton::North),
    Raw::SkipPad(PadButton::North),
];

#[test]
#[ignore = "tier 2"]
fn a_known_pad_is_remapped_for_both_pads_of_its_model_and_reset() {
    let name = test_name(
        module_path!(),
        "a_known_pad_is_remapped_for_both_pads_of_its_model_and_reset",
    );
    in_child(&name, |dir| {
        let mut rig = Rig::new(dir);
        rig.boot(None);
        let first = rig.plug(pad(1));
        let twin = PadInfo {
            path: DeviceIdentity::from_parts(&[b"/dev/input/event42"]),
            ..pad(1)
        };
        let second = rig.plug(twin);
        rig.frames(2);
        open_pad_page(&mut rig, "Player 1");
        assert_eq!(row(&mut rig, "Layout").as_deref(), Some("Known"));
        assert_eq!(row(&mut rig, "A").as_deref(), Some("East button"));
        map_buttons(&mut rig);
        for raw in PAD_ANSWERS {
            answer(&mut rig, first, raw);
        }
        let (message, can_keep) = review(&mut rig).expect("the review");
        assert!(can_keep, "{message}");
        assert_eq!(
            message,
            "14 of 20 controls are set. Skipped: L2, R2, Left stick X, Left stick Y, Right stick X, Right stick Y."
        );
        // The new map: D-pad right, then South, which is A now.
        rig.pads.set(first, PadButton::DPadRight, true);
        rig.frames(2);
        rig.pads.set(first, PadButton::DPadRight, false);
        rig.frames(2);
        rig.pads.set(first, PadButton::South, true);
        rig.frames(2);
        rig.pads.set(first, PadButton::South, false);
        rig.frames(4);
        assert!(rig.shell().snapshot().pad_mapping.is_none(), "kept");
        assert_eq!(row(&mut rig, "Layout").as_deref(), Some("Yours"));
        assert_eq!(row(&mut rig, "A").as_deref(), Some("Button 0"));
        assert_eq!(row(&mut rig, "Up").as_deref(), Some("Button 10"));

        // East is B in the new map, so the twin goes back to the panel.
        rig.pads.set(second, PadButton::East, true);
        rig.frames(2);
        let ports = rig.shell().ports().expect("ports");
        assert!(ports.is_held(1, Control::B), "the twin shares the map");
        assert!(!ports.is_held(1, Control::A));
        rig.pads.set(second, PadButton::East, false);
        rig.frames(2);
        assert_eq!(
            rows(&mut rig)[..2],
            [
                ("Player 1".to_owned(), "Pad 1 (your layout)".to_owned()),
                ("Player 2".to_owned(), "Pad 1 (your layout)".to_owned()),
            ]
        );

        // Reset to default, from the keyboard, through its question.
        rig.focus("Player 2");
        rig.tap(&[Control::A]);
        rig.frames(2);
        rig.focus("Reset to default");
        rig.tap(&[Control::A]);
        rig.tap(&[Control::Right]);
        rig.tap(&[Control::A]);
        rig.frames(4);
        assert_eq!(row(&mut rig, "Layout").as_deref(), Some("Known"));
        rig.pads.set(first, PadButton::South, true);
        rig.frames(2);
        let ports = rig.shell().ports().expect("ports");
        assert!(ports.is_held(0, Control::B), "the default map is back");
        rig.pads.set(first, PadButton::South, false);
        rig.frames(2);
        rig.finish();
        let file = rig.state_file(PAD_MAPS);
        assert!(!file.contains("[[pad]]"), "{file}");
    });
}

#[test]
#[ignore = "tier 2"]
fn a_mapping_ends_with_no_change_from_the_keyboard_and_at_its_timeouts() {
    let name = test_name(
        module_path!(),
        "a_mapping_ends_with_no_change_from_the_keyboard_and_at_its_timeouts",
    );
    in_child(&name, |dir| {
        let mut rig = Rig::new(dir);
        rig.boot(None);
        let one = rig.plug(pad(1));
        rig.frames(2);
        open_pad_page(&mut rig, "Player 1");

        // Back on the keyboard, in the middle of the mapping.
        map_buttons(&mut rig);
        answer(&mut rig, one, Raw::Pad(PadButton::DPadUp));
        rig.tap(&[Control::B]);
        rig.frames(2);
        assert!(rig.shell().snapshot().pad_mapping.is_none());
        assert_eq!(row(&mut rig, "Layout").as_deref(), Some("Known"));
        rig.pads.set(one, PadButton::West, true);
        rig.frames(2);
        assert!(
            rig.shell().ports().expect("ports").is_held(0, Control::Y),
            "the pad plays again with its old map"
        );
        rig.pads.set(one, PadButton::West, false);
        rig.frames(2);

        // No input at all: the prompt counts down and the mapping ends.
        map_buttons(&mut rig);
        let start = rig.clock.now_ns();
        let full = neptron_input::MAP_IDLE_TIMEOUT_NS;
        rig.until(2_000, "five seconds", |r| {
            r.clock.now_ns() - start >= 5_000_000_000
        });
        let left = rig
            .shell()
            .snapshot()
            .pad_mapping
            .as_ref()
            .map(|m| m.remaining_ns)
            .expect("the prompt");
        assert!(
            (full - 5_500_000_000..=full - 4_500_000_000).contains(&left),
            "the countdown shows {left} ns"
        );
        rig.until(2_000, "the timeout", |r| {
            r.shell().snapshot().pad_mapping.is_none()
        });
        // The count starts at the first read of the pad, a few frames
        // after the start.
        let waited = rig.clock.now_ns() - start;
        assert!(
            (full - 200_000_000..=full + 200_000_000).contains(&waited),
            "ended after {waited} ns"
        );
        assert_eq!(row(&mut rig, "Layout").as_deref(), Some("Known"));

        // A review with no answer gives the old map back.
        map_buttons(&mut rig);
        for raw in PAD_ANSWERS {
            answer(&mut rig, one, raw);
        }
        assert!(review(&mut rig).is_some());
        rig.until(2_000, "the end of the review", |r| {
            r.shell().snapshot().pad_mapping.is_none()
        });
        // The source takes the old maps at its next poll.
        rig.frames(3);
        assert_eq!(row(&mut rig, "Layout").as_deref(), Some("Known"));
        rig.pads.set(one, PadButton::West, true);
        rig.frames(2);
        assert!(rig.shell().ports().expect("ports").is_held(0, Control::Y));
        rig.pads.set(one, PadButton::West, false);
        rig.frames(2);

        // A layout with no B cannot be kept.
        map_buttons(&mut rig);
        for raw in PAD_ANSWERS.iter().take(5) {
            answer(&mut rig, one, *raw);
        }
        for _ in 5..20 {
            answer(&mut rig, one, Raw::SkipPad(PadButton::North));
        }
        let (message, can_keep) = review(&mut rig).expect("the review");
        assert!(!can_keep);
        assert_eq!(
            message,
            "This layout has no B. A controller needs the four directions, A and B."
        );
        rig.tap(&[Control::A]);
        rig.frames(2);
        assert!(rig.shell().snapshot().pad_mapping.is_none());
        rig.finish();
        assert_eq!(rig.state_file(PAD_MAPS), "", "nothing was written");
    });
}

#[test]
#[ignore = "tier 2"]
fn a_last_skip_held_with_back_of_the_new_map_leaves_the_review_on_show() {
    let name = test_name(
        module_path!(),
        "a_last_skip_held_with_back_of_the_new_map_leaves_the_review_on_show",
    );
    in_child(&name, |dir| {
        let mut rig = Rig::new(dir);
        rig.boot(None);
        rig.plug(pad(1));
        rig.frames(2);
        let stick = rig.plug_joystick(joystick(), STICK);
        rig.frames(2);
        rig.tap(&[Control::Right]);
        rig.tap(&[Control::A]);
        rig.frames(2);
        assert_eq!(rig.screen(), "pad-mapping");
        for raw in &STICK_ANSWERS[..18] {
            answer(&mut rig, stick, *raw);
        }
        // Button 3 is B in the new map. The last skip ends with it down.
        answer(&mut rig, stick, Raw::Skip(3));
        rig.pads.set_joy_button(stick, 3, true);
        rig.until(300, "the review", |r| review(r).is_some());
        rig.frames(30);
        assert!(review(&mut rig).is_some(), "the held B is not an answer");
        rig.pads.set_joy_button(stick, 3, false);
        rig.frames(4);
        assert!(review(&mut rig).is_some(), "the release is not an answer");

        // Right on the hat, then A on button 0: Keep.
        rig.pads.set_joy_hat(stick, HatState(HatState::RIGHT));
        rig.frames(2);
        rig.pads.set_joy_hat(stick, HatState::default());
        rig.frames(2);
        rig.joy_tap(stick, 0);
        rig.frames(4);
        assert!(rig.shell().snapshot().pad_mapping.is_none());
        rig.pads.set_joy_button(stick, 0, true);
        rig.frames(2);
        assert!(
            rig.shell().ports().expect("ports").is_held(1, Control::A),
            "the new map is kept"
        );
        rig.pads.set_joy_button(stick, 0, false);
        rig.frames(2);
        rig.finish();
    });
}

/// Writes `text` as the file of the controller layouts before a boot.
fn write_pad_maps(dir: &std::path::Path, text: &str) {
    let path = dir.join("state").join(PAD_MAPS);
    std::fs::create_dir_all(path.parent().expect("a folder")).expect("the folder");
    std::fs::write(path, text).expect("the file");
}

/// The identity of [`pad`] `n` as the file writes it.
fn identity_text(n: u8) -> String {
    format!("{:016x}", pad(n).identity.0)
}

#[test]
#[ignore = "tier 2"]
fn a_stored_layout_with_no_b_is_not_used_and_the_boot_says_so() {
    let name = test_name(
        module_path!(),
        "a_stored_layout_with_no_b_is_not_used_and_the_boot_says_so",
    );
    in_child(&name, |dir| {
        // South is raw button 0; the B binding is damaged.
        let text = format!(
            "version = 1\n[[pad]]\nidentity = \"{}\"\nup = \"b10\"\ndown = \"b11\"\nleft = \"b12\"\nright = \"b13\"\na = \"b0\"\nb = \"b1 \"\n",
            identity_text(1)
        );
        write_pad_maps(dir, &text);
        let mut rig = Rig::new(dir);
        rig.boot(None);
        let one = rig.plug(pad(1));
        rig.frames(2);
        assert!(
            crate::tv::notice_texts(&mut rig)
                .iter()
                .any(|n| n.contains("Its controller uses its default layout")),
            "{:?}",
            crate::tv::notice_texts(&mut rig)
        );
        crate::tv::dismiss_notices(&mut rig);
        rig.pads.set(one, PadButton::South, true);
        rig.frames(2);
        let ports = rig.shell().ports().expect("ports");
        assert!(
            ports.is_held(0, Control::B),
            "the default map drives the pad"
        );
        assert!(!ports.is_held(0, Control::A));
        rig.pads.set(one, PadButton::South, false);
        rig.frames(2);
        rig.finish();
        assert_eq!(rig.state_file(PAD_MAPS), text, "the file stays as it is");
    });
}

#[test]
#[ignore = "tier 2"]
fn a_newer_file_is_not_used_and_a_new_layout_keeps_it_as_the_backup() {
    let name = test_name(
        module_path!(),
        "a_newer_file_is_not_used_and_a_new_layout_keeps_it_as_the_backup",
    );
    in_child(&name, |dir| {
        let newer = format!(
            "version = 2\n[[pad]]\nidentity = \"{}\"\nup = \"b10\"\ndown = \"b11\"\nleft = \"b12\"\nright = \"b13\"\na = \"b0\"\nb = \"b1\"\n",
            identity_text(1)
        );
        write_pad_maps(dir, &newer);
        let mut rig = Rig::new(dir);
        rig.boot(None);
        let one = rig.plug(pad(1));
        rig.frames(2);
        assert!(
            crate::tv::notice_texts(&mut rig)
                .iter()
                .any(|n| n.contains("version 2") && n.contains("pads.toml.bak")),
            "{:?}",
            crate::tv::notice_texts(&mut rig)
        );
        crate::tv::dismiss_notices(&mut rig);
        rig.pads.set(one, PadButton::South, true);
        rig.frames(2);
        assert!(
            rig.shell().ports().expect("ports").is_held(0, Control::B),
            "the newer layout is not used"
        );
        rig.pads.set(one, PadButton::South, false);
        rig.frames(2);

        open_pad_page(&mut rig, "Player 1");
        assert_eq!(row(&mut rig, "Layout").as_deref(), Some("Known"));
        map_buttons(&mut rig);
        for raw in PAD_ANSWERS {
            answer(&mut rig, one, raw);
        }
        assert!(review(&mut rig).is_some_and(|(_, can_keep)| can_keep));
        rig.pads.set(one, PadButton::DPadRight, true);
        rig.frames(2);
        rig.pads.set(one, PadButton::DPadRight, false);
        rig.frames(2);
        rig.pads.set(one, PadButton::South, true);
        rig.frames(2);
        rig.pads.set(one, PadButton::South, false);
        rig.frames(4);
        assert!(rig.shell().snapshot().pad_mapping.is_none());
        assert_eq!(row(&mut rig, "Layout").as_deref(), Some("Yours"));

        // Reset works too, and the backup stays the newer file.
        rig.focus("Reset to default");
        rig.tap(&[Control::A]);
        rig.tap(&[Control::Right]);
        rig.tap(&[Control::A]);
        rig.frames(4);
        assert_eq!(row(&mut rig, "Layout").as_deref(), Some("Known"));
        rig.finish();
        let file = rig.state_file(PAD_MAPS);
        assert!(file.contains("version = 1"), "{file}");
        assert!(!file.contains("[[pad]]"), "{file}");
        assert_eq!(rig.state_file("config/input/pads.toml.bak"), newer);
    });
}
