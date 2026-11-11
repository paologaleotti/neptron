//! Scripted input on the simulated clock.

use neptron_backend::InputSource;
use neptron_testkit::{ScriptedInput, SimClock, TimedInput};
use neptron_types::{Control, InputEvent};

fn event(control: Control, value: i16) -> InputEvent {
    InputEvent {
        device: 0,
        control,
        value,
    }
}

#[test]
fn events_come_when_their_time_has_come() {
    let clock = SimClock::new();
    let script = vec![
        TimedInput {
            time_ns: 30,
            event: event(Control::B, 1),
        },
        TimedInput {
            time_ns: 10,
            event: event(Control::A, 1),
        },
        TimedInput {
            time_ns: 10,
            event: event(Control::Start, 1),
        },
    ];
    let mut input = ScriptedInput::new(clock.clone(), script);
    let mut out = vec![event(Control::X, 1)];
    input.poll(&mut out);
    assert_eq!(out.len(), 1, "poll does not clear and gives nothing early");

    clock.advance(10);
    input.poll(&mut out);
    assert_eq!(out[1..], [event(Control::A, 1), event(Control::Start, 1)]);
    clock.advance(100);
    out.clear();
    input.poll(&mut out);
    assert_eq!(out, [event(Control::B, 1)]);
    input.poll(&mut out);
    assert_eq!(out.len(), 1);
    assert_eq!(input.pending(), 0);

    let delivered = input.delivered();
    assert_eq!(delivered.len(), 3);
    assert_eq!(delivered[2].poll_ns, 110);
    assert_eq!(delivered[2].input.time_ns, 30);
}

#[test]
fn helpers_add_presses_releases_and_axes() {
    let clock = SimClock::new();
    let mut input = ScriptedInput::new(clock.clone(), Vec::new());
    input.tap(100, 50, 1, Control::Up);
    input.set(120, 1, Control::LeftX, -32_768);
    input.press(200, 2, Control::Select);
    input.release(300, 2, Control::Select);
    assert_eq!(input.pending(), 5);

    clock.advance_to(160).unwrap();
    let mut out = Vec::new();
    input.poll(&mut out);
    let values: Vec<(u32, Control, i16)> =
        out.iter().map(|e| (e.device, e.control, e.value)).collect();
    assert_eq!(
        values,
        [
            (1, Control::Up, 1),
            (1, Control::LeftX, -32_768),
            (1, Control::Up, 0)
        ]
    );
}

#[test]
fn an_event_added_late_comes_at_the_next_poll() {
    let clock = SimClock::starting_at(1_000);
    let mut input = ScriptedInput::new(clock.clone(), Vec::new());
    let probe = input.clone();
    probe.press(500, 0, Control::L);
    probe.press(2_000, 0, Control::R);
    let mut out = Vec::new();
    input.poll(&mut out);
    assert_eq!(out, [event(Control::L, 1)]);
    assert_eq!(probe.pending(), 1);
}
