//! Long runs at fixed rates: the queue stays inside its bounds with no
//! underrun and no overrun.
//!
//! Each scenario runs ten simulated minutes with the polynomial resampler.
//! The sinc filter costs 21% of real time in the dev profile, so it runs
//! 30 simulated seconds: here for the display scenarios, and in `signal`
//! for the core's own rate and the device 0.1% fast and slow. The law sees
//! only frame counts, which both resamplers give alike up to the constant
//! filter delay.

use neptron_audio::Resampling;

use crate::harness::{Session, Setup, assert_steady};

/// Ten simulated minutes.
const LONG_S: u64 = 600;
/// The sinc runs of the same scenarios.
const SINC_S: u64 = 30;
/// Bounds of the level before a write, as fractions of the span, when the
/// run rate is known.
const LOW: f64 = 0.25;
const HIGH: f64 = 0.75;
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
fn snes_at_its_own_rate_for_ten_minutes() {
    run(Setup::snes(Resampling::Polynomial), LONG_S, LOW, HIGH);
}

#[test]
fn a_device_0_1_percent_fast_for_ten_minutes() {
    run(device_off(1_000, Resampling::Polynomial), LONG_S, LOW, HIGH);
}

#[test]
fn a_device_0_1_percent_slow_for_ten_minutes() {
    run(
        device_off(-1_000, Resampling::Polynomial),
        LONG_S,
        LOW,
        HIGH,
    );
}

#[test]
fn a_display_0_2_percent_fast_for_ten_minutes() {
    run(
        display_off(2_000, true, Resampling::Polynomial),
        LONG_S,
        LOW,
        HIGH,
    );
}

#[test]
fn a_display_0_2_percent_slow_for_ten_minutes() {
    run(
        display_off(-2_000, true, Resampling::Polynomial),
        LONG_S,
        LOW,
        HIGH,
    );
}

#[test]
fn a_display_0_2_percent_off_with_the_sinc_filter() {
    run(
        display_off(2_000, true, Resampling::Sinc),
        SINC_S,
        LOW,
        HIGH,
    );
    run(
        display_off(-2_000, true, Resampling::Sinc),
        SINC_S,
        LOW,
        HIGH,
    );
}

#[test]
fn a_display_0_2_percent_fast_not_told_for_ten_minutes() {
    let setup = display_off(2_000, false, Resampling::Polynomial);
    run(setup, LONG_S, UNTOLD_LOW, UNTOLD_HIGH);
}

#[test]
fn a_display_0_2_percent_slow_not_told_for_ten_minutes() {
    let setup = display_off(-2_000, false, Resampling::Polynomial);
    run(setup, LONG_S, UNTOLD_LOW, UNTOLD_HIGH);
}

#[test]
fn a_display_0_2_percent_off_not_told_with_the_sinc_filter() {
    let fast = display_off(2_000, false, Resampling::Sinc);
    run(fast, SINC_S, UNTOLD_LOW, UNTOLD_HIGH);
    let slow = display_off(-2_000, false, Resampling::Sinc);
    run(slow, SINC_S, UNTOLD_LOW, UNTOLD_HIGH);
}

#[test]
fn the_same_run_gives_the_same_samples() {
    for (resampling, seconds) in [(Resampling::Polynomial, 60), (Resampling::Sinc, 10)] {
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
