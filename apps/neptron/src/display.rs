//! The display configuration of the guard: the detected output, the
//! monitor of the settings and the safe default.

use std::path::Path;

use neptron_platform::{ConnectorStatus, EdidProfile, Matrix, Platform, profile_from_edid};
use neptron_types::GpuCaps;
use neptron_video::{DisplayConfig, Monitor, PolicySettings, policy_settings};

use crate::presets::Presets;
use crate::record::MonitorRef;

/// The preset of the safe start when no monitor reports EDID: a 15 kHz
/// profile, NTSC-timed modes included. The safety model starts an unknown
/// display at 15 kHz, because a 31 kHz monitor shows "out of range"
/// harmlessly, while 31 kHz can stress a 15 kHz chassis.
pub const SAFE_START_PROFILE: &str = "generic_15";

/// The value of `display.monitor` and `display.output` that lets the
/// frontend decide.
pub const AUTO: &str = "auto";

/// What the frontend found about the outputs at start.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Detected {
    /// The connected outputs, by connector name.
    pub outputs: Vec<String>,
    /// The profile from the EDID of the first connected output that gives
    /// one, or of the output the settings name.
    pub edid: Option<EdidProfile>,
    /// The capabilities of the GPU of that output, from the capability
    /// table; `None` when nothing was detected.
    pub caps: Option<GpuCaps>,
}

/// The monitor a setting names cannot be used.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MonitorError {
    /// No preset has this id.
    #[error("the monitor profile {0:?} is not installed; the safe default is used")]
    Unknown(String),
    /// Neither the EDID nor the safe start preset is there.
    #[error(
        "no safe monitor profile: no EDID and no {SAFE_START_PROFILE} preset in the monitor folder"
    )]
    NoSafeProfile,
}

impl Detected {
    /// What `platform` shows, with the output that `output` names first
    /// (`auto` names none). An EDID that gives no profile is skipped.
    pub fn of(platform: &Platform, output: &str) -> Self {
        let connected: Vec<_> = platform
            .connectors
            .iter()
            .filter(|c| c.status == ConnectorStatus::Connected)
            .collect();
        let named = connected.iter().find(|c| c.name == output);
        let chosen = named.or_else(|| connected.first());
        let edid = named
            .into_iter()
            .chain(connected.iter())
            .find_map(|c| c.edid.as_deref().and_then(|e| profile_from_edid(e).ok()));
        let caps = chosen.and_then(|c| {
            let gpu = platform.gpu_of(c)?;
            Matrix::builtin().ok().map(|m| m.caps_for(gpu, &c.kind))
        });
        Self {
            outputs: connected.iter().map(|c| c.name.clone()).collect(),
            edid,
            caps,
        }
    }

    /// Reads the platform under `sysfs`. A root that cannot be read (a
    /// desktop, a test) gives nothing detected.
    pub fn read(sysfs: &Path, output: &str) -> Self {
        neptron_platform::detect(sysfs)
            .map(|p| Self::of(&p, output))
            .unwrap_or_default()
    }

    /// The safe default for the detected output: the EDID profile when
    /// there is EDID, else the 15 kHz safe start, with the picture style
    /// `auto` and the refresh policy `exact`.
    ///
    /// # Errors
    ///
    /// [`MonitorError::NoSafeProfile`].
    pub fn safe(&self, presets: &Presets) -> Result<DisplayConfig, MonitorError> {
        let monitor = match &self.edid {
            Some(edid) => Monitor::Edid(edid.clone()),
            None => Monitor::Profile(
                presets
                    .get(SAFE_START_PROFILE)
                    .cloned()
                    .ok_or(MonitorError::NoSafeProfile)?,
            ),
        };
        Ok(DisplayConfig {
            monitor,
            settings: safe_settings(),
        })
    }

    /// The monitor that `name` (a value of `display.monitor`) gives.
    ///
    /// # Errors
    ///
    /// [`MonitorError::Unknown`] for an id with no preset.
    pub fn monitor(
        &self,
        name: &str,
        presets: &Presets,
        safe: &DisplayConfig,
    ) -> Result<Monitor, MonitorError> {
        if name == AUTO {
            return Ok(safe.monitor.clone());
        }
        presets
            .get(name)
            .cloned()
            .map(Monitor::Profile)
            .ok_or_else(|| MonitorError::Unknown(name.to_owned()))
    }

    /// The monitor of a guard record entry again: a preset by its id, or
    /// the EDID profile of the connected monitor when its id matches.
    pub fn rebuild(&self, presets: &Presets, entry: &MonitorRef) -> Option<Monitor> {
        match entry {
            MonitorRef::Profile { id } => presets.get(id).cloned().map(Monitor::Profile),
            MonitorRef::Edid { id } => self
                .edid
                .as_ref()
                .filter(|e| &e.profile.id == id)
                .cloned()
                .map(Monitor::Edid),
        }
    }
}

/// The value of `display.monitor` that names `monitor`: its preset id, or
/// `auto` for an EDID profile, which has no preset file.
pub fn monitor_setting(monitor: &Monitor) -> String {
    match monitor {
        Monitor::Profile(profile) => profile.id.clone(),
        Monitor::Edid(_) => AUTO.to_owned(),
    }
}

/// Picture style `auto`, refresh policy `exact`.
fn safe_settings() -> PolicySettings {
    // Both values are choices of the schema, which the policy maps.
    policy_settings("auto", "exact", None).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use neptron_types::{MonitorKind, MonitorProfile};

    use super::*;

    fn presets() -> Presets {
        let profile = |id: &str| MonitorProfile {
            id: id.to_owned(),
            kind: MonitorKind::Crt { ranges: Vec::new() },
        };
        Presets::from_profiles(vec![profile(SAFE_START_PROFILE), profile("pc_31_120")])
    }

    #[test]
    fn with_no_edid_the_safe_default_is_the_15_khz_start() {
        let safe = Detected::default().safe(&presets()).expect("safe");
        assert_eq!(monitor_setting(&safe.monitor), SAFE_START_PROFILE);
        assert_eq!(safe.settings.style.key(), "auto");
        assert_eq!(safe.settings.refresh.key(), "exact");
        assert_eq!(
            Detected::default().safe(&Presets::default()),
            Err(MonitorError::NoSafeProfile)
        );
    }

    #[test]
    fn a_monitor_setting_names_a_preset_or_auto() {
        let detected = Detected::default();
        let safe = detected.safe(&presets()).expect("safe");
        let auto = detected.monitor(AUTO, &presets(), &safe).expect("auto");
        assert_eq!(auto, safe.monitor);
        let pc = detected
            .monitor("pc_31_120", &presets(), &safe)
            .expect("pc");
        assert_eq!(monitor_setting(&pc), "pc_31_120");
        assert_eq!(
            detected.monitor("gone", &presets(), &safe),
            Err(MonitorError::Unknown("gone".to_owned()))
        );
    }

    #[test]
    fn a_record_entry_is_built_again() {
        let detected = Detected::default();
        let id = |id: &str| MonitorRef::Profile { id: id.to_owned() };
        assert!(detected.rebuild(&presets(), &id("pc_31_120")).is_some());
        assert!(detected.rebuild(&presets(), &id("gone")).is_none());
        let edid = MonitorRef::Edid {
            id: "edid_npt_0071".to_owned(),
        };
        assert!(detected.rebuild(&presets(), &edid).is_none(), "no EDID now");
    }

    #[test]
    fn an_unreadable_sysfs_detects_nothing() {
        assert_eq!(
            Detected::read(Path::new("/nonexistent/sys"), AUTO),
            Detected::default()
        );
    }
}
