use std::fmt;

/// A field rate in millihertz, printed in hertz with three decimals.
pub(crate) struct Hz(pub(crate) u64);

/// A line rate in millihertz, printed in kilohertz with three decimals.
pub(crate) struct KHz(pub(crate) u64);

impl fmt::Display for Hz {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{:03} Hz", self.0 / 1_000, self.0 % 1_000)
    }
}

impl fmt::Display for KHz {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}.{:03} kHz",
            self.0 / 1_000_000,
            self.0 % 1_000_000 / 1_000
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rates_print_with_three_decimals() {
        assert_eq!(KHz(15_734_266).to_string(), "15.734 kHz");
        assert_eq!(Hz(59_940).to_string(), "59.940 Hz");
        assert_eq!(Hz(120_198).to_string(), "120.198 Hz");
    }
}
