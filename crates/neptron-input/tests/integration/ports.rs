use neptron_input::{
    DeviceIdentity, GAMEPAD_DEVICE_BASE, KeyAction, KeyCode, Mapper, PadAxis, PadButton,
    PortStates, RawInput,
};
use neptron_types::{Control, InputEvent};

/// The window keyboard of `neptron-window` sends this id.
const WINDOW_KEYBOARD: u32 = 0;

fn ev(device: u32, control: Control, value: i16) -> InputEvent {
    InputEvent {
        device,
        control,
        value,
    }
}

#[test]
fn a_button_of_a_port_is_down_while_any_of_its_devices_holds_it() {
    let mut mapper = Mapper::new();
    let mut events = Vec::new();
    let pad = mapper
        .connect_gamepad(DeviceIdentity::from_parts(&[b"pad"]))
        .unwrap();
    let keyboard = mapper.connect_keyboard().unwrap();
    let enter = RawInput::Key {
        code: KeyCode(28),
        action: KeyAction::Press,
    };
    let start = RawInput::Button {
        button: PadButton::Start,
        pressed: true,
    };
    mapper.apply(keyboard, enter, &mut events).unwrap();
    mapper.apply(pad, start, &mut events).unwrap();
    events.push(ev(WINDOW_KEYBOARD, Control::Start, 1));

    let mut ports = PortStates::new();
    ports.apply(&events);
    assert_eq!(ports.value(0, Control::Start), 1);

    // The keyboard goes away and the window keyboard lets go: the pad
    // still holds Start.
    events.clear();
    mapper.disconnect(keyboard, &mut events).unwrap();
    events.push(ev(WINDOW_KEYBOARD, Control::Start, 0));
    ports.apply(&events);
    assert!(ports.is_held(0, Control::Start));

    events.clear();
    mapper.disconnect(pad, &mut events).unwrap();
    ports.apply(&events);
    assert!(!ports.is_held(0, Control::Start));
}

#[test]
fn an_axis_of_a_port_takes_the_largest_magnitude() {
    let mut ports = PortStates::new();
    ports.apply(&[
        ev(GAMEPAD_DEVICE_BASE, Control::LeftX, 1000),
        ev(0, Control::LeftX, -20_000),
        ev(GAMEPAD_DEVICE_BASE + 1, Control::LeftX, 32_767),
    ]);
    assert_eq!(ports.value(0, Control::LeftX), -20_000);
    assert_eq!(ports.value(1, Control::LeftX), 32_767);
    ports.apply(&[ev(0, Control::LeftX, 0)]);
    assert_eq!(ports.value(0, Control::LeftX), 1000);
}

#[test]
fn an_axis_tie_goes_to_the_device_that_came_first() {
    let mut ports = PortStates::new();
    ports.apply(&[
        ev(GAMEPAD_DEVICE_BASE, Control::RightY, 500),
        ev(0, Control::RightY, -500),
    ]);
    assert_eq!(ports.value(0, Control::RightY), 500);
}

#[test]
fn ports_are_independent() {
    let mut mapper = Mapper::new();
    let mut events = Vec::new();
    let one = mapper
        .connect_gamepad(DeviceIdentity::from_parts(&[b"one"]))
        .unwrap();
    let two = mapper
        .connect_gamepad(DeviceIdentity::from_parts(&[b"two"]))
        .unwrap();
    let tilt = RawInput::Axis {
        axis: PadAxis::LeftY,
        value: i16::MIN,
    };
    mapper.apply(two, tilt, &mut events).unwrap();
    let press = RawInput::Button {
        button: PadButton::South,
        pressed: true,
    };
    mapper.apply(one, press, &mut events).unwrap();
    let mut ports = PortStates::new();
    ports.apply(&events);
    assert_eq!(ports.value(0, Control::B), 1);
    assert_eq!(ports.value(0, Control::LeftY), 0);
    assert_eq!(ports.value(1, Control::B), 0);
    assert_eq!(ports.value(1, Control::LeftY), -i16::MAX);
    assert_eq!(ports.value(2, Control::B), 0);
}
