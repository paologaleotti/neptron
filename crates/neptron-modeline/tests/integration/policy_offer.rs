//! The one decision of what the menus offer (`offer`): every picture
//! mode, every refresh choice and every scanline setting, on every display
//! of the repository's monitor set and the source table, and on generated
//! sources and ranges. An offered candidate shows a validated mode inside
//! the profile; a refused candidate has a reason that the test finds again
//! by itself; a refused candidate that the display can still show (a
//! stored one) shows a validated mode inside the profile. The spec table
//! of section 8 is the output of the same function.

use std::collections::BTreeMap;
use std::path::Path;

use neptron_modeline::policy::{
    DeclineReason, Declined, Dropped, Offer, Offered, PictureStyle, Plan, PolicyError,
    PolicySettings, RefreshKind, RefreshPolicy, Rung, ScanlineChoice, ScanlineOutcome, choose,
    offer, picture_choices, walk_ladder,
};
use neptron_safety::validate;
use neptron_types::{
    CrtRange, GpuCaps, Modeline, MonitorKind, MonitorProfile, ScanlineMode, SourceGeometry,
    TvStandard,
};

use crate::arcade_table::{self, Board};
use crate::presets::{all_presets, gpu_table, open_caps, panel_1080, preset, source};

/// A display of the exhaustive run: its profile and its GPU.
struct Screen {
    profile: MonitorProfile,
    caps: GpuCaps,
}

/// Every preset of data/monitors on an open GPU and on the GPIO DPI output
/// of the Raspberry Pi 3 (`pi_vc4_dpi`: no interlace, 165 MHz), the EDID
/// tubes and panels of the arcade table, and every TV standard on the
/// composite encoder of the Pi (`pi_vc4_composite`). A Pi drives a flat
/// panel over HDMI through the panel path, whose modes come from the
/// panel's own list.
fn screens() -> Vec<Screen> {
    let pi_dpi = gpu_table()
        .into_iter()
        .find(|(id, _)| id == "pi_vc4_dpi")
        .map(|(_, caps)| caps)
        .expect("pi_vc4_dpi is in data/hardware/matrix.toml");
    let mut all: Vec<Screen> = all_presets()
        .into_iter()
        .flat_map(|profile| {
            [
                Screen {
                    profile: profile.clone(),
                    caps: open_caps(),
                },
                Screen {
                    profile,
                    caps: pi_dpi,
                },
            ]
        })
        .collect();
    for d in arcade_table::edid_tubes()
        .into_iter()
        .chain(arcade_table::panels())
    {
        all.push(Screen {
            profile: d.profile,
            caps: d.caps,
        });
    }
    let composite = gpu_table()
        .into_iter()
        .find(|(id, _)| id == "pi_vc4_composite")
        .map(|(_, caps)| caps)
        .expect("pi_vc4_composite is in data/hardware/matrix.toml");
    for standard in TvStandard::ALL {
        all.push(Screen {
            profile: standard.profile(),
            caps: composite,
        });
    }
    all
}

/// The scanline settings of the run: each mode at the default strength,
/// and `on` with no strength.
const SCANLINES: [ScanlineChoice; 4] = [
    ScanlineChoice {
        mode: ScanlineMode::Auto,
        strength_percent: 75,
    },
    ScanlineChoice {
        mode: ScanlineMode::On,
        strength_percent: 75,
    },
    ScanlineChoice {
        mode: ScanlineMode::Off,
        strength_percent: 75,
    },
    ScanlineChoice {
        mode: ScanlineMode::On,
        strength_percent: 0,
    },
];

/// The shown mode lies inside the profile: a range of a CRT holds its
/// field rate, and its line rate up to one field rate below the least line
/// rate (the tolerance of the validator, which a whole-kHz clock needs); a
/// panel lists it; a TV standard has it.
fn inside(profile: &MonitorProfile, mode: &Modeline) -> bool {
    match &profile.kind {
        MonitorKind::Crt { ranges } => {
            let (line, field) = (mode.line_rate_mhz(), mode.field_rate_mhz());
            ranges.iter().any(|r| {
                (r.h_min_mhz.saturating_sub(field)..=r.h_max_mhz).contains(&line)
                    && (r.v_min_mhz..=r.v_max_mhz).contains(&field)
            })
        }
        MonitorKind::FlatPanel { modes } => modes.contains(mode),
        MonitorKind::Tv { standard } => standard.modes().contains(mode),
    }
}

fn mode_of(plan: &Plan, index: usize) -> Modeline {
    plan.considered[index]
        .outcome
        .as_ref()
        .map(|p| p.mode)
        .expect("a winner has a proposal")
}

/// The asked rate of a refresh choice, and the target kind of a winner at
/// it.
fn asked_rate(refresh: RefreshPolicy, game_mhz: u32) -> Option<(u64, RefreshKind)> {
    match refresh {
        RefreshPolicy::Exact => None,
        RefreshPolicy::ExactX2 => Some((2 * u64::from(game_mhz), RefreshKind::Double)),
        RefreshPolicy::Force50 => Some((50_000, RefreshKind::Forced50)),
        RefreshPolicy::Force60 => Some((60_000, RefreshKind::Forced60)),
    }
}

fn covers(ranges: &[CrtRange], rate_mhz: u64) -> bool {
    ranges
        .iter()
        .any(|r| r.v_min_mhz <= rate_mhz && rate_mhz <= r.v_max_mhz)
}

/// The test's own reading of why `refusal` holds, from the plan and the
/// validator, with no call into the refusal rules of the policy.
fn refusal_holds(
    refusal: DeclineReason,
    declined: &Declined,
    settings: &PolicySettings,
    screen: &Screen,
) -> Result<(), String> {
    let plan = &declined.plan;
    let valid = |m: &Modeline| validate(m, &screen.profile, &screen.caps).is_ok();
    let first = choose(plan, |m| validate(m, &screen.profile, &screen.caps)).winner;
    let rung = first.as_ref().map(|w| w.considered.rung);
    let tv = matches!(screen.profile.kind, MonitorKind::Tv { .. });
    let panel = matches!(screen.profile.kind, MonitorKind::FlatPanel { .. });
    let factor = settings.style.factor();
    let gpu_only = |asked: PictureStyle| {
        let mut own = plan
            .considered
            .iter()
            .filter(|c| walk_ladder(asked).first() == Some(&c.rung))
            .peekable();
        own.peek().is_some()
            && own.all(|c| match &c.outcome {
                Err(Dropped::NoInterlace) => !screen.caps.interlace,
                Err(Dropped::TallerThanGpu { .. } | Dropped::PixelClockFloor { .. }) => true,
                Ok(_) | Err(_) => false,
            })
    };
    let holds = match refusal {
        DeclineReason::NoPicture => first.is_none(),
        DeclineReason::RateFixedByTv { asked } => {
            tv && asked == settings.refresh && asked != RefreshPolicy::Exact
        }
        DeclineReason::RateOfPanel { asked } => {
            panel && asked == settings.refresh && asked != RefreshPolicy::Exact
        }
        DeclineReason::RateNotCovered { asked, rate_mhz } => match &screen.profile.kind {
            MonitorKind::Crt { ranges } => {
                asked_rate(asked, plan.source.refresh_mhz).map(|(r, _)| r) == Some(rate_mhz)
                    && !covers(ranges, rate_mhz)
            }
            MonitorKind::FlatPanel { .. } | MonitorKind::Tv { .. } => false,
        },
        DeclineReason::RateNotHeld { asked } => match &screen.profile.kind {
            MonitorKind::Crt { ranges } => {
                asked_rate(asked, plan.source.refresh_mhz).is_some_and(|(rate, kind)| {
                    covers(ranges, rate)
                        && first
                            .as_ref()
                            .is_some_and(|w| w.considered.target.kind != kind)
                })
            }
            MonitorKind::FlatPanel { .. } | MonitorKind::Tv { .. } => false,
        },
        DeclineReason::NeedsAnotherMonitor { asked, shown } => {
            !panel
                && asked == settings.style
                && factor.is_some()
                && rung == Some(shown)
                && walk_ladder(asked).first() != Some(&shown)
                && !gpu_only(asked)
        }
        DeclineReason::TooLargeForScreen { asked, shown } => {
            panel
                && asked == settings.style
                && rung == Some(shown)
                && match (factor, shown) {
                    (Some(k), Rung::Native) => k > 1,
                    (Some(k), Rung::Multiple(n)) => u16::from(n) < k,
                    (Some(_), Rung::Fitted) => true,
                    (Some(_), Rung::Interlaced | Rung::Panel) | (None, _) => false,
                }
        }
        DeclineReason::OutputCannotSend { asked, shown } => {
            !panel
                && asked == settings.style
                && rung == Some(shown)
                && walk_ladder(asked).first() != Some(&shown)
                && gpu_only(asked)
        }
        DeclineReason::NoExactMode => {
            settings.style == PictureStyle::Sharp
                && !plan
                    .proposals()
                    .any(|(_, c, p)| !c.rung.is_fitted() && valid(&p.mode))
        }
        DeclineReason::OneRateOnTv => tv && settings.style == PictureStyle::Speed,
    };
    holds
        .then_some(())
        .ok_or_else(|| format!("the reason {refusal:?} does not hold"))
}

/// The checks of one candidate, and the name of its result for the counts.
fn check(
    answer: &Offer,
    settings: &PolicySettings,
    screen: &Screen,
) -> Result<&'static str, String> {
    match answer {
        Ok(Offered {
            plan,
            winner,
            scanlines,
            ..
        }) => {
            let mode = mode_of(plan, *winner);
            validate(&mode, &screen.profile, &screen.caps)
                .map_err(|e| format!("offered, the validator refuses: {e:?}"))?;
            if !inside(&screen.profile, &mode) {
                return Err(format!("offered {mode:?} lies outside the profile"));
            }
            let w = choose(plan, |m| validate(m, &screen.profile, &screen.caps))
                .winner
                .ok_or("offered with no winner")?;
            if w.index != *winner {
                return Err("the offered winner is not the engine's winner".to_owned());
            }
            if let Some((_, kind)) = asked_rate(settings.refresh, plan.source.refresh_mhz)
                && w.considered.target.kind != kind
            {
                return Err(format!("offered {:?} away from it", settings.refresh));
            }
            if *scanlines == ScanlineOutcome::Drawn && mode.interlace {
                return Err("scanlines drawn on an interlaced mode".to_owned());
            }
            Ok("offered")
        }
        Err(declined) => {
            refusal_holds(declined.reason, declined, settings, screen)?;
            if let Some(index) = declined.winner {
                let mode = mode_of(&declined.plan, index);
                validate(&mode, &screen.profile, &screen.caps)
                    .map_err(|e| format!("the shown mode of a refusal fails: {e:?}"))?;
                if !inside(&screen.profile, &mode) {
                    return Err("the shown mode of a refusal lies outside".to_owned());
                }
            }
            Ok(reason_name(declined.reason))
        }
    }
}

fn reason_name(reason: DeclineReason) -> &'static str {
    match reason {
        DeclineReason::NoPicture => "no picture",
        DeclineReason::RateFixedByTv { .. } => "rate fixed by TV",
        DeclineReason::RateOfPanel { .. } => "rate of panel",
        DeclineReason::RateNotCovered { .. } => "rate not covered",
        DeclineReason::RateNotHeld { .. } => "rate not held",
        DeclineReason::NeedsAnotherMonitor { .. } => "needs another monitor",
        DeclineReason::TooLargeForScreen { .. } => "too large for screen",
        DeclineReason::OutputCannotSend { .. } => "output cannot send",
        DeclineReason::NoExactMode => "no exact mode",
        DeclineReason::OneRateOnTv => "one rate on TV",
    }
}

/// Every candidate on every source and display; the counts by result.
fn run(
    sources: &[SourceGeometry],
    screens: &[Screen],
    tolerate_solver: bool,
) -> BTreeMap<&'static str, usize> {
    let mut counts = BTreeMap::new();
    let mut failures = Vec::new();
    let mut kinds: BTreeMap<String, usize> = BTreeMap::new();
    for screen in screens {
        for source in sources {
            for style in PictureStyle::ALL {
                for refresh in RefreshPolicy::ALL {
                    for scanlines in SCANLINES {
                        let settings = PolicySettings {
                            style,
                            refresh,
                            ..PolicySettings::default()
                        };
                        let answer = offer(
                            source,
                            &settings,
                            scanlines,
                            &screen.profile,
                            &screen.caps,
                            |m| validate(m, &screen.profile, &screen.caps).map(drop),
                        );
                        let result = match answer {
                            Ok(answer) => check(&answer, &settings, screen),
                            Err(PolicyError::Solver(_)) if tolerate_solver => Ok("solver refuses"),
                            Err(e) => Err(format!("no plan: {e}")),
                        };
                        match result {
                            Ok(name) => {
                                if name == "no picture" {
                                    println!(
                                        "no picture: {} {source:?} {style} {refresh}",
                                        screen.profile.id
                                    );
                                }
                                *counts.entry(name).or_insert(0) += 1;
                            }
                            Err(e) => {
                                let kind = e.split(['{', ':']).next().unwrap_or("").to_owned();
                                *kinds.entry(kind).or_insert(0) += 1;
                                failures.push(format!(
                                    "{} {source:?} {style} {refresh} {scanlines:?}: {e}",
                                    screen.profile.id
                                ));
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} failures, by kind {kinds:?}:\n{}",
        failures.len(),
        failures[..failures.len().min(40)].join("\n")
    );
    counts
}

#[test]
fn every_candidate_on_every_display_is_offered_safely_or_refused_for_a_true_reason() {
    let sources: Vec<SourceGeometry> = arcade_table::BOARDS
        .iter()
        .chain(arcade_table::SOURCES.iter())
        .map(Board::source)
        .collect();
    let screens = screens();
    let counts = run(&sources, &screens, false);
    let total: usize = counts.values().sum();
    assert_eq!(
        total,
        sources.len()
            * screens.len()
            * PictureStyle::ALL.len()
            * RefreshPolicy::ALL.len()
            * SCANLINES.len()
    );
    assert!(counts["offered"] > total / 4, "{counts:?}");
    // A candidate with no picture is declined, and the engine keeps the
    // mode on screen; the run lists each one.
    println!("no picture: {:?}", counts.get("no picture"));
    println!(
        "offer: {} displays, {} sources, {total} candidates: {counts:?}",
        screens.len(),
        sources.len()
    );
}

/// A small generator with a fixed seed: the same values on every run.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 33
    }

    fn within(&mut self, low: u64, high: u64) -> u64 {
        low + self.next() % (high - low + 1)
    }
}

/// The CRT presets with their line rate limits moved by up to 1 % and
/// their field rate limits by up to 3 %, and generated sources: the same
/// invariants hold. A profile that the solver refuses gives an error, never
/// a panic; such errors stay below a quarter of the candidates.
#[test]
fn generated_sources_and_ranges_keep_every_invariant() {
    let mut rng = Lcg(0x005E_ED0F_0FFE);
    let mut screens = Vec::new();
    for profile in all_presets() {
        let MonitorKind::Crt { ranges } = &profile.kind else {
            continue;
        };
        for n in 0..2 {
            let ranges: Vec<CrtRange> = ranges
                .iter()
                .map(|r| {
                    let h_min = r.h_min_mhz * rng.within(990, 1_010) / 1_000;
                    let h_max = (r.h_max_mhz * rng.within(990, 1_010) / 1_000).max(h_min);
                    let v_min = r.v_min_mhz * rng.within(97, 103) / 100;
                    let v_max = (r.v_max_mhz * rng.within(97, 103) / 100).max(v_min);
                    CrtRange {
                        h_min_mhz: h_min,
                        h_max_mhz: h_max,
                        v_min_mhz: v_min,
                        v_max_mhz: v_max,
                        ..*r
                    }
                })
                .collect();
            screens.push(Screen {
                profile: MonitorProfile {
                    id: format!("{}~{n}", profile.id),
                    kind: MonitorKind::Crt { ranges },
                },
                caps: open_caps(),
            });
        }
    }
    let sources: Vec<SourceGeometry> = (0..12)
        .map(|_| SourceGeometry {
            interlaced: rng.within(0, 5) == 0,
            ..source(
                rng.within(160, 720) as u16,
                rng.within(144, 600) as u16,
                rng.within(45_000, 80_000) as u32,
            )
        })
        .collect();
    let counts = run(&sources, &screens, true);
    println!("generated: {counts:?}");
    let total: usize = counts.values().sum();
    assert!(
        counts.get("solver refuses").copied().unwrap_or(0) * 4 < total,
        "{counts:?}"
    );
}

/// The table of section 8 of docs/spec/video-policy-cases.md is the
/// output of [`picture_choices`] for the SNES (256x224 at 60.099 Hz) with
/// scanlines `auto`: one table per display class, a row per picture mode,
/// a column per refresh choice. To print it again:
/// `cargo xtask test -p neptron-modeline --no-capture spec_table`.
#[test]
fn the_spec_table_of_offered_combinations_is_the_output_of_the_function() {
    let displays = [
        (
            "15 kHz CRT (`generic_15`)",
            preset("generic_15"),
            open_caps(),
        ),
        ("31 kHz CRT (`arcade_31`)", preset("arcade_31"), open_caps()),
        (
            "31 kHz PC CRT up to 120 Hz (`pc_31_120`)",
            preset("pc_31_120"),
            open_caps(),
        ),
        ("TV (`tv_ntsc`)", TvStandard::Ntsc.profile(), composite()),
        ("Flat panel (`panel_1080`)", panel_1080(), open_caps()),
    ];
    let snes = source(256, 224, 60_099);
    let scanlines = SCANLINES[0];
    let mut table = String::new();
    for (title, profile, caps) in &displays {
        table.push_str(&format!("\n**{title}**\n\n| Mode |"));
        for refresh in RefreshPolicy::ALL {
            table.push_str(&format!(" `{refresh}` |"));
        }
        table.push_str("\n| --- |");
        for _ in RefreshPolicy::ALL {
            table.push_str(" --- |");
        }
        table.push('\n');
        for style in PictureStyle::ALL {
            table.push_str(&format!("| `{style}` |"));
            for refresh in RefreshPolicy::ALL {
                let settings = PolicySettings {
                    style,
                    refresh,
                    ..PolicySettings::default()
                };
                let answer = offer(&snes, &settings, scanlines, profile, caps, |m| {
                    validate(m, profile, caps).map(drop)
                })
                .unwrap();
                let cell = match &answer {
                    Ok(o) => {
                        let m = mode_of(&o.plan, o.winner);
                        format!(
                            "{}x{}{} {}.{:02} Hz",
                            m.hactive,
                            m.vactive,
                            if m.interlace { "i" } else { "p" },
                            m.field_rate_mhz() / 1_000,
                            m.field_rate_mhz() % 1_000 / 10
                        )
                    }
                    Err(d) => format!("no: {}", reason_name(d.reason)),
                };
                table.push_str(&format!(" {cell} |"));
            }
            table.push('\n');
        }
    }
    // The rows of the menus come from picture_choices, the same function.
    let choices = picture_choices(
        &snes,
        &PolicySettings::default(),
        scanlines,
        &displays[0].1,
        &displays[0].2,
        |m| validate(m, &displays[0].1, &displays[0].2).map(drop),
    )
    .unwrap();
    assert_eq!(choices.modes.len(), PictureStyle::ALL.len());
    assert_eq!(choices.refresh.len(), RefreshPolicy::ALL.len());
    let spec = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/spec/video-policy-cases.md");
    let text = std::fs::read_to_string(&spec).unwrap();
    let begin = "<!-- offer-table:begin -->\n";
    let end = "<!-- offer-table:end -->";
    let start = text
        .find(begin)
        .map(|i| i + begin.len())
        .expect("the begin marker");
    let stop = text.find(end).expect("the end marker");
    println!("{table}");
    assert_eq!(
        &text[start..stop],
        table.trim_start_matches('\n').to_owned() + "\n",
        "the spec table drifted; print it again and copy it"
    );
}

fn composite() -> GpuCaps {
    gpu_table()
        .into_iter()
        .find(|(id, _)| id == "pi_vc4_composite")
        .map(|(_, caps)| caps)
        .unwrap()
}
