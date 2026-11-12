//! The display configuration of the guard: the detected output, the
//! monitor of the settings and the safe default.

use std::path::Path;

use neptron_platform::{
    Connector, ConnectorStatus, DisplayClass, EdidProfile, Matrix, Platform, display_class,
    panel_profile_from_edid, profile_for_display,
};
use neptron_types::{GpuCaps, Modeline, MonitorKind, OutputKind};
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

/// The value of `display.monitor` that drives the connected display as a
/// flat panel in the modes its EDID lists, whatever its input type: the
/// choice for an LCD with a VGA input that the user wants in its native
/// mode.
pub const FLAT: &str = "flat";

/// The widest flat-panel mode the frontend sets, in pixels: 4K UHD. A
/// scanout buffer of 3840x2160 in XRGB8888 is 33 MB.
pub const PANEL_WIDTH_MAX_PX: u16 = 3_840;
/// The tallest flat-panel mode the frontend sets, in lines.
pub const PANEL_HEIGHT_MAX_PX: u16 = 2_160;

/// The widest scanout buffer of a CRT mode, in pixels: the super width of
/// the policy.
pub const CRT_BUFFER_WIDTH_PX: u16 = 2_560;
/// The tallest scanout buffer of a CRT mode, in lines: 576i and the 31 kHz
/// modes of a PC CRT.
pub const CRT_BUFFER_HEIGHT_PX: u16 = 1_200;

/// What the frontend found about the outputs at start.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Detected {
    /// The connected outputs, by connector name.
    pub outputs: Vec<String>,
    /// The profile of the display by its class (`display_class` of
    /// `neptron-platform`): a CRT profile from the range limits, or a
    /// flat-panel profile of the listed modes that the system can send.
    /// It comes from the output in use; when no connected output has that
    /// name, from the first connected output that gives one.
    pub edid: Option<EdidProfile>,
    /// The flat-panel profile of the same EDID, whatever its class, for
    /// [`FLAT`].
    pub panel: Option<EdidProfile>,
    /// The capabilities of the GPU of that output, from the capability
    /// table; `None` when nothing was detected.
    pub caps: Option<GpuCaps>,
    /// The kind of link of that output, by the class rule: digital only
    /// for a flat panel on a digital link, else analog.
    pub output: OutputKind,
    /// The capabilities of that output as a digital link: `caps` with the
    /// digital limits of the table (`Matrix::caps_for_output`).
    pub digital_caps: Option<GpuCaps>,
    /// What the player must know about the detection: a panel mode above
    /// the limits, an EDID that gives no profile.
    pub notices: Vec<String>,
}

/// The monitor a setting names cannot be used.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MonitorError {
    /// No preset has this id.
    #[error("the monitor profile {0:?} is not installed; the safe default is used")]
    Unknown(String),
    /// `flat` with no EDID that lists a mode the system can send.
    #[error(
        "the monitor profile \"flat\" needs a display that lists its modes; the safe default is used"
    )]
    NoPanel,
    /// Neither the EDID nor the safe start preset is there.
    #[error(
        "no safe monitor profile: no EDID and no {SAFE_START_PROFILE} preset in the monitor folder"
    )]
    NoSafeProfile,
}

impl Detected {
    /// What `platform` shows for the output `output`, with the
    /// `subconnector` property of that output when the backend read it.
    ///
    /// A named output (the connector the backend drives, or the one the
    /// settings name) uses only its own EDID, whatever its status: a VGA
    /// output with no EDID is often `unknown`, and it must never get the
    /// profile of another display. A name that no connector has gives no
    /// EDID. Only `auto` takes the first connected display that gives a
    /// profile. An EDID that gives no profile is skipped, with a notice
    /// when it is the EDID of the named output.
    pub fn of(platform: &Platform, output: &str, subconnector: Option<&str>) -> Self {
        let connected: Vec<_> = platform
            .connectors
            .iter()
            .filter(|c| c.status == ConnectorStatus::Connected)
            .collect();
        let named = platform.connectors.iter().find(|c| c.name == output);
        let mut notices = Vec::new();
        let (source, edid) = match named {
            Some(c) => (Some(c), profile_of(c, subconnector, &mut notices)),
            None if output == AUTO => connected
                .iter()
                .find_map(|c| profile_of(c, None, &mut Vec::new()).map(|p| (Some(*c), Some(p))))
                .unwrap_or((None, None)),
            None => (None, None),
        };
        let panel = source
            .and_then(|c| c.edid.as_deref())
            .and_then(|e| panel_profile_from_edid(e).ok());
        let chosen = named.or(source).or_else(|| connected.first().copied());
        let matrix = Matrix::builtin().ok();
        let gpu = chosen.and_then(|c| Some((platform.gpu_of(c)?, c.kind.as_str())));
        let caps = gpu.and_then(|(g, kind)| Some(matrix.as_ref()?.caps_for(g, kind)));
        let digital_caps = gpu.and_then(|(g, kind)| {
            Some(
                matrix
                    .as_ref()?
                    .caps_for_output(g, kind, OutputKind::Digital),
            )
        });
        let output = source
            .and_then(|c| {
                let sub = if Some(c) == named { subconnector } else { None };
                display_class(&c.kind, sub, c.edid.as_deref()?).ok()
            })
            .map_or(OutputKind::Analog, DisplayClass::output_kind);
        Self {
            outputs: connected.iter().map(|c| c.name.clone()).collect(),
            edid,
            panel,
            caps,
            output,
            digital_caps,
            notices,
        }
    }

    /// Reads the platform under `sysfs`. A root that cannot be read (a
    /// desktop, a test) gives nothing detected.
    pub fn read(sysfs: &Path, output: &str, subconnector: Option<&str>) -> Self {
        neptron_platform::detect(sysfs)
            .map(|p| Self::of(&p, output, subconnector))
            .unwrap_or_default()
    }

    /// Cuts the flat-panel profiles to the modes that `caps` and the
    /// panel buffer limits allow. A panel whose preferred mode does not fit
    /// starts in its largest mode that fits, with a notice; a panel with no
    /// mode that fits gives no profile, with a notice.
    pub fn fit_to(&mut self, caps: &GpuCaps) {
        let limits = format!(
            "{}x{} at {} MHz",
            caps.max_width.min(PANEL_WIDTH_MAX_PX),
            caps.max_height.min(PANEL_HEIGHT_MAX_PX),
            caps.pclock_max_hz / 1_000_000
        );
        let mut notices = Vec::new();
        for slot in [&mut self.edid, &mut self.panel] {
            let Some(profile) = slot.take() else {
                continue;
            };
            match profile.fit_panel(caps, PANEL_WIDTH_MAX_PX, PANEL_HEIGHT_MAX_PX) {
                Some(fit) => {
                    if let (Some(native), Some(shown)) =
                        (fit.native_left_out, fit.profile.panel_mode())
                    {
                        notices.push(format!(
                            "The panel's native mode {} is above what this system sends ({limits}). It runs at {}.",
                            mode_text(&native),
                            mode_text(shown)
                        ));
                    }
                    *slot = Some(fit.profile);
                }
                None => notices.push(format!(
                    "The panel lists no mode that this system sends ({limits})."
                )),
            }
        }
        notices.dedup();
        self.notices.extend(notices);
    }

    /// The safe default for the detected output, with the picture style
    /// `auto` and the refresh policy `exact`.
    ///
    /// - A CRT that gives EDID: its range profile.
    /// - A flat panel whose profile is `confirmed` (the last known good on
    ///   this output): the panel in its native mode.
    /// - Every other case, an unconfirmed panel included: the 15 kHz safe
    ///   start. An adapter that shows its own digital EDID with a CRT
    ///   behind it looks like a panel, so a panel mode is never the safe
    ///   default until the user has kept it on this output.
    ///
    /// # Errors
    ///
    /// [`MonitorError::NoSafeProfile`].
    pub fn safe(
        &self,
        presets: &Presets,
        confirmed: Option<&Monitor>,
    ) -> Result<DisplayConfig, MonitorError> {
        let monitor = match &self.edid {
            Some(edid) if !is_panel(edid) => Monitor::Edid(edid.clone()),
            Some(edid) if confirmed == Some(&Monitor::Edid(edid.clone())) => {
                Monitor::Edid(edid.clone())
            }
            _ => safe_start(presets)?,
        };
        Ok(DisplayConfig {
            monitor,
            settings: safe_settings(),
        })
    }

    /// The monitor that `name` (a value of `display.monitor`) gives:
    /// `auto` is the profile of the detected display, or the 15 kHz safe
    /// start with no EDID.
    ///
    /// # Errors
    ///
    /// [`MonitorError::Unknown`] for an id with no preset,
    /// [`MonitorError::NoPanel`] for [`FLAT`] with no panel profile,
    /// [`MonitorError::NoSafeProfile`] for `auto` with neither.
    pub fn monitor(&self, name: &str, presets: &Presets) -> Result<Monitor, MonitorError> {
        match name {
            AUTO => match &self.edid {
                Some(edid) => Ok(Monitor::Edid(edid.clone())),
                None => safe_start(presets),
            },
            FLAT => self
                .panel
                .clone()
                .map(Monitor::Edid)
                .ok_or(MonitorError::NoPanel),
            _ => presets
                .get(name)
                .cloned()
                .map(Monitor::Profile)
                .ok_or_else(|| MonitorError::Unknown(name.to_owned())),
        }
    }

    /// Whether `auto` gives a flat panel.
    pub fn auto_is_panel(&self) -> bool {
        self.edid.as_ref().is_some_and(is_panel)
    }

    /// The monitor of a guard record entry again: a preset by its id, or
    /// a profile of the connected display's EDID when its id matches.
    pub fn rebuild(&self, presets: &Presets, entry: &MonitorRef) -> Option<Monitor> {
        match entry {
            MonitorRef::Profile { id } => presets.get(id).cloned().map(Monitor::Profile),
            MonitorRef::Edid { id } => [&self.edid, &self.panel]
                .into_iter()
                .flatten()
                .find(|e| &e.profile.id == id)
                .cloned()
                .map(Monitor::Edid),
        }
    }

    /// The value of `display.monitor` that names `monitor`: its preset id,
    /// `auto` for the profile the detection gives, `flat` for the panel
    /// profile of a display that the detection drives as a CRT.
    pub fn monitor_setting(&self, monitor: &Monitor) -> String {
        match monitor {
            Monitor::Profile(profile) => profile.id.clone(),
            Monitor::Edid(edid) if self.edid.as_ref() == Some(edid) => AUTO.to_owned(),
            Monitor::Edid(edid) if is_panel(edid) => FLAT.to_owned(),
            Monitor::Edid(_) => AUTO.to_owned(),
        }
    }

    /// What `auto` drives, for the display panel: the panel's name and the
    /// mode it starts in, the name of a CRT that gives EDID, or the safe
    /// start profile.
    pub fn auto_text(&self) -> String {
        match &self.edid {
            Some(edid) => describe(edid),
            None => SAFE_START_PROFILE.to_owned(),
        }
    }

    /// What `flat` drives, for the display panel; `None` when there is no
    /// panel profile.
    pub fn flat_text(&self) -> Option<String> {
        self.panel.as_ref().map(describe)
    }

    /// The kind of link that `name` (a value of `display.monitor`) drives:
    /// digital only when the output is a digital link to a flat panel and
    /// `name` gives that panel (`auto` or `flat`). A CRT preset on a
    /// digital output is ambiguous, so it is analog, and `flat` on an
    /// analog output stays analog.
    pub fn output_for(&self, name: &str) -> OutputKind {
        match (self.output, name) {
            (OutputKind::Digital, AUTO | FLAT) => OutputKind::Digital,
            _ => OutputKind::Analog,
        }
    }

    /// The caps of the KMS display for `name` (a value of
    /// `display.monitor`), with the scanout buffer size as the largest
    /// mode. An analog output gets the table's caps and buffers of
    /// 2560x1200 at most, as before panels. A digital link to a flat panel
    /// gets the table's digital limits, and buffers that also hold the
    /// panel's largest listed mode within the panel limits, so no listed
    /// mode is refused for its size. Each value is cut to the caps. With
    /// no table entry, the unknown-GPU caps.
    pub fn backend_caps(&self, name: &str) -> GpuCaps {
        let digital = self.output_for(name) == OutputKind::Digital;
        let caps = if digital {
            self.digital_caps.or(self.caps)
        } else {
            self.caps
        }
        .unwrap_or(neptron_platform::UNKNOWN_GPU_CAPS);
        let (panel_w, panel_h) = if digital {
            self.panel_size().unwrap_or((0, 0))
        } else {
            (0, 0)
        };
        GpuCaps {
            max_width: caps.max_width.min(CRT_BUFFER_WIDTH_PX.max(panel_w)),
            max_height: caps.max_height.min(CRT_BUFFER_HEIGHT_PX.max(panel_h)),
            ..caps
        }
    }

    /// Whether the display that `name` (a value of `display.monitor`)
    /// gives is a CRT, for the connector rule of the backend: a preset of
    /// the CRT kind, or `auto` with no flat-panel profile.
    pub fn is_crt(&self, name: &str, presets: &Presets) -> bool {
        match name {
            FLAT => self.panel.is_none(),
            AUTO => !self.edid.as_ref().is_some_and(is_panel),
            _ => !presets
                .get(name)
                .is_some_and(|p| matches!(p.kind, MonitorKind::FlatPanel { .. })),
        }
    }

    /// The largest flat-panel mode within the panel limits that a profile
    /// of this detection can ask for, as width and height: the scanout
    /// buffers must hold it. `None` when there is no panel profile.
    pub fn panel_size(&self) -> Option<(u16, u16)> {
        let modes = [&self.edid, &self.panel]
            .into_iter()
            .flatten()
            .filter_map(|e| match &e.profile.kind {
                MonitorKind::FlatPanel { modes } => Some(modes),
                MonitorKind::Crt { .. } => None,
            })
            .flatten()
            .filter(|m| m.hactive <= PANEL_WIDTH_MAX_PX && m.vactive <= PANEL_HEIGHT_MAX_PX);
        modes.fold(None, |size, m| {
            let (w, h) = size.unwrap_or((0, 0));
            Some((w.max(m.hactive), h.max(m.vactive)))
        })
    }
}

/// The profile of the EDID of `connector`, or `None` with a notice when
/// its EDID gives none.
fn profile_of(
    connector: &Connector,
    subconnector: Option<&str>,
    notices: &mut Vec<String>,
) -> Option<EdidProfile> {
    let edid = connector.edid.as_deref()?;
    profile_for_display(&connector.kind, subconnector, edid)
        .map_err(|e| {
            notices.push(format!(
                "The EDID of {} gives no profile ({e}); the safe start is used.",
                connector.name
            ));
        })
        .ok()
}

/// The preset of the 15 kHz safe start.
fn safe_start(presets: &Presets) -> Result<Monitor, MonitorError> {
    presets
        .get(SAFE_START_PROFILE)
        .cloned()
        .map(Monitor::Profile)
        .ok_or(MonitorError::NoSafeProfile)
}

fn is_panel(edid: &EdidProfile) -> bool {
    matches!(edid.profile.kind, MonitorKind::FlatPanel { .. })
}

/// The name and the first mode of a panel, or the name of a CRT.
fn describe(edid: &EdidProfile) -> String {
    let name = edid.name.as_deref().unwrap_or(&edid.profile.id);
    match edid.panel_mode() {
        Some(mode) => format!("{name}  {}", mode_text(mode)),
        None => format!("{name}  EDID ranges"),
    }
}

/// `1920x1080 60 Hz`, with the hundredths when the rate has them.
fn mode_text(mode: &Modeline) -> String {
    let rate = mode.field_rate_mhz();
    let hundredths = rate % 1_000 / 10;
    if hundredths == 0 {
        format!("{}x{} {} Hz", mode.hactive, mode.vactive, rate / 1_000)
    } else {
        format!(
            "{}x{} {}.{hundredths:02} Hz",
            mode.hactive,
            mode.vactive,
            rate / 1_000
        )
    }
}

/// Picture style `auto`, refresh policy `exact`.
fn safe_settings() -> PolicySettings {
    // Both values are choices of the schema, which the policy maps.
    policy_settings("auto", "exact", None).unwrap_or_default()
}

#[cfg(test)]
mod tests;
