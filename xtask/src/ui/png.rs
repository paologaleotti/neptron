//! A small PNG writer: 8-bit RGB, the Up filter on every row, and zlib
//! with one deflate block of fixed Huffman codes over a greedy LZ77 match.
//! The pictures are flat areas and repeated rows, which this compresses
//! well enough with no compression crate.

const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1A, b'\n'];
/// The PNG filter type Up: each byte less the byte above it.
const FILTER_UP: u8 = 2;
/// The deflate window, in bytes.
const WINDOW: usize = 32_768;
/// The shortest and the longest deflate match, in bytes.
const MIN_MATCH: usize = 3;
const MAX_MATCH: usize = 258;
/// How many earlier positions with the same 3 bytes a match search tries.
const MAX_CHAIN: usize = 48;
const HASH_BITS: u32 = 15;

/// Encodes `rgb` (rows first, three bytes per pixel) as a PNG file. The
/// caller passes `width * height * 3` bytes.
pub fn encode(width: u32, height: u32, rgb: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(rgb.len() / 8 + 128);
    out.extend_from_slice(&SIGNATURE);
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    // Bit depth 8, colour type 2 (RGB), deflate, adaptive filters, no
    // interlace.
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &zlib(&scanlines(width, rgb)));
    chunk(&mut out, b"IEND", &[]);
    out
}

/// Every row with the Up filter: the filter byte, then each byte less the
/// byte above it (0 above the first row).
fn scanlines(width: u32, rgb: &[u8]) -> Vec<u8> {
    let row = (width as usize * 3).max(1);
    let mut raw = Vec::with_capacity(rgb.len() + rgb.len() / row + 1);
    let mut above: &[u8] = &[];
    for line in rgb.chunks(row) {
        raw.push(FILTER_UP);
        for (i, &b) in line.iter().enumerate() {
            raw.push(b.wrapping_sub(above.get(i).copied().unwrap_or(0)));
        }
        above = line;
    }
    raw
}

fn zlib(data: &[u8]) -> Vec<u8> {
    let mut bits = BitWriter::default();
    // CMF: deflate with a 32 KiB window. FLG: no dictionary, and the check
    // bits make CMF * 256 + FLG a multiple of 31.
    bits.bytes.extend_from_slice(&[0x78, 0x01]);
    bits.put(1, 1);
    bits.put(1, 2);
    deflate_fixed(data, &mut bits);
    put_symbol(&mut bits, 256);
    let mut out = bits.finish();
    out.extend_from_slice(&adler32(data).to_be_bytes());
    out
}

/// The body of one fixed-Huffman block: literals and greedy matches.
fn deflate_fixed(data: &[u8], bits: &mut BitWriter) {
    let mut head = vec![usize::MAX; 1 << HASH_BITS];
    let mut prev = vec![usize::MAX; WINDOW];
    let hash = |i: usize| {
        let v = u32::from(data[i]) << 16 | u32::from(data[i + 1]) << 8 | u32::from(data[i + 2]);
        (v.wrapping_mul(0x9E37_79B1) >> (32 - HASH_BITS)) as usize
    };
    let insert = |i: usize, head: &mut Vec<usize>, prev: &mut Vec<usize>| {
        if i + MIN_MATCH <= data.len() {
            let h = hash(i);
            prev[i % WINDOW] = head[h];
            head[h] = i;
        }
    };
    let mut i = 0;
    while i < data.len() {
        let (mut best_len, mut best_dist) = (0, 0);
        if i + MIN_MATCH <= data.len() {
            let mut candidate = head[hash(i)];
            let limit = (data.len() - i).min(MAX_MATCH);
            for _ in 0..MAX_CHAIN {
                if candidate == usize::MAX || candidate >= i || i - candidate >= WINDOW {
                    break;
                }
                let len = data[candidate..]
                    .iter()
                    .zip(&data[i..i + limit])
                    .take_while(|(a, b)| a == b)
                    .count();
                if len > best_len {
                    (best_len, best_dist) = (len, i - candidate);
                    if len == limit {
                        break;
                    }
                }
                candidate = prev[candidate % WINDOW];
            }
        }
        if best_len >= MIN_MATCH {
            put_length(bits, best_len);
            put_distance(bits, best_dist);
            for j in i..i + best_len {
                insert(j, &mut head, &mut prev);
            }
            i += best_len;
        } else {
            put_symbol(bits, u16::from(data[i]));
            insert(i, &mut head, &mut prev);
            i += 1;
        }
    }
}

/// The fixed Huffman code of a literal or length symbol (0 to 287).
fn put_symbol(bits: &mut BitWriter, symbol: u16) {
    let (code, len) = match symbol {
        0..=143 => (0x30 + symbol, 8),
        144..=255 => (0x190 + symbol - 144, 9),
        256..=279 => (symbol - 256, 7),
        _ => (0xC0 + symbol - 280, 8),
    };
    bits.put_reversed(u32::from(code), len);
}

/// The base length of each length symbol 257 to 285, and its extra bits.
const LENGTHS: [(u16, u8); 29] = [
    (3, 0),
    (4, 0),
    (5, 0),
    (6, 0),
    (7, 0),
    (8, 0),
    (9, 0),
    (10, 0),
    (11, 1),
    (13, 1),
    (15, 1),
    (17, 1),
    (19, 2),
    (23, 2),
    (27, 2),
    (31, 2),
    (35, 3),
    (43, 3),
    (51, 3),
    (59, 3),
    (67, 4),
    (83, 4),
    (99, 4),
    (115, 4),
    (131, 5),
    (163, 5),
    (195, 5),
    (227, 5),
    (258, 0),
];

/// The base distance of each distance code 0 to 29, and its extra bits.
const DISTANCES: [(u16, u8); 30] = [
    (1, 0),
    (2, 0),
    (3, 0),
    (4, 0),
    (5, 1),
    (7, 1),
    (9, 2),
    (13, 2),
    (17, 3),
    (25, 3),
    (33, 4),
    (49, 4),
    (65, 5),
    (97, 5),
    (129, 6),
    (193, 6),
    (257, 7),
    (385, 7),
    (513, 8),
    (769, 8),
    (1025, 9),
    (1537, 9),
    (2049, 10),
    (3073, 10),
    (4097, 11),
    (6145, 11),
    (8193, 12),
    (12289, 12),
    (16385, 13),
    (24577, 13),
];

fn put_length(bits: &mut BitWriter, len: usize) {
    // `len` is 3 to 258, so a code exists: the last base not above it.
    let code = LENGTHS
        .iter()
        .rposition(|&(base, _)| usize::from(base) <= len)
        .unwrap_or(0);
    let (base, extra) = LENGTHS[code];
    put_symbol(bits, 257 + code as u16);
    bits.put((len - usize::from(base)) as u32, extra);
}

fn put_distance(bits: &mut BitWriter, dist: usize) {
    // `dist` is 1 to 32767, so a code exists.
    let code = DISTANCES
        .iter()
        .rposition(|&(base, _)| usize::from(base) <= dist)
        .unwrap_or(0);
    let (base, extra) = DISTANCES[code];
    bits.put_reversed(code as u32, 5);
    bits.put((dist - usize::from(base)) as u32, extra);
}

/// Bits packed from the least significant bit of each byte, as deflate
/// packs them.
#[derive(Default)]
struct BitWriter {
    bytes: Vec<u8>,
    acc: u64,
    count: u8,
}

impl BitWriter {
    /// `len` bits of `value`, least significant first.
    fn put(&mut self, value: u32, len: u8) {
        self.acc |= u64::from(value) << self.count;
        self.count += len;
        while self.count >= 8 {
            self.bytes.push(self.acc as u8);
            self.acc >>= 8;
            self.count -= 8;
        }
    }

    /// A Huffman code of `len` bits, most significant first.
    fn put_reversed(&mut self, code: u32, len: u8) {
        let reversed = code.reverse_bits() >> (32 - u32::from(len));
        self.put(reversed, len);
    }

    fn finish(mut self) -> Vec<u8> {
        if self.count > 0 {
            self.bytes.push(self.acc as u8);
        }
        self.bytes
    }
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    // A PNG chunk is far below 4 GiB here: the largest is the IDAT of a
    // contact sheet, a few megabytes.
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

    /// An inflater for one fixed-Huffman block, to check the writer.
    struct BitReader<'a> {
        bytes: &'a [u8],
        pos: usize,
    }

    impl BitReader<'_> {
        fn bit(&mut self) -> u32 {
            let b = u32::from(self.bytes[self.pos / 8] >> (self.pos % 8)) & 1;
            self.pos += 1;
            b
        }

        fn bits(&mut self, n: u8) -> u32 {
            (0..n).fold(0, |acc, i| acc | self.bit() << i)
        }

        fn huffman(&mut self, n: u8) -> u32 {
            (0..n).fold(0, |acc, _| acc << 1 | self.bit())
        }

        fn symbol(&mut self) -> u16 {
            let mut code = self.huffman(7);
            if code <= 0x17 {
                return code as u16 + 256;
            }
            code = code << 1 | self.bit();
            match code {
                0x30..=0xBF => (code - 0x30) as u16,
                0xC0..=0xC7 => (code - 0xC0) as u16 + 280,
                _ => ((code << 1 | self.bit()) - 0x190) as u16 + 144,
            }
        }
    }

    fn inflate(z: &[u8]) -> Vec<u8> {
        assert_eq!(&z[..2], &[0x78, 0x01]);
        let mut r = BitReader {
            bytes: &z[2..z.len() - 4],
            pos: 0,
        };
        assert_eq!(r.bits(1), 1, "final block");
        assert_eq!(r.bits(2), 1, "fixed Huffman");
        let mut out: Vec<u8> = Vec::new();
        loop {
            let s = r.symbol();
            match s {
                0..=255 => out.push(s as u8),
                256 => break,
                _ => {
                    let (base, extra) = LENGTHS[usize::from(s - 257)];
                    let len = usize::from(base) + r.bits(extra) as usize;
                    let code = r.huffman(5) as usize;
                    let (dbase, dextra) = DISTANCES[code];
                    let dist = usize::from(dbase) + r.bits(dextra) as usize;
                    for _ in 0..len {
                        out.push(out[out.len() - dist]);
                    }
                }
            }
        }
        assert_eq!(z[z.len() - 4..], adler32(&out).to_be_bytes());
        out
    }

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
    fn the_compressed_data_inflates_to_the_input() {
        let mut data: Vec<u8> = (0..70_000_u32).map(|i| (i % 251) as u8).collect();
        data.extend(std::iter::repeat_n(7_u8, 1_000));
        data.extend((0..=255_u8).rev());
        data.extend(b"abcabcabcabcx");
        for sample in [&data[..], &[][..], &[1, 2][..], &[9; 300][..]] {
            assert_eq!(inflate(&zlib(sample)), sample);
        }
        assert!(zlib(&data).len() < data.len() / 10, "repeats compress");
    }

    #[test]
    fn the_up_filter_keeps_the_first_row_and_subtracts_the_row_above() {
        let raw = scanlines(1, &[10, 20, 30, 11, 22, 33]);
        assert_eq!(raw, [2, 10, 20, 30, 2, 1, 2, 3]);
    }
}
