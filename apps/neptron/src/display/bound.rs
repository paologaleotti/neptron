//! The safe default of a named CRT preset: a profile whose every mode is a
//! mode of the preset.

use neptron_platform::EdidProfile;
use neptron_types::{CrtRange, MonitorKind, MonitorProfile};
use neptron_video::Monitor;

/// The mark between the id of the named preset and the rest of the id of
/// a profile made from it. No preset id holds it (`PresetError::Reserved`),
/// so the setting of such a profile is the part before it.
pub const DERIVED_ID_MARK: char = '@';

/// The safe default for the named preset `preset`, with `edid`, the
/// analog EDID profile of the display when it gives one, and `start`, the
/// 15 kHz safe start. "Inside" means the line rates, the field rates and
/// the line counts, range by range.
///
/// - `edid` inside `preset`: `edid`.
/// - `preset` inside `edid`: `preset` itself.
/// - A part in common: `preset` cut to the ranges of `edid`, with the
///   timings of `preset` and the pixel clock limit of `edid`.
/// - No part in common, or no analog EDID: `start` when it is inside
///   `preset`; else the ranges of `preset` under 19 kHz; else `start`.
///
/// A preset with no CRT range (a custom panel or TV profile) bounds no
/// line rate, so it gets `start`.
pub fn safe_for_preset(
    preset: &MonitorProfile,
    edid: Option<&EdidProfile>,
    start: Monitor,
) -> Monitor {
    let Some(named) = crt_ranges(preset) else {
        return start;
    };
    if let Some(edid) = edid
        && let Some(found) = crt_ranges(&edid.profile)
    {
        if inside(found, named) {
            return Monitor::Edid(edid.clone());
        }
        if inside(named, found) {
            return Monitor::Profile(preset.clone());
        }
        let common = intersect(named, found);
        if !common.is_empty() {
            return Monitor::Edid(EdidProfile {
                profile: MonitorProfile {
                    id: derived_id(&preset.id, &edid.profile.id),
                    kind: MonitorKind::Crt { ranges: common },
                },
                ..edid.clone()
            });
        }
    }
    if crt_ranges(start.profile()).is_some_and(|ranges| inside(ranges, named)) {
        return start;
    }
    let low: Vec<CrtRange> = named
        .iter()
        .filter(|r| r.h_max_mhz < CLASS_15_KHZ_LINE_RATE_LIMIT_MHZ)
        .copied()
        .collect();
    if low.len() == named.len() {
        Monitor::Profile(preset.clone())
    } else if low.is_empty() {
        start
    } else {
        Monitor::Profile(MonitorProfile {
            id: derived_id(&preset.id, LOW_RANGES_ID),
            kind: MonitorKind::Crt { ranges: low },
        })
    }
}

/// The named preset id of a profile id: the part before
/// [`DERIVED_ID_MARK`], or `None` for an id with no mark.
pub fn named_preset_of(id: &str) -> Option<&str> {
    id.split_once(DERIVED_ID_MARK).map(|(named, _)| named)
}

/// The highest line rate of the 15 kHz class of the hard rails, in
/// millihertz: the class ends under 19 kHz.
const CLASS_15_KHZ_LINE_RATE_LIMIT_MHZ: u64 = 19_000_000;

/// The id part of the 15 kHz ranges of a named preset.
const LOW_RANGES_ID: &str = "15khz";

/// The CRT ranges of `profile`; `None` for a profile with no CRT range.
fn crt_ranges(profile: &MonitorProfile) -> Option<&[CrtRange]> {
    match &profile.kind {
        MonitorKind::Crt { ranges } if !ranges.is_empty() => Some(ranges),
        MonitorKind::Crt { .. } | MonitorKind::FlatPanel { .. } | MonitorKind::Tv { .. } => None,
    }
}

/// Every range of `inner` lies inside one range of `outer`.
fn inside(inner: &[CrtRange], outer: &[CrtRange]) -> bool {
    inner
        .iter()
        .all(|i| outer.iter().any(|o| range_inside(i, o)))
}

fn range_inside(inner: &CrtRange, outer: &CrtRange) -> bool {
    let interlace_inside = interlaced_lines(inner).is_none_or(|(min, max)| {
        interlaced_lines(outer).is_some_and(|(o_min, o_max)| o_min <= min && max <= o_max)
    });
    outer.h_min_mhz <= inner.h_min_mhz
        && inner.h_max_mhz <= outer.h_max_mhz
        && outer.v_min_mhz <= inner.v_min_mhz
        && inner.v_max_mhz <= outer.v_max_mhz
        && outer.progressive_lines_min <= inner.progressive_lines_min
        && inner.progressive_lines_max <= outer.progressive_lines_max
        && interlace_inside
}

/// The parts of the ranges of `named` that lie inside a range of `found`,
/// with the timings of `named`.
fn intersect(named: &[CrtRange], found: &[CrtRange]) -> Vec<CrtRange> {
    named
        .iter()
        .flat_map(|n| found.iter().filter_map(move |f| range_common(n, f)))
        .collect()
}

/// The common part of `named` and `found`: their line rates, field rates
/// and line counts cut to each other, the other fields of `named`. `None`
/// when a line rate, a field rate or a progressive line count has nothing
/// in common.
fn range_common(named: &CrtRange, found: &CrtRange) -> Option<CrtRange> {
    let h = common(
        (named.h_min_mhz, named.h_max_mhz),
        (found.h_min_mhz, found.h_max_mhz),
    )?;
    let v = common(
        (named.v_min_mhz, named.v_max_mhz),
        (found.v_min_mhz, found.v_max_mhz),
    )?;
    let progressive = common(
        (named.progressive_lines_min, named.progressive_lines_max),
        (found.progressive_lines_min, found.progressive_lines_max),
    )?;
    let interlaced = interlaced_lines(named)
        .zip(interlaced_lines(found))
        .and_then(|(n, f)| common(n, f))
        .unwrap_or((0, 0));
    Some(CrtRange {
        h_min_mhz: h.0,
        h_max_mhz: h.1,
        v_min_mhz: v.0,
        v_max_mhz: v.1,
        progressive_lines_min: progressive.0,
        progressive_lines_max: progressive.1,
        interlaced_lines_min: interlaced.0,
        interlaced_lines_max: interlaced.1,
        ..*named
    })
}

/// The interlaced line counts of `range`; `None` when it takes no
/// interlaced mode (both limits 0).
fn interlaced_lines(range: &CrtRange) -> Option<(u16, u16)> {
    (range.interlaced_lines_max != 0)
        .then_some((range.interlaced_lines_min, range.interlaced_lines_max))
}

/// The common part of two closed intervals.
fn common<T: Ord + Copy>(a: (T, T), b: (T, T)) -> Option<(T, T)> {
    let low = a.0.max(b.0);
    let high = a.1.min(b.1);
    (low <= high).then_some((low, high))
}

fn derived_id(named: &str, part: &str) -> String {
    format!("{named}{DERIVED_ID_MARK}{part}")
}
