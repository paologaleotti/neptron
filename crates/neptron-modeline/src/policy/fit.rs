use super::Dropped;
use super::OrientedSource;
use super::shape::game_aspect_milli;
use crate::Aspect;

/// How much less of the lines an exact picture may fill than the fit and
/// still stay under the picture mode `fit`, in per mille of the lines.
///
/// The fit gains less than one percent of the height there, and the exact
/// picture keeps every pixel sharp. The margin also absorbs the rounding
/// of a whole-kHz clock and of a width snapped to a whole sample.
pub const FIT_EXACT_MARGIN_PERMILLE: u32 = 10;

/// The share of the screen height that the fit fills, in per mille: the
/// whole height, or less for a game wider than the face, whose width then
/// fills the face.
pub(crate) fn fit_share_permille(source: &OrientedSource, face: Aspect) -> u32 {
    let game = u64::from(game_aspect_milli(source)).max(1);
    let face_milli = u64::from(face.width) * 1_000 / u64::from(face.height.max(1));
    let share = face_milli * 1_000 / game;
    u32::try_from(share.min(1_000)).unwrap_or(1_000)
}

/// The reason an exact picture gives way to the fit under the picture mode
/// `fit`, or `None` when it stays: it is progressive, cuts no line and
/// fills at least the share of the fit less [`FIT_EXACT_MARGIN_PERMILLE`].
pub(crate) fn fit_larger(
    fill_permille: u32,
    fit_permille: u32,
    cut_lines: u16,
    interlaced: bool,
) -> Option<Dropped> {
    let smaller = fill_permille + FIT_EXACT_MARGIN_PERMILLE < fit_permille;
    (interlaced || cut_lines > 0 || smaller).then_some(Dropped::FitLarger {
        fill_permille,
        fit_permille,
        cut_lines,
        interlaced,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(width: u16, height: u16, aspect_milli: u32) -> OrientedSource {
        OrientedSource {
            width,
            height,
            refresh_mhz: 60_000,
            interlaced: false,
            aspect_milli,
            frame_width: width,
            frame_height: height,
            turned: false,
        }
    }

    const FACE: Aspect = Aspect {
        width: 4,
        height: 3,
    };

    #[test]
    fn a_game_as_wide_as_the_face_or_narrower_fills_the_height() {
        assert_eq!(fit_share_permille(&source(256, 224, 1_333), FACE), 1_000);
        assert_eq!(fit_share_permille(&source(240, 320, 750), FACE), 1_000);
    }

    #[test]
    fn a_game_wider_than_the_face_fills_the_width() {
        // Virtual Boy, 384:224 = 1.714: 1.333 / 1.714.
        assert_eq!(fit_share_permille(&source(384, 224, 1_714), FACE), 777);
    }

    #[test]
    fn square_pixels_give_the_shape_of_the_frame() {
        assert_eq!(fit_share_permille(&source(320, 240, 0), FACE), 1_000);
        assert_eq!(fit_share_permille(&source(480, 240, 0), FACE), 666);
    }

    #[test]
    fn an_exact_picture_as_large_as_the_fit_stays() {
        assert_eq!(fit_larger(1_000, 1_000, 0, false), None);
        assert_eq!(fit_larger(990, 1_000, 0, false), None);
        assert_eq!(fit_larger(770, 777, 0, false), None);
    }

    #[test]
    fn a_smaller_exact_picture_gives_way() {
        assert_eq!(
            fit_larger(933, 1_000, 0, false),
            Some(Dropped::FitLarger {
                fill_permille: 933,
                fit_permille: 1_000,
                cut_lines: 0,
                interlaced: false,
            })
        );
        assert!(fit_larger(989, 1_000, 0, false).is_some());
    }

    #[test]
    fn a_cut_or_an_interlaced_picture_gives_way_at_any_size() {
        assert!(fit_larger(1_000, 1_000, 16, false).is_some());
        assert!(fit_larger(1_000, 1_000, 0, true).is_some());
    }
}
