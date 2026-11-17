//! The map of the player: the raw input of a joystick that drives each
//! RetroPad control, as the guided mapping records it and the settings
//! folder keeps it.

use std::fmt;

use neptron_types::Control;

use crate::pad_map::stick_value;
use crate::raw::HatState;
use crate::raw_state::{MAX_RAW_AXES, MAX_RAW_BUTTONS, MAX_RAW_HATS, RawReads, RawState};
use crate::state::{ALL_CONTROLS, CONTROL_COUNT, index};

/// The travel of an axis from its centre, in axis units, at which a half
/// axis counts as a press: half travel.
pub const MAP_AXIS_THRESHOLD: i16 = 16_384;

/// The controls that a map needs to drive the menus: the four directions,
/// accept (A) and back (B).
pub const MENU_CONTROLS: [Control; 6] = [
    Control::Up,
    Control::Down,
    Control::Left,
    Control::Right,
    Control::A,
    Control::B,
];

/// Each control and its key in the file of the maps, in the order of the
/// libretro ids.
pub const CONTROL_KEYS: [(Control, &str); CONTROL_COUNT] = [
    (Control::B, "b"),
    (Control::Y, "y"),
    (Control::Select, "select"),
    (Control::Start, "start"),
    (Control::Up, "up"),
    (Control::Down, "down"),
    (Control::Left, "left"),
    (Control::Right, "right"),
    (Control::A, "a"),
    (Control::X, "x"),
    (Control::L, "l"),
    (Control::R, "r"),
    (Control::L2, "l2"),
    (Control::R2, "r2"),
    (Control::L3, "l3"),
    (Control::R3, "r3"),
    (Control::LeftX, "left_x"),
    (Control::LeftY, "left_y"),
    (Control::RightX, "right_x"),
    (Control::RightY, "right_y"),
];

/// One direction of a hat switch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HatDirection {
    /// Up.
    Up,
    /// Right.
    Right,
    /// Down.
    Down,
    /// Left.
    Left,
}

/// The raw input that drives one RetroPad control.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Binding {
    /// A button: the control is down while it is down.
    Button(u8),
    /// One direction of a hat: the control is down while the hat points
    /// there. A diagonal sets two directions.
    Hat {
        /// The hat index.
        index: u8,
        /// The direction.
        direction: HatDirection,
    },
    /// One side of an axis, as a button.
    HalfAxis {
        /// The axis index.
        index: u8,
        /// True for the positive side.
        positive: bool,
        /// False: the axis rests at the centre, and the control is down at
        /// [`MAP_AXIS_THRESHOLD`] or past it on that side. True: the axis
        /// rests at the other end (a trigger), and the control is down
        /// from the middle (0) to the end of that side.
        full: bool,
    },
    /// An axis as a RetroPad axis, through the stick dead zone.
    Stick {
        /// The axis index.
        index: u8,
        /// True when the value is negated.
        inverted: bool,
    },
}

/// The bindings of every RetroPad control of one controller model. A fixed
/// value, so the mapper keeps one per pad with no allocation. A control
/// with no binding is never down.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct PlayerMap {
    bindings: [Option<Binding>; CONTROL_COUNT],
}

/// Why a binding is not used.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum BindingError {
    /// The text is no binding.
    #[error("\"{text}\" is not a binding (b3, h0up, a2+, a4+full, a0 or -a0)")]
    BadText {
        /// The text.
        text: String,
    },
    /// The index is past the limit of a raw state.
    #[error("{what} {index} is past the last one that is read ({max})")]
    IndexTooHigh {
        /// `button`, `hat` or `axis`.
        what: &'static str,
        /// The index.
        index: u8,
        /// The highest index that is read.
        max: u8,
    },
    /// A stick binding for a button control, or a button binding for an
    /// axis control.
    #[error("{binding} cannot drive {control:?}")]
    WrongKind {
        /// The control.
        control: Control,
        /// The binding, as text.
        binding: String,
    },
}

impl HatDirection {
    /// The bit of the direction in a [`HatState`].
    pub fn bit(self) -> u8 {
        match self {
            Self::Up => HatState::UP,
            Self::Right => HatState::RIGHT,
            Self::Down => HatState::DOWN,
            Self::Left => HatState::LEFT,
        }
    }

    /// The four directions, in the order of their bits.
    pub const ALL: [Self; 4] = [Self::Up, Self::Right, Self::Down, Self::Left];

    fn word(self) -> &'static str {
        match self {
            Self::Up => "up",
            Self::Right => "right",
            Self::Down => "down",
            Self::Left => "left",
        }
    }
}

impl Binding {
    /// Reads the text of a binding: `b3`, `h0up`, `h0right`, `h0down`,
    /// `h0left`, `a2+`, `a2-`, `a4+full`, `a4-full`, `a0` or `-a0`.
    ///
    /// # Errors
    ///
    /// [`BindingError::BadText`] for any other text, and
    /// [`BindingError::IndexTooHigh`] for an index past the limits of a
    /// raw state.
    pub fn parse(text: &str) -> Result<Self, BindingError> {
        let bad = || BindingError::BadText {
            text: text.to_owned(),
        };
        let binding = if let Some(rest) = text.strip_prefix('b') {
            Self::Button(number(rest).ok_or_else(bad)?)
        } else if let Some(rest) = text.strip_prefix('h') {
            let split = rest.find(|c: char| !c.is_ascii_digit()).ok_or_else(bad)?;
            let (digits, word) = rest.split_at(split);
            let direction = HatDirection::ALL
                .into_iter()
                .find(|d| d.word() == word)
                .ok_or_else(bad)?;
            Self::Hat {
                index: number(digits).ok_or_else(bad)?,
                direction,
            }
        } else if let Some(rest) = text.strip_prefix("-a") {
            Self::Stick {
                index: number(rest).ok_or_else(bad)?,
                inverted: true,
            }
        } else if let Some(rest) = text.strip_prefix('a') {
            let split = rest.find(|c: char| !c.is_ascii_digit());
            match split.map(|at| rest.split_at(at)) {
                None => Self::Stick {
                    index: number(rest).ok_or_else(bad)?,
                    inverted: false,
                },
                Some((digits, side)) => {
                    let (positive, full) = match side {
                        "+" => (true, false),
                        "-" => (false, false),
                        "+full" => (true, true),
                        "-full" => (false, true),
                        _ => return Err(bad()),
                    };
                    Self::HalfAxis {
                        index: number(digits).ok_or_else(bad)?,
                        positive,
                        full,
                    }
                }
            }
        } else {
            return Err(bad());
        };
        binding.check_index()?;
        Ok(binding)
    }

    /// True when the binding can drive `control`: a stick binding drives
    /// an axis control, every other binding a button control.
    pub fn fits(&self, control: Control) -> bool {
        matches!(self, Self::Stick { .. }) == is_axis(control)
    }

    /// The value of the control that the binding drives, in the raw state
    /// `state`: 0 or 1 for a button control, the stick value for an axis.
    pub fn value(&self, state: &RawState) -> i16 {
        match *self {
            Self::Button(index) => i16::from(state.button(index)),
            Self::Hat { index, direction } => i16::from(state.hat(index).has(direction.bit())),
            Self::HalfAxis {
                index,
                positive,
                full,
            } => half_axis_value(state.axis(index), positive, full),
            Self::Stick { index, inverted } => stick(state.axis(index), inverted),
        }
    }

    fn check_index(&self) -> Result<(), BindingError> {
        let (what, index, max) = match *self {
            Self::Button(index) => ("button", index, MAX_RAW_BUTTONS),
            Self::Hat { index, .. } => ("hat", index, MAX_RAW_HATS),
            Self::HalfAxis { index, .. } | Self::Stick { index, .. } => {
                ("axis", index, MAX_RAW_AXES)
            }
        };
        if index < max {
            Ok(())
        } else {
            Err(BindingError::IndexTooHigh {
                what,
                index,
                max: max - 1,
            })
        }
    }
}

impl fmt::Display for Binding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::Button(index) => write!(f, "b{index}"),
            Self::Hat { index, direction } => write!(f, "h{index}{}", direction.word()),
            Self::HalfAxis {
                index,
                positive,
                full,
            } => {
                let side = if positive { '+' } else { '-' };
                let end = if full { "full" } else { "" };
                write!(f, "a{index}{side}{end}")
            }
            Self::Stick { index, inverted } => {
                let sign = if inverted { "-" } else { "" };
                write!(f, "{sign}a{index}")
            }
        }
    }
}

impl PlayerMap {
    /// A map with no binding.
    pub fn new() -> Self {
        Self::default()
    }

    /// The binding of `control`.
    pub fn get(&self, control: Control) -> Option<Binding> {
        self.bindings.get(index(control)).copied().flatten()
    }

    /// Sets the binding of `control`, or removes it with `None`.
    ///
    /// # Errors
    ///
    /// [`BindingError::WrongKind`] when the binding cannot drive the
    /// control, and [`BindingError::IndexTooHigh`] for an index past the
    /// limits. The map does not change.
    pub fn set(&mut self, control: Control, binding: Option<Binding>) -> Result<(), BindingError> {
        if let Some(binding) = binding {
            binding.check_index()?;
            if !binding.fits(control) {
                return Err(BindingError::WrongKind {
                    control,
                    binding: binding.to_string(),
                });
            }
        }
        if let Some(slot) = self.bindings.get_mut(index(control)) {
            *slot = binding;
        }
        Ok(())
    }

    /// Each control with a binding, in the order of the libretro ids.
    pub fn bindings(&self) -> impl Iterator<Item = (Control, Binding)> + '_ {
        ALL_CONTROLS
            .iter()
            .zip(&self.bindings)
            .filter_map(|(control, binding)| binding.map(|b| (*control, b)))
    }

    /// True when no control has a binding.
    pub fn is_empty(&self) -> bool {
        self.bindings.iter().all(Option::is_none)
    }

    /// The indices that the backend reads: one past the highest of each
    /// kind in use.
    pub fn reads(&self) -> RawReads {
        let mut reads = RawReads::default();
        for (_, binding) in self.bindings() {
            let (slot, index) = match binding {
                Binding::Button(index) => (&mut reads.buttons, index),
                Binding::Hat { index, .. } => (&mut reads.hats, index),
                Binding::HalfAxis { index, .. } | Binding::Stick { index, .. } => {
                    (&mut reads.axes, index)
                }
            };
            *slot = (*slot).max(index.saturating_add(1));
        }
        reads
    }

    /// The controls of [`MENU_CONTROLS`] that have no binding: with one of
    /// them the map cannot drive the menus.
    pub fn menu_gaps(&self) -> impl Iterator<Item = Control> + '_ {
        MENU_CONTROLS
            .into_iter()
            .filter(|control| self.get(*control).is_none())
    }

    /// True when Select and Start both have a binding, so the quick menu
    /// combination and the blind recovery work on the pad.
    pub fn has_menu_combo(&self) -> bool {
        self.get(Control::Select).is_some() && self.get(Control::Start).is_some()
    }
}

/// The key of `control` in the file of the maps.
pub fn control_key(control: Control) -> &'static str {
    CONTROL_KEYS
        .iter()
        .find(|(c, _)| *c == control)
        .map_or("", |(_, key)| key)
}

/// The control of a key of the file of the maps.
pub fn control_of_key(key: &str) -> Option<Control> {
    CONTROL_KEYS
        .iter()
        .find(|(_, k)| *k == key)
        .map(|(control, _)| *control)
}

/// True for the four RetroPad axes.
pub fn is_axis(control: Control) -> bool {
    matches!(
        control,
        Control::LeftX | Control::LeftY | Control::RightX | Control::RightY
    )
}

/// The button value (0 or 1) of one side of an axis.
pub(crate) fn half_axis_value(raw: i16, positive: bool, full: bool) -> i16 {
    let down = match (positive, full) {
        (true, false) => raw >= MAP_AXIS_THRESHOLD,
        (false, false) => raw <= -MAP_AXIS_THRESHOLD,
        (true, true) => raw >= 0,
        (false, true) => raw <= 0,
    };
    i16::from(down)
}

/// The RetroPad axis value of a raw axis value, negated when `inverted`.
pub(crate) fn stick(raw: i16, inverted: bool) -> i16 {
    let value = stick_value(raw);
    if inverted {
        value.saturating_neg()
    } else {
        value
    }
}

/// A decimal index with no sign and no leading `+`.
fn number(text: &str) -> Option<u8> {
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_binding_text_reads_back_as_it_was_written() {
        let texts = [
            "b0", "b31", "h0up", "h1right", "h3down", "h0left", "a2+", "a2-", "a4+full", "a4-full",
            "a0", "-a7",
        ];
        for text in texts {
            let binding = Binding::parse(text).expect(text);
            assert_eq!(binding.to_string(), text);
        }
        assert_eq!(
            Binding::parse("a4+full"),
            Ok(Binding::HalfAxis {
                index: 4,
                positive: true,
                full: true
            })
        );
        assert_eq!(
            Binding::parse("-a1"),
            Ok(Binding::Stick {
                index: 1,
                inverted: true
            })
        );
    }

    #[test]
    fn a_damaged_binding_text_is_refused() {
        for text in [
            "", "b", "bx", "b+1", "b-1", "h0", "hup", "h0north", "a", "a2*", "a2+half", "-a",
            "--a1", "x3", "b 3", "b300",
        ] {
            assert!(
                matches!(Binding::parse(text), Err(BindingError::BadText { .. })),
                "{text:?}"
            );
        }
        assert_eq!(
            Binding::parse("b32"),
            Err(BindingError::IndexTooHigh {
                what: "button",
                index: 32,
                max: 31
            })
        );
        assert!(matches!(
            Binding::parse("h4up"),
            Err(BindingError::IndexTooHigh { what: "hat", .. })
        ));
        assert!(matches!(
            Binding::parse("a8+"),
            Err(BindingError::IndexTooHigh { what: "axis", .. })
        ));
    }

    #[test]
    fn a_stick_drives_only_an_axis_and_a_button_only_a_button() {
        let mut map = PlayerMap::new();
        let stick = Binding::Stick {
            index: 0,
            inverted: false,
        };
        assert!(matches!(
            map.set(Control::A, Some(stick)),
            Err(BindingError::WrongKind { .. })
        ));
        assert!(matches!(
            map.set(Control::LeftX, Some(Binding::Button(1))),
            Err(BindingError::WrongKind { .. })
        ));
        assert!(map.is_empty());
        map.set(Control::LeftX, Some(stick)).unwrap();
        map.set(Control::A, Some(Binding::Button(2))).unwrap();
        assert_eq!(map.get(Control::A), Some(Binding::Button(2)));
        map.set(Control::A, None).unwrap();
        assert_eq!(map.get(Control::A), None);
        assert!(map.set(Control::B, Some(Binding::Button(40))).is_err());
    }

    #[test]
    fn the_reads_are_one_past_the_highest_index_of_each_kind() {
        let mut map = PlayerMap::new();
        assert_eq!(map.reads(), RawReads::default());
        map.set(Control::A, Some(Binding::Button(9))).unwrap();
        map.set(Control::B, Some(Binding::Button(2))).unwrap();
        let up = Binding::Hat {
            index: 1,
            direction: HatDirection::Up,
        };
        map.set(Control::Up, Some(up)).unwrap();
        let l2 = Binding::HalfAxis {
            index: 5,
            positive: true,
            full: true,
        };
        map.set(Control::L2, Some(l2)).unwrap();
        let stick = Binding::Stick {
            index: 1,
            inverted: true,
        };
        map.set(Control::LeftY, Some(stick)).unwrap();
        assert_eq!(
            map.reads(),
            RawReads {
                buttons: 10,
                hats: 2,
                axes: 6
            }
        );
    }

    #[test]
    fn the_menu_needs_the_directions_a_and_b_and_the_combo_needs_select_and_start() {
        let mut map = PlayerMap::new();
        assert_eq!(map.menu_gaps().collect::<Vec<_>>(), MENU_CONTROLS);
        for (control, i) in MENU_CONTROLS.into_iter().zip(0..) {
            map.set(control, Some(Binding::Button(i))).unwrap();
        }
        assert_eq!(map.menu_gaps().count(), 0);
        assert!(!map.has_menu_combo());
        map.set(Control::Start, Some(Binding::Button(9))).unwrap();
        assert!(!map.has_menu_combo());
        map.set(Control::Select, Some(Binding::Button(8))).unwrap();
        assert!(map.has_menu_combo());
    }

    #[test]
    fn each_binding_gives_its_value_from_a_raw_state() {
        let mut state = RawState::new(RawReads {
            buttons: 4,
            hats: 1,
            axes: 3,
        });
        state.apply(crate::RawInput::JoyButton {
            index: 2,
            pressed: true,
        });
        state.apply(crate::RawInput::JoyHat {
            index: 0,
            hat: HatState(HatState::UP | HatState::LEFT),
        });
        state.apply(crate::RawInput::JoyAxis {
            index: 0,
            value: -32_768,
        });
        state.apply(crate::RawInput::JoyAxis { index: 2, value: 1 });
        assert_eq!(Binding::Button(2).value(&state), 1);
        assert_eq!(Binding::Button(1).value(&state), 0);
        let hat = |direction| Binding::Hat {
            index: 0,
            direction,
        };
        assert_eq!(hat(HatDirection::Up).value(&state), 1);
        assert_eq!(hat(HatDirection::Left).value(&state), 1);
        assert_eq!(hat(HatDirection::Down).value(&state), 0);
        let half = |index, positive, full| Binding::HalfAxis {
            index,
            positive,
            full,
        };
        assert_eq!(half(0, false, false).value(&state), 1);
        assert_eq!(half(0, true, false).value(&state), 0);
        assert_eq!(half(2, true, true).value(&state), 1, "past the middle");
        assert_eq!(half(2, true, false).value(&state), 0);
        let stick = |inverted| Binding::Stick { index: 0, inverted };
        assert_eq!(stick(false).value(&state), -32_767);
        assert_eq!(stick(true).value(&state), 32_767);
    }

    #[test]
    fn the_half_axis_boundaries_are_inclusive() {
        assert_eq!(half_axis_value(MAP_AXIS_THRESHOLD, true, false), 1);
        assert_eq!(half_axis_value(MAP_AXIS_THRESHOLD - 1, true, false), 0);
        assert_eq!(half_axis_value(-MAP_AXIS_THRESHOLD, false, false), 1);
        assert_eq!(half_axis_value(-MAP_AXIS_THRESHOLD + 1, false, false), 0);
        assert_eq!(half_axis_value(-32_768, true, true), 0, "a trigger at rest");
        assert_eq!(half_axis_value(0, true, true), 1);
        assert_eq!(half_axis_value(32_767, false, true), 0);
        assert_eq!(half_axis_value(0, false, true), 1);
    }

    #[test]
    fn every_control_has_one_key_and_reads_back() {
        for control in ALL_CONTROLS {
            let key = control_key(control);
            assert!(!key.is_empty(), "{control:?}");
            assert_eq!(control_of_key(key), Some(control));
        }
        assert_eq!(control_of_key("turbo"), None);
    }
}
