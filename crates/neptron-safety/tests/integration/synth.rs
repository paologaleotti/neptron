//! A small modeline generator for the tests: it places the nominal porches
//! and sync pulses of a range around an active area, in whole pixels and
//! lines rounded up. It is not Switchres and makes no policy choice.

use std::path::Path;

use neptron_types::{CrtRange, GpuCaps, Modeline, MonitorKind, MonitorProfile};

/// Every preset in data/monitors, in file name order.
pub fn presets() -> Vec<MonitorProfile> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/monitors");
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|e| e == "toml"))
        .collect();
    files.sort();
    files
        .iter()
        .map(|path| toml::from_str(&std::fs::read_to_string(path).unwrap()).unwrap())
        .collect()
}

pub fn ranges(profile: &MonitorProfile) -> &[CrtRange] {
    match &profile.kind {
        MonitorKind::Crt { ranges } => ranges,
        MonitorKind::FlatPanel { .. } | MonitorKind::Tv { .. } => {
            panic!("{} is not a CRT", profile.id)
        }
    }
}

/// GPU capability entries: an unknown GPU with the conservative values,
/// and two outputs that make low clocks and interlace.
pub const GPUS: [GpuCaps; 3] = [
    GpuCaps {
        pclock_min_hz: 25_000_000,
        pclock_max_hz: 400_000_000,
        interlace: false,
        flip_per_field: false,
        max_width: 4096,
        max_height: 4096,
    },
    GpuCaps {
        pclock_min_hz: 1_000_000,
        pclock_max_hz: 400_000_000,
        interlace: true,
        flip_per_field: false,
        max_width: 4096,
        max_height: 4096,
    },
    GpuCaps {
        pclock_min_hz: 6_000_000,
        pclock_max_hz: 165_000_000,
        interlace: true,
        flip_per_field: true,
        max_width: 2560,
        max_height: 1600,
    },
];

/// What to build.
#[derive(Debug, Clone, Copy)]
pub struct Request {
    pub width: u16,
    /// `vactive`: both fields for interlace, each line once for doublescan.
    pub lines: u16,
    pub line_rate_mhz: u64,
    pub field_rate_mhz: u64,
    pub interlace: bool,
    pub doublescan: bool,
}

const PS_PER_S: u128 = 1_000_000_000_000_000;

/// Builds a modeline with the nominal times of `range`. The line rate is
/// exact when `line_rate_mhz * htotal` is a whole number of hertz, so
/// `htotal` grows until it is. The field rate follows from the line count
/// and can differ from the request.
pub fn build(range: &CrtRange, req: Request) -> Modeline {
    let width = u128::from(req.width.max(1));
    let rate = u128::from(req.line_rate_mhz.max(1));
    // One line in picoseconds.
    let line_ps = PS_PER_S / rate;
    let blank_ps =
        u128::from(range.h_front_porch_ns + range.h_sync_ns + range.h_back_porch_ns) * 1_000;
    let active_ps = line_ps.saturating_sub(blank_ps).max(1);
    let pixels = |ns: u32| (u128::from(ns) * 1_000 * width).div_ceil(active_ps).max(1);
    let hfp = pixels(range.h_front_porch_ns);
    let hs = pixels(range.h_sync_ns);
    let mut htotal = width + hfp + hs + pixels(range.h_back_porch_ns);
    while !(rate * htotal).is_multiple_of(1_000) {
        htotal += 1;
    }
    let pclock = rate * htotal / 1_000;

    let fields: u128 = if req.interlace { 2 } else { 1 };
    let scans: u128 = if req.doublescan { 2 } else { 1 };
    // One unit of the vertical values, in picoseconds.
    let unit_ps = line_ps * scans / fields;
    let units = |ns: u32| (u128::from(ns) * 1_000).div_ceil(unit_ps).max(1);
    let lines = u128::from(req.lines.max(1));
    let vfp = units(range.v_front_porch_ns);
    let vs = units(range.v_sync_ns);
    let vbp = units(range.v_back_porch_ns);
    // The total that gives the requested field rate, rounded to the nearest line.
    let per_field = u128::from(req.field_rate_mhz.max(1)) * scans;
    let target = (2 * rate * fields + per_field) / (2 * per_field);
    let vtotal = target.max(lines + vfp + vs + vbp);

    let px = |v: u128| u16::try_from(v).unwrap_or(u16::MAX);
    Modeline {
        pclock_hz: u32::try_from(pclock).unwrap_or(u32::MAX),
        hactive: px(width),
        hsync_start: px(width + hfp),
        hsync_end: px(width + hfp + hs),
        htotal: px(htotal),
        vactive: px(lines),
        vsync_start: px(lines + vfp),
        vsync_end: px(lines + vfp + vs),
        vtotal: px(vtotal),
        interlace: req.interlace,
        doublescan: req.doublescan,
        hsync_positive: range.hsync_positive,
        vsync_positive: range.vsync_positive,
    }
}
