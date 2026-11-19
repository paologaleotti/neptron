use neptron_types::Modeline;

use super::placement::{panel_placement, scale_milli};
use super::settings::Effective;
use super::{
    Considered, HeldWhole, OrientedSource, PanelScale, PictureStyle, Plan, Proposal, RefreshKind,
    RefreshTarget, Rung, Timing, WholeReason,
};

/// One, in thousandths.
const SCALE_ONE_MILLI: u32 = 1_000;

/// The scale, in thousandths, from which a game fills the height of a
/// flat panel instead of taking the largest whole multiple. From a scale
/// of 4 sharp bilinear prescales by a whole 4 or more, so a blended edge is
/// at most a quarter of a game pixel; under 4 the blend is wider, and the
/// whole multiple stays sharper.
pub const PANEL_FILL_MIN_SCALE_MILLI: u32 = 4_000;

/// The speed change of a panel mode, in ppm of the game's rate, that the
/// `speed` style counts as one step when it puts the mode nearest the
/// game's rate first: 0.1 %. Two modes within one step keep the order of
/// `auto`, so a mode a few millihertz nearer does not beat the preferred
/// size.
const SPEED_STEP_PPM: u64 = 1_000;

/// What keeps a whole multiple where the picture mode would fill: the
/// scanlines, or the expert key `display.panel_scale = integer` under a
/// mode that does not name the scale itself (`sharp` and the factors do).
fn held_whole(
    source: &OrientedSource,
    settings: &Effective,
    fill_from: Option<u32>,
) -> Option<HeldWhole> {
    let named = settings.style == PictureStyle::Sharp || settings.style.factor().is_some();
    if fill_from.is_some() || named {
        return None;
    }
    let asked = settings.asks_fit(source) || settings.style == PictureStyle::Fit;
    let fill_from_milli = if asked { 0 } else { PANEL_FILL_MIN_SCALE_MILLI };
    let by = if settings.panel_scanlines {
        WholeReason::Scanlines
    } else {
        WholeReason::PanelScaleKey
    };
    Some(HeldWhole {
        by,
        fill_from_milli,
    })
}

/// The plan on a flat panel: each listed mode, the game scaled into it by
/// the largest whole vertical factor that fits, or by the factor of the
/// style.
///
/// The first listed mode is the panel's preferred mode. The order is: modes
/// that scale the game by a whole factor before those that fit it with a
/// fraction; then modes of the preferred size; then the mode whose rate per
/// game frame is nearest the game's; then the order of the list. The
/// `speed` style puts the rate right after the fit, in steps of
/// [`SPEED_STEP_PPM`], so only a mode that must shrink the game comes
/// after a farther rate. A mode near twice the game's rate shows each
/// frame twice.
pub(crate) fn panel_plan(
    source: &OrientedSource,
    settings: &Effective,
    modes: &[Modeline],
) -> Plan {
    let preferred = modes.first().map(|m| (m.hactive, m.vactive));
    let factor = settings.style.factor();
    // Scanlines need a whole vertical scale, so they rule out the fill. A
    // style with a factor names the scale.
    let fill_from = match settings.panel_scale {
        _ if settings.panel_scanlines || factor.is_some() => None,
        _ if settings.asks_fit(source) || settings.style == PictureStyle::Fit => Some(0),
        PanelScale::Auto => Some(PANEL_FILL_MIN_SCALE_MILLI),
        PanelScale::Integer => None,
    };
    let held_whole = held_whole(source, settings, fill_from);
    let game = u64::from(source.refresh_mhz);
    let mut entries: Vec<_> = modes
        .iter()
        .enumerate()
        .map(|(index, mode)| {
            let field = mode.field_rate_mhz();
            let repeat = if field.abs_diff(2 * game) < field.abs_diff(game) {
                2
            } else {
                1
            };
            let placement = panel_placement(source, mode, fill_from, factor);
            // A fill by the panel rule is the panel's own rung; only a game
            // larger than the mode is a fit.
            let fills = fill_from.is_some_and(|from| scale_milli(source, mode) >= from)
                && scale_milli(source, mode) >= SCALE_ONE_MILLI;
            let rung = match (factor, placement.scale.y) {
                (Some(_), Some(1)) => Rung::Native,
                (Some(_), Some(k)) => Rung::Multiple(u8::try_from(k).unwrap_or(u8::MAX)),
                (Some(_), None) => Rung::Fitted,
                (None, Some(_)) => Rung::Panel,
                (None, None) if fills => Rung::Panel,
                (None, None) => Rung::Fitted,
            };
            let target = RefreshTarget {
                kind: RefreshKind::Panel,
                rate_mhz: u32::try_from(field).unwrap_or(u32::MAX),
                repeat,
            };
            let off_ppm = field.abs_diff(game * u64::from(repeat)) * 1_000_000
                / (game * u64::from(repeat)).max(1);
            let speed_step = match settings.style {
                PictureStyle::Speed => off_ppm / SPEED_STEP_PPM,
                PictureStyle::Auto
                | PictureStyle::Native
                | PictureStyle::X2
                | PictureStyle::X3
                | PictureStyle::X4
                | PictureStyle::Sharp
                | PictureStyle::Fit => 0,
            };
            let key = (
                rung.is_fitted(),
                speed_step,
                Some((mode.hactive, mode.vactive)) != preferred,
                field.abs_diff(game * u64::from(repeat)),
                index,
            );
            let proposal = Proposal {
                mode: *mode,
                source_index: index,
                horizontal: None,
                placement,
                timing: Timing {
                    game_mhz: source.refresh_mhz,
                    target,
                    line_rate_mhz: mode.line_rate_mhz(),
                    field_rate_mhz: field,
                },
                bordered: false,
                strategy_replaced: None,
                padding: None,
                overscan_crop: None,
            };
            (
                key,
                Considered {
                    rung,
                    target,
                    outcome: Ok(proposal),
                },
            )
        })
        .collect();
    entries.sort_by_key(|(key, _)| *key);
    Plan {
        source: *source,
        style: settings.style,
        refresh: settings.refresh,
        horizontal: None,
        skipped: Vec::new(),
        considered: entries.into_iter().map(|(_, c)| c).collect(),
        held_whole,
    }
}
