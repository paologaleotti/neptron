//! The raw declarations of `shim/neptron_switchres.h`.

use core::mem::offset_of;

/// Most ranges in one call: `MAX_RANGES` of Switchres.
pub const MAX_RANGES: usize = 10;

/// Result bit: the refresh differs from the request by more than the
/// tolerance.
pub const WEIGHT_REFRESH_OFF: i32 = 0x1;
/// Result bit: the source does not scale by a whole number into the mode.
pub const WEIGHT_STRETCHED: i32 = 0x2;
/// Result bit: the range cannot show the request.
pub const WEIGHT_OUT_OF_RANGE: i32 = 0x4;

pub(crate) const NSR_OK: i32 = 0;
pub(crate) const NSR_BAD_RANGE: i32 = 2;
pub(crate) const NSR_EXCEPTION: i32 = 3;
pub(crate) const NSR_BAD_REQUEST: i32 = 4;
pub(crate) const NSR_BAD_SETTINGS: i32 = 5;

/// Lowest refresh of a request, in hertz: `NSR_REFRESH_MIN_HZ` of the shim.
pub const REFRESH_MIN_HZ: f64 = 0.001;
/// Highest refresh of a request, in hertz: `NSR_REFRESH_MAX_HZ` of the shim.
pub const REFRESH_MAX_HZ: f64 = 10_000_000.0;
/// Narrowest monitor aspect (width / height): `NSR_MONITOR_ASPECT_MIN`.
pub const MONITOR_ASPECT_MIN: f64 = 0.25;
/// Widest monitor aspect (width / height): `NSR_MONITOR_ASPECT_MAX`.
pub const MONITOR_ASPECT_MAX: f64 = 4.0;

/// `nsr_range`. The public `Range` fills it.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) struct RawRange {
    pub(crate) hfreq_min_hz: f64,
    pub(crate) hfreq_max_hz: f64,
    pub(crate) vfreq_min_hz: f64,
    pub(crate) vfreq_max_hz: f64,
    pub(crate) hfront_porch_us: f64,
    pub(crate) hsync_pulse_us: f64,
    pub(crate) hback_porch_us: f64,
    pub(crate) vfront_porch_ms: f64,
    pub(crate) vsync_pulse_ms: f64,
    pub(crate) vback_porch_ms: f64,
    pub(crate) hsync_positive: i32,
    pub(crate) vsync_positive: i32,
    pub(crate) progressive_lines_min: i32,
    pub(crate) progressive_lines_max: i32,
    pub(crate) interlaced_lines_min: i32,
    pub(crate) interlaced_lines_max: i32,
}

/// `nsr_settings`. The public `Settings` fills it.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) struct RawSettings {
    pub(crate) pclock_min_hz: u64,
    pub(crate) monitor_aspect: f64,
    pub(crate) refresh_tolerance_hz: f64,
    pub(crate) interlace: i32,
    pub(crate) doublescan: i32,
    pub(crate) super_width: i32,
    pub(crate) pixel_precision: i32,
    pub(crate) interlace_force_even: i32,
    pub(crate) scale_proportional: i32,
}

/// `nsr_request`. The public `Request` fills it.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) struct RawRequest {
    pub(crate) refresh_hz: f64,
    pub(crate) width: i32,
    pub(crate) height: i32,
    pub(crate) interlaced: i32,
    pub(crate) rotated: i32,
}

/// One calculated modeline with its score, as Switchres gives it.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Mode {
    /// Pixel clock, in hertz.
    pub pclock_hz: u64,
    /// Horizontal scale of the source into the mode.
    pub x_scale: f64,
    /// Vertical scale of the source into the mode.
    pub y_scale: f64,
    /// Refresh of the mode divided by the refresh of the source.
    pub v_scale: f64,
    /// Horizontal aspect error, in percent.
    pub x_diff: f64,
    /// Vertical border, in percent.
    pub y_diff: f64,
    /// Refresh of the mode minus the refresh of the source, in hertz.
    pub v_diff_hz: f64,
    /// Visible pixels per line.
    pub hactive: i32,
    /// Pixel at which the horizontal sync starts.
    pub hbegin: i32,
    /// Pixel at which the horizontal sync ends.
    pub hend: i32,
    /// Pixels per line.
    pub htotal: i32,
    /// Visible lines.
    pub vactive: i32,
    /// Line at which the vertical sync starts.
    pub vbegin: i32,
    /// Line at which the vertical sync ends.
    pub vend: i32,
    /// Lines per frame.
    pub vtotal: i32,
    /// 1 for an interlaced mode.
    pub interlace: i32,
    /// 1 for a doublescan mode.
    pub doublescan: i32,
    /// 1 for a positive horizontal sync.
    pub hsync_positive: i32,
    /// 1 for a positive vertical sync.
    pub vsync_positive: i32,
    /// The `WEIGHT_` result bits.
    pub weight: i32,
    /// Index of the range that made the mode.
    pub range_index: i32,
}

// The same numbers as the static_assert lines in the shim: the size and the
// offset of every field, so a change or a swap on one side fails the build.
const _: () = assert!(size_of::<RawRange>() == 104);
const _: () = assert!(offset_of!(RawRange, hfreq_min_hz) == 0);
const _: () = assert!(offset_of!(RawRange, hfreq_max_hz) == 8);
const _: () = assert!(offset_of!(RawRange, vfreq_min_hz) == 16);
const _: () = assert!(offset_of!(RawRange, vfreq_max_hz) == 24);
const _: () = assert!(offset_of!(RawRange, hfront_porch_us) == 32);
const _: () = assert!(offset_of!(RawRange, hsync_pulse_us) == 40);
const _: () = assert!(offset_of!(RawRange, hback_porch_us) == 48);
const _: () = assert!(offset_of!(RawRange, vfront_porch_ms) == 56);
const _: () = assert!(offset_of!(RawRange, vsync_pulse_ms) == 64);
const _: () = assert!(offset_of!(RawRange, vback_porch_ms) == 72);
const _: () = assert!(offset_of!(RawRange, hsync_positive) == 80);
const _: () = assert!(offset_of!(RawRange, vsync_positive) == 84);
const _: () = assert!(offset_of!(RawRange, progressive_lines_min) == 88);
const _: () = assert!(offset_of!(RawRange, progressive_lines_max) == 92);
const _: () = assert!(offset_of!(RawRange, interlaced_lines_min) == 96);
const _: () = assert!(offset_of!(RawRange, interlaced_lines_max) == 100);
const _: () = assert!(size_of::<RawSettings>() == 48);
const _: () = assert!(offset_of!(RawSettings, pclock_min_hz) == 0);
const _: () = assert!(offset_of!(RawSettings, monitor_aspect) == 8);
const _: () = assert!(offset_of!(RawSettings, refresh_tolerance_hz) == 16);
const _: () = assert!(offset_of!(RawSettings, interlace) == 24);
const _: () = assert!(offset_of!(RawSettings, doublescan) == 28);
const _: () = assert!(offset_of!(RawSettings, super_width) == 32);
const _: () = assert!(offset_of!(RawSettings, pixel_precision) == 36);
const _: () = assert!(offset_of!(RawSettings, interlace_force_even) == 40);
const _: () = assert!(offset_of!(RawSettings, scale_proportional) == 44);
const _: () = assert!(size_of::<RawRequest>() == 24);
const _: () = assert!(offset_of!(RawRequest, refresh_hz) == 0);
const _: () = assert!(offset_of!(RawRequest, width) == 8);
const _: () = assert!(offset_of!(RawRequest, height) == 12);
const _: () = assert!(offset_of!(RawRequest, interlaced) == 16);
const _: () = assert!(offset_of!(RawRequest, rotated) == 20);
const _: () = assert!(size_of::<Mode>() == 112);
const _: () = assert!(offset_of!(Mode, pclock_hz) == 0);
const _: () = assert!(offset_of!(Mode, x_scale) == 8);
const _: () = assert!(offset_of!(Mode, y_scale) == 16);
const _: () = assert!(offset_of!(Mode, v_scale) == 24);
const _: () = assert!(offset_of!(Mode, x_diff) == 32);
const _: () = assert!(offset_of!(Mode, y_diff) == 40);
const _: () = assert!(offset_of!(Mode, v_diff_hz) == 48);
const _: () = assert!(offset_of!(Mode, hactive) == 56);
const _: () = assert!(offset_of!(Mode, hbegin) == 60);
const _: () = assert!(offset_of!(Mode, hend) == 64);
const _: () = assert!(offset_of!(Mode, htotal) == 68);
const _: () = assert!(offset_of!(Mode, vactive) == 72);
const _: () = assert!(offset_of!(Mode, vbegin) == 76);
const _: () = assert!(offset_of!(Mode, vend) == 80);
const _: () = assert!(offset_of!(Mode, vtotal) == 84);
const _: () = assert!(offset_of!(Mode, interlace) == 88);
const _: () = assert!(offset_of!(Mode, doublescan) == 92);
const _: () = assert!(offset_of!(Mode, hsync_positive) == 96);
const _: () = assert!(offset_of!(Mode, vsync_positive) == 100);
const _: () = assert!(offset_of!(Mode, weight) == 104);
const _: () = assert!(offset_of!(Mode, range_index) == 108);

unsafe extern "C" {
    pub(crate) fn nsr_modelines(
        ranges: *const RawRange,
        range_count: usize,
        settings: *const RawSettings,
        request: *const RawRequest,
        out: *mut Mode,
        out_capacity: usize,
        out_count: *mut usize,
        bad_range: *mut usize,
    ) -> i32;
}
