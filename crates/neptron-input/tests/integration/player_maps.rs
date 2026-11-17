//! The map of the player: the mapper reads raw indices with it, and a new
//! map takes effect at once.

use neptron_backend::InputSource;
use neptron_input::{
    Binding, DeviceIdentity, GAMEPAD_DEVICE_BASE, HatDirection, HatState, InputNotice,
    JoystickCounts, MapKind, Mapper, PadButton, PadInfo, PadMap, PadRoster, PlayerMap, PlayerMaps,
    PortRule, RawInput, Roster, ScriptedPads,
};
use neptron_types::{Control, InputEvent};

fn pad(model: u8, n: u8) -> PadInfo {
    PadInfo {
        identity: DeviceIdentity::from_parts(&[b"model", &[model]]),
        path: DeviceIdentity::from_parts(&[b"path", &[n]]),
        name: format!("Pad {model}.{n}"),
    }
}

fn counts(buttons: u32, hats: u32, axes: u32) -> JoystickCounts {
    JoystickCounts {
        buttons,
        hats,
        axes,
    }
}

fn controls(events: &[InputEvent]) -> Vec<(u32, Control, i16)> {
    events
        .iter()
        .map(|e| (e.device, e.control, e.value))
        .collect()
}

fn poll(pads: &mut ScriptedPads) -> Vec<(u32, Control, i16)> {
    let mut out = Vec::new();
    pads.poll(&mut out);
    controls(&out)
}

fn map(pairs: &[(Control, Binding)]) -> PlayerMap {
    let mut map = PlayerMap::new();
    for (control, binding) in pairs {
        map.set(*control, Some(*binding)).unwrap();
    }
    map
}

fn maps_of(identity: DeviceIdentity, map: PlayerMap) -> PlayerMaps {
    let mut maps = PlayerMaps::new();
    maps.insert(identity, map);
    maps
}

const PAD0: u32 = GAMEPAD_DEVICE_BASE;

#[test]
fn the_mapper_reads_each_kind_of_binding_of_a_player_map() {
    let player = map(&[
        (Control::A, Binding::Button(3)),
        (Control::B, Binding::Button(3)),
        (
            Control::Up,
            Binding::Hat {
                index: 1,
                direction: HatDirection::Up,
            },
        ),
        (
            Control::Left,
            Binding::HalfAxis {
                index: 0,
                positive: false,
                full: false,
            },
        ),
        (
            Control::L2,
            Binding::HalfAxis {
                index: 5,
                positive: true,
                full: true,
            },
        ),
        (
            Control::LeftX,
            Binding::Stick {
                index: 0,
                inverted: true,
            },
        ),
    ]);
    let mut mapper = Mapper::new();
    let id = mapper
        .connect_gamepad_with(pad(1, 1), PadMap::Player(player))
        .unwrap();
    assert_eq!(mapper.roster().get(0).map(|e| e.map), Some(MapKind::Player));
    let mut out = Vec::new();
    let inputs = [
        RawInput::JoyButton {
            index: 3,
            pressed: true,
        },
        RawInput::JoyButton {
            index: 2,
            pressed: true,
        },
        RawInput::JoyHat {
            index: 0,
            hat: HatState(HatState::UP),
        },
        RawInput::JoyHat {
            index: 1,
            hat: HatState(HatState::UP | HatState::RIGHT),
        },
        RawInput::JoyAxis {
            index: 0,
            value: -32_768,
        },
        RawInput::JoyAxis {
            index: 5,
            value: -32_768,
        },
        RawInput::JoyAxis {
            index: 5,
            value: 100,
        },
    ];
    for input in inputs {
        mapper.apply_gamepad(id, input, &mut out).unwrap();
    }
    assert_eq!(
        controls(&out),
        [
            (PAD0, Control::B, 1),
            (PAD0, Control::A, 1),
            (PAD0, Control::Up, 1),
            (PAD0, Control::Left, 1),
            (PAD0, Control::LeftX, 32_767),
            (PAD0, Control::L2, 1),
        ],
        "one raw input drives two controls, and a trigger counts past the middle"
    );
}

#[test]
fn a_known_pad_gets_the_map_of_the_player_at_once_and_back_on_reset() {
    let mut pads = ScriptedPads::new(PortRule::Menus, &Roster::new());
    let test = pads.clone();
    let one = test.plug(pad(1, 1));
    test.set(one, PadButton::South, true);
    assert_eq!(poll(&mut pads), [(PAD0, Control::B, 1)]);
    let changes = pads.roster_changes();

    // Raw button 0 of a scripted gamepad is South.
    let swap = map(&[(Control::A, Binding::Button(0))]);
    pads.set_player_maps(&maps_of(pad(1, 1).identity, swap));
    assert_eq!(
        poll(&mut pads),
        [(PAD0, Control::B, 0), (PAD0, Control::A, 1)],
        "the held control goes up, and the next read uses the new map"
    );
    assert_ne!(pads.roster_changes(), changes);
    assert_eq!(pads.roster().get(0).map(|e| e.map), Some(MapKind::Player));

    pads.set_player_maps(&PlayerMaps::new());
    assert_eq!(
        poll(&mut pads),
        [(PAD0, Control::A, 0), (PAD0, Control::B, 1)]
    );
    assert_eq!(pads.roster().get(0).map(|e| e.map), Some(MapKind::Known));
    assert!(poll(&mut pads).is_empty(), "the same maps change nothing");
}

#[test]
fn a_generic_pad_with_a_map_of_the_player_gives_no_generic_notice() {
    let mut pads = ScriptedPads::new(PortRule::Menus, &Roster::new());
    let test = pads.clone();
    let stick = test.plug_joystick(pad(2, 1), counts(4, 1, 2));
    let mine = map(&[
        (Control::A, Binding::Button(0)),
        (
            Control::Down,
            Binding::Hat {
                index: 0,
                direction: HatDirection::Down,
            },
        ),
    ]);
    pads.set_player_maps(&maps_of(pad(2, 1).identity, mine));
    assert!(poll(&mut pads).is_empty());
    let mut notices = Vec::new();
    pads.take_notices(&mut notices);
    assert!(
        !notices
            .iter()
            .any(|n| matches!(n, InputNotice::GenericLayout { .. })),
        "{notices:?}"
    );
    test.set_joy_button(stick, 0, true);
    test.set_joy_hat(stick, HatState(HatState::DOWN));
    assert_eq!(
        poll(&mut pads),
        [(PAD0, Control::A, 1), (PAD0, Control::Down, 1)]
    );
}

#[test]
fn two_pads_of_one_model_share_the_map() {
    let mut pads = ScriptedPads::new(PortRule::Menus, &Roster::new());
    let test = pads.clone();
    let first = test.plug(pad(3, 1));
    let second = test.plug(pad(3, 2));
    let other = test.plug(pad(4, 1));
    poll(&mut pads);
    pads.set_player_maps(&maps_of(
        pad(3, 1).identity,
        map(&[(Control::X, Binding::Button(1))]),
    ));
    poll(&mut pads);
    let kinds: Vec<_> = pads.roster().ports()[..3]
        .iter()
        .map(|e| e.as_ref().map(|e| e.map))
        .collect();
    assert_eq!(
        kinds,
        [
            Some(MapKind::Player),
            Some(MapKind::Player),
            Some(MapKind::Known)
        ]
    );
    for pad in [first, second, other] {
        test.set(pad, PadButton::East, true);
    }
    assert_eq!(
        poll(&mut pads),
        [
            (PAD0, Control::X, 1),
            (PAD0 + 1, Control::X, 1),
            (PAD0 + 2, Control::A, 1)
        ]
    );
}
