use neptron_modeline::policy::{HorizontalStrategy, PictureStyle, PolicySettings, RefreshPolicy};

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
}

/// The policy settings for the resolved values of `display.style` and
/// `display.refresh_policy`, and the horizontal strategy that the system
/// names (`None` for the GPU default). The generator settings keep their
/// defaults.
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
    Ok(PolicySettings {
        style,
        refresh,
        horizontal,
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
                let settings = policy_settings(style, refresh, None).unwrap();
                assert_eq!(settings.style.key(), style);
                assert_eq!(settings.refresh.key(), refresh);
                assert_eq!(settings.horizontal, None);
            }
        }
    }

    #[test]
    fn the_named_strategy_is_kept() {
        let fixed = Some(HorizontalStrategy::FixedSuperWidth { width: 2_560 });
        let settings = policy_settings("auto", "exact", fixed).unwrap();
        assert_eq!(settings.horizontal, fixed);
        assert_eq!(settings.solver, PolicySettings::default().solver);
    }

    #[test]
    fn an_unknown_value_is_an_error_with_the_value() {
        assert_eq!(
            policy_settings("5x", "exact", None),
            Err(SettingsError::UnknownStyle {
                value: "5x".to_owned()
            })
        );
        assert_eq!(
            policy_settings("auto", "force_70", None),
            Err(SettingsError::UnknownRefreshPolicy {
                value: "force_70".to_owned()
            })
        );
    }
}
