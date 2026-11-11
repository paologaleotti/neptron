//! Monitor presets from `data/monitors` and modes that validate against
//! them, so that a test gets a real `ValidatedMode`.

use std::path::PathBuf;
use std::{fs, io};

use neptron_safety::{Rejection, ValidatedMode, validate};
use neptron_types::{GpuCaps, Modeline, MonitorProfile};

use crate::golden::workspace_root;

/// GPU capabilities with a low pixel-clock floor, so that native 15 kHz
/// modes such as [`MODE_240P`] pass. Interlace is on, one flip per frame.
pub const CAPS_LOW_CLOCK: GpuCaps = GpuCaps {
    pclock_min_hz: 1_000_000,
    pclock_max_hz: 400_000_000,
    interlace: true,
    flip_per_field: false,
    max_width: 4096,
    max_height: 4096,
};

/// 320x240 progressive at 60.03 Hz and 15.728 kHz. It passes the
/// `generic_15` preset with [`CAPS_LOW_CLOCK`].
pub const MODE_240P: Modeline = Modeline {
    pclock_hz: 6_700_000,
    hactive: 320,
    hsync_start: 334,
    hsync_end: 366,
    htotal: 426,
    vactive: 240,
    vsync_start: 241,
    vsync_end: 244,
    vtotal: 262,
    interlace: false,
    doublescan: false,
    hsync_positive: false,
    vsync_positive: false,
};

/// 320x288 progressive at 50.41 Hz and 15.728 kHz. It passes the
/// `generic_15` preset with [`CAPS_LOW_CLOCK`].
pub const MODE_288P: Modeline = Modeline {
    vactive: 288,
    vsync_start: 289,
    vsync_end: 292,
    vtotal: 312,
    ..MODE_240P
};

/// 640x480 interlaced at 59.915 Hz per field and 15.728 kHz. It passes the
/// `generic_15` preset with [`CAPS_LOW_CLOCK`].
pub const MODE_480I: Modeline = Modeline {
    hsync_start: 668,
    hsync_end: 732,
    htotal: 852,
    hactive: 640,
    pclock_hz: 13_400_000,
    vactive: 480,
    vsync_start: 482,
    vsync_end: 488,
    vtotal: 525,
    interlace: true,
    ..MODE_240P
};

/// Why a preset cannot be loaded or a mode does not pass it.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum PresetError {
    /// The id is not a plain file name of `[a-z0-9_]`.
    #[error("the preset id {id:?} must contain only a-z, 0-9 and _")]
    BadId {
        /// The id that was given.
        id: String,
    },
    /// The preset file cannot be read.
    #[error("cannot read the preset {}: {source}", path.display())]
    Read {
        /// The path of the preset file.
        path: PathBuf,
        /// The error of the read.
        source: io::Error,
    },
    /// The preset file is not a valid monitor profile.
    #[error("the preset {} is not a valid monitor profile: {reason}", path.display())]
    Parse {
        /// The path of the preset file.
        path: PathBuf,
        /// What the TOML parser reported.
        reason: String,
    },
    /// The mode does not pass the preset.
    #[error("the preset {id} rejects the mode {mode:?}: {rejection}")]
    Rejected {
        /// The preset id.
        id: String,
        /// The mode that was checked.
        mode: Modeline,
        /// Why `validate` refused it.
        rejection: Rejection,
    },
}

/// The monitor profile `data/monitors/<id>.toml` of the workspace.
///
/// # Errors
///
/// [`PresetError::BadId`] for an id that is not a plain file name,
/// [`PresetError::Read`] or [`PresetError::Parse`] when the file cannot be
/// read or parsed.
pub fn monitor_preset(id: &str) -> Result<MonitorProfile, PresetError> {
    let plain = !id.is_empty()
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_');
    if !plain {
        return Err(PresetError::BadId { id: id.to_owned() });
    }
    let path = workspace_root()
        .join("data")
        .join("monitors")
        .join(format!("{id}.toml"));
    let text = fs::read_to_string(&path).map_err(|source| PresetError::Read {
        path: path.clone(),
        source,
    })?;
    parse(path, &text)
}

/// Validates `mode` against the preset `id` and `caps`, and returns the
/// [`ValidatedMode`] that a display takes.
///
/// # Errors
///
/// The errors of [`monitor_preset`], and [`PresetError::Rejected`] when
/// `validate` refuses the mode.
pub fn validated_mode(
    mode: &Modeline,
    id: &str,
    caps: &GpuCaps,
) -> Result<ValidatedMode, PresetError> {
    let profile = monitor_preset(id)?;
    validate(mode, &profile, caps).map_err(|rejection| PresetError::Rejected {
        id: id.to_owned(),
        mode: *mode,
        rejection,
    })
}

fn parse(path: PathBuf, text: &str) -> Result<MonitorProfile, PresetError> {
    toml::from_str(text).map_err(|e| PresetError::Parse {
        path,
        reason: e.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_broken_file_is_a_parse_error() {
        let err = parse(PathBuf::from("x.toml"), "id = 3").unwrap_err();
        assert!(matches!(err, PresetError::Parse { .. }));
        assert!(
            err.to_string()
                .starts_with("the preset x.toml is not a valid monitor profile: ")
        );
    }

    #[test]
    fn the_shipped_modes_pass_generic_15() {
        for mode in [MODE_240P, MODE_288P, MODE_480I] {
            let validated = validated_mode(&mode, "generic_15", &CAPS_LOW_CLOCK).unwrap();
            assert_eq!(validated.mode(), &mode);
        }
        assert_eq!(MODE_240P.field_rate_mhz(), 60_029);
        assert_eq!(MODE_288P.field_rate_mhz(), 50_409);
        assert_eq!(MODE_480I.field_rate_mhz(), 59_915);
    }

    #[test]
    fn a_preset_loads_by_id() {
        let profile = monitor_preset("pc_31_120").unwrap();
        assert_eq!(profile.id, "pc_31_120");
    }

    #[test]
    fn errors_carry_the_context() {
        for bad in ["", "../pal", "PAL", "pal.toml"] {
            assert!(
                matches!(monitor_preset(bad), Err(PresetError::BadId { .. })),
                "{bad:?}"
            );
        }
        let missing = monitor_preset("no_such_preset").unwrap_err();
        assert!(matches!(missing, PresetError::Read { .. }));
        assert!(missing.to_string().contains("no_such_preset.toml"));

        let rejected = validated_mode(&MODE_240P, "pc_31_120", &CAPS_LOW_CLOCK).unwrap_err();
        let PresetError::Rejected { id, rejection, .. } = &rejected else {
            panic!("expected Rejected, got {rejected:?}");
        };
        assert_eq!(id, "pc_31_120");
        assert!(matches!(rejection, Rejection::LineRate { .. }));
    }
}
