//! A stateless seeded generator. A value depends only on the seed and an
//! index, so the order of the calls never changes a result.

/// The `index`-th value of the SplitMix64 sequence that starts at `seed`.
pub(crate) fn mix64(seed: u64, index: u64) -> u64 {
    let mut z = seed.wrapping_add(index.wrapping_add(1).wrapping_mul(0x9E37_79B9_7F4A_7C15));
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// A value in `[-max, max]` from `mix64(seed, index)`.
pub(crate) fn symmetric(seed: u64, index: u64, max: u64) -> i64 {
    let max = max.min(i64::MAX as u64);
    let span = u128::from(max) * 2 + 1;
    let value = (u128::from(mix64(seed, index)) % span) as i128 - i128::from(max as i64);
    // The remainder is below 2 * max + 1, so the value is in [-max, max]
    // and max is at most i64::MAX.
    value as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_values() {
        // The first outputs of SplitMix64 with seed 0, as in the reference
        // implementation by Sebastiano Vigna.
        assert_eq!(mix64(0, 0), 0xE220_A839_7B1D_CDAF);
        assert_eq!(mix64(0, 1), 0x6E78_9E6A_A1B9_65F4);
        assert_ne!(mix64(1, 0), mix64(0, 0));
    }

    #[test]
    fn symmetric_stays_in_range() {
        for i in 0..1_000 {
            let v = symmetric(42, i, 500);
            assert!((-500..=500).contains(&v), "{v}");
        }
        assert_eq!(symmetric(42, 7, 0), 0);
        let wide = symmetric(1, 1, u64::MAX);
        assert!(wide >= -i64::MAX);
    }
}
