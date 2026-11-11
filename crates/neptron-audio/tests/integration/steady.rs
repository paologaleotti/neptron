//! Long runs at fixed rates: the queue stays inside its bounds with no
//! underrun and no overrun.
//!
//! Each scenario runs 120 simulated seconds with the polynomial resampler.
//! The law's time constant is 6.4 s, so 120 s is about 19 time constants:
//! the queue has reached its fixed point long before the end, and a longer
//! run proves nothing more. One ten-minute run, the full length of timing
//! check 6, is ignored by default and runs on demand. The sinc filter costs
//! 21% of real time in the dev profile; it runs in `signal` and `events`
//! for the core's own rate, the device 0.1% fast and slow, a forced rate
//! and the mode switch. The law sees only frame counts, which both
//! resamplers give alike up to the constant filter delay.

use neptron_audio::Resampling;

use crate::harness::{EXACT_HIGH, EXACT_LOW, HIGH, LOW, Session, Setup, assert_steady};

/// About 19 time constants of the law.
const RUN_S: u64 = 120;
/// The full length of timing check 6.
const TEN_MINUTES_S: u64 = 600;
/// Bounds when the pipeline is not told a display 0.2% off.
const UNTOLD_LOW: f64 = 0.15;
const UNTOLD_HIGH: f64 = 0.85;

fn run(setup: Setup, seconds: u64, low: f64, high: f64) {
    let mut session = Session::new(setup);
    session.run_seconds(seconds);
    let measured = assert_steady(&session, low, high);
    println!(
        "{:?} {seconds} s, device {:+} ppm, display {:.4} Hz told {}: levels {:.3} to {:.3} of the span, \
         deviation {:.5} (after settling {:.5})",
        setup.resampling,
        setup.device_ppm,
        setup.display.as_f64(),
        setup.tell_display,
        measured.low,
        measured.high,
        measured.max_deviation,
        measured.settled_deviation,
    );
}

fn device_off(ppm: i64, resampling: Resampling) -> Setup {
    Setup {
        device_ppm: ppm,
        ..Setup::snes(resampling)
    }
}

fn display_off(ppm: i64, tell_display: bool, resampling: Resampling) -> Setup {
    let snes = Setup::snes(resampling);
    Setup {
        display: snes.core_fps.skewed(ppm),
        tell_display,
        ..snes
    }
}

#[test]
fn snes_at_its_own_rate() {
    run(
        Setup::snes(Resampling::Polynomial),
        RUN_S,
        EXACT_LOW,
        EXACT_HIGH,
    );
}

#[test]
#[ignore = "the full ten minutes of timing check 6, about 6 s in the dev profile; \
            120 s already reach the fixed point"]
fn snes_at_its_own_rate_for_ten_minutes() {
    run(
        Setup::snes(Resampling::Polynomial),
        TEN_MINUTES_S,
        LOW,
        HIGH,
    );
}

#[test]
fn a_device_0_1_percent_fast() {
    run(device_off(1_000, Resampling::Polynomial), RUN_S, LOW, HIGH);
}

#[test]
fn a_device_0_1_percent_slow() {
    run(device_off(-1_000, Resampling::Polynomial), RUN_S, LOW, HIGH);
}

#[test]
fn a_display_0_2_percent_fast() {
    run(
        display_off(2_000, true, Resampling::Polynomial),
        RUN_S,
        EXACT_LOW,
        EXACT_HIGH,
    );
}

#[test]
fn a_display_0_2_percent_slow() {
    run(
        display_off(-2_000, true, Resampling::Polynomial),
        RUN_S,
        EXACT_LOW,
        EXACT_HIGH,
    );
}

#[test]
fn a_display_0_2_percent_fast_not_told() {
    let setup = display_off(2_000, false, Resampling::Polynomial);
    run(setup, RUN_S, UNTOLD_LOW, UNTOLD_HIGH);
}

#[test]
fn a_display_0_2_percent_slow_not_told() {
    let setup = display_off(-2_000, false, Resampling::Polynomial);
    run(setup, RUN_S, UNTOLD_LOW, UNTOLD_HIGH);
}

#[test]
fn the_same_run_gives_the_same_samples() {
    for (resampling, seconds) in [(Resampling::Polynomial, 5), (Resampling::Sinc, 3)] {
        let hashes: Vec<u64> = (0..2)
            .map(|_| {
                let mut session = Session::new(Setup::snes(resampling));
                session.run_seconds(seconds);
                session.audio.sample_hash()
            })
            .collect();
        assert_eq!(hashes[0], hashes[1]);
        println!("{resampling:?} {seconds} s: sample hash {:016x}", hashes[0]);
    }
}
