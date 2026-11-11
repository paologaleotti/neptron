use neptron_types::{Rotation, SourceGeometry};

use super::PolicyError;

/// The source as the policy sees it: the axes after the rotation, so the
/// height is the number of lines the monitor must show.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OrientedSource {
    /// Width on the screen, in pixels.
    pub width: u16,
    /// Height on the screen, in lines. Both fields for an interlaced source.
    pub height: u16,
    /// Frame rate of the core, in millihertz.
    pub refresh_mhz: u32,
    /// The core draws interlaced frames.
    pub interlaced: bool,
    /// Display aspect on the screen, width divided by height, in
    /// thousandths. 0 when the core gives none.
    pub aspect_milli: u32,
    /// Width of the frame as the core draws it, in pixels.
    pub frame_width: u16,
    /// Height of the frame as the core draws it, in lines.
    pub frame_height: u16,
}

/// Step 1: take the source. A rotation of 90 or 270 degrees swaps the axes
/// and inverts the aspect, so the monitor gets the long side as lines.
pub(crate) fn take_source(source: &SourceGeometry) -> Result<OrientedSource, PolicyError> {
    if source.width == 0 || source.height == 0 || source.refresh_mhz == 0 {
        return Err(PolicyError::EmptySource {
            width: source.width,
            height: source.height,
            refresh_mhz: source.refresh_mhz,
        });
    }
    let turned = matches!(source.rotation, Rotation::R90 | Rotation::R270);
    let (width, height) = if turned {
        (source.height, source.width)
    } else {
        (source.width, source.height)
    };
    let aspect_milli = match source.aspect_milli {
        0 => 0,
        a if turned => div_round(1_000_000, u64::from(a)),
        a => a,
    };
    Ok(OrientedSource {
        width,
        height,
        refresh_mhz: source.refresh_mhz,
        interlaced: source.interlaced,
        aspect_milli,
        frame_width: source.width,
        frame_height: source.height,
    })
}

/// `numerator / denominator` rounded to the nearest integer, a half up,
/// saturated to `u32`. The callers never pass a zero denominator.
pub(crate) fn div_round(numerator: u64, denominator: u64) -> u32 {
    let quotient = (numerator + denominator / 2) / denominator.max(1);
    u32::try_from(quotient).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn geometry(width: u16, height: u16, rotation: Rotation) -> SourceGeometry {
        SourceGeometry {
            width,
            height,
            refresh_mhz: 60_000,
            aspect_milli: 1_333,
            interlaced: false,
            rotation,
        }
    }

    #[test]
    fn an_upright_source_keeps_its_axes() {
        let s = take_source(&geometry(256, 224, Rotation::R0)).unwrap();
        assert_eq!((s.width, s.height, s.aspect_milli), (256, 224, 1_333));
        let s = take_source(&geometry(256, 224, Rotation::R180)).unwrap();
        assert_eq!((s.width, s.height), (256, 224));
    }

    #[test]
    fn a_turned_source_swaps_its_axes_and_inverts_its_aspect() {
        for rotation in [Rotation::R90, Rotation::R270] {
            let s = take_source(&geometry(288, 224, rotation)).unwrap();
            assert_eq!((s.width, s.height, s.aspect_milli), (224, 288, 750));
            assert_eq!((s.frame_width, s.frame_height), (288, 224));
        }
    }

    #[test]
    fn an_empty_source_is_an_error() {
        for (w, h, r) in [(0, 224, 60_000), (256, 0, 60_000), (256, 224, 0)] {
            let source = SourceGeometry {
                refresh_mhz: r,
                ..geometry(w, h, Rotation::R0)
            };
            assert_eq!(
                take_source(&source),
                Err(PolicyError::EmptySource {
                    width: w,
                    height: h,
                    refresh_mhz: r
                })
            );
        }
    }
}
