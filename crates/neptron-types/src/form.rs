use crate::Rotation;

/// The shape of the UI canvas, as the player sees it. The canvas is drawn
/// upright for the player in every form, and its size is the size of its
/// `Frame` in `Scene::ui`.
///
/// A closed set: each form has its own layout numbers and goldens in the
/// UI, so a new form is a new value here and a new row of [`Form::of`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Form {
    /// A canvas wider than it is tall: 320x240.
    #[default]
    Landscape,
    /// A canvas taller than it is wide: 240x320.
    Portrait,
}

impl Form {
    /// The form of the UI on a display whose face, in scan-out, is
    /// `face_width` by `face_height` (any one unit: a 4:3 aspect, or the
    /// pixels of a native mode), with the screen orientation `screen`.
    ///
    /// The form is [`Form::Portrait`] when the face, as the player sees it,
    /// is taller than it is wide, else [`Form::Landscape`]. A turn across
    /// (90 or 270) swaps the sides of the face for the player.
    pub const fn of(face_width: u32, face_height: u32, screen: Rotation) -> Form {
        let (width, height) = if screen.across() {
            (face_height, face_width)
        } else {
            (face_width, face_height)
        };
        if height > width {
            Form::Portrait
        } else {
            Form::Landscape
        }
    }

    /// The size of the canvas of this form, in pixels: width, then height.
    pub const fn canvas(self) -> (u16, u16) {
        match self {
            Form::Landscape => (320, 240),
            Form::Portrait => (240, 320),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The rows of the form factor table of `docs/spec/ui-layout.md`,
    /// section 6.2.
    #[test]
    fn the_form_follows_the_face_as_the_player_sees_it() {
        let crt = (4, 3);
        let landscape_panel = (1_920, 1_080);
        let portrait_panel = (1_080, 1_920);
        for (w, h) in [crt, landscape_panel] {
            assert_eq!(Form::of(w, h, Rotation::R0), Form::Landscape);
            assert_eq!(Form::of(w, h, Rotation::R180), Form::Landscape);
            assert_eq!(Form::of(w, h, Rotation::R90), Form::Portrait);
            assert_eq!(Form::of(w, h, Rotation::R270), Form::Portrait);
        }
        let (w, h) = portrait_panel;
        assert_eq!(Form::of(w, h, Rotation::R0), Form::Portrait);
        assert_eq!(Form::of(w, h, Rotation::R180), Form::Portrait);
        assert_eq!(Form::of(w, h, Rotation::R90), Form::Landscape);
        assert_eq!(Form::of(w, h, Rotation::R270), Form::Landscape);
    }

    #[test]
    fn a_square_or_empty_face_is_landscape() {
        for screen in Rotation::ALL {
            assert_eq!(Form::of(5, 5, screen), Form::Landscape);
            assert_eq!(Form::of(0, 0, screen), Form::Landscape);
        }
    }

    #[test]
    fn the_canvas_of_each_form_has_its_shape() {
        assert_eq!(Form::Landscape.canvas(), (320, 240));
        assert_eq!(Form::Portrait.canvas(), (240, 320));
        assert_eq!(Form::default(), Form::Landscape);
    }
}
