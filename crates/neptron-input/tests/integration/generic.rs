//! A joystick with no gamepad mapping, read by the generic layout.

use neptron_backend::InputSource;
use neptron_input::{
    BLIND_RECOVERY_COMBO, DeviceIdentity, GAMEPAD_DEVICE_BASE, GenericLayout, HatState, Hotkey,
    Hotkeys, InputNotice, JoystickCounts, MapKind, Mapper, PadInfo, PadMap, PadRoster, PortRule,
    PortStates, QUICK_MENU_COMBO, QUICK_MENU_HOLD_NS, RawInput, Roster, STICK_DEAD_ZONE,
    ScriptedPads,
};
use neptron_types::{Control, InputEvent};

fn joystick(n: u8) -> PadInfo {
    PadInfo {
        identity: DeviceIdentity::from_parts(&[b"joystick", &[n]]),
        path: DeviceIdentity::from_parts(&[b"/dev/input/event", &[n]]),
        name: format!("USB Joystick {n}"),
    }
}

fn counts(buttons: u32, hats: u32, axes: u32) -> JoystickCounts {
    JoystickCounts {
        buttons,
        hats,
        axes,
    }
}

fn generic(buttons: u32, hats: u32, axes: u32) -> PadMap {
    PadMap::Generic(GenericLayout::for_counts(counts(buttons, hats, axes)).unwrap())
}

fn controls(events: &[InputEvent]) -> Vec<(Control, i16)> {
    events.iter().map(|e| (e.control, e.value)).collect()
}

#[test]
fn a_generic_pad_gives_its_buttons_its_hat_and_its_sticks() {
    let mut mapper = Mapper::new();
    let id = mapper
        .connect_gamepad_with(joystick(1), generic(13, 1, 4))
        .unwrap();
    let mut out = Vec::new();
    for index in 0..13 {
        let press = RawInput::JoyButton {
            index,
            pressed: true,
        };
        mapper.apply_gamepad(id, press, &mut out).unwrap();
    }
    assert_eq!(
        controls(&out),
        [
            (Control::Y, 1),
            (Control::B, 1),
            (Control::A, 1),
            (Control::X, 1),
            (Control::L, 1),
            (Control::R, 1),
            (Control::L2, 1),
            (Control::R2, 1),
            (Control::Select, 1),
            (Control::Start, 1),
            (Control::L3, 1),
            (Control::R3, 1),
        ]
    );
    out.clear();
    let hat = HatState(HatState::UP | HatState::LEFT);
    mapper
        .apply_gamepad(id, RawInput::JoyHat { index: 0, hat }, &mut out)
        .unwrap();
    mapper
        .apply_gamepad(
            id,
            RawInput::JoyHat {
                index: 1,
                hat: HatState(HatState::DOWN),
            },
            &mut out,
        )
        .unwrap();
    assert_eq!(controls(&out), [(Control::Up, 1), (Control::Left, 1)]);
    out.clear();
    for (index, value) in [
        (0, i16::MAX),
        (1, 0),
        (2, i16::MIN),
        (3, STICK_DEAD_ZONE),
        (4, i16::MAX),
    ] {
        mapper
            .apply_gamepad(id, RawInput::JoyAxis { index, value }, &mut out)
            .unwrap();
    }
    assert_eq!(
        controls(&out),
        [(Control::LeftX, i16::MAX), (Control::RightX, -i16::MAX)],
        "a stick with a hat does not drive the D-pad, and axis 4 drives nothing"
    );
    assert_eq!(
        mapper.roster().get(0).map(|e| e.map),
        Some(MapKind::Generic)
    );
}

#[test]
fn a_pad_with_no_hat_drives_the_d_pad_with_its_left_stick() {
    let mut mapper = Mapper::new();
    let id = mapper
        .connect_gamepad_with(joystick(1), generic(12, 0, 2))
        .unwrap();
    let mut out = Vec::new();
    mapper
        .apply_gamepad(
            id,
            RawInput::JoyAxis {
                index: 1,
                value: i16::MIN,
            },
            &mut out,
        )
        .unwrap();
    assert_eq!(
        controls(&out),
        [(Control::LeftY, -i16::MAX), (Control::Up, 1)]
    );
    out.clear();
    mapper
        .apply_gamepad(id, RawInput::JoyAxis { index: 1, value: 0 }, &mut out)
        .unwrap();
    assert_eq!(controls(&out), [(Control::LeftY, 0), (Control::Up, 0)]);
}

#[test]
fn a_known_pad_ignores_raw_indices_and_a_generic_pad_keeps_the_standard_names() {
    let mut mapper = Mapper::new();
    let known = mapper.connect_gamepad(joystick(1)).unwrap();
    let mut out = Vec::new();
    let raw = RawInput::JoyButton {
        index: 1,
        pressed: true,
    };
    mapper.apply_gamepad(known, raw, &mut out).unwrap();
    assert!(out.is_empty());
    assert_eq!(mapper.roster().get(0).map(|e| e.map), Some(MapKind::Known));
}

#[test]
fn a_pad_is_heard_only_after_a_value_that_is_not_neutral() {
    let mut mapper = Mapper::new();
    let id = mapper
        .connect_gamepad_with(joystick(1), generic(12, 1, 2))
        .unwrap();
    let mut out = Vec::new();
    let heard = |m: &Mapper| m.activity().ports[0].map(|s| s.heard);
    let plug = |m: &Mapper| m.activity().ports[0].map(|s| s.plug);
    let first_plug = plug(&mapper);
    let quiet = [
        RawInput::JoyButton {
            index: 0,
            pressed: false,
        },
        RawInput::JoyAxis {
            index: 0,
            value: STICK_DEAD_ZONE,
        },
        RawInput::JoyHat {
            index: 0,
            hat: HatState::default(),
        },
    ];
    for input in quiet {
        mapper.apply_gamepad(id, input, &mut out).unwrap();
    }
    assert_eq!(heard(&mapper), Some(false));
    let press = RawInput::JoyButton {
        index: 3,
        pressed: true,
    };
    mapper.apply_gamepad(id, press, &mut out).unwrap();
    assert_eq!(heard(&mapper), Some(true));
    let activity = mapper.activity();
    assert_eq!(
        activity.ports[0].map(|s| s.identity),
        Some(joystick(1).identity)
    );
    assert!(activity.ports[1..].iter().all(Option::is_none));
    mapper.disconnect_gamepad(id, &mut out).unwrap();
    mapper
        .connect_gamepad_with(joystick(1), generic(12, 1, 2))
        .unwrap();
    assert_eq!(heard(&mapper), Some(false), "a new plug starts unheard");
    assert_ne!(plug(&mapper), first_plug);
}

/// A scripted joystick gives the hotkeys the quick menu: Select is raw
/// button 8 and Start raw button 9.
#[test]
fn a_generic_pad_reaches_the_hotkeys_and_opens_the_quick_menu() {
    assert_eq!(QUICK_MENU_COMBO, [Control::Select, Control::Start]);
    assert_eq!(BLIND_RECOVERY_COMBO, QUICK_MENU_COMBO);
    let mut pads = ScriptedPads::new(PortRule::Menus, &Roster::new());
    let test = pads.clone();
    let stick = test.plug_joystick(joystick(1), counts(13, 1, 4));
    let mut ports = PortStates::new();
    let mut hotkeys = Hotkeys::new();
    let (mut events, mut fired) = (Vec::new(), Vec::new());
    let mut frame = |now_ns: u64, fired: &mut Vec<Hotkey>| {
        events.clear();
        pads.poll(&mut events);
        ports.apply(&events);
        hotkeys.update(&ports, now_ns, fired);
    };
    frame(0, &mut fired);
    // A short overlap opens nothing, also at its release.
    test.set_joy_button(stick, 8, true);
    test.set_joy_button(stick, 9, true);
    frame(10_000_000, &mut fired);
    frame(10_000_000 + QUICK_MENU_HOLD_NS - 1, &mut fired);
    test.set_joy_button(stick, 9, false);
    frame(20_000_000 + QUICK_MENU_HOLD_NS, &mut fired);
    assert_eq!(fired, []);
    // A hold opens it at the hold time, once.
    let t0 = 1_000_000_000;
    test.set_joy_button(stick, 9, true);
    frame(t0, &mut fired);
    frame(t0 + QUICK_MENU_HOLD_NS - 1, &mut fired);
    assert_eq!(fired, []);
    frame(t0 + QUICK_MENU_HOLD_NS, &mut fired);
    frame(t0 + 2 * QUICK_MENU_HOLD_NS, &mut fired);
    assert_eq!(fired, [Hotkey::QuickMenu { port: 0 }]);
}

#[test]
fn a_scripted_joystick_gives_its_notices_and_reads_by_index() {
    let mut pads = ScriptedPads::new(PortRule::Menus, &Roster::new());
    let test = pads.clone();
    let stick = test.plug_joystick(joystick(1), counts(8, 1, 0));
    let mouse = test.plug_joystick(joystick(2), counts(1, 0, 2));
    let mut events = Vec::new();
    pads.poll(&mut events);
    let mut notices = Vec::new();
    pads.take_notices(&mut notices);
    assert!(notices.contains(&InputNotice::GenericLayout {
        device: GAMEPAD_DEVICE_BASE,
        pad: joystick(1)
    }));
    assert!(notices.iter().any(|n| matches!(
        n,
        InputNotice::NotAGamepad { pad, .. } if *pad == joystick(2)
    )));
    assert_eq!(test.device(stick), Some(GAMEPAD_DEVICE_BASE));
    assert_eq!(test.device(mouse), None);
    test.set_joy_hat(stick, HatState(HatState::RIGHT));
    test.set_joy_button(stick, 2, true);
    test.set_joy_axis(stick, 0, i16::MAX);
    events.clear();
    pads.poll(&mut events);
    assert_eq!(controls(&events), [(Control::A, 1), (Control::Right, 1)]);
    notices.clear();
    pads.take_notices(&mut notices);
    assert_eq!(notices, [], "a notice comes once");
    assert_eq!(
        pads.roster().get(0).map(|e| (e.pad.name.clone(), e.map)),
        Some(("USB Joystick 1".to_owned(), MapKind::Generic))
    );
    assert_eq!(pads.activity().ports[0].map(|s| s.heard), Some(true));
}
