//! The text a person reads on the terminal: the profile, the GPU values,
//! the modes and the families that are not tested.

use std::fmt::Write as _;

use neptron_types::{CrtRange, GpuCaps, MonitorKind};

use super::family::{Outcome, describe};
use super::monitor::Setup;

/// The plan as text: the ranges of the profile, the GPU values, each mode
/// and each family that has no mode of its own.
pub fn plan_text(setup: &Setup, outcomes: &[Outcome]) -> String {
    let mut s = String::new();
    // Writing to a String cannot fail.
    let _ = write_plan(&mut s, setup, outcomes);
    s
}

fn write_plan(s: &mut String, setup: &Setup, outcomes: &[Outcome]) -> std::fmt::Result {
    writeln!(s, "Profile {} ({})", setup.profile.id, setup.source)?;
    match &setup.profile.kind {
        MonitorKind::Crt { ranges } => {
            for (i, r) in ranges.iter().enumerate() {
                writeln!(s, "  range {}: {}", i + 1, range_text(r))?;
            }
        }
        MonitorKind::FlatPanel { modes } => {
            writeln!(s, "  flat panel with {} modes", modes.len())?;
        }
    }
    if let Some(limit) = setup.edid_pclock_max_hz {
        writeln!(s, "  EDID pixel clock limit: {}", mhz(limit))?;
    }
    let g = &setup.gpu;
    writeln!(
        s,
        "GPU: {} {} {}, table entry {}; connector {} ({})",
        g.card.as_deref().unwrap_or("not detected"),
        g.driver.as_deref().unwrap_or("-"),
        g.pci_id.as_deref().unwrap_or("-"),
        g.table_entry.as_deref().unwrap_or("none (unknown GPU)"),
        g.connector.as_deref().unwrap_or("not detected"),
        g.connector_status.as_deref().unwrap_or("-"),
    )?;
    writeln!(s, "GPU values: {}", caps_text(&setup.caps))?;
    writeln!(s, "Modes:")?;
    let mut any = false;
    for o in outcomes {
        if let Outcome::Planned(p) = o {
            any = true;
            writeln!(
                s,
                "  {:<22} {}  ({}, {}{})",
                p.family,
                describe(p.token.mode()),
                p.policy.rung,
                p.policy.refresh_kind,
                p.policy
                    .width_rule
                    .as_deref()
                    .map(|w| format!(", width {w}"))
                    .unwrap_or_default()
            )?;
        }
    }
    if !any {
        writeln!(s, "  none: the profile and the GPU give no test mode")?;
    }
    writeln!(s, "Not tested:")?;
    for o in outcomes {
        match o {
            Outcome::Planned(_) => {}
            Outcome::SameAs { family, same_as } => {
                writeln!(s, "  {family:<22} the same mode as {same_as}")?;
            }
            Outcome::Unsupported { family, reason } => {
                writeln!(s, "  {family:<22} {reason}")?;
            }
        }
    }
    Ok(())
}

fn range_text(r: &CrtRange) -> String {
    let lines = |min: u16, max: u16| {
        if max == 0 {
            "none".to_owned()
        } else {
            format!("{min}-{max} lines")
        }
    };
    format!(
        "{}-{} kHz, {}-{} Hz, progressive {}, interlaced {}",
        thousandths(r.h_min_mhz / 1_000),
        thousandths(r.h_max_mhz / 1_000),
        thousandths(r.v_min_mhz),
        thousandths(r.v_max_mhz),
        lines(r.progressive_lines_min, r.progressive_lines_max),
        lines(r.interlaced_lines_min, r.interlaced_lines_max),
    )
}

fn caps_text(c: &GpuCaps) -> String {
    format!(
        "pixel clock {} to {}, interlace {}, flip per field {}, up to {}x{}",
        mhz(c.pclock_min_hz),
        mhz(c.pclock_max_hz),
        yes_no(c.interlace),
        yes_no(c.flip_per_field),
        c.max_width,
        c.max_height
    )
}

/// A value in thousandths as `N.NNN`.
fn thousandths(v: u64) -> String {
    format!("{}.{:03}", v / 1_000, v % 1_000)
}

fn mhz(hz: u32) -> String {
    format!("{} MHz", thousandths(u64::from(hz) / 1_000))
}

fn yes_no(b: bool) -> &'static str {
    if b { "yes" } else { "no" }
}

#[cfg(test)]
mod tests {
    use neptron_platform::UNKNOWN_GPU_CAPS;

    use super::*;
    use crate::selftest::family::plan;
    use crate::selftest::monitor::{GpuFacts, load_preset};

    #[test]
    fn the_plan_text_names_ranges_values_modes_and_gaps() {
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/monitors");
        let setup = Setup {
            profile: load_preset(&dir, "pc_31_120").unwrap(),
            source: "preset",
            edid_pclock_max_hz: None,
            caps: UNKNOWN_GPU_CAPS,
            gpu: GpuFacts::default(),
        };
        let text = plan_text(&setup, &plan(&setup.profile, &setup.caps));
        for part in [
            "Profile pc_31_120 (preset)",
            "range 1: 31.400-31.600 kHz, 100.000-130.000 Hz, progressive 200-256 lines, interlaced none",
            "pixel clock 25.000 MHz to 165.000 MHz, interlace no",
            "240p_double_refresh",
            "480p",
            "2x                     the same mode as 480p",
            "table entry none (unknown GPU)",
        ] {
            assert!(text.contains(part), "{part}\n{text}");
        }
    }
}
