//! Loads the presets of data/monitors and builds the common inputs.

use std::path::{Path, PathBuf};

use neptron_types::{
    CrtRange, GpuCaps, Modeline, MonitorKind, MonitorProfile, Rotation, SourceGeometry,
};
use serde::Deserialize;

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

/// The profile that `neptron_platform::profile_from_edid` builds for a PC
/// CRT whose EDID range limits are 30 to 71 kHz and 50 to 160 Hz, with a
/// 100 MHz pixel clock limit. The values are copied from the test
/// `pc_crt_30_to_71_khz_profile` in
/// crates/neptron-platform/tests/integration/profile.rs.
pub fn pc_crt_30_71() -> MonitorProfile {
    MonitorProfile {
        id: "edid_npt_0071".into(),
        kind: MonitorKind::Crt {
            ranges: vec![CrtRange {
                h_min_mhz: 30_000_000,
                h_max_mhz: 71_000_000,
                v_min_mhz: 50_000,
                v_max_mhz: 160_000,
                h_front_porch_ns: 817,
                h_sync_ns: 2_667,
                h_back_porch_ns: 3_334,
                v_front_porch_ns: 33_334,
                v_sync_ns: 100_000,
                v_back_porch_ns: 507_747,
                hsync_positive: false,
                vsync_positive: true,
                progressive_lines_min: 169,
                progressive_lines_max: 1_374,
                interlaced_lines_min: 0,
                interlaced_lines_max: 0,
            }],
        },
    }
}

/// The EDID profile's GPU side: `EdidProfile::limit_caps` of
/// neptron-platform lowers the highest pixel clock of the GPU to the
/// 100 MHz limit of that EDID.
pub fn edid_caps(gpu: GpuCaps) -> GpuCaps {
    GpuCaps {
        pclock_max_hz: gpu.pclock_max_hz.min(100_000_000),
        ..gpu
    }
}

/// One entry of data/hardware/matrix.toml: the fields of `GpuCaps`.
#[derive(Deserialize)]
pub struct GpuEntry {
    pub id: String,
    pub pclock_min_hz: u32,
    pub pclock_max_hz: u32,
    pub interlace: bool,
    pub flip_per_field: bool,
    pub max_width: u16,
    pub max_height: u16,
}

#[derive(Deserialize)]
struct Matrix {
    gpu: Vec<GpuEntry>,
}

impl GpuEntry {
    pub fn caps(&self) -> GpuCaps {
        GpuCaps {
            pclock_min_hz: self.pclock_min_hz,
            pclock_max_hz: self.pclock_max_hz,
            interlace: self.interlace,
            flip_per_field: self.flip_per_field,
            max_width: self.max_width,
            max_height: self.max_height,
        }
    }
}

/// Every entry of the GPU capability table, then the values that
/// neptron-platform gives an unknown GPU (`UNKNOWN_GPU_CAPS`).
pub fn gpu_table() -> Vec<(String, GpuCaps)> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/hardware/matrix.toml");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let matrix: Matrix =
        toml::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let mut table: Vec<(String, GpuCaps)> = matrix
        .gpu
        .iter()
        .map(|g| (g.id.clone(), g.caps()))
        .collect();
    table.push(("unknown".into(), unknown_gpu()));
    table
}

/// `UNKNOWN_GPU_CAPS` of neptron-platform.
pub fn unknown_gpu() -> GpuCaps {
    GpuCaps {
        pclock_min_hz: 25_000_000,
        pclock_max_hz: 165_000_000,
        interlace: false,
        flip_per_field: false,
        max_width: 1_920,
        max_height: 1_200,
    }
}

/// The intel_gen9 entry of the capability table: the i3-6100T.
pub fn intel_gen9() -> GpuCaps {
    gpu_table()
        .into_iter()
        .find(|(id, _)| id == "intel_gen9")
        .map(|(_, caps)| caps)
        .expect("intel_gen9 is in data/hardware/matrix.toml")
}

/// A 1080p flat panel with its CEA-861 modes: 1920x1080 at 60 Hz (the
/// preferred mode, listed first) and at 50 Hz, and 1280x720 at 60 Hz.
pub fn panel_1080() -> MonitorProfile {
    let mode = |pclock_hz, h: [u16; 4], v: [u16; 4]| Modeline {
        pclock_hz,
        hactive: h[0],
        hsync_start: h[1],
        hsync_end: h[2],
        htotal: h[3],
        vactive: v[0],
        vsync_start: v[1],
        vsync_end: v[2],
        vtotal: v[3],
        interlace: false,
        doublescan: false,
        hsync_positive: true,
        vsync_positive: true,
    };
    MonitorProfile {
        id: "panel_1080".into(),
        kind: MonitorKind::FlatPanel {
            modes: vec![
                mode(
                    148_500_000,
                    [1920, 2008, 2052, 2200],
                    [1080, 1084, 1089, 1125],
                ),
                mode(
                    148_500_000,
                    [1920, 2448, 2492, 2640],
                    [1080, 1084, 1089, 1125],
                ),
                mode(74_250_000, [1280, 1390, 1430, 1650], [720, 725, 730, 750]),
            ],
        },
    }
}
