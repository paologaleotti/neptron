//! Loads the presets of data/monitors and builds the common inputs.

use std::path::{Path, PathBuf};

use neptron_types::{GpuCaps, Modeline, MonitorProfile, Rotation, SourceGeometry};

pub fn monitors_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/monitors")
}

pub fn preset(id: &str) -> MonitorProfile {
    let path = monitors_dir().join(format!("{id}.toml"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    toml::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

pub fn all_presets() -> Vec<MonitorProfile> {
    let mut ids: Vec<String> = std::fs::read_dir(monitors_dir())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|e| e == "toml"))
        .map(|path| path.file_stem().unwrap().to_str().unwrap().to_owned())
        .collect();
    ids.sort();
    ids.iter().map(|id| preset(id)).collect()
}

/// A GPU that sets no limit: no pixel clock floor, interlace allowed.
pub fn open_caps() -> GpuCaps {
    GpuCaps {
        pclock_min_hz: 0,
        pclock_max_hz: u32::MAX,
        interlace: true,
        flip_per_field: false,
        max_width: u16::MAX,
        max_height: u16::MAX,
    }
}

pub fn source(width: u16, height: u16, refresh_mhz: u32) -> SourceGeometry {
    SourceGeometry {
        width,
        height,
        refresh_mhz,
        aspect_milli: 1_333,
        interlaced: false,
        rotation: Rotation::R0,
    }
}

/// The timing part of a modeline as the Switchres standalone program
/// prints it: pixel clock in MHz with six decimals, the eight timings, the
/// flags and the sync polarities.
pub fn modeline_text(m: &Modeline) -> String {
    let mut parts = vec![
        format!("{}.{:06}", m.pclock_hz / 1_000_000, m.pclock_hz % 1_000_000),
        m.hactive.to_string(),
        m.hsync_start.to_string(),
        m.hsync_end.to_string(),
        m.htotal.to_string(),
        m.vactive.to_string(),
        m.vsync_start.to_string(),
        m.vsync_end.to_string(),
        m.vtotal.to_string(),
    ];
    if m.interlace {
        parts.push("interlace".into());
    }
    if m.doublescan {
        parts.push("doublescan".into());
    }
    parts.push(if m.hsync_positive { "+hsync" } else { "-hsync" }.into());
    parts.push(if m.vsync_positive { "+vsync" } else { "-vsync" }.into());
    parts.join(" ")
}
