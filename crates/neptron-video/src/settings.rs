use neptron_modeline::policy::{
    HorizontalStrategy, PictureStyle, PolicySettings, RefreshPolicy, TvWidth,
};
use neptron_types::{ScanlineMode, Scanlines};

/// A settings value that the policy does not know.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum SettingsError {
    /// The value of `display.style` is not a picture style.
    #[error("display.style {value:?} is not one of auto, native, 2x, 3x, 4x")]
    UnknownStyle {
        /// The value that was given.
        value: String,
    },
    /// The value of `display.refresh_policy` is not a refresh policy.
    #[error("display.refresh_policy {value:?} is not one of exact, exact_x2, force_50, force_60")]
    UnknownRefreshPolicy {
        /// The value that was given.
        value: String,
    },
    /// The value of `display.tv_width` is not a TV width.
    #[error("display.tv_width {value:?} is not one of aspect, integer, fill")]
    UnknownTvWidth {
        /// The value that was given.
        value: String,
    },
    /// The value of `display.scanlines` is not a scanline mode.
    #[error("display.scanlines {value:?} is not one of auto, off, on")]
    UnknownScanlineMode {
        /// The value that was given.
        value: String,
    },
    /// The value of `display.scanline_strength` is outside 0 to 100.
    #[error("display.scanline_strength {value} is outside 0 to {max}")]
    ScanlineStrength {
        /// The value that was given, in percent.
        value: i64,
        /// The largest value, in percent.
        max: u8,
    },
}

/// The scanline settings the engine applies to each scene: the resolved
/// values of `display.scanlines` and `display.scanline_strength`. A change
/// needs no modeset, only a new scene.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ScanlineSettings {
    /// The scanline mode.
    pub mode: ScanlineMode,
    /// How much a dark line loses, in whole percent.
    pub strength_percent: u8,
}

impl ScanlineSettings {
    /// No scanlines: what the engine draws until the session passes the
    /// settings.
    pub const OFF: Self = Self {
        mode: ScanlineMode::Off,
        strength_percent: 0,
    };
}

/// The scanline settings for the resolved values of `display.scanlines` and
/// `display.scanline_strength`.
///
/// # Errors
///
/// [`SettingsError::UnknownScanlineMode`] for a mode that is not a choice,
/// and [`SettingsError::ScanlineStrength`] for a strength outside 0 to 100.
/// The schema already refuses both.
pub fn scanline_settings(
    mode: &str,
    strength_percent: i64,
) -> Result<ScanlineSettings, SettingsError> {
    let mode = ScanlineMode::from_key(mode).ok_or_else(|| SettingsError::UnknownScanlineMode {
        value: mode.to_owned(),
    })?;
    let max = Scanlines::MAX_STRENGTH_PERCENT;
    let strength_percent = u8::try_from(strength_percent)
        .ok()
        .filter(|s| *s <= max)
        .ok_or(SettingsError::ScanlineStrength {
            value: strength_percent,
            max,
        })?;
    Ok(ScanlineSettings {
        mode,
        strength_percent,
    })
}

/// The policy settings for the resolved values of `display.style`,
/// `display.refresh_policy` and `display.tv_width`, and the horizontal
/// strategy that the system names (`None` for the GPU default). The
/// generator settings keep their defaults.
///
/// # Errors
///
/// [`SettingsError`] for a value that is not a choice of the schema. The
/// schema already refuses such a value, so this happens only when the
/// schema and the policy disagree.
pub fn policy_settings(
    style: &str,
    refresh_policy: &str,
    horizontal: Option<HorizontalStrategy>,
    tv_width: &str,
) -> Result<PolicySettings, SettingsError> {
    let style = PictureStyle::ALL
        .into_iter()
        .find(|s| s.key() == style)
        .ok_or_else(|| SettingsError::UnknownStyle {
            value: style.to_owned(),
        })?;
    let refresh = REFRESH_POLICIES
        .into_iter()
        .find(|r| r.key() == refresh_policy)
        .ok_or_else(|| SettingsError::UnknownRefreshPolicy {
            value: refresh_policy.to_owned(),
        })?;
    let tv_width = TvWidth::ALL
        .into_iter()
        .find(|w| w.key() == tv_width)
        .ok_or_else(|| SettingsError::UnknownTvWidth {
            value: tv_width.to_owned(),
        })?;
    Ok(PolicySettings {
        style,
        refresh,
        horizontal,
        tv_width,
        ..PolicySettings::default()
    })
}

const REFRESH_POLICIES: [RefreshPolicy; 4] = [
    RefreshPolicy::Exact,
    RefreshPolicy::ExactX2,
    RefreshPolicy::Force50,
    RefreshPolicy::Force60,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_choice_of_the_schema_maps_to_the_policy() {
        for &style in neptron_settings::PICTURE_STYLES {
            for &refresh in neptron_settings::REFRESH_POLICIES {
                let settings = policy_settings(style, refresh, None, "aspect").unwrap();
                assert_eq!(settings.style.key(), style);
                assert_eq!(settings.refresh.key(), refresh);
                assert_eq!(settings.horizontal, None);
            }
        }
        let widths: Vec<&str> = TvWidth::ALL.iter().map(|w| w.key()).collect();
        assert_eq!(
            neptron_settings::TV_WIDTHS,
            widths,
            "same names in the same order"
        );
        for &width in neptron_settings::TV_WIDTHS {
            let settings = policy_settings("auto", "exact", None, width).unwrap();
            assert_eq!(settings.tv_width.key(), width);
        }
        assert_eq!(
            policy_settings("auto", "exact", None, "aspect")
                .unwrap()
                .tv_width,
            TvWidth::Aspect,
            "the default of the schema is the default of the policy"
        );
    }

    #[test]
    fn every_tv_standard_of_the_schema_is_a_standard_of_the_types() {
        let keys: Vec<&str> = neptron_types::TvStandard::ALL
            .iter()
            .map(|s| s.key())
            .collect();
        assert_eq!(neptron_settings::TV_STANDARDS[0], "auto");
        assert_eq!(
            &neptron_settings::TV_STANDARDS[1..],
            keys,
            "same names in the same order"
        );
    }

    #[test]
    fn every_scanline_mode_of_the_schema_is_a_mode_of_the_scene_types() {
        let schema: Vec<&str> = neptron_settings::SCANLINE_MODES.to_vec();
        let types: Vec<&str> = ScanlineMode::ALL.iter().map(|m| m.key()).collect();
        assert_eq!(schema, types, "same names in the same order");
        for &key in neptron_settings::SCANLINE_MODES {
            let settings = scanline_settings(key, 50).unwrap();
            assert_eq!(settings.mode.key(), key);
            assert_eq!(settings.strength_percent, 50);
        }
        assert_eq!(
            neptron_settings::SCANLINE_STRENGTH_MAX_PERCENT,
            i64::from(Scanlines::MAX_STRENGTH_PERCENT)
        );
    }

    #[test]
    fn a_bad_scanline_value_is_an_error_with_the_value() {
        assert_eq!(
            scanline_settings("black", 50),
            Err(SettingsError::UnknownScanlineMode {
                value: "black".to_owned()
            })
        );
        for value in [-1, 101, 1_000] {
            assert_eq!(
                scanline_settings("on", value),
                Err(SettingsError::ScanlineStrength { value, max: 100 })
            );
        }
        assert!(scanline_settings("on", 0).is_ok());
        assert!(scanline_settings("on", 100).is_ok());
    }

    #[test]
    fn the_named_strategy_is_kept() {
        let fixed = Some(HorizontalStrategy::FixedSuperWidth { width: 2_560 });
        let settings = policy_settings("auto", "exact", fixed, "aspect").unwrap();
        assert_eq!(settings.horizontal, fixed);
        assert_eq!(settings.solver, PolicySettings::default().solver);
    }

    #[test]
    fn an_unknown_value_is_an_error_with_the_value() {
        assert_eq!(
            policy_settings("5x", "exact", None, "aspect"),
            Err(SettingsError::UnknownStyle {
                value: "5x".to_owned()
            })
        );
        assert_eq!(
            policy_settings("auto", "force_70", None, "aspect"),
            Err(SettingsError::UnknownRefreshPolicy {
                value: "force_70".to_owned()
            })
        );
        assert_eq!(
            policy_settings("auto", "exact", None, "wide"),
            Err(SettingsError::UnknownTvWidth {
                value: "wide".to_owned()
            })
        );
    }
}
