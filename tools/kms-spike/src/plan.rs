//! The plan of a run: each test mode, its modeline and the verdict of
//! `validate`. The dry run prints the plan; a confirmed run shows only the
//! modes that passed.

use neptron_safety::{Rejection, ValidatedMode, validate};
use neptron_types::{CrtRange, GpuCaps, Modeline};

use crate::caps::{Allowances, Beyond, beyond_conservative};
use crate::modes::{Derived, TestMode, derive};
use crate::presets::Preset;

/// One planned test mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Planned {
    /// The derived modeline and where it comes from.
    pub derived: Derived,
    /// The verdict of `validate`.
    pub verdict: Verdict,
    /// Ways the mode is outside the conservative GPU values.
    pub beyond: Vec<Beyond>,
}

/// What `validate` said about a planned mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// The mode passed and can be shown.
    Valid(ValidatedMode),
    /// The mode failed and is skipped.
    Rejected(Rejection),
    /// No modeline fits the width, so there is nothing to validate.
    NoModeline,
}

/// Derives and validates every requested test mode.
pub fn plan(preset: &Preset, modes: &[TestMode], width: u16, allow: &Allowances) -> Vec<Planned> {
    let profile = preset.profile();
    let caps = allow.caps();
    modes
        .iter()
        .map(|&test| {
            let derived = derive(test, width, preset.ranges);
            let (verdict, beyond) = match &derived.modeline {
                Some(m) => (
                    match validate(m, &profile, &caps) {
                        Ok(v) => Verdict::Valid(v),
                        Err(r) => Verdict::Rejected(r),
                    },
                    beyond_conservative(m),
                ),
                None => (Verdict::NoModeline, Vec::new()),
            };
            Planned {
                derived,
                verdict,
                beyond,
            }
        })
        .collect()
}

impl Planned {
    /// The validated mode, when the mode passed.
    pub fn valid(&self) -> Option<&ValidatedMode> {
        match &self.verdict {
            Verdict::Valid(v) => Some(v),
            Verdict::Rejected(_) | Verdict::NoModeline => None,
        }
    }

    /// Flags the user did not give that would allow this mode on the GPU
    /// side. The range checks and the hard rails still apply.
    pub fn missing_flags(&self, allow: &Allowances) -> Vec<&'static str> {
        self.beyond
            .iter()
            .filter(|b| match b {
                Beyond::LowPixelClock { pclock_hz } => {
                    allow.pclock_min_hz.is_none_or(|min| *pclock_hz < min)
                }
                Beyond::Interlace => !allow.interlace,
            })
            .map(Beyond::flag)
            .collect()
    }
}

/// The preset's ranges as text, one block per range.
pub fn describe_preset(preset: &Preset) -> String {
    let mut out = format!("preset {} ({} ranges):\n", preset.id, preset.ranges.len());
    for (i, r) in preset.ranges.iter().enumerate() {
        out.push_str(&describe_range(i, r));
    }
    out
}

/// The GPU values that `validate` gets, as one line.
pub fn describe_caps(caps: &GpuCaps) -> String {
    format!(
        "GPU values: pixel clock {} to {} Hz, interlace {}, at most {}x{}",
        caps.pclock_min_hz,
        caps.pclock_max_hz,
        if caps.interlace { "allowed" } else { "refused" },
        caps.max_width,
        caps.max_height
    )
}

/// One planned mode as text: the modeline, its rates and the verdict.
pub fn describe_planned(p: &Planned, allow: &Allowances) -> String {
    let mut out = format!("{} ({}):\n", p.derived.test, p.derived.source);
    match &p.derived.modeline {
        Some(m) => {
            out.push_str(&format!("  modeline {}\n", modeline_text(m)));
        }
        None => out.push_str("  no modeline fits this width\n"),
    }
    for b in &p.beyond {
        out.push_str(&format!("  outside the conservative GPU values: {b}\n"));
    }
    match &p.verdict {
        Verdict::Valid(_) => out.push_str("  validate: passed\n"),
        Verdict::Rejected(r) => {
            out.push_str(&format!("  validate: REJECTED, {r}; the mode is skipped\n"));
        }
        Verdict::NoModeline => out.push_str("  validate: not run; the mode is skipped\n"),
    }
    let missing = p.missing_flags(allow);
    if !missing.is_empty() {
        out.push_str(&format!(
            "  the mode needs {} on the GPU side; the ranges and hard rails still apply\n",
            missing.join(" and ")
        ));
    }
    out
}

/// A modeline in the xrandr order, with its rates.
pub fn modeline_text(m: &Modeline) -> String {
    format!(
        "{} kHz  {} {} {} {}  {} {} {} {}{}{} {}hsync {}vsync  ({} Hz line, {} Hz field)",
        milli_text(u64::from(m.pclock_hz)),
        m.hactive,
        m.hsync_start,
        m.hsync_end,
        m.htotal,
        m.vactive,
        m.vsync_start,
        m.vsync_end,
        m.vtotal,
        if m.interlace { " interlace" } else { "" },
        if m.doublescan { " doublescan" } else { "" },
        if m.hsync_positive { "+" } else { "-" },
        if m.vsync_positive { "+" } else { "-" },
        milli_text(m.line_rate_mhz()),
        milli_text(m.field_rate_mhz()),
    )
}

/// A value in thousandths as a decimal with three places.
pub fn milli_text(value: u64) -> String {
    format!("{}.{:03}", value / 1_000, value % 1_000)
}

fn describe_range(i: usize, r: &CrtRange) -> String {
    let lines = |min: u16, max: u16| {
        if max == 0 {
            "none".to_owned()
        } else {
            format!("{min} to {max}")
        }
    };
    format!(
        "  range {}: line {} to {} Hz, field {} to {} Hz\n\
         \x20   h porch/sync/porch {}/{}/{} ns, v porch/sync/porch {}/{}/{} ns, sync {}h {}v\n\
         \x20   progressive lines {}, interlaced lines {}\n",
        i + 1,
        milli_text(r.h_min_mhz),
        milli_text(r.h_max_mhz),
        milli_text(r.v_min_mhz),
        milli_text(r.v_max_mhz),
        r.h_front_porch_ns,
        r.h_sync_ns,
        r.h_back_porch_ns,
        r.v_front_porch_ns,
        r.v_sync_ns,
        r.v_back_porch_ns,
        if r.hsync_positive { "+" } else { "-" },
        if r.vsync_positive { "+" } else { "-" },
        lines(r.progressive_lines_min, r.progressive_lines_max),
        lines(r.interlaced_lines_min, r.interlaced_lines_max),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presets;

    fn ntsc() -> &'static Preset {
        presets::find("ntsc").unwrap()
    }

    #[test]
    fn conservative_caps_reject_15_khz_modes_at_640_pixels() {
        let p = plan(ntsc(), &TestMode::ALL, 640, &Allowances::default());
        assert_eq!(p.len(), 3);
        for planned in &p {
            assert!(planned.valid().is_none(), "{:?}", planned.derived.test);
        }
        assert!(matches!(
            p[0].verdict,
            Verdict::Rejected(Rejection::PixelClock { .. })
        ));
        assert_eq!(
            p[0].missing_flags(&Allowances::default()),
            ["--allow-low-pclock <HZ>"]
        );
        assert_eq!(
            p[1].missing_flags(&Allowances::default()),
            ["--allow-low-pclock <HZ>", "--allow-interlace"]
        );
    }

    #[test]
    fn named_allowances_let_the_15_khz_modes_pass() {
        let allow = Allowances {
            pclock_min_hz: Some(6_000_000),
            interlace: true,
        };
        let p = plan(ntsc(), &[TestMode::P240, TestMode::I480], 640, &allow);
        for planned in &p {
            assert!(planned.valid().is_some(), "{:?}", planned.verdict);
            assert!(!planned.beyond.is_empty());
            assert!(planned.missing_flags(&allow).is_empty());
        }
    }

    #[test]
    fn a_clock_floor_above_the_mode_still_names_the_flag() {
        let allow = Allowances {
            pclock_min_hz: Some(24_000_000),
            interlace: false,
        };
        let p = plan(ntsc(), &[TestMode::P240], 640, &allow);
        assert!(p[0].valid().is_none());
        assert_eq!(p[0].missing_flags(&allow), ["--allow-low-pclock <HZ>"]);
    }

    #[test]
    fn allowances_never_open_a_range_the_preset_refuses() {
        let allow = Allowances {
            pclock_min_hz: Some(1_000_000),
            interlace: true,
        };
        let p = plan(ntsc(), &[TestMode::P480], 640, &allow);
        assert!(matches!(p[0].verdict, Verdict::Rejected(_)));
        let pc = presets::find("pc_31_120").unwrap();
        let p = plan(pc, &[TestMode::I480], 640, &allow);
        assert!(matches!(p[0].verdict, Verdict::Rejected(_)));
    }

    #[test]
    fn a_31_khz_preset_passes_480p_with_conservative_caps() {
        let pc = presets::find("pc_31_120").unwrap();
        let p = plan(pc, &[TestMode::P480], 640, &Allowances::default());
        assert!(p[0].valid().is_some(), "{:?}", p[0].verdict);
        assert!(p[0].beyond.is_empty());
    }

    #[test]
    fn descriptions_show_the_verdict_and_the_hint() {
        let p = plan(ntsc(), &[TestMode::I480], 640, &Allowances::default());
        let text = describe_planned(&p[0], &Allowances::default());
        assert!(text.contains("480i (range 1 of the preset)"), "{text}");
        assert!(text.contains("interlace"), "{text}");
        assert!(text.contains("REJECTED"), "{text}");
        assert!(text.contains("--allow-interlace"), "{text}");
        let preset = describe_preset(ntsc());
        assert!(
            preset.contains("line 15734.260 to 15734.260 Hz"),
            "{preset}"
        );
        assert!(preset.contains("interlaced lines 448 to 480"), "{preset}");
        assert!(describe_caps(&Allowances::default().caps()).contains("interlace refused"));
    }

    #[test]
    fn modeline_text_is_in_xrandr_order() {
        let m = Modeline {
            pclock_hz: 25_175_000,
            hactive: 640,
            hsync_start: 656,
            hsync_end: 752,
            htotal: 800,
            vactive: 480,
            vsync_start: 490,
            vsync_end: 492,
            vtotal: 525,
            interlace: false,
            doublescan: false,
            hsync_positive: false,
            vsync_positive: false,
        };
        assert_eq!(
            modeline_text(&m),
            "25175.000 kHz  640 656 752 800  480 490 492 525 -hsync -vsync  \
             (31468.750 Hz line, 59.940 Hz field)"
        );
    }
}
