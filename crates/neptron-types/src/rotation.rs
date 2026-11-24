use serde::{Deserialize, Serialize};

/// A rotation of the picture, in steps of 90 degrees counter-clockwise,
/// as libretro defines it.
///
/// The arithmetic is modulo a full turn: [`Rotation::plus`] adds two
/// turns, [`Rotation::minus`] takes one from another, and
/// [`Rotation::inverse`] is the turn back.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Rotation {
    /// No rotation.
    R0,
    /// 90 degrees counter-clockwise.
    R90,
    /// 180 degrees.
    R180,
    /// 270 degrees counter-clockwise.
    R270,
}

impl Rotation {
    /// The four turns, in the order of their degrees.
    pub const ALL: [Rotation; 4] = [Rotation::R0, Rotation::R90, Rotation::R180, Rotation::R270];

    /// The number of quarter turns counter-clockwise, 0 to 3.
    pub const fn quarter_turns(self) -> u8 {
        match self {
            Rotation::R0 => 0,
            Rotation::R90 => 1,
            Rotation::R180 => 2,
            Rotation::R270 => 3,
        }
    }

    /// The turn of `quarters` quarter turns counter-clockwise, modulo a
    /// full turn: 4 is [`Rotation::R0`], 5 is [`Rotation::R90`].
    pub const fn from_quarter_turns(quarters: u8) -> Rotation {
        match quarters % 4 {
            0 => Rotation::R0,
            1 => Rotation::R90,
            2 => Rotation::R180,
            _ => Rotation::R270,
        }
    }

    /// The turn in degrees counter-clockwise: 0, 90, 180 or 270.
    pub const fn degrees(self) -> u16 {
        self.quarter_turns() as u16 * 90
    }

    /// The turn of `degrees` counter-clockwise, or `None` when the value
    /// is not 0, 90, 180 or 270. A full turn or more is refused, so a
    /// stored value has one spelling.
    pub const fn from_degrees(degrees: u16) -> Option<Rotation> {
        match degrees {
            0 => Some(Rotation::R0),
            90 => Some(Rotation::R90),
            180 => Some(Rotation::R180),
            270 => Some(Rotation::R270),
            _ => None,
        }
    }

    /// This turn followed by `other`.
    pub const fn plus(self, other: Rotation) -> Rotation {
        Rotation::from_quarter_turns(self.quarter_turns() + other.quarter_turns())
    }

    /// This turn less `other`, modulo a full turn: `(self - other) mod 360`.
    pub const fn minus(self, other: Rotation) -> Rotation {
        Rotation::from_quarter_turns(self.quarter_turns() + 4 - other.quarter_turns())
    }

    /// The turn that brings this one back to [`Rotation::R0`].
    pub const fn inverse(self) -> Rotation {
        Rotation::R0.minus(self)
    }

    /// The turn swaps the axes of the picture: [`Rotation::R90`] and
    /// [`Rotation::R270`]. The lines of a picture turned so are columns of
    /// its frame.
    pub const fn across(self) -> bool {
        matches!(self, Rotation::R90 | Rotation::R270)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minus_of_every_pair_is_the_difference_of_the_degrees_modulo_a_turn() {
        for a in Rotation::ALL {
            for b in Rotation::ALL {
                let want = (360 + a.degrees() - b.degrees()) % 360;
                assert_eq!(a.minus(b).degrees(), want, "{a:?} - {b:?}");
                assert_eq!(a.minus(b).plus(b), a, "{a:?} - {b:?} + {b:?}");
            }
        }
    }

    #[test]
    fn plus_of_every_pair_is_the_sum_of_the_degrees_modulo_a_turn() {
        for a in Rotation::ALL {
            for b in Rotation::ALL {
                let want = (a.degrees() + b.degrees()) % 360;
                assert_eq!(a.plus(b).degrees(), want, "{a:?} + {b:?}");
                assert_eq!(a.plus(b), b.plus(a));
            }
        }
    }

    #[test]
    fn four_quarter_turns_come_back() {
        for r in Rotation::ALL {
            let mut turned = r;
            for _ in 0..4 {
                turned = turned.plus(Rotation::R90);
            }
            assert_eq!(turned, r);
            assert_eq!(r.plus(r.inverse()), Rotation::R0);
            assert_eq!(r.inverse().inverse(), r);
        }
        assert_eq!(Rotation::R90.inverse(), Rotation::R270);
        assert_eq!(Rotation::R180.inverse(), Rotation::R180);
    }

    #[test]
    fn degrees_and_quarter_turns_round_trip() {
        for (r, degrees) in Rotation::ALL.into_iter().zip([0, 90, 180, 270]) {
            assert_eq!(r.degrees(), degrees);
            assert_eq!(Rotation::from_degrees(degrees), Some(r));
            assert_eq!(Rotation::from_quarter_turns(r.quarter_turns()), r);
            assert_eq!(Rotation::from_quarter_turns(r.quarter_turns() + 4), r);
        }
        assert_eq!(Rotation::from_quarter_turns(u8::MAX), Rotation::R270);
    }

    #[test]
    fn a_degree_value_that_is_not_a_quarter_turn_is_refused() {
        for degrees in [1, 45, 89, 91, 179, 269, 271, 360, 450, u16::MAX] {
            assert_eq!(Rotation::from_degrees(degrees), None, "{degrees}");
        }
    }

    #[test]
    fn across_is_true_for_the_turns_that_swap_the_axes() {
        assert!(!Rotation::R0.across());
        assert!(Rotation::R90.across());
        assert!(!Rotation::R180.across());
        assert!(Rotation::R270.across());
    }
}
