//! Standard base64 (RFC 4648, section 4) with padding, for binary payloads in
//! a JSON line.

/// A base64 text that does not decode.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Base64Error {
    /// The text length is not a multiple of 4.
    #[error("base64 text has length {len}, which is not a multiple of 4")]
    Length {
        /// The length of the text, in bytes.
        len: usize,
    },
    /// A byte is not in the alphabet, or a padding byte is out of place.
    #[error("base64 text has the byte {byte:#04x} at offset {offset}, which is not valid there")]
    Byte {
        /// The byte.
        byte: u8,
        /// Its offset in the text.
        offset: usize,
    },
}

/// Encodes `bytes` as standard base64 with padding.
pub fn encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0];
        let b1 = chunk.get(1).copied().unwrap_or(0);
        let b2 = chunk.get(2).copied().unwrap_or(0);
        let n = (u32::from(b0) << 16) | (u32::from(b1) << 8) | u32::from(b2);
        out.push(symbol(n >> 18));
        out.push(symbol(n >> 12));
        out.push(if chunk.len() > 1 { symbol(n >> 6) } else { '=' });
        out.push(if chunk.len() > 2 { symbol(n) } else { '=' });
    }
    out
}

/// Decodes standard base64 with padding. Line breaks and white space are not
/// accepted.
pub fn decode(text: &str) -> Result<Vec<u8>, Base64Error> {
    let bytes = text.as_bytes();
    if !bytes.len().is_multiple_of(4) {
        return Err(Base64Error::Length { len: bytes.len() });
    }
    let mut out = Vec::with_capacity(bytes.len() / 4 * 3);
    let quads = bytes.len() / 4;
    for (q, quad) in bytes.chunks_exact(4).enumerate() {
        let last = q + 1 == quads;
        let base = q * 4;
        let pad = match (quad[2], quad[3]) {
            (b'=', b'=') if last => 2,
            (_, b'=') if last => 1,
            _ => 0,
        };
        let mut n = 0u32;
        for (i, &byte) in quad.iter().enumerate() {
            let value = if i >= 4 - pad {
                0
            } else {
                value(byte).ok_or(Base64Error::Byte {
                    byte,
                    offset: base + i,
                })?
            };
            n = (n << 6) | value;
        }
        let [_, b0, b1, b2] = n.to_be_bytes();
        out.push(b0);
        if pad < 2 {
            out.push(b1);
        }
        if pad < 1 {
            out.push(b2);
        }
    }
    Ok(out)
}

/// Serde support: `#[serde(with = "crate::base64::serde_bytes")]` on a
/// `Vec<u8>` field writes it as one base64 string.
pub mod serde_bytes {
    use serde::{Deserialize, Deserializer, Serializer};

    /// Writes the bytes as a base64 string.
    pub fn serialize<S: Serializer>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&super::encode(bytes))
    }

    /// Reads a base64 string into bytes.
    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<u8>, D::Error> {
        let text = String::deserialize(deserializer)?;
        super::decode(&text).map_err(serde::de::Error::custom)
    }
}

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn symbol(six_bits: u32) -> char {
    char::from(ALPHABET[(six_bits & 0x3f) as usize])
}

fn value(byte: u8) -> Option<u32> {
    let v = match byte {
        b'A'..=b'Z' => byte - b'A',
        b'a'..=b'z' => byte - b'a' + 26,
        b'0'..=b'9' => byte - b'0' + 52,
        b'+' => 62,
        b'/' => 63,
        _ => return None,
    };
    Some(u32::from(v))
}

#[cfg(test)]
mod tests {
    use super::*;

    // The test vectors of RFC 4648, section 10.
    const VECTORS: &[(&str, &str)] = &[
        ("", ""),
        ("f", "Zg=="),
        ("fo", "Zm8="),
        ("foo", "Zm9v"),
        ("foob", "Zm9vYg=="),
        ("fooba", "Zm9vYmE="),
        ("foobar", "Zm9vYmFy"),
    ];

    #[test]
    fn rfc_vectors_encode() {
        for (plain, coded) in VECTORS {
            assert_eq!(encode(plain.as_bytes()), *coded);
        }
    }

    #[test]
    fn rfc_vectors_decode() {
        for (plain, coded) in VECTORS {
            assert_eq!(decode(coded).unwrap(), plain.as_bytes());
        }
    }

    #[test]
    fn every_byte_value_round_trips() {
        let all: Vec<u8> = (0..=255).collect();
        for len in 0..all.len() {
            assert_eq!(decode(&encode(&all[..len])).unwrap(), &all[..len]);
        }
    }

    #[test]
    fn length_not_a_multiple_of_four_is_refused() {
        assert_eq!(decode("Zm9"), Err(Base64Error::Length { len: 3 }));
    }

    #[test]
    fn byte_outside_the_alphabet_is_refused() {
        assert_eq!(
            decode("Zm-v"),
            Err(Base64Error::Byte {
                byte: b'-',
                offset: 2
            })
        );
    }

    #[test]
    fn padding_before_the_last_quad_is_refused() {
        assert_eq!(
            decode("Zg==Zm9v"),
            Err(Base64Error::Byte {
                byte: b'=',
                offset: 2
            })
        );
    }

    #[test]
    fn padding_in_the_first_two_places_is_refused() {
        assert_eq!(
            decode("Z==="),
            Err(Base64Error::Byte {
                byte: b'=',
                offset: 1
            })
        );
    }
}
