//! The cost of the answers of the picture menus, printed, not checked: a
//! wall clock gives no stable pass or fail. Run it alone with
//! `cargo xtask test -p neptron-modeline --run-ignored only --no-capture
//! policy_cost`.
//!
//! It measures each display class (a 15 kHz CRT, a 31 kHz CRT, a flat
//! panel and a TV) and the fallback, the dearest decision: the stored
//! mode, then `auto`, then `fit`, each plan walked to its end. The frame
//! makes at most `PICTURE_DECISIONS_PER_FRAME` (2) decisions; the line
//! "per frame" gives two of the dearest decisions of each display.

use std::time::{Duration, Instant};

use neptron_modeline::policy::{
    PictureChoices, PictureStyle, PolicySettings, REFERENCE_SOURCES, RefreshPolicy, ScanlineChoice,
    decide, decide_shown, for_every_game, picture_choices,
};
use neptron_safety::validate;
use neptron_types::{GpuCaps, Modeline, MonitorProfile, SourceGeometry};

use crate::presets::{gpu_table, panel_1080, preset, source};

const RUNS: u32 = 5;

/// The decisions of the answers of one source: each of the 8 picture
/// modes and 4 refresh choices, and at most one more each for the pair
/// with the other row at Auto.
const DECISIONS_PER_SOURCE_MAX: u32 = 2 * (8 + 4);

fn caps(id: &str) -> GpuCaps {
    gpu_table()
        .into_iter()
        .find(|(gpu, _)| gpu == id)
        .map(|(_, caps)| caps)
        .expect("a GPU of the matrix")
}

fn answers(source: &SourceGeometry, profile: &MonitorProfile, caps: &GpuCaps) -> PictureChoices {
    picture_choices(
        source,
        &PolicySettings::default(),
        ScanlineChoice::OFF,
        profile,
        caps,
        |m| validate(m, profile, caps).map(drop),
    )
    .expect("answers")
}

/// The mean time of `RUNS` runs of `work`.
fn timed(mut work: impl FnMut()) -> Duration {
    let start = Instant::now();
    for _ in 0..RUNS {
        work();
    }
    start.elapsed() / RUNS
}

#[test]
#[ignore = "a measurement, not a check"]
fn the_cost_of_the_picture_answers() {
    let displays = [
        ("generic_15", preset("generic_15"), "pi_vc4_dpi"),
        ("pc_31_120", preset("pc_31_120"), "intel_gen9"),
        ("panel_1080", panel_1080(), "intel_gen9"),
        ("ntsc", preset("ntsc"), "pi_vc4_composite"),
    ];
    for (id, profile, gpu) in displays {
        let caps = caps(gpu);
        let snes = source(256, 224, 60_099);
        let one = timed(|| {
            std::hint::black_box(decide(&snes, &PolicySettings::default(), &profile, &caps).ok());
        });
        // The fallback: Sharp lines with 50 Hz and a validator that takes
        // nothing, so the stored plan, then `auto`, then `fit` are walked
        // to their ends.
        let stored = PolicySettings {
            style: PictureStyle::Sharp,
            refresh: RefreshPolicy::Force50,
            ..PolicySettings::default()
        };
        let fallback = timed(|| {
            std::hint::black_box(
                decide_shown(&snes, &stored, &profile, &caps, |_: &Modeline| {
                    Err::<(), ()>(())
                })
                .ok(),
            );
        });
        let game = timed(|| {
            std::hint::black_box(answers(&snes, &profile, &caps));
        });
        let every = timed(|| {
            let per: Vec<PictureChoices> = REFERENCE_SOURCES
                .iter()
                .map(|s| answers(s, &profile, &caps))
                .collect();
            std::hint::black_box(for_every_game(&per));
        });
        let per_decision = game / DECISIONS_PER_SOURCE_MAX;
        let frame = 2 * one.max(fallback);
        println!(
            "{id} with {gpu}: one plan {one:?}, the fallback {fallback:?}, one game {game:?} \
             (at least {per_decision:?} a decision), every game {every:?}, per frame at most \
             {frame:?}"
        );
    }
}
