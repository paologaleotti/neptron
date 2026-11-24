//! Stage B, the glass: a simple model of what the screen shows of the
//! frame buffer of a mode.
//!
//! A tube (a CRT or a TV) shows a fixed window of the scan: a part of each
//! line, in time after the end of the horizontal sync, and a part of each
//! field, in time after the end of the vertical sync. The window is the
//! active area of the standard mode of the monitor class, so that mode
//! fills the screen. Each line of the mode is a band as high as the beam
//! spot; where the lines are further apart than the spot, a dark gap
//! shows. A flat panel shows its frame buffer pixel for pixel.

use neptron_modeline::policy::{TV_525_PICTURE_LINES, TV_625_PICTURE_LINES, TV_SAFE_WIDTH_SAMPLES};
use neptron_types::{
    CRT_15KHZ_CLASS_MAX_LINE_RATE_MHZ, CrtRange, Modeline, MonitorKind, MonitorProfile,
    TV_PCLOCK_HZ, TvStandard,
};

use super::image::Rgb;

/// The overscan of a consumer TV, in per mille of the picture at each
/// side: the part of the 4:3 picture that the bezel hides.
pub const TV_OVERSCAN_PERMILLE: u32 = 50;
/// The field rate that a 15 kHz tube is set up for, in millihertz: the
/// vertical size of the 60 Hz 240-line picture fills the screen. A slower
/// field scans longer, so its picture is taller.
pub const TUBE_15KHZ_FIELD_MHZ: u64 = 60_000;
/// The aspect of the glass of a tube, width to height.
pub const TUBE_ASPECT: (u32, u32) = (4, 3);
/// The size of the glass picture, in pixels.
pub const GLASS_SIZE: (u32, u32) = (1440, 1080);

/// The named model of the part of the scan that a tube shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TubeClass {
    /// A TV of this standard: the 4:3 picture of the line system (704
    /// samples of the 720 at 13.5 MHz, 240 or 288 lines per field) less
    /// the overscan.
    Tv(TvStandard),
    /// An arcade or a consumer 15 kHz monitor, set up once for a 60 Hz
    /// picture with the porches of its range: the window holds at every
    /// field rate, so a 50 Hz picture is taller.
    Fixed15khz,
    /// A PC or multisync CRT in a range at 20 kHz or more: the user or the
    /// monitor sets the size of each mode, so the window is the field of
    /// the mode less the vertical blank of its range.
    PerMode,
}

/// A tube for one mode: where the visible window lies in the scan, and the
/// height of the beam spot.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tube {
    /// The class model.
    pub class: TubeClass,
    /// The start of the visible part of a line, in ns after the end of the
    /// horizontal sync.
    pub h_start_ns: f64,
    /// The visible part of a line, in ns.
    pub h_len_ns: f64,
    /// The start of the visible part of a field, in ns after the end of the
    /// vertical sync.
    pub v_start_ns: f64,
    /// The visible part of a field, in ns.
    pub v_len_ns: f64,
    /// The lines of the densest raster of the tube over the visible height:
    /// the beam spot is as high as one of them.
    pub spot_lines: f64,
    /// The part that the bezel hides at each side, in per mille.
    pub overscan_permille: u32,
}

/// What the screen is to the glass model.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Glass {
    /// A CRT or a TV.
    Tube(Tube),
    /// A flat panel of this native size.
    Panel {
        /// Width, in pixels.
        width: u32,
        /// Height, in pixels.
        height: u32,
    },
}

/// One band of light on the glass: a line of the mode, in fractions of
/// the visible height.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Band {
    /// The row of the frame buffer.
    pub row: u32,
    /// The top edge.
    pub top: f64,
    /// The bottom edge.
    pub bottom: f64,
}

impl Glass {
    /// The glass of `profile` in `mode`. `overscan_permille` replaces the
    /// overscan of the class (TV: [`TV_OVERSCAN_PERMILLE`], other: 0).
    pub fn of(profile: &MonitorProfile, mode: &Modeline, overscan_permille: Option<u32>) -> Self {
        match &profile.kind {
            MonitorKind::FlatPanel { .. } => Self::Panel {
                width: u32::from(mode.hactive),
                height: u32::from(mode.vactive),
            },
            MonitorKind::Tv { standard } => Self::Tube(Tube::tv(
                *standard,
                overscan_permille.unwrap_or(TV_OVERSCAN_PERMILLE),
            )),
            MonitorKind::Crt { ranges } => {
                Self::Tube(Tube::crt(ranges, mode, overscan_permille.unwrap_or(0)))
            }
        }
    }

    /// The left edge of column edge `column` of `mode`, in fractions of
    /// the visible width: 0 is the left edge of the glass, 1 the right.
    pub fn x(&self, mode: &Modeline, column: f64) -> f64 {
        match self {
            Self::Panel { width, .. } => column / f64::from((*width).max(1)),
            Self::Tube(tube) => tube.x(mode, column),
        }
    }

    /// The top of row edge `row` of `mode`, in fractions of the visible
    /// height: 0 is the top of the glass, 1 the bottom. A row of an
    /// interlaced mode is half a field line: the two fields weave.
    pub fn y(&self, mode: &Modeline, row: f64) -> f64 {
        match self {
            Self::Panel { height, .. } => row / f64::from((*height).max(1)),
            Self::Tube(tube) => tube.y(mode, row),
        }
    }

    /// The glass with no overscan: the window of the scan alone.
    pub fn without_overscan(&self) -> Self {
        match self {
            Self::Tube(tube) => Self::Tube(Tube {
                overscan_permille: 0,
                ..*tube
            }),
            Self::Panel { .. } => *self,
        }
    }

    /// The width to height of the visible screen: 4:3 for a tube, the
    /// native size of a panel.
    pub fn aspect(&self) -> (u32, u32) {
        match self {
            Self::Tube(_) => TUBE_ASPECT,
            Self::Panel { width, height } => (*width, *height),
        }
    }

    /// The size of the glass picture in the box `size`: the whole box for
    /// a tube, the largest size of the panel's aspect inside it for a
    /// panel.
    pub fn picture_size(&self, size: (u32, u32)) -> (u32, u32) {
        let (aw, ah) = self.aspect();
        let (w, h) = size;
        if u64::from(aw) * u64::from(h) >= u64::from(ah) * u64::from(w) {
            (
                w,
                (u64::from(w) * u64::from(ah) / u64::from(aw.max(1))) as u32,
            )
        } else {
            (
                (u64::from(h) * u64::from(aw) / u64::from(ah.max(1))) as u32,
                h,
            )
        }
    }

    /// The bands of light of every row of `mode`. A doublescan row is two
    /// bands. A band is as high as the beam spot, at most as high as the
    /// distance to the next line.
    pub fn bands(&self, mode: &Modeline) -> Vec<Band> {
        let scans = if mode.doublescan { 2 } else { 1 };
        let spot = match self {
            Self::Panel { .. } => f64::INFINITY,
            Self::Tube(tube) => tube.spot(),
        };
        let mut out = Vec::with_capacity(usize::from(mode.vactive) * scans);
        for row in 0..u32::from(mode.vactive) {
            for scan in 0..scans {
                let k = scans as f64;
                let top = self.y(mode, f64::from(row) + scan as f64 / k);
                let bottom = self.y(mode, f64::from(row) + (scan as f64 + 1.0) / k);
                let height = (bottom - top).min(spot);
                let centre = (top + bottom) / 2.0;
                out.push(Band {
                    row,
                    top: centre - height / 2.0,
                    bottom: centre + height / 2.0,
                });
            }
        }
        out
    }

    /// The glass picture of `frame_buffer` in `mode`, `size` pixels: each
    /// glass pixel is the mean of the light that falls on it.
    pub fn render(&self, mode: &Modeline, frame_buffer: &Rgb, size: (u32, u32)) -> Rgb {
        let (w, h) = size;
        let rows = row_weights(&self.bands(mode), h);
        let columns = column_weights(self, mode, u32::from(mode.hactive), w);
        let mut out = Rgb::new(w, h, [0; 3]);
        for (y, row_list) in rows.iter().enumerate() {
            for (x, column_list) in columns.iter().enumerate() {
                let mut sum = [0.0_f64; 3];
                for &(row, wr) in row_list {
                    for &(column, wc) in column_list {
                        let c = frame_buffer.get(column, row);
                        for i in 0..3 {
                            sum[i] += wr * wc * f64::from(c[i]);
                        }
                    }
                }
                out.put(
                    x as u32,
                    y as u32,
                    sum.map(|v| v.round().clamp(0.0, 255.0) as u8),
                );
            }
        }
        out
    }
}

impl Tube {
    /// The TV of `standard`: the 4:3 picture of its progressive mode,
    /// [`TV_SAFE_WIDTH_SAMPLES`] wide and 240 or 288 lines high, less
    /// `overscan_permille` at each side. The spot is one line of the
    /// interlaced picture.
    pub fn tv(standard: TvStandard, overscan_permille: u32) -> Self {
        let reference = standard.modes()[0];
        let lines = if standard.lines() == 525 {
            TV_525_PICTURE_LINES
        } else {
            TV_625_PICTURE_LINES
        };
        let sample_ns = 1e9 / f64::from(TV_PCLOCK_HZ);
        let line_ns = f64::from(reference.htotal) * sample_ns;
        let side = f64::from(reference.hactive.saturating_sub(TV_SAFE_WIDTH_SAMPLES)) / 2.0;
        let back = f64::from(reference.htotal - reference.hsync_end);
        Self {
            class: TubeClass::Tv(standard),
            h_start_ns: (back + side) * sample_ns,
            h_len_ns: f64::from(TV_SAFE_WIDTH_SAMPLES) * sample_ns,
            v_start_ns: f64::from(reference.vtotal - reference.vsync_end) * line_ns,
            v_len_ns: f64::from(lines) * line_ns,
            spot_lines: 2.0 * f64::from(lines),
            overscan_permille,
        }
    }

    /// The CRT with `ranges` in `mode`: the window of the range that holds
    /// the mode (or the range nearest its line rate).
    pub fn crt(ranges: &[CrtRange], mode: &Modeline, overscan_permille: u32) -> Self {
        let line_mhz = mode.line_rate_mhz();
        let field_mhz = mode.field_rate_mhz();
        let fallback = CrtRange {
            h_min_mhz: line_mhz,
            h_max_mhz: line_mhz,
            v_min_mhz: field_mhz,
            v_max_mhz: field_mhz,
            h_front_porch_ns: 0,
            h_sync_ns: 0,
            h_back_porch_ns: 0,
            v_front_porch_ns: 0,
            v_sync_ns: 0,
            v_back_porch_ns: 0,
            hsync_positive: false,
            vsync_positive: false,
            progressive_lines_min: 0,
            progressive_lines_max: 0,
            interlaced_lines_min: 0,
            interlaced_lines_max: 0,
        };
        let range = holding_range(ranges, line_mhz, field_mhz).unwrap_or(&fallback);
        let low = range.h_max_mhz < CRT_15KHZ_CLASS_MAX_LINE_RATE_MHZ;
        let (class, field_ref_mhz) = if low {
            (TubeClass::Fixed15khz, reference_field_mhz(range))
        } else {
            (TubeClass::PerMode, field_mhz.max(1))
        };
        let line_ns = 1e12 / line_mhz.max(1) as f64;
        let h_blank = f64::from(range.h_front_porch_ns + range.h_sync_ns + range.h_back_porch_ns);
        let field_ns = 1e12 / field_ref_mhz as f64;
        let spot_lines = ranges
            .iter()
            .map(raster_lines)
            .fold(0.0_f64, f64::max)
            .max(1.0);
        Self {
            class,
            h_start_ns: f64::from(range.h_back_porch_ns),
            h_len_ns: (line_ns - h_blank).max(1.0),
            v_start_ns: f64::from(range.v_back_porch_ns),
            v_len_ns: (field_ns - v_blank_ns(range)).max(1.0),
            spot_lines,
            overscan_permille,
        }
    }

    fn overscan(&self) -> f64 {
        f64::from(self.overscan_permille.min(499)) / 1000.0
    }

    /// A fraction of the window to a fraction of the glass.
    fn after_overscan(&self, f: f64) -> f64 {
        let o = self.overscan();
        (f - o) / (1.0 - 2.0 * o)
    }

    fn x(&self, mode: &Modeline, column: f64) -> f64 {
        let pixel_ns = 1e9 / f64::from(mode.pclock_hz.max(1));
        let t = (f64::from(mode.htotal - mode.hsync_end) + column) * pixel_ns;
        self.after_overscan((t - self.h_start_ns) / self.h_len_ns)
    }

    fn y(&self, mode: &Modeline, row: f64) -> f64 {
        let line_ns = 1e12 / mode.line_rate_mhz().max(1) as f64;
        let back = f64::from(mode.vtotal - mode.vsync_end);
        // Scan lines after the end of the vertical sync.
        let scan = if mode.interlace {
            (back + row) / 2.0
        } else if mode.doublescan {
            2.0 * (back + row)
        } else {
            back + row
        };
        self.after_overscan((scan * line_ns - self.v_start_ns) / self.v_len_ns)
    }

    /// The spot height, in fractions of the glass height.
    fn spot(&self) -> f64 {
        1.0 / self.spot_lines / (1.0 - 2.0 * self.overscan())
    }
}

/// The range that holds a mode at these rates: its line rate inside the
/// range, from one field rate below the least (the tolerance of the
/// validator for a whole-kHz clock), and its field rate inside. Else the
/// range nearest its line rate.
fn holding_range(ranges: &[CrtRange], line_mhz: u64, field_mhz: u64) -> Option<&CrtRange> {
    ranges
        .iter()
        .find(|r| {
            (r.h_min_mhz.saturating_sub(field_mhz)..=r.h_max_mhz).contains(&line_mhz)
                && (r.v_min_mhz..=r.v_max_mhz).contains(&field_mhz)
        })
        .or_else(|| {
            ranges.iter().min_by_key(|r| {
                let mid = (r.h_min_mhz + r.h_max_mhz) / 2;
                mid.abs_diff(line_mhz)
            })
        })
}

/// [`TUBE_15KHZ_FIELD_MHZ`] inside the field rates of `range`.
fn reference_field_mhz(range: &CrtRange) -> u64 {
    TUBE_15KHZ_FIELD_MHZ
        .clamp(range.v_min_mhz, range.v_max_mhz.max(range.v_min_mhz))
        .max(1)
}

fn v_blank_ns(range: &CrtRange) -> f64 {
    f64::from(range.v_front_porch_ns + range.v_sync_ns + range.v_back_porch_ns)
}

/// The visible lines of the densest raster of a range: at its highest
/// line rate and its reference field rate, twice that below 20 kHz, where
/// the spot is sized for the interlaced picture of the broadcast standard.
fn raster_lines(range: &CrtRange) -> f64 {
    let field_ns = 1e12 / reference_field_mhz(range) as f64;
    let visible_s = (field_ns - v_blank_ns(range)).max(0.0) / 1e9;
    let lines = range.h_max_mhz as f64 / 1000.0 * visible_s;
    if range.h_max_mhz < CRT_15KHZ_CLASS_MAX_LINE_RATE_MHZ {
        2.0 * lines
    } else {
        lines
    }
}

/// For each glass row of `height`, the bands that fall on it and the share
/// of the row each one covers.
fn row_weights(bands: &[Band], height: u32) -> Vec<Vec<(u32, f64)>> {
    let mut rows = vec![Vec::new(); height as usize];
    let h = f64::from(height);
    for band in bands {
        let (top, bottom) = (band.top.max(0.0), band.bottom.min(1.0));
        if bottom <= top {
            continue;
        }
        let first = (top * h).floor() as u32;
        let last = ((bottom * h).ceil() as u32).min(height);
        for y in first..last {
            let (y0, y1) = (f64::from(y) / h, f64::from(y + 1) / h);
            let cover = (bottom.min(y1) - top.max(y0)) * h;
            if cover > 0.0 {
                rows[y as usize].push((band.row, cover));
            }
        }
    }
    rows
}

/// For each glass column of `width`, the columns of the frame buffer that
/// fall on it and the share of the column each one covers.
fn column_weights(
    glass: &Glass,
    mode: &Modeline,
    columns: u32,
    width: u32,
) -> Vec<Vec<(u32, f64)>> {
    let w = f64::from(width);
    let x0 = glass.x(mode, 0.0);
    let per_column = glass.x(mode, 1.0) - x0;
    (0..width)
        .map(|x| {
            let (g0, g1) = (f64::from(x) / w, f64::from(x + 1) / w);
            if per_column <= 0.0 {
                return Vec::new();
            }
            let c0 = ((g0 - x0) / per_column).floor().max(0.0) as u32;
            let c1 = (((g1 - x0) / per_column).ceil().max(0.0) as u32).min(columns);
            (c0..c1)
                .filter_map(|c| {
                    let (a, b) = (
                        x0 + f64::from(c) * per_column,
                        x0 + f64::from(c + 1) * per_column,
                    );
                    let cover = (b.min(g1) - a.max(g0)) * w;
                    (cover > 0.0).then_some((c, cover))
                })
                .collect()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use neptron_types::{TV_525_240P, TV_525_480I};

    use super::*;

    /// A 15 kHz range whose standard mode is 320x240 at 60 Hz with a
    /// 6.4 MHz clock and 400 x 262 totals: 40 px of horizontal blank and
    /// 22 lines of vertical blank.
    fn range_320() -> CrtRange {
        let pixel_ns = 1e9 / 6_400_000.0;
        let line_ns = 400.0 * pixel_ns;
        CrtRange {
            h_min_mhz: 15_000_000,
            h_max_mhz: 16_500_000,
            v_min_mhz: 50_000,
            v_max_mhz: 65_000,
            h_front_porch_ns: (10.0 * pixel_ns) as u32,
            h_sync_ns: (40.0 * pixel_ns) as u32,
            h_back_porch_ns: (30.0 * pixel_ns) as u32,
            v_front_porch_ns: (2.0 * line_ns) as u32,
            v_sync_ns: (3.0 * line_ns) as u32,
            v_back_porch_ns: (17.0 * line_ns) as u32,
            hsync_positive: false,
            vsync_positive: false,
            progressive_lines_min: 192,
            progressive_lines_max: 288,
            interlaced_lines_min: 448,
            interlaced_lines_max: 576,
        }
    }

    /// 320x240 with the porches of [`range_320`], at 6.4 MHz / 400 / 266
    /// = 60.15 Hz.
    const MODE_320: Modeline = Modeline {
        pclock_hz: 6_400_000,
        hactive: 320,
        hsync_start: 330,
        hsync_end: 370,
        htotal: 400,
        vactive: 240,
        vsync_start: 242,
        vsync_end: 245,
        vtotal: 262,
        interlace: false,
        doublescan: false,
        hsync_positive: false,
        vsync_positive: false,
    };

    fn crt_320() -> Glass {
        let profile = MonitorProfile {
            id: "t".into(),
            kind: MonitorKind::Crt {
                ranges: vec![range_320()],
            },
        };
        Glass::of(&profile, &MODE_320, None)
    }

    fn close(a: f64, b: f64, tolerance: f64) -> bool {
        (a - b).abs() <= tolerance
    }

    #[test]
    fn the_standard_mode_of_the_range_fills_the_width() {
        let glass = crt_320();
        assert!(close(glass.x(&MODE_320, 0.0), 0.0, 0.002));
        assert!(close(glass.x(&MODE_320, 320.0), 1.0, 0.002));
        assert!(close(glass.x(&MODE_320, 160.0), 0.5, 0.002));
    }

    #[test]
    fn the_standard_mode_fills_the_height_at_60_hz() {
        let glass = crt_320();
        // The window is 16.67 ms less 22 lines; 240 lines of 62.5 us are
        // 15.0 ms, the window 15.29 ms: the picture fills 98 %.
        let top = glass.y(&MODE_320, 0.0);
        let bottom = glass.y(&MODE_320, 240.0);
        assert!(close(top, 0.0, 0.01), "{top}");
        assert!(close(bottom - top, 0.981, 0.01), "{}", bottom - top);
    }

    #[test]
    fn a_240_line_mode_on_a_15_khz_tube_shows_gaps() {
        let glass = crt_320();
        let bands = glass.bands(&MODE_320);
        assert_eq!(bands.len(), 240);
        let pitch = bands[1].top - bands[0].top;
        let height = bands[0].bottom - bands[0].top;
        assert!(height < 0.6 * pitch, "{height} {pitch}");
    }

    #[test]
    fn interlaced_lines_weave_with_no_gap() {
        let tv = Glass::Tube(Tube::tv(TvStandard::Ntsc, 0));
        let bands = tv.bands(&TV_525_480I);
        assert_eq!(bands.len(), 480);
        let pitch = bands[1].top - bands[0].top;
        assert!(close(pitch, 1.0 / 480.0, 0.0005), "{pitch}");
        let height = bands[0].bottom - bands[0].top;
        assert!(close(height, pitch, 1e-9), "{height} {pitch}");
    }

    #[test]
    fn the_tv_picture_is_704_samples_wide_and_240_lines_high() {
        let tv = Glass::Tube(Tube::tv(TvStandard::Ntsc, 0));
        assert!(close(tv.x(&TV_525_240P, 8.0), 0.0, 1e-6));
        assert!(close(tv.x(&TV_525_240P, 712.0), 1.0, 1e-6));
        assert!(close(tv.y(&TV_525_240P, 0.0), 0.0, 1e-6));
        assert!(close(tv.y(&TV_525_240P, 240.0), 1.0, 1e-6));
    }

    #[test]
    fn an_overscan_of_5_percent_hides_the_right_samples() {
        let tv = Glass::Tube(Tube::tv(TvStandard::Ntsc, 50));
        // 5 % of 704 is 35.2 samples at each side: sample 8 + 35.2 is the
        // left edge, sample 712 - 35.2 the right edge.
        assert!(close(tv.x(&TV_525_240P, 43.2), 0.0, 1e-6));
        assert!(close(tv.x(&TV_525_240P, 676.8), 1.0, 1e-6));
        // 12 lines at the top and the bottom.
        assert!(close(tv.y(&TV_525_240P, 12.0), 0.0, 1e-6));
        assert!(close(tv.y(&TV_525_240P, 228.0), 1.0, 1e-6));
    }

    #[test]
    fn a_panel_maps_pixel_for_pixel() {
        let panel = Glass::Panel {
            width: 1920,
            height: 1080,
        };
        let mode = super::super::case::panel_mode(1920, 1080, 60_000);
        assert!(close(panel.x(&mode, 960.0), 0.5, 1e-9));
        assert!(close(panel.y(&mode, 540.0), 0.5, 1e-9));
        assert_eq!(panel.picture_size(GLASS_SIZE), (1440, 810));
        assert_eq!(crt_320().picture_size(GLASS_SIZE), GLASS_SIZE);
    }

    #[test]
    fn a_rendered_panel_is_the_mean_of_the_pixels() {
        let panel = Glass::Panel {
            width: 2,
            height: 2,
        };
        let mode = super::super::case::panel_mode(2, 2, 60_000);
        let fb = Rgb::from_rgb(
            2,
            2,
            vec![0, 0, 0, 200, 200, 200, 100, 100, 100, 100, 100, 100],
        )
        .unwrap();
        let one = panel.render(&mode, &fb, (1, 1));
        assert_eq!(one.get(0, 0), [100; 3]);
        let same = panel.render(&mode, &fb, (2, 2));
        assert_eq!(same, fb);
    }

    #[test]
    fn a_rendered_tube_has_dark_gaps_between_the_lines_of_240p() {
        let glass = crt_320();
        let fb = Rgb::new(320, 240, [200; 3]);
        let img = glass.render(&MODE_320, &fb, (320, 1080));
        let column: Vec<u8> = (100..120).map(|y| img.get(160, y)[0]).collect();
        assert!(column.iter().any(|&v| v >= 190), "{column:?}");
        assert!(column.iter().any(|&v| v <= 20), "{column:?}");
    }
}
