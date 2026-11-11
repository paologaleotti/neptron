//! What sysfs says about the GPUs and their connectors, and the pure
//! parsers of the sysfs text.

use thiserror::Error;

/// The GPUs and connectors that the kernel's DRM class lists.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Platform {
    /// The DRM devices (`card0`, `card1`, ...), sorted by number.
    pub gpus: Vec<Gpu>,
    /// The connectors of every card, sorted by card number and name.
    pub connectors: Vec<Connector>,
}

/// One DRM device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gpu {
    /// The DRM device name, for example `card0`.
    pub card: String,
    /// The kernel driver of the device, from `DRIVER=` in its uevent.
    pub driver: Option<String>,
    /// The PCI vendor and device id, from `PCI_ID=` in its uevent.
    pub pci_id: Option<PciId>,
    /// The device tree compatible strings, from `OF_COMPATIBLE_<n>=` in its
    /// uevent, in the order of `n`.
    pub of_compatible: Vec<String>,
}

/// A PCI vendor and device id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PciId {
    /// The vendor id, for example `0x8086` for Intel.
    pub vendor: u16,
    /// The device id.
    pub device: u16,
}

/// One connector of a DRM device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Connector {
    /// The DRM device name of its card, for example `card0`.
    pub card: String,
    /// The connector name, for example `VGA-1` or `HDMI-A-1`.
    pub name: String,
    /// The connector type, the name without its index: `VGA`, `HDMI-A`.
    pub kind: String,
    /// What the kernel says about a display on it.
    pub status: ConnectorStatus,
    /// The EDID the display gave, or `None` when the file is missing or
    /// empty.
    pub edid: Option<Vec<u8>>,
}

/// The `status` of a connector.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConnectorStatus {
    /// A display is connected.
    Connected,
    /// No display is connected.
    Disconnected,
    /// The driver cannot tell, as for a VGA output with no EDID.
    Unknown,
}

/// The parts of a uevent file that identify a GPU.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Uevent {
    /// `DRIVER=`.
    pub driver: Option<String>,
    /// `PCI_ID=`.
    pub pci_id: Option<PciId>,
    /// `OF_COMPATIBLE_<n>=`, in the order of `n`.
    pub of_compatible: Vec<String>,
}

/// The parts of a connector directory name such as `card0-HDMI-A-1`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectorName {
    /// `card0`.
    pub card: String,
    /// `HDMI-A-1`.
    pub name: String,
    /// `HDMI-A`.
    pub kind: String,
}

/// Why a sysfs text did not parse.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ParseError {
    /// A uevent line has no `=`.
    #[error("uevent line {line:?} has no '='")]
    UeventLine {
        /// The line.
        line: String,
    },
    /// `PCI_ID=` is not two hexadecimal ids of 4 digits.
    #[error("PCI_ID {value:?} is not VVVV:DDDD in hexadecimal")]
    PciId {
        /// The value after `PCI_ID=`.
        value: String,
    },
    /// An `OF_COMPATIBLE_<n>` key has no number, or a number twice.
    #[error("uevent key {key:?} is not a valid OF_COMPATIBLE_<n> key")]
    OfCompatible {
        /// The key.
        key: String,
    },
    /// A connector status is not one the kernel writes.
    #[error("connector status {value:?} is not connected, disconnected or unknown")]
    Status {
        /// The text of the file, trimmed.
        value: String,
    },
}

/// Parses a uevent file and keeps the keys that identify a GPU.
///
/// # Errors
///
/// Returns a [`ParseError`] for a line without `=`, a bad `PCI_ID` or a bad
/// `OF_COMPATIBLE_<n>` key.
pub fn parse_uevent(text: &str) -> Result<Uevent, ParseError> {
    let mut uevent = Uevent::default();
    let mut compatible: Vec<(u32, String)> = Vec::new();
    for line in text.lines().filter(|line| !line.is_empty()) {
        let Some((key, value)) = line.split_once('=') else {
            return Err(ParseError::UeventLine { line: line.into() });
        };
        match key {
            "DRIVER" => uevent.driver = Some(value.into()),
            "PCI_ID" => uevent.pci_id = Some(parse_pci_id(value)?),
            "OF_COMPATIBLE_N" => {}
            _ => {
                if let Some(index) = key.strip_prefix(OF_COMPATIBLE_PREFIX) {
                    let index: u32 = index
                        .parse()
                        .map_err(|_| ParseError::OfCompatible { key: key.into() })?;
                    if compatible.iter().any(|(seen, _)| *seen == index) {
                        return Err(ParseError::OfCompatible { key: key.into() });
                    }
                    compatible.push((index, value.into()));
                }
            }
        }
    }
    compatible.sort_by_key(|(index, _)| *index);
    uevent.of_compatible = compatible.into_iter().map(|(_, value)| value).collect();
    Ok(uevent)
}

/// The card number of a DRM device directory name such as `card0`, or
/// `None` for any other name.
pub fn parse_card_name(name: &str) -> Option<u32> {
    let digits = name.strip_prefix(CARD_PREFIX)?;
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}

/// Splits a connector directory name such as `card0-HDMI-A-1`, or gives
/// `None` for any other name.
pub fn parse_connector_name(name: &str) -> Option<ConnectorName> {
    let (card, connector) = name.split_once('-')?;
    parse_card_name(card)?;
    let (kind, index) = connector.rsplit_once('-')?;
    if kind.is_empty() || index.is_empty() || !index.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some(ConnectorName {
        card: card.into(),
        name: connector.into(),
        kind: kind.into(),
    })
}

/// Parses the `status` file of a connector.
///
/// # Errors
///
/// Returns [`ParseError::Status`] for any text but the three the kernel
/// writes.
pub fn parse_status(text: &str) -> Result<ConnectorStatus, ParseError> {
    match text.trim_end_matches('\n') {
        "connected" => Ok(ConnectorStatus::Connected),
        "disconnected" => Ok(ConnectorStatus::Disconnected),
        "unknown" => Ok(ConnectorStatus::Unknown),
        other => Err(ParseError::Status {
            value: other.into(),
        }),
    }
}

impl Platform {
    /// The GPU that owns `connector`.
    pub fn gpu_of(&self, connector: &Connector) -> Option<&Gpu> {
        self.gpus.iter().find(|gpu| gpu.card == connector.card)
    }
}

const CARD_PREFIX: &str = "card";
const OF_COMPATIBLE_PREFIX: &str = "OF_COMPATIBLE_";

fn parse_pci_id(value: &str) -> Result<PciId, ParseError> {
    let bad = || ParseError::PciId {
        value: value.into(),
    };
    let (vendor, device) = value.split_once(':').ok_or_else(bad)?;
    let hex = |part: &str| {
        if part.len() == 4 && part.bytes().all(|b| b.is_ascii_hexdigit()) {
            u16::from_str_radix(part, 16).map_err(|_| bad())
        } else {
            Err(bad())
        }
    };
    Ok(PciId {
        vendor: hex(vendor)?,
        device: hex(device)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pci_uevent() {
        let text = "DRIVER=i915\nPCI_CLASS=30000\nPCI_ID=8086:1912\n\
                    PCI_SUBSYS_ID=17AA:30BC\nPCI_SLOT_NAME=0000:00:02.0\n\
                    MODALIAS=pci:v00008086d00001912sv000017AAsd000030BCbc03sc00i00\n";
        assert_eq!(
            parse_uevent(text),
            Ok(Uevent {
                driver: Some("i915".into()),
                pci_id: Some(PciId {
                    vendor: 0x8086,
                    device: 0x1912
                }),
                of_compatible: vec![],
            })
        );
    }

    #[test]
    fn device_tree_uevent_sorts_compatible_strings() {
        let text = "DRIVER=vc4-drm\nOF_NAME=gpu\nOF_COMPATIBLE_1=second\n\
                    OF_COMPATIBLE_0=brcm,bcm2835-vc4\nOF_COMPATIBLE_N=2\n";
        let uevent = parse_uevent(text).unwrap();
        assert_eq!(uevent.of_compatible, ["brcm,bcm2835-vc4", "second"]);
        assert_eq!(uevent.pci_id, None);
    }

    #[test]
    fn bad_uevents_are_refused() {
        assert!(matches!(
            parse_uevent("DRIVER"),
            Err(ParseError::UeventLine { .. })
        ));
        for id in ["8086", "8086:191", "zzzz:1912", "8086:19120", "+086:1912"] {
            assert_eq!(
                parse_uevent(&format!("PCI_ID={id}")),
                Err(ParseError::PciId { value: id.into() })
            );
        }
        assert!(matches!(
            parse_uevent("OF_COMPATIBLE_X=a"),
            Err(ParseError::OfCompatible { .. })
        ));
        assert!(matches!(
            parse_uevent("OF_COMPATIBLE_0=a\nOF_COMPATIBLE_0=b"),
            Err(ParseError::OfCompatible { .. })
        ));
    }

    #[test]
    fn card_names() {
        assert_eq!(parse_card_name("card0"), Some(0));
        assert_eq!(parse_card_name("card12"), Some(12));
        for name in ["card", "card-1", "card0-VGA-1", "renderD128", "version"] {
            assert_eq!(parse_card_name(name), None, "{name}");
        }
    }

    #[test]
    fn connector_names() {
        assert_eq!(
            parse_connector_name("card0-HDMI-A-1"),
            Some(ConnectorName {
                card: "card0".into(),
                name: "HDMI-A-1".into(),
                kind: "HDMI-A".into(),
            })
        );
        assert_eq!(parse_connector_name("card1-VGA-1").unwrap().kind, "VGA");
        for name in [
            "card0",
            "card0-VGA",
            "card0-VGA-x",
            "cardx-VGA-1",
            "card0--1",
        ] {
            assert_eq!(parse_connector_name(name), None, "{name}");
        }
    }

    #[test]
    fn statuses() {
        assert_eq!(parse_status("connected\n"), Ok(ConnectorStatus::Connected));
        assert_eq!(
            parse_status("disconnected\n"),
            Ok(ConnectorStatus::Disconnected)
        );
        assert_eq!(parse_status("unknown"), Ok(ConnectorStatus::Unknown));
        assert_eq!(
            parse_status("on\n"),
            Err(ParseError::Status { value: "on".into() })
        );
    }
}
