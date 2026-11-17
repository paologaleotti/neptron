use neptron_input::{
    DeviceIdentity, GAMEPAD_DEVICE_BASE, HatState, KEYBOARD_DEVICE_BASE, KeyAction, KeyCode,
    MAX_KEYBOARDS, MAX_PORTS, Mapper, MapperError, PadAxis, PadButton, PadId, PadInfo, PortRule,
    RawInput, Roster, RosterEntry, STICK_DEAD_ZONE, TRIGGER_THRESHOLD, port_of,
};
use neptron_types::{Control, InputEvent};

/// Pad `n`: its own model, at its own path.
fn pad(n: u8) -> PadInfo {
    twin(n, n)
}

/// A pad of model `model` at path `path`.
fn twin(model: u8, path: u8) -> PadInfo {
    PadInfo {
        identity: DeviceIdentity::from_parts(&[b"pad", &[model]]),
        path: DeviceIdentity::from_parts(&[b"/dev/input/event", &[path]]),
        name: format!("Pad {model}"),
    }
}

fn game() -> Mapper {
    Mapper::with_rule(PortRule::Game, &Roster::new())
}

/// Connects `info` and returns its device id.
fn connect(mapper: &mut Mapper, info: PadInfo) -> u32 {
    let id = mapper.connect_gamepad(info).unwrap();
    mapper.device_of(id).unwrap()
}

fn port(mapper: &Mapper, id: PadId) -> Option<u8> {
    mapper.device_of(id).and_then(port_of)
}

/// The model and presence on each port, `None` for a free port.
fn seats(mapper: &Mapper) -> Vec<Option<(String, bool)>> {
    mapper
        .roster()
        .ports()
        .iter()
        .map(|e| e.as_ref().map(|e| (e.pad.name.clone(), e.present)))
        .collect()
}

fn press(button: PadButton) -> RawInput {
    RawInput::Button {
        button,
        pressed: true,
    }
}

fn release(button: PadButton) -> RawInput {
    RawInput::Button {
        button,
        pressed: false,
    }
}

fn axis(axis: PadAxis, value: i16) -> RawInput {
    RawInput::Axis { axis, value }
}

fn key(code: u16, action: KeyAction) -> RawInput {
    RawInput::Key {
        code: KeyCode(code),
        action,
    }
}

fn ev(device: u32, control: Control, value: i16) -> InputEvent {
    InputEvent {
        device,
        control,
        value,
    }
}

#[test]
fn gamepads_take_the_lowest_free_port_in_connection_order() {
    let mut mapper = Mapper::new();
    let first = connect(&mut mapper, pad(1));
    let second = connect(&mut mapper, pad(2));
    assert_eq!(first, GAMEPAD_DEVICE_BASE);
    assert_eq!(second, GAMEPAD_DEVICE_BASE + 1);
    assert_eq!(port_of(first), Some(0));
    assert_eq!(port_of(second), Some(1));
}

#[test]
fn a_ninth_gamepad_gets_no_port() {
    for mut mapper in [Mapper::new(), game()] {
        for n in 0..MAX_PORTS {
            mapper.connect_gamepad(pad(n)).unwrap();
        }
        assert_eq!(
            mapper.connect_gamepad(pad(99)),
            Err(MapperError::NoFreePort { max: MAX_PORTS })
        );
    }
    assert_eq!(
        MapperError::NoFreePort { max: 8 }.to_string(),
        "every one of the 8 gamepad ports is in use"
    );
}

#[test]
fn a_disconnect_releases_every_held_control_and_frees_the_port() {
    let mut mapper = Mapper::new();
    let device = connect(&mut mapper, pad(1));
    let mut out = Vec::new();
    mapper
        .apply(device, press(PadButton::South), &mut out)
        .unwrap();
    mapper
        .apply(device, press(PadButton::DPadLeft), &mut out)
        .unwrap();
    mapper
        .apply(device, axis(PadAxis::LeftX, i16::MAX), &mut out)
        .unwrap();
    mapper
        .apply(device, axis(PadAxis::RightTrigger, i16::MAX), &mut out)
        .unwrap();
    out.clear();

    mapper.disconnect(device, &mut out).unwrap();
    assert_eq!(
        out,
        [
            ev(device, Control::B, 0),
            ev(device, Control::Left, 0),
            ev(device, Control::R2, 0),
            ev(device, Control::LeftX, 0),
        ]
    );
    assert!(!mapper.is_connected(device));
    assert_eq!(
        mapper.apply(device, press(PadButton::South), &mut out),
        Err(MapperError::NotConnected { device })
    );
    assert_eq!(
        mapper.disconnect(device, &mut out),
        Err(MapperError::NotConnected { device })
    );
}

#[test]
fn a_pad_that_is_gone_is_not_connected_by_its_id() {
    let mut mapper = Mapper::new();
    let mut out = Vec::new();
    let id = mapper.connect_gamepad(pad(1)).unwrap();
    mapper.disconnect_gamepad(id, &mut out).unwrap();
    assert_eq!(mapper.device_of(id), None);
    assert_eq!(
        mapper.disconnect_gamepad(id, &mut out),
        Err(MapperError::PadNotConnected { pad: id })
    );
    assert_eq!(
        mapper.apply_gamepad(id, press(PadButton::South), &mut out),
        Err(MapperError::PadNotConnected { pad: id })
    );
    assert!(out.is_empty());
}

#[test]
fn in_the_menus_the_pads_above_a_gone_pad_move_down() {
    let mut mapper = Mapper::new();
    let mut out = Vec::new();
    let a = mapper.connect_gamepad(pad(1)).unwrap();
    let b = mapper.connect_gamepad(pad(2)).unwrap();
    let c = mapper.connect_gamepad(pad(3)).unwrap();
    mapper
        .apply_gamepad(c, press(PadButton::Start), &mut out)
        .unwrap();
    out.clear();

    mapper.disconnect_gamepad(a, &mut out).unwrap();
    assert_eq!(port(&mapper, b), Some(0));
    assert_eq!(port(&mapper, c), Some(1));
    // C held Start on port 2: it is released there, and C starts neutral
    // on port 1, so the next read gives the press again on its new port.
    assert_eq!(out, [ev(GAMEPAD_DEVICE_BASE + 2, Control::Start, 0)]);
    out.clear();
    mapper
        .apply_gamepad(c, press(PadButton::Start), &mut out)
        .unwrap();
    assert_eq!(out, [ev(GAMEPAD_DEVICE_BASE + 1, Control::Start, 1)]);

    // A comes back as a new pad, at the end.
    let a = mapper.connect_gamepad(pad(1)).unwrap();
    assert_eq!(port(&mapper, a), Some(2));
    assert_eq!(
        seats(&mapper)[..4],
        [
            Some(("Pad 2".to_owned(), true)),
            Some(("Pad 3".to_owned(), true)),
            Some(("Pad 1".to_owned(), true)),
            None
        ]
    );
}

#[test]
fn in_a_game_a_gone_pad_keeps_its_port_and_gets_it_back() {
    let mut mapper = game();
    let mut out = Vec::new();
    let a = mapper.connect_gamepad(pad(1)).unwrap();
    let b = mapper.connect_gamepad(pad(2)).unwrap();
    mapper
        .apply_gamepad(a, press(PadButton::East), &mut out)
        .unwrap();
    out.clear();

    mapper.disconnect_gamepad(a, &mut out).unwrap();
    assert_eq!(out, [ev(GAMEPAD_DEVICE_BASE, Control::A, 0)]);
    assert_eq!(port(&mapper, b), Some(1), "nothing moves in a game");
    assert_eq!(seats(&mapper)[0], Some(("Pad 1".to_owned(), false)));

    // A pad of another model does not take player 1.
    let c = mapper.connect_gamepad(pad(3)).unwrap();
    assert_eq!(port(&mapper, c), Some(2));
    // Player 1 comes back on its port, neutral.
    let a = mapper.connect_gamepad(pad(1)).unwrap();
    assert_eq!(port(&mapper, a), Some(0));
    assert_eq!(seats(&mapper)[0], Some(("Pad 1".to_owned(), true)));
    out.clear();
    mapper
        .apply_gamepad(a, release(PadButton::East), &mut out)
        .unwrap();
    assert!(out.is_empty());
}

#[test]
fn a_held_port_takes_the_same_model_at_a_new_path() {
    let mut mapper = game();
    let mut out = Vec::new();
    let a = mapper.connect_gamepad(twin(1, 10)).unwrap();
    mapper.disconnect_gamepad(a, &mut out).unwrap();
    // A replug can give the pad another event node.
    let a = mapper.connect_gamepad(twin(1, 11)).unwrap();
    assert_eq!(port(&mapper, a), Some(0));
}

#[test]
fn a_fifth_pad_takes_an_extra_port_and_moves_down_in_the_menus() {
    let mut mapper = Mapper::new();
    let mut out = Vec::new();
    let ids: Vec<_> = (1..=5)
        .map(|n| mapper.connect_gamepad(pad(n)).unwrap())
        .collect();
    assert_eq!(port(&mapper, ids[4]), Some(4));
    mapper.disconnect_gamepad(ids[1], &mut out).unwrap();
    assert_eq!(port(&mapper, ids[4]), Some(3));
}

#[test]
fn in_a_game_a_fifth_pad_waits_above_the_held_ports() {
    let mut mapper = game();
    let mut out = Vec::new();
    let ids: Vec<_> = (1..=4)
        .map(|n| mapper.connect_gamepad(pad(n)).unwrap())
        .collect();
    mapper.disconnect_gamepad(ids[0], &mut out).unwrap();
    let fifth = mapper.connect_gamepad(pad(5)).unwrap();
    assert_eq!(port(&mapper, fifth), Some(4));
}

#[test]
fn held_ports_count_as_used_for_a_ninth_pad() {
    let mut mapper = game();
    let mut out = Vec::new();
    for n in 0..MAX_PORTS {
        let id = mapper.connect_gamepad(pad(n)).unwrap();
        mapper.disconnect_gamepad(id, &mut out).unwrap();
    }
    assert_eq!(
        mapper.connect_gamepad(pad(99)),
        Err(MapperError::NoFreePort { max: MAX_PORTS })
    );
    // A pad that has a held port still gets it.
    let back = mapper.connect_gamepad(pad(3)).unwrap();
    assert_eq!(port(&mapper, back), Some(3));
}

#[test]
fn two_pads_of_one_model_are_kept_apart_by_their_path() {
    let mut mapper = game();
    let mut out = Vec::new();
    let a = mapper.connect_gamepad(twin(7, 1)).unwrap();
    let b = mapper.connect_gamepad(twin(7, 2)).unwrap();
    mapper.disconnect_gamepad(a, &mut out).unwrap();
    mapper.disconnect_gamepad(b, &mut out).unwrap();
    // Back in the other order, at the same paths: each finds its port.
    let b = mapper.connect_gamepad(twin(7, 2)).unwrap();
    let a = mapper.connect_gamepad(twin(7, 1)).unwrap();
    assert_eq!(port(&mapper, a), Some(0));
    assert_eq!(port(&mapper, b), Some(1));
}

#[test]
fn two_pads_of_one_model_at_new_paths_can_swap() {
    let mut mapper = game();
    let mut out = Vec::new();
    let a = mapper.connect_gamepad(twin(7, 1)).unwrap();
    let b = mapper.connect_gamepad(twin(7, 2)).unwrap();
    mapper.disconnect_gamepad(a, &mut out).unwrap();
    mapper.disconnect_gamepad(b, &mut out).unwrap();
    // B comes back first at a new path: it takes the lower held port.
    let b = mapper.connect_gamepad(twin(7, 3)).unwrap();
    let a = mapper.connect_gamepad(twin(7, 4)).unwrap();
    assert_eq!(port(&mapper, b), Some(0));
    assert_eq!(port(&mapper, a), Some(1));
}

/// The roster of a run with pads 1, 2 and 3 on ports 0 to 2.
fn earlier() -> Roster {
    let mut roster = Roster::new();
    for n in 1..=3 {
        let entry = RosterEntry {
            pad: pad(n),
            present: true,
        };
        roster.set(n - 1, Some(entry)).unwrap();
    }
    roster
}

#[test]
fn a_menu_run_keeps_the_order_of_the_run_before_and_drops_missing_pads() {
    let mut mapper = Mapper::with_rule(PortRule::Menus, &earlier());
    let mut out = Vec::new();
    // SDL lists the pads in another order, and pad 1 is gone.
    let new = mapper.connect_gamepad(pad(9)).unwrap();
    let three = mapper.connect_gamepad(pad(3)).unwrap();
    let two = mapper.connect_gamepad(pad(2)).unwrap();
    assert_eq!(port(&mapper, three), Some(2));
    assert_eq!(port(&mapper, two), Some(1));
    assert_eq!(port(&mapper, new), Some(3));
    mapper.settle(&mut out);
    assert!(out.is_empty(), "the pads are neutral before the first read");
    assert_eq!(port(&mapper, two), Some(0));
    assert_eq!(port(&mapper, three), Some(1));
    assert_eq!(port(&mapper, new), Some(2));
}

#[test]
fn a_game_run_keeps_the_ports_of_missing_pads() {
    let mut mapper = Mapper::with_rule(PortRule::Game, &earlier());
    let mut out = Vec::new();
    let three = mapper.connect_gamepad(pad(3)).unwrap();
    mapper.settle(&mut out);
    assert_eq!(port(&mapper, three), Some(2));
    assert_eq!(
        seats(&mapper)[..4],
        [
            Some(("Pad 1".to_owned(), false)),
            Some(("Pad 2".to_owned(), false)),
            Some(("Pad 3".to_owned(), true)),
            None
        ]
    );
    let one = mapper.connect_gamepad(pad(1)).unwrap();
    assert_eq!(port(&mapper, one), Some(0));
}

#[test]
fn the_roster_counter_changes_with_each_hotplug() {
    let mut mapper = Mapper::new();
    let mut out = Vec::new();
    let start = mapper.roster_changes();
    let a = mapper.connect_gamepad(pad(1)).unwrap();
    let connected = mapper.roster_changes();
    assert_ne!(connected, start);
    mapper
        .apply_gamepad(a, press(PadButton::South), &mut out)
        .unwrap();
    assert_eq!(mapper.roster_changes(), connected, "input is no change");
    mapper.disconnect_gamepad(a, &mut out).unwrap();
    assert_ne!(mapper.roster_changes(), connected);
}

#[test]
fn a_reconnected_device_starts_neutral() {
    let mut mapper = Mapper::new();
    let mut out = Vec::new();
    let a = connect(&mut mapper, pad(1));
    mapper.apply(a, press(PadButton::Start), &mut out).unwrap();
    mapper.disconnect(a, &mut out).unwrap();
    assert_eq!(connect(&mut mapper, pad(1)), a);
    out.clear();
    // Start is up after the reconnect, so a release gives no event and a
    // press gives one.
    mapper
        .apply(a, release(PadButton::Start), &mut out)
        .unwrap();
    assert!(out.is_empty());
    mapper.apply(a, press(PadButton::Start), &mut out).unwrap();
    assert_eq!(out, [ev(a, Control::Start, 1)]);
}

#[test]
fn two_devices_on_two_ports_keep_their_own_state() {
    let mut mapper = game();
    let mut out = Vec::new();
    let one = connect(&mut mapper, pad(1));
    let two = connect(&mut mapper, pad(2));
    mapper.apply(one, press(PadButton::East), &mut out).unwrap();
    mapper.apply(two, press(PadButton::East), &mut out).unwrap();
    mapper
        .apply(one, release(PadButton::East), &mut out)
        .unwrap();
    assert_eq!(
        out,
        [
            ev(one, Control::A, 1),
            ev(two, Control::A, 1),
            ev(one, Control::A, 0),
        ]
    );
    out.clear();
    mapper.disconnect(one, &mut out).unwrap();
    assert!(out.is_empty());
    mapper.disconnect(two, &mut out).unwrap();
    assert_eq!(out, [ev(two, Control::A, 0)]);
}

#[test]
fn only_a_change_gives_an_event() {
    let mut mapper = Mapper::new();
    let mut out = Vec::new();
    let device = connect(&mut mapper, pad(1));
    // A backend feeds the full state each frame: nothing changes here.
    for _ in 0..3 {
        mapper
            .apply(device, release(PadButton::South), &mut out)
            .unwrap();
        mapper
            .apply(device, axis(PadAxis::LeftY, STICK_DEAD_ZONE), &mut out)
            .unwrap();
        mapper
            .apply(
                device,
                axis(PadAxis::LeftTrigger, TRIGGER_THRESHOLD - 1),
                &mut out,
            )
            .unwrap();
    }
    assert!(out.is_empty());
    mapper
        .apply(
            device,
            axis(PadAxis::LeftTrigger, TRIGGER_THRESHOLD),
            &mut out,
        )
        .unwrap();
    mapper
        .apply(device, axis(PadAxis::LeftTrigger, i16::MAX), &mut out)
        .unwrap();
    mapper
        .apply(device, axis(PadAxis::LeftY, -i16::MAX), &mut out)
        .unwrap();
    assert_eq!(
        out,
        [
            ev(device, Control::L2, 1),
            ev(device, Control::LeftY, -i16::MAX)
        ]
    );
}

#[test]
fn a_hat_drives_the_d_pad() {
    let mut mapper = Mapper::new();
    let mut out = Vec::new();
    let device = connect(&mut mapper, pad(1));
    let up_right = HatState(HatState::UP | HatState::RIGHT);
    mapper
        .apply(device, RawInput::Hat(up_right), &mut out)
        .unwrap();
    mapper
        .apply(device, RawInput::Hat(HatState(HatState::RIGHT)), &mut out)
        .unwrap();
    mapper
        .apply(device, RawInput::Hat(HatState::default()), &mut out)
        .unwrap();
    assert_eq!(
        out,
        [
            ev(device, Control::Up, 1),
            ev(device, Control::Right, 1),
            ev(device, Control::Up, 0),
            ev(device, Control::Right, 0),
        ]
    );
}

#[test]
fn keyboards_take_slots_and_ignore_repeats() {
    let mut mapper = Mapper::new();
    let mut out = Vec::new();
    let kb = mapper.connect_keyboard().unwrap();
    assert_eq!(kb, KEYBOARD_DEVICE_BASE);
    assert_eq!(port_of(kb), Some(0));
    let enter = 28;
    let escape = 1;
    mapper
        .apply(kb, key(enter, KeyAction::Press), &mut out)
        .unwrap();
    mapper
        .apply(kb, key(enter, KeyAction::Repeat), &mut out)
        .unwrap();
    mapper
        .apply(kb, key(escape, KeyAction::Press), &mut out)
        .unwrap();
    mapper
        .apply(kb, key(enter, KeyAction::Release), &mut out)
        .unwrap();
    assert_eq!(out, [ev(kb, Control::Start, 1), ev(kb, Control::Start, 0)]);
}

#[test]
fn a_lost_keyboard_releases_its_keys_and_frees_its_slot() {
    let mut mapper = Mapper::new();
    let mut out = Vec::new();
    let kb = mapper.connect_keyboard().unwrap();
    mapper
        .apply(kb, key(103, KeyAction::Press), &mut out)
        .unwrap();
    out.clear();
    mapper.disconnect(kb, &mut out).unwrap();
    assert_eq!(out, [ev(kb, Control::Up, 0)]);
    assert_eq!(mapper.connect_keyboard().unwrap(), kb);
}

#[test]
fn every_keyboard_slot_can_fill() {
    let mut mapper = Mapper::new();
    for slot in 0..MAX_KEYBOARDS {
        assert_eq!(
            mapper.connect_keyboard().unwrap(),
            KEYBOARD_DEVICE_BASE + u32::from(slot)
        );
    }
    assert_eq!(
        mapper.connect_keyboard(),
        Err(MapperError::NoFreeKeyboardSlot { max: MAX_KEYBOARDS })
    );
}

#[test]
fn ids_outside_the_mapper_ranges_are_not_connected() {
    let mut mapper = Mapper::new();
    let mut out = Vec::new();
    for device in [
        0,
        7,
        GAMEPAD_DEVICE_BASE + 8,
        KEYBOARD_DEVICE_BASE + 8,
        u32::MAX,
    ] {
        assert!(!mapper.is_connected(device));
        assert_eq!(
            mapper.apply(device, press(PadButton::South), &mut out),
            Err(MapperError::NotConnected { device })
        );
    }
    assert!(out.is_empty());
}

#[test]
fn a_flood_of_events_in_one_frame_keeps_its_order_and_ends_in_the_last_state() {
    let mut mapper = Mapper::new();
    let mut out = Vec::with_capacity(4096);
    let device = connect(&mut mapper, pad(1));
    for i in 0..1000 {
        mapper
            .apply(device, press(PadButton::South), &mut out)
            .unwrap();
        mapper
            .apply(device, release(PadButton::South), &mut out)
            .unwrap();
        let value = if i % 2 == 0 { i16::MAX } else { i16::MIN };
        mapper
            .apply(device, axis(PadAxis::RightX, value), &mut out)
            .unwrap();
    }
    assert_eq!(out.len(), 3000);
    assert_eq!(out[0], ev(device, Control::B, 1));
    assert_eq!(out[1], ev(device, Control::B, 0));
    assert_eq!(out[2], ev(device, Control::RightX, i16::MAX));
    assert_eq!(out[2999], ev(device, Control::RightX, -i16::MAX));
    // The buffer had room, so the mapper did not grow it.
    assert_eq!(out.capacity(), 4096);
}

#[test]
fn the_same_script_gives_the_same_events_twice() {
    fn run(rule: PortRule) -> Vec<InputEvent> {
        let mut mapper = Mapper::with_rule(rule, &Roster::new());
        let mut out = Vec::new();
        let mut seed: u32 = 0x1234_5678;
        let mut pads = Vec::new();
        for step in 0..5000u32 {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let pick = seed >> 16;
            match pick % 10 {
                0 if pads.len() < 4 => {
                    let n = u8::try_from(pick % 6).unwrap();
                    if let Ok(id) = mapper.connect_gamepad(pad(n)) {
                        pads.push(id);
                    }
                }
                1 if !pads.is_empty() => {
                    let id = pads.remove(step as usize % pads.len());
                    mapper.disconnect_gamepad(id, &mut out).unwrap();
                }
                _ if !pads.is_empty() => {
                    let id = pads[step as usize % pads.len()];
                    let value = i16::try_from(pick % 65_536 / 2).unwrap();
                    let input = match pick % 3 {
                        0 => press(PadButton::North),
                        1 => release(PadButton::North),
                        _ => axis(PadAxis::LeftX, value),
                    };
                    mapper.apply_gamepad(id, input, &mut out).unwrap();
                }
                _ => {}
            }
        }
        out
    }
    for rule in [PortRule::Menus, PortRule::Game] {
        let first = run(rule);
        assert!(first.len() > 1000);
        assert_eq!(first, run(rule));
    }
}

#[test]
fn a_scan_gives_each_pad_of_one_model_the_port_held_for_its_path() {
    // The run before had model 7 at path 2 on port 0 and at path 1 on
    // port 1. The scan finds a new pad of model 7 at path 3 first.
    let mut earlier = Roster::new();
    for (port, path) in [(0, 2), (1, 1)] {
        let entry = RosterEntry {
            pad: twin(7, path),
            present: true,
        };
        earlier.set(port, Some(entry)).unwrap();
    }
    let mut mapper = Mapper::with_rule(PortRule::Game, &earlier);
    let mut out = Vec::new();
    let ids = mapper.connect_scan(vec![twin(7, 3), twin(7, 2), twin(7, 1)], &mut out);
    let ports: Vec<_> = ids
        .iter()
        .map(|id| port(&mapper, *id.as_ref().unwrap()))
        .collect();
    assert_eq!(ports, [Some(2), Some(0), Some(1)]);
}

#[test]
fn the_first_scan_of_the_menus_frees_held_ports_before_it_refuses_a_pad() {
    // Eight pads of the run before are gone; eight new pads come.
    let mut earlier = Roster::new();
    for n in 0..MAX_PORTS {
        let entry = RosterEntry {
            pad: pad(100 + n),
            present: true,
        };
        earlier.set(n, Some(entry)).unwrap();
    }
    let mut mapper = Mapper::with_rule(PortRule::Menus, &earlier);
    let mut out = Vec::new();
    let new: Vec<_> = (0..MAX_PORTS).map(pad).collect();
    let ids = mapper.connect_scan(new, &mut out);
    let ports: Vec<_> = ids
        .iter()
        .map(|id| port(&mapper, *id.as_ref().unwrap()))
        .collect();
    assert_eq!(ports, (0..MAX_PORTS).map(Some).collect::<Vec<_>>());
    // A later scan settles nothing again: a ninth pad is refused.
    let late = mapper.connect_scan(vec![pad(99)], &mut out);
    assert_eq!(late, [Err(MapperError::NoFreePort { max: MAX_PORTS })]);
}

#[test]
fn in_a_game_the_first_scan_keeps_the_held_ports() {
    let mut earlier = Roster::new();
    let entry = RosterEntry {
        pad: pad(1),
        present: true,
    };
    earlier.set(0, Some(entry)).unwrap();
    let mut mapper = Mapper::with_rule(PortRule::Game, &earlier);
    let mut out = Vec::new();
    let ids = mapper.connect_scan(vec![pad(2)], &mut out);
    assert_eq!(port(&mapper, ids[0].unwrap()), Some(1));
    assert_eq!(seats(&mapper)[0], Some(("Pad 1".to_owned(), false)));
}
