//! The measures of a case: the mode, the place of the picture, the scale,
//! what is cut and what is black, how much of the screen the picture
//! fills, its shape and its speed. Pure.

use neptron_types::{Orientation, Rect, Rotation, ScanlineKind, SourceGeometry};
use neptron_video::{
    Notice, Rung, TV_525_CONSOLE_WIDTH_SAMPLES, TV_525_PICTURE_LINES, TV_625_CONSOLE_WIDTH_SAMPLES,
    TV_CONSOLE_LINES_MIN,
};

use super::card::GlassShape;
use super::fields::{self, Fields};
use super::glass::{Glass, Tube, TubeClass};
use super::image::Rgb;
use super::json::{Json, object};
use super::pattern::CORNERS;
use super::wire::{Shown, Wire};

/// A value at each edge of a picture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Edges<T> {
    /// The top edge.
    pub top: T,
    /// The bottom edge.
    pub bottom: T,
    /// The left edge.
    pub left: T,
    /// The right edge.
    pub right: T,
}

/// The mode on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModeFacts {
    /// Active width, in pixels.
    pub hactive: u16,
    /// Active lines; both fields of an interlaced mode.
    pub vactive: u16,
    /// Total width, in pixels.
    pub htotal: u16,
    /// Total lines.
    pub vtotal: u16,
    /// Pixel clock, in hertz.
    pub pclock_hz: u32,
    /// Line rate, in millihertz.
    pub line_rate_mhz: u64,
    /// Field rate, in millihertz.
    pub field_rate_mhz: u64,
    /// Interlaced.
    pub interlace: bool,
    /// Each line sent twice.
    pub doublescan: bool,
}

/// The game picture as the player sees it, in front of a screen that
/// stands at its orientation: the measures that assert TATE mode. They
/// read the source of the core and the game orientation, not the display
/// source of the engine, so a wrong turn or a wrong aspect shows here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Player {
    /// The screen orientation, in degrees counter-clockwise.
    pub screen_degrees: u16,
    /// The width to height of the picture for the player, in thousandths:
    /// the shape in the mode, turned with the screen.
    pub aspect_milli: u32,
    /// The width to height of the upright game, in thousandths: its
    /// display aspect, the crop and the turn of the source applied.
    pub aspect_asked_milli: u32,
    /// Shown over asked, less 1, in per mille.
    pub aspect_error_permille: i64,
    /// The part of the width of the screen, for the player, that the
    /// picture fills, in per mille.
    pub fill_w_permille: u32,
    /// The part of the height of the screen, for the player, that the
    /// picture fills, in per mille.
    pub fill_h_permille: u32,
    /// The test pattern stands upright for the player: the corner mark
    /// at the top-left corner of the picture is the mark of the top-left
    /// corner of the upright game. `None` when no corner mark shows, or
    /// the frame is not the test pattern.
    pub upright: Option<bool>,
}

/// How the player stands in front of the screen, for [`measure`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct View<'a> {
    /// The screen orientation and the game override that the engine has.
    pub orientation: Orientation,
    /// The source as the core sends it, before the screen turn.
    pub core_source: &'a SourceGeometry,
    /// The frame is the built-in test pattern, so its corner marks tell
    /// whether the picture is upright.
    pub pattern: bool,
}

/// The measures of a case that shows a picture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picture {
    /// The mode the display read back.
    pub mode: ModeFacts,
    /// The rung of the winner.
    pub rung: String,
    /// The part of the frame shown, in frame pixels.
    pub crop: Rect,
    /// Where the picture goes, in mode pixels.
    pub dest: Rect,
    /// The turn of the picture.
    pub rotation: Rotation,
    /// The filter.
    pub filter: String,
    /// Mode pixels per source pixel across, in thousandths.
    pub scale_x_milli: u32,
    /// The scale across is a whole number.
    pub scale_x_whole: bool,
    /// Mode lines per source line, in thousandths.
    pub scale_y_milli: u32,
    /// The scale down is a whole number.
    pub scale_y_whole: bool,
    /// Source lines (top, bottom) and columns (left, right) of the upright
    /// picture that are not in the frame buffer: the crop of the policy
    /// and any part of the destination outside the mode.
    pub cropped: Edges<u32>,
    /// Mode pixels with no picture at each edge; negative where the
    /// destination reaches past the mode.
    pub borders: Edges<i64>,
    /// The part of the visible width the picture fills, in per mille.
    pub fill_w_permille: u32,
    /// The part of the visible height the picture fills, in per mille.
    pub fill_h_permille: u32,
    /// The width to height of the picture as the policy intends it: as it
    /// places it in the mode, with the reference area of the mode filling
    /// the screen, and on a TV against the console width of the policy:
    /// the
    /// active area of a CRT mode filling 4:3 (the usual set-up of the
    /// size controls), the 4:3 picture of a TV, square pixels on a panel.
    /// In thousandths.
    pub policy_aspect_milli: u32,
    /// The width to height the source asks for the shown part, in
    /// thousandths.
    pub aspect_asked_milli: u32,
    /// Intended over asked, less 1, in per mille: what the policy meant.
    /// It reads the rule of the policy, so it cannot prove the shape; the
    /// glass shape does.
    pub policy_aspect_error_permille: i64,
    /// The error of the shape from the geometry of the glass model, worked
    /// out from `dest` and the window of the scan, in per mille. The glass
    /// shape measures the same from the light; the two agree when the
    /// model and the picture agree.
    pub model_aspect_error_permille: i64,
    /// Source lines do not map to whole mode lines: the scale down is not
    /// a whole number, so some lines show taller than others.
    pub uneven_lines: bool,
    /// Source lines inside the crop that no screen line samples: the
    /// two-tap filter of the renderer skips them. 0 at every scale from
    /// 0.5 up with sharp bilinear, and at every scale from 1 up.
    pub dropped_lines: u32,
    /// The source lines that the notice of the policy says it crops.
    pub notice_cropped_lines: u16,
    /// A console picture on a TV: its shape is measured against the
    /// console width of the policy, not the 4:3 line.
    pub console_picture: bool,
    /// The part of the reference area of the mode that the picture covers,
    /// in per mille: its size as the policy places it, with no glass
    /// model.
    pub placed_area_permille: u32,
    /// Source lines and columns in the frame buffer that the glass does
    /// not show: outside the window of the scan, or under the bezel.
    pub glass_hidden: Edges<u32>,
    /// On a TV, the part of `glass_hidden` that the overscan alone hides.
    pub overscan_hidden: Option<Edges<u32>>,
    /// The speed of the game on the display less its own, in parts per
    /// million of its own.
    pub speed_ppm: i64,
    /// The speed alert of the status line shows.
    pub speed_alert: bool,
    /// The speed from the mode read back, not from the timing of the
    /// policy: the field rate over the whole number of fields nearest one
    /// game frame, less the rate of the game, in parts per million of it.
    pub mode_speed_ppm: i64,
    /// The scanlines the renderer draws: `none` or `black N%`.
    pub scanlines: String,
    /// The notice of the policy, one entry per part.
    pub notices: Vec<String>,
    /// The picture as the player sees it.
    pub player: Player,
    /// The sides of the outer border of the test card that show in the
    /// frame buffer; `None` when the frame is not the test card.
    pub border_on_wire: Option<Edges<bool>>,
    /// The sides of the outer border that show on the glass; `None` until
    /// the card is measured on the glass.
    pub border_on_glass: Option<Edges<bool>>,
    /// The shape of the test card on the glass against the two
    /// references; `None` until it is measured.
    pub glass_shape: Option<GlassShape>,
    /// The centre of the lit picture on the glass, across and down, in per
    /// mille of the glass, read from the light; `None` until it is
    /// measured or when nothing is lit.
    pub glass_centre_permille: Option<(u32, u32)>,
    /// The two fields of an interlaced mode; `None` for a progressive
    /// mode.
    pub fields: Option<Fields>,
}

/// The measures of one case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Measures {
    /// The display name.
    pub display: String,
    /// The class of the display: `crt`, `tv` or `panel`.
    pub class: String,
    /// The GPU entry.
    pub gpu: String,
    /// The source name.
    pub source: String,
    /// The settings label.
    pub settings: String,
    /// The answer of the menus for the settings: `offered`, or
    /// `declined: REASON`.
    pub offer: String,
    /// The measures of the picture, or `None` with no picture.
    pub picture: Option<Picture>,
    /// Why nothing shows, when nothing does.
    pub no_picture: Option<String>,
}

/// The names of a case.
#[derive(Debug, Clone, Copy)]
pub struct Names<'a> {
    /// The display.
    pub display: &'a str,
    /// The class of the display: `crt`, `tv` or `panel`.
    pub class: &'a str,
    /// The GPU entry.
    pub gpu: &'a str,
    /// The source.
    pub source: &'a str,
    /// The settings.
    pub settings: &'a str,
    /// The source is an arcade board: the policy gives it the arcade
    /// rules.
    pub arcade: bool,
}

/// The measures of `wire` for the source of `view`. `glass` is the glass
/// of the case and the same glass with no overscan; `None` with no
/// picture. The measures of the mode read the display source, the source
/// that the engine plans with; [`Player`] reads the source of the core.
pub fn measure(
    names: Names<'_>,
    view: View<'_>,
    wire: &Wire,
    glass: Option<(&Glass, &Glass)>,
) -> Measures {
    let source = &view.orientation.display_source(view.core_source);
    let offer = match &wire.offer {
        Ok(Ok(_)) => "offered".to_owned(),
        Ok(Err(declined)) => format!("declined: {:?}", declined.reason),
        Err(error) => format!("no plan: {error}"),
    };
    let picture = wire
        .shown
        .as_ref()
        .zip(glass)
        .map(|(shown, (glass, bare))| {
            let mut p = picture(source, names.arcade, shown, glass, bare);
            p.player = player(&view, shown, &p);
            p
        });
    Measures {
        display: names.display.to_owned(),
        class: names.class.to_owned(),
        gpu: names.gpu.to_owned(),
        source: names.source.to_owned(),
        settings: names.settings.to_owned(),
        offer,
        picture,
        no_picture: wire.no_picture.clone(),
    }
}

fn picture(
    source: &SourceGeometry,
    arcade: bool,
    shown: &Shown,
    glass: &Glass,
    bare: &Glass,
) -> Picture {
    let mode = &shown.mode;
    let scene = &shown.scene;
    let (tw, th) = turned(&scene.crop, scene.rotation);
    let dest = scene.dest;
    let (mw, mh) = (i64::from(mode.hactive), i64::from(mode.vactive));
    let (dx, dy, dw, dh) = (
        i64::from(dest.x),
        i64::from(dest.y),
        i64::from(dest.w),
        i64::from(dest.h),
    );
    let borders = Edges {
        top: dy,
        bottom: mh - dy - dh,
        left: dx,
        right: mw - dx - dw,
    };
    let in_frame = frame_cut(source, &scene.crop, scene.rotation);
    let past_mode = Edges {
        top: to_source(-borders.top, dh, th),
        bottom: to_source(-borders.bottom, dh, th),
        left: to_source(-borders.left, dw, tw),
        right: to_source(-borders.right, dw, tw),
    };
    let cropped = Edges {
        top: in_frame.top + past_mode.top,
        bottom: in_frame.bottom + past_mode.bottom,
        left: in_frame.left + past_mode.left,
        right: in_frame.right + past_mode.right,
    };
    // The part of the destination inside the mode, in mode pixels.
    let inside = (
        dx.clamp(0, mw) as f64,
        dy.clamp(0, mh) as f64,
        (dx + dw).clamp(0, mw) as f64,
        (dy + dh).clamp(0, mh) as f64,
    );
    let shown_lines = th.saturating_sub(past_mode.top + past_mode.bottom);
    let shown_columns = tw.saturating_sub(past_mode.left + past_mode.right);
    let hidden = |g: &Glass| {
        let (x0, x1) = (g.x(mode, inside.0), g.x(mode, inside.2));
        let (y0, y1) = (g.y(mode, inside.1), g.y(mode, inside.3));
        Edges {
            top: share(-y0, y1 - y0, shown_lines),
            bottom: share(y1 - 1.0, y1 - y0, shown_lines),
            left: share(-x0, x1 - x0, shown_columns),
            right: share(x1 - 1.0, x1 - x0, shown_columns),
        }
    };
    let glass_hidden = hidden(glass);
    let overscan_hidden = matches!(glass, Glass::Tube(t) if t.overscan_permille > 0).then(|| {
        let bare = hidden(bare);
        Edges {
            top: glass_hidden.top.saturating_sub(bare.top),
            bottom: glass_hidden.bottom.saturating_sub(bare.bottom),
            left: glass_hidden.left.saturating_sub(bare.left),
            right: glass_hidden.right.saturating_sub(bare.right),
        }
    });
    let fill = |a: f64, b: f64| ((b.min(1.0) - a.max(0.0)).max(0.0) * 1000.0).round() as u32;
    let fill_w_permille = fill(glass.x(mode, inside.0), glass.x(mode, inside.2));
    let fill_h_permille = fill(glass.y(mode, inside.1), glass.y(mode, inside.3));
    let (aw, ah) = glass.aspect();
    let width = (glass.x(mode, (dx + dw) as f64) - glass.x(mode, dx as f64)) * f64::from(aw);
    let height = (glass.y(mode, (dy + dh) as f64) - glass.y(mode, dy as f64)) * f64::from(ah);
    let glass_aspect = if height > 0.0 { width / height } else { 0.0 };
    let ((mut ref_w, mut ref_h), (sw, sh)) = glass.reference(mode);
    let cover = |placed: i64, reference: f64| (placed as f64 / reference).clamp(0.0, 1.0);
    let placed_area_permille = (cover(dw, ref_w) * cover(dh, ref_h) * 1000.0).round() as u32;
    if matches!(glass, Glass::Tube(t) if !matches!(t.class, TubeClass::Tv(_))) {
        // The size controls of a CRT are set for the whole frame of the
        // game: lines that the policy cuts would show above and below.
        ref_w += f64::from(cropped.left + cropped.right) * dw as f64 / f64::from(tw.max(1));
        ref_h += f64::from(cropped.top + cropped.bottom) * dh as f64 / f64::from(th.max(1));
    }
    let placed_w = dw as f64 / ref_w * f64::from(sw);
    let placed_h = dh as f64 / ref_h * f64::from(sh);
    let asked_aspect = asked_aspect(source, &scene.crop, scene.rotation);
    // The policy reads the lines of the whole frame. The fitted rung keeps
    // that shape and scales it with the lines it fills.
    let own_lines = upright_lines(source);
    let console = console_width(glass, own_lines, source.aspect_milli, arcade, mode);
    let shown_aspect = match console {
        // A console picture does not fill the 4:3 line: the policy gives
        // it the console width, so its shape is its width against that.
        Some(width) => {
            let reference = match shown.rung {
                Rung::Fitted => f64::from(width) * dh as f64 / f64::from(own_lines.max(1)),
                _ => f64::from(width),
            };
            dw as f64 / reference * asked_aspect
        }
        None if placed_h > 0.0 => placed_w / placed_h,
        None => 0.0,
    };
    let error = |shown: f64| {
        if asked_aspect > 0.0 {
            ((shown / asked_aspect - 1.0) * 1000.0).round() as i64
        } else {
            0
        }
    };
    let timing = &shown.proposal.timing;
    let game = i64::from(source.refresh_mhz.max(1));
    let speed_ppm = (timing.shown_mhz() as i64 - game) * 1_000_000 / game;
    let field = i64::try_from(mode.field_rate_mhz()).unwrap_or(i64::MAX);
    let repeats = ((field + game / 2) / game).max(1);
    let mode_speed_ppm = (field * 1_000_000 / repeats - game * 1_000_000) / game;
    Picture {
        mode: ModeFacts {
            hactive: mode.hactive,
            vactive: mode.vactive,
            htotal: mode.htotal,
            vtotal: mode.vtotal,
            pclock_hz: mode.pclock_hz,
            line_rate_mhz: mode.line_rate_mhz(),
            field_rate_mhz: mode.field_rate_mhz(),
            interlace: mode.interlace,
            doublescan: mode.doublescan,
        },
        rung: shown.rung.to_string(),
        crop: scene.crop,
        dest,
        rotation: scene.rotation,
        filter: format!("{:?}", scene.filter).to_lowercase(),
        scale_x_milli: ratio_milli(dest.w, tw),
        scale_x_whole: tw > 0 && dest.w.is_multiple_of(tw),
        scale_y_milli: ratio_milli(dest.h, th),
        scale_y_whole: th > 0 && dest.h.is_multiple_of(th),
        cropped,
        borders,
        fill_w_permille,
        fill_h_permille,
        policy_aspect_milli: (shown_aspect * 1000.0).round() as u32,
        aspect_asked_milli: (asked_aspect * 1000.0).round() as u32,
        policy_aspect_error_permille: error(shown_aspect),
        model_aspect_error_permille: error(glass_aspect),
        uneven_lines: th == 0 || !dest.h.is_multiple_of(th),
        dropped_lines: dropped_lines(th, dest.h, scene.filter),
        console_picture: console.is_some(),
        placed_area_permille,
        notice_cropped_lines: shown.notice.map_or(0, |n| n.cropped_lines),
        glass_hidden,
        overscan_hidden,
        speed_ppm,
        speed_alert: timing.changes_speed(),
        mode_speed_ppm,
        scanlines: match scene.scanlines.kind {
            ScanlineKind::None => "none".to_owned(),
            ScanlineKind::Black => format!("black {}%", scene.scanlines.strength_percent),
        },
        notices: shown.notice.as_ref().map(notice_parts).unwrap_or_default(),
        player: Player::default(),
        border_on_wire: None,
        border_on_glass: None,
        glass_shape: None,
        glass_centre_permille: None,
        fields: fields::fields(glass, mode, &shown.frame_buffer, dest),
    }
}

/// The picture of `shown` as the player of `view` sees it, from the
/// measures `p` in the mode.
fn player(view: &View<'_>, shown: &Shown, p: &Picture) -> Player {
    let screen = view.orientation.screen;
    // The truth is the turn that the core asks for: the source draws its
    // frame for that turn. A game override that differs shows as an error.
    let game = view.core_source.rotation;
    let asked = asked_aspect(view.core_source, &p.crop, game);
    let shown_aspect = f64::from(p.policy_aspect_milli) / 1000.0;
    let seen = match screen {
        Rotation::R90 | Rotation::R270 if shown_aspect > 0.0 => 1.0 / shown_aspect,
        _ => shown_aspect,
    };
    let (fill_w_permille, fill_h_permille) = if screen.across() {
        (p.fill_h_permille, p.fill_w_permille)
    } else {
        (p.fill_w_permille, p.fill_h_permille)
    };
    let upright = view
        .pattern
        .then(|| corner_seen_top_left(shown, screen))
        .flatten()
        .map(|corner| corner == upright_top_left(game));
    Player {
        screen_degrees: screen.degrees(),
        aspect_milli: (seen * 1000.0).round() as u32,
        aspect_asked_milli: (asked * 1000.0).round() as u32,
        aspect_error_permille: if asked > 0.0 {
            ((seen / asked - 1.0) * 1000.0).round() as i64
        } else {
            0
        },
        fill_w_permille,
        fill_h_permille,
        upright,
    }
}

/// A channel of a pixel within this of a corner mark counts as that mark:
/// the filter blends the 1-pixel arms of a mark with their neighbours.
const CORNER_MATCH_MAX: u8 = 40;

/// The corner marks lie within this many frame pixels of their corner.
const CORNER_REACH_PX: u32 = 9;

/// The index in [`CORNERS`] of the frame corner that a turn of the
/// upright game by `game` puts at the top left. The turn is
/// counter-clockwise: at 90 the top-right corner of the frame comes to
/// the top left.
fn upright_top_left(game: Rotation) -> usize {
    match game {
        Rotation::R0 => 0,
        Rotation::R90 => 1,
        Rotation::R180 => 3,
        Rotation::R270 => 2,
    }
}

/// The index in [`CORNERS`] of the corner mark that shows at the corner of
/// the picture that the player of a screen at `screen` sees at the top
/// left, or `None` when no mark shows there.
fn corner_seen_top_left(shown: &Shown, screen: Rotation) -> Option<usize> {
    let fb = &shown.frame_buffer;
    let dest = shown.scene.dest;
    let (tw, th) = turned(&shown.scene.crop, shown.scene.rotation);
    let reach = |size: u32, units: u32| {
        (u64::from(size) * u64::from(CORNER_REACH_PX)).div_ceil(u64::from(units.max(1))) as i64
    };
    let (rw, rh) = (reach(dest.w, tw), reach(dest.h, th));
    let (x0, y0) = (i64::from(dest.x), i64::from(dest.y));
    let (x1, y1) = (x0 + i64::from(dest.w), y0 + i64::from(dest.h));
    // The screen turns counter-clockwise by `screen`: at 90 the top-right
    // corner of the mode is at the top left for the player.
    let (xs, ys) = match screen {
        Rotation::R0 => (x0..x0 + rw, y0..y0 + rh),
        Rotation::R90 => (x1 - rw..x1, y0..y0 + rh),
        Rotation::R180 => (x1 - rw..x1, y1 - rh..y1),
        Rotation::R270 => (x0..x0 + rw, y1 - rh..y1),
    };
    let mut counts = [0_u32; 4];
    for y in ys.filter(|&y| (0..i64::from(fb.height)).contains(&y)) {
        for x in xs
            .clone()
            .filter(|&x| (0..i64::from(fb.width)).contains(&x))
        {
            if let Some(i) = corner_mark(fb, x as u32, y as u32) {
                counts[i] += 1;
            }
        }
    }
    let (best, &n) = counts.iter().enumerate().max_by_key(|(_, n)| **n)?;
    (n > 0).then_some(best)
}

/// The corner mark that pixel `(x, y)` of `fb` shows, if any.
fn corner_mark(fb: &Rgb, x: u32, y: u32) -> Option<usize> {
    let px = fb.get(x, y);
    CORNERS.iter().position(|mark| {
        mark.iter()
            .zip(px)
            .all(|(a, b)| a.abs_diff(b) <= CORNER_MATCH_MAX)
    })
}

/// The console width of the policy, in samples, for a console picture on a
/// TV: not an arcade board, a 4:3 frame, and 224 to 240 of its `lines` per
/// field (the rule of the policy's `aspect_samples`).
fn console_width(
    glass: &Glass,
    lines: u16,
    aspect_milli: u32,
    arcade: bool,
    mode: &neptron_types::Modeline,
) -> Option<u16> {
    let Glass::Tube(Tube {
        class: TubeClass::Tv(standard),
        ..
    }) = glass
    else {
        return None;
    };
    let field_lines = if mode.interlace { lines / 2 } else { lines };
    let four_thirds = aspect_milli.abs_diff(FOUR_THIRDS_MILLI) <= 1;
    let console = !arcade
        && four_thirds
        && (TV_CONSOLE_LINES_MIN..=TV_525_PICTURE_LINES).contains(&field_lines);
    console.then(|| {
        if standard.lines() == 525 {
            TV_525_CONSOLE_WIDTH_SAMPLES
        } else {
            TV_625_CONSOLE_WIDTH_SAMPLES
        }
    })
}

/// The source lines of `lines` that no line of `dest_lines` samples, by
/// the tap rule of the reference renderer: each screen line samples the
/// source line under its centre, and sharp bilinear below a scale of 1
/// also blends the neighbour on the side of the centre, unless the centre
/// is the middle of the line.
pub fn dropped_lines(lines: u32, dest_lines: u32, filter: neptron_types::Filter) -> u32 {
    if lines == 0 || dest_lines == 0 {
        return lines;
    }
    let (n, m) = (u64::from(lines), u64::from(dest_lines));
    let mut seen = vec![false; lines as usize];
    for i in 0..m {
        let num = (2 * i + 1) * n;
        let (near, rem) = (num / (2 * m), num % (2 * m));
        seen[near.min(n - 1) as usize] = true;
        let blends = filter == neptron_types::Filter::SharpBilinear && m < n && 2 * rem != 2 * m;
        if blends {
            let far = if 2 * rem < 2 * m {
                near.checked_sub(1)
            } else {
                Some(near + 1)
            };
            if let Some(far) = far.filter(|&f| f < n) {
                seen[far as usize] = true;
            }
        }
    }
    u32::try_from(seen.iter().filter(|s| !**s).count()).unwrap_or(u32::MAX)
}

/// The lines of the upright source.
fn upright_lines(source: &SourceGeometry) -> u16 {
    match source.rotation {
        Rotation::R0 | Rotation::R180 => source.height,
        Rotation::R90 | Rotation::R270 => source.width,
    }
}

/// The display aspect 4:3, in thousandths, as a core gives it.
const FOUR_THIRDS_MILLI: u32 = 1_333;

/// The columns and lines of `crop` after the turn.
pub fn turned(crop: &Rect, rotation: Rotation) -> (u32, u32) {
    match rotation {
        Rotation::R0 | Rotation::R180 => (crop.w, crop.h),
        Rotation::R90 | Rotation::R270 => (crop.h, crop.w),
    }
}

/// The source pixels outside `crop`, at each edge of the upright picture.
/// The turn is counter-clockwise: after a turn of 90 the right edge of the
/// frame is the top of the picture.
fn frame_cut(source: &SourceGeometry, crop: &Rect, rotation: Rotation) -> Edges<u32> {
    let (w, h) = (i64::from(source.width), i64::from(source.height));
    let (x, y, cw, ch) = (
        i64::from(crop.x),
        i64::from(crop.y),
        i64::from(crop.w),
        i64::from(crop.h),
    );
    let at = |v: i64| u32::try_from(v.max(0)).unwrap_or(u32::MAX);
    let frame = Edges {
        top: at(y),
        bottom: at(h - y - ch),
        left: at(x),
        right: at(w - x - cw),
    };
    match rotation {
        Rotation::R0 => frame,
        Rotation::R90 => Edges {
            top: frame.right,
            bottom: frame.left,
            left: frame.top,
            right: frame.bottom,
        },
        Rotation::R180 => Edges {
            top: frame.bottom,
            bottom: frame.top,
            left: frame.right,
            right: frame.left,
        },
        Rotation::R270 => Edges {
            top: frame.left,
            bottom: frame.right,
            left: frame.bottom,
            right: frame.top,
        },
    }
}

/// `over` mode pixels past an edge, as source pixels of a picture of
/// `source` pixels over `dest` mode pixels; 0 when nothing is past.
fn to_source(over: i64, dest: i64, source: u32) -> u32 {
    if over <= 0 || dest <= 0 {
        return 0;
    }
    let pixels = (over * i64::from(source) + dest / 2) / dest;
    u32::try_from(pixels).unwrap_or(u32::MAX).min(source)
}

/// `part` of `whole` (both on the glass), as a count of `units`, rounded.
fn share(part: f64, whole: f64, units: u32) -> u32 {
    if part <= 0.0 || whole <= 0.0 {
        return 0;
    }
    ((part / whole).min(1.0) * f64::from(units)).round() as u32
}

fn ratio_milli(to: u32, from: u32) -> u32 {
    if from == 0 {
        return 0;
    }
    ((u64::from(to) * 1_000 + u64::from(from) / 2) / u64::from(from)) as u32
}

/// The width to height that the source asks for the part `crop` of its
/// frame, upright.
fn asked_aspect(source: &SourceGeometry, crop: &Rect, rotation: Rotation) -> f64 {
    let (w, h) = (f64::from(source.width), f64::from(source.height));
    if crop.h == 0 || crop.w == 0 || w == 0.0 {
        return 0.0;
    }
    let pixel_aspect = match source.aspect_milli {
        0 => 1.0,
        a => f64::from(a) / 1000.0 * h / w,
    };
    let frame_part = f64::from(crop.w) * pixel_aspect / f64::from(crop.h);
    match rotation {
        Rotation::R0 | Rotation::R180 => frame_part,
        Rotation::R90 | Rotation::R270 => 1.0 / frame_part,
    }
}

/// The parts of a notice that say something, as `name` or `name=value`.
pub fn notice_parts(notice: &Notice) -> Vec<String> {
    let mut out = Vec::new();
    if notice.fitted {
        out.push("fitted".to_owned());
    }
    if notice.speed_change {
        out.push("speed_change".to_owned());
    }
    if let Some(replaced) = notice.strategy_replaced {
        out.push(format!("strategy_replaced={replaced:?}"));
    }
    if notice.double_refresh {
        out.push("double_refresh".to_owned());
    }
    if notice.style_fallback {
        out.push("style_fallback".to_owned());
    }
    if notice.cropped_lines > 0 {
        out.push(format!("cropped_lines={}", notice.cropped_lines));
    }
    if let Some(fill) = notice.padded_fill_permille {
        out.push(format!("padded_fill={fill}permille"));
    }
    if let Some(soft) = notice.soft {
        out.push(format!("soft={soft:?}"));
    }
    if let Some(fill) = notice.small_fill_permille {
        out.push(format!("small_fill={fill}permille"));
    }
    if let Some(why) = notice.whole_steps {
        out.push(format!("whole_steps={why:?}"));
    }
    if let Some(style) = notice.fill_fallback {
        out.push(format!("fill_fallback={style:?}"));
    }
    out
}

impl Measures {
    /// The measures as JSON.
    pub fn to_json(&self) -> Json {
        object([
            ("display", (&self.display).into()),
            ("class", (&self.class).into()),
            ("gpu", (&self.gpu).into()),
            ("source", (&self.source).into()),
            ("settings", (&self.settings).into()),
            ("offer", (&self.offer).into()),
            ("no_picture", self.no_picture.clone().into()),
            (
                "picture",
                self.picture.as_ref().map(Picture::to_json).into(),
            ),
        ])
    }

    /// The header of [`Self::row`].
    pub const HEADER: &'static str = "| display | source | settings | mode | kHz | Hz | dest | scale x/y | cropped t/b/l/r | borders t/b/l/r | fill w/h | aspect | speed | scanlines | hidden by glass t/b/l/r | notices | offer | player |";

    /// The measures as one row of a Markdown table.
    pub fn row(&self) -> String {
        let names = format!("| {} | {} | {} |", self.display, self.source, self.settings);
        let Some(p) = &self.picture else {
            let why = self.no_picture.as_deref().unwrap_or("no picture");
            return format!(
                "{names} no picture | | | | | | | | | | | | {why} | {} | |",
                self.offer
            );
        };
        let m = &p.mode;
        format!(
            "{names} {}x{}{} | {} | {} | {}x{}+{}+{} | {}/{} | {} | {} | {}/{} | {} | {} | {} | {} | {} | {} | {} |",
            m.hactive,
            m.vactive,
            scan_letter(m.interlace, m.doublescan),
            milli_text(m.line_rate_mhz / 1_000),
            milli_text(m.field_rate_mhz),
            p.dest.w,
            p.dest.h,
            p.dest.x,
            p.dest.y,
            scale_text(p.scale_x_milli, p.scale_x_whole),
            scale_text(p.scale_y_milli, p.scale_y_whole),
            edges_text(&p.cropped),
            edges_text(&p.borders),
            permille_text(i64::from(p.fill_w_permille)),
            permille_text(i64::from(p.fill_h_permille)),
            signed_permille_text(p.policy_aspect_error_permille),
            speed_text(p.speed_ppm, p.speed_alert),
            p.scanlines,
            edges_text(&p.glass_hidden),
            if p.notices.is_empty() {
                "-".to_owned()
            } else {
                p.notices.join(", ")
            },
            self.offer,
            player_text(&p.player),
        )
    }
}

/// The player measures of a row: the screen orientation, the shape error,
/// the fill across and down, and whether the picture is upright.
fn player_text(p: &Player) -> String {
    let upright = match p.upright {
        Some(true) => "upright",
        Some(false) => "NOT UPRIGHT",
        None => "upright ?",
    };
    format!(
        "{} deg {} {}/{} {upright}",
        p.screen_degrees,
        signed_permille_text(p.aspect_error_permille),
        permille_text(i64::from(p.fill_w_permille)),
        permille_text(i64::from(p.fill_h_permille)),
    )
}

impl Picture {
    fn to_json(&self) -> Json {
        let m = &self.mode;
        object([
            (
                "mode",
                object([
                    ("hactive", m.hactive.into()),
                    ("vactive", m.vactive.into()),
                    ("htotal", m.htotal.into()),
                    ("vtotal", m.vtotal.into()),
                    ("pclock_hz", m.pclock_hz.into()),
                    ("line_rate_mhz", m.line_rate_mhz.into()),
                    ("field_rate_mhz", m.field_rate_mhz.into()),
                    ("interlace", m.interlace.into()),
                    ("doublescan", m.doublescan.into()),
                ]),
            ),
            ("rung", (&self.rung).into()),
            ("crop", rect_json(&self.crop)),
            ("dest", rect_json(&self.dest)),
            ("rotation", format!("{:?}", self.rotation).into()),
            ("filter", (&self.filter).into()),
            ("scale_x_milli", self.scale_x_milli.into()),
            ("scale_x_whole", self.scale_x_whole.into()),
            ("scale_y_milli", self.scale_y_milli.into()),
            ("scale_y_whole", self.scale_y_whole.into()),
            ("cropped_source_px", edges_json(&self.cropped)),
            ("borders_mode_px", edges_json(&self.borders)),
            ("fill_w_permille", self.fill_w_permille.into()),
            ("fill_h_permille", self.fill_h_permille.into()),
            ("policy_aspect_milli", self.policy_aspect_milli.into()),
            ("aspect_asked_milli", self.aspect_asked_milli.into()),
            (
                "policy_aspect_error_permille",
                self.policy_aspect_error_permille.into(),
            ),
            (
                "model_aspect_error_permille",
                self.model_aspect_error_permille.into(),
            ),
            ("uneven_lines", self.uneven_lines.into()),
            ("dropped_lines", self.dropped_lines.into()),
            ("notice_cropped_lines", self.notice_cropped_lines.into()),
            ("console_picture", self.console_picture.into()),
            ("placed_area_permille", self.placed_area_permille.into()),
            ("glass_hidden_source_px", edges_json(&self.glass_hidden)),
            (
                "overscan_hidden_source_px",
                self.overscan_hidden.as_ref().map(edges_json).into(),
            ),
            ("speed_ppm", self.speed_ppm.into()),
            ("speed_alert", self.speed_alert.into()),
            ("mode_speed_ppm", self.mode_speed_ppm.into()),
            ("scanlines", (&self.scanlines).into()),
            ("notices", self.notices.clone().into()),
            ("player", self.player.to_json()),
            (
                "border_on_wire",
                self.border_on_wire.as_ref().map(edges_json).into(),
            ),
            (
                "border_on_glass",
                self.border_on_glass.as_ref().map(edges_json).into(),
            ),
            (
                "glass_shape",
                self.glass_shape.as_ref().map(GlassShape::to_json).into(),
            ),
            (
                "glass_centre_permille",
                self.glass_centre_permille
                    .map(|(x, y)| object([("x", x.into()), ("y", y.into())]))
                    .into(),
            ),
            ("fields", self.fields.as_ref().map(fields_json).into()),
        ])
    }
}

impl Player {
    fn to_json(self) -> Json {
        object([
            ("screen_orientation", self.screen_degrees.into()),
            ("aspect_milli", self.aspect_milli.into()),
            ("aspect_asked_milli", self.aspect_asked_milli.into()),
            ("aspect_error_permille", self.aspect_error_permille.into()),
            ("fill_w_permille", self.fill_w_permille.into()),
            ("fill_h_permille", self.fill_h_permille.into()),
            ("upright", self.upright.into()),
        ])
    }
}

fn fields_json(f: &Fields) -> Json {
    object([
        ("field_offset_milli", f.field_offset_milli.into()),
        ("own_lines", f.own_lines.into()),
        ("flicker_permille", f.flicker_permille.into()),
        (
            "filtered_flicker_permille",
            f.filtered_flicker_permille.into(),
        ),
        (
            "source_lines",
            f.source_lines
                .map(|l| object([("own", l.own.into()), ("shared", l.shared.into())]))
                .into(),
        ),
    ])
}

fn rect_json(r: &Rect) -> Json {
    object([
        ("x", r.x.into()),
        ("y", r.y.into()),
        ("w", r.w.into()),
        ("h", r.h.into()),
    ])
}

fn edges_json<T: Copy + Into<Json>>(e: &Edges<T>) -> Json {
    object([
        ("top", e.top.into()),
        ("bottom", e.bottom.into()),
        ("left", e.left.into()),
        ("right", e.right.into()),
    ])
}

fn edges_text<T: std::fmt::Display>(e: &Edges<T>) -> String {
    format!("{}/{}/{}/{}", e.top, e.bottom, e.left, e.right)
}

/// `p`, `i` or `d` after the size of a mode.
pub fn scan_letter(interlace: bool, doublescan: bool) -> &'static str {
    match (interlace, doublescan) {
        (true, _) => "i",
        (false, true) => "d",
        (false, false) => "p",
    }
}

/// Thousandths as a decimal with three places.
pub fn milli_text(milli: u64) -> String {
    format!("{}.{:03}", milli / 1_000, milli % 1_000)
}

fn scale_text(milli: u32, whole: bool) -> String {
    if whole {
        format!("{}", milli / 1_000)
    } else {
        format!("{}.{:03}", milli / 1_000, milli % 1_000)
    }
}

fn permille_text(p: i64) -> String {
    format!("{}.{} %", p / 10, (p % 10).abs())
}

fn signed_permille_text(p: i64) -> String {
    let sign = if p < 0 { "-" } else { "+" };
    let a = p.abs();
    format!("{sign}{}.{} %", a / 10, a % 10)
}

fn speed_text(ppm: i64, alert: bool) -> String {
    let base = format!("{ppm:+} ppm");
    if alert { format!("{base} ALERT") } else { base }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE: SourceGeometry = SourceGeometry {
        width: 384,
        height: 256,
        refresh_mhz: 55_000,
        aspect_milli: 1_333,
        interlaced: false,
        rotation: Rotation::R0,
    };

    fn rect(x: i32, y: i32, w: u32, h: u32) -> Rect {
        Rect { x, y, w, h }
    }

    #[test]
    fn sharp_bilinear_drops_no_line_from_a_scale_of_one_half() {
        use neptron_types::Filter::{Nearest, SharpBilinear};
        for (lines, dest) in [(256, 240), (272, 240), (480, 240), (224, 241), (320, 512)] {
            assert_eq!(
                dropped_lines(lines, dest, SharpBilinear),
                0,
                "{lines} in {dest}"
            );
        }
        // Nearest drops 16 of 256 lines in 240.
        assert_eq!(dropped_lines(256, 240, Nearest), 16);
        assert_eq!(dropped_lines(224, 448, Nearest), 0);
        // Below one half the two taps skip lines.
        assert!(dropped_lines(640, 240, SharpBilinear) > 0);
    }

    #[test]
    fn a_crop_of_8_lines_per_edge_is_counted_at_the_top_and_the_bottom() {
        let cut = frame_cut(&SOURCE, &rect(0, 8, 384, 240), Rotation::R0);
        assert_eq!(
            cut,
            Edges {
                top: 8,
                bottom: 8,
                left: 0,
                right: 0
            }
        );
    }

    #[test]
    fn a_turned_crop_moves_its_edges() {
        let source = SourceGeometry {
            width: 320,
            height: 240,
            rotation: Rotation::R90,
            ..SOURCE
        };
        // 4 columns cut at the right of the frame: the top of the upright
        // picture after a counter-clockwise turn.
        let cut = frame_cut(&source, &rect(0, 0, 316, 240), Rotation::R90);
        assert_eq!((cut.top, cut.right), (4, 0));
        let cut = frame_cut(&source, &rect(0, 0, 316, 240), Rotation::R270);
        assert_eq!((cut.bottom, cut.top), (4, 0));
    }

    #[test]
    fn a_256_line_source_in_a_240_line_mode_loses_16_lines() {
        // A destination of 256 lines centred in 240: 8 lines past each
        // edge.
        assert_eq!(to_source(8, 256, 256), 8);
        assert_eq!(to_source(-8, 256, 256), 0);
        // At 2x, 16 mode lines past the edge are 8 source lines.
        assert_eq!(to_source(16, 512, 256), 8);
    }

    #[test]
    fn the_asked_aspect_follows_the_crop_and_the_turn() {
        let full = asked_aspect(&SOURCE, &rect(0, 0, 384, 256), Rotation::R0);
        assert!((full - 1.333).abs() < 1e-9);
        let cropped = asked_aspect(&SOURCE, &rect(0, 8, 384, 240), Rotation::R0);
        assert!((cropped - 1.333 * 256.0 / 240.0).abs() < 1e-9);
        let turned = asked_aspect(&SOURCE, &rect(0, 0, 384, 256), Rotation::R90);
        assert!((turned - 1.0 / 1.333).abs() < 1e-9);
    }

    #[test]
    fn the_scale_is_whole_only_for_a_whole_multiple() {
        assert_eq!(ratio_milli(1280, 256), 5_000);
        assert_eq!(ratio_milli(704, 384), 1_833);
        assert_eq!(turned(&rect(0, 0, 320, 240), Rotation::R270), (240, 320));
        assert_eq!(share(0.05, 1.0, 240), 12);
        assert_eq!(share(-0.1, 1.0, 240), 0);
    }

    #[test]
    fn a_row_shows_the_numbers() {
        let m = Measures {
            display: "d".into(),
            class: "crt".into(),
            gpu: "g".into(),
            source: "s".into(),
            settings: "auto/exact".into(),
            offer: "offered".into(),
            picture: None,
            no_picture: Some("declined: NoPicture".into()),
        };
        assert!(m.row().contains("no picture"), "{}", m.row());
        assert_eq!(m.to_json().get("picture"), Some(&Json::Null));
        assert_eq!(signed_permille_text(-62), "-6.2 %");
        assert_eq!(permille_text(1000), "100.0 %");
        assert_eq!(scale_text(1_833, false), "1.833");
        let player = Player {
            screen_degrees: 90,
            aspect_error_permille: -4,
            fill_w_permille: 1000,
            fill_h_permille: 750,
            upright: Some(false),
            ..Player::default()
        };
        assert_eq!(
            player_text(&player),
            "90 deg -0.4 % 100.0 %/75.0 % NOT UPRIGHT"
        );
    }
}
