//! The modes a display lists in its EDID, and its product name.
//!
//! The same checks as [`parse_edid`](crate::parse_edid) guard the data:
//! length, header, every block checksum, version, manufacturer id and the
//! layout of the four descriptors. The content of the range limits
//! descriptor is not read: a flat panel is driven only in its listed
//! modes, so the range limits do not bound anything here.

use neptron_types::Modeline;

use crate::edid::{
    DESCRIPTOR_LEN, DESCRIPTOR_OFFSETS, EDID_BLOCK_LEN, EdidError, INPUT_DIGITAL_BIT, INPUT_OFFSET,
    PRODUCT_OFFSET, check_blocks, check_display_descriptor, check_version, manufacturer,
};

/// The identity of a display and the progressive modes its EDID lists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListedModes {
    /// The three-letter manufacturer id, in upper case.
    pub manufacturer: String,
    /// The product code the manufacturer gives the model.
    pub product_code: u16,
    /// The display has an analog video input (byte 0x14, bit 7 clear).
    pub analog: bool,
    /// The product name of descriptor tag `0xFC`, trimmed, when there is one.
    pub name: Option<String>,
    /// The progressive modes, each once, in this order: the detailed
    /// timings of the base block (the first is the preferred mode), the
    /// detailed timings of the CTA-861 extensions, the CTA-861 video codes
    /// of [`CTA_MODES`], the standard timings of [`DMT_MODES`], and the
    /// established timings 640x480, 800x600 and 1024x768 at 60 Hz.
    pub modes: Vec<Modeline>,
}

/// Reads the identity, the product name and the listed modes of an EDID.
///
/// Interlaced timings, standard timings at a rate or size that
/// [`DMT_MODES`] does not hold, CTA-861 video codes that [`CTA_MODES`] does
/// not hold, the other established timings, and the DisplayID and
/// HDMI Forum blocks are left out. A detailed timing whose values are out
/// of order is left out too. A CTA-861 block whose offsets point outside
/// the block gives no modes; the rest of the EDID is still read.
///
/// # Errors
///
/// Returns an [`EdidError`] for the first structural check that fails:
/// length, header, checksum, version, manufacturer id, a first descriptor
/// that is not a detailed timing, a reserved byte or tag in a display
/// descriptor.
pub fn listed_modes(bytes: &[u8]) -> Result<ListedModes, EdidError> {
    let base = check_blocks(bytes)?;
    let revision = check_version(base)?;
    let manufacturer = manufacturer(base)?;
    let product_code = u16::from_le_bytes([base[PRODUCT_OFFSET], base[PRODUCT_OFFSET + 1]]);
    let analog = base[INPUT_OFFSET] & INPUT_DIGITAL_BIT == 0;
    let mut name = None;
    let mut modes = Vec::new();
    for (index, &offset) in DESCRIPTOR_OFFSETS.iter().enumerate() {
        let d = &base[offset..offset + DESCRIPTOR_LEN];
        if d[0] != 0 || d[1] != 0 {
            push(&mut modes, detailed_timing(d));
            continue;
        }
        if index == 0 {
            return Err(EdidError::NoPreferredTiming);
        }
        if check_display_descriptor(d, offset, revision)? == TAG_PRODUCT_NAME {
            name = product_name(d);
        }
    }
    let extensions: Vec<&[u8]> = bytes
        .chunks_exact(EDID_BLOCK_LEN)
        .skip(1)
        .filter(|block| block[0] == CTA_EXTENSION_TAG)
        .collect();
    for block in &extensions {
        for d in cta_detailed(block) {
            push(&mut modes, detailed_timing(d));
        }
    }
    for block in &extensions {
        for vic in cta_video_codes(block) {
            push(&mut modes, cta_mode(vic));
        }
    }
    for pair in base[STANDARD_TIMINGS].chunks_exact(2) {
        push(&mut modes, standard_timing(pair[0], pair[1]));
    }
    for &(byte, bit, (w, h)) in &ESTABLISHED_60_HZ {
        if base[byte] & bit != 0 {
            push(&mut modes, dmt_mode(w, h));
        }
    }
    Ok(ListedModes {
        manufacturer,
        product_code,
        analog,
        name,
        modes,
    })
}

/// One mode of a fixed timing table: size, pixel clock in kilohertz, the
/// horizontal front porch, sync and total, the vertical front porch, sync
/// and total, and the sync polarities (horizontal, vertical; true is
/// positive).
type TableMode = (u16, u16, u32, [u16; 3], [u16; 3], (bool, bool));

/// The CTA-861 video codes that this parser turns into modes, with the
/// timings of CTA-861-G table 1. Codes 1 (640x480, 25.175 MHz) and 2 and
/// 3 (720x480, 27 MHz) take their 59.94 Hz clock. The HD codes take the
/// integer rate, not the 1000/1001 variant: 4 (1280x720p60), 16
/// (1920x1080p60), 17 and 18 (720x576p50), 19 (1280x720p50), 31
/// (1920x1080p50), 95 (3840x2160p30), 96 (3840x2160p50) and 97
/// (3840x2160p60).
pub const CTA_MODES: [(u8, TableMode); 12] = [
    (
        1,
        (
            640,
            480,
            25_175,
            [16, 96, 800],
            [10, 2, 525],
            (false, false),
        ),
    ),
    (
        2,
        (720, 480, 27_000, [16, 62, 858], [9, 6, 525], (false, false)),
    ),
    (
        3,
        (720, 480, 27_000, [16, 62, 858], [9, 6, 525], (false, false)),
    ),
    (
        4,
        (
            1280,
            720,
            74_250,
            [110, 40, 1650],
            [5, 5, 750],
            (true, true),
        ),
    ),
    (
        16,
        (
            1920,
            1080,
            148_500,
            [88, 44, 2200],
            [4, 5, 1125],
            (true, true),
        ),
    ),
    (
        17,
        (720, 576, 27_000, [12, 64, 864], [5, 5, 625], (false, false)),
    ),
    (
        18,
        (720, 576, 27_000, [12, 64, 864], [5, 5, 625], (false, false)),
    ),
    (
        19,
        (
            1280,
            720,
            74_250,
            [440, 40, 1980],
            [5, 5, 750],
            (true, true),
        ),
    ),
    (
        31,
        (
            1920,
            1080,
            148_500,
            [528, 44, 2640],
            [4, 5, 1125],
            (true, true),
        ),
    ),
    (
        95,
        (
            3840,
            2160,
            297_000,
            [176, 88, 4400],
            [8, 10, 2250],
            (true, true),
        ),
    ),
    (
        96,
        (
            3840,
            2160,
            594_000,
            [1056, 88, 5280],
            [8, 10, 2250],
            (true, true),
        ),
    ),
    (
        97,
        (
            3840,
            2160,
            594_000,
            [176, 88, 4400],
            [8, 10, 2250],
            (true, true),
        ),
    ),
];

/// The VESA DMT 1.0 r13 modes at 60 Hz that a standard or established
/// timing can name. Reduced blanking where DMT gives only that timing
/// (1600x900, 1920x1200).
pub const DMT_MODES: [TableMode; 13] = [
    (
        640,
        480,
        25_175,
        [16, 96, 800],
        [10, 2, 525],
        (false, false),
    ),
    (800, 600, 40_000, [40, 128, 1056], [1, 4, 628], (true, true)),
    (
        1024,
        768,
        65_000,
        [24, 136, 1344],
        [3, 6, 806],
        (false, false),
    ),
    (
        1280,
        720,
        74_250,
        [110, 40, 1650],
        [5, 5, 750],
        (true, true),
    ),
    (
        1280,
        800,
        83_500,
        [72, 128, 1680],
        [3, 6, 831],
        (false, true),
    ),
    (
        1280,
        960,
        108_000,
        [96, 112, 1800],
        [1, 3, 1000],
        (true, true),
    ),
    (
        1280,
        1024,
        108_000,
        [48, 112, 1688],
        [1, 3, 1066],
        (true, true),
    ),
    (
        1366,
        768,
        85_500,
        [70, 143, 1792],
        [3, 3, 798],
        (true, true),
    ),
    (
        1440,
        900,
        106_500,
        [80, 152, 1904],
        [3, 6, 934],
        (false, true),
    ),
    (
        1600,
        900,
        108_000,
        [24, 80, 1800],
        [1, 3, 1000],
        (true, true),
    ),
    (
        1680,
        1050,
        146_250,
        [104, 176, 2240],
        [3, 6, 1089],
        (false, true),
    ),
    (
        1920,
        1080,
        148_500,
        [88, 44, 2200],
        [4, 5, 1125],
        (true, true),
    ),
    (
        1920,
        1200,
        154_000,
        [48, 32, 2080],
        [3, 6, 1235],
        (true, false),
    ),
];

const TAG_PRODUCT_NAME: u8 = 0xfc;
const CTA_EXTENSION_TAG: u8 = 0x02;
const CTA_DATA_START: usize = 4;
const CTA_VIDEO_BLOCK_TAG: u8 = 2;
const STANDARD_TIMINGS: std::ops::Range<usize> = 0x26..0x36;
const STANDARD_UNUSED: u8 = 0x01;
const STANDARD_RATE_BASE_HZ: u16 = 60;
const STANDARD_WIDTH_BASE: u16 = 31;
const DTD_INTERLACED_BIT: u8 = 0x80;
const DTD_HSYNC_POSITIVE_BIT: u8 = 0x02;
const DTD_VSYNC_POSITIVE_BIT: u8 = 0x04;
const PCLOCK_UNIT_HZ: u32 = 10_000;
const HZ_PER_KHZ: u32 = 1_000;
/// Video codes 129 to 192 carry the native flag in bit 7 (CTA-861-F).
const SVD_NATIVE_RANGE: std::ops::RangeInclusive<u8> = 129..=192;

/// Byte, bit and size of the established timings at 60 Hz that the
/// parser reads.
const ESTABLISHED_60_HZ: [(usize, u8, (u16, u16)); 3] = [
    (0x23, 0x20, (640, 480)),
    (0x23, 0x01, (800, 600)),
    (0x24, 0x08, (1024, 768)),
];

/// Adds `mode` when there is one and the list does not hold it yet.
fn push(modes: &mut Vec<Modeline>, mode: Option<Modeline>) {
    if let Some(mode) = mode
        && !modes.contains(&mode)
    {
        modes.push(mode);
    }
}

/// The mode of an 18-byte detailed timing descriptor, or `None` for an
/// interlaced one or one whose values are out of order.
fn detailed_timing(d: &[u8]) -> Option<Modeline> {
    let hi = |byte: u8, shift: u8, mask: u8| u16::from((byte >> shift) & mask);
    let pclock_hz = u32::from(u16::from_le_bytes([d[0], d[1]])) * PCLOCK_UNIT_HZ;
    let hactive = u16::from(d[2]) | hi(d[4], 4, 0x0f) << 8;
    let hblank = u16::from(d[3]) | hi(d[4], 0, 0x0f) << 8;
    let vactive = u16::from(d[5]) | hi(d[7], 4, 0x0f) << 8;
    let vblank = u16::from(d[6]) | hi(d[7], 0, 0x0f) << 8;
    let hfront = u16::from(d[8]) | hi(d[11], 6, 0x03) << 8;
    let hsync = u16::from(d[9]) | hi(d[11], 4, 0x03) << 8;
    let vfront = hi(d[10], 4, 0x0f) | hi(d[11], 2, 0x03) << 4;
    let vsync = hi(d[10], 0, 0x0f) | hi(d[11], 0, 0x03) << 4;
    let flags = d[17];
    if flags & DTD_INTERLACED_BIT != 0 || hactive == 0 || vactive == 0 || pclock_hz == 0 {
        return None;
    }
    let mode = Modeline {
        pclock_hz,
        hactive,
        hsync_start: hactive + hfront,
        hsync_end: hactive + hfront + hsync,
        htotal: hactive + hblank,
        vactive,
        vsync_start: vactive + vfront,
        vsync_end: vactive + vfront + vsync,
        vtotal: vactive + vblank,
        interlace: false,
        doublescan: false,
        // The kernel reads these two bits for every sync type, so the
        // panel gets the polarities that it gets from any other system.
        hsync_positive: flags & DTD_HSYNC_POSITIVE_BIT != 0,
        vsync_positive: flags & DTD_VSYNC_POSITIVE_BIT != 0,
    };
    (mode.hsync_end <= mode.htotal && mode.vsync_end <= mode.vtotal).then_some(mode)
}

/// The mode of a row of a timing table.
fn table_mode(row: &TableMode) -> Modeline {
    let &(hactive, vactive, pclock_khz, [hf, hs, ht], [vf, vs, vt], (hp, vp)) = row;
    Modeline {
        pclock_hz: pclock_khz * HZ_PER_KHZ,
        hactive,
        hsync_start: hactive + hf,
        hsync_end: hactive + hf + hs,
        htotal: ht,
        vactive,
        vsync_start: vactive + vf,
        vsync_end: vactive + vf + vs,
        vtotal: vt,
        interlace: false,
        doublescan: false,
        hsync_positive: hp,
        vsync_positive: vp,
    }
}

fn cta_mode(vic: u8) -> Option<Modeline> {
    CTA_MODES
        .iter()
        .find(|(code, _)| *code == vic)
        .map(|(_, row)| table_mode(row))
}

fn dmt_mode(width: u16, height: u16) -> Option<Modeline> {
    DMT_MODES
        .iter()
        .find(|row| (row.0, row.1) == (width, height))
        .map(table_mode)
}

/// The mode of a standard timing, when it is a 60 Hz mode of
/// [`DMT_MODES`]. The aspect bits are 16:10, 4:3, 5:4 and 16:9 in EDID 1.3
/// and 1.4.
fn standard_timing(first: u8, second: u8) -> Option<Modeline> {
    if first == STANDARD_UNUSED && second == STANDARD_UNUSED || first == 0 {
        return None;
    }
    let rate_hz = u16::from(second & 0x3f) + STANDARD_RATE_BASE_HZ;
    if rate_hz != STANDARD_RATE_BASE_HZ {
        return None;
    }
    let width = (u16::from(first) + STANDARD_WIDTH_BASE) * 8;
    let (num, den) = match second >> 6 {
        0 => (10, 16),
        1 => (3, 4),
        2 => (4, 5),
        _ => (9, 16),
    };
    dmt_mode(width, width * num / den)
}

/// The detailed timing descriptors of a CTA-861 extension block.
fn cta_detailed(block: &[u8]) -> impl Iterator<Item = &[u8]> {
    let start = usize::from(block[2]);
    let valid = (CTA_DATA_START..EDID_BLOCK_LEN).contains(&start);
    block[if valid { start } else { EDID_BLOCK_LEN - 1 }..EDID_BLOCK_LEN - 1]
        .chunks_exact(DESCRIPTOR_LEN)
        .take_while(|d| d[0] != 0 || d[1] != 0)
}

/// The video codes of the video data blocks of a CTA-861 extension block.
fn cta_video_codes(block: &[u8]) -> Vec<u8> {
    let end = usize::from(block[2]);
    let mut codes = Vec::new();
    if !(CTA_DATA_START..EDID_BLOCK_LEN).contains(&end) {
        return codes;
    }
    let mut at = CTA_DATA_START;
    while at < end {
        let header = block[at];
        let len = usize::from(header & 0x1f);
        let Some(payload) = block
            .get(at + 1..at + 1 + len)
            .filter(|_| at + 1 + len <= end)
        else {
            break;
        };
        if header >> 5 == CTA_VIDEO_BLOCK_TAG {
            codes.extend(payload.iter().map(|&svd| {
                if SVD_NATIVE_RANGE.contains(&svd) {
                    svd & 0x7f
                } else {
                    svd
                }
            }));
        }
        at += 1 + len;
    }
    codes
}

/// The text of a product name descriptor: up to 13 bytes, ended by a line
/// feed, without padding; `None` when nothing printable is left.
fn product_name(d: &[u8]) -> Option<String> {
    let text: String = d[5..DESCRIPTOR_LEN]
        .iter()
        .take_while(|&&b| b != b'\n')
        .map(|&b| {
            if b.is_ascii_graphic() || b == b' ' {
                char::from(b)
            } else {
                '?'
            }
        })
        .collect();
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_table_mode_is_in_order_and_at_its_rate() {
        // DMT rates are near, not at, the nominal rate: 800x600 is 60.317 Hz.
        // 400 mHz holds every row and still tells 50 from 60 Hz.
        let rows = CTA_MODES.iter().map(|(_, row)| row).chain(DMT_MODES.iter());
        for row in rows {
            let m = table_mode(row);
            assert!(m.hsync_end <= m.htotal && m.vsync_end <= m.vtotal, "{m:?}");
            assert!(m.pclock_is_whole_khz(), "{m:?}");
            let rate = m.field_rate_mhz();
            assert!(
                [30_000, 50_000, 60_000]
                    .iter()
                    .any(|r| rate.abs_diff(*r) < 400),
                "{}x{} at {rate} mHz",
                m.hactive,
                m.vactive
            );
        }
    }

    #[test]
    fn standard_timings_name_dmt_modes() {
        // 1920 = (209 + 31) * 8; 0xc0 is 16:9 at 60 Hz.
        assert_eq!(standard_timing(209, 0xc0), dmt_mode(1920, 1080));
        // 1280x1024 is 5:4 (0x80); 1440x900 is 16:10 (0x00).
        assert_eq!(standard_timing(129, 0x80), dmt_mode(1280, 1024));
        assert_eq!(standard_timing(149, 0x00), dmt_mode(1440, 900));
        // 75 Hz, the unused pair and a size DMT_MODES does not hold.
        assert_eq!(standard_timing(209, 0xcf), None);
        assert_eq!(standard_timing(0x01, 0x01), None);
        assert_eq!(standard_timing(0, 0x40), None);
        assert_eq!(standard_timing(1, 0x40), None);
    }

    #[test]
    fn a_video_data_block_past_its_end_gives_the_codes_before_it() {
        let mut block = [0u8; 128];
        block[0] = CTA_EXTENSION_TAG;
        block[2] = 9;
        // A video block of 2 codes (16, and 31 with no flag), then a block
        // that claims 10 bytes past the start of the timings.
        block[4..9].copy_from_slice(&[0x42, 16, 31, 0x2a, 0]);
        assert_eq!(cta_video_codes(&block), vec![16, 31]);
        block[5] = 0x90;
        assert_eq!(cta_video_codes(&block)[0], 16, "native flag removed");
        block[2] = 200;
        assert!(cta_video_codes(&block).is_empty());
        assert_eq!(cta_detailed(&block).count(), 0);
    }

    #[test]
    fn product_names_lose_their_padding() {
        let mut d = [0u8; 18];
        d[3] = TAG_PRODUCT_NAME;
        d[5..18].copy_from_slice(b"DELL U2415\n  ");
        assert_eq!(product_name(&d).as_deref(), Some("DELL U2415"));
        d[5..18].copy_from_slice(b"\n            ");
        assert_eq!(product_name(&d), None);
        d[5..18].copy_from_slice(b"AB\x01CDEFGHIJKL");
        assert_eq!(product_name(&d).as_deref(), Some("AB?CDEFGHIJKL"));
    }
}
