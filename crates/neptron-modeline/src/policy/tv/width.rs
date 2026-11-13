use crate::policy::TvWidth;
use crate::policy::source::div_round;

/// The width of the picture area of a TV line, in 13.5 MHz samples: the
/// 4:3 width of ITU-R BT.601 practice, centred in the 720 active samples.
/// `fill` takes it, `integer` stays inside it, and every width stays
/// inside it. The 8 samples at each side are blanking on most sets.
pub const TV_SAFE_WIDTH_SAMPLES: u16 = 704;

/// The lines of one field that a 4:3 picture spans on the 525-line system.
/// [`TV_SAFE_WIDTH_SAMPLES`] over these lines at 4:3 gives the shape of a
/// sample: 704 / (4/3 x 240) = 2.2 samples per line of aspect 1, so a
/// sample has a pixel aspect of 10/11.
pub const TV_525_PICTURE_LINES: u16 = 240;

/// The lines of one field that a 4:3 picture spans on the 625-line system:
/// 704 / (4/3 x 288) = 1.8333 samples per line of aspect 1, a pixel
/// aspect of 12/11.
pub const TV_625_PICTURE_LINES: u16 = 288;

/// The width of a 4:3 console picture on the 525-line system, in 13.5 MHz
/// samples. The NES, the SNES and the Mega Drive H32 draw 256 pixels at
/// 21.477 MHz / 4, the Mega Drive H40 320 pixels at 53.693 MHz / 8, and
/// the SNES 512 pixels at 10.739 MHz: each is 47.68 us of active picture,
/// 643.7 samples. The console does not fill the 4:3 line, so its picture
/// is narrower than the shape of its 4:3 aspect.
pub const TV_525_CONSOLE_WIDTH_SAMPLES: u16 = 644;

/// The width of a 4:3 console picture on the 625-line system, in 13.5 MHz
/// samples: the NES PAL draws 256 pixels at 26.602 MHz / 5, 48.12 us,
/// 649.6 samples.
pub const TV_625_CONSOLE_WIDTH_SAMPLES: u16 = 650;

/// The fewest lines per field of a console picture: a 4:3 source with
/// this many lines per field up to the 525-line picture lines (224 to
/// 240) gets the console width of its line system. A taller 4:3 source
/// gets at least the console width: it fills more of the frame than a
/// console picture, so it can be wider, never narrower.
pub const TV_CONSOLE_LINES_MIN: u16 = 224;

/// How far a whole or half factor of the game width can be from the
/// `aspect` width and still win, in percent of the `aspect` width.
pub const TV_WHOLE_FACTOR_TOLERANCE_PERCENT: u32 = 3;

/// The display aspect 4:3, in thousandths. A core gives 4/3 as a float,
/// so the source carries 1333 or 1334.
const FOUR_THIRDS_MILLI: u32 = 1_333;

/// The line system of a TV standard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LineSystem {
    /// 525 lines, 240p and 480i.
    L525,
    /// 625 lines, 288p and 576i.
    L625,
}

/// The game's width on a TV line, in samples, by the rule of
/// `display.tv_width`. `width` and `height` are the game frame in pixels,
/// `aspect_milli` its display aspect (0 for square pixels), and
/// `field_lines` the lines of one field that the whole frame spans as it
/// is placed. The result is at least 1 and at most
/// [`TV_SAFE_WIDTH_SAMPLES`].
pub(crate) fn tv_samples(
    rule: TvWidth,
    system: LineSystem,
    width: u16,
    height: u16,
    aspect_milli: u32,
    field_lines: u16,
) -> u32 {
    let safe = u32::from(TV_SAFE_WIDTH_SAMPLES);
    let width = u32::from(width.max(1));
    match rule {
        TvWidth::Aspect => {
            let aspect = match aspect_milli {
                0 => div_round(u64::from(width) * 1_000, u64::from(height.max(1))),
                a => a,
            };
            snap(width, aspect_samples(system, aspect, field_lines))
        }
        TvWidth::Integer => match safe / width {
            0 => safe,
            k => k * width,
        },
        TvWidth::Fill => safe,
    }
}

/// The width that keeps the display aspect over `field_lines` lines of
/// the line system, or the console width for a 4:3 console picture.
///
/// A 4:3 source of 224 to 240 lines per field gets the console width. A
/// taller 4:3 source gets the larger of the console width and the aspect
/// width, so the width never falls as the lines rise: on 625 lines the
/// aspect width of 241 lines is 589 samples, below the console's 650.
///
/// The division rounds to the nearest sample, a half up.
fn aspect_samples(system: LineSystem, aspect_milli: u32, field_lines: u16) -> u32 {
    let (picture_lines, console) = match system {
        LineSystem::L525 => (TV_525_PICTURE_LINES, TV_525_CONSOLE_WIDTH_SAMPLES),
        LineSystem::L625 => (TV_625_PICTURE_LINES, TV_625_CONSOLE_WIDTH_SAMPLES),
    };
    let samples = div_round(
        u64::from(aspect_milli) * u64::from(field_lines) * u64::from(TV_SAFE_WIDTH_SAMPLES) * 3,
        4_000 * u64::from(picture_lines),
    );
    let console = u32::from(console);
    let four_thirds = aspect_milli.abs_diff(FOUR_THIRDS_MILLI) <= 1;
    let samples = match field_lines {
        _ if !four_thirds => samples,
        lines if lines < TV_CONSOLE_LINES_MIN => samples,
        lines if lines <= TV_525_PICTURE_LINES => console,
        _ => samples.max(console),
    };
    samples.clamp(1, u32::from(TV_SAFE_WIDTH_SAMPLES))
}

/// The whole or half factor of `width` nearest `target`, when it is a
/// whole number of samples, within [`TV_WHOLE_FACTOR_TOLERANCE_PERCENT`]
/// and inside the safe width; else `target` itself.
fn snap(width: u32, target: u32) -> u32 {
    let safe = u32::from(TV_SAFE_WIDTH_SAMPLES);
    // Half factors are counted in halves of the width: n halves, n >= 2.
    let low = (target * 2 / width).max(2);
    [low, low + 1]
        .into_iter()
        .filter(|&n| (n * width).is_multiple_of(2))
        .map(|n| n * width / 2)
        .filter(|&w| w <= safe)
        .min_by_key(|&w| w.abs_diff(target))
        .filter(|&w| {
            u64::from(w.abs_diff(target)) * 100
                <= u64::from(TV_WHOLE_FACTOR_TOLERANCE_PERCENT) * u64::from(target)
        })
        .unwrap_or(target)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FOUR_THIRDS: u32 = 1_333;

    fn aspect(system: LineSystem, width: u16, lines: u16, aspect_milli: u32) -> u32 {
        tv_samples(TvWidth::Aspect, system, width, lines, aspect_milli, lines)
    }

    #[test]
    fn a_4_3_console_picture_gets_the_console_width_snapped_to_a_half_factor() {
        use LineSystem::{L525, L625};
        // 644 / 256 = 2.52: 2.5x is 640, 0.6 % off.
        assert_eq!(aspect(L525, 256, 224, FOUR_THIRDS), 640);
        assert_eq!(aspect(L525, 256, 240, FOUR_THIRDS), 640);
        // 650 / 256: 640 is 1.5 % off.
        assert_eq!(aspect(L625, 256, 239, FOUR_THIRDS), 640);
        // 320: 2x.
        assert_eq!(aspect(L525, 320, 224, FOUR_THIRDS), 640);
        assert_eq!(aspect(L625, 320, 240, FOUR_THIRDS), 640);
        // 512: 1x and 1.5x are 20 % off.
        assert_eq!(aspect(L525, 512, 224, FOUR_THIRDS), 644);
        assert_eq!(aspect(L625, 512, 224, FOUR_THIRDS), 650);
        // 640: 1x.
        assert_eq!(aspect(L525, 640, 240, FOUR_THIRDS), 640);
        assert_eq!(aspect(L625, 640, 240, FOUR_THIRDS), 640);
        // A core that rounds 4/3 up.
        assert_eq!(aspect(L525, 512, 224, 1_334), 644);
    }

    #[test]
    fn any_other_source_keeps_its_aspect_with_the_sample_shape_of_the_system() {
        use LineSystem::{L525, L625};
        // Game Boy, 10:9, 144 lines: 1.111 x 144 x 2.2 = 352 and
        // 1.111 x 144 x 1.8333 = 293.
        assert_eq!(aspect(L525, 160, 144, 1_111), 352);
        assert_eq!(aspect(L625, 160, 144, 1_111), 293);
        // GBA, 3:2, 160 lines: 528 and 440.
        assert_eq!(aspect(L525, 240, 160, 1_500), 528);
        assert_eq!(aspect(L625, 240, 160, 1_500), 440);
        // A turned 224x256 board, 3:4: 422 and 352.
        assert_eq!(aspect(L525, 224, 256, 750), 422);
        assert_eq!(aspect(L625, 224, 256, 750), 352);
        // 224x288, 3:4, on 625 lines: 396.
        assert_eq!(aspect(L625, 224, 288, 750), 396);
        // A 4:3 picture of 288 lines on 625 lines fills the 4:3 width.
        assert_eq!(aspect(L625, 352, 288, FOUR_THIRDS), 704);
        // A 4:3 picture of 192 lines is not a console picture of the
        // console rule: 1.333 x 192 x 2.2 = 563.
        assert_eq!(aspect(L525, 256, 192, FOUR_THIRDS), 563);
    }

    #[test]
    fn a_taller_4_3_source_is_never_narrower_than_a_console_picture() {
        use LineSystem::{L525, L625};
        // 241 lines: 707 samples, capped at 704, on 525 lines; the
        // console's 650 on 625 lines, where the aspect width is 589.
        assert_eq!(aspect_samples(L525, FOUR_THIRDS, 240), 644);
        assert_eq!(aspect_samples(L525, FOUR_THIRDS, 241), 704);
        assert_eq!(aspect_samples(L625, FOUR_THIRDS, 240), 650);
        assert_eq!(aspect_samples(L625, FOUR_THIRDS, 241), 650);
        assert_eq!(aspect_samples(L625, FOUR_THIRDS, 288), 704);
        // Through the snap, 256 wide: 2.5x up to the console edge.
        assert_eq!(aspect(L525, 256, 240, FOUR_THIRDS), 640);
        assert_eq!(aspect(L525, 256, 241, FOUR_THIRDS), 704);
        assert_eq!(aspect(L625, 256, 240, FOUR_THIRDS), 640);
        assert_eq!(aspect(L625, 256, 241, FOUR_THIRDS), 640);
    }

    #[test]
    fn the_width_of_a_4_3_source_never_falls_as_its_lines_rise() {
        for system in [LineSystem::L525, LineSystem::L625] {
            let widths: Vec<u32> = (224..=288)
                .map(|lines| aspect(system, 256, lines, FOUR_THIRDS))
                .collect();
            assert!(
                widths.windows(2).all(|w| w[0] <= w[1]),
                "{system:?}: {widths:?}"
            );
        }
    }

    #[test]
    fn square_pixels_give_the_aspect_of_the_frame() {
        assert_eq!(
            tv_samples(TvWidth::Aspect, LineSystem::L525, 160, 144, 0, 144),
            352
        );
    }

    #[test]
    fn the_tolerance_is_3_percent_of_the_aspect_width() {
        // 3 % of 644 is 19.32 samples: 625 and 663 are inside, 624 and
        // 664 are not.
        assert_eq!(snap(625, 644), 625);
        assert_eq!(snap(663, 644), 663);
        assert_eq!(snap(624, 644), 644);
        assert_eq!(snap(664, 644), 644);
        // 1.5x of 250 is 375, 2 % from 383.
        assert_eq!(snap(250, 383), 375);
        // An odd width has no whole half factor: 2.5 x 255 is 637.5.
        assert_eq!(snap(255, 640), 640);
        assert_eq!(snap(255, 645), 645);
    }

    #[test]
    fn integer_takes_the_largest_whole_factor_inside_the_safe_width() {
        let integer = |w| tv_samples(TvWidth::Integer, LineSystem::L525, w, 224, FOUR_THIRDS, 224);
        assert_eq!(integer(256), 512);
        assert_eq!(integer(320), 640);
        assert_eq!(integer(512), 512);
        assert_eq!(integer(640), 640);
        assert_eq!(integer(160), 640);
        assert_eq!(integer(176), 704);
        assert_eq!(integer(720), 704);
    }

    #[test]
    fn fill_takes_the_safe_width() {
        for w in [256, 320, 512, 640, 1_024] {
            assert_eq!(
                tv_samples(TvWidth::Fill, LineSystem::L625, w, 224, FOUR_THIRDS, 224),
                704
            );
        }
    }

    #[test]
    fn the_width_stays_inside_the_safe_width_at_the_extremes() {
        use LineSystem::L525;
        assert_eq!(tv_samples(TvWidth::Aspect, L525, 1, 1, u32::MAX, 240), 704);
        assert_eq!(tv_samples(TvWidth::Aspect, L525, u16::MAX, 1, 1, 1), 1);
        assert_eq!(tv_samples(TvWidth::Aspect, L525, 0, 0, 0, 0), 1);
        assert_eq!(
            tv_samples(TvWidth::Aspect, L525, 1, 1, u32::MAX, u16::MAX),
            704
        );
        assert_eq!(tv_samples(TvWidth::Integer, L525, u16::MAX, 1, 0, 1), 704);
    }
}
