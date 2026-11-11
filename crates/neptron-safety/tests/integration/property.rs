//! Random modes for every shipped profile and every GPU entry: each one
//! validates or is rejected with a reason, and nothing panics. A mode that
//! validates is checked again here, without the code of the crate.

use neptron_safety::validate;
use neptron_types::{GpuCaps, Modeline, MonitorProfile};

use crate::synth::{GPUS, Request, build, presets, ranges};

/// SplitMix64: a fixed seed gives the same modes on every run and host.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A value in `low..=high`.
    fn range(&mut self, low: u64, high: u64) -> u64 {
        low + self.next() % (high - low + 1)
    }

    fn chance(&mut self, percent: u64) -> bool {
        self.next() % 100 < percent
    }

    fn u16(&mut self, low: u16, high: u16) -> u16 {
        u16::try_from(self.range(u64::from(low), u64::from(high))).unwrap()
    }
}

const MODES_PER_PROFILE_AND_GPU: usize = 400;

/// The widest values of all shipped presets, read from the files by hand.
/// No shipped range has a line rate below 15250 Hz, a field rate above
/// 130 Hz or more than 1024 lines.
const SHIPPED_LINE_RATE_MIN_MHZ: u64 = 15_250_000;
const SHIPPED_FIELD_RATE_MAX_MHZ: u64 = 130_000;
const SHIPPED_LINES_MAX: u16 = 1_024;
const EDID_MIN_LINE_RATE_MHZ: u64 = 40_000_000;

/// The facts an accepted mode must show, computed here from the modeline.
fn assert_sound(mode: &Modeline, profile: &MonitorProfile, caps: &GpuCaps) {
    let at = format!("{}: {mode:?}", profile.id);
    assert!(
        mode.pclock_hz >= caps.pclock_min_hz && mode.pclock_hz <= caps.pclock_max_hz,
        "{at}"
    );
    assert!(caps.interlace || !mode.interlace, "{at}");
    assert!(mode.hactive > 0 && mode.vactive > 0, "{at}");
    assert!(mode.hactive < mode.hsync_start, "{at}");
    assert!(mode.hsync_start < mode.hsync_end, "{at}");
    assert!(mode.hsync_end < mode.htotal, "{at}");
    assert!(mode.vactive < mode.vsync_start, "{at}");
    assert!(mode.vsync_start < mode.vsync_end, "{at}");
    assert!(mode.vsync_end < mode.vtotal, "{at}");
    let line = mode.line_rate_mhz();
    let field = mode.field_rate_mhz();
    let lines = if mode.doublescan {
        mode.vactive * 2
    } else {
        mode.vactive
    };
    let in_a_range = ranges(profile).iter().any(|r| {
        let (low, high) = if mode.interlace {
            (r.interlaced_lines_min, r.interlaced_lines_max)
        } else {
            (r.progressive_lines_min, r.progressive_lines_max)
        };
        // Switchres can end up to one field rate below h_min when one more
        // line would pass h_max.
        let lowest = r.h_min_mhz.min(r.h_max_mhz.saturating_sub(field));
        (lowest..=r.h_max_mhz).contains(&line)
            && (r.v_min_mhz..=r.v_max_mhz).contains(&field)
            && high > 0
            && (low..=high).contains(&lines)
    });
    assert!(in_a_range, "{at}");
    if line < EDID_MIN_LINE_RATE_MHZ {
        assert!(line >= SHIPPED_LINE_RATE_MIN_MHZ, "{at}");
        assert!(field <= SHIPPED_FIELD_RATE_MAX_MHZ, "{at}");
        assert!(lines <= SHIPPED_LINES_MAX, "{at}");
    }
}

/// A request near a random range of the profile: every value can land a
/// little outside the range.
fn near(rng: &mut Rng, profile: &MonitorProfile) -> (usize, Request) {
    let all = ranges(profile);
    let index = usize::try_from(rng.range(0, all.len() as u64 - 1)).unwrap();
    let r = all[index];
    let interlace = rng.chance(30);
    let doublescan = rng.chance(10);
    let (low, high) = if interlace && r.interlaced_lines_max > 0 {
        (r.interlaced_lines_min, r.interlaced_lines_max)
    } else {
        (r.progressive_lines_min, r.progressive_lines_max)
    };
    let mut lines = rng.u16(low.saturating_sub(40).max(1), high + 40);
    if doublescan {
        lines = (lines / 2).max(1);
    }
    let h_margin = r.h_max_mhz / 50;
    let line_rate_mhz = rng.range(r.h_min_mhz.saturating_sub(h_margin), r.h_max_mhz + h_margin);
    let field_rate_mhz = rng.range(
        r.v_min_mhz.saturating_sub(10_000).max(1),
        r.v_max_mhz + 10_000,
    );
    let request = Request {
        width: rng.u16(64, 1920),
        lines,
        // Whole hertz most of the time, so that some modes hit a range
        // exactly.
        line_rate_mhz: if rng.chance(80) {
            line_rate_mhz / 1_000 * 1_000
        } else {
            line_rate_mhz
        },
        field_rate_mhz,
        interlace,
        doublescan,
    };
    (index, request)
}

#[test]
fn random_modes_near_every_profile_validate_or_are_rejected_with_a_reason() {
    let mut rng = Rng(0x4E45_5054_524F_4E00);
    let (mut accepted, mut rejected) = (0_usize, 0_usize);
    for profile in presets() {
        for caps in &GPUS {
            for _ in 0..MODES_PER_PROFILE_AND_GPU {
                let (index, request) = near(&mut rng, &profile);
                let mode = build(&ranges(&profile)[index], request);
                match validate(&mode, &profile, caps) {
                    Ok(validated) => {
                        assert_eq!(validated.mode(), &mode);
                        assert_sound(&mode, &profile, caps);
                        accepted += 1;
                    }
                    Err(rejection) => {
                        assert!(!rejection.to_string().is_empty());
                        rejected += 1;
                    }
                }
            }
        }
    }
    // Both outcomes occur often, so both paths are exercised.
    assert!(accepted > 2_000, "accepted {accepted}");
    assert!(rejected > 2_000, "rejected {rejected}");
}

fn random_modeline(rng: &mut Rng) -> Modeline {
    let mut v = || u16::try_from(rng.next() >> 48).unwrap();
    let (a, b, c, d) = (v(), v(), v(), v());
    let (e, f, g, h) = (v(), v(), v(), v());
    let mut flags = rng.next();
    let mut flag = || {
        flags >>= 1;
        flags & 1 == 1
    };
    Modeline {
        pclock_hz: u32::try_from(rng.next() >> 32).unwrap(),
        hactive: a,
        hsync_start: b,
        hsync_end: c,
        htotal: d,
        vactive: e,
        vsync_start: f,
        vsync_end: g,
        vtotal: h,
        interlace: flag(),
        doublescan: flag(),
        hsync_positive: flag(),
        vsync_positive: flag(),
    }
}

/// Raw random fields, mostly malformed, and the same fields sorted into
/// order: no input panics.
#[test]
fn arbitrary_modelines_never_panic() {
    let mut rng = Rng(7);
    let profiles = presets();
    for _ in 0..2_000 {
        let raw = random_modeline(&mut rng);
        let mut h = [raw.hactive, raw.hsync_start, raw.hsync_end, raw.htotal];
        let mut v = [raw.vactive, raw.vsync_start, raw.vsync_end, raw.vtotal];
        h.sort_unstable();
        v.sort_unstable();
        let sorted = Modeline {
            hactive: h[0],
            hsync_start: h[1],
            hsync_end: h[2],
            htotal: h[3],
            vactive: v[0],
            vsync_start: v[1],
            vsync_end: v[2],
            vtotal: v[3],
            ..raw
        };
        for profile in &profiles {
            for caps in &GPUS {
                for mode in [raw, sorted] {
                    if let Ok(validated) = validate(&mode, profile, caps) {
                        assert_sound(validated.mode(), profile, caps);
                    }
                }
            }
        }
    }
}
