//! The measures of a case: the mode, the place of the picture, the scale,
//! what is cut and what is black, how much of the screen the picture
//! fills, its shape and its speed. Pure.

use neptron_types::{Rect, Rotation, ScanlineKind, SourceGeometry};
use neptron_video::Notice;
use serde_json::{Value, json};

use super::glass::Glass;
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
    /// The width to height of the picture on the glass, in thousandths.
    pub aspect_shown_milli: u32,
    /// The width to height the source asks for the shown part, in
    /// thousandths.
    pub aspect_asked_milli: u32,
    /// Shown over asked, less 1, in per mille.
    pub aspect_error_permille: i64,
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
    /// The scanlines the renderer draws: `none` or `black N%`.
    pub scanlines: String,
    /// The notice of the policy, one entry per part.
    pub notices: Vec<String>,
}

/// The measures of one case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Measures {
    /// The display name.
    pub display: String,
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
    /// The GPU entry.
    pub gpu: &'a str,
    /// The source.
    pub source: &'a str,
    /// The settings.
    pub settings: &'a str,
}

/// The measures of `wire` for `source`. `glass` is the glass of the case
/// and the same glass with no overscan; `None` with no picture.
pub fn measure(
    names: Names<'_>,
    source: &SourceGeometry,
    wire: &Wire,
    glass: Option<(&Glass, &Glass)>,
) -> Measures {
    let offer = match &wire.offer {
        Ok(Ok(_)) => "offered".to_owned(),
        Ok(Err(declined)) => format!("declined: {:?}", declined.reason),
        Err(error) => format!("no plan: {error}"),
    };
    let picture = wire
        .shown
        .as_ref()
        .zip(glass)
        .map(|(shown, (glass, bare))| picture(source, shown, glass, bare));
    Measures {
        display: names.display.to_owned(),
        gpu: names.gpu.to_owned(),
        source: names.source.to_owned(),
        settings: names.settings.to_owned(),
        offer,
        picture,
        no_picture: wire.no_picture.clone(),
    }
}

fn picture(source: &SourceGeometry, shown: &Shown, glass: &Glass, bare: &Glass) -> Picture {
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
    let shown_aspect = if height > 0.0 { width / height } else { 0.0 };
    let asked_aspect = asked_aspect(source, &scene.crop, scene.rotation);
    let aspect_error_permille = if asked_aspect > 0.0 {
        ((shown_aspect / asked_aspect - 1.0) * 1000.0).round() as i64
    } else {
        0
    };
    let timing = &shown.proposal.timing;
    let game = i64::from(source.refresh_mhz.max(1));
    let speed_ppm = (timing.shown_mhz() as i64 - game) * 1_000_000 / game;
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
        aspect_shown_milli: (shown_aspect * 1000.0).round() as u32,
        aspect_asked_milli: (asked_aspect * 1000.0).round() as u32,
        aspect_error_permille,
        glass_hidden,
        overscan_hidden,
        speed_ppm,
        speed_alert: timing.changes_speed(),
        scanlines: match scene.scanlines.kind {
            ScanlineKind::None => "none".to_owned(),
            ScanlineKind::Black => format!("black {}%", scene.scanlines.strength_percent),
        },
        notices: shown.notice.as_ref().map(notice_parts).unwrap_or_default(),
    }
}

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
    pub fn to_json(&self) -> Value {
        json!({
            "display": self.display,
            "gpu": self.gpu,
            "source": self.source,
            "settings": self.settings,
            "offer": self.offer,
            "no_picture": self.no_picture,
            "picture": self.picture.as_ref().map(Picture::to_json),
        })
    }

    /// The header of [`Self::row`].
    pub const HEADER: &'static str = "| display | source | settings | mode | kHz | Hz | dest | scale x/y | cropped t/b/l/r | borders t/b/l/r | fill w/h | aspect | speed | scanlines | hidden by glass t/b/l/r | notices | offer |";

    /// The measures as one row of a Markdown table.
    pub fn row(&self) -> String {
        let names = format!("| {} | {} | {} |", self.display, self.source, self.settings);
        let Some(p) = &self.picture else {
            let why = self.no_picture.as_deref().unwrap_or("no picture");
            return format!(
                "{names} no picture | | | | | | | | | | | | {why} | {} |",
                self.offer
            );
        };
        let m = &p.mode;
        format!(
            "{names} {}x{}{} | {} | {} | {}x{}+{}+{} | {}/{} | {} | {} | {}/{} | {} | {} | {} | {} | {} | {} |",
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
            signed_permille_text(p.aspect_error_permille),
            speed_text(p.speed_ppm, p.speed_alert),
            p.scanlines,
            edges_text(&p.glass_hidden),
            if p.notices.is_empty() {
                "-".to_owned()
            } else {
                p.notices.join(", ")
            },
            self.offer,
        )
    }
}

impl Picture {
    fn to_json(&self) -> Value {
        let m = &self.mode;
        json!({
            "mode": {
                "hactive": m.hactive,
                "vactive": m.vactive,
                "htotal": m.htotal,
                "vtotal": m.vtotal,
                "pclock_hz": m.pclock_hz,
                "line_rate_mhz": m.line_rate_mhz,
                "field_rate_mhz": m.field_rate_mhz,
                "interlace": m.interlace,
                "doublescan": m.doublescan,
            },
            "rung": self.rung,
            "crop": rect_json(&self.crop),
            "dest": rect_json(&self.dest),
            "rotation": format!("{:?}", self.rotation),
            "filter": self.filter,
            "scale_x_milli": self.scale_x_milli,
            "scale_x_whole": self.scale_x_whole,
            "scale_y_milli": self.scale_y_milli,
            "scale_y_whole": self.scale_y_whole,
            "cropped_source_px": edges_json(&self.cropped),
            "borders_mode_px": edges_json(&self.borders),
            "fill_w_permille": self.fill_w_permille,
            "fill_h_permille": self.fill_h_permille,
            "aspect_shown_milli": self.aspect_shown_milli,
            "aspect_asked_milli": self.aspect_asked_milli,
            "aspect_error_permille": self.aspect_error_permille,
            "glass_hidden_source_px": edges_json(&self.glass_hidden),
            "overscan_hidden_source_px": self.overscan_hidden.as_ref().map(edges_json),
            "speed_ppm": self.speed_ppm,
            "speed_alert": self.speed_alert,
            "scanlines": self.scanlines,
            "notices": self.notices,
        })
    }
}

fn rect_json(r: &Rect) -> Value {
    json!({ "x": r.x, "y": r.y, "w": r.w, "h": r.h })
}

fn edges_json<T: Copy + Into<Value>>(e: &Edges<T>) -> Value {
    json!({ "top": e.top.into(), "bottom": e.bottom.into(), "left": e.left.into(), "right": e.right.into() })
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
            gpu: "g".into(),
            source: "s".into(),
            settings: "auto/exact".into(),
            offer: "offered".into(),
            picture: None,
            no_picture: Some("declined: NoPicture".into()),
        };
        assert!(m.row().contains("no picture"), "{}", m.row());
        assert_eq!(m.to_json()["picture"], Value::Null);
        assert_eq!(signed_permille_text(-62), "-6.2 %");
        assert_eq!(permille_text(1000), "100.0 %");
        assert_eq!(scale_text(1_833, false), "1.833");
    }
}
