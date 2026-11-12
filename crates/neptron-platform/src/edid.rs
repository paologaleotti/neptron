//! A strict parser for the EDID base block: the identity of the display, its
//! input type and its display range limits descriptor.
//!
//! EDID is a safety input, so the parser checks the header, every block
//! checksum, the version and the layout of the four descriptors, and
//! refuses what it cannot verify. It reads EDID 1.3 and 1.4 only.

use thiserror::Error;

/// Length of one EDID block, in bytes.
pub const EDID_BLOCK_LEN: usize = 128;

/// The identity, input type and range limits of a display, from its EDID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edid {
    /// The three-letter manufacturer id, in upper case.
    pub manufacturer: String,
    /// The product code the manufacturer gives the model.
    pub product_code: u16,
    /// The EDID revision: 3 or 4. The version is always 1.
    pub revision: u8,
    /// The display has an analog video input.
    pub analog: bool,
    /// Bit 0 of byte 0x18 is set: the display accepts timings inside its
    /// range limits that it does not list.
    ///
    /// EDID 1.3 names the bit `default GTF supported`; EDID 1.4 names the
    /// same bit `continuous frequency`.
    pub continuous_frequency: bool,
    /// The display range limits descriptor, when the EDID has one.
    pub range_limits: Option<RangeLimits>,
}

/// The display range limits descriptor (tag `0xFD`), with the rate offsets
/// of EDID 1.4 applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RangeLimits {
    /// Lowest vertical field rate, in hertz.
    pub v_min_hz: u16,
    /// Highest vertical field rate, in hertz.
    pub v_max_hz: u16,
    /// Lowest horizontal line rate, in kilohertz.
    pub h_min_khz: u16,
    /// Highest horizontal line rate, in kilohertz.
    pub h_max_khz: u16,
    /// The highest pixel clock, in hertz.
    ///
    /// Byte 9 holds the highest pixel clock in units of 10 MHz, rounded up
    /// by the display. The true limit can then be up to 10 MHz lower, so
    /// this value is one unit less than byte 9. For CVT, EDID 1.4 gives the
    /// exact value: byte 9 minus the reduction in units of 250 kHz from
    /// byte 12.
    pub pclock_max_hz: u32,
    /// The timing formula the display names.
    pub formula: TimingFormula,
}

/// The secondary timing formula that a range limits descriptor names.
///
/// The profile built from an EDID uses none of the formula parameters: it
/// uses only the rates and the highest pixel clock.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TimingFormula {
    /// Byte 10 is `0x00`: the default GTF.
    DefaultGtf,
    /// Byte 10 is `0x01` (EDID 1.4 only): range limits only, no formula.
    RangeLimitsOnly,
    /// Byte 10 is `0x02`: GTF with a secondary curve.
    SecondaryGtf,
    /// Byte 10 is `0x04` (EDID 1.4 only): CVT.
    Cvt,
}

/// Why an EDID was refused.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum EdidError {
    /// The data is not a whole number of blocks, or is shorter than the
    /// extension count in the base block says.
    #[error("EDID is {len} bytes, expected {expected}")]
    Length {
        /// The length of the data, in bytes.
        len: usize,
        /// The length the data must have, in bytes.
        expected: usize,
    },
    /// The first 8 bytes are not the fixed EDID header.
    #[error("EDID header is {found:02x?}, expected 00 ff ff ff ff ff ff 00")]
    Header {
        /// The 8 bytes found.
        found: [u8; 8],
    },
    /// The bytes of a block do not add up to 0 modulo 256.
    #[error("EDID block {block} has checksum remainder {remainder:#04x}, expected 0")]
    Checksum {
        /// The index of the block, 0 for the base block.
        block: usize,
        /// The sum of the block bytes modulo 256.
        remainder: u8,
    },
    /// The version is not 1.3 or 1.4.
    #[error("EDID version {version}.{revision} is not supported, only 1.3 and 1.4")]
    Version {
        /// The version byte.
        version: u8,
        /// The revision byte.
        revision: u8,
    },
    /// The manufacturer id has its reserved bit set or a letter out of
    /// range.
    #[error("EDID manufacturer id {raw:#06x} is not three letters A to Z")]
    Manufacturer {
        /// The two bytes of the id, big-endian.
        raw: u16,
    },
    /// The first descriptor is not a detailed timing.
    #[error("the first EDID descriptor is not a detailed timing")]
    NoPreferredTiming,
    /// A display descriptor has a reserved byte that is not 0.
    #[error("EDID descriptor at {offset:#04x} has {value:#04x} in reserved byte {byte}")]
    DescriptorReserved {
        /// The offset of the descriptor in the base block.
        offset: usize,
        /// The index of the byte inside the descriptor.
        byte: usize,
        /// The value of the byte.
        value: u8,
    },
    /// A display descriptor has a reserved tag.
    #[error("EDID descriptor at {offset:#04x} has reserved tag {tag:#04x}")]
    ReservedTag {
        /// The offset of the descriptor in the base block.
        offset: usize,
        /// The tag byte.
        tag: u8,
    },
    /// The base block has more than one range limits descriptor.
    #[error("EDID has range limits descriptors at {first:#04x} and {second:#04x}")]
    DuplicateRangeLimits {
        /// The offset of the first descriptor.
        first: usize,
        /// The offset of the second descriptor.
        second: usize,
    },
    /// The rate offset flags are reserved, or set in EDID 1.3.
    #[error("EDID range limits offset flags {flags:#04x} are not valid for revision {revision}")]
    OffsetFlags {
        /// The flags byte.
        flags: u8,
        /// The EDID revision.
        revision: u8,
    },
    /// A rate or the pixel clock byte is 0.
    #[error("EDID range limits field {field} is 0")]
    ZeroLimit {
        /// The name of the field.
        field: &'static str,
    },
    /// A lowest rate is above the highest rate.
    #[error("EDID range limits {field}: minimum {min} is above maximum {max}")]
    Inverted {
        /// `vertical` or `horizontal`.
        field: &'static str,
        /// The lowest rate, in the unit of the field.
        min: u16,
        /// The highest rate, in the unit of the field.
        max: u16,
    },
    /// The timing formula byte is unknown or not valid for the revision.
    #[error(
        "EDID range limits timing support byte {value:#04x} is not valid for revision {revision}"
    )]
    TimingSupport {
        /// The byte.
        value: u8,
        /// The EDID revision.
        revision: u8,
    },
    /// The padding or the version byte after the timing formula byte is
    /// wrong.
    #[error("EDID range limits byte {byte} is {value:#04x}, expected {expected:#04x}")]
    Padding {
        /// The index of the byte inside the descriptor.
        byte: usize,
        /// The value found.
        value: u8,
        /// The value the specification requires.
        expected: u8,
    },
    /// A CVT range limits descriptor limits the active pixels per line.
    /// The profile cannot carry that limit, so the descriptor is refused.
    #[error("EDID CVT range limits set a maximum of {pixels} active pixels per line")]
    CvtMaxActivePixels {
        /// The limit, in pixels: 8 times the 10-bit value of byte 12 bits 1
        /// to 0 and byte 13.
        pixels: u32,
    },
    /// The highest pixel clock left after rounding down is 0.
    #[error(
        "EDID range limits leave no pixel clock: byte 9 is {coarse_10mhz}, reduction {reduction_250khz}"
    )]
    NoPixelClock {
        /// Byte 9, in units of 10 MHz.
        coarse_10mhz: u8,
        /// The CVT reduction, in units of 250 kHz.
        reduction_250khz: u8,
    },
}

/// Parses an EDID: the base block, and the checksum of each extension
/// block.
///
/// `bytes` must hold the base block and every extension block its byte 126
/// announces, as the kernel gives them in the `edid` file of a connector.
///
/// # Errors
///
/// Returns an [`EdidError`] for the first check that fails.
pub fn parse_edid(bytes: &[u8]) -> Result<Edid, EdidError> {
    let base = check_blocks(bytes)?;
    let revision = check_version(base)?;
    let manufacturer = manufacturer(base)?;
    let product_code = u16::from_le_bytes([base[PRODUCT_OFFSET], base[PRODUCT_OFFSET + 1]]);
    let analog = base[INPUT_OFFSET] & INPUT_DIGITAL_BIT == 0;
    let continuous_frequency = base[FEATURES_OFFSET] & CONTINUOUS_BIT != 0;
    let range_limits = range_limits(base, revision)?;
    Ok(Edid {
        manufacturer,
        product_code,
        revision,
        analog,
        continuous_frequency,
        range_limits,
    })
}

const HEADER: [u8; 8] = [0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x00];
pub(crate) const MANUFACTURER_OFFSET: usize = 0x08;
pub(crate) const PRODUCT_OFFSET: usize = 0x0a;
const VERSION_OFFSET: usize = 0x12;
const REVISION_OFFSET: usize = 0x13;
pub(crate) const INPUT_OFFSET: usize = 0x14;
pub(crate) const INPUT_DIGITAL_BIT: u8 = 0x80;
const FEATURES_OFFSET: usize = 0x18;
const CONTINUOUS_BIT: u8 = 0x01;
pub(crate) const DESCRIPTOR_OFFSETS: [usize; 4] = [0x36, 0x48, 0x5a, 0x6c];
pub(crate) const DESCRIPTOR_LEN: usize = 18;
pub(crate) const EXTENSION_COUNT_OFFSET: usize = 0x7e;

const TAG_RANGE_LIMITS: u8 = 0xfd;
const TAG_DUMMY: u8 = 0x10;
const TAG_FIRST_DEFINED: u8 = 0xf7;

const FORMULA_DEFAULT_GTF: u8 = 0x00;
const FORMULA_RANGE_ONLY: u8 = 0x01;
const FORMULA_SECONDARY_GTF: u8 = 0x02;
const FORMULA_CVT: u8 = 0x04;
const PADDING_NEWLINE: u8 = 0x0a;
const PADDING_SPACE: u8 = 0x20;
const SECONDARY_GTF_RESERVED: u8 = 0x00;
const CVT_VERSION_1_1: u8 = 0x11;

const RATE_OFFSET: u16 = 255;
const OFFSET_RESERVED_MASK: u8 = 0xf0;
const PCLOCK_UNIT_HZ: u32 = 10_000_000;
const CVT_REDUCTION_UNIT_HZ: u32 = 250_000;
const CVT_ACTIVE_PIXELS_UNIT: u32 = 8;

/// Checks the length, the header and every block checksum, and returns the
/// base block.
pub(crate) fn check_blocks(bytes: &[u8]) -> Result<&[u8], EdidError> {
    if bytes.len() < EDID_BLOCK_LEN {
        return Err(EdidError::Length {
            len: bytes.len(),
            expected: EDID_BLOCK_LEN,
        });
    }
    let base = &bytes[..EDID_BLOCK_LEN];
    if base[..HEADER.len()] != HEADER {
        let mut found = [0; 8];
        found.copy_from_slice(&base[..HEADER.len()]);
        return Err(EdidError::Header { found });
    }
    let expected = EDID_BLOCK_LEN * (1 + usize::from(base[EXTENSION_COUNT_OFFSET]));
    if bytes.len() != expected {
        return Err(EdidError::Length {
            len: bytes.len(),
            expected,
        });
    }
    for (block, chunk) in bytes.chunks_exact(EDID_BLOCK_LEN).enumerate() {
        let remainder = chunk.iter().fold(0u8, |sum, byte| sum.wrapping_add(*byte));
        if remainder != 0 {
            return Err(EdidError::Checksum { block, remainder });
        }
    }
    Ok(base)
}

pub(crate) fn check_version(base: &[u8]) -> Result<u8, EdidError> {
    let version = base[VERSION_OFFSET];
    let revision = base[REVISION_OFFSET];
    if version != 1 || !(3..=4).contains(&revision) {
        return Err(EdidError::Version { version, revision });
    }
    Ok(revision)
}

pub(crate) fn manufacturer(base: &[u8]) -> Result<String, EdidError> {
    let raw = u16::from_be_bytes([base[MANUFACTURER_OFFSET], base[MANUFACTURER_OFFSET + 1]]);
    if raw & 0x8000 != 0 {
        return Err(EdidError::Manufacturer { raw });
    }
    [10u16, 5, 0]
        .iter()
        .map(|shift| {
            let code = (raw >> shift) & 0x1f;
            if (1..=26).contains(&code) {
                // `code` is 1 to 26, so the sum is an ASCII upper-case letter.
                Ok(char::from(b'A' + (code as u8) - 1))
            } else {
                Err(EdidError::Manufacturer { raw })
            }
        })
        .collect()
}

/// Checks the layout of the four descriptors and parses the one range
/// limits descriptor, if there is one.
fn range_limits(base: &[u8], revision: u8) -> Result<Option<RangeLimits>, EdidError> {
    let mut found: Option<(usize, RangeLimits)> = None;
    for (index, &offset) in DESCRIPTOR_OFFSETS.iter().enumerate() {
        let descriptor = &base[offset..offset + DESCRIPTOR_LEN];
        let is_timing = descriptor[0] != 0 || descriptor[1] != 0;
        if index == 0 && !is_timing {
            return Err(EdidError::NoPreferredTiming);
        }
        if is_timing {
            continue;
        }
        let tag = check_display_descriptor(descriptor, offset, revision)?;
        if tag != TAG_RANGE_LIMITS {
            continue;
        }
        if let Some((first, _)) = found {
            return Err(EdidError::DuplicateRangeLimits {
                first,
                second: offset,
            });
        }
        found = Some((offset, parse_range_limits(descriptor, revision)?));
    }
    Ok(found.map(|(_, limits)| limits))
}

/// Checks the reserved bytes and the tag of a display descriptor, and
/// returns the tag.
pub(crate) fn check_display_descriptor(
    descriptor: &[u8],
    offset: usize,
    revision: u8,
) -> Result<u8, EdidError> {
    let tag = descriptor[3];
    if descriptor[2] != 0 {
        return Err(EdidError::DescriptorReserved {
            offset,
            byte: 2,
            value: descriptor[2],
        });
    }
    if tag > TAG_DUMMY && tag < TAG_FIRST_DEFINED {
        return Err(EdidError::ReservedTag { offset, tag });
    }
    // EDID 1.4 uses byte 4 of the range limits descriptor for rate offset
    // flags; parse_range_limits checks it.
    let byte_4_is_flags = tag == TAG_RANGE_LIMITS && revision == 4;
    if !byte_4_is_flags && descriptor[4] != 0 {
        return Err(EdidError::DescriptorReserved {
            offset,
            byte: 4,
            value: descriptor[4],
        });
    }
    Ok(tag)
}

fn parse_range_limits(d: &[u8], revision: u8) -> Result<RangeLimits, EdidError> {
    let flags = d[4];
    let (v_min_offset, v_max_offset) = rate_offsets(flags & 0x03, flags, revision)?;
    let (h_min_offset, h_max_offset) = rate_offsets((flags >> 2) & 0x03, flags, revision)?;
    if flags & OFFSET_RESERVED_MASK != 0 {
        return Err(EdidError::OffsetFlags { flags, revision });
    }
    let v_min_hz = nonzero(d[5], "v_min")? + v_min_offset;
    let v_max_hz = nonzero(d[6], "v_max")? + v_max_offset;
    let h_min_khz = nonzero(d[7], "h_min")? + h_min_offset;
    let h_max_khz = nonzero(d[8], "h_max")? + h_max_offset;
    let coarse_10mhz = d[9];
    nonzero(coarse_10mhz, "pclock_max")?;
    ordered("vertical", v_min_hz, v_max_hz)?;
    ordered("horizontal", h_min_khz, h_max_khz)?;
    let formula = timing_formula(d[10], revision)?;
    check_formula_bytes(d, formula)?;
    let reduction_250khz = if formula == TimingFormula::Cvt {
        d[12] >> 2
    } else {
        0
    };
    let pclock_max_hz =
        pclock_max_hz(coarse_10mhz, formula, reduction_250khz).ok_or(EdidError::NoPixelClock {
            coarse_10mhz,
            reduction_250khz,
        })?;
    Ok(RangeLimits {
        v_min_hz,
        v_max_hz,
        h_min_khz,
        h_max_khz,
        pclock_max_hz,
        formula,
    })
}

/// The offsets that two flag bits add to the lowest and the highest rate.
fn rate_offsets(bits: u8, flags: u8, revision: u8) -> Result<(u16, u16), EdidError> {
    match (revision, bits) {
        (_, 0b00) => Ok((0, 0)),
        (4, 0b10) => Ok((0, RATE_OFFSET)),
        (4, 0b11) => Ok((RATE_OFFSET, RATE_OFFSET)),
        _ => Err(EdidError::OffsetFlags { flags, revision }),
    }
}

fn nonzero(value: u8, field: &'static str) -> Result<u16, EdidError> {
    if value == 0 {
        return Err(EdidError::ZeroLimit { field });
    }
    Ok(u16::from(value))
}

fn ordered(field: &'static str, min: u16, max: u16) -> Result<(), EdidError> {
    if min > max {
        return Err(EdidError::Inverted { field, min, max });
    }
    Ok(())
}

fn timing_formula(value: u8, revision: u8) -> Result<TimingFormula, EdidError> {
    match (value, revision) {
        (FORMULA_DEFAULT_GTF, _) => Ok(TimingFormula::DefaultGtf),
        (FORMULA_SECONDARY_GTF, _) => Ok(TimingFormula::SecondaryGtf),
        (FORMULA_RANGE_ONLY, 4) => Ok(TimingFormula::RangeLimitsOnly),
        (FORMULA_CVT, 4) => Ok(TimingFormula::Cvt),
        _ => Err(EdidError::TimingSupport { value, revision }),
    }
}

/// Checks bytes 11 to 17 where the specification fixes them, and refuses
/// a CVT limit on the active pixels per line.
fn check_formula_bytes(d: &[u8], formula: TimingFormula) -> Result<(), EdidError> {
    let expect = |byte: usize, expected: u8| {
        if d[byte] == expected {
            Ok(())
        } else {
            Err(EdidError::Padding {
                byte,
                value: d[byte],
                expected,
            })
        }
    };
    match formula {
        TimingFormula::DefaultGtf | TimingFormula::RangeLimitsOnly => {
            expect(11, PADDING_NEWLINE)?;
            (12..DESCRIPTOR_LEN).try_for_each(|byte| expect(byte, PADDING_SPACE))
        }
        TimingFormula::SecondaryGtf => expect(11, SECONDARY_GTF_RESERVED),
        TimingFormula::Cvt => {
            expect(11, CVT_VERSION_1_1)?;
            let units = (u32::from(d[12] & 0x03) << 8) | u32::from(d[13]);
            if units != 0 {
                return Err(EdidError::CvtMaxActivePixels {
                    pixels: units * CVT_ACTIVE_PIXELS_UNIT,
                });
            }
            Ok(())
        }
    }
}

/// The highest pixel clock that the display surely accepts, or `None` when
/// nothing is left.
fn pclock_max_hz(coarse_10mhz: u8, formula: TimingFormula, reduction_250khz: u8) -> Option<u32> {
    let coarse_hz = u32::from(coarse_10mhz) * PCLOCK_UNIT_HZ;
    let max_hz = if formula == TimingFormula::Cvt {
        coarse_hz.checked_sub(u32::from(reduction_250khz) * CVT_REDUCTION_UNIT_HZ)?
    } else {
        coarse_hz.checked_sub(PCLOCK_UNIT_HZ)?
    };
    (max_hz > 0).then_some(max_hz)
}
