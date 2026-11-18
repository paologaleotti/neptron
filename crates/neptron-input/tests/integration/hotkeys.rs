use neptron_backend::{Clock, InputSource};
use neptron_input::{
    BLIND_RECOVERY_HOLD_NS, DeviceIdentity, GAMEPAD_DEVICE_BASE, Hotkey, Hotkeys, KEY_MAP,
    KEYBOARD_DEVICE_BASE, Mapper, PadButton, PadInfo, PortStates, QUICK_MENU_HOLD_NS, RawInput,
};
use neptron_testkit::{ScriptedInput, SimClock};
use neptron_types::{Control, InputEvent};

/// A frame period that divides every time of the scripts, so that each
/// scripted change lands on a frame.
const FRAME_NS: u64 = 10_000_000;
const MS: u64 = 1_000_000;

/// The frontend loop, reduced to input: one poll per frame, the port
/// state, the hotkeys.
struct Frontend {
    clock: SimClock,
    input: ScriptedInput,
    events: Vec<InputEvent>,
    ports: PortStates,
    hotkeys: Hotkeys,
    fired: Vec<(u64, Hotkey)>,
    out: Vec<Hotkey>,
}

impl Frontend {
    fn new() -> Self {
        let clock = SimClock::new();
        Self {
            input: ScriptedInput::new(clock.clone(), Vec::new()),
            clock,
            events: Vec::with_capacity(64),
            ports: PortStates::new(),
            hotkeys: Hotkeys::new(),
            fired: Vec::new(),
            out: Vec::with_capacity(8),
        }
    }

    fn frame(&mut self) {
        self.events.clear();
        self.input.poll(&mut self.events);
        self.ports.apply(&self.events);
        self.out.clear();
        let now = self.clock.now_ns();
        self.hotkeys.update(&self.ports, now, &mut self.out);
        self.fired
            .extend(self.out.iter().map(|hotkey| (now, *hotkey)));
    }

    /// Runs frames until the clock reaches `end_ns`, then one frame exactly
    /// at `end_ns`.
    fn run_to(&mut self, end_ns: u64) {
        while self.clock.now_ns() + FRAME_NS < end_ns {
            self.frame();
            self.clock.advance(FRAME_NS);
        }
        self.clock.advance_to(end_ns).unwrap();
        self.frame();
    }

    fn recoveries(&self) -> Vec<u64> {
        self.fired
            .iter()
            .filter(|(_, hotkey)| matches!(hotkey, Hotkey::BlindRecovery { .. }))
            .map(|(time, _)| *time)
            .collect()
    }

    fn menus(&self) -> Vec<(u64, u8)> {
        self.fired
            .iter()
            .filter_map(|(time, hotkey)| match hotkey {
                Hotkey::QuickMenu { port } => Some((*time, *port)),
                Hotkey::BlindRecovery { .. } => None,
            })
            .collect()
    }

    fn hold_combo(&self, time_ns: u64, device: u32) {
        self.input.press(time_ns, device, Control::Select);
        self.input.press(time_ns, device, Control::Start);
    }
}

/// The control of the keyboard key with the RetroArch name `name`.
fn key(name: &str) -> Control {
    KEY_MAP
        .iter()
        .find(|(_, key, _)| *key == name)
        .map(|(_, _, control)| *control)
        .expect("a mapped key")
}

/// A pad on port 0, a pad on extra port 5, and the keyboard pair Right
/// Shift and Enter on port 0, with the port of each.
fn sources() -> [(u32, [Control; 2], u8); 3] {
    [
        (0, [Control::Select, Control::Start], 0),
        (
            GAMEPAD_DEVICE_BASE + 5,
            [Control::Select, Control::Start],
            5,
        ),
        (KEYBOARD_DEVICE_BASE + 1, [key("rshift"), key("enter")], 0),
    ]
}

#[test]
fn the_keyboard_pair_is_select_and_start() {
    assert_eq!(key("rshift"), Control::Select);
    assert_eq!(key("enter"), Control::Start);
}

#[test]
fn a_coin_and_a_start_that_overlap_for_less_than_the_hold_open_nothing() {
    for (device, [select, start], _) in sources() {
        let mut fe = Frontend::new();
        // The coin, then Start while the coin is still down.
        fe.input.press(100 * MS, device, select);
        fe.input.press(200 * MS, device, start);
        fe.input.release(300 * MS, device, select);
        fe.input.release(400 * MS, device, start);
        // Start first, then the coin, with the longest overlap that is
        // still short.
        let overlap = QUICK_MENU_HOLD_NS - FRAME_NS;
        fe.input.press(SECOND, device, start);
        fe.input.press(SECOND + 50 * MS, device, select);
        fe.input.release(SECOND + 50 * MS + overlap, device, start);
        fe.input.release(2 * SECOND, device, select);
        fe.run_to(4 * SECOND);
        assert_eq!(fe.fired, [], "{device:#x}");
    }
}

#[test]
fn a_hold_of_exactly_the_hold_time_fires_once_at_the_threshold() {
    let mut bench = Bench::new(0);
    let t0 = 7 * MS;
    bench.set(0, Control::Select, true);
    assert_eq!(bench.update(0), []);
    bench.set(0, Control::Start, true);
    assert_eq!(bench.update(t0), []);
    assert_eq!(bench.update(t0 + QUICK_MENU_HOLD_NS - 1), []);
    assert_eq!(
        bench.update(t0 + QUICK_MENU_HOLD_NS),
        [Hotkey::QuickMenu { port: 0 }]
    );
    assert_eq!(bench.update(t0 + QUICK_MENU_HOLD_NS + 1), []);
    bench.set(0, Control::Start, false);
    assert_eq!(bench.update(t0 + SECOND), [], "nothing at the release");
}

#[test]
fn every_source_opens_the_quick_menu_at_the_hold_time_and_not_at_the_release() {
    for (device, [select, start], port) in sources() {
        let mut fe = Frontend::new();
        fe.input.press(100 * MS, device, select);
        fe.input.press(100 * MS, device, start);
        fe.input.release(2 * SECOND, device, start);
        fe.input.release(2 * SECOND, device, select);
        fe.run_to(100 * MS + QUICK_MENU_HOLD_NS - FRAME_NS);
        assert_eq!(fe.fired, [], "{device:#x}");
        fe.run_to(100 * MS + QUICK_MENU_HOLD_NS);
        assert_eq!(
            fe.fired,
            [(100 * MS + QUICK_MENU_HOLD_NS, Hotkey::QuickMenu { port })],
            "{device:#x}"
        );
        fe.run_to(4 * SECOND);
        assert_eq!(fe.fired.len(), 1, "{device:#x}");
    }
}

#[test]
fn a_longer_hold_does_not_fire_twice() {
    let mut fe = Frontend::new();
    fe.hold_combo(0, 0);
    fe.run_to(BLIND_RECOVERY_HOLD_NS - FRAME_NS);
    assert_eq!(
        fe.fired,
        [(QUICK_MENU_HOLD_NS, Hotkey::QuickMenu { port: 0 })]
    );
}

#[test]
fn a_break_in_the_hold_restarts_the_time() {
    let mut fe = Frontend::new();
    fe.hold_combo(0, 0);
    fe.input.release(400 * MS, 0, Control::Start);
    fe.input.press(450 * MS, 0, Control::Start);
    fe.run_to(450 * MS + QUICK_MENU_HOLD_NS - FRAME_NS);
    assert_eq!(fe.menus(), []);
    fe.run_to(450 * MS + QUICK_MENU_HOLD_NS);
    assert_eq!(fe.menus(), [(450 * MS + QUICK_MENU_HOLD_NS, 0)]);
}

#[test]
fn a_release_of_one_button_and_a_new_press_give_a_new_hold() {
    let mut fe = Frontend::new();
    fe.hold_combo(0, 0);
    fe.input.release(SECOND, 0, Control::Select);
    fe.input.press(1200 * MS, 0, Control::Select);
    fe.input.release(3 * SECOND, 0, Control::Start);
    fe.input.release(3 * SECOND, 0, Control::Select);
    fe.hold_combo(4 * SECOND, 0);
    fe.run_to(5 * SECOND);
    assert_eq!(
        fe.menus(),
        [
            (QUICK_MENU_HOLD_NS, 0),
            (1200 * MS + QUICK_MENU_HOLD_NS, 0),
            (4 * SECOND + QUICK_MENU_HOLD_NS, 0),
        ]
    );
    assert_eq!(fe.recoveries(), []);
}

#[test]
fn a_cancelled_hold_opens_no_menu_and_the_recovery_hold_goes_on() {
    let mut bench = Bench::new(0);
    bench.set(0, Control::Select, true);
    bench.set(0, Control::Start, true);
    assert_eq!(bench.update(0), []);
    bench.hotkeys.cancel_quick_menu();
    assert_eq!(bench.update(SECOND), []);
    assert_eq!(
        bench.update(BLIND_RECOVERY_HOLD_NS),
        [Hotkey::BlindRecovery { port: 0 }]
    );
    bench.set(0, Control::Start, false);
    assert_eq!(bench.update(BLIND_RECOVERY_HOLD_NS + 1), []);
    bench.set(0, Control::Start, true);
    assert_eq!(bench.update(7 * SECOND), []);
    assert_eq!(
        bench.update(7 * SECOND + QUICK_MENU_HOLD_NS),
        [Hotkey::QuickMenu { port: 0 }]
    );
}

#[test]
fn a_cancel_keeps_the_next_hold_of_a_port_that_was_up() {
    let mut bench = Bench::new(0);
    assert_eq!(bench.update(0), []);
    bench.hotkeys.cancel_quick_menu();
    bench.set(0, Control::Select, true);
    bench.set(0, Control::Start, true);
    assert_eq!(bench.update(SECOND), []);
    assert_eq!(
        bench.update(SECOND + QUICK_MENU_HOLD_NS),
        [Hotkey::QuickMenu { port: 0 }]
    );
}

#[test]
fn a_recovery_hold_opens_the_quick_menu_first_and_the_recovery_at_5_seconds() {
    let mut fe = Frontend::new();
    fe.hold_combo(0, 0);
    fe.input
        .release(BLIND_RECOVERY_HOLD_NS - 10 * MS, 0, Control::Select);
    fe.hold_combo(6 * SECOND, 0);
    fe.input
        .release(6 * SECOND + 2 * BLIND_RECOVERY_HOLD_NS, 0, Control::Start);
    fe.run_to(4 * BLIND_RECOVERY_HOLD_NS);
    assert_eq!(
        fe.menus(),
        [
            (QUICK_MENU_HOLD_NS, 0),
            (6 * SECOND + QUICK_MENU_HOLD_NS, 0)
        ]
    );
    assert_eq!(fe.recoveries(), [6 * SECOND + BLIND_RECOVERY_HOLD_NS]);
}

#[test]
fn a_hold_of_4_9_seconds_is_not_a_recovery() {
    let mut fe = Frontend::new();
    fe.hold_combo(0, 0);
    fe.run_to(BLIND_RECOVERY_HOLD_NS - 100 * MS);
    assert!(fe.recoveries().is_empty());
}

#[test]
fn a_hold_of_5_seconds_is_a_recovery_and_fires_once() {
    let mut fe = Frontend::new();
    fe.hold_combo(0, 0);
    fe.run_to(BLIND_RECOVERY_HOLD_NS);
    assert_eq!(fe.recoveries(), [BLIND_RECOVERY_HOLD_NS]);
    fe.run_to(3 * BLIND_RECOVERY_HOLD_NS);
    assert_eq!(fe.recoveries(), [BLIND_RECOVERY_HOLD_NS]);
    assert_eq!(
        fe.fired,
        [
            (QUICK_MENU_HOLD_NS, Hotkey::QuickMenu { port: 0 }),
            (BLIND_RECOVERY_HOLD_NS, Hotkey::BlindRecovery { port: 0 })
        ]
    );
}

#[test]
fn a_release_restarts_the_hold() {
    let mut fe = Frontend::new();
    fe.hold_combo(0, 0);
    fe.input.release(3000 * MS, 0, Control::Select);
    fe.input.press(3500 * MS, 0, Control::Select);
    fe.run_to(3500 * MS + BLIND_RECOVERY_HOLD_NS - 100 * MS);
    assert!(fe.recoveries().is_empty());
    fe.run_to(3500 * MS + BLIND_RECOVERY_HOLD_NS);
    assert_eq!(fe.recoveries(), [3500 * MS + BLIND_RECOVERY_HOLD_NS]);
}

#[test]
fn the_combination_must_be_on_one_port() {
    let mut fe = Frontend::new();
    fe.input.press(0, 0, Control::Select);
    fe.input.press(0, 1, Control::Start);
    fe.run_to(2 * BLIND_RECOVERY_HOLD_NS);
    assert!(fe.fired.is_empty());
}

#[test]
fn every_port_and_every_device_of_a_port_counts() {
    let mut fe = Frontend::new();
    // Select on the window keyboard, Start on the first gamepad: both are
    // port 0. Port 3 holds the combination on its own.
    fe.input.press(0, 0, Control::Select);
    fe.input.press(0, 0x100, Control::Start);
    fe.hold_combo(0, 3);
    fe.run_to(BLIND_RECOVERY_HOLD_NS);
    assert_eq!(
        fe.fired,
        [
            (QUICK_MENU_HOLD_NS, Hotkey::QuickMenu { port: 0 }),
            (QUICK_MENU_HOLD_NS, Hotkey::QuickMenu { port: 3 }),
            (BLIND_RECOVERY_HOLD_NS, Hotkey::BlindRecovery { port: 0 }),
            (BLIND_RECOVERY_HOLD_NS, Hotkey::BlindRecovery { port: 3 }),
        ]
    );
}

#[test]
fn the_hold_is_measured_from_the_first_frame_that_sees_it() {
    let mut fe = Frontend::new();
    // Between two frames: the first frame at or after the press starts
    // the hold.
    let press = 5 * MS;
    fe.hold_combo(press, 0);
    fe.run_to(press + BLIND_RECOVERY_HOLD_NS);
    let start = FRAME_NS;
    assert!(fe.recoveries().is_empty());
    fe.run_to(start + BLIND_RECOVERY_HOLD_NS);
    assert_eq!(fe.recoveries(), [start + BLIND_RECOVERY_HOLD_NS]);
}

#[test]
fn the_same_script_fires_the_same_hotkeys_twice() {
    fn run() -> Vec<(u64, Hotkey)> {
        let mut fe = Frontend::new();
        for k in 0..8u64 {
            let device = u32::try_from(k).unwrap();
            fe.hold_combo(k * 1000 * MS, device);
            fe.input
                .release(k * 1000 * MS + k * 2000 * MS, device, Control::Start);
        }
        fe.run_to(30_000 * MS);
        fe.fired
    }
    let first = run();
    // Ports 3 to 7 hold for 6 seconds or more.
    let recoveries: Vec<_> = first
        .iter()
        .filter_map(|(_, h)| match h {
            Hotkey::BlindRecovery { port } => Some(*port),
            Hotkey::QuickMenu { .. } => None,
        })
        .collect();
    assert_eq!(recoveries, [3, 4, 5, 6, 7]);
    assert_eq!(first, run());
}

const SECOND: u64 = 1_000_000_000;

/// The Start hold of an arcade panel.
const HOLD_NS: u64 = 2 * SECOND;

fn event(port: u8, control: Control, pressed: bool) -> InputEvent {
    InputEvent {
        device: u32::from(port),
        control,
        value: i16::from(pressed),
    }
}

/// Port state and hotkeys driven by hand, one update at a time.
struct Bench {
    ports: PortStates,
    hotkeys: Hotkeys,
    out: Vec<Hotkey>,
}

impl Bench {
    fn new(hold_ns: u64) -> Self {
        let mut hotkeys = Hotkeys::new();
        hotkeys.set_start_hold_ns(hold_ns);
        Self {
            ports: PortStates::new(),
            hotkeys,
            out: Vec::new(),
        }
    }

    fn set(&mut self, port: u8, control: Control, pressed: bool) {
        self.ports.apply(&[event(port, control, pressed)]);
    }

    fn update(&mut self, now_ns: u64) -> Vec<Hotkey> {
        self.out.clear();
        self.hotkeys.update(&self.ports, now_ns, &mut self.out);
        self.out.clone()
    }
}

#[test]
fn the_start_hold_is_off_by_default() {
    let mut fe = Frontend::new();
    assert_eq!(fe.hotkeys.start_hold_ns(), 0);
    fe.input.press(0, 0, Control::Start);
    fe.run_to(30 * SECOND);
    assert!(fe.fired.is_empty());
}

#[test]
fn a_start_hold_of_0_never_fires() {
    let mut bench = Bench::new(0);
    bench.set(0, Control::Start, true);
    for now in [0, 1, HOLD_NS, BLIND_RECOVERY_HOLD_NS, u64::MAX] {
        assert_eq!(bench.update(now), [], "{now}");
    }
}

#[test]
fn start_alone_fires_once_at_exactly_the_hold_time() {
    let mut bench = Bench::new(HOLD_NS);
    let t0 = 7 * MS;
    bench.set(0, Control::Start, true);
    assert_eq!(bench.update(t0), []);
    assert_eq!(bench.update(t0 + HOLD_NS - 1), []);
    assert_eq!(bench.update(t0 + HOLD_NS), [Hotkey::QuickMenu { port: 0 }]);
    assert_eq!(bench.update(t0 + HOLD_NS + 1), []);
    assert_eq!(bench.update(t0 + 100 * SECOND), []);
}

#[test]
fn the_start_hold_fires_once_per_hold_in_the_frame_loop() {
    let mut fe = Frontend::new();
    fe.hotkeys.set_start_hold_ns(HOLD_NS);
    fe.input.press(0, 0, Control::Start);
    fe.run_to(4 * BLIND_RECOVERY_HOLD_NS);
    assert_eq!(fe.fired, [(HOLD_NS, Hotkey::QuickMenu { port: 0 })]);
}

#[test]
fn a_release_of_start_restarts_the_hold() {
    let mut fe = Frontend::new();
    fe.hotkeys.set_start_hold_ns(HOLD_NS);
    fe.input.press(0, 0, Control::Start);
    fe.input.release(1500 * MS, 0, Control::Start);
    fe.input.press(1700 * MS, 0, Control::Start);
    fe.input
        .release(1700 * MS + HOLD_NS + 500 * MS, 0, Control::Start);
    fe.input.press(6000 * MS, 0, Control::Start);
    fe.run_to(6000 * MS + HOLD_NS - 10 * MS);
    assert_eq!(fe.menus(), [(1700 * MS + HOLD_NS, 0)]);
    fe.run_to(6000 * MS + HOLD_NS);
    assert_eq!(
        fe.menus(),
        [(1700 * MS + HOLD_NS, 0), (6000 * MS + HOLD_NS, 0)]
    );
}

#[test]
fn the_start_hold_is_per_port() {
    let mut bench = Bench::new(HOLD_NS);
    bench.set(2, Control::Start, true);
    assert_eq!(bench.update(0), []);
    bench.set(5, Control::Start, true);
    assert_eq!(bench.update(SECOND), []);
    // Select on another port does not stop the hold of port 2.
    bench.set(3, Control::Select, true);
    assert_eq!(bench.update(HOLD_NS), [Hotkey::QuickMenu { port: 2 }]);
    bench.set(2, Control::Start, false);
    assert_eq!(
        bench.update(SECOND + HOLD_NS),
        [Hotkey::QuickMenu { port: 5 }]
    );
}

#[test]
fn other_buttons_do_not_stop_the_start_hold() {
    let mut bench = Bench::new(HOLD_NS);
    bench.set(0, Control::Start, true);
    assert_eq!(bench.update(0), []);
    bench.set(0, Control::A, true);
    bench.set(0, Control::Left, true);
    assert_eq!(bench.update(SECOND), []);
    bench.set(0, Control::A, false);
    assert_eq!(bench.update(HOLD_NS), [Hotkey::QuickMenu { port: 0 }]);
}

#[test]
fn select_and_start_give_one_quick_menu_and_the_recovery_with_the_hold_on() {
    for hold_ns in [0, 1, HOLD_NS, 10 * SECOND] {
        let mut fe = Frontend::new();
        fe.hotkeys.set_start_hold_ns(hold_ns);
        fe.hold_combo(100 * MS, 0);
        fe.input
            .release(100 * MS + 2 * BLIND_RECOVERY_HOLD_NS, 0, Control::Select);
        fe.run_to(100 * MS + 4 * BLIND_RECOVERY_HOLD_NS);
        assert_eq!(
            fe.fired,
            [
                (100 * MS + QUICK_MENU_HOLD_NS, Hotkey::QuickMenu { port: 0 }),
                (
                    100 * MS + BLIND_RECOVERY_HOLD_NS,
                    Hotkey::BlindRecovery { port: 0 }
                ),
            ],
            "hold {hold_ns}"
        );
    }
}

#[test]
fn select_after_start_stops_the_hold_until_start_is_released() {
    let mut fe = Frontend::new();
    fe.hotkeys.set_start_hold_ns(HOLD_NS);
    fe.input.press(0, 0, Control::Start);
    fe.input.press(SECOND, 0, Control::Select);
    // Select goes up before the combination fires, Start stays down: no
    // quick menu from either.
    fe.input.release(1400 * MS, 0, Control::Select);
    fe.run_to(10 * SECOND);
    assert_eq!(fe.menus(), []);
    assert!(fe.recoveries().is_empty());
    fe.input.release(10 * SECOND, 0, Control::Start);
    fe.input.press(11 * SECOND, 0, Control::Start);
    fe.run_to(11 * SECOND + HOLD_NS);
    assert_eq!(fe.menus(), [(11 * SECOND + HOLD_NS, 0)]);
}

#[test]
fn select_after_a_start_hold_that_fired_gives_no_second_quick_menu() {
    let mut fe = Frontend::new();
    fe.hotkeys.set_start_hold_ns(HOLD_NS);
    fe.input.press(0, 0, Control::Start);
    fe.input.press(3 * SECOND, 0, Control::Select);
    fe.input.release(4 * SECOND, 0, Control::Select);
    fe.input.press(4100 * MS, 0, Control::Select);
    fe.input.release(5 * SECOND, 0, Control::Start);
    fe.input.press(6 * SECOND, 0, Control::Start);
    fe.run_to(6 * SECOND + QUICK_MENU_HOLD_NS);
    assert_eq!(
        fe.menus(),
        [(HOLD_NS, 0), (6 * SECOND + QUICK_MENU_HOLD_NS, 0)],
        "a new press of Start gives a new hold"
    );
}

#[test]
fn select_held_first_stops_the_hold_until_start_is_released() {
    let mut fe = Frontend::new();
    fe.hotkeys.set_start_hold_ns(HOLD_NS);
    fe.input.press(0, 0, Control::Select);
    fe.input.press(100 * MS, 0, Control::Start);
    fe.input.release(200 * MS, 0, Control::Select);
    fe.run_to(10 * SECOND);
    assert_eq!(fe.menus(), []);
}

#[test]
fn the_recovery_still_fires_at_5_seconds_after_a_start_hold() {
    let mut fe = Frontend::new();
    fe.hotkeys.set_start_hold_ns(HOLD_NS);
    fe.input.press(0, 0, Control::Start);
    fe.input.press(3 * SECOND, 0, Control::Select);
    fe.run_to(3 * SECOND + 2 * BLIND_RECOVERY_HOLD_NS);
    assert_eq!(
        fe.fired,
        [
            (HOLD_NS, Hotkey::QuickMenu { port: 0 }),
            (
                3 * SECOND + BLIND_RECOVERY_HOLD_NS,
                Hotkey::BlindRecovery { port: 0 }
            ),
        ]
    );
}

#[test]
fn a_disconnect_during_the_hold_resets_it() {
    let mut mapper = Mapper::new();
    let pad = mapper
        .connect_gamepad(PadInfo {
            identity: DeviceIdentity::from_parts(&[b"pad"]),
            path: DeviceIdentity(0),
            name: "Pad".to_owned(),
        })
        .unwrap();
    let mut ports = PortStates::new();
    let mut hotkeys = Hotkeys::new();
    hotkeys.set_start_hold_ns(HOLD_NS);
    let mut events = Vec::new();
    let mut out = Vec::new();
    let start = |pressed| RawInput::Button {
        button: PadButton::Start,
        pressed,
    };

    mapper.apply_gamepad(pad, start(true), &mut events).unwrap();
    ports.apply(&events);
    hotkeys.update(&ports, 0, &mut out);

    events.clear();
    mapper.disconnect_gamepad(pad, &mut events).unwrap();
    ports.apply(&events);
    hotkeys.update(&ports, SECOND, &mut out);

    events.clear();
    let pad = mapper
        .connect_gamepad(PadInfo {
            identity: DeviceIdentity::from_parts(&[b"pad"]),
            path: DeviceIdentity(0),
            name: "Pad".to_owned(),
        })
        .unwrap();
    mapper.apply_gamepad(pad, start(true), &mut events).unwrap();
    ports.apply(&events);
    hotkeys.update(&ports, 1500 * MS, &mut out);
    hotkeys.update(&ports, HOLD_NS, &mut out);
    hotkeys.update(&ports, 1500 * MS + HOLD_NS - 1, &mut out);
    assert_eq!(out, []);
    hotkeys.update(&ports, 1500 * MS + HOLD_NS, &mut out);
    assert_eq!(out, [Hotkey::QuickMenu { port: 0 }]);
}

#[test]
fn the_start_hold_time_arithmetic_holds_at_the_u64_extremes() {
    let mut bench = Bench::new(HOLD_NS);
    bench.set(0, Control::Start, true);
    assert_eq!(bench.update(u64::MAX - HOLD_NS), []);
    assert_eq!(bench.update(u64::MAX - 1), []);
    assert_eq!(bench.update(u64::MAX), [Hotkey::QuickMenu { port: 0 }]);

    let mut bench = Bench::new(u64::MAX);
    bench.set(0, Control::Start, true);
    assert_eq!(bench.update(0), []);
    assert_eq!(bench.update(u64::MAX - 1), []);
    assert_eq!(bench.update(u64::MAX), [Hotkey::QuickMenu { port: 0 }]);

    // A time that goes back gives no quick menu and no overflow.
    let mut bench = Bench::new(1);
    bench.set(0, Control::Start, true);
    assert_eq!(bench.update(u64::MAX), []);
    assert_eq!(bench.update(0), []);
    assert_eq!(bench.update(u64::MAX - 1), []);
}

#[test]
fn a_new_hold_time_stops_the_hold_in_progress() {
    let mut bench = Bench::new(HOLD_NS);
    bench.set(0, Control::Start, true);
    assert_eq!(bench.update(0), []);
    bench.hotkeys.set_start_hold_ns(SECOND);
    assert_eq!(bench.hotkeys.start_hold_ns(), SECOND);
    assert_eq!(bench.update(SECOND), []);
    assert_eq!(bench.update(10 * SECOND), []);
    bench.set(0, Control::Start, false);
    assert_eq!(bench.update(11 * SECOND), []);
    bench.set(0, Control::Start, true);
    assert_eq!(bench.update(12 * SECOND), []);
    assert_eq!(bench.update(13 * SECOND), [Hotkey::QuickMenu { port: 0 }]);
}

#[test]
fn turning_the_hold_on_during_a_press_waits_for_the_next_press() {
    let mut bench = Bench::new(0);
    bench.set(0, Control::Start, true);
    assert_eq!(bench.update(0), []);
    bench.hotkeys.set_start_hold_ns(HOLD_NS);
    assert_eq!(bench.update(SECOND), []);
    assert_eq!(bench.update(10 * SECOND), []);
    bench.set(0, Control::Start, false);
    assert_eq!(bench.update(11 * SECOND), []);
    bench.set(0, Control::Start, true);
    assert_eq!(bench.update(12 * SECOND), []);
    assert_eq!(
        bench.update(12 * SECOND + HOLD_NS),
        [Hotkey::QuickMenu { port: 0 }]
    );
}

#[test]
fn turning_the_hold_off_during_a_hold_stops_it() {
    let mut bench = Bench::new(HOLD_NS);
    bench.set(0, Control::Start, true);
    assert_eq!(bench.update(0), []);
    bench.hotkeys.set_start_hold_ns(0);
    assert_eq!(bench.update(HOLD_NS), []);
    assert_eq!(bench.update(u64::MAX), []);
}

#[test]
fn the_same_hold_time_again_changes_nothing() {
    let mut bench = Bench::new(HOLD_NS);
    bench.set(0, Control::Start, true);
    assert_eq!(bench.update(0), []);
    bench.hotkeys.set_start_hold_ns(HOLD_NS);
    assert_eq!(bench.update(SECOND), []);
    bench.hotkeys.set_start_hold_ns(HOLD_NS);
    assert_eq!(bench.update(HOLD_NS), [Hotkey::QuickMenu { port: 0 }]);
}

#[test]
fn the_combinations_work_from_every_extra_port_and_from_a_keyboard() {
    use neptron_input::{GAMEPAD_DEVICE_BASE, KEYBOARD_DEVICE_BASE};
    let devices = (4..8)
        .map(|port| (GAMEPAD_DEVICE_BASE + port, u8::try_from(port).unwrap()))
        .chain([(KEYBOARD_DEVICE_BASE + 3, 0)]);
    for (device, port) in devices {
        let mut fe = Frontend::new();
        fe.hold_combo(0, device);
        fe.input.release(300 * MS, device, Control::Start);
        fe.hold_combo(SECOND, device);
        fe.run_to(SECOND + BLIND_RECOVERY_HOLD_NS);
        assert_eq!(
            fe.menus(),
            [(SECOND + QUICK_MENU_HOLD_NS, port)],
            "{device:#x}"
        );
        assert_eq!(
            fe.recoveries(),
            [SECOND + BLIND_RECOVERY_HOLD_NS],
            "{device:#x}"
        );
    }
}

#[test]
fn a_pad_that_moves_in_the_menus_starts_its_recovery_hold_again() {
    let pad = |n: u8| PadInfo {
        identity: DeviceIdentity::from_parts(&[b"pad", &[n]]),
        path: DeviceIdentity(u64::from(n)),
        name: String::new(),
    };
    let mut mapper = Mapper::new();
    let mut ports = PortStates::new();
    let mut hotkeys = Hotkeys::new();
    let mut events = Vec::new();
    let mut fired = Vec::new();
    let one = mapper.connect_gamepad(pad(1)).unwrap();
    let two = mapper.connect_gamepad(pad(2)).unwrap();
    let combo = |mapper: &mut Mapper, events: &mut Vec<InputEvent>| {
        for button in [PadButton::Back, PadButton::Start] {
            let input = RawInput::Button {
                button,
                pressed: true,
            };
            mapper.apply_gamepad(two, input, events).unwrap();
        }
    };
    let mut frame = |events: &mut Vec<InputEvent>, now: u64, fired: &mut Vec<Hotkey>| {
        ports.apply(events);
        events.clear();
        hotkeys.update(&ports, now, fired);
    };
    combo(&mut mapper, &mut events);
    frame(&mut events, 0, &mut fired);
    frame(&mut events, QUICK_MENU_HOLD_NS, &mut fired);
    assert_eq!(fired, [Hotkey::QuickMenu { port: 1 }]);
    fired.clear();
    // Player 1 goes 3 s into the hold: player 2 moves to port 0. The
    // release on port 1 ends the holds there and fires nothing.
    mapper.disconnect_gamepad(one, &mut events).unwrap();
    frame(&mut events, 3 * SECOND, &mut fired);
    assert_eq!(fired, []);
    combo(&mut mapper, &mut events);
    let moved = 3 * SECOND + FRAME_NS;
    frame(&mut events, moved, &mut fired);
    frame(&mut events, moved + QUICK_MENU_HOLD_NS - 1, &mut fired);
    assert_eq!(fired, [], "the holds of port 1 do not carry over");
    frame(&mut events, BLIND_RECOVERY_HOLD_NS, &mut fired);
    assert_eq!(fired, [Hotkey::QuickMenu { port: 0 }]);
    fired.clear();
    frame(
        &mut events,
        3 * SECOND + FRAME_NS + BLIND_RECOVERY_HOLD_NS,
        &mut fired,
    );
    assert_eq!(fired, [Hotkey::BlindRecovery { port: 0 }]);
}
