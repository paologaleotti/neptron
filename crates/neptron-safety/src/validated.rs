use neptron_types::{GpuCaps, Modeline, MonitorKind, MonitorProfile};

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
///
/// # Panics
///
/// The checks of a CRT profile are not implemented yet, so a call with a
/// CRT profile panics.
pub fn validate(
    mode: &Modeline,
    profile: &MonitorProfile,
    caps: &GpuCaps,
) -> Result<ValidatedMode, Rejection> {
    check_structure(mode)?;
    check_gpu(mode, caps)?;
    match &profile.kind {
        MonitorKind::Crt { .. } => todo!("CRT checks for profile {}", profile.id),
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
