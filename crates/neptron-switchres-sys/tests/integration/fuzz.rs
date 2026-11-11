//! Random ranges, settings and requests from a fixed seed: every call
//! returns, with a list or a typed error. Each field is drawn over its whole
//! Rust type, and often from a plausible range, so that many calls also
//! reach the calculation. A hang fails the run on the slow-test timeout of
//! nextest.

use neptron_switchres_sys::{
    CallError, MAX_RANGES, Range, Request, Settings, VBLANK_MAX_FRACTION, modelines,
};

/// xorshift64: a fixed sequence with no dependency.
struct Random(u64);

impl Random {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn uniform(&mut self, low: f64, high: f64) -> f64 {
        low + (high - low) * ((self.next() >> 11) as f64 / (1u64 << 53) as f64)
    }

    fn one_in(&mut self, n: u64) -> bool {
        self.next().is_multiple_of(n)
    }

    fn flag(&mut self) -> bool {
        self.one_in(2)
    }

    fn u16(&mut self) -> u16 {
        (self.next() >> 48) as u16
    }

    fn u16_in(&mut self, low: u16, high: u16) -> u16 {
        let span = u64::from(high.saturating_sub(low)) + 1;
        low + u16::try_from(self.next() % span).unwrap()
    }

    /// Any f64: every bit pattern, so NaN, infinities, zeros, negatives,
    /// subnormals and huge values too, with the special values more often.
    fn any_f64(&mut self) -> f64 {
        const SPECIAL: [f64; 8] = [
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
            0.0,
            -0.0,
            f64::MIN_POSITIVE,
            f64::MAX,
            f64::MIN,
        ];
        if self.one_in(4) {
            SPECIAL[(self.next() % 8) as usize]
        } else {
            f64::from_bits(self.next())
        }
    }

    /// A plausible value most of the time, any f64 one time in 40.
    fn mostly(&mut self, plausible: f64) -> f64 {
        if self.one_in(40) {
            self.any_f64()
        } else {
            plausible
        }
    }

    /// Three positive parts of `total`.
    fn split(&mut self, total: f64) -> [f64; 3] {
        let parts = [
            self.uniform(0.05, 1.0),
            self.uniform(0.05, 1.0),
            self.uniform(0.05, 1.0),
        ];
        let sum: f64 = parts.iter().sum();
        parts.map(|p| total * p / sum)
    }
}

fn range(random: &mut Random) -> Range {
    let hfreq_min_hz = random.uniform(14_000.0, 120_000.0);
    let hfreq_max_hz = hfreq_min_hz
        + if random.one_in(4) {
            0.0
        } else {
            random.uniform(0.0, 60_000.0)
        };
    let vfreq_min_hz = random.uniform(40.0, 150.0);
    let vfreq_max_hz = vfreq_min_hz
        + if random.one_in(4) {
            0.0
        } else {
            random.uniform(0.0, 60.0)
        };
    let line_us = 1_000_000.0 / hfreq_max_hz;
    let field_ms = 1_000.0 / vfreq_max_hz;
    let h_fraction = if random.one_in(10) {
        random.uniform(0.7, 1.2)
    } else {
        random.uniform(0.0001, 0.75)
    };
    let v_fraction = if random.one_in(10) {
        random.uniform(0.4, 1.2)
    } else {
        random.uniform(0.0001, VBLANK_MAX_FRACTION)
    };
    let [hfront, hsync, hback] = random.split(line_us * h_fraction);
    let [vfront, vsync, vback] = random.split(field_ms * v_fraction);
    let vblank_s = (vfront + vsync + vback) / 1_000.0;
    // The most lines that monitor_evaluate_range lets fit, so that most
    // ranges pass it.
    let fit =
        (hfreq_max_hz / vfreq_min_hz - hfreq_max_hz * vblank_s - 1.0).clamp(129.0, 13_000.0) as u16;
    let (progressive_lines_min, progressive_lines_max, interlaced_lines_min, interlaced_lines_max) =
        if random.one_in(8) {
            (random.u16(), random.u16(), random.u16(), random.u16())
        } else {
            let p_min = if random.one_in(4) {
                0
            } else {
                random.u16_in(128, fit)
            };
            let p_max = random.u16_in(p_min, fit);
            if random.flag() {
                let i_min = p_max.max(256);
                (
                    p_min,
                    p_max,
                    i_min,
                    random.u16_in(i_min, fit.saturating_mul(2)),
                )
            } else {
                (p_min, p_max, 0, 0)
            }
        };
    Range {
        hfreq_min_hz: random.mostly(hfreq_min_hz),
        hfreq_max_hz: random.mostly(hfreq_max_hz),
        vfreq_min_hz: random.mostly(vfreq_min_hz),
        vfreq_max_hz: random.mostly(vfreq_max_hz),
        hfront_porch_us: random.mostly(hfront),
        hsync_pulse_us: random.mostly(hsync),
        hback_porch_us: random.mostly(hback),
        vfront_porch_ms: random.mostly(vfront),
        vsync_pulse_ms: random.mostly(vsync),
        vback_porch_ms: random.mostly(vback),
        hsync_positive: random.flag(),
        vsync_positive: random.flag(),
        progressive_lines_min,
        progressive_lines_max,
        interlaced_lines_min,
        interlaced_lines_max,
    }
}

fn settings(random: &mut Random) -> Settings {
    let plausible_aspect = if random.flag() { 4.0 / 3.0 } else { 16.0 / 9.0 };
    Settings {
        pclock_min_hz: if random.flag() {
            0
        } else {
            (random.next() >> 32) as u32
        },
        monitor_aspect: if random.one_in(4) {
            random.uniform(0.0, 5.0)
        } else {
            random.mostly(plausible_aspect)
        },
        refresh_tolerance_hz: random.mostly(2.0),
        interlace: random.flag(),
        doublescan: random.flag(),
        super_width: random.u16(),
        pixel_precision: random.flag(),
        interlace_force_even: random.flag(),
        scale_proportional: random.flag(),
    }
}

fn request(random: &mut Random) -> Request {
    let plausible_refresh = if random.one_in(10) {
        random.uniform(300.0, 10_000_000.0)
    } else {
        random.uniform(0.001, 300.0)
    };
    Request {
        refresh_hz: random.mostly(plausible_refresh),
        width: if random.one_in(4) {
            random.u16()
        } else {
            random.u16_in(1, 4_000)
        },
        height: if random.one_in(4) {
            random.u16()
        } else {
            random.u16_in(1, 1_300)
        },
        interlaced: random.flag(),
        rotated: random.flag(),
    }
}

#[derive(Debug, Default)]
struct Counts {
    modes: usize,
    in_range: usize,
    refused_range: usize,
    refused_blanking: usize,
    refused_request: usize,
    refused_settings: usize,
}

fn run(seed: u64, calls: usize) -> Counts {
    let mut random = Random(seed);
    let mut counts = Counts::default();
    for _ in 0..calls {
        let count = 1 + (random.next() % 3) as usize;
        let ranges: Vec<Range> = (0..count).map(|_| range(&mut random)).collect();
        let settings = settings(&mut random);
        let request = request(&mut random);
        match modelines(&ranges, &settings, &request) {
            Ok(modes) => {
                assert!(modes.len() <= ranges.len().min(MAX_RANGES));
                counts.in_range += 1;
                counts.modes += modes.len();
            }
            Err(CallError::InvalidRange { .. }) => counts.refused_range += 1,
            Err(CallError::Blanking { .. }) => counts.refused_blanking += 1,
            Err(CallError::InvalidRequest { .. }) => counts.refused_request += 1,
            Err(CallError::InvalidSettings { .. }) => counts.refused_settings += 1,
            Err(other) => panic!("{other:?} for {ranges:?} {settings:?} {request:?}"),
        }
    }
    counts
}

#[test]
fn random_inputs_always_return() {
    let counts = run(0x9e37_79b9_7f4a_7c15, 50_000);
    // The seed reaches every outcome, so each check and the calculation
    // all run.
    assert!(counts.in_range > 10_000, "{counts:?}");
    assert!(counts.refused_blanking > 1_000, "{counts:?}");
    assert!(counts.refused_range > 1_000, "{counts:?}");
    assert!(counts.refused_request > 1_000, "{counts:?}");
    assert!(counts.refused_settings > 1_000, "{counts:?}");
    assert!(counts.modes >= counts.in_range, "{counts:?}");
}

/// The larger run: `cargo xtask test -p neptron-switchres-sys --run-ignored
/// only`.
#[test]
#[ignore = "about a minute; run on demand"]
fn two_million_random_calls_always_return() {
    let counts = run(0x2545_f491_4f6c_dd1d, 2_000_000);
    assert!(counts.in_range > 400_000, "{counts:?}");
}
