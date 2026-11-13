use neptron_safety::ValidatedMode;
use neptron_types::Modeline;

/// The fields of the kernel's mode record (`struct drm_mode_modeinfo`).
///
/// [`ModeRecord::from_validated`] is the only way to make one for the
/// kernel, so only a [`ValidatedMode`] reaches it. A record read back from
/// the kernel goes the other way with [`ModeRecord::to_modeline`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModeRecord {
    /// Pixel clock, in kHz.
    pub clock_khz: u32,
    /// `hdisplay`, `hsync_start`, `hsync_end`, `htotal`.
    pub h: [u16; 4],
    /// `vdisplay`, `vsync_start`, `vsync_end`, `vtotal`.
    pub v: [u16; 4],
    /// `hskew`.
    pub hskew: u16,
    /// `vscan`.
    pub vscan: u16,
    /// The field rate rounded to whole hertz, for logs of the kernel.
    pub vrefresh_hz: u32,
    /// The `DRM_MODE_FLAG_*` bits.
    pub flags: u32,
    /// The `DRM_MODE_TYPE_*` bits.
    pub kind: u32,
    /// The name, zero-terminated.
    pub name: [u8; NAME_LEN],
}

/// A mode that cannot go to the kernel, or a kernel mode that is not a
/// [`Modeline`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ModeRecordError {
    /// The kernel takes the clock in kHz, and rounding would program a mode
    /// that did not pass validation.
    #[error("the pixel clock {pclock_hz} Hz is not a whole number of kHz")]
    ClockNotWholeKhz {
        /// The pixel clock, in hertz.
        pclock_hz: u32,
    },
    /// A clock read back from the kernel does not fit 32 bits in hertz.
    #[error("the pixel clock {clock_khz} kHz does not fit 32 bits in Hz")]
    ClockTooHigh {
        /// The pixel clock, in kHz.
        clock_khz: u32,
    },
    /// The kernel mode has flags or fields that a `Modeline` cannot hold.
    #[error(
        "the kernel mode has flags {flags:#x}, hskew {hskew} or vscan {vscan} that a modeline cannot hold"
    )]
    Unsupported {
        /// The flag bits that a `Modeline` cannot hold.
        flags: u32,
        /// `hskew` of the mode.
        hskew: u16,
        /// `vscan` of the mode.
        vscan: u16,
    },
    /// A sync polarity is not stated, or stated both ways.
    #[error("the kernel mode flags {flags:#x} do not state each sync polarity once")]
    Polarity {
        /// The flag bits of the mode.
        flags: u32,
    },
}

/// Length of the name field, terminator included.
pub const NAME_LEN: usize = 32;
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

impl ModeRecord {
    /// The kernel record of a validated mode.
    ///
    /// # Errors
    ///
    /// [`ModeRecordError::ClockNotWholeKhz`] when the clock is not a whole
    /// number of kHz.
    pub fn from_validated(mode: &ValidatedMode) -> Result<ModeRecord, ModeRecordError> {
        let m = mode.mode();
        if !m.pclock_is_whole_khz() {
            return Err(ModeRecordError::ClockNotWholeKhz {
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
        Ok(ModeRecord {
            clock_khz: m.pclock_hz / HZ_PER_KHZ,
            h: [m.hactive, m.hsync_start, m.hsync_end, m.htotal],
            v: [m.vactive, m.vsync_start, m.vsync_end, m.vtotal],
            hskew: 0,
            vscan: 0,
            vrefresh_hz: u32::try_from((m.field_rate_mhz() + MHZ_PER_HZ / 2) / MHZ_PER_HZ)
                .unwrap_or(u32::MAX),
            flags,
            kind: TYPE_USERDEF,
            name: name_of(m),
        })
    }

    /// The modeline of a kernel mode. A mode with a flag or a field that a
    /// `Modeline` cannot hold is an error, so that it is never read as a
    /// different mode. The aspect-ratio bits say nothing about timing and
    /// are ignored.
    ///
    /// # Errors
    ///
    /// [`ModeRecordError`] for a mode that is not a `Modeline`.
    pub fn to_modeline(&self) -> Result<Modeline, ModeRecordError> {
        let unknown = self.flags & !FLAGS_KNOWN;
        if unknown != 0 || self.hskew != 0 || self.vscan > 1 {
            return Err(ModeRecordError::Unsupported {
                flags: unknown,
                hskew: self.hskew,
                vscan: self.vscan,
            });
        }
        let polarity = |pos: u32, neg: u32| match (self.flags & pos != 0, self.flags & neg != 0) {
            (true, false) => Ok(true),
            (false, true) => Ok(false),
            _ => Err(ModeRecordError::Polarity { flags: self.flags }),
        };
        let pclock_hz =
            self.clock_khz
                .checked_mul(HZ_PER_KHZ)
                .ok_or(ModeRecordError::ClockTooHigh {
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

const HZ_PER_KHZ: u32 = 1_000;
const MHZ_PER_HZ: u64 = 1_000;
/// `DRM_MODE_FLAG_PIC_AR_MASK`: the kernel reports the aspect-ratio bits
/// only to clients that ask for them.
const FLAG_PIC_AR_MASK: u32 = 0x0f << 19;
const FLAGS_KNOWN: u32 = FLAG_PHSYNC
    | FLAG_NHSYNC
    | FLAG_PVSYNC
    | FLAG_NVSYNC
    | FLAG_INTERLACE
    | FLAG_DBLSCAN
    | FLAG_PIC_AR_MASK;

/// `640x480`, `640x480i` or `320x240d`: the size and the scan.
fn name_of(m: &Modeline) -> [u8; NAME_LEN] {
    let scan = match (m.interlace, m.doublescan) {
        (true, _) => "i",
        (false, true) => "d",
        (false, false) => "",
    };
    let text = format!("{}x{}{scan}", m.hactive, m.vactive);
    let mut out = [0; NAME_LEN];
    for (dst, src) in out.iter_mut().zip(text.bytes().take(NAME_LEN - 1)) {
        *dst = src;
    }
    out
}

#[cfg(test)]
mod tests {
    use neptron_safety::validate;
    use neptron_types::{
        GpuCaps, MonitorKind, MonitorProfile, TV_525_240P, TV_625_288P, TvStandard,
    };

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

    fn validated(mode: Modeline) -> ValidatedMode {
        let caps = GpuCaps {
            pclock_min_hz: 1_000_000,
            pclock_max_hz: 400_000_000,
            interlace: true,
            flip_per_field: false,
            max_width: 4096,
            max_height: 4096,
        };
        let panel = MonitorProfile {
            id: "panel".to_owned(),
            kind: MonitorKind::FlatPanel { modes: vec![mode] },
        };
        validate(&mode, &panel, &caps).unwrap()
    }

    #[test]
    fn a_validated_mode_goes_to_the_kernel_unchanged() {
        let r = ModeRecord::from_validated(&validated(vga())).unwrap();
        assert_eq!(r.clock_khz, 25_175);
        assert_eq!(r.h, [640, 656, 752, 800]);
        assert_eq!(r.v, [480, 490, 492, 525]);
        assert_eq!(r.flags, FLAG_NHSYNC | FLAG_NVSYNC);
        assert_eq!(r.kind, TYPE_USERDEF);
        assert_eq!(r.vrefresh_hz, 60);
        assert_eq!(&r.name[..8], b"640x480\0");
        assert_eq!(r.to_modeline(), Ok(vga()));
    }

    #[test]
    fn flags_follow_the_modeline() {
        let m = Modeline {
            interlace: true,
            hsync_positive: true,
            vsync_positive: true,
            ..vga()
        };
        let r = ModeRecord::from_validated(&validated(m)).unwrap();
        assert_eq!(r.flags, FLAG_PHSYNC | FLAG_PVSYNC | FLAG_INTERLACE);
        assert_eq!(&r.name[..9], b"640x480i\0");
        assert_eq!(r.to_modeline(), Ok(m));
        let d = Modeline {
            doublescan: true,
            ..vga()
        };
        let r = ModeRecord::from_validated(&validated(d)).unwrap();
        assert_eq!(r.flags & FLAG_DBLSCAN, FLAG_DBLSCAN);
        assert_eq!(r.to_modeline(), Ok(d));
    }

    #[test]
    fn a_clock_that_is_not_whole_khz_is_refused_not_rounded() {
        let m = Modeline {
            pclock_hz: 25_175_500,
            ..vga()
        };
        assert_eq!(
            ModeRecord::from_validated(&validated(m)),
            Err(ModeRecordError::ClockNotWholeKhz {
                pclock_hz: 25_175_500
            })
        );
    }

    fn record() -> ModeRecord {
        ModeRecord::from_validated(&validated(vga())).unwrap()
    }

    #[test]
    fn the_readback_refuses_what_a_modeline_cannot_hold() {
        let csync = ModeRecord {
            flags: record().flags | (1 << 6),
            ..record()
        };
        assert_eq!(
            csync.to_modeline(),
            Err(ModeRecordError::Unsupported {
                flags: 1 << 6,
                hskew: 0,
                vscan: 0
            })
        );
        let skew = ModeRecord {
            hskew: 2,
            ..record()
        };
        assert!(matches!(
            skew.to_modeline(),
            Err(ModeRecordError::Unsupported { hskew: 2, .. })
        ));
        let vscan = ModeRecord {
            vscan: 2,
            ..record()
        };
        assert!(matches!(
            vscan.to_modeline(),
            Err(ModeRecordError::Unsupported { vscan: 2, .. })
        ));
        let one = ModeRecord {
            vscan: 1,
            ..record()
        };
        assert_eq!(one.to_modeline(), Ok(vga()));
    }

    #[test]
    fn the_readback_needs_each_polarity_once() {
        for flags in [FLAG_NVSYNC, FLAG_PHSYNC | FLAG_NHSYNC | FLAG_NVSYNC] {
            let r = ModeRecord { flags, ..record() };
            assert_eq!(r.to_modeline(), Err(ModeRecordError::Polarity { flags }));
        }
    }

    #[test]
    fn the_readback_ignores_the_aspect_ratio_and_checks_the_clock() {
        let aspect = ModeRecord {
            flags: record().flags | (1 << 19),
            ..record()
        };
        assert_eq!(aspect.to_modeline(), Ok(vga()));
        let huge = ModeRecord {
            clock_khz: u32::MAX,
            ..record()
        };
        assert_eq!(
            huge.to_modeline(),
            Err(ModeRecordError::ClockTooHigh {
                clock_khz: u32::MAX
            })
        );
    }

    fn tv_validated(mode: Modeline, standard: TvStandard) -> ValidatedMode {
        let caps = GpuCaps {
            pclock_min_hz: 1_000_000,
            pclock_max_hz: 400_000_000,
            interlace: true,
            flip_per_field: false,
            max_width: 4096,
            max_height: 4096,
        };
        validate(&mode, &standard.profile(), &caps).unwrap()
    }

    /// The CRTC mode read back after a modeset is the blob of the request,
    /// flags included: `drm_mode_getcrtc` converts `crtc->state->mode`,
    /// which `drm_atomic_set_mode_prop_for_crtc` made from the blob, and
    /// the vc4 encoder check only reads the adjusted mode. So each table
    /// mode reads back with its negative sync flags.
    #[test]
    fn each_tv_table_mode_reads_back_unchanged() {
        for standard in TvStandard::ALL {
            for mode in standard.modes() {
                let r = ModeRecord::from_validated(&tv_validated(mode, standard)).unwrap();
                assert_eq!(r.clock_khz, 13_500);
                let scan = if mode.interlace { FLAG_INTERLACE } else { 0 };
                assert_eq!(r.flags, FLAG_NHSYNC | FLAG_NVSYNC | scan);
                assert_eq!(r.to_modeline(), Ok(mode), "{standard:?} {mode:?}");
            }
        }
    }

    /// The kernel lists `720x240` and `720x288` (`drm_mode_240p` and
    /// `drm_mode_288p` of the vc4 encoder) with no sync flag. Such a mode
    /// is never read as a table mode: the readback refuses it.
    #[test]
    fn a_kernel_listed_tv_mode_without_sync_flags_is_not_read_as_a_table_mode() {
        // `DRM_MODE_TYPE_DRIVER`
        const TYPE_DRIVER: u32 = 1 << 6;
        for (mode, standard) in [
            (TV_525_240P, TvStandard::Ntsc),
            (TV_625_288P, TvStandard::Pal),
        ] {
            let listed = ModeRecord {
                flags: 0,
                kind: TYPE_DRIVER,
                ..ModeRecord::from_validated(&tv_validated(mode, standard)).unwrap()
            };
            assert_eq!(
                listed.to_modeline(),
                Err(ModeRecordError::Polarity { flags: 0 })
            );
        }
    }

    #[test]
    fn a_long_name_is_cut_and_terminated() {
        let m = Modeline {
            hactive: u16::MAX,
            vactive: u16::MAX,
            ..vga()
        };
        let name = name_of(&m);
        assert_eq!(&name[..12], b"65535x65535\0");
        assert_eq!(name[NAME_LEN - 1], 0);
    }
}
