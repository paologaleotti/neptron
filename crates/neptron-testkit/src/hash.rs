/// FNV-1a 64 offset basis.
const OFFSET: u64 = 0xCBF2_9CE4_8422_2325;
/// FNV-1a 64 prime.
const PRIME: u64 = 0x0000_0100_0000_01B3;

/// A 64-bit hash of `bytes` that is the same on every host and in every
/// run.
///
/// It is FNV-1a 64 over little-endian 64-bit words instead of single
/// bytes, which is eight times fewer steps in an unoptimised test build.
/// The last word is padded with zero bytes, and the length is mixed in
/// last, so that the padding cannot collide with real zero bytes. It is
/// not a cryptographic hash.
pub fn frame_hash(bytes: &[u8]) -> u64 {
    let mut hash = OFFSET;
    let mut words = bytes.chunks_exact(8);
    for chunk in &mut words {
        let mut word = [0_u8; 8];
        word.copy_from_slice(chunk);
        hash = (hash ^ u64::from_le_bytes(word)).wrapping_mul(PRIME);
    }
    let rest = words.remainder();
    if !rest.is_empty() {
        let mut word = [0_u8; 8];
        word[..rest.len()].copy_from_slice(rest);
        hash = (hash ^ u64::from_le_bytes(word)).wrapping_mul(PRIME);
    }
    (hash ^ bytes.len() as u64).wrapping_mul(PRIME)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_values() {
        assert_eq!(frame_hash(&[]), OFFSET.wrapping_mul(PRIME));
        let one = (OFFSET ^ 1).wrapping_mul(PRIME);
        assert_eq!(frame_hash(&[1]), (one ^ 1).wrapping_mul(PRIME));
    }

    #[test]
    fn padding_and_length_count() {
        assert_ne!(frame_hash(&[0]), frame_hash(&[0, 0]));
        assert_ne!(frame_hash(&[1, 0, 0, 0, 0, 0, 0, 0]), frame_hash(&[1]));
        assert_ne!(frame_hash(&[1, 2]), frame_hash(&[2, 1]));
        let long: Vec<u8> = (0..=255).collect();
        assert_eq!(frame_hash(&long), frame_hash(&long.clone()));
    }
}
