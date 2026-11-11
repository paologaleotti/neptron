//! The Switchres C++ build and the raw FFI calls, and nothing else.
//! `neptron-modeline` is its safe wrapper.
//!
//! The values here are in the units and the floating-point types of
//! Switchres. The conversion to the integer contract types is in
//! `neptron-modeline`.
//!
//! The public input types hold no value that the C++ cannot take: sizes and
//! line counts are `u16`, the pixel clock floor is `u32`, and the flags are
//! `bool`. The shim checks what a type cannot express: finite range values,
//! and the limits of the refresh and of the monitor aspect.

mod ffi;
mod types;

use std::fmt;
use std::sync::{Mutex, PoisonError};

pub use ffi::{
    MAX_RANGES, MONITOR_ASPECT_MAX, MONITOR_ASPECT_MIN, Mode, REFRESH_MAX_HZ, REFRESH_MIN_HZ,
    WEIGHT_OUT_OF_RANGE, WEIGHT_REFRESH_OFF, WEIGHT_STRETCHED,
};
pub use types::{Range, Request, Settings};

/// Why a call into Switchres gave no list.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CallError {
    /// More ranges than Switchres holds.
    TooManyRanges {
        /// Ranges in the call.
        count: usize,
        /// Most ranges Switchres holds.
        max: usize,
    },
    /// The request has a zero size, or a refresh outside
    /// [`REFRESH_MIN_HZ`] to [`REFRESH_MAX_HZ`].
    InvalidRequest {
        /// Width of the source.
        width: u16,
        /// Height of the source.
        height: u16,
        /// Refresh of the source, in hertz.
        refresh_hz: f64,
    },
    /// The monitor aspect is outside [`MONITOR_ASPECT_MIN`] to
    /// [`MONITOR_ASPECT_MAX`].
    InvalidSettings {
        /// The monitor aspect.
        monitor_aspect: f64,
    },
    /// A range has a value that is not finite, or `monitor_evaluate_range`
    /// of Switchres refuses it.
    InvalidRange {
        /// Index of the range in the call.
        index: usize,
    },
    /// The C++ code threw an exception, and the shim caught it.
    Exception,
    /// The shim returned a status or a count that this crate does not know.
    UnknownStatus {
        /// The status code.
        status: i32,
        /// The count of modes.
        count: usize,
    },
}

/// Calculates one modeline per range with Switchres and returns the ones
/// in range, best first, in the order of Switchres.
///
/// The first mode is the mode that `display_manager::get_mode` of
/// Switchres returns for a display that can add a mode. An empty list
/// means that no range can show the request.
///
/// Calls are serialised behind one lock: Switchres keeps its log functions
/// in global variables.
pub fn modelines(
    ranges: &[Range],
    settings: &Settings,
    request: &Request,
) -> Result<Vec<Mode>, CallError> {
    if ranges.len() > MAX_RANGES {
        return Err(CallError::TooManyRanges {
            count: ranges.len(),
            max: MAX_RANGES,
        });
    }
    let raw_ranges: Vec<ffi::RawRange> = ranges.iter().map(Range::raw).collect();
    let raw_settings = settings.raw();
    let raw_request = request.raw();
    let mut out = [Mode::default(); MAX_RANGES];
    let mut count = 0usize;
    let mut bad_range = 0usize;

    let status = {
        let _guard = SWITCHRES.lock().unwrap_or_else(PoisonError::into_inner);
        // SAFETY: every pointer comes from a live reference or a local, and
        // stays valid for the call. `raw_ranges` holds `raw_ranges.len()`
        // elements and `out` holds `MAX_RANGES` elements, the capacity
        // passed. The types are `#[repr(C)]` copies of the shim structs, and
        // the layout asserts on both sides check the size and every offset.
        // The public types keep each integer in the range the C++ takes,
        // and the shim checks the doubles. The shim catches every
        // C++ exception, so no unwind crosses the boundary. The lock makes
        // the call the only one in Switchres at this time.
        unsafe {
            ffi::nsr_modelines(
                raw_ranges.as_ptr(),
                raw_ranges.len(),
                &raw_settings,
                &raw_request,
                out.as_mut_ptr(),
                out.len(),
                &mut count,
                &mut bad_range,
            )
        }
    };

    match status {
        ffi::NSR_OK => out
            .get(..count)
            .map(<[Mode]>::to_vec)
            .ok_or(CallError::UnknownStatus { status, count }),
        ffi::NSR_BAD_REQUEST => Err(CallError::InvalidRequest {
            width: request.width,
            height: request.height,
            refresh_hz: request.refresh_hz,
        }),
        ffi::NSR_BAD_SETTINGS => Err(CallError::InvalidSettings {
            monitor_aspect: settings.monitor_aspect,
        }),
        ffi::NSR_BAD_RANGE => Err(CallError::InvalidRange { index: bad_range }),
        ffi::NSR_EXCEPTION => Err(CallError::Exception),
        _ => Err(CallError::UnknownStatus { status, count }),
    }
}

impl fmt::Display for CallError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooManyRanges { count, max } => {
                write!(f, "{count} ranges, but Switchres holds at most {max}")
            }
            Self::InvalidRequest {
                width,
                height,
                refresh_hz,
            } => write!(
                f,
                "request {width}x{height} at {refresh_hz} Hz: the size must be above 0 and \
                 the refresh from {REFRESH_MIN_HZ} to {REFRESH_MAX_HZ} Hz"
            ),
            Self::InvalidSettings { monitor_aspect } => write!(
                f,
                "monitor aspect {monitor_aspect}: must be from {MONITOR_ASPECT_MIN} to \
                 {MONITOR_ASPECT_MAX}"
            ),
            Self::InvalidRange { index } => write!(f, "Switchres refuses range {index}"),
            Self::Exception => write!(f, "Switchres threw a C++ exception"),
            Self::UnknownStatus { status, count } => {
                write!(f, "unknown shim result: status {status}, {count} modes")
            }
        }
    }
}

impl std::error::Error for CallError {}

static SWITCHRES: Mutex<()> = Mutex::new(());
