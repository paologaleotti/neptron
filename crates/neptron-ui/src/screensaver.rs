//! The screensaver: a black screen with the word `NEPTRON` that moves on a
//! diagonal and takes the next colour of a fixed cycle at each wall, and
//! the idle timer that decides when it shows.
//!
//! Both are pure. The position is a function of the time since the
//! screensaver started and the size of the screen, so it is the same at
//! every refresh rate and drifts at no frame rate.

use crate::canvas::{UI_HEIGHT, UI_WIDTH};
use crate::font;
use crate::frame::UiFrame;
use crate::geom::{Color, Rect};

/// The screen id of the screensaver in the tree.
pub const SCREENSAVER_SCREEN: &str = "screensaver";

/// The word that moves.
pub const WORDMARK: &str = "NEPTRON";

/// Width of the word in pixels.
pub const WORDMARK_WIDTH_PX: u32 = WORDMARK.len() as u32 * font::ADVANCE;

/// Height of the word in pixels: the glyph box.
pub const WORDMARK_HEIGHT_PX: u32 = font::GLYPH_HEIGHT;

/// The speed on each axis, as a fraction of the screen side per second:
/// the numerator.
pub const SPEED_SCREENS_PER_S_NUM: u64 = 1;

/// The speed on each axis, as a fraction of the screen side per second:
/// the denominator.
pub const SPEED_SCREENS_PER_S_DEN: u64 = 8;

/// Nanoseconds in one second.
pub const NANOS_PER_SECOND: u64 = 1_000_000_000;

/// Red at 75 %.
pub const RED: Color = Color::rgb(0xBF, 0x00, 0x00);
/// Orange at 75 %.
pub const ORANGE: Color = Color::rgb(0xBF, 0x5F, 0x00);
/// Yellow at 75 %.
pub const YELLOW: Color = Color::rgb(0xBF, 0xBF, 0x00);
/// Green at 75 %.
pub const GREEN: Color = Color::rgb(0x00, 0xBF, 0x00);
/// Cyan at 75 %.
pub const CYAN: Color = Color::rgb(0x00, 0xBF, 0xBF);
/// Blue at 75 %, with some green, so it stays visible on a dark tube.
pub const BLUE: Color = Color::rgb(0x00, 0x5F, 0xBF);
/// Violet at 75 %.
pub const VIOLET: Color = Color::rgb(0x5F, 0x00, 0xBF);
/// Magenta at 75 %.
pub const MAGENTA: Color = Color::rgb(0xBF, 0x00, 0xBF);

/// The colours of the word, in cycle order. Each wall gives the next one.
/// No channel is above 0xBF, to keep the load on a tube low.
pub const PALETTE: [Color; 8] = [RED, ORANGE, YELLOW, GREEN, CYAN, BLUE, VIOLET, MAGENTA];

/// Where the word is, and which colour it has.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pose {
    /// The left edge of the word, in whole pixels.
    pub x: u32,
    /// The top edge of the word, in whole pixels.
    pub y: u32,
    /// The walls hit since the start, both axes together.
    pub walls: u64,
    /// The colour of the word: the entry of [`PALETTE`] at `walls`.
    pub color: Color,
}

/// The idle timer: when the screensaver starts and ends.
///
/// The app calls [`Idle::step`] once per frame with the time of the
/// frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Idle {
    /// The time of the last input, or of the last frame in which the
    /// screensaver could not show.
    since_ns: u64,
    /// The time the screensaver started, while it shows.
    shown_at_ns: Option<u64>,
}

/// What one frame of the idle timer gives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdleStep {
    /// The menus show.
    Hidden,
    /// The screensaver shows, for this time since its start.
    Shows {
        /// The time since the screensaver started, in nanoseconds.
        elapsed_ns: u64,
    },
    /// The input of this frame ended the screensaver. The app consumes it:
    /// it gives the UI no action in this frame.
    Woken,
}

/// The pose of the word `elapsed_ns` after the screensaver started, on a
/// screen of `width` by `height` pixels. The word starts at the centre of
/// its free area and moves right and down.
pub fn pose(elapsed_ns: u64, width: u32, height: u32) -> Pose {
    let (x, walls_x) = axis(elapsed_ns, width, WORDMARK_WIDTH_PX);
    let (y, walls_y) = axis(elapsed_ns, height, WORDMARK_HEIGHT_PX);
    let walls = walls_x + walls_y;
    let index = (walls % PALETTE.len() as u128) as usize;
    Pose {
        x,
        y,
        walls: u64::try_from(walls).unwrap_or(u64::MAX),
        color: PALETTE[index],
    }
}

/// Draws the screensaver on the UI canvas, `elapsed_ns` after its start.
pub fn draw(elapsed_ns: u64, out: &mut UiFrame) {
    let pose = pose(elapsed_ns, UI_WIDTH, UI_HEIGHT);
    out.scene
        .rect(Rect::new(0, 0, UI_WIDTH, UI_HEIGHT), Color::BLACK);
    let x = i32::try_from(pose.x).unwrap_or(0);
    let y = i32::try_from(pose.y).unwrap_or(0);
    out.scene.text_run(x, y, pose.color, WORDMARK);
}

impl Idle {
    /// A timer that starts to count at `now_ns`.
    pub fn new(now_ns: u64) -> Self {
        Self {
            since_ns: now_ns,
            shown_at_ns: None,
        }
    }

    /// True while the screensaver shows.
    pub fn shows(&self) -> bool {
        self.shown_at_ns.is_some()
    }

    /// One frame at `now_ns`. `input` is true when a wake input came in
    /// the frame. `allowed` is false while the screensaver may not show:
    /// a game runs, or a dialog with a timeout shows. `timeout_ns` is the
    /// time with no input before the screensaver starts, `None` for the
    /// setting Off.
    ///
    /// While the screensaver may not show, the timer restarts. A wake ends
    /// the screensaver with [`IdleStep::Woken`]. A screensaver that may no
    /// longer show ends with [`IdleStep::Hidden`]: no input is consumed.
    pub fn step(
        &mut self,
        now_ns: u64,
        input: bool,
        allowed: bool,
        timeout_ns: Option<u64>,
    ) -> IdleStep {
        let off = !allowed || timeout_ns.is_none();
        if let Some(shown_at_ns) = self.shown_at_ns {
            if input || off {
                self.shown_at_ns = None;
                self.since_ns = now_ns;
                return if input {
                    IdleStep::Woken
                } else {
                    IdleStep::Hidden
                };
            }
            return IdleStep::Shows {
                elapsed_ns: now_ns.saturating_sub(shown_at_ns),
            };
        }
        if input || off {
            self.since_ns = now_ns;
            return IdleStep::Hidden;
        }
        match timeout_ns {
            Some(timeout) if now_ns.saturating_sub(self.since_ns) >= timeout => {
                self.shown_at_ns = Some(now_ns);
                IdleStep::Shows { elapsed_ns: 0 }
            }
            _ => IdleStep::Hidden,
        }
    }
}

/// The position on one axis of a screen side `side` for a word of `mark`
/// pixels, and the walls hit on it. The unfolded coordinate goes up at a
/// fixed speed; the position is its triangle wave over the free room.
fn axis(elapsed_ns: u64, side: u32, mark: u32) -> (u32, u128) {
    let free = u128::from(side.saturating_sub(mark));
    if free == 0 {
        return (0, 0);
    }
    let distance = u128::from(elapsed_ns) * u128::from(side) * u128::from(SPEED_SCREENS_PER_S_NUM)
        / (u128::from(SPEED_SCREENS_PER_S_DEN) * u128::from(NANOS_PER_SECOND));
    let unfolded = free / 2 + distance;
    let phase = unfolded % (2 * free);
    let position = if phase <= free {
        phase
    } else {
        2 * free - phase
    };
    // `position` is at most `free`, which came from a `u32`.
    (u32::try_from(position).unwrap_or(0), unfolded / free)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINUTE_NS: u64 = 60 * NANOS_PER_SECOND;
    const FIVE_MINUTES_NS: u64 = 5 * MINUTE_NS;
    const FRAME_60_NS: u64 = 16_666_667;
    const FRAME_50_NS: u64 = 20_000_000;

    #[test]
    fn the_screensaver_starts_at_the_set_time_and_not_before() {
        let mut idle = Idle::new(1_000);
        let on = Some(FIVE_MINUTES_NS);
        assert_eq!(idle.step(1_000, false, true, on), IdleStep::Hidden);
        assert_eq!(
            idle.step(1_000 + FIVE_MINUTES_NS - 1, false, true, on),
            IdleStep::Hidden
        );
        assert_eq!(
            idle.step(1_000 + FIVE_MINUTES_NS, false, true, on),
            IdleStep::Shows { elapsed_ns: 0 }
        );
        assert_eq!(
            idle.step(1_000 + FIVE_MINUTES_NS + 7, false, true, on),
            IdleStep::Shows { elapsed_ns: 7 }
        );
        assert!(idle.shows());
    }

    #[test]
    fn input_restarts_the_time() {
        let mut idle = Idle::new(0);
        let on = Some(MINUTE_NS);
        assert_eq!(idle.step(MINUTE_NS - 1, true, true, on), IdleStep::Hidden);
        assert_eq!(
            idle.step(2 * MINUTE_NS - 2, false, true, on),
            IdleStep::Hidden
        );
        assert_eq!(
            idle.step(2 * MINUTE_NS - 1, false, true, on),
            IdleStep::Shows { elapsed_ns: 0 }
        );
    }

    #[test]
    fn a_wake_is_consumed_once_and_the_time_counts_again_from_it() {
        let mut idle = Idle::new(0);
        let on = Some(MINUTE_NS);
        let _ = idle.step(MINUTE_NS, false, true, on);
        assert_eq!(idle.step(MINUTE_NS + 5, true, true, on), IdleStep::Woken);
        assert!(!idle.shows());
        assert_eq!(idle.step(MINUTE_NS + 6, true, true, on), IdleStep::Hidden);
        assert_eq!(
            idle.step(2 * MINUTE_NS + 5, false, true, on),
            IdleStep::Hidden
        );
        assert_eq!(
            idle.step(2 * MINUTE_NS + 6, false, true, on),
            IdleStep::Shows { elapsed_ns: 0 }
        );
    }

    #[test]
    fn it_never_starts_while_it_may_not_show_or_is_off() {
        let mut idle = Idle::new(0);
        for minute in 0..60 {
            let now = minute * MINUTE_NS;
            assert_eq!(
                idle.step(now, false, false, Some(MINUTE_NS)),
                IdleStep::Hidden
            );
        }
        let mut off = Idle::new(0);
        for minute in 0..60 {
            assert_eq!(
                off.step(minute * MINUTE_NS, false, true, None),
                IdleStep::Hidden
            );
        }
    }

    #[test]
    fn the_time_counts_from_the_moment_it_may_show_again() {
        let mut idle = Idle::new(0);
        let on = Some(MINUTE_NS);
        let _ = idle.step(10 * MINUTE_NS, false, false, on);
        assert_eq!(
            idle.step(11 * MINUTE_NS - 1, false, true, on),
            IdleStep::Hidden
        );
        assert_eq!(
            idle.step(11 * MINUTE_NS, false, true, on),
            IdleStep::Shows { elapsed_ns: 0 }
        );
    }

    #[test]
    fn a_screensaver_that_may_no_longer_show_ends_with_no_wake() {
        let mut idle = Idle::new(0);
        let on = Some(MINUTE_NS);
        let _ = idle.step(MINUTE_NS, false, true, on);
        assert_eq!(idle.step(MINUTE_NS + 1, false, false, on), IdleStep::Hidden);
        assert!(!idle.shows());
        let _ = idle.step(3 * MINUTE_NS, false, true, on);
        assert!(idle.shows());
        assert_eq!(
            idle.step(3 * MINUTE_NS + 1, false, true, None),
            IdleStep::Hidden
        );
    }

    /// Sizes of a screen: the UI canvas, a VGA mode, a flat panel, a
    /// super-wide 15 kHz mode, and a screen smaller than the word.
    const SIZES: [(u32, u32); 5] = [(320, 240), (640, 480), (1920, 1080), (1920, 240), (40, 8)];

    #[test]
    fn the_word_stays_on_the_screen_in_whole_pixels() {
        for (width, height) in SIZES {
            for frame in 0..20_000 {
                let pose = pose(frame * FRAME_60_NS, width, height);
                assert!(
                    pose.x + WORDMARK_WIDTH_PX <= width.max(WORDMARK_WIDTH_PX),
                    "{width}x{height} frame {frame}: {pose:?}"
                );
                assert!(
                    pose.y + WORDMARK_HEIGHT_PX <= height.max(WORDMARK_HEIGHT_PX),
                    "{width}x{height} frame {frame}: {pose:?}"
                );
            }
        }
    }

    #[test]
    fn it_starts_at_the_centre_in_red_and_moves_right_and_down() {
        let start = pose(0, 320, 240);
        assert_eq!((start.x, start.y, start.color), (139, 114, RED));
        let later = pose(NANOS_PER_SECOND, 320, 240);
        assert_eq!((later.x, later.y), (139 + 40, 114 + 30));
    }

    #[test]
    fn the_speed_is_a_fraction_of_the_screen_at_every_size_and_rate() {
        for (width, height) in SIZES.into_iter().take(4) {
            for frame_ns in [FRAME_60_NS, FRAME_50_NS] {
                // Short enough to hit no wall on any of the sizes.
                let frames = NANOS_PER_SECOND / 2 / frame_ns;
                let a = pose(0, width, height);
                let b = pose(frames * frame_ns, width, height);
                let moved_x = u64::from(b.x - a.x);
                let expected_x = frames * frame_ns * u64::from(width) / (8 * NANOS_PER_SECOND);
                assert_eq!(moved_x, expected_x, "{width}x{height}");
                assert!(b.y > a.y, "{width}x{height}");
            }
        }
    }

    #[test]
    fn the_colour_changes_exactly_at_a_wall_and_follows_the_cycle() {
        for (width, height) in SIZES.into_iter().take(4) {
            let free_x = width - WORDMARK_WIDTH_PX;
            let free_y = height - WORDMARK_HEIGHT_PX;
            // The most pixels the word moves in one frame on each axis.
            let reach = |side: u32| u64::from(side) * FRAME_60_NS / (8 * NANOS_PER_SECOND) + 1;
            let (reach_x, reach_y) = (reach(width), reach(height));
            let near = |pos: u32, free: u32, reach: u64| {
                u64::from(pos) <= reach || u64::from(free - pos) <= reach
            };
            let mut before = pose(0, width, height);
            let mut changes = 0;
            for frame in 1..40_000u64 {
                let now = pose(frame * FRAME_60_NS, width, height);
                assert_eq!(now.color, PALETTE[(now.walls % 8) as usize]);
                if now.walls == before.walls {
                    assert_eq!(now.color, before.color);
                } else {
                    let wall = near(now.x, free_x, reach_x) || near(now.y, free_y, reach_y);
                    assert!(wall, "{width}x{height} frame {frame}: {before:?} {now:?}");
                    assert!(now.walls - before.walls <= 2, "{now:?}");
                    assert_ne!(now.color, before.color);
                    changes += 1;
                }
                before = now;
            }
            assert!(changes > 8, "{width}x{height}: {changes} changes");
        }
    }

    #[test]
    fn a_bounce_turns_the_motion_at_the_wall() {
        // At 320 wide the word is 139 px from the right wall at the start
        // and moves 40 px/s: it touches the wall after 3.475 s.
        let at_wall = pose(3_475_000_000, 320, 240);
        assert_eq!((at_wall.x, at_wall.walls, at_wall.color), (278, 1, ORANGE));
        let after = pose(3_600_000_000, 320, 240);
        assert_eq!((after.x, after.walls), (278 - 5, 1));
    }

    #[test]
    fn the_palette_never_repeats_a_colour_and_is_never_white() {
        for (i, color) in PALETTE.iter().enumerate() {
            let next = PALETTE[(i + 1) % PALETTE.len()];
            assert_ne!(*color, next);
            assert!(color.r() <= 0xBF && color.g() <= 0xBF && color.b() <= 0xBF);
        }
    }

    #[test]
    fn a_frame_of_the_screensaver_does_not_allocate_after_the_first() {
        let mut frame = UiFrame::new();
        draw(0, &mut frame);
        let capacity = frame.scene.capacity();
        for n in 1..600 {
            frame.clear();
            draw(n * FRAME_60_NS, &mut frame);
            assert_eq!(frame.scene.capacity(), capacity);
        }
    }
}
