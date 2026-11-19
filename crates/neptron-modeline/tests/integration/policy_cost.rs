//! The cost of the answers of the picture menus, printed, not checked: a
//! wall clock gives no stable pass or fail. Run it alone with
//! `cargo xtask test -p neptron-modeline --run-ignored ignored-only
//! --no-capture policy_cost`.

use std::time::Instant;

use neptron_modeline::policy::{
    PictureChoices, PolicySettings, REFERENCE_SOURCES, ScanlineChoice, decide, for_every_game,
    picture_choices,
};
use neptron_safety::validate;
use neptron_types::{GpuCaps, MonitorProfile, SourceGeometry};

use crate::presets::{gpu_table, preset, source};

const RUNS: u32 = 5;

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

#[test]
#[ignore = "a measurement, not a check"]
fn the_cost_of_the_picture_answers() {
    for (id, gpu) in [("generic_15", "pi_vc4_dpi"), ("pc_31_120", "intel_gen9")] {
        let profile = preset(id);
        let caps = caps(gpu);
        let snes = source(256, 224, 60_099);
        let start = Instant::now();
        for _ in 0..RUNS {
            std::hint::black_box(decide(&snes, &PolicySettings::default(), &profile, &caps).ok());
        }
        let one = start.elapsed() / RUNS;
        let start = Instant::now();
        for _ in 0..RUNS {
            std::hint::black_box(answers(&snes, &profile, &caps));
        }
        let game = start.elapsed() / RUNS;
        let start = Instant::now();
        for _ in 0..RUNS {
            let per: Vec<PictureChoices> = REFERENCE_SOURCES
                .iter()
                .map(|s| answers(s, &profile, &caps))
                .collect();
            std::hint::black_box(for_every_game(&per));
        }
        let every = start.elapsed() / RUNS;
        println!("{id} with {gpu}: one plan {one:?}, one game {game:?}, every game {every:?}");
    }
}
