//! Measures read from the pixels of a picture of the test card: the size
//! of the circle and of the outer border box, and which edges of the
//! border show. They read the light, not the placement of the policy, so
//! a placement and its measure cannot agree with each other by
//! construction. Pure.

use neptron_types::TvStandard;

use super::consoles::Console;
use super::glass::{Glass, TUBE_ASPECT, Tube, TubeClass};
use super::image::Rgb;
use super::json::{Json, object};
use super::measure::Edges;

/// The shape of the test card on the glass, measured from its light,
/// against the two references of a right shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GlassShape {
    /// The width to height of the circle on the glass, in thousandths.
    pub shown_milli: u32,
    /// Reference (a): the width to height of the circle when the frame
    /// has exactly the display aspect that the core gives, in thousandths.
    pub core_milli: u32,
    /// Shown over reference (a), less 1, in per mille.
    pub core_error_permille: i64,
    /// Reference (b), when the console of the source is in the table.
    pub console: Option<ConsoleShape>,
    /// The width to height of the outer border box against reference
    /// (a), less 1, in per mille, when its four sides show.
    pub border_error_permille: Option<i64>,
}

/// Reference (b): the circle when each pixel has the width that the dot
/// clock of the real console gives it on this screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConsoleShape {
    /// The console.
    pub name: &'static str,
    /// Its dot clock for the width of the frame, in hertz.
    pub dot_clock_hz: u32,
    /// The width to height of the circle, in thousandths.
    pub milli: u32,
    /// Shown over reference (b), less 1, in per mille.
    pub error_permille: i64,
}

/// What the card on the glass is measured against.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Expected {
    /// The card in the frame of the core, in frame pixels.
    pub frame: CardSize,
    /// The size of the frame, in pixels.
    pub frame_size: (u16, u16),
    /// The scene turns the frame by 90 or 270 degrees.
    pub across: bool,
    /// The width of a frame pixel over its height that the core gives.
    pub core_pixel_aspect: f64,
    /// The console, its dot clock and the width of a frame pixel over its
    /// height that it gives on this screen.
    pub console: Option<(&'static str, u32, f64)>,
}

/// The circle and the border box of the test card in one picture, in the
/// units of that picture: the centre of the line at each extreme.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CardSize {
    /// The width of the circle, centre of the line to centre of the line.
    pub circle_w: f64,
    /// The height of the circle.
    pub circle_h: f64,
    /// The width and the height of the outer border box, when all four
    /// sides of it show.
    pub border: Option<(f64, f64)>,
}

/// A colour channel of the circle stands out from blue by at least this
/// much: the circle line blends with the background on the way to the
/// glass, and less than this is not the circle.
const CIRCLE_MIN_LEVEL: f64 = 24.0;
/// The green of the circle over its red, both less blue, lies in this
/// range: yellow is 0.86 and stays so when it blends with grey or white;
/// the orange corner mark (0.57) and a blend of orange or red with the
/// green border (1.0 and above) do not pass.
const CIRCLE_GREEN_OVER_RED: (f64, f64) = (0.75, 0.95);
/// The red of the outer border over the green and the blue: at least this.
const BORDER_MIN_LEVEL: f64 = 60.0;
/// The part of the radius around the axis where the circle is measured:
/// close to the axis the line runs across the axis.
const AXIS_BAND: f64 = 0.15;
/// A column belongs to a side of the border when its red covers at least
/// this part of the rows between the first and the last red row (and a
/// row the same across). A side of a 240-line picture on a tube lights
/// about half of its height between the gaps; a side that the glass hides
/// leaves only the few red pixels of the top and the bottom.
const BORDER_SIDE_SHARE: f64 = 0.3;
/// The size of the circle and of the border box in `img`, whose pixels
/// are `unit` wide and high (`(ux, uy)`). `None` when no circle shows.
pub fn card_size(img: &Rgb, unit: (f64, f64)) -> Option<CardSize> {
    let (cw, ch) = circle(img)?;
    Some(CardSize {
        circle_w: cw * unit.0,
        circle_h: ch * unit.1,
        border: border_box(img).map(|(w, h)| (w * unit.0, h * unit.1)),
    })
}

/// The edges of the outer border that show in `img`: a side shows when a
/// red line runs along it.
pub fn border_seen(img: &Rgb) -> Edges<bool> {
    let (rows, columns) = border_sides(img);
    Edges {
        top: rows.0.is_some(),
        bottom: rows.1.is_some(),
        left: columns.0.is_some(),
        right: columns.1.is_some(),
    }
}

/// The shape of the card `glass`, measured on the glass, against
/// `expected`.
pub fn glass_shape(glass: &CardSize, expected: &Expected) -> GlassShape {
    let frame = &expected.frame;
    let turned = |w: f64, h: f64| if expected.across { h / w } else { w / h };
    let shown = glass.circle_w / glass.circle_h;
    let core = turned(frame.circle_w * expected.core_pixel_aspect, frame.circle_h);
    let error = |shown: f64, reference: f64| ((shown / reference - 1.0) * 1000.0).round() as i64;
    let milli = |v: f64| (v * 1000.0).round() as u32;
    let (fw, fh) = (
        f64::from(expected.frame_size.0) - 1.0,
        f64::from(expected.frame_size.1) - 1.0,
    );
    let border_core = turned(fw * expected.core_pixel_aspect, fh);
    GlassShape {
        shown_milli: milli(shown),
        core_milli: milli(core),
        core_error_permille: error(shown, core),
        console: expected.console.map(|(name, dot_clock_hz, par)| {
            let reference = turned(frame.circle_w * par, frame.circle_h);
            ConsoleShape {
                name,
                dot_clock_hz,
                milli: milli(reference),
                error_permille: error(shown, reference),
            }
        }),
        border_error_permille: glass.border.map(|(w, h)| error(w / h, border_core)),
    }
}

/// The centre of the lit part of `img` (any channel above
/// [`LIT_MIN_LEVEL`]), across and down, in per mille of `img`.
pub fn lit_centre(img: &Rgb) -> Option<(u32, u32)> {
    let (mut x0, mut x1, mut y0, mut y1) = (u32::MAX, 0, u32::MAX, 0);
    for y in 0..img.height {
        for x in 0..img.width {
            if img.get(x, y).iter().any(|&c| c > LIT_MIN_LEVEL) {
                x0 = x0.min(x);
                x1 = x1.max(x + 1);
                y0 = y0.min(y);
                y1 = y1.max(y + 1);
            }
        }
    }
    let centre = |a: u32, b: u32, len: u32| {
        ((u64::from(a + b) * 500 + u64::from(len) / 2) / u64::from(len.max(1))) as u32
    };
    (x0 < x1).then(|| (centre(x0, x1, img.width), centre(y0, y1, img.height)))
}

/// A glass pixel with a channel above this is lit: the darkest part of the
/// card is 0x20.
const LIT_MIN_LEVEL: u8 = 16;

/// The width of a frame pixel over its height that `console` gives a
/// frame `width` pixels wide (both fields of an interlaced frame when
/// `interlaced`) on the screen of `glass`, with its dot clock. A pixel is
/// the time of one dot of the console over the visible line of the tube,
/// against one line of the console over the visible field of the tube.
/// A TV and a 15 kHz tube have a fixed window; a PC monitor and a panel
/// have none, so they take the window of an NTSC TV: the shape the
/// console had on the TV it was made for.
pub fn console_pixel_aspect(
    console: &Console,
    width: u16,
    interlaced: bool,
    glass: &Glass,
) -> Option<(u32, f64)> {
    let tube = match glass {
        Glass::Tube(t) if t.class != TubeClass::PerMode => *t,
        _ => Tube::tv(TvStandard::Ntsc, 0),
    };
    let pal = matches!(tube.class, TubeClass::Tv(s) if s.lines() == 625);
    let clock = console.dot_clock_hz(width, pal)?;
    let line_ns = 1e12 / console.line_rate_mhz(pal) as f64;
    let dot = 1e9 / f64::from(clock) / tube.h_len_ns * f64::from(TUBE_ASPECT.0);
    let fields = if interlaced { 2.0 } else { 1.0 };
    let line = line_ns / fields / tube.v_len_ns * f64::from(TUBE_ASPECT.1);
    Some((clock, dot / line))
}

impl GlassShape {
    /// The error against the reference that judges the case: the console
    /// when the table has its clock, else the core.
    pub fn verdict_error_permille(&self) -> i64 {
        self.console
            .map_or(self.core_error_permille, |c| c.error_permille)
    }

    /// The name of that reference: `console` or `core 4:3`.
    pub fn verdict_reference(&self) -> &'static str {
        if self.console.is_some() {
            "console"
        } else {
            "core 4:3"
        }
    }

    /// The shape as JSON.
    pub fn to_json(&self) -> Json {
        object([
            ("shown_milli", self.shown_milli.into()),
            ("core_milli", self.core_milli.into()),
            ("core_error_permille", self.core_error_permille.into()),
            (
                "console",
                self.console
                    .map(|c| {
                        object([
                            ("name", c.name.into()),
                            ("dot_clock_hz", c.dot_clock_hz.into()),
                            ("milli", c.milli.into()),
                            ("error_permille", c.error_permille.into()),
                        ])
                    })
                    .into(),
            ),
            ("border_error_permille", self.border_error_permille.into()),
        ])
    }
}

fn yellow(px: [u8; 3]) -> f64 {
    let [r, g, b] = px.map(f64::from);
    let (red, green) = (r - b, g - b);
    let (low, high) = CIRCLE_GREEN_OVER_RED;
    if red >= CIRCLE_MIN_LEVEL && green >= low * red && green <= high * red {
        green
    } else {
        0.0
    }
}

fn red(px: [u8; 3]) -> bool {
    let [r, g, b] = px.map(f64::from);
    r - g.max(b) >= BORDER_MIN_LEVEL && (g - b).abs() < BORDER_MIN_LEVEL
}

/// The width and the height of the circle, from the centre of its line at
/// the left to the centre at the right, near the axis, and the same down.
fn circle(img: &Rgb) -> Option<(f64, f64)> {
    let (mut x0, mut x1, mut y0, mut y1) = (u32::MAX, 0, u32::MAX, 0);
    for y in 0..img.height {
        for x in 0..img.width {
            if yellow(img.get(x, y)) > 0.0 {
                x0 = x0.min(x);
                x1 = x1.max(x);
                y0 = y0.min(y);
                y1 = y1.max(y);
            }
        }
    }
    if x0 >= x1 || y0 >= y1 {
        return None;
    }
    let (cx, cy) = (f64::from(x0 + x1) / 2.0, f64::from(y0 + y1) / 2.0);
    let (rx, ry) = (f64::from(x1 - x0) / 2.0, f64::from(y1 - y0) / 2.0);
    let rows = band(cy, ry, img.height);
    let width = rows
        .filter_map(|y| span((0..img.width).map(|x| yellow(img.get(x, y))), cx))
        .fold(None, max);
    let columns = band(cx, rx, img.width);
    let height = columns
        .filter_map(|x| span((0..img.height).map(|y| yellow(img.get(x, y))), cy))
        .fold(None, max);
    width.zip(height)
}

fn max(a: Option<f64>, b: f64) -> Option<f64> {
    Some(a.map_or(b, |a| a.max(b)))
}

/// The lines within [`AXIS_BAND`] of the radius around `centre`.
fn band(centre: f64, radius: f64, len: u32) -> impl Iterator<Item = u32> {
    let reach = (radius * AXIS_BAND).max(1.0);
    let first = (centre - reach).floor().max(0.0) as u32;
    let last = ((centre + reach).ceil() as u32).min(len.saturating_sub(1));
    first..=last
}

/// The distance between the weighted centres of the first run of light
/// before `centre` and the last run after it, along one line of weights.
fn span(weights: impl Iterator<Item = f64>, centre: f64) -> Option<f64> {
    let w: Vec<f64> = weights.collect();
    let runs = runs(&w);
    let left = runs.iter().find(|r| r.1 < centre)?;
    let right = runs.iter().rev().find(|r| r.1 > centre)?;
    (right.1 > left.1).then_some(right.1 - left.1)
}

/// The runs of weight above 0: their total weight and their weighted
/// centre (a pixel's centre is at its index plus one half).
fn runs(w: &[f64]) -> Vec<(f64, f64)> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < w.len() {
        if w[i] <= 0.0 {
            i += 1;
            continue;
        }
        let (mut sum, mut moment) = (0.0, 0.0);
        while i < w.len() && w[i] > 0.0 {
            sum += w[i];
            moment += w[i] * (i as f64 + 0.5);
            i += 1;
        }
        out.push((sum, moment / sum));
    }
    out
}

/// The width and the height of the outer border box, centre of line to
/// centre of line, when its four sides show.
/// A side on the edge of `img` may be cut in part, so it does not count.
fn border_box(img: &Rgb) -> Option<(f64, f64)> {
    let (rows, columns) = border_sides(img);
    let inside = |c: f64, len: u32| c > 1.0 && c < f64::from(len) - 1.0;
    let (top, bottom) = (rows.0?, rows.1?);
    let (left, right) = (columns.0?, columns.1?);
    let all_inside = inside(top, img.height)
        && inside(bottom, img.height)
        && inside(left, img.width)
        && inside(right, img.width);
    all_inside.then_some((right - left, bottom - top))
}

type Sides = (Option<f64>, Option<f64>);

/// The centres of the first and the last side of the border box, as rows
/// and as columns: a side is a group of neighbour rows (columns) whose red
/// covers [`BORDER_SIDE_SHARE`] of the red span across them.
fn border_sides(img: &Rgb) -> (Sides, Sides) {
    let mut row_red = vec![0.0; img.height as usize];
    let mut column_red = vec![0.0; img.width as usize];
    for y in 0..img.height {
        for x in 0..img.width {
            if red(img.get(x, y)) {
                row_red[y as usize] += 1.0;
                column_red[x as usize] += 1.0;
            }
        }
    }
    let (row_span, column_span) = (extent(&row_red), extent(&column_red));
    (sides(&row_red, column_span), sides(&column_red, row_span))
}

/// The count of entries from the first above 0 to the last.
fn extent(counts: &[f64]) -> f64 {
    let first = counts.iter().position(|&c| c > 0.0);
    let last = counts.iter().rposition(|&c| c > 0.0);
    match (first, last) {
        (Some(a), Some(b)) => (b - a + 1) as f64,
        _ => 0.0,
    }
}

fn sides(red: &[f64], span: f64) -> Sides {
    if span <= 0.0 {
        return (None, None);
    }
    let strong: Vec<f64> = red
        .iter()
        .map(|&r| {
            if r >= BORDER_SIDE_SHARE * span {
                r
            } else {
                0.0
            }
        })
        .collect();
    let groups = runs(&strong);
    let half = red.len() as f64 / 2.0;
    let first = groups.first().map(|g| g.1).filter(|&c| c < half);
    let last = groups.last().map(|g| g.1).filter(|&c| c > half);
    (first, last)
}

#[cfg(test)]
mod tests {
    use super::super::pattern::test_pattern;
    use super::*;

    /// `img` in the middle of a black picture 8 pixels larger at each
    /// side, as a picture lies on the glass.
    fn padded(img: &Rgb) -> Rgb {
        let mut out = Rgb::new(img.width + 16, img.height + 16, [0; 3]);
        out.paste(img, 8, 8);
        out
    }

    /// On the card itself the circle is round in display units: 320x240
    /// at 4:3 has square pixels, and the circle is 0.8 of the height.
    #[test]
    fn the_card_has_a_round_circle_and_a_full_border() {
        let card = padded(&test_pattern(320, 240, 1_333));
        let size = card_size(&card, (1.0, 1.0)).unwrap();
        assert!((size.circle_w - 192.0).abs() <= 1.0, "{size:?}");
        assert!((size.circle_h - 192.0).abs() <= 1.0, "{size:?}");
        assert_eq!(size.border, Some((319.0, 239.0)));
        assert_eq!(
            border_seen(&card),
            Edges {
                top: true,
                bottom: true,
                left: true,
                right: true
            }
        );
    }

    /// A picture with its top 8 lines cut has no top side.
    #[test]
    fn a_cut_top_shows_no_top_side() {
        let card = test_pattern(320, 240, 1_333);
        let cut = padded(&card.crop(0, 8, 320, 232));
        let seen = border_seen(&cut);
        assert!(!seen.top && seen.bottom && seen.left && seen.right);
        assert_eq!(card_size(&cut, (1.0, 1.0)).unwrap().border, None);
        let raw = test_pattern(320, 240, 1_333);
        assert!(
            border_seen(&raw).top,
            "a side on the edge of a frame buffer"
        );
        assert_eq!(card_size(&raw, (1.0, 1.0)).unwrap().border, None);
    }

    /// The circle of a picture squeezed to half its width, each pixel the
    /// mean of two, is half as wide.
    #[test]
    fn a_squeezed_circle_is_narrower() {
        let card = test_pattern(320, 240, 1_333);
        let mut squeezed = Rgb::new(160, 240, [0; 3]);
        for y in 0..240 {
            for x in 0..160 {
                let (a, b) = (card.get(2 * x, y), card.get(2 * x + 1, y));
                let mean = |i: usize| ((u16::from(a[i]) + u16::from(b[i])) / 2) as u8;
                squeezed.put(x, y, [mean(0), mean(1), mean(2)]);
            }
        }
        let size = card_size(&padded(&squeezed), (1.0, 1.0)).unwrap();
        assert!((size.circle_w - 96.0).abs() <= 1.0, "{size:?}");
        assert!((size.circle_h - 192.0).abs() <= 1.0, "{size:?}");
    }

    /// On an NTSC TV the dot clocks give the pixel aspects that the
    /// sources name: 8:7 for the SNES, 16:35 for a PlayStation at 640, and
    /// twice that for a frame line of an interlaced frame.
    #[test]
    fn the_console_pixel_aspect_on_a_tv_is_the_known_one() {
        use super::super::consoles::{PLAYSTATION, SNES};
        let ntsc = Glass::Tube(Tube::tv(TvStandard::Ntsc, 50));
        let (clock, snes) = console_pixel_aspect(&SNES, 256, false, &ntsc).unwrap();
        assert_eq!(clock, 5_369_318);
        assert!((snes - 8.0 / 7.0).abs() < 2e-3, "{snes}");
        let (_, psx) = console_pixel_aspect(&PLAYSTATION, 640, true, &ntsc).unwrap();
        assert!((psx - 32.0 / 35.0).abs() < 2e-3, "{psx}");
        let pal = Glass::Tube(Tube::tv(TvStandard::Pal, 50));
        let (clock, snes) = console_pixel_aspect(&SNES, 256, false, &pal).unwrap();
        assert_eq!(clock, 5_320_343);
        // 288 lines in the 4:3 height and 5.32 MHz: 1.384.
        assert!((snes - 1.384).abs() < 2e-3, "{snes}");
        assert_eq!(console_pixel_aspect(&SNES, 320, false, &pal), None);
    }

    /// A circle twice as wide on the glass as the core asks is an error of
    /// +100 %; the reference of the console moves with its pixel aspect.
    #[test]
    fn the_glass_shape_reads_both_references() {
        let frame = CardSize {
            circle_w: 100.0,
            circle_h: 100.0,
            border: None,
        };
        let expected = Expected {
            frame,
            frame_size: (320, 240),
            across: false,
            core_pixel_aspect: 1.0,
            console: Some(("x", 1, 0.5)),
        };
        let glass = CardSize {
            circle_w: 200.0,
            circle_h: 100.0,
            border: Some((638.0, 239.0)),
        };
        let shape = glass_shape(&glass, &expected);
        assert_eq!(shape.core_error_permille, 1000);
        assert_eq!(shape.console.unwrap().error_permille, 3000);
        assert_eq!(shape.border_error_permille, Some(1000));
        let turned = glass_shape(
            &glass,
            &Expected {
                across: true,
                ..expected
            },
        );
        assert_eq!(turned.core_error_permille, 1000);
    }

    #[test]
    fn the_lit_centre_is_the_middle_of_the_lit_box() {
        let mut img = Rgb::new(100, 50, [0; 3]);
        assert_eq!(lit_centre(&img), None);
        for y in 10..20 {
            for x in 20..60 {
                img.put(x, y, [0x20; 3]);
            }
        }
        assert_eq!(lit_centre(&img), Some((400, 300)));
    }

    #[test]
    fn a_picture_with_no_circle_has_no_card_size() {
        assert_eq!(card_size(&Rgb::new(8, 8, [0; 3]), (1.0, 1.0)), None);
    }
}
