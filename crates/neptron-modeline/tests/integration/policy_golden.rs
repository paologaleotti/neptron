//! The golden list of systems run through the policy: the profile goldens
//! in tests/golden/policy.toml, timing check 4, the per-GPU goldens in
//! tests/golden/policy_gpus.toml, and the TV goldens in
//! tests/golden/policy_tv.toml. The file headers say where the expected
//! values come from.

use std::collections::BTreeMap;
use std::path::Path;

use neptron_modeline::policy::{PolicySettings, RefreshKind, TvWidth, choose, decide};
use neptron_safety::validate;
use neptron_types::{Filter, GpuCaps, MonitorProfile, Rotation, SourceGeometry, TvStandard};
use serde::Deserialize;

use crate::arcade_table;
use crate::golden::golden;
use crate::presets::{edid_caps, gpu_table, open_caps, panel_1080, pc_crt_30_71, preset};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    case: BTreeMap<String, String>,
}

fn expected(file: &str) -> BTreeMap<String, String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden")
        .join(file);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let parsed: Expected =
        toml::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    parsed.case
}

/// The display aspect that the core of a handheld reports: square pixels
/// of its screen (Gambatte 10:9, mGBA 3:2, Beetle Cygne 14:9). Every other
/// system of the list is 4:3, the Game Gear too: its core reports 4:3 for
/// the 160x144 window of a 4:3 Master System picture.
fn aspect_milli(name: &str) -> u32 {
    match name {
        "game_boy" => 1_111,
        "gba" => 1_500,
        "wonderswan" => 1_556,
        _ => 1_333,
    }
}

fn systems() -> Vec<(String, SourceGeometry)> {
    golden()
        .system
        .iter()
        .map(|s| {
            (
                s.name.clone(),
                SourceGeometry {
                    width: s.width,
                    height: s.height,
                    refresh_mhz: s.refresh_mhz,
                    aspect_milli: aspect_milli(&s.name),
                    interlaced: s.interlaced,
                    rotation: s.rotation,
                },
            )
        })
        .collect()
}

/// The winner for a source with the default settings (`auto`, `exact`, the
/// GPU's horizontal strategy), in one line: rung, refresh kind, mode, line
/// rate, field rate, the game's rectangle and the filter, then "crop N"
/// when an overscan crop hides N source lines at each edge. "none" when no
/// proposal validates.
fn outcome(source: &SourceGeometry, profile: &MonitorProfile, caps: &GpuCaps) -> String {
    outcome_with(source, &PolicySettings::default(), profile, caps)
}

/// [`outcome`] with other settings. A winner that changes the game speed
/// ends with "speed".
fn outcome_with(
    source: &SourceGeometry,
    settings: &PolicySettings,
    profile: &MonitorProfile,
    caps: &GpuCaps,
) -> String {
    let plan =
        decide(source, settings, profile, caps).unwrap_or_else(|e| panic!("{}: {e}", profile.id));
    let choice = choose(&plan, |m| validate(m, profile, caps));
    let Some(w) = choice.winner else {
        return "none".into();
    };
    let p = w.proposal;
    let m = &p.mode;
    // The rule of every display backend: the kernel's mode record holds
    // the clock in kHz, and a backend refuses any other clock.
    assert!(
        m.pclock_is_whole_khz(),
        "{}: the winner's pixel clock {} Hz is not whole kHz",
        profile.id,
        m.pclock_hz
    );
    let d = &p.placement.dest;
    let crop = p
        .overscan_crop
        .map(|c| format!(" crop {}", c.lines_per_edge))
        .unwrap_or_default();
    let speed = if p.timing.changes_speed() && w.considered.target.kind == RefreshKind::Tv {
        " speed"
    } else {
        ""
    };
    // Each frame twice at a rate near twice the game's: the kind alone
    // does not say it.
    let twice = if w.considered.target.kind == RefreshKind::Nearest && p.timing.repeat() == 2 {
        " x2"
    } else {
        ""
    };
    format!(
        "{} {} {}x{}{} {}.{:03}kHz {}.{:03}Hz game {}x{}+{}+{} {}{crop}{speed}{twice}",
        w.considered.rung,
        w.considered.target.kind,
        m.hactive,
        m.vactive,
        if m.interlace { "i" } else { "p" },
        p.timing.line_rate_mhz / 1_000_000,
        p.timing.line_rate_mhz % 1_000_000 / 1_000,
        p.timing.field_rate_mhz / 1_000,
        p.timing.field_rate_mhz % 1_000,
        d.w,
        d.h,
        d.x,
        d.y,
        match p.placement.filter {
            Filter::Nearest => "nearest",
            Filter::SharpBilinear => "sharp",
        }
    )
}

/// Compares every case with the file and lists each difference, missing
/// case and extra case as a line of the file.
fn compare(file: &str, actual: BTreeMap<String, String>) {
    let expected = expected(file);
    let mut diffs = Vec::new();
    for (key, got) in &actual {
        match expected.get(key) {
            Some(want) if want == got => {}
            Some(want) => diffs.push(format!("# was: {want}\n\"{key}\" = \"{got}\"")),
            None => diffs.push(format!("# missing\n\"{key}\" = \"{got}\"")),
        }
    }
    for key in expected.keys().filter(|k| !actual.contains_key(*k)) {
        diffs.push(format!("# extra case in the file: {key}"));
    }
    assert!(
        diffs.is_empty(),
        "{} of {} cases differ from {file}:\n{}",
        diffs.len(),
        actual.len(),
        diffs.join("\n")
    );
}

#[test]
fn every_system_on_each_profile_gives_its_golden_decision() {
    let profiles: Vec<(MonitorProfile, GpuCaps)> = vec![
        (preset("generic_15"), open_caps()),
        (preset("arcade_15"), open_caps()),
        (preset("pc_31_120"), open_caps()),
        (preset("arcade_31"), open_caps()),
        (pc_crt_30_71(), edid_caps(open_caps())),
        (panel_1080(), open_caps()),
    ];
    let mut actual = BTreeMap::new();
    for (name, source) in systems() {
        for (profile, caps) in &profiles {
            let key = format!("{name} {}", profile.id);
            actual.insert(key, outcome(&source, profile, caps));
        }
    }
    compare("policy.toml", actual);
}

/// Timing check 4: the suite reruns for each entry of the GPU capability
/// table, on a 15 kHz TV and on the EDID profile.
#[test]
fn every_gpu_of_the_capability_table_gives_its_golden_decisions() {
    let names = [
        "nes_ntsc",
        "snes_ntsc",
        "md_ntsc_h40",
        "md_pal_h40",
        "game_boy",
        "neogeo",
        "vga_640x480",
        "ntsc_480i",
    ];
    let systems: Vec<(String, SourceGeometry)> = systems()
        .into_iter()
        .filter(|(n, _)| names.contains(&n.as_str()))
        .collect();
    assert_eq!(systems.len(), names.len());
    let mut actual = BTreeMap::new();
    for (gpu, caps) in gpu_table() {
        for (profile, caps) in [
            (preset("generic_15"), caps),
            (pc_crt_30_71(), edid_caps(caps)),
        ] {
            for (name, source) in &systems {
                let key = format!("{gpu} {} {name}", profile.id);
                actual.insert(key, outcome(source, &profile, &caps));
            }
        }
    }
    compare("policy_gpus.toml", actual);
}

/// The handhelds. Their TV width comes from their own aspect, which
/// `policy_tv.rs` tests with each rule; the TV goldens hold the systems of
/// a 4:3 picture.
const NOT_FOUR_THIRDS: [&str; 4] = ["game_boy", "game_gear", "gba", "wonderswan"];

/// Every system on NTSC and on PAL, with each width rule, on the composite
/// encoder of the Raspberry Pi. Each case is "<system> <profile>
/// <tv_width>".
#[test]
fn every_system_on_a_tv_gives_its_golden_decision() {
    let caps = gpu_table()
        .into_iter()
        .find(|(id, _)| id == "pi_vc4_composite")
        .map(|(_, caps)| caps)
        .unwrap();
    let mut actual = BTreeMap::new();
    for (name, source) in systems() {
        if NOT_FOUR_THIRDS.contains(&name.as_str()) {
            continue;
        }
        for standard in [TvStandard::Ntsc, TvStandard::Pal] {
            let profile = standard.profile();
            for tv_width in TvWidth::ALL {
                let settings = PolicySettings {
                    tv_width,
                    ..PolicySettings::default()
                };
                let key = format!("{name} {} {tv_width}", profile.id);
                actual.insert(key, outcome_with(&source, &settings, &profile, &caps));
            }
        }
    }
    compare("policy_tv.toml", actual);
}

/// The arcade boards of `arcade_table` on every display of its table: each
/// cell whose winner follows the arcade rule of docs/spec/arcade.md is a
/// golden in tests/golden/policy_arcade.toml, and the file holds no other
/// cell. A cell that breaks the rule is work for the policy, not a golden.
#[test]
fn every_arcade_board_that_follows_the_rule_gives_its_golden_decision() {
    let displays = arcade_table::displays();
    let mut actual = BTreeMap::new();
    for board in &arcade_table::BOARDS {
        for display in &displays {
            if !arcade_table::cell(board, display).rule.starts_with("ok") {
                continue;
            }
            let key = format!("{} {}", board.name, display.profile.id);
            let got = outcome_with(
                &board.source(),
                &arcade_table::settings(),
                &display.profile,
                &display.caps,
            );
            actual.insert(key, got);
        }
    }
    compare("policy_arcade.toml", actual);
}

/// A turn of 270 degrees gives the same decision as a turn of 90, and a
/// turn of 180 the same as none: the policy sees the upright picture, and
/// the renderer makes the turn.
#[test]
fn the_direction_of_a_turn_does_not_change_the_decision() {
    let displays = arcade_table::displays();
    for board in &arcade_table::BOARDS {
        let pair = match board.rotation {
            Rotation::R0 | Rotation::R180 => [Rotation::R0, Rotation::R180],
            Rotation::R90 | Rotation::R270 => [Rotation::R90, Rotation::R270],
        };
        let [a, b] = pair.map(|rotation| SourceGeometry {
            rotation,
            ..board.source()
        });
        for display in &displays {
            let (profile, caps) = (&display.profile, &display.caps);
            let settings = arcade_table::settings();
            assert_eq!(
                outcome_with(&a, &settings, profile, caps),
                outcome_with(&b, &settings, profile, caps),
                "{} on {}",
                board.name,
                profile.id
            );
        }
    }
}

/// The wider table: every source of `arcade_table::SOURCES` (consoles,
/// handhelds, computers, the arcade rate extremes) on every display, each
/// cell that follows the rule or carries an allowed reason, in
/// tests/golden/policy_sources.toml.
#[test]
fn every_source_of_the_wider_table_gives_its_golden_decision() {
    let displays = arcade_table::displays();
    let mut actual = BTreeMap::new();
    for board in &arcade_table::SOURCES {
        for display in &displays {
            if !arcade_table::cell(board, display).rule.starts_with("ok") {
                continue;
            }
            let key = format!("{} {}", board.name, display.profile.id);
            let got = outcome_with(
                &board.source(),
                &arcade_table::settings_of(board),
                &display.profile,
                &display.caps,
            );
            actual.insert(key, got);
        }
    }
    compare("policy_sources.toml", actual);
}

/// A source of the wider table that is a system of systems.toml, with
/// its geometry and 4:3, gives on every profile of policy.toml the
/// decision that policy.toml records.
#[test]
fn the_wider_table_agrees_with_the_console_goldens() {
    let recorded = expected("policy.toml");
    let systems = systems();
    let displays = arcade_table::displays();
    let mut compared = 0;
    for board in &arcade_table::SOURCES {
        let Some((_, system)) = systems.iter().find(|(name, _)| name == board.name) else {
            continue;
        };
        if *system != board.source() {
            continue;
        }
        for display in &displays {
            let key = format!("{} {}", board.name, display.profile.id);
            let Some(want) = recorded.get(&key) else {
                continue;
            };
            let got = outcome(&board.source(), &display.profile, &display.caps);
            assert_eq!(&got, want, "{key}");
            compared += 1;
        }
    }
    assert!(compared >= 40, "{compared} cases compared");
}

/// Every system on the 1440p and 4K panels, with the scanlines of the
/// panel off and on. Scanlines need a whole vertical scale, so with them
/// on the panel takes the largest whole multiple and never the fill. Each
/// case is "<system> <panel> scanlines <off|on>", in
/// tests/golden/policy_panel_scanlines.toml.
#[test]
fn panels_with_and_without_scanlines_give_their_golden_decisions() {
    let panels: Vec<_> = arcade_table::panels()
        .into_iter()
        .filter(|d| ["panel_1440", "panel_2160"].contains(&d.profile.id.as_str()))
        .collect();
    assert_eq!(panels.len(), 2);
    let mut actual = BTreeMap::new();
    for (name, source) in systems() {
        for panel in &panels {
            for on in [false, true] {
                let settings = PolicySettings {
                    panel_scanlines: on,
                    ..PolicySettings::default()
                };
                let key = format!(
                    "{name} {} scanlines {}",
                    panel.profile.id,
                    if on { "on" } else { "off" }
                );
                let got = outcome_with(&source, &settings, &panel.profile, &panel.caps);
                if on {
                    let plan = decide(&source, &settings, &panel.profile, &panel.caps).unwrap();
                    let (_, _, p) = plan.proposals().next().unwrap();
                    assert!(p.placement.scale.y.is_some(), "{key}: {got}");
                }
                actual.insert(key, got);
            }
        }
    }
    compare("policy_panel_scanlines.toml", actual);
}
