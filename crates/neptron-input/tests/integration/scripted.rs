use neptron_backend::InputSource;
use neptron_input::{
    DeviceIdentity, GAMEPAD_DEVICE_BASE, PadButton, PadInfo, PadRoster, PortRule, Roster,
    ScriptedPads,
};
use neptron_types::{Control, InputEvent};

fn pad(n: u8) -> PadInfo {
    PadInfo {
        identity: DeviceIdentity::from_parts(&[b"pad", &[n]]),
        path: DeviceIdentity::from_parts(&[b"path", &[n]]),
        name: format!("Pad {n}"),
    }
}

fn poll(pads: &mut ScriptedPads) -> Vec<InputEvent> {
    let mut out = Vec::new();
    pads.poll(&mut out);
    out
}

fn ev(device: u32, control: Control, value: i16) -> InputEvent {
    InputEvent {
        device,
        control,
        value,
    }
}

#[test]
fn scripted_pads_plug_press_and_unplug_at_the_next_poll() {
    let mut pads = ScriptedPads::new(PortRule::Menus, &Roster::new());
    let test = pads.clone();
    let a = test.plug(pad(1));
    let b = test.plug(pad(2));
    assert_eq!(test.device(a), None, "not polled yet");
    let changes = pads.roster_changes();
    assert!(poll(&mut pads).is_empty());
    assert_ne!(pads.roster_changes(), changes);
    assert_eq!(test.device(a), Some(GAMEPAD_DEVICE_BASE));
    assert_eq!(test.device(b), Some(GAMEPAD_DEVICE_BASE + 1));

    test.set(b, PadButton::East, true);
    assert_eq!(
        poll(&mut pads),
        [ev(GAMEPAD_DEVICE_BASE + 1, Control::A, 1)]
    );
    assert!(poll(&mut pads).is_empty(), "a held button gives one event");

    // In the menus B moves down when A goes, with the release on port 1
    // and the press again on port 0.
    test.unplug(a);
    assert_eq!(
        poll(&mut pads),
        [
            ev(GAMEPAD_DEVICE_BASE + 1, Control::A, 0),
            ev(GAMEPAD_DEVICE_BASE, Control::A, 1),
        ]
    );
    assert_eq!(test.device(a), None);
    assert_eq!(
        pads.roster().get(0).map(|e| e.pad.name.as_str()),
        Some("Pad 2")
    );

    test.replug(a);
    poll(&mut pads);
    assert_eq!(test.device(a), Some(GAMEPAD_DEVICE_BASE + 1));
}

#[test]
fn scripted_pads_follow_the_rule_of_a_game() {
    let mut earlier = Roster::new();
    for (port, n) in [(0, 1), (1, 2)] {
        let entry = neptron_input::RosterEntry {
            pad: pad(n),
            present: true,
        };
        earlier.set(port, Some(entry)).unwrap();
    }
    let mut pads = ScriptedPads::new(PortRule::Game, &earlier);
    let test = pads.clone();
    let b = test.plug(pad(2));
    let c = test.plug(pad(3));
    poll(&mut pads);
    assert_eq!(test.device(b), Some(GAMEPAD_DEVICE_BASE + 1));
    assert_eq!(test.device(c), Some(GAMEPAD_DEVICE_BASE + 2));
    let roster = pads.roster();
    let first = roster.get(0).unwrap();
    assert_eq!((first.pad.name.as_str(), first.present), ("Pad 1", false));
    let a = test.plug(pad(1));
    poll(&mut pads);
    assert_eq!(test.device(a), Some(GAMEPAD_DEVICE_BASE));
}

#[test]
fn a_ninth_scripted_pad_waits_for_a_replug() {
    let mut pads = ScriptedPads::new(PortRule::Menus, &Roster::new());
    let test = pads.clone();
    let all: Vec<_> = (0..9).map(|n| test.plug(pad(n))).collect();
    poll(&mut pads);
    assert_eq!(test.device(all[8]), None);
    test.unplug(all[0]);
    poll(&mut pads);
    assert_eq!(test.device(all[8]), None, "a refused pad does not retry");
    test.replug(all[8]);
    poll(&mut pads);
    assert_eq!(test.device(all[8]), Some(GAMEPAD_DEVICE_BASE + 7));
}
