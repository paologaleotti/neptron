use neptron_types::{MonitorKind, MonitorProfile};

/// The signal of a console with a VGA cable and an interlaced cable
/// (Dreamcast, Naomi) that the monitor takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CableSignal {
    /// 640x480 progressive at 60 Hz: the VGA cable.
    Vga,
    /// 640x480 interlaced at 59.94 Hz: the cable for a TV.
    Interlaced,
}

/// The line rate of the VGA signal of the console, in millihertz: 525
/// lines at 59.94 Hz.
pub const VGA_LINE_RATE_MHZ: u64 = 31_469_000;

/// The cable that the console gets on `profile`, with no setting: VGA on a
/// flat panel and on a CRT with a range that covers
/// [`VGA_LINE_RATE_MHZ`] (a 31 kHz or a multi-sync monitor); the
/// interlaced signal on a TV and on a 15 kHz or 25 kHz CRT.
pub fn cable_signal(profile: &MonitorProfile) -> CableSignal {
    match &profile.kind {
        MonitorKind::FlatPanel { .. } => CableSignal::Vga,
        MonitorKind::Tv { .. } => CableSignal::Interlaced,
        MonitorKind::Crt { ranges }
            if ranges
                .iter()
                .any(|r| r.h_min_mhz <= VGA_LINE_RATE_MHZ && VGA_LINE_RATE_MHZ <= r.h_max_mhz) =>
        {
            CableSignal::Vga
        }
        MonitorKind::Crt { .. } => CableSignal::Interlaced,
    }
}
