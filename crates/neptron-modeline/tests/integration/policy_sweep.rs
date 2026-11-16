//! The invariant sweep: a fixed grid of synthetic sources (no random
//! numbers) on every display of the arcade table, with the default
//! settings. Each case checks the invariants of the video rule of
//! docs/spec/arcade.md (section 11); the first failures of each invariant
//! print in a readable form.
//!
//! The grid: 4 widths, 8 heights, both orientations, progressive (and
//! interlaced for the two tallest heights), and per display the rates
//! 30, 50, 55, 60, 70 and 80 Hz with every edge of each range, one
//! millihertz inside and one outside.

use std::collections::BTreeMap;

use neptron_modeline::policy::{
    Plan, PolicyError, PolicySettings, Proposal, REFRESH_MATCH_MHZ, Rung, choose, decide,
};
use neptron_modeline::{Settings, modelines_with};
use neptron_safety::{Rejection, validate};
use neptron_types::{CrtRange, GpuCaps, MonitorKind, MonitorProfile, Rotation, SourceGeometry};

use crate::arcade_table::{Display, crop_lines, displays, field_lines, shape_error};

const WIDTHS: [u16; 4] = [160, 320, 512, 800];
const HEIGHTS: [u16; 8] = [100, 144, 224, 240, 288, 320, 480, 640];
const RATES_MHZ: [u32; 6] = [30_000, 50_000, 55_000, 60_000, 70_000, 80_000];

/// The tolerance of the shape on the screen, in per mille, on top of one
/// pixel of rounding on each axis of the picture.
const SHAPE_TOLERANCE_PERMILLE: i64 = 20;

/// How many failures of one invariant print.
const PRINTED_FAILURES: usize = 8;

/// The sources of the grid: the frame as the core draws it.
fn sources() -> Vec<SourceGeometry> {
    let mut out = Vec::new();
    for &width in &WIDTHS {
        for &height in &HEIGHTS {
            for rotation in [Rotation::R0, Rotation::R90] {
                let scans: &[bool] = if height >= 480 {
                    &[false, true]
                } else {
                    &[false]
                };
                for &interlaced in scans {
                    // The frame as the core draws it; a turned frame shows
                    // its width as lines.
                    let (w, h) = (width, height);
                    out.push(SourceGeometry {
                        width: w,
                        height: h,
                        refresh_mhz: 0,
                        // A 4:3 frame, as most cores report it; a turned
                        // frame then shows a 3:4 picture.
                        aspect_milli: 1_333,
                        interlaced: interlaced && rotation == Rotation::R0,
                        rotation,
                    });
                }
            }
        }
    }
    out.dedup();
    out
}

/// The rates of a display: the fixed list, and each edge of each range,
/// one millihertz inside and one outside.
fn rates(profile: &MonitorProfile) -> Vec<u32> {
    let mut rates: Vec<u32> = RATES_MHZ.to_vec();
    if let MonitorKind::Crt { ranges } = &profile.kind {
        for r in ranges {
            for edge in [r.v_min_mhz, r.v_max_mhz] {
                let edge = u32::try_from(edge).unwrap_or(u32::MAX);
                rates.extend([edge.saturating_sub(1), edge, edge.saturating_add(1)]);
            }
        }
    }
    rates.sort_unstable();
    rates.dedup();
    rates
}

/// The class of an outcome, for the continuity check.
/// A fit is one class whatever its scan: the scan of a fit is the mode
/// that Switchres fits, which may flip at any line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Class {
    NoMode,
    Exact { interlace: bool },
    Fit,
}

/// The named limits a source can cross between two neighbouring cases:
/// for each range that covers the rate, whether the lines fit its
/// progressive window, its crop reach and its interlaced window; on a TV,
/// whether they fit each mode and its crop; on a panel, whether the fill
/// scale reaches 4. A change of class is allowed only where one of these
/// changes.
fn limits(profile: &MonitorProfile, lines: u32, rate: u32) -> Vec<bool> {
    match &profile.kind {
        MonitorKind::Crt { ranges } => ranges
            .iter()
            .flat_map(|r| {
                let asked = u64::from(rate);
                let covers = r.v_min_mhz <= asked && asked <= r.v_max_mhz;
                let double = r.v_min_mhz <= 2 * asked && 2 * asked <= r.v_max_mhz;
                // A rate that no range covers becomes the nearest covered
                // one, so the windows are those of that rate.
                let rate = asked.clamp(r.v_min_mhz, r.v_max_mhz.max(r.v_min_mhz));
                let cap = u32::from(r.progressive_lines_max).min(field_lines(r, rate));
                let icap = u32::from(r.interlaced_lines_max).min(2 * field_lines(r, rate));
                let cap2 = u32::from(r.progressive_lines_max).min(field_lines(r, 2 * rate));
                [
                    covers,
                    double,
                    lines <= cap,
                    lines <= cap + 16,
                    lines <= icap,
                    2 * lines <= cap,
                    2 * lines <= cap + 32,
                    lines <= cap2,
                    lines >= u32::from(r.progressive_lines_min),
                    u32::from(r.progressive_lines_min) <= cap,
                    u32::from(r.interlaced_lines_min) <= icap,
                    lines <= u32::from(r.interlaced_lines_min),
                ]
            })
            .collect(),
        MonitorKind::Tv { standard } => {
            let progressive = if standard.lines() == 525 { 240 } else { 288 };
            vec![
                lines <= progressive,
                lines <= progressive + 16,
                lines <= 2 * progressive,
            ]
        }
        MonitorKind::FlatPanel { modes } => modes
            .first()
            .map(|m| vec![u32::from(m.vactive) * 1_000 / lines.max(1) >= 4_000])
            .unwrap_or_default(),
    }
}

/// The rate lies within `REFRESH_MATCH_MHZ` of an edge of a range: a mode
/// there may miss the rate by the match, so the class may change at the
/// edge.
fn near_edge(profile: &MonitorProfile, rate: u32) -> bool {
    let MonitorKind::Crt { ranges } = &profile.kind else {
        return false;
    };
    let rate = u64::from(rate);
    // A rate that no range covers becomes an edge itself.
    let covered = ranges
        .iter()
        .any(|r| r.v_min_mhz <= rate && rate <= r.v_max_mhz);
    !covered
        || ranges.iter().any(|r| {
            [r.v_min_mhz, r.v_max_mhz].iter().any(|&edge| {
                edge.abs_diff(rate) < REFRESH_MATCH_MHZ
                    || edge.abs_diff(2 * rate) < REFRESH_MATCH_MHZ
            })
        })
}

/// An exact mode exists for the source on a CRT profile when a range
/// holds a whole multiple of its lines, progressive or interlaced 1:1, at
/// its rate or twice it, and Switchres gives that mode at that rate. The
/// search asks each range alone, so the policy's own walk is not used.
fn exact_exists(
    source: &SourceGeometry,
    (width, lines, aspect_milli): (u16, u16, u32),
    ranges: &[CrtRange],
    caps: &GpuCaps,
) -> bool {
    // The rate the policy uses for a console: the game's own, or the
    // nearest covered one when no range covers it.
    let rate = u64::from(source.refresh_mhz);
    let covered = ranges
        .iter()
        .any(|r| r.v_min_mhz <= rate && rate <= r.v_max_mhz);
    let nearest = ranges
        .iter()
        .map(|r| rate.clamp(r.v_min_mhz, r.v_max_mhz.max(r.v_min_mhz)))
        .min_by_key(|&c| c.abs_diff(rate))
        .unwrap_or(rate);
    let chosen = if covered { rate } else { nearest };
    let rates = [u32::try_from(chosen).unwrap_or(u32::MAX)];
    ranges.iter().any(|range| {
        let single = MonitorProfile {
            id: "single".into(),
            kind: MonitorKind::Crt {
                ranges: vec![*range],
            },
        };
        rates.iter().any(|&rate| {
            let covered = range.v_min_mhz <= u64::from(rate) && u64::from(rate) <= range.v_max_mhz;
            if !covered {
                return false;
            }
            let field = field_lines(range, u64::from(rate));
            // The fewest lines the policy borders a game in: the range's
            // least, and the lines that its lowest line rate forces.
            let total = range.h_min_mhz.div_ceil(u64::from(rate).max(1));
            let blank_ns = u128::from(range.v_front_porch_ns)
                + u128::from(range.v_sync_ns)
                + u128::from(range.v_back_porch_ns);
            let blank = (blank_ns * u128::from(range.h_min_mhz)).div_ceil(1_000_000_000_000);
            let forced = u32::try_from(u128::from(total).saturating_sub(blank)).unwrap_or(0);
            let fewest = u32::from(range.progressive_lines_min).max(forced);
            // (mode lines, game lines, interlaced)
            let mut asks: Vec<(u32, u32, bool)> = (1..=4u32)
                .map(|k| u32::from(lines) * k)
                .filter(|&l| l <= u32::from(range.progressive_lines_max).min(field))
                .map(|l| {
                    (
                        l.max(fewest)
                            .min(u32::from(range.progressive_lines_max).min(field)),
                        l,
                        false,
                    )
                })
                .collect();
            if caps.interlace && range.interlaced_lines_max > 0 {
                let l = u32::from(lines).max(u32::from(range.interlaced_lines_min));
                if l <= u32::from(range.interlaced_lines_max).min(2 * field) {
                    asks.push((l, u32::from(lines), true));
                }
            }
            asks.into_iter().any(|(l, game_lines, interlaced)| {
                // The width at which the game keeps its shape on the 4:3
                // face, never below the game width: the pixel clock of the
                // mode then counts against the display's limit.
                let shaped = (u64::from(width) * u64::from(l) * 4_000)
                    .div_ceil(3 * u64::from(game_lines) * u64::from(aspect_milli.max(1)));
                let width = u16::try_from(shaped.max(u64::from(width))).unwrap_or(u16::MAX);
                let Ok(height) = u16::try_from(l) else {
                    return false;
                };
                let ask = SourceGeometry {
                    width,
                    height,
                    refresh_mhz: rate,
                    aspect_milli: 1_333,
                    interlaced,
                    rotation: Rotation::R0,
                };
                let open = GpuCaps {
                    pclock_min_hz: 0,
                    ..*caps
                };
                modelines_with(&ask, &single, &open, &Settings::default())
                    .unwrap_or_default()
                    .iter()
                    .any(|c| {
                        let m = &c.modeline;
                        m.vactive == height
                            && m.interlace == interlaced
                            && m.field_rate_mhz().abs_diff(u64::from(rate)) < 5
                            && validate(m, &single, caps).is_ok()
                    })
            })
        })
    })
}

/// The failures of one invariant: their count and the first ones.
#[derive(Default)]
struct Failures {
    count: usize,
    first: Vec<String>,
}

impl Failures {
    fn add(&mut self, line: String) {
        self.count += 1;
        if self.first.len() < PRINTED_FAILURES {
            self.first.push(line);
        }
    }
}

struct Outcome {
    class: Class,
    lines: u32,
}

#[derive(Default)]
struct Sweep {
    cases: usize,
    checked: BTreeMap<&'static str, usize>,
    failed: BTreeMap<&'static str, Failures>,
}

impl Sweep {
    fn check(&mut self, name: &'static str, ok: bool, line: impl FnOnce() -> String) {
        *self.checked.entry(name).or_default() += 1;
        if !ok {
            self.failed.entry(name).or_default().add(line());
        }
    }

    /// One case: every invariant on the decision for `source` on
    /// `display`, and its class for the continuity check.
    fn case(&mut self, source: &SourceGeometry, display: &Display) -> Outcome {
        self.cases += 1;
        let (profile, caps) = (&display.profile, &display.caps);
        let what = || {
            format!(
                "{} {}x{}{} {:?} at {} mHz",
                profile.id,
                source.width,
                source.height,
                if source.interlaced { "i" } else { "p" },
                source.rotation,
                source.refresh_mhz
            )
        };
        let settings = PolicySettings::default();
        let plan = match decide(source, &settings, profile, caps) {
            Ok(plan) => plan,
            Err(PolicyError::EmptySource { .. }) => unreachable!("the grid has no empty source"),
            Err(e) => {
                self.check("decision", false, || format!("{}: {e}", what()));
                return Outcome {
                    class: Class::NoMode,
                    lines: 0,
                };
            }
        };
        self.check("decision", true, String::new);
        let lines = u32::from(plan.source.height);
        let choice = choose(&plan, |m| validate(m, profile, caps));
        let Some(w) = choice.winner else {
            // The allowed reasons: a range of one line rate or one field
            // rate (the clock of a narrow mode cannot hit it), or the
            // display's pixel clock limit refuses every proposal.
            let one_rate = match &profile.kind {
                MonitorKind::Crt { ranges } => ranges
                    .iter()
                    .any(|r| r.h_min_mhz == r.h_max_mhz || r.v_min_mhz == r.v_max_mhz),
                _ => false,
            };
            let clock = !choice.refused.is_empty()
                && choice
                    .refused
                    .iter()
                    .all(|r| matches!(r.reason, Rejection::PixelClock { .. }));
            self.check("no mode has an allowed reason", one_rate || clock, || {
                what()
            });
            return Outcome {
                class: Class::NoMode,
                lines,
            };
        };
        let p = w.proposal;
        let fitted = w.considered.rung == Rung::Fitted;
        self.check(
            "the winner passes the validator",
            validate(&p.mode, profile, caps).is_ok(),
            what,
        );
        if let MonitorKind::Crt { ranges } = &profile.kind {
            let declares = ranges.iter().any(|r| r.interlaced_lines_max > 0);
            self.check(
                "no interlace on a profile that declares none",
                declares || !p.mode.interlace,
                what,
            );
        }
        self.shape(&plan, profile, p, &what);
        if !fitted && w.considered.rung != Rung::Panel {
            let shown = lines - 2 * crop_lines(p);
            let whole = p
                .placement
                .scale
                .y
                .is_some_and(|k| p.placement.dest.h == u32::from(k) * shown);
            self.check("an exact picture has a whole vertical scale", whole, || {
                format!("{}: {:?} for {shown} lines", what(), p.placement)
            });
        }
        if let MonitorKind::Crt { ranges } = &profile.kind {
            if fitted {
                // A range of one field rate (ntsc, pal) takes a mode only
                // at that rate to the millihertz, which the whole-kHz clock
                // of a narrow mode may not reach: a stated exception.
                let one_field = ranges.iter().any(|r| r.v_min_mhz == r.v_max_mhz);
                let shown = (
                    plan.source.width,
                    plan.source.height,
                    plan.source.aspect_milli,
                );
                let exact = exact_exists(source, shown, ranges, caps);
                self.check(
                    "no soft fit when an exact mode exists",
                    one_field || !exact,
                    what,
                );
            }
            let rate = u64::from(source.refresh_mhz);
            let covered = ranges
                .iter()
                .any(|r| r.v_min_mhz <= rate && rate <= r.v_max_mhz);
            if covered && !fitted {
                let shown = p.timing.field_rate_mhz;
                let exact = shown.abs_diff(rate) < 5 || shown.abs_diff(2 * rate) < 5;
                self.check("the rate is exact where a range covers it", exact, || {
                    format!("{}: shown {shown} mHz ({:?})", what(), p.timing.target.kind)
                });
            }
        }
        Outcome {
            class: if fitted {
                Class::Fit
            } else {
                Class::Exact {
                    interlace: p.mode.interlace,
                }
            },
            lines,
        }
    }

    /// The shape on the screen: the source's aspect within the tolerance
    /// and one pixel of rounding on each axis. The overscan crop and the
    /// console width of a TV are stated exceptions (docs/spec/arcade.md).
    fn shape(
        &mut self,
        plan: &Plan,
        profile: &MonitorProfile,
        p: &Proposal,
        what: &dyn Fn() -> String,
    ) {
        let error = shape_error(plan, profile, p);
        let d = p.placement.dest;
        let rounding = 1_000 / i64::from(d.w.max(1)) + 1_000 / i64::from(d.h.max(1));
        let crop = i64::from(crop_lines(p)) * 2_000 / i64::from(plan.source.height.max(1));
        let tv_console = matches!(profile.kind, MonitorKind::Tv { .. });
        let tolerance = SHAPE_TOLERANCE_PERMILLE + rounding + crop;
        let name = if tv_console {
            "the shape is kept (TV: console width rule)"
        } else {
            "the shape is kept"
        };
        self.check(name, tv_console || error.abs() <= tolerance, || {
            format!(
                "{}: {error} per mille, {:?}, {:?}",
                what(),
                d,
                p.timing.target.kind
            )
        });
    }

    fn report(&self) -> String {
        let mut out = format!("{} cases\n", self.cases);
        for (name, checked) in &self.checked {
            let failed = self.failed.get(name);
            out.push_str(&format!(
                "{name}: {checked} checked, {} failed\n",
                failed.map_or(0, |f| f.count)
            ));
            for line in failed.iter().flat_map(|f| f.first.iter()) {
                out.push_str(&format!("    {line}\n"));
            }
        }
        out
    }
}

#[test]
fn the_policy_keeps_its_invariants_over_a_grid_of_sources() {
    let start = std::time::Instant::now();
    let mut sweep = Sweep::default();
    let mut continuity = Failures::default();
    let mut continuity_checked = 0;
    for display in displays() {
        let profile = &display.profile;
        for &rate in &rates(profile) {
            for base in sources() {
                let source = SourceGeometry {
                    refresh_mhz: rate,
                    ..base
                };
                let here = sweep.case(&source, &display);
                // One line taller (on the screen) and one millihertz
                // faster: the class changes only across a named limit.
                let taller = if source.rotation == Rotation::R90 {
                    SourceGeometry {
                        width: source.width + 1,
                        ..source
                    }
                } else {
                    SourceGeometry {
                        height: source.height + 1,
                        ..source
                    }
                };
                let faster = SourceGeometry {
                    refresh_mhz: rate + 1,
                    ..source
                };
                for (next, next_rate) in [(taller, rate), (faster, rate + 1)] {
                    let there = sweep.case(&next, &display);
                    continuity_checked += 1;
                    let crossed = limits(profile, here.lines, rate)
                        != limits(profile, there.lines, next_rate)
                        || near_edge(profile, rate)
                        || near_edge(profile, next_rate);
                    if here.class != there.class && !crossed {
                        continuity.add(format!(
                            "{} {}x{} {:?} at {rate} mHz: {:?}, then {}x{} at {next_rate}: {:?}",
                            profile.id,
                            source.width,
                            source.height,
                            source.rotation,
                            here.class,
                            next.width,
                            next.height,
                            there.class
                        ));
                    }
                }
            }
        }
    }
    let mut report = sweep.report();
    report.push_str(&format!(
        "continuity: {continuity_checked} checked, {} failed\n",
        continuity.count
    ));
    for line in &continuity.first {
        report.push_str(&format!("    {line}\n"));
    }
    report.push_str(&format!("time: {} ms\n", start.elapsed().as_millis()));
    println!("{report}");
    let failed = sweep.failed.values().map(|f| f.count).sum::<usize>() + continuity.count;
    assert_eq!(failed, 0, "{report}");
}

#[test]
fn the_grid_holds_every_shape_of_source() {
    let sources = sources();
    assert!(sources.iter().any(|s| s.interlaced));
    assert!(sources.iter().any(|s| s.rotation == Rotation::R90));
    let rates = rates(&crate::presets::preset("generic_15"));
    assert!(rates.contains(&49_499) && rates.contains(&49_500) && rates.contains(&49_501));
    assert!(rates.contains(&64_999) && rates.contains(&65_000) && rates.contains(&65_001));
}
