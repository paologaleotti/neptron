use std::fmt;

use super::PictureStyle;

/// A rung of the pixel-perfect ladder, or the panel case.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Rung {
    /// Rung 1: the game's own line count, progressive, or interlaced for an
    /// interlaced source. A source with fewer lines than the profile allows
    /// is centred in a 224 or 240-line mode with borders.
    Native,
    /// Rung 2: a whole multiple (2, 3 or 4) of the game's line count,
    /// progressive.
    Multiple(u8),
    /// Rung 3: the game's own line count, interlaced, for a source with more
    /// lines than any progressive range allows.
    Interlaced,
    /// Rung 4: the mode that Switchres fits to the source, scaled with a
    /// filter. The only rung that is not pixel exact.
    Fitted,
    /// A flat panel: one of the panel's own modes, the game scaled by a
    /// whole factor on the vertical axis.
    Panel,
}

impl Rung {
    /// The rung is not pixel exact, so reaching it shows a notice.
    pub fn is_fitted(self) -> bool {
        self == Self::Fitted
    }
}

const AUTO: &[Rung] = &[
    Rung::Native,
    Rung::Multiple(2),
    Rung::Multiple(3),
    Rung::Multiple(4),
    Rung::Interlaced,
    Rung::Fitted,
];
const X2: &[Rung] = &[Rung::Multiple(2), Rung::Interlaced, Rung::Fitted];
const X3: &[Rung] = &[
    Rung::Multiple(3),
    Rung::Multiple(2),
    Rung::Interlaced,
    Rung::Fitted,
];
const X4: &[Rung] = &[
    Rung::Multiple(4),
    Rung::Multiple(3),
    Rung::Multiple(2),
    Rung::Interlaced,
    Rung::Fitted,
];

/// Step 3: the rungs a style walks, in order.
///
/// Auto walks every rung. An explicit style starts at its own rung and
/// falls through the rungs below it. For a multiple, "below" reads as: the
/// smaller multiples first, because they still give an exact scale, then
/// rungs 3 and 4; the native rung is above every multiple and is never
/// reached from one. Native starts at rung 1, so it walks the same rungs as
/// Auto. The tables above are the whole reading.
pub fn walk_ladder(style: PictureStyle) -> &'static [Rung] {
    match style {
        PictureStyle::Auto | PictureStyle::Native => AUTO,
        PictureStyle::X2 => X2,
        PictureStyle::X3 => X3,
        PictureStyle::X4 => X4,
    }
}

impl fmt::Display for Rung {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Native => f.write_str("native"),
            Self::Multiple(n) => write!(f, "{n}x"),
            Self::Interlaced => f.write_str("interlaced 1:1"),
            Self::Fitted => f.write_str("fitted"),
            Self::Panel => f.write_str("panel"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_walks_every_rung_in_order() {
        assert_eq!(walk_ladder(PictureStyle::Auto), AUTO);
        assert_eq!(walk_ladder(PictureStyle::Auto).first(), Some(&Rung::Native));
        assert_eq!(walk_ladder(PictureStyle::Auto).last(), Some(&Rung::Fitted));
    }

    #[test]
    fn native_starts_at_rung_one_and_falls_through_every_rung_below() {
        assert_eq!(walk_ladder(PictureStyle::Native), AUTO);
    }

    #[test]
    fn a_multiple_falls_to_the_smaller_multiples_then_rungs_three_and_four() {
        assert_eq!(
            walk_ladder(PictureStyle::X2),
            [Rung::Multiple(2), Rung::Interlaced, Rung::Fitted]
        );
        assert_eq!(
            walk_ladder(PictureStyle::X4),
            [
                Rung::Multiple(4),
                Rung::Multiple(3),
                Rung::Multiple(2),
                Rung::Interlaced,
                Rung::Fitted
            ]
        );
        assert!(!walk_ladder(PictureStyle::X3).contains(&Rung::Native));
    }

    #[test]
    fn every_walk_ends_at_the_fitted_rung() {
        for style in PictureStyle::ALL {
            assert_eq!(walk_ladder(style).last(), Some(&Rung::Fitted), "{style}");
        }
    }
}
