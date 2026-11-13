mod width;

pub use width::{
    TV_525_CONSOLE_WIDTH_SAMPLES, TV_525_PICTURE_LINES, TV_625_CONSOLE_WIDTH_SAMPLES,
    TV_625_PICTURE_LINES, TV_CONSOLE_LINES_MIN, TV_SAFE_WIDTH_SAMPLES,
    TV_WHOLE_FACTOR_TOLERANCE_PERCENT,
};

use width::LineSystem;

use neptron_types::{GpuCaps, Modeline, Rect, TvStandard};

use super::placement::{centre, filter_for, line_crop, whole, whole_u32};
use super::settings::Effective;
use super::{
    Considered, Dropped, FORCE_50_MHZ, FORCE_60_MHZ, OVERSCAN_CROP_MAX_LINES_PER_EDGE,
    OrientedSource, OverscanCrop, Placement, Plan, Proposal, RefreshKind, RefreshPolicy,
    RefreshTarget, Rung, ScaleClaim, SkippedTarget, Timing, UI_CANVAS, walk_ladder,
};

/// The largest difference between the game's rate and the field rate of a
/// TV mode that keeps the game `exact`, in parts per million of the field
/// rate: the bound of the audio rate control
/// (`neptron_audio::MAX_RATE_DEVIATION_PPM`). The standard fixes the
/// rate, so the user cannot choose another. Within the bound the audio
/// keeps its nominal rate and the control absorbs the rest; beyond it the
/// game runs at the field rate and the player must know
/// ([`Timing::changes_speed`]).
pub const TV_SPEED_SILENT_PPM: u32 = 5_000;

/// The plan on a TV profile: the modes of the standard's fixed table, no
/// Switchres.
///
/// A progressive game walks: its lines centred in the progressive mode
/// (240p or 288p); the progressive mode with up to
/// [`OVERSCAN_CROP_MAX_LINES_PER_EDGE`] lines hidden per edge; the
/// interlaced mode (480i or 576i) with its lines 1:1; the fit into the
/// progressive mode. An interlaced game takes the interlaced mode first.
/// No exact rung scales a line. The game runs at the field rate of the
/// mode; a doubled or forced rate is a skipped target, because the
/// standard fixes the rate. `2x`, `3x` and `4x` have no mode: their
/// multiples are dropped, and the walk goes on as for `native`.
pub(crate) fn tv_plan(
    source: &OrientedSource,
    settings: &Effective,
    standard: TvStandard,
    caps: &GpuCaps,
) -> Plan {
    let tv = Tv {
        source,
        settings,
        modes: standard.modes(),
        system: if standard.lines() == 525 {
            LineSystem::L525
        } else {
            LineSystem::L625
        },
    };
    let mut considered: Vec<Considered> = walk_ladder(settings.style)
        .iter()
        .filter_map(|&rung| match rung {
            Rung::Multiple(factor) => {
                Some(tv.dropped(rung, PROGRESSIVE, Dropped::TvNoMultiple { factor }))
            }
            _ => None,
        })
        .collect();
    if source.interlaced {
        considered.push(tv.interlaced(Rung::Native, caps));
        considered.push(tv.dropped(Rung::Interlaced, INTERLACED, Dropped::SameAsNative));
    } else {
        considered.extend(tv.progressive_native());
        considered.push(tv.interlaced(Rung::Interlaced, caps));
    }
    considered.push(tv.proposal(Rung::Fitted, PROGRESSIVE, Fit::Fill));
    Plan {
        source: *source,
        style: settings.style,
        refresh: settings.refresh,
        horizontal: None,
        skipped: skipped(settings.refresh, source.refresh_mhz),
        considered,
    }
}

/// Index of the progressive mode in [`TvStandard::modes`].
const PROGRESSIVE: usize = 0;
/// Index of the interlaced mode in [`TvStandard::modes`].
const INTERLACED: usize = 1;

struct Tv<'a> {
    source: &'a OrientedSource,
    settings: &'a Effective,
    modes: [Modeline; 2],
    system: LineSystem,
}

/// How the game's lines sit in a TV mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fit {
    /// One game line per mode line, centred, less the lines of the
    /// overscan crop at the top and the bottom.
    Exact { crop: Option<OverscanCrop> },
    /// The game fills the active lines, scaled.
    Fill,
}

impl Tv<'_> {
    /// Rung 1 of a progressive game: centred when its lines fit, else the
    /// drop and, when at most the crop bound per edge is missing, the crop.
    fn progressive_native(&self) -> Vec<Considered> {
        let lines = self.source.height;
        let capacity = self.modes[PROGRESSIVE].vactive;
        if lines <= capacity {
            return vec![self.proposal(Rung::Native, PROGRESSIVE, Fit::Exact { crop: None })];
        }
        let mut out = vec![self.dropped(
            Rung::Native,
            PROGRESSIVE,
            Dropped::TvLines {
                lines,
                mode_lines: capacity,
            },
        )];
        let lines_per_edge = (lines - capacity).div_ceil(2);
        if lines_per_edge <= OVERSCAN_CROP_MAX_LINES_PER_EDGE {
            let crop = OverscanCrop {
                lines_per_edge,
                asked_lines: lines,
                capacity_lines: capacity,
            };
            out.push(self.proposal(Rung::Native, PROGRESSIVE, Fit::Exact { crop: Some(crop) }));
        }
        out
    }

    /// The interlaced mode with the game's lines 1:1: rung 1 of an
    /// interlaced game, rung 3 of a progressive game too tall for the
    /// progressive mode and its crop.
    fn interlaced(&self, rung: Rung, caps: &GpuCaps) -> Considered {
        let lines = self.source.height;
        if !self.source.interlaced {
            let progressive = self.modes[PROGRESSIVE].vactive;
            let reach = progressive + 2 * OVERSCAN_CROP_MAX_LINES_PER_EDGE;
            let limit = if lines <= progressive {
                Some(progressive)
            } else {
                (lines <= reach).then_some(reach)
            };
            if let Some(progressive_max) = limit {
                let reason = Dropped::FitsProgressive {
                    lines,
                    progressive_max,
                };
                return self.dropped(rung, INTERLACED, reason);
            }
        }
        if !caps.interlace {
            return self.dropped(rung, INTERLACED, Dropped::NoInterlace);
        }
        let capacity = self.modes[INTERLACED].vactive;
        if lines > capacity {
            let reason = Dropped::TvLines {
                lines,
                mode_lines: capacity,
            };
            return self.dropped(rung, INTERLACED, reason);
        }
        self.proposal(rung, INTERLACED, Fit::Exact { crop: None })
    }

    fn dropped(&self, rung: Rung, index: usize, reason: Dropped) -> Considered {
        Considered {
            rung,
            target: self.target(&self.modes[index]),
            outcome: Err(reason),
        }
    }

    fn proposal(&self, rung: Rung, index: usize, fit: Fit) -> Considered {
        let mode = self.modes[index];
        let target = self.target(&mode);
        let crop = match fit {
            Fit::Exact { crop } => crop,
            Fit::Fill => None,
        };
        Considered {
            rung,
            target,
            outcome: Ok(Proposal {
                mode,
                source_index: index,
                horizontal: None,
                placement: self.placement(&mode, fit),
                timing: Timing {
                    game_mhz: self.source.refresh_mhz,
                    target,
                    line_rate_mhz: mode.line_rate_mhz(),
                    field_rate_mhz: mode.field_rate_mhz(),
                },
                bordered: matches!(fit, Fit::Exact { .. }) && self.shown_lines(crop) < mode.vactive,
                strategy_replaced: None,
                padding: None,
                overscan_crop: crop,
            }),
        }
    }

    /// The game's lines after the crop.
    fn shown_lines(&self, crop: Option<OverscanCrop>) -> u16 {
        self.source
            .height
            .saturating_sub(crop.map_or(0, |c| c.lines()))
    }

    /// Where the game sits: its lines 1:1 and centred, or filling the
    /// active lines; the width by the `tv_width` rule, centred. The UI
    /// sits at whole factors ([`ui_dest`]).
    fn placement(&self, mode: &Modeline, fit: Fit) -> Placement {
        let source = self.source;
        let (y, h, claim_y, crop_per_edge) = match fit {
            Fit::Exact { crop } => {
                let lines = self.shown_lines(crop);
                (
                    centre(mode.vactive, u32::from(lines)),
                    u32::from(lines),
                    Some(1),
                    crop.map_or(0, |c| c.lines_per_edge),
                )
            }
            Fit::Fill => (
                0,
                u32::from(mode.vactive),
                whole(mode.vactive, source.height),
                0,
            ),
        };
        // The aspect holds for the whole frame: a crop hides lines and does
        // not change the width; a fit spans the active lines.
        let frame_lines = match fit {
            Fit::Exact { .. } => u32::from(source.height),
            Fit::Fill => h,
        };
        let fields = if mode.interlace { 2 } else { 1 };
        let field_lines = u16::try_from(frame_lines.div_ceil(fields)).unwrap_or(u16::MAX);
        let w = width::tv_samples(
            self.settings.tv_width,
            self.system,
            source.width,
            source.height,
            source.aspect_milli,
            field_lines,
        )
        .min(u32::from(mode.hactive));
        let scale = ScaleClaim {
            x: whole_u32(w, source.width),
            y: claim_y,
        };
        Placement {
            crop: line_crop(source, crop_per_edge),
            dest: Rect {
                x: centre(mode.hactive, w),
                y,
                w,
                h,
            },
            filter: filter_for(scale),
            ui_dest: ui_dest(mode),
            scale,
        }
    }

    /// The field rate of the mode. A game near half of it shows each
    /// frame twice.
    fn target(&self, mode: &Modeline) -> RefreshTarget {
        let field = mode.field_rate_mhz();
        let game = u64::from(self.source.refresh_mhz);
        let repeat = if field.abs_diff(2 * game) < field.abs_diff(game) {
            2
        } else {
            1
        };
        RefreshTarget {
            kind: RefreshKind::Tv,
            rate_mhz: u32::try_from(field).unwrap_or(u32::MAX),
            repeat,
        }
    }
}

/// The UI canvas at the largest whole factor across that fits the safe
/// width and the largest whole factor down that fits the active lines (at
/// least 1), centred. A fraction would give columns or lines of uneven
/// size, which shows on text. 320x240 is 640x240 at (40, 0) on 240p,
/// at (40, 24) on 288p, 640x480 at (40, 0) on 480i and at (40, 48) on 576i.
fn ui_dest(mode: &Modeline) -> Rect {
    let (canvas_w, canvas_h) = UI_CANVAS;
    let across = (TV_SAFE_WIDTH_SAMPLES.min(mode.hactive) / canvas_w.max(1)).max(1);
    let down = (mode.vactive / canvas_h.max(1)).max(1);
    let w = u32::from(canvas_w) * u32::from(across);
    let h = u32::from(canvas_h) * u32::from(down);
    Rect {
        x: centre(mode.hactive, w),
        y: centre(mode.vactive, h),
        w,
        h,
    }
}

/// The rate that the refresh policy asks for, which a TV cannot give: the
/// standard fixes the field rate.
fn skipped(policy: RefreshPolicy, game_mhz: u32) -> Vec<SkippedTarget> {
    let asked = match policy {
        RefreshPolicy::Exact => return Vec::new(),
        RefreshPolicy::ExactX2 => (RefreshKind::Double, u64::from(game_mhz) * 2),
        RefreshPolicy::Force50 => (RefreshKind::Forced50, u64::from(FORCE_50_MHZ)),
        RefreshPolicy::Force60 => (RefreshKind::Forced60, u64::from(FORCE_60_MHZ)),
    };
    vec![SkippedTarget {
        kind: asked.0,
        rate_mhz: asked.1,
    }]
}
