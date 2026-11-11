//! The three test modes and how the spike derives a modeline for each from
//! the ranges of a preset.
//!
//! The derivation only proposes a modeline. `neptron_safety::validate`
//! judges it, and a mode that fails is never shown.

use std::fmt;

use neptron_types::{CrtRange, Modeline};

/// One test mode of the spike.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TestMode {
    /// 240 progressive lines.
    P240,
    /// 480 interlaced lines, two fields of 240.
    I480,
    /// 480 progressive lines.
    P480,
}

/// Where the timings of a derived modeline come from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// A range of the preset, by index, accepts the line count and gives the
    /// timings.
    PresetRange(usize),
    /// No range of the preset accepts the mode. The timings come from a
    /// built-in reference range, so that `validate` can say why the preset
    /// refuses it.
    Reference(&'static str),
}

/// A modeline derived for one test mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Derived {
    /// The test mode.
    pub test: TestMode,
    /// The modeline, or `None` when no timing fits the requested width.
    pub modeline: Option<Modeline>,
    /// Where the timings come from.
    pub source: Source,
}

impl TestMode {
    /// Every test mode, in the order the spike shows them.
    pub const ALL: [TestMode; 3] = [TestMode::P240, TestMode::I480, TestMode::P480];

    /// The name on the command line and in the report.
    pub fn name(self) -> &'static str {
        match self {
            TestMode::P240 => "240p",
            TestMode::I480 => "480i",
            TestMode::P480 => "480p",
        }
    }

    /// The test mode with this name.
    pub fn parse(name: &str) -> Option<TestMode> {
        TestMode::ALL.into_iter().find(|m| m.name() == name)
    }

    /// Active lines of a frame, both fields counted.
    pub fn active_lines(self) -> u16 {
        match self {
            TestMode::P240 => 240,
            TestMode::I480 | TestMode::P480 => 480,
        }
    }

    /// True for the interlaced mode.
    pub fn interlace(self) -> bool {
        self == TestMode::I480
    }
}

impl fmt::Display for TestMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Source::PresetRange(i) => write!(f, "range {} of the preset", i + 1),
            Source::Reference(name) => write!(
                f,
                "the {name} reference timings, because no range of the preset accepts the mode"
            ),
        }
    }
}

/// Derives the modeline of `test` at `width` pixels from the first range of
/// `ranges` that accepts its line count and gives a fitting timing. When no
/// range does, the modeline comes from a reference range.
pub fn derive(test: TestMode, width: u16, ranges: &[CrtRange]) -> Derived {
    for (index, range) in ranges.iter().enumerate() {
        if !accepts_lines(range, test) {
            continue;
        }
        for field_mhz in field_rate_targets(range) {
            if let Some(modeline) = fit(range, field_mhz, test, width) {
                return Derived {
                    test,
                    modeline: Some(modeline),
                    source: Source::PresetRange(index),
                };
            }
        }
    }
    let (name, reference) = match test {
        TestMode::P240 | TestMode::I480 => ("15 kHz NTSC", REFERENCE_15_KHZ),
        TestMode::P480 => ("31 kHz VGA", REFERENCE_31_KHZ),
    };
    let modeline = field_rate_targets(&reference)
        .into_iter()
        .find_map(|field_mhz| fit(&reference, field_mhz, test, width));
    Derived {
        test,
        modeline,
        source: Source::Reference(name),
    }
}

/// The field rate the spike aims for first.
const FIELD_RATE_TARGET_MHZ: u64 = 60_000;
/// Twice the target, for 240 lines on a 31 kHz range.
const FIELD_RATE_DOUBLE_MHZ: u64 = 120_000;
/// The most lines of blanking per field the search tries.
const BLANK_LINES_MAX: u16 = 400;
/// How many pixels above the smallest horizontal total the search tries,
/// to find a total whose rounded pixel clock lands inside the range.
const HTOTAL_SEARCH_PX: u16 = 256;
/// The kernel takes the pixel clock in kilohertz.
const PCLOCK_STEP_HZ: u64 = 1_000;
const NS_PER_S: u64 = 1_000_000_000;
/// A pixel rate in millihertz per kilohertz.
const MHZ_PER_KHZ: u64 = 1_000_000;
const MHZ_NS_PER_LINE: u64 = 1_000_000_000_000;

/// NTSC timings, as in the `ntsc` preset, with the line count limits of a
/// 15 kHz TV.
const REFERENCE_15_KHZ: CrtRange = CrtRange {
    h_min_mhz: 15_734_260,
    h_max_mhz: 15_734_260,
    v_min_mhz: 59_940,
    v_max_mhz: 59_940,
    h_front_porch_ns: 1_500,
    h_sync_ns: 4_700,
    h_back_porch_ns: 4_700,
    v_front_porch_ns: 191_000,
    v_sync_ns: 191_000,
    v_back_porch_ns: 953_000,
    hsync_positive: false,
    vsync_positive: false,
    progressive_lines_min: 192,
    progressive_lines_max: 240,
    interlaced_lines_min: 448,
    interlaced_lines_max: 480,
};

/// VGA timings at 31.469 kHz and 59.94 Hz. The vertical porches are a
/// little below the VGA values so that 480 lines fit in a total of 525.
const REFERENCE_31_KHZ: CrtRange = CrtRange {
    h_min_mhz: 31_469_000,
    h_max_mhz: 31_469_000,
    v_min_mhz: 59_940,
    v_max_mhz: 59_940,
    h_front_porch_ns: 636,
    h_sync_ns: 3_813,
    h_back_porch_ns: 1_907,
    v_front_porch_ns: 300_000,
    v_sync_ns: 64_000,
    v_back_porch_ns: 1_000_000,
    hsync_positive: false,
    vsync_positive: false,
    progressive_lines_min: 400,
    progressive_lines_max: 512,
    interlaced_lines_min: 0,
    interlaced_lines_max: 0,
};

fn accepts_lines(range: &CrtRange, test: TestMode) -> bool {
    let lines = test.active_lines();
    if test.interlace() {
        range.interlaced_lines_max > 0
            && (range.interlaced_lines_min..=range.interlaced_lines_max).contains(&lines)
    } else {
        (range.progressive_lines_min..=range.progressive_lines_max).contains(&lines)
    }
}

/// 60 Hz and 120 Hz when the range allows them, then the allowed rate
/// nearest to 60 Hz.
fn field_rate_targets(range: &CrtRange) -> Vec<u64> {
    let window = range.v_min_mhz..=range.v_max_mhz;
    let mut targets: Vec<u64> = [FIELD_RATE_TARGET_MHZ, FIELD_RATE_DOUBLE_MHZ]
        .into_iter()
        .filter(|f| window.contains(f))
        .collect();
    let nearest =
        FIELD_RATE_TARGET_MHZ.clamp(range.v_min_mhz, range.v_max_mhz.max(range.v_min_mhz));
    if !targets.contains(&nearest) {
        targets.push(nearest);
    }
    targets
}

/// The smallest vertical total with enough blanking for the range at this
/// field rate, and a horizontal total whose pixel clock in whole kilohertz
/// keeps both rates inside the range.
fn fit(range: &CrtRange, field_mhz: u64, test: TestMode, width: u16) -> Option<Modeline> {
    let interlace = test.interlace();
    let active_field = if interlace {
        test.active_lines() / 2
    } else {
        test.active_lines()
    };
    // The same lowest line rate as the validator: a fixed range accepts up
    // to one field rate below its line rate.
    let line_min_mhz = range
        .h_min_mhz
        .min(range.h_max_mhz.saturating_sub(field_mhz));
    for blank in 3..=BLANK_LINES_MAX {
        let field_lines = active_field + blank;
        let vtotal = if interlace {
            2 * field_lines + 1
        } else {
            field_lines
        };
        let line_mhz = if interlace {
            u64::from(vtotal) * field_mhz / 2
        } else {
            u64::from(vtotal) * field_mhz
        };
        if line_mhz > range.h_max_mhz {
            return None;
        }
        if line_mhz < line_min_mhz {
            continue;
        }
        let Some(v) = vertical(range, line_mhz, active_field, field_lines) else {
            continue;
        };
        for htotal in htotal_min(range, width, line_mhz)?..=htotal_max(range, width, line_mhz)? {
            let Some(h) = horizontal(range, width, htotal, line_mhz) else {
                continue;
            };
            let m = assemble(range, &h, &v, test, vtotal);
            let field = m.field_rate_mhz();
            let line = m.line_rate_mhz();
            if (range.v_min_mhz..=range.v_max_mhz).contains(&field)
                && (line_min_mhz..=range.h_max_mhz).contains(&line)
            {
                return Some(m);
            }
        }
    }
    None
}

/// Lines of one field: front porch, sync and back porch.
struct Vertical {
    active: u16,
    front: u16,
    sync: u16,
}

/// Pixels of one line, and the pixel clock.
struct Horizontal {
    pclock_hz: u32,
    active: u16,
    front: u16,
    sync: u16,
    total: u16,
}

fn vertical(range: &CrtRange, line_mhz: u64, active: u16, field_lines: u16) -> Option<Vertical> {
    let lines = |ns: u32, round_up: bool| -> u16 {
        let exact = u64::from(ns) * line_mhz;
        let n = if round_up {
            exact.div_ceil(MHZ_NS_PER_LINE)
        } else {
            (exact + MHZ_NS_PER_LINE / 2) / MHZ_NS_PER_LINE
        };
        u16::try_from(n.max(1)).unwrap_or(u16::MAX)
    };
    let front = lines(range.v_front_porch_ns, true);
    let sync = lines(range.v_sync_ns, false);
    let back = lines(range.v_back_porch_ns, true);
    let used = active
        .checked_add(front)?
        .checked_add(sync)?
        .checked_add(back)?;
    let extra = field_lines.checked_sub(used)?;
    Some(Vertical {
        active,
        front: front + extra / 2,
        sync,
    })
}

fn htotal_min(range: &CrtRange, width: u16, line_mhz: u64) -> Option<u16> {
    let period_ns = MHZ_NS_PER_LINE / line_mhz.max(1);
    let blank_ns = u64::from(range.h_front_porch_ns)
        + u64::from(range.h_sync_ns)
        + u64::from(range.h_back_porch_ns);
    let active_ns = period_ns.checked_sub(blank_ns).filter(|&t| t > 0)?;
    u16::try_from((u64::from(width) * period_ns).div_ceil(active_ns)).ok()
}

fn htotal_max(range: &CrtRange, width: u16, line_mhz: u64) -> Option<u16> {
    Some(htotal_min(range, width, line_mhz)?.saturating_add(HTOTAL_SEARCH_PX))
}

fn horizontal(range: &CrtRange, width: u16, htotal: u16, line_mhz: u64) -> Option<Horizontal> {
    let pclock_khz = (line_mhz * u64::from(htotal) + MHZ_PER_KHZ / 2) / MHZ_PER_KHZ;
    let pclock_hz = u32::try_from(pclock_khz * PCLOCK_STEP_HZ).ok()?;
    // Rounding up keeps each value at or above its nominal time; at a low
    // pixel clock a rounded-down sync pulse falls below the shortest
    // pulse the validator accepts.
    let pixels = |ns: u32| -> u16 {
        let n = (u64::from(ns) * u64::from(pclock_hz)).div_ceil(NS_PER_S);
        u16::try_from(n.max(1)).unwrap_or(u16::MAX)
    };
    let front = pixels(range.h_front_porch_ns);
    let sync = pixels(range.h_sync_ns);
    let back = pixels(range.h_back_porch_ns);
    let used = width
        .checked_add(front)?
        .checked_add(sync)?
        .checked_add(back)?;
    let extra = htotal.checked_sub(used)?;
    Some(Horizontal {
        pclock_hz,
        active: width,
        front: front + extra / 2,
        sync,
        total: htotal,
    })
}

fn assemble(
    range: &CrtRange,
    h: &Horizontal,
    v: &Vertical,
    test: TestMode,
    vtotal: u16,
) -> Modeline {
    // Vertical values count both fields of an interlaced mode, as in DRM.
    let scale = if test.interlace() { 2 } else { 1 };
    let vsync_start = (v.active + v.front) * scale;
    Modeline {
        pclock_hz: h.pclock_hz,
        hactive: h.active,
        hsync_start: h.active + h.front,
        hsync_end: h.active + h.front + h.sync,
        htotal: h.total,
        vactive: v.active * scale,
        vsync_start,
        vsync_end: vsync_start + v.sync * scale,
        vtotal,
        interlace: test.interlace(),
        doublescan: false,
        hsync_positive: range.hsync_positive,
        vsync_positive: range.vsync_positive,
    }
}

#[cfg(test)]
mod tests {
    use neptron_safety::validate;
    use neptron_types::GpuCaps;

    use super::*;
    use crate::presets;

    const ANY_CLOCK: GpuCaps = GpuCaps {
        pclock_min_hz: 1_000_000,
        pclock_max_hz: 400_000_000,
        interlace: true,
        flip_per_field: false,
        max_width: 4096,
        max_height: 4096,
    };

    fn derived(preset: &str, test: TestMode, width: u16) -> Derived {
        derive(test, width, presets::find(preset).unwrap().ranges)
    }

    #[test]
    fn names_round_trip() {
        for m in TestMode::ALL {
            assert_eq!(TestMode::parse(m.name()), Some(m));
        }
        assert_eq!(TestMode::parse("576i"), None);
        assert_eq!(TestMode::parse("240P"), None);
    }

    #[test]
    fn ntsc_240p_and_480i_come_from_the_preset_and_validate() {
        let profile = presets::find("ntsc").unwrap().profile();
        for test in [TestMode::P240, TestMode::I480] {
            let d = derived("ntsc", test, 640);
            assert_eq!(d.source, Source::PresetRange(0), "{test}");
            let m = d.modeline.unwrap();
            assert_eq!(m.vactive, test.active_lines());
            assert_eq!(m.interlace, test.interlace());
            assert_eq!(m.field_rate_mhz(), 59_940);
            assert_eq!(m.pclock_hz % 1_000, 0);
            validate(&m, &profile, &ANY_CLOCK).unwrap();
        }
    }

    #[test]
    fn interlaced_vertical_values_count_both_fields() {
        let m = derived("ntsc", TestMode::I480, 640).modeline.unwrap();
        assert_eq!(m.vtotal % 2, 1);
        assert_eq!((m.vsync_start - m.vactive) % 2, 0);
        assert_eq!((m.vsync_end - m.vsync_start) % 2, 0);
    }

    #[test]
    fn pc_31_120_gives_240p_at_120_hz_and_480p_at_60_hz() {
        let profile = presets::find("pc_31_120").unwrap().profile();
        let p240 = derived("pc_31_120", TestMode::P240, 640);
        assert_eq!(p240.source, Source::PresetRange(0));
        let m = p240.modeline.unwrap();
        assert!((119_000..=121_000).contains(&m.field_rate_mhz()));
        validate(&m, &profile, &ANY_CLOCK).unwrap();

        let p480 = derived("pc_31_120", TestMode::P480, 640);
        assert_eq!(p480.source, Source::PresetRange(1));
        let m = p480.modeline.unwrap();
        assert!((59_000..=61_000).contains(&m.field_rate_mhz()));
        validate(&m, &profile, &ANY_CLOCK).unwrap();
    }

    #[test]
    fn a_mode_the_preset_refuses_comes_from_a_reference_and_is_rejected() {
        let profile = presets::find("pc_31_120").unwrap().profile();
        let d = derived("pc_31_120", TestMode::I480, 640);
        assert_eq!(d.source, Source::Reference("15 kHz NTSC"));
        let m = d.modeline.unwrap();
        assert!(m.interlace);
        assert!(validate(&m, &profile, &ANY_CLOCK).is_err());

        let d = derived("ntsc", TestMode::P480, 640);
        assert_eq!(d.source, Source::Reference("31 kHz VGA"));
        assert!(validate(&d.modeline.unwrap(), &profile, &ANY_CLOCK).is_err());
    }

    #[test]
    fn every_preset_gives_a_modeline_for_every_mode_and_nothing_panics() {
        for preset in presets::all() {
            let profile = preset.profile();
            for test in TestMode::ALL {
                for width in [320, 640, 2560] {
                    let d = derive(test, width, preset.ranges);
                    let m = d
                        .modeline
                        .unwrap_or_else(|| panic!("{} {test} {width}", preset.id));
                    assert_eq!(m.hactive, width);
                    assert_eq!(m.pclock_hz % 1_000, 0);
                    if let Source::PresetRange(_) = d.source {
                        validate(&m, &profile, &ANY_CLOCK)
                            .unwrap_or_else(|r| panic!("{} {test} {width}: {r}", preset.id));
                    }
                }
            }
        }
    }

    #[test]
    fn a_width_too_wide_for_the_line_gives_no_modeline() {
        let d = derived("ntsc", TestMode::P240, u16::MAX);
        assert_eq!(d.modeline, None);
    }

    #[test]
    fn field_rate_targets_prefer_60_then_120_then_the_nearest() {
        let mut r = REFERENCE_15_KHZ;
        (r.v_min_mhz, r.v_max_mhz) = (50_000, 130_000);
        assert_eq!(field_rate_targets(&r), [60_000, 120_000]);
        (r.v_min_mhz, r.v_max_mhz) = (100_000, 130_000);
        assert_eq!(field_rate_targets(&r), [120_000, 100_000]);
        (r.v_min_mhz, r.v_max_mhz) = (50_000, 50_000);
        assert_eq!(field_rate_targets(&r), [50_000]);
    }
}
