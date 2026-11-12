//! The class of a display (CRT or flat panel) from its connector and its
//! EDID, and the flat-panel profile of the modes an EDID lists.

use neptron_types::{GpuCaps, Modeline, MonitorKind, MonitorProfile, OutputKind};

use crate::edid::EdidError;
use crate::modes::listed_modes;
use crate::profile::{EdidProfile, ProfileError, profile_from_edid};

/// What a display is driven as.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DisplayClass {
    /// A display that takes any mode inside its ranges: the CRT path,
    /// also for an LCD with a VGA input.
    Crt,
    /// A display that takes only its listed modes.
    FlatPanel,
}

impl DisplayClass {
    /// The kind of link to a display of this class: a flat panel is on a
    /// digital link, a CRT (also an LCD with a VGA input) on an analog
    /// one.
    pub fn output_kind(self) -> OutputKind {
        match self {
            DisplayClass::Crt => OutputKind::Analog,
            DisplayClass::FlatPanel => OutputKind::Digital,
        }
    }
}

/// The connector types (the sysfs name without its index) that carry only
/// an analog signal.
pub const ANALOG_CONNECTOR_KINDS: [&str; 8] = [
    "VGA",
    "DVI-A",
    "Composite",
    "SVIDEO",
    "Component",
    "DIN",
    "TV",
    "DPI",
];

/// The values of the `subconnector` property of a DisplayPort or DVI-I
/// connector that name an analog downstream port: a DAC between the GPU
/// and the display.
pub const ANALOG_SUBCONNECTORS: [&str; 2] = ["VGA", "DVI-A"];

/// The class of the display on a connector of type `connector_kind`
/// (`HDMI-A`, `DP`, `VGA`), with the downstream port type `subconnector`
/// when the kernel gives one, and the EDID `edid`.
///
/// The rule, in order:
///
/// 1. An EDID that does not pass the checks of [`listed_modes`] gives an
///    error: the caller has no EDID and uses its safe start.
/// 2. An analog input (byte 0x14, bit 7 clear) is a CRT, on any connector:
///    an active DP-to-VGA or HDMI-to-VGA adapter passes the EDID of the
///    monitor behind it.
/// 3. An analog downstream port ([`ANALOG_SUBCONNECTORS`]) is a CRT, even
///    with a digital EDID: the adapter is a DAC.
/// 4. An analog-only connector ([`ANALOG_CONNECTOR_KINDS`]) is a CRT, even
///    with a digital EDID.
/// 5. Every other case is a digital input on a digital connector: a flat
///    panel. The parser has already proved that the first descriptor is a
///    detailed timing, the preferred mode.
///
/// A CRT with a digital EDID passes no CRT profile
/// ([`ProfileError::NotAnalog`]), so the caller uses its safe start.
///
/// # Errors
///
/// The [`EdidError`] of the EDID.
pub fn display_class(
    connector_kind: &str,
    subconnector: Option<&str>,
    edid: &[u8],
) -> Result<DisplayClass, EdidError> {
    let listed = listed_modes(edid)?;
    let analog_port = subconnector.is_some_and(|s| ANALOG_SUBCONNECTORS.contains(&s));
    let analog_connector = ANALOG_CONNECTOR_KINDS.contains(&connector_kind);
    if listed.analog || analog_port || analog_connector {
        Ok(DisplayClass::Crt)
    } else {
        Ok(DisplayClass::FlatPanel)
    }
}

/// The profile of the display by [`display_class`]: [`profile_from_edid`]
/// for a CRT, [`panel_profile_from_edid`] for a flat panel.
///
/// # Errors
///
/// The [`ProfileError`] of the class's builder, or of the EDID.
pub fn profile_for_display(
    connector_kind: &str,
    subconnector: Option<&str>,
    edid: &[u8],
) -> Result<EdidProfile, ProfileError> {
    match display_class(connector_kind, subconnector, edid)? {
        DisplayClass::Crt => profile_from_edid(edid),
        DisplayClass::FlatPanel => panel_profile_from_edid(edid),
    }
}

/// A flat-panel profile of the progressive modes an EDID lists, in the
/// order of [`listed_modes`]: the preferred mode first.
///
/// It does not look at the input type, so it also builds the profile of an
/// LCD with a VGA input when the user names it.
///
/// # Errors
///
/// [`ProfileError::Edid`], or [`ProfileError::NoListedModes`] when the
/// EDID lists no progressive mode that the parser reads.
pub fn panel_profile_from_edid(edid: &[u8]) -> Result<EdidProfile, ProfileError> {
    let listed = listed_modes(edid)?;
    let id = format!(
        "panel_{}_{:04x}",
        listed.manufacturer.to_ascii_lowercase(),
        listed.product_code
    );
    let Some(pclock_max_hz) = listed.modes.iter().map(|m| m.pclock_hz).max() else {
        return Err(ProfileError::NoListedModes { id });
    };
    Ok(EdidProfile {
        profile: MonitorProfile {
            id,
            kind: MonitorKind::FlatPanel {
                modes: listed.modes,
            },
        },
        pclock_max_hz,
        formula: None,
        name: listed.name,
    })
}

/// The lowest field rate of a mode that a fallback prefers, in millihertz:
/// a 50 Hz game runs at its speed from here up.
pub const GAME_RATE_MIN_MHZ: u64 = 50_000;

/// A flat-panel profile cut to what the system can send.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PanelFit {
    /// The profile with the modes that fit. Its first mode is the panel's
    /// preferred mode when that fits, else the largest mode that fits.
    pub profile: EdidProfile,
    /// The preferred mode of the panel when it does not fit, for a notice.
    pub native_left_out: Option<Modeline>,
}

impl EdidProfile {
    /// The modes of a flat-panel profile no wider than `max_width`, no
    /// taller than `max_height`, and with a pixel clock in the range of
    /// `caps`. When the preferred mode does not fit, the largest mode that
    /// fits comes first, so the policy takes it as the preferred size: a
    /// mode of at least [`GAME_RATE_MIN_MHZ`] before a slower one (2160p30
    /// would halve the speed of a 60 Hz game), then by area, then the rate
    /// nearest the preferred one, then list order.
    ///
    /// A CRT profile is returned unchanged. `None` when no listed mode
    /// fits.
    pub fn fit_panel(&self, caps: &GpuCaps, max_width: u16, max_height: u16) -> Option<PanelFit> {
        let MonitorKind::FlatPanel { modes } = &self.profile.kind else {
            return Some(PanelFit {
                profile: self.clone(),
                native_left_out: None,
            });
        };
        let native = *modes.first()?;
        let fits = |m: &&Modeline| {
            m.hactive <= max_width.min(caps.max_width)
                && m.vactive <= max_height.min(caps.max_height)
                && (caps.pclock_min_hz..=caps.pclock_max_hz).contains(&m.pclock_hz)
        };
        let mut kept: Vec<Modeline> = modes.iter().filter(fits).copied().collect();
        let native_left_out = (kept.first() != Some(&native)).then_some(native);
        if native_left_out.is_some() {
            let native_rate = native.field_rate_mhz();
            let best = kept
                .iter()
                .enumerate()
                .min_by_key(|(i, m)| {
                    let area = u32::from(m.hactive) * u32::from(m.vactive);
                    (
                        m.field_rate_mhz() < GAME_RATE_MIN_MHZ,
                        std::cmp::Reverse(area),
                        m.field_rate_mhz().abs_diff(native_rate),
                        *i,
                    )
                })
                .map(|(i, _)| i)?;
            let first = kept.remove(best);
            kept.insert(0, first);
        }
        let pclock_max_hz = kept.iter().map(|m| m.pclock_hz).max()?;
        Some(PanelFit {
            profile: EdidProfile {
                profile: MonitorProfile {
                    id: self.profile.id.clone(),
                    kind: MonitorKind::FlatPanel { modes: kept },
                },
                pclock_max_hz,
                formula: None,
                name: self.name.clone(),
            },
            native_left_out,
        })
    }

    /// The first listed mode of a flat-panel profile: the mode the panel
    /// starts in. `None` for a CRT profile.
    pub fn panel_mode(&self) -> Option<&Modeline> {
        match &self.profile.kind {
            MonitorKind::FlatPanel { modes } => modes.first(),
            MonitorKind::Crt { .. } => None,
        }
    }
}
