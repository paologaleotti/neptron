//! EDID test blobs, built byte by byte from the VESA E-EDID specification
//! (release A, revision 1 for EDID 1.3; release A, revision 2 for EDID 1.4).
//!
//! No blob is the EDID of a real monitor. `pc_crt_30_71` describes a
//! 17-inch PC CRT of the SyncMaster 740s class: 30 to 71 kHz, 50 to
//! 160 Hz. Its manufacturer id `NPT` and its product code are made up.

/// Byte offsets in the base block.
pub mod at {
    pub const MANUFACTURER: usize = 0x08;
    pub const VERSION: usize = 0x12;
    pub const REVISION: usize = 0x13;
    pub const INPUT: usize = 0x14;
    pub const FEATURES: usize = 0x18;
    pub const DESCRIPTOR_1: usize = 0x36;
    pub const DESCRIPTOR_2: usize = 0x48;
    pub const DESCRIPTOR_3: usize = 0x5a;
    pub const DESCRIPTOR_4: usize = 0x6c;
    pub const EXTENSIONS: usize = 0x7e;
    pub const CHECKSUM: usize = 0x7f;
}

/// The EDID 1.3 base block of a 17-inch PC CRT: 30 to 71 kHz, 50 to
/// 160 Hz, 110 MHz, default GTF.
pub fn pc_crt_30_71() -> Vec<u8> {
    let mut b = vec![0u8; 128];
    // 0x00..0x08: the fixed header.
    b[0x00..0x08].copy_from_slice(&[0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x00]);
    // 0x08..0x0a: manufacturer "NPT", big-endian, 5 bits per letter with
    // A = 1: N = 14, P = 16, T = 20, so 0 01110 10000 10100 = 0x3a14.
    b[0x08..0x0a].copy_from_slice(&[0x3a, 0x14]);
    // 0x0a..0x0c: product code 0x0071, little-endian.
    b[0x0a..0x0c].copy_from_slice(&[0x71, 0x00]);
    // 0x0c..0x10: serial number 0 (not used).
    // 0x10: week 1; 0x11: year 1990 + 13 = 2003.
    b[0x10] = 0x01;
    b[0x11] = 0x0d;
    // 0x12, 0x13: EDID version 1, revision 3.
    b[0x12] = 0x01;
    b[0x13] = 0x03;
    // 0x14: analog input (bit 7 clear), 0.7 V white level, separate sync,
    // composite sync and sync on green supported.
    b[0x14] = 0x0e;
    // 0x15, 0x16: image size 32 cm by 24 cm.
    b[0x15] = 0x20;
    b[0x16] = 0x18;
    // 0x17: gamma 2.2, stored as (2.2 * 100) - 100 = 120.
    b[0x17] = 0x78;
    // 0x18: DPMS standby, suspend and off (bits 7 to 5), RGB colour
    // (bits 4 to 3 = 01), preferred timing in descriptor 1 (bit 1), default
    // GTF supported (bit 0).
    b[0x18] = 0xeb;
    // 0x19..0x23: chromaticity, arbitrary values. Not parsed.
    b[0x19..0x23].copy_from_slice(&[0xee, 0x91, 0xa3, 0x54, 0x4c, 0x99, 0x26, 0x0f, 0x50, 0x54]);
    // 0x23..0x26: established timings 640x480 at 60 Hz, 800x600 at 60 Hz,
    // 1024x768 at 60 Hz. Not parsed.
    b[0x23..0x26].copy_from_slice(&[0x21, 0x08, 0x00]);
    // 0x26..0x36: eight unused standard timings, 0x01 0x01 each.
    b[0x26..0x36].fill(0x01);
    // 0x36..0x48: descriptor 1, a detailed timing: VESA DMT 1024x768 at
    // 75 Hz. 78.75 MHz = 7875 units of 10 kHz, little-endian; 1024 active
    // and 288 blank pixels; 768 active and 32 blank lines; front porch 16,
    // sync 96 pixels; front porch 1, sync 3 lines; image 320 by 240 mm;
    // digital separate sync with both polarities positive.
    b[0x36..0x48].copy_from_slice(&[
        0xc3, 0x1e, 0x00, 0x20, 0x41, 0x00, 0x20, 0x30, 0x10, 0x60, 0x13, 0x00, 0x40, 0xf0, 0x10,
        0x00, 0x00, 0x1e,
    ]);
    // 0x48..0x5a: descriptor 2, the display range limits (tag 0xfd).
    // Byte 4: no rate offsets. 50 Hz, 160 Hz, 30 kHz, 71 kHz, 11 units of
    // 10 MHz, timing formula 0x00 (default GTF), then 0x0a and six spaces.
    b[0x48..0x5a].copy_from_slice(&[
        0x00, 0x00, 0x00, 0xfd, 0x00, 50, 160, 30, 71, 11, 0x00, 0x0a, 0x20, 0x20, 0x20, 0x20,
        0x20, 0x20,
    ]);
    // 0x5a..0x6c: descriptor 3, the product name (tag 0xfc): "TEST CRT 17",
    // a line feed and one space.
    let mut name = vec![0x00, 0x00, 0x00, 0xfc, 0x00];
    name.extend_from_slice(b"TEST CRT 17\n ");
    b[0x5a..0x6c].copy_from_slice(&name);
    // 0x6c..0x7e: descriptor 4, a dummy descriptor (tag 0x10), all zero.
    b[0x6c + 3] = 0x10;
    // 0x7e: no extension block. 0x7f: the checksum.
    b[0x7e] = 0x00;
    fix_checksum(&mut b, 0);
    b
}

/// An EDID 1.4 block with a range limits descriptor that uses the rate
/// offsets: 30 to 300 kHz (the maximum carries the +255 offset), 50 to
/// 160 Hz, timing formula `range limits only`, 200 MHz.
pub fn edid_14_high_rate() -> Vec<u8> {
    let mut b = pc_crt_30_71();
    b[at::REVISION] = 0x04;
    // Bit 0 of 0x18 is the continuous frequency bit in EDID 1.4 and the
    // default GTF bit in EDID 1.3; 0xeb sets it in both.
    let d = at::DESCRIPTOR_2;
    // Byte 4: bits 3 to 2 = 10, horizontal maximum + 255 kHz.
    b[d + 4] = 0x08;
    b[d + 8] = 45;
    b[d + 9] = 20;
    b[d + 10] = 0x01;
    fix_checksum(&mut b, 0);
    b
}

/// An EDID 1.4 block with a CVT range limits descriptor: 110 MHz coarse
/// limit with a reduction of 9 units of 250 kHz, so 107.75 MHz.
pub fn edid_14_cvt() -> Vec<u8> {
    let mut b = pc_crt_30_71();
    b[at::REVISION] = 0x04;
    let d = at::DESCRIPTOR_2;
    // Bytes 10 to 17: CVT, version 1.1, reduction 9 in bits 7 to 2 of byte
    // 12, no limit on active pixels (byte 12 bits 1 to 0 and byte 13 at 0),
    // then CVT fields that the parser does not read.
    b[d + 10..d + 18].copy_from_slice(&[0x04, 0x11, 9 << 2, 0x00, 0x08, 0x10, 0x00, 0x3c]);
    fix_checksum(&mut b, 0);
    b
}

/// Writes the byte that makes block `block` add up to 0 modulo 256.
pub fn fix_checksum(bytes: &mut [u8], block: usize) {
    let start = block * 128;
    let sum = bytes[start..start + 127]
        .iter()
        .fold(0u8, |sum, byte| sum.wrapping_add(*byte));
    bytes[start + 127] = 0u8.wrapping_sub(sum);
}

/// `bytes` with `value` at `offset` and the checksum fixed.
pub fn with(mut bytes: Vec<u8>, offset: usize, value: u8) -> Vec<u8> {
    bytes[offset] = value;
    fix_checksum(&mut bytes, offset / 128);
    bytes
}
