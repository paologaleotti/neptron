//! The raw state of a joystick: every button, hat and axis by index, as
//! the guided mapping reads it.

use crate::raw::{HatState, RawInput};

/// The most button indices that a map of the player or the raw state of a
/// pad holds. A button at or past it is not read.
pub const MAX_RAW_BUTTONS: u8 = 32;

/// The most hat indices that a map of the player or the raw state holds.
pub const MAX_RAW_HATS: u8 = 4;

/// The most axis indices that a map of the player or the raw state holds.
pub const MAX_RAW_AXES: u8 = 8;

/// How many button, hat and axis indices a backend reads of a joystick,
/// from index 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct RawReads {
    /// The button indices.
    pub buttons: u8,
    /// The hat indices.
    pub hats: u8,
    /// The axis indices.
    pub axes: u8,
}

/// The whole raw state of one joystick at one time. It is a fixed value,
/// so a backend fills one in each poll with no allocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct RawState {
    counts: RawReads,
    buttons: u32,
    hats: [HatState; MAX_RAW_HATS as usize],
    axes: [i16; MAX_RAW_AXES as usize],
}

impl RawReads {
    /// The counts of a joystick, cut to the limits of a raw state.
    pub fn clamped(buttons: u32, hats: u32, axes: u32) -> Self {
        let cut = |count: u32, max: u8| u8::try_from(count.min(u32::from(max))).unwrap_or(max);
        Self {
            buttons: cut(buttons, MAX_RAW_BUTTONS),
            hats: cut(hats, MAX_RAW_HATS),
            axes: cut(axes, MAX_RAW_AXES),
        }
    }
}

impl RawState {
    /// A neutral state of a joystick with `counts`, cut to the limits.
    pub fn new(counts: RawReads) -> Self {
        Self {
            counts: RawReads {
                buttons: counts.buttons.min(MAX_RAW_BUTTONS),
                hats: counts.hats.min(MAX_RAW_HATS),
                axes: counts.axes.min(MAX_RAW_AXES),
            },
            ..Self::default()
        }
    }

    /// The indices that the state holds.
    pub fn counts(&self) -> RawReads {
        self.counts
    }

    /// True when button `index` is down.
    pub fn button(&self, index: u8) -> bool {
        index < self.counts.buttons && self.buttons & (1 << index) != 0
    }

    /// The state of hat `index`; centred past the count.
    pub fn hat(&self, index: u8) -> HatState {
        self.hats
            .get(usize::from(index))
            .filter(|_| index < self.counts.hats)
            .copied()
            .unwrap_or_default()
    }

    /// The value of axis `index`; 0 past the count.
    pub fn axis(&self, index: u8) -> i16 {
        self.axes
            .get(usize::from(index))
            .filter(|_| index < self.counts.axes)
            .copied()
            .unwrap_or_default()
    }

    /// Sets what one raw input of a joystick reads. An index past the
    /// counts changes nothing, and so does an input that is not a joystick
    /// input.
    pub fn apply(&mut self, input: RawInput) {
        match input {
            RawInput::JoyButton { index, pressed } if index < self.counts.buttons => {
                let bit = 1u32 << index;
                if pressed {
                    self.buttons |= bit;
                } else {
                    self.buttons &= !bit;
                }
            }
            RawInput::JoyHat { index, hat } if index < self.counts.hats => {
                if let Some(slot) = self.hats.get_mut(usize::from(index)) {
                    *slot = hat;
                }
            }
            RawInput::JoyAxis { index, value } if index < self.counts.axes => {
                if let Some(slot) = self.axes.get_mut(usize::from(index)) {
                    *slot = value;
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_counts_are_cut_to_the_limits() {
        let counts = RawReads::clamped(200, 9, 1_000);
        assert_eq!(
            counts,
            RawReads {
                buttons: MAX_RAW_BUTTONS,
                hats: MAX_RAW_HATS,
                axes: MAX_RAW_AXES
            }
        );
        assert_eq!(RawReads::clamped(12, 1, 4).buttons, 12);
    }

    #[test]
    fn a_state_reads_what_it_was_given_inside_its_counts() {
        let mut state = RawState::new(RawReads {
            buttons: 4,
            hats: 1,
            axes: 2,
        });
        state.apply(RawInput::JoyButton {
            index: 3,
            pressed: true,
        });
        state.apply(RawInput::JoyButton {
            index: 4,
            pressed: true,
        });
        state.apply(RawInput::JoyHat {
            index: 0,
            hat: HatState(HatState::UP),
        });
        state.apply(RawInput::JoyAxis {
            index: 1,
            value: -300,
        });
        state.apply(RawInput::JoyAxis {
            index: 2,
            value: 300,
        });
        assert!(state.button(3));
        assert!(!state.button(4), "past the count");
        assert!(!state.button(40));
        assert_eq!(state.hat(0), HatState(HatState::UP));
        assert_eq!(state.hat(1), HatState::default());
        assert_eq!(state.axis(1), -300);
        assert_eq!(state.axis(2), 0);
        state.apply(RawInput::JoyButton {
            index: 3,
            pressed: false,
        });
        assert!(!state.button(3));
    }
}
