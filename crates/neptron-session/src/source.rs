//! The source geometry of a core frame, and the settle at launch.

use neptron_libretro::{GameGeometry, Timing};
use neptron_types::{Rotation, SourceGeometry};

/// A frame with more lines than this is interlaced content: two fields per
/// frame. 288 lines is the tallest progressive picture of a console (PAL).
/// The cores do not say whether a frame is interlaced, so the line count
/// decides.
pub const INTERLACE_MIN_LINES: u16 = 289;

/// At launch, the source must stay the same for this many core frames
/// before the first mode is set. Genesis Plus GX reports 256x192 at load
/// and sends 320x224 from its fifth frame; six equal frames cover a change
/// up to the sixth frame, so the first mode is the game's mode.
pub const LAUNCH_STABLE_FRAMES: u32 = 6;

/// At launch, the first mode is set after at most this many core frames,
/// also when the source still changes: half a second at 60 Hz. The
/// engine's debounce handles the later changes.
pub const LAUNCH_SETTLE_MAX_FRAMES: u32 = 30;

/// The source of a frame of `width` x `height` from a core with `geometry`
/// and `timing`. The aspect and the rate come from the core; the size comes
/// from the frame, because some cores send a new size without telling the
/// frontend. Rotation is not in the MVP.
pub fn source_geometry(
    width: u16,
    height: u16,
    geometry: &GameGeometry,
    timing: &Timing,
) -> SourceGeometry {
    SourceGeometry {
        width,
        height,
        refresh_mhz: timing.fps_mhz,
        aspect_milli: geometry.aspect_milli,
        interlaced: height >= INTERLACE_MIN_LINES,
        rotation: Rotation::R0,
    }
}

/// Decides when the source at launch has settled. Pure: the session runs
/// the core and passes each frame's source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct LaunchSettle {
    last: Option<SourceGeometry>,
    stable: u32,
    frames: u32,
}

impl LaunchSettle {
    /// Takes the source of one core frame. True when the session can set
    /// the first mode for it.
    pub(crate) fn push(&mut self, source: SourceGeometry) -> bool {
        self.frames += 1;
        if self.last == Some(source) {
            self.stable += 1;
        } else {
            self.last = Some(source);
            self.stable = 1;
        }
        self.stable >= LAUNCH_STABLE_FRAMES || self.frames >= LAUNCH_SETTLE_MAX_FRAMES
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GEOMETRY: GameGeometry = GameGeometry {
        base_width: 256,
        base_height: 192,
        max_width: 348,
        max_height: 240,
        aspect_milli: 1_306,
    };
    const TIMING: Timing = Timing {
        fps_mhz: 59_923,
        sample_rate_mhz: 44_100_000,
    };

    fn source(width: u16, height: u16) -> SourceGeometry {
        source_geometry(width, height, &GEOMETRY, &TIMING)
    }

    #[test]
    fn the_frame_gives_the_size_and_the_core_the_rate_and_aspect() {
        let s = source(320, 224);
        assert_eq!((s.width, s.height), (320, 224));
        assert_eq!(s.refresh_mhz, 59_923);
        assert_eq!(s.aspect_milli, 1_306);
        assert!(!s.interlaced);
        assert_eq!(s.rotation, Rotation::R0);
    }

    #[test]
    fn more_than_288_lines_is_interlaced() {
        assert!(!source(256, 288).interlaced);
        assert!(source(256, 289).interlaced);
        assert!(source(512, 448).interlaced);
    }

    /// The Genesis Plus GX trace: 256x192 for frames 0 to 3, then 320x224.
    /// The first four frames do not settle; the sixth frame of 320x224 does.
    #[test]
    fn a_change_at_the_fifth_frame_settles_on_the_new_size() {
        let mut settle = LaunchSettle::default();
        for _ in 0..4 {
            assert!(!settle.push(source(256, 192)));
        }
        for n in 1..LAUNCH_STABLE_FRAMES {
            assert!(!settle.push(source(320, 224)), "frame {n} of 320x224");
        }
        assert!(settle.push(source(320, 224)));
        assert_eq!(settle.frames, 10);
    }

    #[test]
    fn a_source_that_never_settles_stops_at_the_limit() {
        let mut settle = LaunchSettle::default();
        for n in 1..LAUNCH_SETTLE_MAX_FRAMES {
            let height = if n % 2 == 0 { 224 } else { 240 };
            assert!(!settle.push(source(320, height)));
        }
        assert!(settle.push(source(320, 224)));
        assert_eq!(settle.frames, LAUNCH_SETTLE_MAX_FRAMES);
    }

    #[test]
    fn a_steady_source_settles_after_the_stable_count() {
        let mut settle = LaunchSettle::default();
        for _ in 1..LAUNCH_STABLE_FRAMES {
            assert!(!settle.push(source(256, 224)));
        }
        assert!(settle.push(source(256, 224)));
    }
}
