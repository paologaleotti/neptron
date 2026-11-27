//! The policy for 480i and 480p systems (docs/spec/interlaced-systems.md):
//! the wide mode of a system that changes its width.

use neptron_modeline::policy::{
    GameWidths, HorizontalScale, HorizontalStrategy, PolicySettings, Proposal, REFRESH_MATCH_MHZ,
    Rung, choose, decide,
};
use neptron_safety::validate;
use neptron_types::{GpuCaps, Modeline, MonitorProfile, SourceGeometry, TvStandard};

use crate::presets::{all_presets, gpu_table, open_caps, panel_1080, preset, source};

/// The widths of a PlayStation game, the most used first. Its system file
/// comes with its core (task S10); until then the list lives here.
const PLAYSTATION_WIDTHS: [u16; 5] = [320, 256, 640, 512, 368];
const PSX_240P_MHZ: u32 = 59_826;

fn caps(id: &str) -> GpuCaps {
    gpu_table()
        .into_iter()
        .find(|(gpu, _)| gpu == id)
        .map(|(_, caps)| caps)
        .unwrap()
}

fn playstation() -> PolicySettings {
    PolicySettings {
        horizontal: Some(HorizontalStrategy::Wide {
            widths: GameWidths::from_slice(&PLAYSTATION_WIDTHS).unwrap(),
        }),
        ..PolicySettings::default()
    }
}

/// The winner after the validator, with its rung.
struct Won {
    rung: Rung,
    proposal: Proposal,
}

fn won_with(
    game: &SourceGeometry,
    settings: &PolicySettings,
    profile: &MonitorProfile,
    caps: &GpuCaps,
    mut refuse: impl FnMut(&Modeline) -> bool,
) -> Won {
    let plan = decide(game, settings, profile, caps).unwrap();
    let choice = choose(&plan, |m| {
        if refuse(m) {
            return Err(None);
        }
        validate(m, profile, caps).map_err(Some)
    });
    let w = choice
        .winner
        .unwrap_or_else(|| panic!("{} {game:?}: no winner", profile.id));
    Won {
        rung: w.considered.rung,
        proposal: *w.proposal,
    }
}

fn won(
    game: &SourceGeometry,
    settings: &PolicySettings,
    profile: &MonitorProfile,
    caps: &GpuCaps,
) -> Won {
    won_with(game, settings, profile, caps, |_| false)
}

#[test]
fn every_playstation_width_shares_one_wide_mode_on_a_crt() {
    for id in ["generic_15", "arcade_15", "pc_31_120", "arcade_31", "d9800"] {
        let profile = preset(id);
        for gpu in ["intel_gen9", "pi_vc4_dpi", "pi5_rp1_dpi", "unknown"] {
            let caps = caps(gpu);
            let modes: Vec<(u16, Proposal)> = PLAYSTATION_WIDTHS
                .iter()
                .map(|&w| {
                    let game = source(w, 240, PSX_240P_MHZ);
                    (w, won(&game, &playstation(), &profile, &caps).proposal)
                })
                .collect();
            let first = modes[0].1.mode;
            for (width, proposal) in &modes {
                assert_eq!(proposal.mode, first, "{id} {gpu} width {width}");
                assert_eq!(
                    proposal.horizontal,
                    Some(HorizontalScale::Fixed(1_280)),
                    "{id} {gpu} width {width}"
                );
                assert_eq!(proposal.placement.dest.w, u32::from(first.hactive));
                let whole = 1_280 % width == 0;
                assert_eq!(
                    proposal.placement.scale.x,
                    whole.then(|| 1_280 / width),
                    "{id} {gpu} width {width}"
                );
            }
            assert_eq!(first.hactive, 1_280, "{id} {gpu}");
        }
    }
}

#[test]
fn the_wide_mode_keeps_the_lines_and_the_rate_of_the_mode_of_today() {
    for id in ["generic_15", "pc_31_120"] {
        let profile = preset(id);
        let caps = caps("intel_gen9");
        for &width in &PLAYSTATION_WIDTHS {
            let game = source(width, 240, PSX_240P_MHZ);
            let wide = won(&game, &playstation(), &profile, &caps);
            let today = won(&game, &PolicySettings::default(), &profile, &caps);
            assert_eq!(wide.rung, today.rung, "{id} {width}");
            assert_eq!(wide.proposal.mode.vactive, today.proposal.mode.vactive);
            assert_eq!(
                wide.proposal.mode.field_rate_mhz(),
                today.proposal.mode.field_rate_mhz(),
                "{id} {width}"
            );
            assert_eq!(
                wide.proposal.placement.dest.h,
                today.proposal.placement.dest.h
            );
            assert_eq!(
                wide.proposal.placement.scale.y,
                today.proposal.placement.scale.y
            );
        }
    }
}

#[test]
fn a_wide_mode_the_validator_refuses_falls_back_to_the_mode_of_today() {
    let profile = preset("generic_15");
    let caps = caps("intel_gen9");
    for &width in &PLAYSTATION_WIDTHS {
        let game = source(width, 240, PSX_240P_MHZ);
        let today = won(&game, &PolicySettings::default(), &profile, &caps);
        let wide_refused = won_with(&game, &playstation(), &profile, &caps, |m| {
            m.hactive == 1_280 && today.proposal.mode.hactive != 1_280
        });
        if today.proposal.mode.hactive != 1_280 {
            assert_eq!(wide_refused.proposal, today.proposal, "width {width}");
        }
    }
}

#[test]
fn a_gpu_with_no_valid_wide_mode_keeps_the_mode_of_today() {
    let profile = preset("generic_15");
    // The SNES base is 512; 1024 is below the pixel clock floor at 15 kHz
    // and 1536 is wider than this GPU, so no wide mode exists.
    let narrow = GpuCaps {
        max_width: 1_400,
        ..caps("intel_gen9")
    };
    let snes = PolicySettings {
        horizontal: Some(HorizontalStrategy::Wide {
            widths: GameWidths::from_slice(&[256, 512]).unwrap(),
        }),
        ..PolicySettings::default()
    };
    let game = source(256, 224, 60_099);
    let wide = won(&game, &snes, &profile, &narrow);
    let today = won(&game, &PolicySettings::default(), &profile, &narrow);
    assert_eq!(wide.proposal, today.proposal);
    assert_eq!(wide.proposal.mode.hactive, 1_280);
}

#[test]
fn a_width_the_system_does_not_name_keeps_the_mode_of_today() {
    // Genesis Plus GX with its overscan option on sends 348 pixels, which
    // the Mega Drive list does not name.
    let mega_drive = PolicySettings {
        horizontal: Some(HorizontalStrategy::Wide {
            widths: GameWidths::from_slice(&[320, 256]).unwrap(),
        }),
        ..PolicySettings::default()
    };
    for id in ["generic_15", "pc_31_120"] {
        let profile = preset(id);
        let intel = caps("intel_gen9");
        let game = source(348, 224, 59_922);
        let wide = won(&game, &mega_drive, &profile, &intel);
        let today = won(&game, &PolicySettings::default(), &profile, &intel);
        assert_eq!(wide.proposal, today.proposal, "{id}");
        assert!(
            wide.proposal.placement.scale.x.is_some(),
            "{id}: a whole factor"
        );
    }
}

#[test]
fn the_wide_mode_changes_nothing_on_a_tv_or_a_panel() {
    let composite = caps("pi_vc4_composite");
    let digital = open_caps();
    let screens = [
        (TvStandard::Ntsc.profile(), composite),
        (TvStandard::Pal.profile(), composite),
        (panel_1080(), digital),
    ];
    for (profile, caps) in &screens {
        for &width in &PLAYSTATION_WIDTHS {
            let game = source(width, 240, PSX_240P_MHZ);
            let wide = won(&game, &playstation(), profile, caps);
            let today = won(&game, &PolicySettings::default(), profile, caps);
            assert_eq!(wide.proposal, today.proposal, "{} {width}", profile.id);
        }
    }
}

/// The systems of `data/systems` that change their width in play, with
/// the widths of their files and their 60 Hz frame.
const WIDTH_SYSTEMS: [(&str, [u16; 2], u16, u32); 2] = [
    ("snes", [256, 512], 224, 60_099),
    ("megadrive", [320, 256], 224, 59_922),
];

/// The winner of `game`, or `None` when the display has no picture.
fn winner(
    game: &SourceGeometry,
    settings: &PolicySettings,
    profile: &MonitorProfile,
    caps: &GpuCaps,
) -> Option<Proposal> {
    let plan = decide(game, settings, profile, caps).ok()?;
    let choice = choose(&plan, |m| validate(m, profile, caps));
    choice.winner.map(|w| *w.proposal)
}

/// On every preset with every GPU of the matrix that drives a CRT, each
/// width of the SNES and the Mega Drive is either a whole factor of one
/// shared mode, at the lines and the rate of today, or, where no wide mode
/// is valid, the mode of today. Prints the cases that keep the mode of
/// today.
#[test]
fn every_width_of_a_system_is_a_whole_factor_of_one_mode_or_the_mode_of_today() {
    let gpus = [
        "intel_gen9",
        "amd_amdgpu",
        "pi_vc4_dpi",
        "pi5_rp1_dpi",
        "unknown",
    ];
    for (system, widths, lines, rate) in WIDTH_SYSTEMS {
        let settings = PolicySettings {
            horizontal: Some(HorizontalStrategy::Wide {
                widths: GameWidths::from_slice(&widths).unwrap(),
            }),
            ..PolicySettings::default()
        };
        for profile in all_presets() {
            for gpu in gpus {
                let caps = caps(gpu);
                let shown: Vec<_> = widths
                    .iter()
                    .map(|&w| {
                        let game = source(w, lines, rate);
                        (
                            w,
                            winner(&game, &settings, &profile, &caps),
                            winner(&game, &PolicySettings::default(), &profile, &caps),
                        )
                    })
                    .collect();
                let shared = shown
                    .iter()
                    .all(|(_, wide, _)| wide.map(|p| p.mode) == shown[0].1.map(|p| p.mode));
                let whole = shown.iter().all(|(w, wide, _)| {
                    wide.is_some_and(|p| {
                        p.mode.hactive % w == 0 && p.placement.scale.x == Some(p.mode.hactive / w)
                    })
                });
                if shared && whole {
                    for (w, wide, today) in &shown {
                        let (wide, today) = (wide.unwrap(), today.unwrap());
                        let at = format!("{system} {w} {} {gpu}", profile.id);
                        assert_eq!(wide.mode.vactive, today.mode.vactive, "{at}");
                        assert_eq!(wide.mode.interlace, today.mode.interlace, "{at}");
                        // The whole-kHz clock of another width can move the
                        // rate by a few millihertz.
                        let drift = wide
                            .mode
                            .field_rate_mhz()
                            .abs_diff(today.mode.field_rate_mhz());
                        assert!(drift <= REFRESH_MATCH_MHZ, "{at}: {drift}");
                        assert_eq!(wide.placement.dest.h, today.placement.dest.h, "{at}");
                    }
                    continue;
                }
                for (w, wide, today) in &shown {
                    assert_eq!(
                        wide, today,
                        "{system} {w} {} {gpu}: no wide mode, so the mode of today",
                        profile.id
                    );
                }
                println!("today's mode: {system} on {} with {gpu}", profile.id);
            }
        }
    }
}
