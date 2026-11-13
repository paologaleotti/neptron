//! The TV standard of a composite encoder on the wire: which value of the
//! connector property `TV mode` a modeset sets, and which standard a value
//! read back is. Pure: the backend reads the property and sends the
//! request.

use neptron_types::{Modeline, TvStandard};

/// The `TV mode` property of a connector: its id and its enum values, by
/// the names the kernel gives them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TvModeProperty {
    /// The property id.
    pub prop: u32,
    /// Each value of the enum with its name, such as `(3, "PAL")`.
    pub values: Vec<(u64, String)>,
}

/// The `TV mode` value that a modeset sets on the connector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TvWrite {
    /// The property id of `TV mode`.
    pub prop: u32,
    /// The enum value of the standard.
    pub value: u64,
}

/// Why the backend cannot set the TV standard of a mode, or why the
/// standard read back is not the standard that was set.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(crate) enum TvRefusal {
    /// The mode carries a standard and the connector has no `TV mode`
    /// property.
    #[error("the mode carries the TV standard {}, and the connector has no TV mode property",
        .standard.key())]
    NoProperty {
        /// The standard of the mode.
        standard: TvStandard,
    },
    /// The connector's `TV mode` property has no value of the name.
    #[error("the connector has no TV mode value {} for the standard {}; it offers: {offered}",
        .standard.kms_name(), .standard.key())]
    NoValue {
        /// The standard of the mode.
        standard: TvStandard,
        /// The names the property offers, separated by commas.
        offered: String,
    },
    /// The connector sets a TV standard, and the mode carries none: the
    /// standard on the wire would be one that was not validated with the
    /// mode.
    #[error("the connector has a TV mode property, and the mode carries no TV standard")]
    NoStandard,
    /// The `TV mode` value read back is not a value of the property.
    #[error("the TV mode value {value} read back is not a value of the property")]
    UnknownValue {
        /// The value read back.
        value: u64,
    },
    /// The standard read back is not the standard that was set.
    #[error("the TV mode read back is {name} on a line of {htotal} samples, not the standard {}",
        .requested.key())]
    Mismatch {
        /// The standard that was set.
        requested: TvStandard,
        /// The name of the value read back.
        name: String,
        /// The horizontal total of the mode read back.
        htotal: u16,
    },
}

/// The `TV mode` value to set for a mode with `requested` standard on a
/// connector with `property`. `None` when the connector has no such
/// property and the mode carries no standard.
///
/// A mode with no standard on a connector with the property is refused:
/// the encoder would keep whatever standard is on the connector, which
/// did not pass validation with the mode.
pub(crate) fn plan(
    requested: Option<TvStandard>,
    property: Option<&TvModeProperty>,
) -> Result<Option<TvWrite>, TvRefusal> {
    match (requested, property) {
        (None, None) => Ok(None),
        (None, Some(_)) => Err(TvRefusal::NoStandard),
        (Some(standard), None) => Err(TvRefusal::NoProperty { standard }),
        (Some(standard), Some(p)) => p
            .values
            .iter()
            .find(|(_, name)| name == standard.kms_name())
            .map(|&(value, _)| {
                Some(TvWrite {
                    prop: p.prop,
                    value,
                })
            })
            .ok_or_else(|| TvRefusal::NoValue {
                standard,
                offered: p
                    .values
                    .iter()
                    .map(|(_, name)| name.as_str())
                    .collect::<Vec<_>>()
                    .join(", "),
            }),
    }
}

/// The standard of the `TV mode` value `name` on a mode with horizontal
/// total `htotal`. PAL and PAL-60 share the name `PAL`, and the two
/// monochrome standards share `Mono`; the encoder tells them apart by the
/// horizontal total of the mode, 858 samples for 525 lines and 864 for
/// 625, and so does this function. `None` for a pair that is no standard.
pub(crate) fn standard_of(name: &str, htotal: u16) -> Option<TvStandard> {
    TvStandard::ALL
        .into_iter()
        .find(|s| s.kms_name() == name && s.modes().iter().all(|m| m.htotal == htotal))
}

/// Checks that the `TV mode` value `value` read back after a modeset is
/// `requested` on the mode `readback`.
pub(crate) fn check_readback(
    requested: TvStandard,
    value: u64,
    property: &TvModeProperty,
    readback: &Modeline,
) -> Result<(), TvRefusal> {
    let name = property
        .values
        .iter()
        .find(|(v, _)| *v == value)
        .map(|(_, name)| name.as_str())
        .ok_or(TvRefusal::UnknownValue { value })?;
    if standard_of(name, readback.htotal) == Some(requested) {
        Ok(())
    } else {
        Err(TvRefusal::Mismatch {
            requested,
            name: name.to_owned(),
            htotal: readback.htotal,
        })
    }
}

/// The standards whose names `property` offers. A name stands for both
/// line systems where it has two standards (`PAL`, `Mono`); the driver's
/// atomic check refuses a pair it does not have.
pub(crate) fn offered(property: &TvModeProperty) -> Vec<TvStandard> {
    TvStandard::ALL
        .into_iter()
        .filter(|s| property.values.iter().any(|(_, n)| n == s.kms_name()))
        .collect()
}

#[cfg(test)]
mod tests {
    use neptron_types::{TV_525_240P, TV_525_480I, TV_625_288P, TV_625_576I};

    use super::*;

    /// The values of the `TV mode` property as `drm_tv_mode_enum_list`
    /// numbers them, for a driver that offers every standard.
    fn all() -> TvModeProperty {
        let names = [
            "NTSC", "NTSC-443", "NTSC-J", "PAL", "PAL-M", "PAL-N", "SECAM", "Mono",
        ];
        TvModeProperty {
            prop: 77,
            values: names
                .iter()
                .zip(0_u64..)
                .map(|(n, v)| (v, (*n).to_owned()))
                .collect(),
        }
    }

    fn value_of(name: &str) -> u64 {
        all().values.iter().find(|(_, n)| n == name).unwrap().0
    }

    #[test]
    fn each_standard_sets_its_value_by_name() {
        for s in TvStandard::ALL {
            assert_eq!(
                plan(Some(s), Some(&all())),
                Ok(Some(TvWrite {
                    prop: 77,
                    value: value_of(s.kms_name()),
                })),
                "{s:?}"
            );
        }
        assert_eq!(
            plan(Some(TvStandard::Pal60), Some(&all())),
            plan(Some(TvStandard::Pal), Some(&all()))
        );
    }

    #[test]
    fn no_standard_on_a_connector_without_the_property_sets_nothing() {
        assert_eq!(plan(None, None), Ok(None));
    }

    #[test]
    fn a_standard_needs_the_property() {
        let e = plan(Some(TvStandard::Ntsc), None).unwrap_err();
        assert_eq!(
            e,
            TvRefusal::NoProperty {
                standard: TvStandard::Ntsc
            }
        );
        assert_eq!(
            e.to_string(),
            "the mode carries the TV standard ntsc, and the connector has no TV mode property"
        );
    }

    #[test]
    fn a_standard_needs_its_value() {
        let only_ntsc = TvModeProperty {
            prop: 5,
            values: vec![(0, "NTSC".to_owned()), (2, "NTSC-J".to_owned())],
        };
        let e = plan(Some(TvStandard::Secam), Some(&only_ntsc)).unwrap_err();
        assert_eq!(
            e.to_string(),
            "the connector has no TV mode value SECAM for the standard secam; it offers: NTSC, \
             NTSC-J"
        );
        assert!(plan(Some(TvStandard::Mono50), Some(&only_ntsc)).is_err());
        let empty = TvModeProperty {
            prop: 5,
            values: Vec::new(),
        };
        assert!(matches!(
            plan(Some(TvStandard::Ntsc), Some(&empty)),
            Err(TvRefusal::NoValue { .. })
        ));
    }

    #[test]
    fn a_mode_without_a_standard_on_a_tv_connector_is_refused() {
        assert_eq!(plan(None, Some(&all())), Err(TvRefusal::NoStandard));
    }

    #[test]
    fn the_name_and_the_line_system_decode_each_of_the_ten_standards() {
        for s in TvStandard::ALL {
            for m in s.modes() {
                assert_eq!(standard_of(s.kms_name(), m.htotal), Some(s), "{s:?}");
            }
        }
        assert_eq!(standard_of("PAL", 858), Some(TvStandard::Pal60));
        assert_eq!(standard_of("PAL", 864), Some(TvStandard::Pal));
        assert_eq!(standard_of("Mono", 858), Some(TvStandard::Mono60));
        assert_eq!(standard_of("Mono", 864), Some(TvStandard::Mono50));
    }

    #[test]
    fn a_pair_that_is_no_standard_decodes_to_nothing() {
        assert_eq!(standard_of("NTSC", 864), None);
        assert_eq!(standard_of("SECAM", 858), None);
        assert_eq!(standard_of("PAL-60", 858), None);
        assert_eq!(standard_of("PAL", 800), None);
        assert_eq!(standard_of("", 858), None);
    }

    #[test]
    fn the_readback_accepts_the_standard_that_was_set() {
        for s in TvStandard::ALL {
            for m in s.modes() {
                assert_eq!(
                    check_readback(s, value_of(s.kms_name()), &all(), &m),
                    Ok(()),
                    "{s:?}"
                );
            }
        }
    }

    #[test]
    fn the_readback_refuses_another_standard() {
        let e = check_readback(TvStandard::Pal60, value_of("NTSC"), &all(), &TV_525_240P);
        assert_eq!(
            e,
            Err(TvRefusal::Mismatch {
                requested: TvStandard::Pal60,
                name: "NTSC".to_owned(),
                htotal: 858
            })
        );
        assert_eq!(
            e.unwrap_err().to_string(),
            "the TV mode read back is NTSC on a line of 858 samples, not the standard pal_60"
        );
        // The same name on the other line system is another standard.
        assert!(check_readback(TvStandard::Pal60, value_of("PAL"), &all(), &TV_625_288P).is_err());
        assert!(check_readback(TvStandard::Pal, value_of("PAL"), &all(), &TV_525_480I).is_err());
        assert!(
            check_readback(TvStandard::Mono50, value_of("Mono"), &all(), &TV_525_240P).is_err()
        );
        assert!(
            check_readback(TvStandard::Mono60, value_of("Mono"), &all(), &TV_625_576I).is_err()
        );
    }

    #[test]
    fn the_readback_refuses_a_value_the_property_does_not_have() {
        assert_eq!(
            check_readback(TvStandard::Ntsc, 99, &all(), &TV_525_240P),
            Err(TvRefusal::UnknownValue { value: 99 })
        );
    }

    #[test]
    fn the_offered_standards_follow_the_names() {
        assert_eq!(offered(&all()), TvStandard::ALL.to_vec());
        let some = TvModeProperty {
            prop: 1,
            values: vec![(3, "PAL".to_owned()), (7, "Mono".to_owned())],
        };
        assert_eq!(
            offered(&some),
            vec![
                TvStandard::Pal60,
                TvStandard::Pal,
                TvStandard::Mono60,
                TvStandard::Mono50
            ]
        );
        let none = TvModeProperty {
            prop: 1,
            values: Vec::new(),
        };
        assert!(offered(&none).is_empty());
    }
}
