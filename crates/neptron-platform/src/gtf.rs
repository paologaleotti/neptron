//! Porch and sync times for a line-rate range, from the default parameters
//! of the VESA Generalized Timing Formula (GTF 1.1, 1999).
//!
//! EDID range limits give rates only. The porches and syncs of a profile
//! built from them come from GTF with its default parameters
//! (C = 40 %, M = 600 %/kHz, K = 128, J = 20 %): a horizontal blanking
//! duty cycle of `C' - M' * period / 1000` with `C' = 30` and `M' = 300`,
//! a horizontal sync of 8 % of the line, the blanking split in two halves
//! around the end of the sync, a vertical front porch of 1 line, a vertical
//! sync of 3 lines, and at least 550 us of vertical sync and back porch.
//!
//! A profile holds one value per porch for the whole line-rate range, while
//! GTF gives a value per line rate. Each porch is a minimum, so it gets
//! the largest GTF value anywhere in the range, rounded up to whole
//! nanoseconds. Each sync is a target, so it gets the GTF value at the
//! lowest line rate, rounded up.

/// The porch and sync times for a range of line rates, in nanoseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Blanking {
    pub h_front_porch_ns: u64,
    pub h_sync_ns: u64,
    pub h_back_porch_ns: u64,
    pub v_front_porch_ns: u64,
    pub v_sync_ns: u64,
    pub v_back_porch_ns: u64,
}

impl Blanking {
    /// The vertical blanking, in nanoseconds.
    pub(crate) fn v_total_ns(&self) -> u64 {
        self.v_front_porch_ns + self.v_sync_ns + self.v_back_porch_ns
    }
}

/// The GTF blanking for line rates from `h_min_mhz` to `h_max_mhz`.
///
/// Both rates must be at least 1 Hz (1000 mHz) and `h_min_mhz` must not
/// be above `h_max_mhz`; the EDID parser ensures more than that.
pub(crate) fn blanking(h_min_mhz: u64, h_max_mhz: u64) -> Blanking {
    let lo = u128::from(h_min_mhz);
    let hi = u128::from(h_max_mhz);
    Blanking {
        h_front_porch_ns: narrow(half_blank_max_ns(FRONT_PORCH_A, lo, hi)),
        h_sync_ns: narrow(ceil_div(H_SYNC_PERCENT * NS_TIMES_MHZ, 100 * lo)),
        h_back_porch_ns: narrow(half_blank_max_ns(BACK_PORCH_A, lo, hi)),
        v_front_porch_ns: narrow(ceil_div(V_FRONT_PORCH_LINES * NS_TIMES_MHZ, lo)),
        v_sync_ns: narrow(ceil_div(V_SYNC_LINES * NS_TIMES_MHZ, lo)),
        v_back_porch_ns: narrow(
            MIN_V_SYNC_BACK_PORCH_NS.saturating_sub(V_SYNC_LINES * NS_TIMES_MHZ / hi),
        ),
    }
}

/// A line period in nanoseconds is this value divided by the line rate in
/// millihertz.
const NS_TIMES_MHZ: u128 = 1_000_000_000_000;

const GTF_C_PERCENT: u128 = 40;
const GTF_M_PERCENT_PER_KHZ: u128 = 600;
const GTF_K: u128 = 128;
const GTF_J_PERCENT: u128 = 20;
const C_PRIME_PERCENT: u128 = (GTF_C_PERCENT - GTF_J_PERCENT) * GTF_K / 256 + GTF_J_PERCENT;
const M_PRIME: u128 = GTF_K * GTF_M_PERCENT_PER_KHZ / 256;
const H_SYNC_PERCENT: u128 = 8;
const V_FRONT_PORCH_LINES: u128 = 1;
const V_SYNC_LINES: u128 = 3;
const MIN_V_SYNC_BACK_PORCH_NS: u128 = 550_000;

// With the line rate h in mHz and the period P = NS_TIMES_MHZ / h in ns,
// the blanking is (C' - M' * P / 1_000_000) / 100 * P, which is
// (C' * 1e12 * h - M' * 1e18) / (100 * h^2). Half of it is the back porch;
// half of it minus the sync (8 % of P, or 16e12 * h / (200 * h^2)) is the
// front porch. Both have the form (a * h - b) / (200 * h^2).
const BACK_PORCH_A: u128 = C_PRIME_PERCENT * NS_TIMES_MHZ;
const FRONT_PORCH_A: u128 = (C_PRIME_PERCENT - 2 * H_SYNC_PERCENT) * NS_TIMES_MHZ;
const HALF_BLANK_B: u128 = M_PRIME * 1_000_000_000_000_000_000;
const HALF_BLANK_DENOMINATOR: u128 = 200;

/// The largest value of `(a * h - b) / (200 * h^2)` for `h` from `lo` to
/// `hi`, rounded up; 0 where the value is negative.
///
/// In `x = 1 / h` the function is `(a * x - b * x^2) / 200`, a downward
/// parabola with its top at `h = 2 * b / a`. The largest value is at the
/// top when the top is inside the range, and at one of the ends otherwise.
fn half_blank_max_ns(a: u128, lo: u128, hi: u128) -> u128 {
    let b = HALF_BLANK_B;
    let top = a * lo <= 2 * b && 2 * b <= a * hi;
    if top {
        return ceil_div(a * a, 4 * b * HALF_BLANK_DENOMINATOR);
    }
    let at = |h: u128| ceil_div((a * h).saturating_sub(b), HALF_BLANK_DENOMINATOR * h * h);
    at(lo).max(at(hi))
}

fn ceil_div(numerator: u128, denominator: u128) -> u128 {
    numerator.div_ceil(denominator)
}

/// Every value here is at most 3 lines at 1 Hz (3e9 ns), so it fits a
/// u64; the saturation only guards that bound.
fn narrow(value: u128) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    const KHZ: u64 = 1_000_000;

    #[test]
    fn derived_gtf_constants() {
        assert_eq!(C_PRIME_PERCENT, 30);
        assert_eq!(M_PRIME, 300);
    }

    #[test]
    fn pc_crt_30_to_71_khz() {
        let b = blanking(30 * KHZ, 71 * KHZ);
        // Front porch: top at 2 * 3e20 / 14e12 = 42.857 kHz, inside the
        // range: (14e12)^2 / (4 * 3e20 * 200) = 816.67 ns.
        assert_eq!(b.h_front_porch_ns, 817);
        // Sync: 8 % of 33 333.33 ns.
        assert_eq!(b.h_sync_ns, 2_667);
        // Back porch: top at 20 kHz, below the range, so the value at
        // 30 kHz: (9e20 - 3e20) / (200 * 9e14) = 3 333.33 ns.
        assert_eq!(b.h_back_porch_ns, 3_334);
        // One line at 30 kHz.
        assert_eq!(b.v_front_porch_ns, 33_334);
        // Three lines at 30 kHz.
        assert_eq!(b.v_sync_ns, 100_000);
        // 550 000 ns less three lines at 71 kHz (42 253.52 ns).
        assert_eq!(b.v_back_porch_ns, 507_747);
        assert_eq!(b.v_total_ns(), 641_081);
    }

    #[test]
    fn single_rate_at_31_5_khz_matches_gtf() {
        let b = blanking(31_500 * 1_000, 31_500 * 1_000);
        // Period 31 746.03 ns, duty 30 - 300 * 31.746 / 1000 = 20.476 %,
        // blanking 6 500.38 ns, half 3 250.19 ns, sync 2 539.68 ns.
        assert_eq!(b.h_back_porch_ns, 3_251);
        assert_eq!(b.h_sync_ns, 2_540);
        assert_eq!(b.h_front_porch_ns, 711);
    }

    #[test]
    fn back_porch_top_inside_the_range() {
        // The back porch peaks at 20 kHz: 30e12^2 / (4 * 3e20 * 200) =
        // 3 750 ns.
        let b = blanking(15 * KHZ, 40 * KHZ);
        assert_eq!(b.h_back_porch_ns, 3_750);
    }

    #[test]
    fn negative_blanking_is_zero() {
        // At 5 kHz the GTF duty cycle is 30 - 60 = -30 %.
        let b = blanking(5 * KHZ, 5 * KHZ);
        assert_eq!(b.h_front_porch_ns, 0);
        assert_eq!(b.h_back_porch_ns, 0);
        // Three lines at 5 kHz are 600 000 ns, more than 550 000 ns.
        assert_eq!(b.v_back_porch_ns, 0);
    }

    #[test]
    fn widest_edid_range_does_not_overflow() {
        let b = blanking(KHZ, 510 * KHZ);
        assert_eq!(b.v_sync_ns, 3_000_000);
        assert!(b.h_back_porch_ns > 0);
    }
}
