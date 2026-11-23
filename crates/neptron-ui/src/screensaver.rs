//! The screensaver: a black screen with the word `NEPTRON` that moves on a
//! diagonal and takes the next colour of a fixed cycle at each wall, and
//! the idle timer that decides when it shows.
//!
//! Both are pure. The idle timer follows the clock. The position is a
//! function of the count of UI frames since the screensaver started and
//! the size of the screen: the word moves one pixel on each axis in each
//! frame, a true 45 degree path. A speed in pixels per second would step
//! the two axes at different frames, and the path would shake on a panel
//! that shows each canvas pixel as several of its own.

use crate::canvas::{UI_HEIGHT, UI_WIDTH};
use crate::font;
use crate::frame::UiFrame;
use crate::geom::{Color, Rect};

/// The screen id of the screensaver in the tree.
pub const SCREENSAVER_SCREEN: &str = "screensaver";

/// The word that moves.
pub const WORDMARK: &str = "NEPTRON";

/// The scale of the word: each pixel of the UI font is a square of this
/// side.
pub const WORDMARK_SCALE: u32 = 2;

/// Width of the word in pixels.
pub const WORDMARK_WIDTH_PX: u32 = WORDMARK.len() as u32 * font::ADVANCE * WORDMARK_SCALE;

/// Height of the word in pixels: the scaled glyph box.
pub const WORDMARK_HEIGHT_PX: u32 = font::GLYPH_HEIGHT * WORDMARK_SCALE;

/// The pixels the word moves on each axis in one UI frame.
pub const STEP_PX: u64 = 1;

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
/// Violet at 75 %, with red and green raised, so it does not look dark
/// on black.
pub const VIOLET: Color = Color::rgb(0x90, 0x40, 0xBF);
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

/// The pose of the word `frames` UI frames after the screensaver started,
/// on a screen of `width` by `height` pixels. The word starts at the
/// centre of its free area and moves right and down.
pub fn pose(frames: u64, width: u32, height: u32) -> Pose {
    let (x, walls_x) = axis(frames, width, WORDMARK_WIDTH_PX);
    let (y, walls_y) = axis(frames, height, WORDMARK_HEIGHT_PX);
    let walls = walls_x + walls_y;
    let index = (walls % PALETTE.len() as u128) as usize;
    Pose {
        x,
        y,
        walls: u64::try_from(walls).unwrap_or(u64::MAX),
        color: PALETTE[index],
    }
}

/// Draws the screensaver on the UI canvas, `frames` UI frames after its
/// start.
pub fn draw(frames: u64, out: &mut UiFrame) {
    let pose = pose(frames, UI_WIDTH, UI_HEIGHT);
    out.scene
        .rect(Rect::new(0, 0, UI_WIDTH, UI_HEIGHT), Color::BLACK);
    let x = i32::try_from(pose.x).unwrap_or(0);
    let y = i32::try_from(pose.y).unwrap_or(0);
    let mut left = x;
    for c in WORDMARK.chars() {
        draw_glyph(out, left, y, pose.color, c);
        left += (font::ADVANCE * WORDMARK_SCALE) as i32;
    }
}

/// Draws `c` at [`WORDMARK_SCALE`] with its top-left corner at `x`, `y`:
/// one rectangle for each run of set pixels in a glyph row. The canvas
/// draws the font at one size only.
fn draw_glyph(out: &mut UiFrame, x: i32, y: i32, color: Color, c: char) {
    let scale = WORDMARK_SCALE as i32;
    for (row, bits) in (0i32..).zip(font::glyph(c).iter()) {
        let mut column = 0;
        while column < font::GLYPH_WIDTH {
            if bits & (0x80 >> column) == 0 {
                column += 1;
                continue;
            }
            let start = column;
            while column < font::GLYPH_WIDTH && bits & (0x80 >> column) != 0 {
                column += 1;
            }
            let rect = Rect::new(
                x + start as i32 * scale,
                y + row * scale,
                (column - start) * WORDMARK_SCALE,
                WORDMARK_SCALE,
            );
            out.scene.rect(rect, color);
        }
    }
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
/// pixels, and the walls hit on it. The unfolded coordinate goes up by
/// [`STEP_PX`] each frame; the position is its triangle wave over the free
/// room.
fn axis(frames: u64, side: u32, mark: u32) -> (u32, u128) {
    let free = u128::from(side.saturating_sub(mark));
    if free == 0 {
        return (0, 0);
    }
    let distance = u128::from(frames) * u128::from(STEP_PX);
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

    const MINUTE_NS: u64 = 60_000_000_000;
    const FIVE_MINUTES_NS: u64 = 5 * MINUTE_NS;

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
                let pose = pose(frame, width, height);
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
        assert_eq!((start.x, start.y, start.color), (118, 108, RED));
        let later = pose(60, 320, 240);
        assert_eq!((later.x, later.y), (118 + 60, 108 + 60));
    }

    #[test]
    fn the_colour_changes_exactly_at_a_wall_and_follows_the_cycle() {
        for (width, height) in SIZES.into_iter().take(4) {
            let free_x = width - WORDMARK_WIDTH_PX;
            let free_y = height - WORDMARK_HEIGHT_PX;
            let at = |pos: u32, free: u32| pos == 0 || pos == free;
            let mut before = pose(0, width, height);
            let mut changes = 0;
            for frame in 1..40_000u64 {
                let now = pose(frame, width, height);
                assert_eq!(now.color, PALETTE[(now.walls % 8) as usize]);
                if now.walls == before.walls {
                    assert_eq!(now.color, before.color);
                } else {
                    let wall = at(now.x, free_x) || at(now.y, free_y);
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
        // At 320x240 the word starts 108 px above the bottom wall and 118 px
        // left of the right wall: it touches the bottom first.
        let at_wall = pose(108, 320, 240);
        assert_eq!((at_wall.x, at_wall.y), (226, 216));
        assert_eq!((at_wall.walls, at_wall.color), (1, ORANGE));
        let after = pose(110, 320, 240);
        assert_eq!((after.x, after.y, after.walls), (228, 214, 1));
        let right = pose(118, 320, 240);
        assert_eq!(
            (right.x, right.y, right.walls, right.color),
            (236, 206, 2, YELLOW)
        );
    }

    /// The least light of a colour of the palette, as `luma` gives it: red
    /// at 75 % is the darkest colour that still reads well on black.
    const LUMA_MIN: u32 = 40;

    /// The light of a colour, 0 to 255, with the weights of Rec. 709.
    fn luma(color: Color) -> u32 {
        (2126 * u32::from(color.r()) + 7152 * u32::from(color.g()) + 722 * u32::from(color.b()))
            / 10_000
    }

    #[test]
    fn no_colour_of_the_palette_is_dark_on_black() {
        for color in PALETTE {
            assert!(luma(color) >= LUMA_MIN, "{color:?}: {}", luma(color));
        }
        assert!(
            luma(Color::rgb(0x5F, 0x00, 0xBF)) < LUMA_MIN,
            "a dark violet fails"
        );
    }

    #[test]
    fn the_word_is_the_font_at_twice_its_size() {
        let mut frame = UiFrame::new();
        draw(0, &mut frame);
        let mut canvas = crate::canvas::CpuCanvas::new();
        frame.render(&mut canvas);
        let mut small = UiFrame::new();
        small.scene.text_run(0, 0, RED, WORDMARK);
        let mut reference = crate::canvas::CpuCanvas::new();
        small.render(&mut reference);
        let start = pose(0, UI_WIDTH, UI_HEIGHT);
        for y in 0..font::GLYPH_HEIGHT {
            for x in 0..WORDMARK.len() as u32 * font::ADVANCE {
                let lit = reference.pixel(x, y) == Some(RED);
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let big = canvas.pixel(start.x + 2 * x + dx, start.y + 2 * y + dy);
                    assert_eq!(big == Some(RED), lit, "{x},{y}");
                }
            }
        }
    }

    #[test]
    fn each_frame_moves_one_pixel_on_each_axis_or_reflects_at_a_wall() {
        for (width, height) in SIZES.into_iter().take(4) {
            let free_x = width - WORDMARK_WIDTH_PX;
            let free_y = height - WORDMARK_HEIGHT_PX;
            let mut before = pose(0, width, height);
            for frame in 1..20_000u64 {
                let now = pose(frame, width, height);
                for (a, b, free) in [(before.x, now.x, free_x), (before.y, now.y, free_y)] {
                    let step = i64::from(b) - i64::from(a);
                    assert_eq!(
                        step.abs(),
                        1,
                        "{width}x{height} frame {frame}: {before:?} {now:?}"
                    );
                    assert!(b <= free);
                }
                before = now;
            }
        }
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
            draw(n, &mut frame);
            assert_eq!(frame.scene.capacity(), capacity);
        }
    }
}
