//! The guided mapping: the player presses each RetroPad control in turn on
//! one pad, and the result is a map of the player.
//!
//! The rules are pure. The input is the whole raw state of the pad in each
//! frame and the time of the frame; the output is the prompt and, at the
//! end, the map.

use neptron_types::Control;

use crate::player_map::{Binding, HatDirection, MAP_AXIS_THRESHOLD, PlayerMap, is_axis};
use crate::raw_state::{MAX_RAW_AXES, MAX_RAW_HATS, RawState};

/// How long an input of the pad must stay down to skip a step, in
/// nanoseconds. A shorter press records the input.
pub const MAP_SKIP_HOLD_NS: u64 = 2_000_000_000;

/// How long the pad may give no press and no release before the mapping
/// ends with no change, in nanoseconds.
pub const MAP_IDLE_TIMEOUT_NS: u64 = 15_000_000_000;

/// The controls of the guided mapping, in order: the D-pad first, so the
/// player can drive the menus with it soon, then accept and back, Start
/// and Select, the other buttons, and the sticks. A stick step asks for a
/// move right (X) or down (Y).
pub const GUIDED_STEPS: [Control; 20] = [
    Control::Up,
    Control::Down,
    Control::Left,
    Control::Right,
    Control::A,
    Control::B,
    Control::Start,
    Control::Select,
    Control::Y,
    Control::X,
    Control::L,
    Control::R,
    Control::L2,
    Control::R2,
    Control::L3,
    Control::R3,
    Control::LeftX,
    Control::LeftY,
    Control::RightX,
    Control::RightY,
];

/// A guided mapping of one pad.
#[derive(Debug, Clone)]
pub struct GuidedMapping {
    step: usize,
    map: PlayerMap,
    /// The rest of each axis, from the first raw state.
    rests: Option<[Rest; MAX_RAW_AXES as usize]>,
    phase: Phase,
    /// The presses of the last frame, to tell a press or a release.
    last: Option<Presses>,
    /// The time of the last press or release, or of the start.
    last_change_ns: Option<u64>,
    note: Option<GuidedNote>,
}

/// What the guided mapping shows now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Guided {
    /// The prompt of a step.
    Prompt(Prompt),
    /// Every step is done: the map, with no binding for a skipped step.
    Done(PlayerMap),
    /// The pad gave no press and no release for [`MAP_IDLE_TIMEOUT_NS`].
    TimedOut,
}

/// The prompt of one step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Prompt {
    /// The step, from 0, in [`GUIDED_STEPS`].
    pub step: usize,
    /// The control to press. An axis control asks for a move of a stick
    /// right (X) or down (Y).
    pub control: Control,
    /// What happened at the last input, if it needs saying.
    pub note: Option<GuidedNote>,
    /// The time left before the timeout, in nanoseconds.
    pub remaining_ns: u64,
    /// True while an input is held: past [`MAP_SKIP_HOLD_NS`] the step is
    /// skipped.
    pub holding: bool,
}

/// What the screen says about the last input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuidedNote {
    /// The input drives this other control already, so it was refused.
    Taken(Control),
    /// A button in a stick step: only a stick move counts there.
    NeedsStick,
    /// The step before was skipped by a hold.
    Skipped(Control),
}

/// The rest of an axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Rest {
    #[default]
    Centre,
    Low,
    High,
}

/// One input of a pad that can be active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Press {
    Button(u8),
    Hat(u8, HatDirection),
    Axis { index: u8, positive: bool },
}

/// The active inputs of one raw state, as bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct Presses {
    buttons: u32,
    hats: [u8; MAX_RAW_HATS as usize],
    /// Two bits per axis: the negative and the positive side.
    axes: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// The pad must be neutral before the step takes an input.
    Neutral,
    Ready,
    Held {
        press: Press,
        since_ns: u64,
    },
    Done,
    TimedOut,
}

impl Default for GuidedMapping {
    fn default() -> Self {
        Self::new()
    }
}

impl GuidedMapping {
    /// A mapping at its first step. The first raw state that it reads
    /// gives the rest of each axis, and the pad must be neutral before the
    /// first step takes an input.
    pub fn new() -> Self {
        Self {
            step: 0,
            map: PlayerMap::new(),
            rests: None,
            phase: Phase::Neutral,
            last: None,
            last_change_ns: None,
            note: None,
        }
    }

    /// Reads the raw state of the pad at `now_ns`, and gives what to show.
    pub fn update(&mut self, state: &RawState, now_ns: u64) -> Guided {
        let rests = *self.rests.get_or_insert_with(|| rests_of(state));
        let presses = presses(state, &rests);
        if self.last != Some(presses) {
            self.last = Some(presses);
            self.last_change_ns = Some(now_ns);
        }
        let since = now_ns.saturating_sub(self.last_change_ns.unwrap_or(now_ns));
        if matches!(
            self.phase,
            Phase::Neutral | Phase::Ready | Phase::Held { .. }
        ) && since >= MAP_IDLE_TIMEOUT_NS
        {
            self.phase = Phase::TimedOut;
        }
        match self.phase {
            Phase::Neutral => {
                if presses == Presses::default() {
                    self.phase = Phase::Ready;
                }
            }
            Phase::Ready => {
                if let Some(press) = first_press(&presses, state) {
                    self.phase = Phase::Held {
                        press,
                        since_ns: now_ns,
                    };
                }
            }
            Phase::Held { press, since_ns } => {
                if presses.has(press) {
                    if now_ns.saturating_sub(since_ns) >= MAP_SKIP_HOLD_NS {
                        self.skip();
                    }
                } else {
                    self.take(press, &rests);
                }
            }
            Phase::Done | Phase::TimedOut => {}
        }
        // A step that ends on a release can be neutral in the same frame.
        if self.phase == Phase::Neutral && presses == Presses::default() {
            self.phase = Phase::Ready;
        }
        self.view(since)
    }

    /// The map so far.
    pub fn map(&self) -> &PlayerMap {
        &self.map
    }

    fn view(&self, since: u64) -> Guided {
        match (self.phase, GUIDED_STEPS.get(self.step)) {
            (Phase::TimedOut, _) => Guided::TimedOut,
            (Phase::Done, _) | (_, None) => Guided::Done(self.map),
            (phase, Some(control)) => Guided::Prompt(Prompt {
                step: self.step,
                control: *control,
                note: self.note,
                remaining_ns: MAP_IDLE_TIMEOUT_NS.saturating_sub(since),
                holding: matches!(phase, Phase::Held { .. }),
            }),
        }
    }

    fn skip(&mut self) {
        if let Some(control) = GUIDED_STEPS.get(self.step) {
            self.note = Some(GuidedNote::Skipped(*control));
        }
        self.next();
    }

    /// Records the press that was released before the skip hold, or
    /// refuses it with a note.
    fn take(&mut self, press: Press, rests: &[Rest; MAX_RAW_AXES as usize]) {
        self.phase = Phase::Neutral;
        let Some(control) = GUIDED_STEPS.get(self.step).copied() else {
            return;
        };
        let Some(binding) = binding_for(control, press, rests) else {
            self.note = Some(GuidedNote::NeedsStick);
            return;
        };
        if let Some(other) = self.taken_by(control, binding) {
            self.note = Some(GuidedNote::Taken(other));
            return;
        }
        // `binding_for` gives a binding that fits the control, with an
        // index of a raw state, so `set` cannot refuse it.
        if self.map.set(control, Some(binding)).is_ok() {
            self.note = None;
            self.next();
        }
    }

    fn next(&mut self) {
        self.step += 1;
        self.phase = if self.step >= GUIDED_STEPS.len() {
            Phase::Done
        } else {
            Phase::Neutral
        };
    }

    /// The control of the same kind that `binding` drives already. A
    /// stick conflicts with a stick on the same axis, either way round.
    fn taken_by(&self, control: Control, binding: Binding) -> Option<Control> {
        self.map
            .bindings()
            .find(|(other, existing)| {
                *other != control
                    && match (existing, binding) {
                        (Binding::Stick { index: a, .. }, Binding::Stick { index: b, .. }) => {
                            *a == b
                        }
                        _ => *existing == binding,
                    }
            })
            .map(|(other, _)| other)
    }
}

impl Presses {
    fn has(&self, press: Press) -> bool {
        match press {
            Press::Button(index) => self.buttons & (1 << index) != 0,
            Press::Hat(index, direction) => self
                .hats
                .get(usize::from(index))
                .is_some_and(|bits| bits & direction.bit() != 0),
            Press::Axis { index, positive } => self.axes & axis_bit(index, positive) != 0,
        }
    }
}

fn axis_bit(index: u8, positive: bool) -> u16 {
    1 << (2 * u16::from(index) + u16::from(positive))
}

fn rests_of(state: &RawState) -> [Rest; MAX_RAW_AXES as usize] {
    let mut rests = [Rest::Centre; MAX_RAW_AXES as usize];
    for (index, rest) in (0..).zip(&mut rests) {
        let value = state.axis(index);
        *rest = if value <= -MAP_AXIS_THRESHOLD {
            Rest::Low
        } else if value >= MAP_AXIS_THRESHOLD {
            Rest::High
        } else {
            Rest::Centre
        };
    }
    rests
}

fn presses(state: &RawState, rests: &[Rest; MAX_RAW_AXES as usize]) -> Presses {
    let counts = state.counts();
    let mut presses = Presses::default();
    for index in 0..counts.buttons {
        if state.button(index) {
            presses.buttons |= 1 << index;
        }
    }
    for (index, slot) in (0..counts.hats).zip(&mut presses.hats) {
        *slot = state.hat(index).0;
    }
    for (index, rest) in (0..counts.axes).zip(rests) {
        let value = state.axis(index);
        let (negative, positive) = match rest {
            Rest::Centre => (value <= -MAP_AXIS_THRESHOLD, value >= MAP_AXIS_THRESHOLD),
            Rest::Low => (false, value >= 0),
            Rest::High => (value <= 0, false),
        };
        if negative {
            presses.axes |= axis_bit(index, false);
        }
        if positive {
            presses.axes |= axis_bit(index, true);
        }
    }
    presses
}

/// The first active input, in this order: buttons, hats, axes, each by
/// index.
fn first_press(presses: &Presses, state: &RawState) -> Option<Press> {
    let counts = state.counts();
    let buttons = (0..counts.buttons).map(Press::Button);
    let hats = (0..counts.hats)
        .flat_map(|index| HatDirection::ALL.map(|direction| Press::Hat(index, direction)));
    let axes = (0..counts.axes)
        .flat_map(|index| [false, true].map(|positive| Press::Axis { index, positive }));
    buttons
        .chain(hats)
        .chain(axes)
        .find(|press| presses.has(*press))
}

/// The binding that `press` gives `control`, or `None` when it cannot
/// drive it: a stick step takes only an axis that rests at the centre.
fn binding_for(
    control: Control,
    press: Press,
    rests: &[Rest; MAX_RAW_AXES as usize],
) -> Option<Binding> {
    let rest = |index: u8| rests.get(usize::from(index)).copied().unwrap_or_default();
    if is_axis(control) {
        return match press {
            Press::Axis { index, positive } if rest(index) == Rest::Centre => {
                Some(Binding::Stick {
                    index,
                    inverted: !positive,
                })
            }
            _ => None,
        };
    }
    Some(match press {
        Press::Button(index) => Binding::Button(index),
        Press::Hat(index, direction) => Binding::Hat { index, direction },
        Press::Axis { index, positive } => Binding::HalfAxis {
            index,
            positive,
            full: rest(index) != Rest::Centre,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::raw::{HatState, RawInput};
    use crate::raw_state::RawReads;

    const MS: u64 = 1_000_000;

    /// A pad and a guided mapping, with the time of the frame.
    struct Rig {
        state: RawState,
        guided: GuidedMapping,
        now: u64,
    }

    impl Rig {
        fn new(axes: &[(u8, i16)]) -> Self {
            let mut state = RawState::new(RawReads {
                buttons: 12,
                hats: 1,
                axes: 6,
            });
            for (index, value) in axes {
                state.apply(RawInput::JoyAxis {
                    index: *index,
                    value: *value,
                });
            }
            let mut rig = Self {
                state,
                guided: GuidedMapping::new(),
                now: 1_000 * MS,
            };
            rig.frame(0);
            rig
        }

        fn frame(&mut self, after_ms: u64) -> Guided {
            self.now += after_ms * MS;
            self.guided.update(&self.state, self.now)
        }

        fn set(&mut self, input: RawInput) -> Guided {
            self.state.apply(input);
            self.frame(16)
        }

        fn button(&mut self, index: u8) -> Guided {
            self.set(RawInput::JoyButton {
                index,
                pressed: true,
            });
            self.set(RawInput::JoyButton {
                index,
                pressed: false,
            })
        }

        fn hat(&mut self, bits: u8) -> Guided {
            self.set(RawInput::JoyHat {
                index: 0,
                hat: HatState(bits),
            });
            self.set(RawInput::JoyHat {
                index: 0,
                hat: HatState::default(),
            })
        }

        fn axis(&mut self, index: u8, value: i16, rest: i16) -> Guided {
            self.set(RawInput::JoyAxis { index, value });
            self.set(RawInput::JoyAxis { index, value: rest })
        }

        fn hold(&mut self, index: u8, ms: u64) -> Guided {
            self.set(RawInput::JoyButton {
                index,
                pressed: true,
            });
            let shown = self.frame(ms);
            self.set(RawInput::JoyButton {
                index,
                pressed: false,
            });
            shown
        }

        fn prompt(&mut self) -> Prompt {
            match self.frame(0) {
                Guided::Prompt(prompt) => prompt,
                other => panic!("no prompt: {other:?}"),
            }
        }
    }

    fn up(index: u8) -> Binding {
        Binding::Hat {
            index,
            direction: HatDirection::Up,
        }
    }

    #[test]
    fn the_steps_start_with_the_d_pad_then_accept_back_start_and_select() {
        assert_eq!(
            GUIDED_STEPS[..8],
            [
                Control::Up,
                Control::Down,
                Control::Left,
                Control::Right,
                Control::A,
                Control::B,
                Control::Start,
                Control::Select
            ]
        );
        let mut seen = GUIDED_STEPS.to_vec();
        seen.sort_by_key(|c| crate::state::index(*c));
        seen.dedup();
        assert_eq!(seen.len(), 20, "every control once");
    }

    #[test]
    fn a_full_mapping_records_hats_buttons_axes_and_sticks() {
        let mut rig = Rig::new(&[(4, -32_768)]);
        let prompt = rig.prompt();
        assert_eq!((prompt.step, prompt.control), (0, Control::Up));
        for bits in [
            HatState::UP,
            HatState::DOWN,
            HatState::LEFT,
            HatState::RIGHT,
        ] {
            rig.hat(bits);
        }
        for index in [2, 1, 9, 8, 0, 3, 4, 5] {
            rig.button(index);
        }
        assert_eq!(rig.prompt().control, Control::L2);
        rig.axis(4, 32_767, -32_768);
        rig.axis(5, -20_000, 0);
        rig.button(10);
        rig.button(11);
        assert_eq!(rig.prompt().control, Control::LeftX);
        rig.axis(0, 32_767, 0);
        rig.axis(1, -32_768, 0);
        rig.axis(2, 30_000, 0);
        let done = rig.axis(3, 30_000, 0);
        let Guided::Done(map) = done else {
            panic!("{done:?}");
        };
        assert_eq!(map.get(Control::Up), Some(up(0)));
        assert_eq!(
            map.get(Control::Right),
            Some(Binding::Hat {
                index: 0,
                direction: HatDirection::Right
            })
        );
        assert_eq!(map.get(Control::A), Some(Binding::Button(2)));
        assert_eq!(map.get(Control::Select), Some(Binding::Button(8)));
        assert_eq!(
            map.get(Control::L2),
            Some(Binding::HalfAxis {
                index: 4,
                positive: true,
                full: true
            }),
            "a trigger at full travel counts from its rest"
        );
        assert_eq!(
            map.get(Control::R2),
            Some(Binding::HalfAxis {
                index: 5,
                positive: false,
                full: false
            })
        );
        assert_eq!(
            map.get(Control::LeftX),
            Some(Binding::Stick {
                index: 0,
                inverted: false
            })
        );
        assert_eq!(
            map.get(Control::LeftY),
            Some(Binding::Stick {
                index: 1,
                inverted: true
            }),
            "a move up for a prompt of down is an inverted axis"
        );
        assert_eq!(map.menu_gaps().count(), 0);
        assert!(map.has_menu_combo());
    }

    #[test]
    fn the_pad_must_be_neutral_before_a_step_takes_an_input() {
        let mut rig = Rig::new(&[]);
        rig.guided = GuidedMapping::new();
        rig.set(RawInput::JoyButton {
            index: 2,
            pressed: true,
        });
        rig.set(RawInput::JoyButton {
            index: 2,
            pressed: false,
        });
        assert_eq!(
            rig.prompt().step,
            0,
            "the release of a press from before the start counts for nothing"
        );
        rig.set(RawInput::JoyHat {
            index: 0,
            hat: HatState(HatState::UP),
        });
        rig.set(RawInput::JoyButton {
            index: 3,
            pressed: true,
        });
        rig.set(RawInput::JoyHat {
            index: 0,
            hat: HatState::default(),
        });
        assert_eq!(rig.prompt().step, 1, "up is taken");
        rig.set(RawInput::JoyButton {
            index: 3,
            pressed: false,
        });
        assert_eq!(rig.prompt().step, 1, "a held button does not fill Down");
        rig.hat(HatState::DOWN);
        assert_eq!(rig.prompt().step, 2);
    }

    #[test]
    fn a_hold_skips_a_step_and_a_shorter_press_records_it() {
        let mut rig = Rig::new(&[]);
        let shown = rig.hold(0, 1_900);
        assert!(matches!(shown, Guided::Prompt(p) if p.holding && p.step == 0));
        assert_eq!(rig.guided.map().get(Control::Up), Some(Binding::Button(0)));
        let shown = rig.hold(1, 2_000);
        assert!(
            matches!(shown, Guided::Prompt(p) if p.step == 2 && p.note == Some(GuidedNote::Skipped(Control::Down))),
            "{shown:?}"
        );
        assert_eq!(rig.guided.map().get(Control::Down), None);
        assert_eq!(
            rig.prompt().step,
            2,
            "the release of the hold counts for nothing"
        );
    }

    #[test]
    fn an_input_used_twice_is_refused_and_the_step_stays() {
        let mut rig = Rig::new(&[]);
        rig.hat(HatState::UP);
        rig.hat(HatState::UP);
        let prompt = rig.prompt();
        assert_eq!(prompt.step, 1);
        assert_eq!(prompt.note, Some(GuidedNote::Taken(Control::Up)));
        rig.hat(HatState::DOWN);
        let prompt = rig.prompt();
        assert_eq!((prompt.step, prompt.note), (2, None));
    }

    #[test]
    fn a_stick_step_takes_only_a_centred_axis_and_shares_it_with_the_d_pad() {
        let mut rig = Rig::new(&[(4, -32_768)]);
        rig.axis(1, -32_768, 0);
        rig.axis(1, 32_767, 0);
        rig.axis(0, -32_768, 0);
        rig.axis(0, 32_767, 0);
        for _ in 0..12 {
            rig.hold(0, 2_000);
        }
        assert_eq!(rig.prompt().control, Control::LeftX);
        rig.button(5);
        assert_eq!(rig.prompt().note, Some(GuidedNote::NeedsStick));
        rig.axis(4, 32_767, -32_768);
        let prompt = rig.prompt();
        assert_eq!(
            (prompt.control, prompt.note),
            (Control::LeftX, Some(GuidedNote::NeedsStick)),
            "a trigger is no stick"
        );
        rig.axis(0, 32_767, 0);
        assert_eq!(
            rig.prompt().control,
            Control::LeftY,
            "the D-pad axis is the stick too"
        );
        rig.axis(0, 32_767, 0);
        assert_eq!(
            rig.prompt().note,
            Some(GuidedNote::Taken(Control::LeftX)),
            "one axis is one stick axis"
        );
    }

    #[test]
    fn the_mapping_times_out_with_no_press_and_counts_down() {
        let mut rig = Rig::new(&[]);
        assert_eq!(rig.prompt().remaining_ns, MAP_IDLE_TIMEOUT_NS);
        assert_eq!(
            rig.frame(5_000),
            Guided::Prompt(Prompt {
                step: 0,
                control: Control::Up,
                note: None,
                remaining_ns: MAP_IDLE_TIMEOUT_NS - 5_000 * MS,
                holding: false,
            })
        );
        rig.button(4);
        assert_eq!(
            rig.prompt().remaining_ns,
            MAP_IDLE_TIMEOUT_NS,
            "a release starts it again"
        );
        rig.state.apply(RawInput::JoyAxis {
            index: 0,
            value: 3_000,
        });
        rig.frame(14_999);
        assert!(
            matches!(rig.frame(0), Guided::Prompt(_)),
            "noise is no input"
        );
        assert_eq!(rig.frame(1), Guided::TimedOut);
        rig.button(5);
        assert_eq!(rig.frame(0), Guided::TimedOut, "it stays over");
    }
}
