use neptron_types::Modeline;

use super::placement::panel_placement;
use super::settings::Effective;
use super::{Considered, OrientedSource, Plan, Proposal, RefreshKind, RefreshTarget, Rung, Timing};

/// The plan on a flat panel: each listed mode, the game scaled into it by
/// the largest whole vertical factor that fits.
///
/// The first listed mode is the panel's preferred mode. The order is: modes
/// that scale the game by a whole factor before those that fit it with a
/// fraction; then modes of the preferred size; then the mode whose rate per
/// game frame is nearest the game's; then the order of the list. A mode near
/// twice the game's rate shows each frame twice.
pub(crate) fn panel_plan(
    source: &OrientedSource,
    settings: &Effective,
    modes: &[Modeline],
) -> Plan {
    let preferred = modes.first().map(|m| (m.hactive, m.vactive));
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
            let placement = panel_placement(source, mode);
            let rung = if placement.scale.y.is_some() {
                Rung::Panel
            } else {
                Rung::Fitted
            };
            let target = RefreshTarget {
                kind: RefreshKind::Panel,
                rate_mhz: u32::try_from(field).unwrap_or(u32::MAX),
                repeat,
            };
            let key = (
                rung.is_fitted(),
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
    }
}
