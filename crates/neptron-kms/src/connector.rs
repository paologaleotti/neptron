/// What the connector choice needs to know about one connector.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectorFacts {
    /// The kernel name, such as `VGA-1` or `DP-1`.
    pub name: String,
    /// The kind of signal.
    pub class: ConnectorClass,
    /// The kernel reports a display on it.
    pub connected: bool,
    /// The display on it reports an EDID.
    pub has_edid: bool,
}

/// The kind of signal of a connector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectorClass {
    /// VGA, DVI-A, composite, S-Video, component, 9-pin DIN, TV and DPI
    /// (the GPIO DAC of a Raspberry Pi), and DVI-I with an analog EDID or
    /// no EDID.
    Analog,
    /// Every other output that drives a display.
    Digital,
    /// A writeback connector: it writes the picture to memory and is never
    /// a display.
    Writeback,
}

/// No connector can be chosen.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ChoiceError {
    /// The named connector does not exist.
    #[error("no connector is named {name}; the connectors are: {available}")]
    NotFound {
        /// The name from the settings.
        name: String,
        /// The names that exist, separated by commas.
        available: String,
    },
    /// No connector is connected, apart from analog connectors without an
    /// EDID, which are used only when named.
    #[error(
        "no connected display found; an analog connector that reports no EDID is used only \
         when the settings name it; the connectors are: {available}"
    )]
    NoneConnected {
        /// The names that exist, separated by commas.
        available: String,
    },
}

/// The DRM connector type numbers (`DRM_MODE_CONNECTOR_*`) of analog
/// outputs: VGA, DVI-I, DVI-A, composite, S-Video, component, 9-pin DIN,
/// TV and DPI. DVI-I carries both signals: [`ConnectorClass::of_connector`]
/// lets its EDID decide.
pub const ANALOG_TYPES: [u32; 9] = [1, 2, 4, 5, 6, 8, 9, 13, 17];
/// `DRM_MODE_CONNECTOR_DVII`: a DVI connector with the digital and the
/// analog pins.
pub const DVI_I_TYPE: u32 = 2;
/// The byte of the EDID base block that describes the video input.
pub const EDID_INPUT_OFFSET: usize = 0x14;
/// The bit of [`EDID_INPUT_OFFSET`] that is set for a digital input.
pub const EDID_DIGITAL_INPUT_BIT: u8 = 0x80;
/// `DRM_MODE_CONNECTOR_WRITEBACK`
pub const WRITEBACK_TYPE: u32 = 18;
/// The `subconnector` value of a DVI-I connector that drives its analog
/// pins.
pub const ANALOG_DVI_SUBCONNECTOR: &str = "DVI-A";

/// The length of an EDID block, in bytes.
const EDID_BLOCK_BYTES: usize = 128;
/// The fixed first 8 bytes of an EDID.
const EDID_HEADER: [u8; 8] = [0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x00];
/// The byte of the base block that counts the extension blocks.
const EDID_EXTENSIONS_OFFSET: usize = 0x7e;
/// The bytes of the version and the revision; 1.3 and 1.4 are read.
const EDID_VERSION_OFFSET: usize = 0x12;
const EDID_REVISION_OFFSET: usize = 0x13;
/// The manufacturer id, big-endian, with a reserved top bit.
const EDID_MANUFACTURER_OFFSET: usize = 0x08;
/// The first descriptor: a detailed timing, the preferred mode, has a
/// pixel clock that is not 0 in its first two bytes.
const EDID_FIRST_DESCRIPTOR_OFFSET: usize = 0x36;

/// The base block of `edid` when it passes the checks that
/// `neptron-platform` makes before it reads the descriptors: the length
/// with every extension, the header, the checksum of every block, version
/// 1.3 or 1.4, the manufacturer id, and a detailed timing first.
fn edid_base(edid: &[u8]) -> Option<&[u8]> {
    let base = edid.get(..EDID_BLOCK_BYTES)?;
    let blocks = 1 + usize::from(base[EDID_EXTENSIONS_OFFSET]);
    let checksums = edid
        .chunks_exact(EDID_BLOCK_BYTES)
        .all(|b| b.iter().fold(0u8, |s, x| s.wrapping_add(*x)) == 0);
    let raw = u16::from_be_bytes([
        base[EDID_MANUFACTURER_OFFSET],
        base[EDID_MANUFACTURER_OFFSET + 1],
    ]);
    let letters = [10u16, 5, 0]
        .iter()
        .all(|shift| (1..=26).contains(&((raw >> shift) & 0x1f)));
    let first = &base[EDID_FIRST_DESCRIPTOR_OFFSET..EDID_FIRST_DESCRIPTOR_OFFSET + 2];
    let valid = edid.len() == blocks * EDID_BLOCK_BYTES
        && base[..EDID_HEADER.len()] == EDID_HEADER
        && checksums
        && base[EDID_VERSION_OFFSET] == 1
        && (3..=4).contains(&base[EDID_REVISION_OFFSET])
        && raw & 0x8000 == 0
        && letters
        && first != [0, 0];
    valid.then_some(base)
}

impl ConnectorClass {
    /// The class of a DRM connector type number.
    pub fn of_type(kernel_type: u32) -> ConnectorClass {
        if kernel_type == WRITEBACK_TYPE {
            ConnectorClass::Writeback
        } else if ANALOG_TYPES.contains(&kernel_type) {
            ConnectorClass::Analog
        } else {
            ConnectorClass::Digital
        }
    }

    /// The class of a connector of DRM type `kernel_type` with its
    /// `subconnector` property and the EDID `edid` of its display. The
    /// same rule as `display_class` of `neptron-platform`, which this crate
    /// cannot depend on. DVI-I carries both signals, so it is digital only
    /// when the subconnector is not `DVI-A`, and the EDID passes the
    /// structural checks of `neptron-platform` that come before its
    /// descriptors and has the digital input bit. In every other case it
    /// is analog: analog wins when in doubt. Every other type has its class
    /// from [`ConnectorClass::of_type`].
    pub fn of_connector(
        kernel_type: u32,
        subconnector: Option<&str>,
        edid: Option<&[u8]>,
    ) -> ConnectorClass {
        if kernel_type != DVI_I_TYPE {
            return Self::of_type(kernel_type);
        }
        let analog_pins = subconnector == Some(ANALOG_DVI_SUBCONNECTOR);
        let digital = !analog_pins
            && edid
                .and_then(edid_base)
                .is_some_and(|base| base[EDID_INPUT_OFFSET] & EDID_DIGITAL_INPUT_BIT != 0);
        if digital {
            ConnectorClass::Digital
        } else {
            ConnectorClass::Analog
        }
    }
}

/// Chooses the output with the rule of the Definition and returns its
/// index in `connectors`. `named` is the output in the settings; `crt` is
/// true for a CRT monitor profile.
///
/// # Errors
///
/// [`ChoiceError`] when the named connector does not exist, or when no
/// connector qualifies.
pub fn choose(
    connectors: &[ConnectorFacts],
    named: Option<&str>,
    crt: bool,
) -> Result<usize, ChoiceError> {
    let displays = || {
        connectors
            .iter()
            .enumerate()
            .filter(|(_, c)| c.class != ConnectorClass::Writeback)
    };
    if let Some(name) = named {
        return displays()
            .find(|(_, c)| c.name == name)
            .map(|(i, _)| i)
            .ok_or_else(|| ChoiceError::NotFound {
                name: name.to_owned(),
                available: names(connectors),
            });
    }
    let candidates = || {
        displays()
            .filter(|(_, c)| c.connected && !(c.class == ConnectorClass::Analog && !c.has_edid))
    };
    let wanted = if crt {
        ConnectorClass::Analog
    } else {
        ConnectorClass::Digital
    };
    candidates()
        .find(|(_, c)| c.class == wanted)
        .or_else(|| candidates().next())
        .map(|(i, _)| i)
        .ok_or_else(|| ChoiceError::NoneConnected {
            available: names(connectors),
        })
}

fn names(connectors: &[ConnectorFacts]) -> String {
    let list: Vec<&str> = connectors
        .iter()
        .filter(|c| c.class != ConnectorClass::Writeback)
        .map(|c| c.name.as_str())
        .collect();
    if list.is_empty() {
        "none".to_owned()
    } else {
        list.join(", ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(name: &str, class: ConnectorClass, connected: bool, has_edid: bool) -> ConnectorFacts {
        ConnectorFacts {
            name: name.to_owned(),
            class,
            connected,
            has_edid,
        }
    }

    use ConnectorClass::{Analog, Digital, Writeback};

    #[test]
    fn a_named_connector_wins_in_any_state() {
        let list = [
            c("DP-1", Digital, true, true),
            c("VGA-1", Analog, false, false),
        ];
        assert_eq!(choose(&list, Some("VGA-1"), false), Ok(1));
        assert_eq!(choose(&list, Some("DP-1"), true), Ok(0));
    }

    #[test]
    fn a_missing_name_lists_the_connectors() {
        let list = [
            c("DP-1", Digital, true, true),
            c("Writeback-1", Writeback, true, false),
        ];
        assert_eq!(
            choose(&list, Some("HDMI-A-1"), false),
            Err(ChoiceError::NotFound {
                name: "HDMI-A-1".to_owned(),
                available: "DP-1".to_owned()
            })
        );
        assert!(choose(&list, Some("Writeback-1"), false).is_err());
    }

    #[test]
    fn one_connected_connector_is_used() {
        let list = [
            c("DP-1", Digital, false, false),
            c("HDMI-A-1", Digital, true, true),
        ];
        assert_eq!(choose(&list, None, true), Ok(1));
        let vkms = [
            c("Virtual-1", ConnectorClass::of_type(15), true, true),
            c("Writeback-1", ConnectorClass::of_type(18), true, false),
        ];
        assert_eq!(choose(&vkms, None, true), Ok(0));
    }

    #[test]
    fn an_analog_connector_without_edid_needs_a_name() {
        let list = [c("VGA-1", Analog, true, false)];
        assert_eq!(
            choose(&list, None, true),
            Err(ChoiceError::NoneConnected {
                available: "VGA-1".to_owned()
            })
        );
        let with_edid = [c("VGA-1", Analog, true, true)];
        assert_eq!(choose(&with_edid, None, true), Ok(0));
    }

    #[test]
    fn with_several_the_profile_picks_the_class() {
        let list = [
            c("DP-1", Digital, true, true),
            c("VGA-1", Analog, true, true),
            c("DP-2", Digital, true, true),
        ];
        assert_eq!(choose(&list, None, true), Ok(1));
        assert_eq!(choose(&list, None, false), Ok(0));
    }

    #[test]
    fn with_no_candidate_of_the_class_the_first_candidate_wins() {
        let digital = [
            c("VGA-1", Analog, true, false),
            c("DP-1", Digital, true, true),
            c("DP-2", Digital, true, true),
        ];
        assert_eq!(choose(&digital, None, true), Ok(1));
        let analog = [
            c("VGA-1", Analog, true, true),
            c("DVI-I-1", Analog, true, true),
        ];
        assert_eq!(choose(&analog, None, false), Ok(0));
    }

    #[test]
    fn nothing_connected_is_an_error() {
        assert_eq!(
            choose(&[], None, false),
            Err(ChoiceError::NoneConnected {
                available: "none".to_owned()
            })
        );
        let list = [c("DP-1", Digital, false, false)];
        assert!(choose(&list, None, false).is_err());
    }

    /// EDID blobs of `apps/neptron/tests/fixtures/edid`, made from the
    /// specification. `neptron-platform` reads the same files in its test of
    /// this table.
    const PANEL: &[u8] =
        include_bytes!("../../../apps/neptron/tests/fixtures/edid/panel_1080p.bin");
    const CRT: &[u8] = include_bytes!("../../../apps/neptron/tests/fixtures/edid/crt_analog.bin");

    fn broken_checksum() -> Vec<u8> {
        let mut b = PANEL.to_vec();
        b[0x40] ^= 0x01;
        b
    }

    fn broken_header() -> Vec<u8> {
        let mut b = PANEL.to_vec();
        b[1] = 0;
        b[127] = b[127].wrapping_add(0xff);
        b
    }

    /// What a DVI-I row is, its subconnector, its EDID and the class.
    type DviRow<'a> = (&'a str, Option<&'a str>, Option<&'a [u8]>, ConnectorClass);

    /// The DVI-I rows of the class rule: the same rows as the test
    /// `dvi_i_follows_the_kms_rule` of `neptron-platform`.
    #[test]
    fn dvi_i_takes_the_class_of_a_valid_edid_and_else_is_analog() {
        let checksum = broken_checksum();
        let header = broken_header();
        let rows: [DviRow<'_>; 7] = [
            ("a panel", None, Some(PANEL), Digital),
            ("a CRT", None, Some(CRT), Analog),
            (
                "a panel on the analog pins",
                Some("DVI-A"),
                Some(PANEL),
                Analog,
            ),
            (
                "a panel on the digital pins",
                Some("DVI-D"),
                Some(PANEL),
                Digital,
            ),
            ("a bad checksum", None, Some(&checksum), Analog),
            ("a bad header", None, Some(&header), Analog),
            ("no EDID", None, None, Analog),
        ];
        for (what, sub, edid, class) in rows {
            assert_eq!(
                ConnectorClass::of_connector(DVI_I_TYPE, sub, edid),
                class,
                "{what}"
            );
        }
        // VGA stays analog and HDMI-A digital, whatever the EDID says.
        assert_eq!(ConnectorClass::of_connector(1, None, Some(PANEL)), Analog);
        assert_eq!(ConnectorClass::of_connector(11, None, Some(CRT)), Digital);
    }

    #[test]
    fn kernel_types_map_to_classes() {
        for t in ANALOG_TYPES {
            assert_eq!(ConnectorClass::of_type(t), Analog, "{t}");
        }
        // HDMI-A, DisplayPort, eDP, Virtual, DSI, Unknown.
        for t in [11, 10, 14, 15, 16, 0] {
            assert_eq!(ConnectorClass::of_type(t), Digital, "{t}");
        }
        assert_eq!(ConnectorClass::of_type(WRITEBACK_TYPE), Writeback);
    }
}
