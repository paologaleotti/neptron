//! Timing check 5, switching logic: a scripted core changes its geometry,
//! its rate and its scan at given frames, and the list of mode switches is
//! compared with a golden trace.
//!
//! The script stands in for `neptron-testcore` at tier 1: the engine sees
//! only the source of each core frame, so the same list of sources gives
//! the same switches. The golden holds the modes that the policy picks on
//! `generic_15`, so a change of the policy shows up here as a diff to
//! review.

use std::fmt::Write as _;

use neptron_types::SourceGeometry;
use neptron_video::{Directive, Outcome, PolicySettings, SwitchRecord};

use crate::harness::{
    MD_H32, MD_H40, PAL_239, Rig, SNES, SNES_239, SNES_448I, SNES_HIRES, SNES_PAL, Step,
    fixed_width, native_width,
};

const GOLDEN: &str = include_str!("../golden/switch_trace.txt");

/// One stretch of the script: `frames` core frames of each source in
/// `cycle`, in turn.
struct Stretch {
    frames: u32,
    cycle: &'static [SourceGeometry],
    /// The engine may set a new mode during the stretch.
    modeset_allowed: bool,
}

const fn hold(frames: u32, source: &'static [SourceGeometry]) -> Stretch {
    Stretch {
        frames,
        cycle: source,
        modeset_allowed: true,
    }
}

const fn no_modeset(frames: u32, cycle: &'static [SourceGeometry]) -> Stretch {
    Stretch {
        frames,
        cycle,
        modeset_allowed: false,
    }
}

/// SNES under a fixed super width: width changes and flicker cost no
/// modeset; the lines, the scan and the rate do. The last stretch changes
/// the rate alone.
const SNES_SCRIPT: &[Stretch] = &[
    hold(60, &[SNES]),
    no_modeset(60, &[SNES_HIRES]),
    no_modeset(20, &[SNES, SNES_HIRES]),
    hold(60, &[SNES_239]),
    hold(60, &[SNES_448I]),
    no_modeset(2, &[SNES]),
    no_modeset(58, &[SNES_448I]),
    no_modeset(20, &[SNES_448I, SNES]),
    hold(80, &[PAL_239]),
    hold(60, &[SNES]),
    hold(60, &[SNES_PAL]),
];

/// Mega Drive under the native width: H32 and H40 are two modes, and the
/// debounce holds a flicker.
const MD_SCRIPT: &[Stretch] = &[
    hold(30, &[MD_H40]),
    hold(30, &[MD_H32]),
    no_modeset(40, &[MD_H40, MD_H32]),
    no_modeset(1, &[MD_H32]),
    no_modeset(2, &[MD_H40, MD_H40]),
    hold(30, &[MD_H40]),
];

/// Runs a script and checks the rules that hold for every frame. Returns
/// the trace lines of its modesets.
fn run(name: &str, settings: PolicySettings, script: &[Stretch]) -> String {
    let mut rig = Rig::generic_15(settings);
    let mut frame = 0u32;
    for stretch in script {
        for i in 0..stretch.frames {
            frame += 1;
            let source = stretch.cycle[i as usize % stretch.cycle.len()];
            let step = rig.frame(source);
            check_step(frame, &step);
            if !stretch.modeset_allowed {
                assert_ne!(step.directive, Directive::Modeset, "{name} frame {frame}");
            }
        }
    }
    let switches = rig.fake.mode_switches();
    let modesets: Vec<&SwitchRecord> = rig.engine.trace().modesets().collect();
    assert_eq!(
        switches.len(),
        modesets.len(),
        "{name}: one set_mode per modeset"
    );
    for (switch, record) in switches.iter().zip(&modesets) {
        assert_eq!(Some(switch.active), record.read_back);
    }
    assert!(rig.fake.frames().iter().all(|f| f.present.is_ok()));
    let mut out = format!("[{name}]\n");
    for record in rig.engine.trace().records() {
        if record.outcome != Outcome::ScaleOnly {
            writeln!(out, "{}", line(record)).unwrap();
        }
    }
    out
}

/// Audio is silent only from a modeset to the first flip of the new mode,
/// and the mode changes only at the end of the debounce or at launch.
fn check_step(frame: u32, step: &Step) {
    let modeset = step.directive == Directive::Modeset;
    assert_eq!(step.muted_before_present, modeset, "frame {frame}");
    assert_eq!(step.mode_calls, usize::from(modeset), "frame {frame}");
    assert!(
        !step.muted_after_flip,
        "frame {frame}: unmuted after the flip"
    );
}

fn line(record: &SwitchRecord) -> String {
    let s = &record.source;
    let mut text = format!(
        "frame {} asked {} shown {} {} {}x{}{}@{}",
        record.decided_frame,
        record.asked_frame,
        record.shown_frame.map_or("-".to_owned(), |f| f.to_string()),
        record.trigger,
        s.width,
        s.height,
        if s.interlaced { "i" } else { "p" },
        s.refresh_mhz,
    );
    if let Some(w) = &record.winner {
        let m = w.token.mode();
        write!(
            text,
            " -> {} {}x{}{} {} mHz {} Hz pclock",
            w.rung,
            m.hactive,
            m.vactive,
            if m.interlace { "i" } else { "p" },
            m.field_rate_mhz(),
            m.pclock_hz
        )
        .unwrap();
    }
    if let Some(gap) = record.gap_ns {
        write!(text, ", gap {gap} ns").unwrap();
    }
    if !matches!(record.outcome, Outcome::Modeset) {
        write!(text, ", {}", record.outcome).unwrap();
    }
    text
}

#[test]
fn timing_check_5_switches_match_the_golden_trace() {
    let actual = run("snes fixed super width", fixed_width(), SNES_SCRIPT)
        + &run("mega drive native width", native_width(), MD_SCRIPT);
    assert!(
        actual == GOLDEN,
        "the switch trace differs from tests/golden/switch_trace.txt. Check each \
         line against the policy before you change the file.\nactual:\n{actual}"
    );
}
