//! Random rate pairs from a fixed seed: every pair stays inside the steady
//! bounds.

use neptron_audio::Resampling;

use crate::harness::{Hz, Session, Setup, assert_steady};

const SEED: u64 = 0x5EED_A0D1_0000_0017;
/// Cases per test; four tests run in parallel.
const CASES_PER_TEST: u64 = 3;
/// About 4.7 time constants of the law: the fixed point is reached.
const RUN_S: u64 = 30;

/// SplitMix64 of a seed and an index: the same draw in every run.
fn draw(index: u64) -> u64 {
    let mut z = SEED.wrapping_add(index.wrapping_mul(0x9E37_79B9_7F4A_7C15));
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// A value in `low..=high` from draw `index`.
fn pick(index: u64, low: u64, high: u64) -> u64 {
    low + draw(index) % (high - low + 1)
}

fn setup(case: u64) -> Setup {
    let base = case * 8;
    let core_fps = Hz::new(pick(base, 49_500, 61_000), 1_000);
    let devices = [32_000, 44_100, 48_000];
    Setup {
        core_rate: Hz::new(pick(base + 1, 22_050_000, 55_930_000), 1_000),
        core_fps,
        display: core_fps.skewed(pick(base + 2, 0, 4_000) as i64 - 2_000),
        device_hz: devices[(draw(base + 3) % 3) as usize],
        device_ppm: pick(base + 4, 0, 2_000) as i64 - 1_000,
        ..Setup::snes(Resampling::Polynomial)
    }
}

fn check(first: u64) {
    for case in first..first + CASES_PER_TEST {
        let setup = setup(case);
        let mut session = Session::new(setup);
        session.run_seconds(RUN_S);
        let measured = assert_steady(&session, 0.25, 0.75);
        println!(
            "core {:.3} Hz at {:.3} fps, display {:.4} Hz, device {} Hz {:+} ppm: levels {:.3} to {:.3}",
            setup.core_rate.as_f64(),
            setup.core_fps.as_f64(),
            setup.display.as_f64(),
            setup.device_hz,
            setup.device_ppm,
            measured.low,
            measured.high
        );
    }
}

#[test]
fn random_rate_pairs_0_to_2_stay_inside_the_bounds() {
    check(0);
}

#[test]
fn random_rate_pairs_3_to_5_stay_inside_the_bounds() {
    check(3);
}

#[test]
fn random_rate_pairs_6_to_8_stay_inside_the_bounds() {
    check(6);
}

#[test]
fn random_rate_pairs_9_to_11_stay_inside_the_bounds() {
    check(9);
}
