//! Safe Switchres wrapper, monitor profiles and the ODREN policy: from source
//! geometry, profile and GPU capabilities to a list of candidate modelines.
//! No I/O.
//!
//! Switchres calculates the modelines. This crate converts the integer
//! contract types into its units and back, in one module, and ranks nothing
//! itself: the order of the list is the order of Switchres.

#![forbid(unsafe_code)]

mod candidate;
mod convert;
mod error;
mod settings;

pub use candidate::Candidate;
pub use error::{ConvertError, ModelineError};
pub use neptron_switchres_sys::BlankingFault;
pub use settings::{Aspect, Settings};

use neptron_switchres_sys as sys;
use neptron_types::{CrtRange, GpuCaps, MonitorKind, MonitorProfile, SourceGeometry};

/// The modelines that Switchres calculates for a source on a CRT profile,
/// with the default settings. See [`modelines_with`].
pub fn modelines(
    source: &SourceGeometry,
    profile: &MonitorProfile,
    caps: &GpuCaps,
) -> Result<Vec<Candidate>, ModelineError> {
    modelines_with(source, profile, caps, &Settings::default())
}

/// The modelines that Switchres calculates for a source on a CRT profile.
///
/// The list holds at most one candidate per range of the profile, best
/// first, in the order of Switchres. The first candidate is the mode that
/// Switchres selects. An empty list means that no range can show the source.
///
/// From the GPU, Switchres uses the lowest pixel clock and whether interlace
/// is allowed. It does not check the highest pixel clock or the largest size:
/// the safety validator judges every candidate. Switchres takes every source
/// as 4:3, so `aspect_milli` of the source is not used.
pub fn modelines_with(
    source: &SourceGeometry,
    profile: &MonitorProfile,
    caps: &GpuCaps,
    settings: &Settings,
) -> Result<Vec<Candidate>, ModelineError> {
    let ranges = crt_ranges(profile)?;
    check_source(source)?;
    check_settings(settings)?;

    let ranges: Vec<sys::Range> = ranges.iter().map(convert::range).collect();
    let modes = sys::modelines(
        &ranges,
        &convert::settings(settings, caps),
        &convert::request(source),
    )
    .map_err(|error| call_error(profile, error))?;

    modes
        .iter()
        .map(convert::candidate)
        .collect::<Result<_, _>>()
        .map_err(|error| ModelineError::Convert {
            profile: profile.id.clone(),
            error,
        })
}

fn crt_ranges(profile: &MonitorProfile) -> Result<&[CrtRange], ModelineError> {
    let ranges = match &profile.kind {
        MonitorKind::Crt { ranges } => ranges,
        MonitorKind::FlatPanel { .. } => {
            return Err(ModelineError::FlatPanel {
                profile: profile.id.clone(),
            });
        }
    };
    if ranges.is_empty() {
        return Err(ModelineError::NoRanges {
            profile: profile.id.clone(),
        });
    }
    if ranges.len() > sys::MAX_RANGES {
        return Err(ModelineError::TooManyRanges {
            profile: profile.id.clone(),
            count: ranges.len(),
            max: sys::MAX_RANGES,
        });
    }
    Ok(ranges)
}

fn check_source(source: &SourceGeometry) -> Result<(), ModelineError> {
    if source.width == 0 || source.height == 0 || source.refresh_mhz == 0 {
        return Err(ModelineError::EmptySource {
            width: source.width,
            height: source.height,
            refresh_mhz: source.refresh_mhz,
        });
    }
    Ok(())
}

fn check_settings(settings: &Settings) -> Result<(), ModelineError> {
    // From 1:4 to 4:1, the limits of neptron-switchres-sys, in integers:
    // the ratio of two whole numbers is then at least 0.25 and at most 4.0
    // in f64 as well, because the division rounds to the nearest f64.
    let Aspect { width, height } = settings.monitor_aspect;
    let (w, h) = (u32::from(width), u32::from(height));
    if w == 0 || h == 0 || 4 * w < h || w > 4 * h {
        return Err(ModelineError::AspectOutOfRange { width, height });
    }
    Ok(())
}

fn call_error(profile: &MonitorProfile, error: sys::CallError) -> ModelineError {
    let profile = profile.id.clone();
    match error {
        sys::CallError::InvalidRange { index } => ModelineError::InvalidRange { profile, index },
        sys::CallError::TooManyRanges { count, max } => ModelineError::TooManyRanges {
            profile,
            count,
            max,
        },
        sys::CallError::Blanking {
            index,
            fault,
            blanking,
            limit,
        } => ModelineError::Blanking {
            profile,
            index,
            fault,
            blanking_ns: convert::blanking_ns(fault, blanking),
            limit_ns: convert::blanking_ns(fault, limit),
        },
        error => ModelineError::Switchres { profile, error },
    }
}
