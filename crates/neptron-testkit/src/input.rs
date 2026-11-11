//! Scripted input: timed RetroPad events replayed on the simulated clock.

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use neptron_backend::{Clock, InputSource};
use neptron_types::{Control, InputEvent};

use crate::SimClock;

/// An input source that replays a script of timed events.
///
/// `poll` gives every event whose time is at or before the clock, in time
/// order, and events with the same time in the order they were added. An
/// event added with a time that has passed comes at the next `poll`.
///
/// Clones share one script, so a test can add events while the code under
/// test polls.
#[derive(Debug, Clone)]
pub struct ScriptedInput {
    clock: SimClock,
    state: Arc<Mutex<State>>,
}

/// One event of a script and its time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimedInput {
    /// When the event happens, in nanoseconds.
    pub time_ns: u64,
    /// The event.
    pub event: InputEvent,
}

/// One event that `poll` gave.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Delivered {
    /// When `poll` gave it, in nanoseconds.
    pub poll_ns: u64,
    /// The scripted event.
    pub input: TimedInput,
}

impl ScriptedInput {
    /// A source that replays `script`.
    pub fn new(clock: SimClock, script: Vec<TimedInput>) -> Self {
        let source = Self {
            clock,
            state: Arc::new(Mutex::new(State {
                script: Vec::with_capacity(script.len()),
                next: 0,
                delivered: Vec::new(),
            })),
        };
        for input in script {
            source.push(input);
        }
        source
    }

    /// Adds `input` to the script.
    pub fn push(&self, input: TimedInput) {
        let mut state = self.lock();
        let next = state.next;
        let at = next + state.script[next..].partition_point(|e| e.time_ns <= input.time_ns);
        state.script.insert(at, input);
    }

    /// Adds a press (value 1) of `control` on `device` at `time_ns`.
    pub fn press(&self, time_ns: u64, device: u32, control: Control) {
        self.set(time_ns, device, control, 1);
    }

    /// Adds a release (value 0) of `control` on `device` at `time_ns`.
    pub fn release(&self, time_ns: u64, device: u32, control: Control) {
        self.set(time_ns, device, control, 0);
    }

    /// Adds a press at `time_ns` and a release `hold_ns` later.
    pub fn tap(&self, time_ns: u64, hold_ns: u64, device: u32, control: Control) {
        self.press(time_ns, device, control);
        self.release(time_ns.saturating_add(hold_ns), device, control);
    }

    /// Adds an event that sets `control` on `device` to `value` at
    /// `time_ns`.
    pub fn set(&self, time_ns: u64, device: u32, control: Control, value: i16) {
        self.push(TimedInput {
            time_ns,
            event: InputEvent {
                device,
                control,
                value,
            },
        });
    }

    /// Every event that `poll` gave, in order.
    pub fn delivered(&self) -> Vec<Delivered> {
        self.lock().delivered.clone()
    }

    /// Events of the script that `poll` has not given yet.
    pub fn pending(&self) -> usize {
        let state = self.lock();
        state.script.len() - state.next
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        // A poisoned lock means a test thread panicked; the script is
        // still usable.
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl InputSource for ScriptedInput {
    fn poll(&mut self, out: &mut Vec<InputEvent>) {
        let poll_ns = self.clock.now_ns();
        let mut state = self.lock();
        while let Some(input) = state.script.get(state.next).copied() {
            if input.time_ns > poll_ns {
                break;
            }
            state.next += 1;
            out.push(input.event);
            state.delivered.push(Delivered { poll_ns, input });
        }
    }
}

#[derive(Debug)]
struct State {
    /// The script in time order. Events before `next` are delivered.
    script: Vec<TimedInput>,
    next: usize,
    delivered: Vec<Delivered>,
}
