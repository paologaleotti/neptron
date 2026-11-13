use crate::policy::TvWidth;
use crate::policy::source::div_round;

/// The width of the picture area of a TV line, in 13.5 MHz samples: the
/// 4:3 width of ITU-R BT.601 practice, centred in the 720 active samples.
/// `fill` takes it, `integer` stays inside it, and the UI layer fills it.
/// The 8 samples at each side are blanking on most sets.
pub const TV_SAFE_WIDTH_SAMPLES: u16 = 704;

/// The width of a console picture with a 4:3 display aspect, in 13.5 MHz
/// samples. The NES, the SNES and the Mega Drive H32 draw 256 pixels at
/// 21.477 MHz / 4, the Mega Drive H40 320 pixels at 53.693 MHz / 8, and
/// the SNES 512 pixels at 10.739 MHz: each is 47.68 us, 643.7 samples.
/// The NES PAL draws 649.6 samples, within 1 %.
pub const TV_CONSOLE_WIDTH_SAMPLES: u16 = 644;

/// The lines per field of a console picture: a frame of at least this
/// many lines with a 4:3 display aspect is
/// [`TV_CONSOLE_WIDTH_SAMPLES`] wide. A core gives 4:3 for a 224-line
/// SNES frame and for a 240-line NES frame alike, and both consoles drew
/// the same width. A frame with fewer lines, such as a handheld, is a
/// smaller picture: its width shrinks with its lines.
pub const TV_CONSOLE_LINES: u16 = 224;

/// How far a whole factor of the game width can be from the `aspect`
/// width and still win, in percent of the `aspect` width.
pub const TV_WHOLE_FACTOR_TOLERANCE_PERCENT: u32 = 3;

/// The game's width on a TV line, in samples, by the rule of
/// `display.tv_width`. `width` is the game width in pixels, `aspect_milli`
/// its display aspect (0 for square pixels of a `width` by `height`
/// frame), and `field_lines` the lines of one field that the game fills.
/// The result is at least 1 and at most [`TV_SAFE_WIDTH_SAMPLES`].
pub(crate) fn tv_samples(
    rule: TvWidth,
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
            snap(width, aspect_samples(aspect, field_lines))
        }
        TvWidth::Integer => match safe / width {
            0 => safe,
            k => k * width,
        },
        TvWidth::Fill => safe,
    }
}

/// The width of the console picture at a display aspect: the 4:3 width
/// [`TV_CONSOLE_WIDTH_SAMPLES`] times the aspect over 4:3, for a game of
/// [`TV_CONSOLE_LINES`] lines or more, and less in proportion for fewer
/// lines.
fn aspect_samples(aspect_milli: u32, field_lines: u16) -> u32 {
    let lines = u64::from(field_lines.min(TV_CONSOLE_LINES));
    let samples = div_round(
        u64::from(aspect_milli) * u64::from(TV_CONSOLE_WIDTH_SAMPLES) * 3 * lines,
        4_000 * u64::from(TV_CONSOLE_LINES),
    );
    samples.clamp(1, u32::from(TV_SAFE_WIDTH_SAMPLES))
}

/// The whole factor of `width` nearest `target` when it is within
/// [`TV_WHOLE_FACTOR_TOLERANCE_PERCENT`] and inside the safe width, else
/// `target` itself.
fn snap(width: u32, target: u32) -> u32 {
    let safe = u32::from(TV_SAFE_WIDTH_SAMPLES);
    let low = (target / width).max(1);
    [low, low + 1]
        .into_iter()
        .map(|k| k * width)
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

    fn aspect(width: u16, lines: u16) -> u32 {
        tv_samples(TvWidth::Aspect, width, lines, FOUR_THIRDS, lines)
    }

    #[test]
    fn aspect_gives_the_console_width_or_a_whole_factor_within_3_percent() {
        // 644 / 256 = 2.52: 512 and 768 are 20 % off, so no factor wins.
        assert_eq!(aspect(256, 224), 644);
        assert_eq!(aspect(256, 240), 644);
        // 640 is 0.6 % from 644: factor 2.
        assert_eq!(aspect(320, 224), 640);
        // 512 is 20 % off: 644, factor 1.26.
        assert_eq!(aspect(512, 224), 644);
        // 640 is 0.6 % off: factor 1.
        assert_eq!(aspect(640, 240), 640);
        // A 384-wide board: 768 is wider than the safe width.
        assert_eq!(aspect(384, 224), 644);
    }

    #[test]
    fn the_tolerance_is_3_percent_of_the_aspect_width() {
        // 3 % of 644 is 19.32 samples: 625 and 663 are inside, 624 and
        // 664 are not.
        assert_eq!(snap(625, 644), 625);
        assert_eq!(snap(663, 644), 663);
        assert_eq!(snap(624, 644), 644);
        assert_eq!(snap(664, 644), 644);
    }

    #[test]
    fn a_handheld_is_narrower_in_proportion_to_its_lines() {
        // Game Boy, 10:9: 1.111 x 644 x 3/4 x 144 / 224 = 345.
        assert_eq!(tv_samples(TvWidth::Aspect, 160, 144, 1_111, 144), 345);
        // With square pixels the aspect is 160 / 144, the same.
        assert_eq!(tv_samples(TvWidth::Aspect, 160, 144, 0, 144), 345);
    }

    #[test]
    fn integer_takes_the_largest_whole_factor_inside_the_safe_width() {
        let integer = |w| tv_samples(TvWidth::Integer, w, 224, FOUR_THIRDS, 224);
        assert_eq!(integer(256), 512);
        assert_eq!(integer(320), 640);
        assert_eq!(integer(512), 512);
        assert_eq!(integer(640), 640);
        assert_eq!(integer(160), 640);
        assert_eq!(integer(176), 704);
        // Wider than the safe width: the safe width, scaled down.
        assert_eq!(integer(720), 704);
    }

    #[test]
    fn fill_takes_the_safe_width() {
        for w in [256, 320, 512, 640, 1_024] {
            assert_eq!(tv_samples(TvWidth::Fill, w, 224, FOUR_THIRDS, 224), 704);
        }
    }

    #[test]
    fn the_width_stays_inside_the_safe_width_at_the_extremes() {
        assert_eq!(tv_samples(TvWidth::Aspect, 1, 1, u32::MAX, 240), 704);
        assert_eq!(tv_samples(TvWidth::Aspect, u16::MAX, 1, 1, 1), 1);
        assert_eq!(tv_samples(TvWidth::Aspect, 0, 0, 0, 0), 1);
        assert_eq!(tv_samples(TvWidth::Integer, u16::MAX, 1, 0, 1), 704);
    }
}
