use neptron_types::GpuCaps;

/// `caps` with its highest pixel clock and its largest mode lowered to
/// those of `limit`, never raised. The other values stay.
pub fn lowered_caps(caps: &GpuCaps, limit: &GpuCaps) -> GpuCaps {
    GpuCaps {
        pclock_max_hz: caps.pclock_max_hz.min(limit.pclock_max_hz),
        max_width: caps.max_width.min(limit.max_width),
        max_height: caps.max_height.min(limit.max_height),
        ..*caps
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIGITAL: GpuCaps = GpuCaps {
        pclock_min_hz: 25_000_000,
        pclock_max_hz: 533_250_000,
        interlace: false,
        flip_per_field: false,
        max_width: 3840,
        max_height: 2160,
    };

    #[test]
    fn caps_only_go_down() {
        let analog = GpuCaps {
            pclock_min_hz: 1,
            pclock_max_hz: 165_000_000,
            interlace: true,
            max_width: 1920,
            max_height: 1200,
            ..DIGITAL
        };
        let low = lowered_caps(&DIGITAL, &analog);
        assert_eq!(
            (low.pclock_max_hz, low.max_width, low.max_height),
            (165_000_000, 1920, 1200)
        );
        assert_eq!(low.pclock_min_hz, DIGITAL.pclock_min_hz);
        assert!(!low.interlace);
        let high = GpuCaps {
            pclock_max_hz: 900_000_000,
            max_width: 8192,
            max_height: 8192,
            ..DIGITAL
        };
        assert_eq!(lowered_caps(&DIGITAL, &high), DIGITAL);
    }
}
