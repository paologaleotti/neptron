use neptron_types::{GpuCaps, Modeline, MonitorKind, MonitorProfile};

use crate::measure::Measure;
use crate::range::check_ranges;
use crate::structure::check_structure;
use crate::{InterlaceLimit, Rejection};

/// A mode that passed `validate`. It is the only input that can program a
/// display.
///
/// The field is private and the type has no `Default`, no public
/// constructor and no `Deserialize`, so `validate` is the only way to make
/// one:
///
/// ```compile_fail,E0277
/// let mode: neptron_safety::ValidatedMode = Default::default();
/// ```
///
/// ```compile_fail,E0451
/// fn forge(mode: neptron_types::Modeline) -> neptron_safety::ValidatedMode {
///     neptron_safety::ValidatedMode { mode }
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ValidatedMode {
    mode: Modeline,
}

impl ValidatedMode {
    /// The modeline that passed the checks.
    pub fn mode(&self) -> &Modeline {
        &self.mode
    }
}

/// Checks `mode` against the monitor profile, the GPU capabilities and the
/// hard rails, with integer maths only. It judges the modeline it gets and
/// changes nothing.
///
/// # Errors
///
/// Returns the first [`Rejection`] it finds, with the value and the limit.
pub fn validate(
    mode: &Modeline,
    profile: &MonitorProfile,
    caps: &GpuCaps,
) -> Result<ValidatedMode, Rejection> {
    check_structure(mode)?;
    check_gpu(mode, caps)?;
    match &profile.kind {
        MonitorKind::Crt { ranges } => check_ranges(&Measure::of(mode), ranges)?,
        MonitorKind::FlatPanel { modes } => check_panel(mode, modes)?,
    }
    Ok(ValidatedMode { mode: *mode })
}

fn check_gpu(mode: &Modeline, caps: &GpuCaps) -> Result<(), Rejection> {
    if mode.pclock_hz < caps.pclock_min_hz || mode.pclock_hz > caps.pclock_max_hz {
        return Err(Rejection::PixelClock {
            value_hz: mode.pclock_hz,
            min_hz: caps.pclock_min_hz,
            max_hz: caps.pclock_max_hz,
        });
    }
    if mode.interlace && !caps.interlace {
        return Err(Rejection::Interlace {
            forbidden_by: InterlaceLimit::Gpu,
        });
    }
    Ok(())
}

/// A flat panel takes only the modes it lists, field for field.
fn check_panel(mode: &Modeline, modes: &[Modeline]) -> Result<(), Rejection> {
    if modes.contains(mode) {
        Ok(())
    } else {
        Err(Rejection::NotAPanelMode {
            mode: *mode,
            panel_modes: modes.len(),
        })
    }
}

#[cfg(test)]
mod tests {
    use neptron_types::CrtRange;

    use super::*;

    const CAPS: GpuCaps = GpuCaps {
        pclock_min_hz: 25_000_000,
        pclock_max_hz: 400_000_000,
        interlace: true,
        flip_per_field: false,
        max_width: 4096,
        max_height: 4096,
    };

    fn vga() -> Modeline {
        Modeline {
            pclock_hz: 25_175_000,
            hactive: 640,
            hsync_start: 656,
            hsync_end: 752,
            htotal: 800,
            vactive: 480,
            vsync_start: 490,
            vsync_end: 492,
            vtotal: 525,
            interlace: false,
            doublescan: false,
            hsync_positive: false,
            vsync_positive: false,
        }
    }

    /// 320x240 at 60.03 Hz, 15.728 kHz, with porches and syncs at or
    /// above the nominal times of generic_15.
    fn mode_240p() -> Modeline {
        Modeline {
            pclock_hz: 6_700_000,
            hactive: 320,
            hsync_start: 334,
            hsync_end: 366,
            htotal: 426,
            vactive: 240,
            vsync_start: 241,
            vsync_end: 244,
            vtotal: 262,
            ..vga()
        }
    }

    fn generic_15() -> MonitorProfile {
        MonitorProfile {
            id: "generic_15".to_owned(),
            kind: MonitorKind::Crt {
                ranges: vec![CrtRange {
                    h_min_mhz: 15_625_000,
                    h_max_mhz: 15_750_000,
                    v_min_mhz: 49_500,
                    v_max_mhz: 65_000,
                    h_front_porch_ns: 2_000,
                    h_sync_ns: 4_700,
                    h_back_porch_ns: 8_000,
                    v_front_porch_ns: 64_000,
                    v_sync_ns: 192_000,
                    v_back_porch_ns: 1_024_000,
                    hsync_positive: false,
                    vsync_positive: false,
                    progressive_lines_min: 192,
                    progressive_lines_max: 288,
                    interlaced_lines_min: 448,
                    interlaced_lines_max: 576,
                }],
            },
        }
    }

    const LOW_CLOCK: GpuCaps = GpuCaps {
        pclock_min_hz: 1_000_000,
        ..CAPS
    };

    #[test]
    fn a_crt_mode_inside_the_profile_passes_unchanged() {
        let validated = validate(&mode_240p(), &generic_15(), &LOW_CLOCK).unwrap();
        assert_eq!(validated.mode(), &mode_240p());
    }

    #[test]
    fn a_crt_mode_outside_the_profile_is_rejected() {
        let vga_on_15 = validate(&vga(), &generic_15(), &CAPS);
        assert_eq!(
            vga_on_15,
            Err(Rejection::LineRate {
                value_mhz: 31_468_750,
                min_mhz: 15_625_000,
                max_mhz: 15_750_000
            })
        );
        let short_porch = Modeline {
            hsync_start: 330,
            hsync_end: 362,
            ..mode_240p()
        };
        // 10 px at 6.7 MHz is 1492.5 ns.
        assert_eq!(
            validate(&short_porch, &generic_15(), &LOW_CLOCK),
            Err(Rejection::Porch {
                axis: crate::Axis::Horizontal,
                edge: crate::PorchEdge::Front,
                value_ns: 1_492,
                min_ns: 1_800
            })
        );
    }

    #[test]
    fn gpu_limits_come_before_the_profile() {
        assert_eq!(
            validate(&mode_240p(), &generic_15(), &CAPS),
            Err(Rejection::PixelClock {
                value_hz: 6_700_000,
                min_hz: 25_000_000,
                max_hz: 400_000_000
            })
        );
    }

    fn panel(modes: Vec<Modeline>) -> MonitorProfile {
        MonitorProfile {
            id: "panel".to_owned(),
            kind: MonitorKind::FlatPanel { modes },
        }
    }

    #[test]
    fn a_listed_panel_mode_passes_unchanged() {
        let validated = validate(&vga(), &panel(vec![vga()]), &CAPS).unwrap();
        assert_eq!(validated.mode(), &vga());
    }

    #[test]
    fn a_panel_mode_must_match_every_field() {
        let other = Modeline {
            hsync_positive: true,
            ..vga()
        };
        assert_eq!(
            validate(&other, &panel(vec![vga()]), &CAPS),
            Err(Rejection::NotAPanelMode {
                mode: other,
                panel_modes: 1
            })
        );
        assert_eq!(
            validate(&vga(), &panel(Vec::new()), &CAPS),
            Err(Rejection::NotAPanelMode {
                mode: vga(),
                panel_modes: 0
            })
        );
    }

    #[test]
    fn structure_is_checked_first() {
        let broken = Modeline {
            htotal: 700,
            ..vga()
        };
        assert_eq!(
            validate(&broken, &panel(vec![broken]), &CAPS),
            Err(Rejection::Malformed {
                field: "htotal",
                value: 700,
                limit: 752
            })
        );
    }

    fn pclock(pclock_hz: u32) -> Result<ValidatedMode, Rejection> {
        let mode = Modeline { pclock_hz, ..vga() };
        validate(&mode, &panel(vec![mode]), &CAPS)
    }

    #[test]
    fn pixel_clock_boundaries() {
        let rejected = |value_hz| {
            Err(Rejection::PixelClock {
                value_hz,
                min_hz: 25_000_000,
                max_hz: 400_000_000,
            })
        };
        assert_eq!(pclock(24_999_999), rejected(24_999_999));
        assert!(pclock(25_000_000).is_ok());
        assert!(pclock(25_000_001).is_ok());
        assert!(pclock(399_999_999).is_ok());
        assert!(pclock(400_000_000).is_ok());
        assert_eq!(pclock(400_000_001), rejected(400_000_001));
        assert_eq!(pclock(6_000_000), rejected(6_000_000));
    }

    #[test]
    fn interlace_needs_gpu_support() {
        let interlaced = Modeline {
            interlace: true,
            ..vga()
        };
        let profile = panel(vec![interlaced]);
        assert!(validate(&interlaced, &profile, &CAPS).is_ok());
        let no_interlace = GpuCaps {
            interlace: false,
            ..CAPS
        };
        assert_eq!(
            validate(&interlaced, &profile, &no_interlace),
            Err(Rejection::Interlace {
                forbidden_by: InterlaceLimit::Gpu
            })
        );
        let progressive = panel(vec![vga()]);
        assert!(validate(&vga(), &progressive, &no_interlace).is_ok());
    }
}
