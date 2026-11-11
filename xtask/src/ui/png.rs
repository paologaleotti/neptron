//! A small PNG writer: 8-bit RGB, no filter, zlib with stored (not
//! compressed) blocks. The files are for people to look at, so size does not
//! matter and no compression crate is needed.

const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1A, b'\n'];
const MAX_STORED_BLOCK: usize = 0xFFFF;

/// Encodes `rgb` (rows first, three bytes per pixel) as a PNG file. The
/// caller passes `width * height * 3` bytes.
pub fn encode(width: u32, height: u32, rgb: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(rgb.len() + rgb.len() / 100 + 128);
    out.extend_from_slice(&SIGNATURE);
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    // Bit depth 8, colour type 2 (RGB), deflate, adaptive filters, no
    // interlace.
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &zlib_stored(&scanlines(width, rgb)));
    chunk(&mut out, b"IEND", &[]);
    out
}

/// Every row with the filter byte 0 in front.
fn scanlines(width: u32, rgb: &[u8]) -> Vec<u8> {
    let row = width as usize * 3;
    let mut raw = Vec::with_capacity(rgb.len() + rgb.len() / row.max(1) + 1);
    for line in rgb.chunks(row.max(1)) {
        raw.push(0);
        raw.extend_from_slice(line);
    }
    raw
}

fn zlib_stored(data: &[u8]) -> Vec<u8> {
    let blocks = data.len() / MAX_STORED_BLOCK + 1;
    let mut out = Vec::with_capacity(data.len() + blocks * 5 + 6);
    // CMF: deflate with a 32 KiB window. FLG: no dictionary, and the check
    // bits make CMF * 256 + FLG a multiple of 31.
    out.extend_from_slice(&[0x78, 0x01]);
    let mut pieces = data.chunks(MAX_STORED_BLOCK).peekable();
    if pieces.peek().is_none() {
        out.extend_from_slice(&[1, 0, 0, 0xFF, 0xFF]);
    }
    while let Some(piece) = pieces.next() {
        let last = pieces.peek().is_none();
        out.push(u8::from(last));
        // `chunks(MAX_STORED_BLOCK)` keeps every piece at 65535 bytes or less.
        let len = piece.len() as u16;
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&(!len).to_le_bytes());
        out.extend_from_slice(piece);
    }
    out.extend_from_slice(&adler32(data).to_be_bytes());
    out
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    // A PNG chunk is far below 4 GiB here: the largest is the IDAT of the
    // gallery, a few megabytes.
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let start = out.len();
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let crc = crc32(&out[start..]);
    out.extend_from_slice(&crc.to_be_bytes());
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFF_u32;
    for &b in bytes {
        crc ^= u32::from(b);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

fn adler32(bytes: &[u8]) -> u32 {
    const MOD: u32 = 65521;
    let (mut a, mut b) = (1_u32, 0_u32);
    for &x in bytes {
        a = (a + u32::from(x)) % MOD;
        b = (b + a) % MOD;
    }
    (b << 16) | a
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checksums_match_known_values() {
        assert_eq!(crc32(b"IEND"), 0xAE42_6082);
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(adler32(b"Wikipedia"), 0x11E6_0398);
    }

    #[test]
    fn a_one_pixel_image() {
        let png = encode(1, 1, &[0xFF, 0x00, 0x80]);
        assert_eq!(&png[..8], &SIGNATURE);
        assert_eq!(&png[12..16], b"IHDR");
        assert_eq!(&png[16..24], &[0, 0, 0, 1, 0, 0, 0, 1]);
        assert_eq!(
            &png[png.len() - 12..],
            &[0, 0, 0, 0, b'I', b'E', b'N', b'D', 0xAE, 0x42, 0x60, 0x82]
        );
    }

    #[test]
    fn large_data_is_split_into_stored_blocks() {
        let data = vec![7_u8; MAX_STORED_BLOCK + 10];
        let z = zlib_stored(&data);
        assert_eq!(z.len(), 2 + 5 + MAX_STORED_BLOCK + 5 + 10 + 4);
        assert_eq!(z[2], 0);
        assert_eq!(z[2 + 5 + MAX_STORED_BLOCK], 1);
        assert_eq!(&z[3..5], &[0xFF, 0xFF]);
        assert_eq!(&z[5..7], &[0, 0]);
    }

    #[test]
    fn empty_data_is_one_empty_final_block() {
        assert_eq!(
            zlib_stored(&[]),
            [0x78, 0x01, 1, 0, 0, 0xFF, 0xFF, 0, 0, 0, 1]
        );
    }
}
