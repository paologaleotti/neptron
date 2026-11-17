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

/// What the session knows about the frames of a game beyond the core's
/// geometry and timing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameRules {
    /// The turn that the core asks for (`Host::rotation`).
    pub rotation: Rotation,
    /// Every frame of the system is progressive, so the line count does not
    /// make a frame interlaced (the system data key `always_progressive`).
    pub always_progressive: bool,
}

impl FrameRules {
    /// No turn, and the line count decides interlace.
    pub const CONSOLE: FrameRules = FrameRules {
        rotation: Rotation::R0,
        always_progressive: false,
    };
}

/// The source of a frame of `width` x `height` from a core with `geometry`
/// and `timing`, with the turn and the interlace rule of `rules`.
///
/// The rate comes from the core. The size comes from the frame, because
/// some cores send a new size without telling the frontend. The aspect is
/// the one of the frame as the core draws it, which the policy reads
/// before it turns the source. A core reports the aspect of the upright
/// picture (FinalBurn Neo: 3:4 for a 320x240 frame on its side), so for a
/// turn of 90 or 270 degrees the session inverts an aspect that the core
/// reported. The host's fallback, the base width over the base height, is
/// already the shape of the frame as drawn, and stays.
pub fn source_geometry(
    width: u16,
    height: u16,
    geometry: &GameGeometry,
    timing: &Timing,
    rules: FrameRules,
) -> SourceGeometry {
    SourceGeometry {
        width,
        height,
        refresh_mhz: timing.fps_mhz,
        aspect_milli: frame_aspect_milli(geometry, rules.rotation),
        interlaced: !rules.always_progressive && height >= INTERLACE_MIN_LINES,
        rotation: rules.rotation,
    }
}

/// The aspect of the frame as drawn, in thousandths.
fn frame_aspect_milli(geometry: &GameGeometry, rotation: Rotation) -> u32 {
    let aspect = geometry.aspect_milli;
    match rotation {
        Rotation::R90 | Rotation::R270 if geometry.aspect_from_core && aspect > 0 => {
            (1_000_000 + aspect / 2) / aspect
        }
        Rotation::R0 | Rotation::R90 | Rotation::R180 | Rotation::R270 => aspect,
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
        aspect_from_core: true,
    };
    const TIMING: Timing = Timing {
        fps_mhz: 59_923,
        sample_rate_mhz: 44_100_000,
    };

    fn source(width: u16, height: u16) -> SourceGeometry {
        source_geometry(width, height, &GEOMETRY, &TIMING, FrameRules::CONSOLE)
    }

    fn turned(rotation: Rotation, aspect_milli: u32, aspect_from_core: bool) -> SourceGeometry {
        let geometry = GameGeometry {
            base_width: 320,
            base_height: 240,
            max_width: 320,
            max_height: 240,
            aspect_milli,
            aspect_from_core,
        };
        let rules = FrameRules {
            rotation,
            always_progressive: false,
        };
        source_geometry(320, 240, &geometry, &TIMING, rules)
    }

    /// DoDonPachi: FinalBurn Neo reports 3:4 for its 320x240 frame on its
    /// side; the frame as drawn is 4:3, and the policy turns it back.
    #[test]
    fn a_quarter_turn_inverts_the_aspect_that_the_core_reported() {
        for rotation in [Rotation::R90, Rotation::R270] {
            let s = turned(rotation, 750, true);
            assert_eq!(s.aspect_milli, 1_333, "{rotation:?}");
            assert_eq!(s.rotation, rotation);
        }
        assert_eq!(turned(Rotation::R90, 1_333, true).aspect_milli, 750);
        assert_eq!(turned(Rotation::R90, 1_000, true).aspect_milli, 1_000);
        assert_eq!(turned(Rotation::R90, 3, true).aspect_milli, 333_333);
    }

    #[test]
    fn a_half_turn_or_no_turn_keeps_the_aspect() {
        for rotation in [Rotation::R0, Rotation::R180] {
            let s = turned(rotation, 750, true);
            assert_eq!((s.aspect_milli, s.rotation), (750, rotation));
        }
    }

    /// With no aspect from the core the host's fallback is the shape of the
    /// frame as drawn, so it stays.
    #[test]
    fn the_fallback_aspect_of_the_host_is_not_inverted() {
        assert_eq!(turned(Rotation::R90, 1_333, false).aspect_milli, 1_333);
        assert_eq!(turned(Rotation::R270, 1_333, false).aspect_milli, 1_333);
        assert_eq!(turned(Rotation::R90, 0, true).aspect_milli, 0);
    }

    #[test]
    fn an_always_progressive_system_has_no_interlaced_frame() {
        let rules = FrameRules {
            rotation: Rotation::R0,
            always_progressive: true,
        };
        for height in [224, 289, 384, 480, 640] {
            let s = source_geometry(496, height, &GEOMETRY, &TIMING, rules);
            assert!(!s.interlaced, "{height} lines");
        }
        assert!(source(496, 384).interlaced, "a console frame of 384 lines");
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
