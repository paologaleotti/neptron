use neptron_input::{
    DeviceIdentity, GAMEPAD_DEVICE_BASE, HatState, KEYBOARD_DEVICE_BASE, KeyAction, KeyCode,
    MAX_KEYBOARDS, MAX_PORTS, Mapper, MapperError, PadAxis, PadButton, RawInput, STICK_DEAD_ZONE,
    TRIGGER_THRESHOLD, port_of,
};
use neptron_types::{Control, InputEvent};

fn pad(n: u8) -> DeviceIdentity {
    DeviceIdentity::from_parts(&[b"pad", &[n]])
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
    let first = mapper.connect_gamepad(pad(1)).unwrap();
    let second = mapper.connect_gamepad(pad(2)).unwrap();
    assert_eq!(first, GAMEPAD_DEVICE_BASE);
    assert_eq!(second, GAMEPAD_DEVICE_BASE + 1);
    assert_eq!(port_of(first), Some(0));
    assert_eq!(port_of(second), Some(1));
}

#[test]
fn a_ninth_gamepad_gets_no_port() {
    let mut mapper = Mapper::new();
    for n in 0..MAX_PORTS {
        mapper.connect_gamepad(pad(n)).unwrap();
    }
    assert_eq!(
        mapper.connect_gamepad(pad(99)),
        Err(MapperError::NoFreePort { max: MAX_PORTS })
    );
    assert_eq!(
        MapperError::NoFreePort { max: 8 }.to_string(),
        "every one of the 8 gamepad ports is in use"
    );
}

#[test]
fn a_disconnect_releases_every_held_control_and_frees_the_port() {
    let mut mapper = Mapper::new();
    let device = mapper.connect_gamepad(pad(1)).unwrap();
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
fn a_reconnect_gets_its_port_back() {
    let mut mapper = Mapper::new();
    let mut out = Vec::new();
    let a = mapper.connect_gamepad(pad(1)).unwrap();
    let b = mapper.connect_gamepad(pad(2)).unwrap();
    mapper.disconnect(a, &mut out).unwrap();
    mapper.disconnect(b, &mut out).unwrap();
    // Reconnected in the other order: each finds its own port.
    assert_eq!(mapper.connect_gamepad(pad(2)).unwrap(), b);
    assert_eq!(mapper.connect_gamepad(pad(1)).unwrap(), a);
}

#[test]
fn a_new_gamepad_takes_the_lowest_free_port_even_if_another_had_it() {
    let mut mapper = Mapper::new();
    let mut out = Vec::new();
    let a = mapper.connect_gamepad(pad(1)).unwrap();
    mapper.connect_gamepad(pad(2)).unwrap();
    mapper.disconnect(a, &mut out).unwrap();
    // Player one swaps pads: the new pad is player one.
    assert_eq!(mapper.connect_gamepad(pad(3)).unwrap(), a);
    // The old pad comes back and gets the next free port.
    assert_eq!(
        mapper.connect_gamepad(pad(1)).unwrap(),
        GAMEPAD_DEVICE_BASE + 2
    );
}

#[test]
fn a_reconnected_device_starts_neutral() {
    let mut mapper = Mapper::new();
    let mut out = Vec::new();
    let a = mapper.connect_gamepad(pad(1)).unwrap();
    mapper.apply(a, press(PadButton::Start), &mut out).unwrap();
    mapper.disconnect(a, &mut out).unwrap();
    mapper.connect_gamepad(pad(1)).unwrap();
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
    let mut mapper = Mapper::new();
    let mut out = Vec::new();
    let one = mapper.connect_gamepad(pad(1)).unwrap();
    let two = mapper.connect_gamepad(pad(2)).unwrap();
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
    let device = mapper.connect_gamepad(pad(1)).unwrap();
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
    let device = mapper.connect_gamepad(pad(1)).unwrap();
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
    let device = mapper.connect_gamepad(pad(1)).unwrap();
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
    fn run() -> Vec<InputEvent> {
        let mut mapper = Mapper::new();
        let mut out = Vec::new();
        let mut seed: u32 = 0x1234_5678;
        let mut devices = Vec::new();
        for step in 0..5000u32 {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let pick = seed >> 16;
            match pick % 10 {
                0 if devices.len() < 4 => {
                    let n = u8::try_from(pick % 6).unwrap();
                    if let Ok(device) = mapper.connect_gamepad(pad(n)) {
                        devices.push(device);
                    }
                }
                1 if !devices.is_empty() => {
                    let device = devices.remove(step as usize % devices.len());
                    mapper.disconnect(device, &mut out).unwrap();
                }
                _ if !devices.is_empty() => {
                    let device = devices[step as usize % devices.len()];
                    let value = i16::try_from(pick % 65_536 / 2).unwrap();
                    let input = match pick % 3 {
                        0 => press(PadButton::North),
                        1 => release(PadButton::North),
                        _ => axis(PadAxis::LeftX, value),
                    };
                    mapper.apply(device, input, &mut out).unwrap();
                }
                _ => {}
            }
        }
        out
    }
    let first = run();
    assert!(first.len() > 1000);
    assert_eq!(first, run());
}
