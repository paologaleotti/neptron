//! CRC-32 (IEEE 802.3, reflected, polynomial 0xEDB88320), as in zlib.

const POLYNOMIAL: u32 = 0xEDB8_8320;

const TABLE: [u32; 256] = make_table();

const fn make_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    let mut i = 0;
    while i < 256 {
        let mut c = i as u32;
        let mut bit = 0;
        while bit < 8 {
            c = if c & 1 == 0 {
                c >> 1
            } else {
                POLYNOMIAL ^ (c >> 1)
            };
            bit += 1;
        }
        table[i] = c;
        i += 1;
    }
    table
}

pub(super) fn crc32(bytes: &[u8]) -> u32 {
    let mut c = !0u32;
    for &b in bytes {
        c = TABLE[((c ^ u32::from(b)) & 0xFF) as usize] ^ (c >> 8);
    }
    !c
}

#[cfg(test)]
mod tests {
    use super::crc32;

    #[test]
    fn matches_the_standard_check_values() {
        assert_eq!(crc32(b""), 0);
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(
            crc32(b"The quick brown fox jumps over the lazy dog"),
            0x414F_A339
        );
    }
}
