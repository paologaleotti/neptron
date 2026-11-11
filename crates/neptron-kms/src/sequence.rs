/// Extends the kernel's 32-bit vblank counter of one CRTC to 64 bits.
///
/// The kernel counter wraps after 2^32 vblanks (about 2.3 years at 60 Hz).
/// Two events are never that far apart, so the forward distance from the
/// last value is the number of vblanks between them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct VblankCounter {
    last: Option<(u32, u64)>,
}

impl VblankCounter {
    /// A counter that has seen no event.
    pub fn new() -> VblankCounter {
        VblankCounter { last: None }
    }

    /// The 64-bit value of the kernel value `frame`. The first value
    /// counts from the kernel's own value.
    pub fn extend(&mut self, frame: u32) -> u64 {
        let wide = match self.last {
            None => u64::from(frame),
            Some((kernel, wide)) => wide + u64::from(frame.wrapping_sub(kernel)),
        };
        self.last = Some((frame, wide));
        wide
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_count_on_from_the_kernel_value() {
        let mut c = VblankCounter::new();
        assert_eq!(c.extend(100), 100);
        assert_eq!(c.extend(101), 101);
        assert_eq!(c.extend(104), 104);
    }

    #[test]
    fn a_wrap_keeps_counting() {
        let mut c = VblankCounter::new();
        assert_eq!(c.extend(u32::MAX - 1), u64::from(u32::MAX) - 1);
        assert_eq!(c.extend(u32::MAX), u64::from(u32::MAX));
        assert_eq!(c.extend(0), u64::from(u32::MAX) + 1);
        assert_eq!(c.extend(2), u64::from(u32::MAX) + 3);
    }

    #[test]
    fn an_equal_value_adds_nothing() {
        let mut c = VblankCounter::new();
        c.extend(7);
        assert_eq!(c.extend(7), 7);
    }
}
