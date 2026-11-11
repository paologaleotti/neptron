//! The kernel's mode record (`struct drm_mode_modeinfo`) and the
//! conversions between it and a `Modeline`.
//!
//! A record that goes to the kernel is made from a `ValidatedMode` only.
//! The kernel takes the pixel clock in kilohertz, so a validated modeline
//! whose clock is not a whole number of kilohertz is refused instead of
//! rounded: the kernel must get exactly the modeline that passed.

use std::fmt;

use neptron_safety::ValidatedMode;
use neptron_types::Modeline;

/// The kernel's mode record, byte for byte (`struct drm_mode_modeinfo` in
/// `drm_mode.h`, 68 bytes, no padding). The fields are private: only
/// [`KernelMode::from_validated`] makes one.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KernelMode {
    clock_khz: u32,
    hdisplay: u16,
    hsync_start: u16,
    hsync_end: u16,
    htotal: u16,
    hskew: u16,
    vdisplay: u16,
    vsync_start: u16,
    vsync_end: u16,
    vtotal: u16,
    vscan: u16,
    vrefresh: u32,
    flags: u32,
    kind: u32,
    name: [u8; NAME_LEN],
}

/// The mode fields as the kernel reports them, before conversion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RawMode {
    /// Pixel clock in kilohertz.
    pub clock_khz: u32,
    /// `hdisplay`, `hsync_start`, `hsync_end`, `htotal`.
    pub h: [u16; 4],
    /// `vdisplay`, `vsync_start`, `vsync_end`, `vtotal`.
    pub v: [u16; 4],
    /// `hskew`.
    pub hskew: u16,
    /// `vscan`.
    pub vscan: u16,
    /// The `DRM_MODE_FLAG_*` bits.
    pub flags: u32,
}

/// A mode that cannot be converted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConvertError {
    /// The pixel clock is not a whole number of kilohertz.
    ClockNotWholeKhz {
        /// The pixel clock, in hertz.
        pclock_hz: u32,
    },
    /// The pixel clock in hertz does not fit in 32 bits.
    ClockTooHigh {
        /// The pixel clock, in kilohertz.
        clock_khz: u32,
    },
    /// The kernel mode has flags or fields that a `Modeline` cannot hold.
    Unsupported {
        /// The flag bits that a `Modeline` cannot hold.
        flags: u32,
        /// `hskew` of the mode.
        hskew: u16,
        /// `vscan` of the mode.
        vscan: u16,
    },
    /// A sync polarity is not stated, or stated both ways.
    Polarity {
        /// The flag bits of the mode.
        flags: u32,
    },
}

/// `DRM_MODE_FLAG_PHSYNC`
pub const FLAG_PHSYNC: u32 = 1 << 0;
/// `DRM_MODE_FLAG_NHSYNC`
pub const FLAG_NHSYNC: u32 = 1 << 1;
/// `DRM_MODE_FLAG_PVSYNC`
pub const FLAG_PVSYNC: u32 = 1 << 2;
/// `DRM_MODE_FLAG_NVSYNC`
pub const FLAG_NVSYNC: u32 = 1 << 3;
/// `DRM_MODE_FLAG_INTERLACE`
pub const FLAG_INTERLACE: u32 = 1 << 4;
/// `DRM_MODE_FLAG_DBLSCAN`
pub const FLAG_DBLSCAN: u32 = 1 << 5;
/// `DRM_MODE_TYPE_USERDEF`
pub const TYPE_USERDEF: u32 = 1 << 5;

impl KernelMode {
    /// The kernel record of a validated mode, named `name` (cut to 31
    /// bytes).
    pub fn from_validated(mode: &ValidatedMode, name: &str) -> Result<KernelMode, ConvertError> {
        let m = mode.mode();
        if !m.pclock_hz.is_multiple_of(HZ_PER_KHZ) {
            return Err(ConvertError::ClockNotWholeKhz {
                pclock_hz: m.pclock_hz,
            });
        }
        let mut flags = if m.hsync_positive {
            FLAG_PHSYNC
        } else {
            FLAG_NHSYNC
        };
        flags |= if m.vsync_positive {
            FLAG_PVSYNC
        } else {
            FLAG_NVSYNC
        };
        if m.interlace {
            flags |= FLAG_INTERLACE;
        }
        if m.doublescan {
            flags |= FLAG_DBLSCAN;
        }
        let mut name_bytes = [0; NAME_LEN];
        for (dst, src) in name_bytes.iter_mut().zip(name.bytes().take(NAME_LEN - 1)) {
            *dst = src;
        }
        Ok(KernelMode {
            clock_khz: m.pclock_hz / HZ_PER_KHZ,
            hdisplay: m.hactive,
            hsync_start: m.hsync_start,
            hsync_end: m.hsync_end,
            htotal: m.htotal,
            hskew: 0,
            vdisplay: m.vactive,
            vsync_start: m.vsync_start,
            vsync_end: m.vsync_end,
            vtotal: m.vtotal,
            vscan: 0,
            vrefresh: u32::try_from((m.field_rate_mhz() + 500) / 1_000).unwrap_or(u32::MAX),
            flags,
            kind: TYPE_USERDEF,
            name: name_bytes,
        })
    }

    /// The fields of the record, as the kernel would report them.
    #[cfg(test)]
    pub fn raw(&self) -> RawMode {
        RawMode {
            clock_khz: self.clock_khz,
            h: [self.hdisplay, self.hsync_start, self.hsync_end, self.htotal],
            v: [self.vdisplay, self.vsync_start, self.vsync_end, self.vtotal],
            hskew: self.hskew,
            vscan: self.vscan,
            flags: self.flags,
        }
    }
}

impl RawMode {
    /// The modeline of a kernel mode. A mode with a flag or field that a
    /// `Modeline` cannot hold is an error, so that it is never judged as a
    /// different mode.
    pub fn to_modeline(self) -> Result<Modeline, ConvertError> {
        let unknown = self.flags & !FLAGS_KNOWN;
        if unknown != 0 || self.hskew != 0 || self.vscan > 1 {
            return Err(ConvertError::Unsupported {
                flags: unknown,
                hskew: self.hskew,
                vscan: self.vscan,
            });
        }
        let polarity = |pos: u32, neg: u32| match (self.flags & pos != 0, self.flags & neg != 0) {
            (true, false) => Ok(true),
            (false, true) => Ok(false),
            _ => Err(ConvertError::Polarity { flags: self.flags }),
        };
        let pclock_hz =
            self.clock_khz
                .checked_mul(HZ_PER_KHZ)
                .ok_or(ConvertError::ClockTooHigh {
                    clock_khz: self.clock_khz,
                })?;
        Ok(Modeline {
            pclock_hz,
            hactive: self.h[0],
            hsync_start: self.h[1],
            hsync_end: self.h[2],
            htotal: self.h[3],
            vactive: self.v[0],
            vsync_start: self.v[1],
            vsync_end: self.v[2],
            vtotal: self.v[3],
            interlace: self.flags & FLAG_INTERLACE != 0,
            doublescan: self.flags & FLAG_DBLSCAN != 0,
            hsync_positive: polarity(FLAG_PHSYNC, FLAG_NHSYNC)?,
            vsync_positive: polarity(FLAG_PVSYNC, FLAG_NVSYNC)?,
        })
    }
}

/// The fields that differ between the requested and the read-back
/// modeline, as `field: requested -> read back`.
pub fn differences(requested: &Modeline, readback: &Modeline) -> Vec<String> {
    let pairs: [(&str, u64, u64); 13] = [
        (
            "pclock_hz",
            requested.pclock_hz.into(),
            readback.pclock_hz.into(),
        ),
        ("hactive", requested.hactive.into(), readback.hactive.into()),
        (
            "hsync_start",
            requested.hsync_start.into(),
            readback.hsync_start.into(),
        ),
        (
            "hsync_end",
            requested.hsync_end.into(),
            readback.hsync_end.into(),
        ),
        ("htotal", requested.htotal.into(), readback.htotal.into()),
        ("vactive", requested.vactive.into(), readback.vactive.into()),
        (
            "vsync_start",
            requested.vsync_start.into(),
            readback.vsync_start.into(),
        ),
        (
            "vsync_end",
            requested.vsync_end.into(),
            readback.vsync_end.into(),
        ),
        ("vtotal", requested.vtotal.into(), readback.vtotal.into()),
        (
            "interlace",
            requested.interlace.into(),
            readback.interlace.into(),
        ),
        (
            "doublescan",
            requested.doublescan.into(),
            readback.doublescan.into(),
        ),
        (
            "hsync_positive",
            requested.hsync_positive.into(),
            readback.hsync_positive.into(),
        ),
        (
            "vsync_positive",
            requested.vsync_positive.into(),
            readback.vsync_positive.into(),
        ),
    ];
    pairs
        .iter()
        .filter(|(_, a, b)| a != b)
        .map(|(name, a, b)| format!("{name}: {a} -> {b}"))
        .collect()
}

const NAME_LEN: usize = 32;
const HZ_PER_KHZ: u32 = 1_000;
/// The aspect-ratio bits (`DRM_MODE_FLAG_PIC_AR_MASK`) say nothing about
/// timing; the kernel reports them only to clients that ask for them.
const FLAG_PIC_AR_MASK: u32 = 0x0f << 19;
const FLAGS_KNOWN: u32 = FLAG_PHSYNC
    | FLAG_NHSYNC
    | FLAG_PVSYNC
    | FLAG_NVSYNC
    | FLAG_INTERLACE
    | FLAG_DBLSCAN
    | FLAG_PIC_AR_MASK;

impl fmt::Display for ConvertError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConvertError::ClockNotWholeKhz { pclock_hz } => write!(
                f,
                "pixel clock {pclock_hz} Hz is not a whole number of kHz, as the kernel needs"
            ),
            ConvertError::ClockTooHigh { clock_khz } => {
                write!(f, "pixel clock {clock_khz} kHz does not fit in hertz")
            }
            ConvertError::Unsupported {
                flags,
                hskew,
                vscan,
            } => write!(
                f,
                "the mode has flags 0x{flags:x}, hskew {hskew} or vscan {vscan}, which a modeline cannot hold"
            ),
            ConvertError::Polarity { flags } => {
                write!(
                    f,
                    "the mode flags 0x{flags:x} state no single sync polarity"
                )
            }
        }
    }
}

impl std::error::Error for ConvertError {}

#[cfg(test)]
mod tests {
    use neptron_safety::validate;
    use neptron_types::{CrtRange, GpuCaps, MonitorKind, MonitorProfile};

    use super::*;

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

    fn profile() -> MonitorProfile {
        MonitorProfile {
            id: "test".to_owned(),
            kind: MonitorKind::Crt {
                ranges: vec![CrtRange {
                    h_min_mhz: 31_000_000,
                    h_max_mhz: 32_000_000,
                    v_min_mhz: 50_000,
                    v_max_mhz: 70_000,
                    h_front_porch_ns: 500,
                    h_sync_ns: 3_813,
                    h_back_porch_ns: 1_500,
                    v_front_porch_ns: 300_000,
                    v_sync_ns: 64_000,
                    v_back_porch_ns: 1_000_000,
                    hsync_positive: false,
                    vsync_positive: false,
                    progressive_lines_min: 400,
                    progressive_lines_max: 512,
                    interlaced_lines_min: 0,
                    interlaced_lines_max: 0,
                }],
            },
        }
    }

    const CAPS: GpuCaps = GpuCaps {
        pclock_min_hz: 1_000_000,
        pclock_max_hz: 400_000_000,
        interlace: true,
        flip_per_field: false,
        max_width: 4096,
        max_height: 4096,
    };

    #[test]
    fn the_record_has_the_kernel_layout() {
        assert_eq!(std::mem::size_of::<KernelMode>(), 68);
        assert_eq!(std::mem::align_of::<KernelMode>(), 4);
    }

    #[test]
    fn a_validated_mode_round_trips() {
        let v = validate(&vga(), &profile(), &CAPS).unwrap();
        let k = KernelMode::from_validated(&v, "480p").unwrap();
        assert_eq!(k.clock_khz, 25_175);
        assert_eq!(k.flags, FLAG_NHSYNC | FLAG_NVSYNC);
        assert_eq!(k.vrefresh, 60);
        assert_eq!(k.kind, TYPE_USERDEF);
        assert_eq!(&k.name[..5], b"480p\0");
        assert_eq!(k.raw().to_modeline().unwrap(), vga());
    }

    #[test]
    fn a_long_name_keeps_its_terminator() {
        let v = validate(&vga(), &profile(), &CAPS).unwrap();
        let k = KernelMode::from_validated(&v, &"x".repeat(40)).unwrap();
        assert_eq!(k.name[NAME_LEN - 1], 0);
        assert_eq!(k.name[NAME_LEN - 2], b'x');
    }

    #[test]
    fn a_clock_with_hertz_is_refused_not_rounded() {
        let m = Modeline {
            pclock_hz: 25_175_001,
            ..vga()
        };
        let v = validate(&m, &profile(), &CAPS).unwrap();
        assert_eq!(
            KernelMode::from_validated(&v, "x"),
            Err(ConvertError::ClockNotWholeKhz {
                pclock_hz: 25_175_001
            })
        );
    }

    fn raw(flags: u32) -> RawMode {
        RawMode {
            clock_khz: 13_500,
            h: [720, 739, 801, 858],
            v: [480, 488, 494, 525],
            hskew: 0,
            vscan: 0,
            flags,
        }
    }

    #[test]
    fn kernel_flags_map_to_the_modeline() {
        let m = raw(FLAG_PHSYNC | FLAG_PVSYNC | FLAG_INTERLACE)
            .to_modeline()
            .unwrap();
        assert!(m.interlace && m.hsync_positive && m.vsync_positive && !m.doublescan);
        assert_eq!(m.pclock_hz, 13_500_000);
        let ar = raw(FLAG_NHSYNC | FLAG_NVSYNC | (1 << 19)).to_modeline();
        assert!(ar.is_ok());
    }

    #[test]
    fn a_kernel_mode_a_modeline_cannot_hold_is_refused() {
        let csync = 1 << 6;
        assert!(matches!(
            raw(FLAG_NHSYNC | FLAG_NVSYNC | csync).to_modeline(),
            Err(ConvertError::Unsupported { flags: 0x40, .. })
        ));
        assert!(matches!(
            RawMode {
                hskew: 2,
                ..raw(FLAG_NHSYNC | FLAG_NVSYNC)
            }
            .to_modeline(),
            Err(ConvertError::Unsupported { hskew: 2, .. })
        ));
        assert!(matches!(
            raw(FLAG_NVSYNC).to_modeline(),
            Err(ConvertError::Polarity { .. })
        ));
        assert!(matches!(
            raw(FLAG_PHSYNC | FLAG_NHSYNC | FLAG_NVSYNC).to_modeline(),
            Err(ConvertError::Polarity { .. })
        ));
        assert!(matches!(
            RawMode {
                clock_khz: u32::MAX,
                ..raw(FLAG_NHSYNC | FLAG_NVSYNC)
            }
            .to_modeline(),
            Err(ConvertError::ClockTooHigh { .. })
        ));
    }

    #[test]
    fn differences_name_each_changed_field() {
        assert!(differences(&vga(), &vga()).is_empty());
        let other = Modeline {
            pclock_hz: 25_170_000,
            interlace: true,
            ..vga()
        };
        assert_eq!(
            differences(&vga(), &other),
            ["pclock_hz: 25175000 -> 25170000", "interlace: 0 -> 1"]
        );
    }
}
