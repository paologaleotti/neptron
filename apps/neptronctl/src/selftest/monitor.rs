//! The monitor profile and the GPU values of a self-test: a preset file or
//! the EDID profile, and the capabilities of the detected GPU.

use std::path::{Path, PathBuf};

use neptron_platform::{ConnectorStatus, Matrix, Platform, UNKNOWN_GPU_CAPS, profile_from_edid};
use neptron_types::{GpuCaps, MonitorProfile, TvStandard};

use super::args::ProfileChoice;
use super::error::SelftestError;

/// The monitor and the GPU that the modes are planned for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Setup {
    /// The monitor profile.
    pub profile: MonitorProfile,
    /// `preset` or `edid`.
    pub source: &'static str,
    /// The highest pixel clock of the EDID, when the profile is the EDID
    /// profile.
    pub edid_pclock_max_hz: Option<u32>,
    /// The GPU values that `validate` gets: the capability table, with the
    /// EDID pixel clock limit for the EDID profile.
    pub caps: GpuCaps,
    /// What sysfs says about the GPU and the connector.
    pub gpu: GpuFacts,
}

/// The GPU and the connector as sysfs and the capability table give them.
/// Every field is `None` where nothing was detected.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GpuFacts {
    /// The DRM device name, `card0`.
    pub card: Option<String>,
    /// The kernel driver from sysfs.
    pub driver: Option<String>,
    /// The PCI id as `vendor:device` in hex.
    pub pci_id: Option<String>,
    /// The id of the capability table entry; `None` is an unknown GPU.
    pub table_entry: Option<String>,
    /// The name of that entry.
    pub table_name: Option<String>,
    /// The connector the detection chose.
    pub connector: Option<String>,
    /// Its status: `connected`, `disconnected` or `unknown`.
    pub connector_status: Option<String>,
}

/// Reads the platform under `sysfs` and the profile, and builds the setup.
/// A profile id `tv_<standard>` (`TvStandard::profile`) is the built-in TV
/// profile of that standard, not a file.
///
/// # Errors
///
/// [`SelftestError`] when the preset cannot be read or the EDID gives no
/// profile.
pub fn read(
    choice: &ProfileChoice,
    monitors: &Path,
    sysfs: &Path,
    connector: Option<&str>,
) -> Result<Setup, SelftestError> {
    // A host with no DRM class (a desktop, a test) detects nothing, and the
    // GPU gets the values of an unknown GPU.
    let platform = neptron_platform::detect(sysfs).unwrap_or_default();
    let preset = match choice {
        ProfileChoice::Preset(id) => Some(match TvStandard::from_profile_id(id) {
            Some(standard) => standard.profile(),
            None => load_preset(monitors, id)?,
        }),
        ProfileChoice::Edid => None,
    };
    setup(preset, &platform, connector)
}

/// Reads the preset `id` from `dir/<id>.toml`.
///
/// # Errors
///
/// [`SelftestError::Preset`] when the file cannot be read, does not parse,
/// or holds another id.
pub fn load_preset(dir: &Path, id: &str) -> Result<MonitorProfile, SelftestError> {
    let path = dir.join(format!("{id}.toml"));
    let failed = |path: &PathBuf, reason: String| SelftestError::Preset {
        path: path.clone(),
        reason,
    };
    let text = std::fs::read_to_string(&path).map_err(|e| failed(&path, e.to_string()))?;
    let profile: MonitorProfile =
        toml::from_str(&text).map_err(|e| failed(&path, e.message().to_owned()))?;
    if profile.id != id {
        return Err(failed(
            &path,
            format!("it holds the profile {:?}", profile.id),
        ));
    }
    Ok(profile)
}

/// The setup from a preset (or `None` for the EDID profile) and what sysfs
/// shows.
///
/// # Errors
///
/// [`SelftestError::NoEdidProfile`] when the EDID profile is asked for and
/// the chosen connector gives none.
pub fn setup(
    preset: Option<MonitorProfile>,
    platform: &Platform,
    named: Option<&str>,
) -> Result<Setup, SelftestError> {
    let connector = match named {
        Some(name) => platform.connectors.iter().find(|c| c.name == name),
        None => platform
            .connectors
            .iter()
            .find(|c| c.status == ConnectorStatus::Connected),
    };
    let gpu = connector.and_then(|c| platform.gpu_of(c));
    let entry = match (gpu, connector, Matrix::builtin()) {
        (Some(gpu), Some(c), Ok(matrix)) => matrix.lookup(gpu, &c.kind).cloned(),
        _ => None,
    };
    let table_caps = entry.as_ref().map_or(UNKNOWN_GPU_CAPS, |e| e.caps);
    let facts = GpuFacts {
        card: gpu.map(|g| g.card.clone()),
        driver: gpu.and_then(|g| g.driver.clone()),
        pci_id: gpu
            .and_then(|g| g.pci_id)
            .map(|id| format!("{:04x}:{:04x}", id.vendor, id.device)),
        table_entry: entry.as_ref().map(|e| e.id.clone()),
        table_name: entry.as_ref().map(|e| e.name.clone()),
        connector: connector.map(|c| c.name.clone()),
        connector_status: connector.map(|c| status_text(c.status).to_owned()),
    };
    if let Some(profile) = preset {
        return Ok(Setup {
            profile,
            source: "preset",
            edid_pclock_max_hz: None,
            caps: table_caps,
            gpu: facts,
        });
    }
    let Some(c) = connector else {
        return Err(SelftestError::NoEdidProfile {
            reason: match named {
                Some(name) => format!("sysfs shows no connector {name}"),
                None => "sysfs shows no connected connector".to_owned(),
            },
        });
    };
    let Some(bytes) = c.edid.as_deref() else {
        return Err(SelftestError::NoEdidProfile {
            reason: format!("{} gives no EDID", c.name),
        });
    };
    let edid = profile_from_edid(bytes).map_err(|e| SelftestError::NoEdidProfile {
        reason: format!("the EDID of {} gives no profile: {e}", c.name),
    })?;
    Ok(Setup {
        caps: edid.limit_caps(&table_caps),
        edid_pclock_max_hz: Some(edid.pclock_max_hz),
        profile: edid.profile,
        source: "edid",
        gpu: facts,
    })
}

fn status_text(status: ConnectorStatus) -> &'static str {
    match status {
        ConnectorStatus::Connected => "connected",
        ConnectorStatus::Disconnected => "disconnected",
        ConnectorStatus::Unknown => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use neptron_platform::{Connector, Gpu, PciId};
    use neptron_types::MonitorKind;

    use super::*;

    pub(crate) fn monitors_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/monitors")
    }

    fn connector(name: &str, kind: &str, status: ConnectorStatus) -> Connector {
        Connector {
            card: "card0".into(),
            name: name.into(),
            kind: kind.into(),
            status,
            edid: None,
        }
    }

    fn skylake() -> Platform {
        Platform {
            gpus: vec![Gpu {
                card: "card0".into(),
                driver: Some("i915".into()),
                pci_id: Some(PciId {
                    vendor: 0x8086,
                    device: 0x1912,
                }),
                of_compatible: Vec::new(),
            }],
            connectors: vec![
                connector("HDMI-A-1", "HDMI-A", ConnectorStatus::Disconnected),
                connector("DP-1", "DP", ConnectorStatus::Connected),
            ],
        }
    }

    fn empty() -> MonitorProfile {
        MonitorProfile {
            id: "x".into(),
            kind: MonitorKind::Crt { ranges: Vec::new() },
        }
    }

    #[test]
    fn a_shipped_preset_loads_by_id() {
        let profile = load_preset(&monitors_dir(), "pc_31_120").unwrap();
        assert_eq!(profile.id, "pc_31_120");
        assert!(matches!(profile.kind, MonitorKind::Crt { ref ranges } if ranges.len() == 2));
    }

    #[test]
    fn a_tv_profile_id_is_the_built_in_profile_of_its_standard() {
        let tree = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../crates/neptron-platform/tests/fixtures/sysfs/pi3_composite");
        let choice = ProfileChoice::Preset("tv_pal_60".into());
        let setup = read(
            &choice,
            Path::new("/nonexistent"),
            &tree,
            Some("Composite-1"),
        )
        .unwrap();
        assert_eq!(setup.profile, TvStandard::Pal60.profile());
        assert_eq!(setup.gpu.table_entry.as_deref(), Some("pi_vc4_composite"));
        assert_eq!(setup.caps.pclock_min_hz, 13_500_000);
        let unknown = ProfileChoice::Preset("tv_pal_70".into());
        assert!(
            read(&unknown, &monitors_dir(), &tree, None).is_err(),
            "no such file"
        );
    }

    #[test]
    fn a_missing_or_wrong_preset_is_an_error_with_the_path() {
        let err = load_preset(&monitors_dir(), "no_such_monitor").unwrap_err();
        assert!(err.to_string().contains("no_such_monitor.toml"), "{err}");
        let dir = std::env::temp_dir().join(format!("neptronctl-preset-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let text = std::fs::read_to_string(monitors_dir().join("ntsc.toml")).unwrap();
        std::fs::write(dir.join("other.toml"), text).unwrap();
        std::fs::write(dir.join("broken.toml"), "id = 3").unwrap();
        let other = load_preset(&dir, "other").unwrap_err();
        assert!(other.to_string().contains("\"ntsc\""), "{other}");
        assert!(load_preset(&dir, "broken").is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_connected_connector_gives_the_gpu_and_its_table_entry() {
        let setup = setup(Some(empty()), &skylake(), None).unwrap();
        assert_eq!(setup.source, "preset");
        assert_eq!(setup.gpu.connector.as_deref(), Some("DP-1"));
        assert_eq!(setup.gpu.connector_status.as_deref(), Some("connected"));
        assert_eq!(setup.gpu.driver.as_deref(), Some("i915"));
        assert_eq!(setup.gpu.pci_id.as_deref(), Some("8086:1912"));
        assert_eq!(setup.gpu.table_entry.as_deref(), Some("intel_gen9"));
        assert_eq!(setup.caps.pclock_min_hz, 25_000_000);
    }

    #[test]
    fn a_named_connector_is_used_in_any_state() {
        let setup = setup(Some(empty()), &skylake(), Some("HDMI-A-1")).unwrap();
        assert_eq!(setup.gpu.connector.as_deref(), Some("HDMI-A-1"));
        assert_eq!(setup.gpu.connector_status.as_deref(), Some("disconnected"));
    }

    #[test]
    fn nothing_detected_gives_the_unknown_gpu() {
        let setup = setup(Some(empty()), &Platform::default(), None).unwrap();
        assert_eq!(setup.caps, UNKNOWN_GPU_CAPS);
        assert_eq!(setup.gpu, GpuFacts::default());
    }

    #[test]
    fn the_edid_profile_comes_from_the_sysfs_tree() {
        let tree = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../crates/neptron-platform/tests/fixtures/sysfs/i3_6100t_vga");
        let setup = read(&ProfileChoice::Edid, &monitors_dir(), &tree, None).unwrap();
        assert_eq!(setup.source, "edid");
        assert!(
            setup.profile.id.starts_with("edid_"),
            "{}",
            setup.profile.id
        );
        assert_eq!(setup.gpu.connector.as_deref(), Some("VGA-1"));
        let limit = setup.edid_pclock_max_hz.unwrap();
        assert!(setup.caps.pclock_max_hz <= limit);
        let preset = read(
            &ProfileChoice::Preset("pc_31_120".into()),
            &monitors_dir(),
            &tree,
            None,
        )
        .unwrap();
        assert_eq!(preset.edid_pclock_max_hz, None);
        assert_eq!(preset.gpu.table_entry.as_deref(), Some("intel_gen9"));
    }

    #[test]
    fn the_edid_profile_needs_an_edid() {
        for (platform, named) in [
            (Platform::default(), None),
            (skylake(), None),
            (skylake(), Some("VGA-9")),
        ] {
            assert!(matches!(
                setup(None, &platform, named),
                Err(SelftestError::NoEdidProfile { .. })
            ));
        }
        let mut bad = skylake();
        bad.connectors[1].edid = Some(vec![0; 128]);
        let err = setup(None, &bad, None).unwrap_err();
        assert!(err.to_string().contains("DP-1"), "{err}");
    }
}
