use crate::ffi::{RawRange, RawRequest, RawSettings};

/// One `crt_range` line in the units of `switchres.ini`: frequencies in
/// hertz, horizontal times in microseconds, vertical times in milliseconds.
///
/// Every value must be finite; `monitor_evaluate_range` of Switchres checks
/// the rest.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Range {
    /// Lowest line rate, in hertz.
    pub hfreq_min_hz: f64,
    /// Highest line rate, in hertz.
    pub hfreq_max_hz: f64,
    /// Lowest field rate, in hertz.
    pub vfreq_min_hz: f64,
    /// Highest field rate, in hertz.
    pub vfreq_max_hz: f64,
    /// Horizontal front porch, in microseconds.
    pub hfront_porch_us: f64,
    /// Horizontal sync pulse, in microseconds.
    pub hsync_pulse_us: f64,
    /// Horizontal back porch, in microseconds.
    pub hback_porch_us: f64,
    /// Vertical front porch, in milliseconds.
    pub vfront_porch_ms: f64,
    /// Vertical sync pulse, in milliseconds.
    pub vsync_pulse_ms: f64,
    /// Vertical back porch, in milliseconds.
    pub vback_porch_ms: f64,
    /// The horizontal sync pulse is positive.
    pub hsync_positive: bool,
    /// The vertical sync pulse is positive.
    pub vsync_positive: bool,
    /// Fewest active lines of a progressive mode.
    pub progressive_lines_min: u16,
    /// Most active lines of a progressive mode.
    pub progressive_lines_max: u16,
    /// Fewest active lines of an interlaced mode; 0 for none.
    pub interlaced_lines_min: u16,
    /// Most active lines of an interlaced mode; 0 for none.
    pub interlaced_lines_max: u16,
}

/// The generator settings of Switchres that NEPTRON sets. The shim keeps
/// the geometry adjustment and the vertical shift correction off.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Settings {
    /// Lowest pixel clock, in hertz: `dotclock_min` times 1,000,000.
    pub pclock_min_hz: u32,
    /// Width of the screen divided by its height, from
    /// [`MONITOR_ASPECT_MIN`](crate::MONITOR_ASPECT_MIN) to
    /// [`MONITOR_ASPECT_MAX`](crate::MONITOR_ASPECT_MAX).
    pub monitor_aspect: f64,
    /// Largest refresh difference that is not "off", in hertz. Switchres
    /// only compares with it, so every value is safe.
    pub refresh_tolerance_hz: f64,
    /// Interlaced modes are allowed.
    pub interlace: bool,
    /// Doublescan modes are allowed.
    pub doublescan: bool,
    /// Width from which Switchres treats a mode as a super width.
    pub super_width: u16,
    /// The width goes in steps of 1 pixel, not 8.
    pub pixel_precision: bool,
    /// The vertical values of an interlaced mode round to even.
    pub interlace_force_even: bool,
    /// The width scales by the same factor as the height.
    pub scale_proportional: bool,
}

/// The arguments of `display_manager::get_mode`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Request {
    /// Refresh of the source, in hertz, from
    /// [`REFRESH_MIN_HZ`](crate::REFRESH_MIN_HZ) to
    /// [`REFRESH_MAX_HZ`](crate::REFRESH_MAX_HZ).
    pub refresh_hz: f64,
    /// Width of the source, in pixels; above 0.
    pub width: u16,
    /// Height of the source, in lines; above 0.
    pub height: u16,
    /// The source is interlaced.
    pub interlaced: bool,
    /// The source is rotated by 90 or 270 degrees.
    pub rotated: bool,
}

impl Range {
    pub(crate) fn raw(&self) -> RawRange {
        RawRange {
            hfreq_min_hz: self.hfreq_min_hz,
            hfreq_max_hz: self.hfreq_max_hz,
            vfreq_min_hz: self.vfreq_min_hz,
            vfreq_max_hz: self.vfreq_max_hz,
            hfront_porch_us: self.hfront_porch_us,
            hsync_pulse_us: self.hsync_pulse_us,
            hback_porch_us: self.hback_porch_us,
            vfront_porch_ms: self.vfront_porch_ms,
            vsync_pulse_ms: self.vsync_pulse_ms,
            vback_porch_ms: self.vback_porch_ms,
            hsync_positive: i32::from(self.hsync_positive),
            vsync_positive: i32::from(self.vsync_positive),
            progressive_lines_min: i32::from(self.progressive_lines_min),
            progressive_lines_max: i32::from(self.progressive_lines_max),
            interlaced_lines_min: i32::from(self.interlaced_lines_min),
            interlaced_lines_max: i32::from(self.interlaced_lines_max),
        }
    }
}

impl Settings {
    pub(crate) fn raw(&self) -> RawSettings {
        RawSettings {
            pclock_min_hz: u64::from(self.pclock_min_hz),
            monitor_aspect: self.monitor_aspect,
            refresh_tolerance_hz: self.refresh_tolerance_hz,
            interlace: i32::from(self.interlace),
            doublescan: i32::from(self.doublescan),
            super_width: i32::from(self.super_width),
            pixel_precision: i32::from(self.pixel_precision),
            interlace_force_even: i32::from(self.interlace_force_even),
            scale_proportional: i32::from(self.scale_proportional),
        }
    }
}

impl Request {
    pub(crate) fn raw(&self) -> RawRequest {
        RawRequest {
            refresh_hz: self.refresh_hz,
            width: i32::from(self.width),
            height: i32::from(self.height),
            interlaced: i32::from(self.interlaced),
            rotated: i32::from(self.rotated),
        }
    }
}
