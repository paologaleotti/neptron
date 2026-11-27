//! The two fields of an interlaced mode: their pictures, where field 2
//! lies against field 1 on the glass, and the flicker of thin horizontal
//! detail, with and without a model of the flicker filter. Pure.
//!
//! The frame buffer of an interlaced mode holds both fields woven: row
//! `r` is scanned in field `r % 2 + 1`.

use neptron_types::{Modeline, Rect, Rotation};

use super::glass::{Band, Glass};
use super::image::Rgb;

/// The facts of the two fields of an interlaced mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fields {
    /// How far field 2 lies below field 1 on the glass, in thousandths of
    /// the distance between two lines of one field: 500 when field 2 lies
    /// half a line below.
    pub field_offset_milli: i64,
    /// Each line of the frame buffer has its own band on the glass: the
    /// bands are in order and do not overlap.
    pub own_lines: bool,
    /// The flicker of thin horizontal detail, in per mille of full white:
    /// the mean, over the pixels of the game area, of the difference of
    /// light between the line of field 1 and the line of field 2 of each
    /// line pair. A line pair is one place on the glass that the two
    /// fields light in turn, so the difference blinks at the frame rate.
    pub flicker_permille: u32,
    /// The same with the model of the flicker filter applied to the game
    /// area.
    pub filtered_flicker_permille: u32,
    /// Which source lines each field carries, read from the frame buffer
    /// of the line card; `None` until it is read.
    pub source_lines: Option<SourceLines>,
}

/// The source lines that the two fields carry, read from the light of the
/// line card ([`line_card`]) drawn with the scene of the case.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceLines {
    /// Each line of the game area shows one source line, no source line
    /// shows twice, field 1 carries the even source lines and field 2 the
    /// odd ones.
    pub own: bool,
    /// The source lines that both fields show: a frame of half the lines
    /// doubled into an interlaced mode shows every line in both.
    pub shared: u32,
}

/// The blue of every pixel of the line card: a pixel with another blue is
/// a blend of lines or no picture.
const LINE_MARK: u8 = 0x5A;

/// A frame of `width` by `height` whose pixels have the colour of the line
/// of the upright picture they land on after the turn `rotation`: red the
/// low byte, green the high byte, blue [`LINE_MARK`]. Upright, line `y` of
/// the frame is line `y`; turned by 90 or 270, a column of the frame is a
/// line of the picture.
pub fn line_card(width: u16, height: u16, rotation: Rotation) -> Rgb {
    let (w, h) = (u32::from(width), u32::from(height));
    let mut img = Rgb::new(w, h, [0; 3]);
    for y in 0..h {
        for x in 0..w {
            let line = match rotation {
                Rotation::R0 => y,
                Rotation::R180 => h - 1 - y,
                Rotation::R90 => w - 1 - x,
                Rotation::R270 => x,
            };
            img.put(
                x,
                y,
                [(line & 0xFF) as u8, ((line >> 8) & 0xFF) as u8, LINE_MARK],
            );
        }
    }
    img
}

/// The source lines that the fields of `fb` carry in the game area `dest`,
/// when `fb` shows the line card. Row `r` is in field `r % 2 + 1`.
pub fn source_lines(fb: &Rgb, dest: Rect) -> SourceLines {
    let (x0, x1, y0, y1) = inside(fb, dest);
    let x = (x0 + x1) / 2;
    let mut fields_of: std::collections::BTreeMap<u32, u8> = std::collections::BTreeMap::new();
    let mut own = y1 > y0;
    for y in y0..y1 {
        let px = fb.get(x, y);
        if px[2] != LINE_MARK {
            own = false;
            continue;
        }
        let line = u32::from(px[0]) | u32::from(px[1]) << 8;
        let field = 1_u8 << (y % 2);
        let seen = fields_of.entry(line).or_insert(0);
        if *seen != 0 || line % 2 != y % 2 {
            own = false;
        }
        *seen |= field;
    }
    let shared = fields_of.values().filter(|&&f| f == 3).count();
    SourceLines {
        own,
        shared: u32::try_from(shared).unwrap_or(u32::MAX),
    }
}

/// The facts of the fields of `fb` in `mode` on `glass`, with the game in
/// `dest`; `None` for a progressive mode.
pub fn fields(glass: &Glass, mode: &Modeline, fb: &Rgb, dest: Rect) -> Option<Fields> {
    if !mode.interlace || mode.vactive < 3 {
        return None;
    }
    let (field_offset_milli, own_lines) = pairing(&glass.bands(mode));
    Some(Fields {
        field_offset_milli,
        own_lines,
        flicker_permille: flicker(fb, dest),
        filtered_flicker_permille: flicker(&flicker_filter(fb, dest), dest),
        source_lines: None,
    })
}

/// Where the second band lies against the first, in thousandths of the
/// distance from the first to the third (the pitch of one field), and
/// whether every band has its own place: in order, with no overlap.
pub fn pairing(bands: &[Band]) -> (i64, bool) {
    if bands.len() < 3 {
        return (0, false);
    }
    let centre = |i: usize| (bands[i].top + bands[i].bottom) / 2.0;
    let field_pitch = centre(2) - centre(0);
    let offset = if field_pitch > 0.0 {
        (centre(1) - centre(0)) / field_pitch
    } else {
        0.0
    };
    let own_lines = bands
        .windows(2)
        .all(|w| w[0].bottom <= w[1].top + 1e-9 && w[0].top < w[1].top);
    ((offset * 1000.0).round() as i64, own_lines)
}

/// The glass picture of field `field` (1 or 2) alone: the lines of the
/// other field stay dark.
pub fn field_picture(
    glass: &Glass,
    mode: &Modeline,
    fb: &Rgb,
    size: (u32, u32),
    field: u32,
) -> Rgb {
    let parity = field.saturating_sub(1) % 2;
    glass.render_bands(mode, fb, size, |b| b.row % 2 == parity)
}

/// The flicker of `fb` over the game area `dest`: see
/// [`Fields::flicker_permille`].
pub fn flicker(fb: &Rgb, dest: Rect) -> u32 {
    let (x0, x1, y0, y1) = inside(fb, dest);
    let (mut sum, mut count) = (0_u64, 0_u64);
    let mut y = y0 + y0 % 2;
    while y + 1 < y1 {
        for x in x0..x1 {
            sum += u64::from(luma(fb.get(x, y)).abs_diff(luma(fb.get(x, y + 1))));
            count += 1;
        }
        y += 2;
    }
    if count == 0 {
        return 0;
    }
    ((sum * 1000 + count * 255 / 2) / (count * 255)) as u32
}

/// The model of the flicker filter: each line of the game area takes a
/// quarter of the line above and a quarter of the line below,
/// `(in(y - 1) + 2 x in(y) + in(y + 1) + 2) / 4` per channel, with the
/// edge lines of the area repeated outside it. The product has no such
/// filter yet; this is the blend of its spec, for the simulator only.
pub fn flicker_filter(fb: &Rgb, dest: Rect) -> Rgb {
    let mut out = fb.clone();
    let (x0, x1, y0, y1) = inside(fb, dest);
    if y1 <= y0 {
        return out;
    }
    for y in y0..y1 {
        let up = y.saturating_sub(1).max(y0);
        let down = (y + 1).min(y1 - 1);
        for x in x0..x1 {
            let (a, b, c) = (fb.get(x, up), fb.get(x, y), fb.get(x, down));
            let blend = |i: usize| {
                ((u16::from(a[i]) + 2 * u16::from(b[i]) + u16::from(c[i]) + 2) / 4) as u8
            };
            out.put(x, y, [blend(0), blend(1), blend(2)]);
        }
    }
    out
}

/// The part of `dest` inside `fb`: first and end column, first and end
/// row.
fn inside(fb: &Rgb, dest: Rect) -> (u32, u32, u32, u32) {
    let clamp = |v: i64, max: u32| v.clamp(0, i64::from(max)) as u32;
    let (x, y) = (i64::from(dest.x), i64::from(dest.y));
    (
        clamp(x, fb.width),
        clamp(x + i64::from(dest.w), fb.width),
        clamp(y, fb.height),
        clamp(y + i64::from(dest.h), fb.height),
    )
}

/// The light of a pixel, 0 to 255, with the weights of BT.601.
fn luma(px: [u8; 3]) -> u32 {
    (299 * u32::from(px[0]) + 587 * u32::from(px[1]) + 114 * u32::from(px[2]) + 500) / 1000
}

#[cfg(test)]
mod tests {
    use neptron_types::{TV_525_240P, TV_525_480I, TvStandard};

    use super::super::glass::Tube;
    use super::*;

    fn all(fb: &Rgb) -> Rect {
        Rect {
            x: 0,
            y: 0,
            w: fb.width,
            h: fb.height,
        }
    }

    /// One white line on black: the pair blinks between white and black,
    /// so over two lines the mean is one half of full white. The filter
    /// spreads it over three lines: 1/4, 1/2, 1/4.
    #[test]
    fn a_thin_line_flickers_and_the_filter_lowers_it() {
        let mut fb = Rgb::new(4, 4, [0; 3]);
        for x in 0..4 {
            fb.put(x, 1, [255; 3]);
        }
        assert_eq!(flicker(&fb, all(&fb)), 500);
        let filtered = flicker_filter(&fb, all(&fb));
        assert_eq!(filtered.get(0, 0), [64; 3]);
        assert_eq!(filtered.get(0, 1), [128; 3]);
        assert_eq!(filtered.get(0, 2), [64; 3]);
        // Pair (0, 1): |64 - 128|; pair (2, 3): |64 - 0|: 64 of 255 a pair.
        assert_eq!(flicker(&filtered, all(&fb)), 251);
    }

    #[test]
    fn a_flat_picture_does_not_flicker() {
        let fb = Rgb::new(8, 8, [200; 3]);
        assert_eq!(flicker(&fb, all(&fb)), 0);
        assert_eq!(flicker_filter(&fb, all(&fb)), fb);
    }

    /// On a TV in 480i, field 2 lies half a line below field 1, and every
    /// row has its own band.
    #[test]
    fn field_2_lies_half_a_line_below_field_1() {
        let tv = Glass::Tube(Tube::tv(TvStandard::Ntsc, 0));
        let fb = Rgb::new(720, 480, [100; 3]);
        let f = fields(&tv, &TV_525_480I, &fb, all(&fb)).unwrap();
        assert_eq!(f.field_offset_milli, 500);
        assert!(f.own_lines);
        assert_eq!(fields(&tv, &TV_525_240P, &fb, all(&fb)), None);
    }

    /// A wrong weave, with field 2 on top of field 1, is caught.
    #[test]
    fn field_2_on_top_of_field_1_is_caught() {
        let band = |row: u32, top: f64| Band {
            row,
            top,
            bottom: top + 0.01,
        };
        let wrong = [band(0, 0.0), band(1, 0.0), band(2, 0.02), band(3, 0.02)];
        assert_eq!(pairing(&wrong), (0, false));
        let right = [band(0, 0.0), band(1, 0.01), band(2, 0.02), band(3, 0.03)];
        assert_eq!(pairing(&right), (500, true));
    }

    /// A 480-line card 1:1 in a 480i mode: each field carries its own
    /// lines. The top 240 lines doubled: both fields carry every line.
    #[test]
    fn a_turned_line_card_numbers_the_columns() {
        let card = line_card(3, 2, Rotation::R90);
        assert_eq!(card.get(0, 0)[0], 2);
        assert_eq!(card.get(2, 1)[0], 0);
        assert_eq!(line_card(3, 2, Rotation::R270).get(2, 0)[0], 2);
    }

    #[test]
    fn a_doubled_frame_shows_the_same_lines_in_both_fields() {
        let card = line_card(4, 480, Rotation::R0);
        let dest = all(&card);
        assert_eq!(
            source_lines(&card, dest),
            SourceLines {
                own: true,
                shared: 0
            }
        );
        let mut doubled = Rgb::new(4, 480, [0; 3]);
        for y in 0..480 {
            for x in 0..4 {
                doubled.put(x, y, card.get(x, y / 2));
            }
        }
        let lines = source_lines(&doubled, dest);
        assert!(!lines.own);
        assert_eq!(lines.shared, 240);
        let mut shifted = Rgb::new(4, 480, [0; 3]);
        shifted.paste(&card, 0, 1);
        assert!(
            !source_lines(&shifted, dest).own,
            "field 2 carries even lines"
        );
    }

    /// Field 1 alone lights only its rows.
    #[test]
    fn field_1_alone_lights_only_its_rows() {
        let tv = Glass::Tube(Tube::tv(TvStandard::Ntsc, 0));
        let mut fb = Rgb::new(720, 480, [0; 3]);
        for x in 0..720 {
            fb.put(x, 241, [255; 3]);
        }
        let one = field_picture(&tv, &TV_525_480I, &fb, (720, 480), 1);
        let two = field_picture(&tv, &TV_525_480I, &fb, (720, 480), 2);
        let lit = |img: &Rgb| (0..480).filter(|&y| img.get(360, y)[0] > 0).count();
        assert_eq!(lit(&one), 0, "row 241 is in field 2");
        assert!(lit(&two) > 0);
    }
}
