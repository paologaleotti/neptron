use neptron_backend::{Clock, InputSource};
use neptron_input::{BLIND_RECOVERY_HOLD_NS, Hotkey, Hotkeys, PortStates};
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

    fn hold_combo(&self, time_ns: u64, device: u32) {
        self.input.press(time_ns, device, Control::Select);
        self.input.press(time_ns, device, Control::Start);
    }
}

#[test]
fn select_and_start_open_the_quick_menu_once_per_press() {
    let mut fe = Frontend::new();
    fe.hold_combo(100 * MS, 0);
    fe.input.release(400 * MS, 0, Control::Start);
    fe.input.press(600 * MS, 0, Control::Start);
    fe.run_to(1000 * MS);
    let menus: Vec<_> = fe
        .fired
        .iter()
        .filter(|(_, hotkey)| *hotkey == Hotkey::QuickMenu { port: 0 })
        .collect();
    assert_eq!(menus.len(), 2);
    assert!(fe.recoveries().is_empty());
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
        fe.fired[1..],
        [(BLIND_RECOVERY_HOLD_NS, Hotkey::BlindRecovery { port: 0 })]
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
            (0, Hotkey::QuickMenu { port: 0 }),
            (0, Hotkey::QuickMenu { port: 3 }),
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
