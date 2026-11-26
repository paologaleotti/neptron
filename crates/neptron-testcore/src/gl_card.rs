//! The test card of a GL core, drawn with scissored clears only.
//!
//! The card is the `shape` card of script version 4 without its circle: a
//! white border, a blue top-left corner, a green staircase from the
//! top-left to the bottom-right corner, and a yellow arrow of stacked
//! rectangles that points to the top, all on the upright picture. Each part
//! is a rectangle that one `glScissor` and one `glClear` fill, so the card
//! needs no shader and works in every context type.

#![forbid(unsafe_code)]

use std::ffi::CStr;

use crate::script::Size;

/// `GL_FRAMEBUFFER`.
pub const GL_FRAMEBUFFER: u32 = 0x8D40;
/// `GL_SCISSOR_TEST`.
pub const GL_SCISSOR_TEST: u32 = 0x0C11;
/// `GL_BLEND`.
pub const GL_BLEND: u32 = 0x0BE2;
/// `GL_COLOR_BUFFER_BIT`.
pub const GL_COLOR_BUFFER_BIT: u32 = 0x4000;
/// `GL_NO_ERROR`.
pub const GL_NO_ERROR: u32 = 0;

/// The GL functions that the card uses. Each one exists in every context
/// type that a GL core can ask for.
pub const GL_FUNCTIONS: [&CStr; 8] = [
    c"glBindFramebuffer",
    c"glViewport",
    c"glEnable",
    c"glDisable",
    c"glScissor",
    c"glClearColor",
    c"glClear",
    c"glGetError",
];

/// The number of rectangles of the card, background included.
pub const CARD_RECTS: usize = 1 + 1 + STAIRS + ARROW_ROWS + 1 + 4;

/// One `glScissor` and `glClear` of the card, in GL window coordinates of
/// the framebuffer: `y` counts rows from GL `y = 0`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ClearRect {
    /// Left column.
    pub x: u32,
    /// First row.
    pub y: u32,
    /// Width, in pixels. 0 clears nothing.
    pub width: u32,
    /// Height, in rows. 0 clears nothing.
    pub height: u32,
    /// The colour, `0xRRGGBB`.
    pub rgb: u32,
}

/// The rectangles of the card for a frame of `size`, in drawing order.
///
/// The card takes the turn of `rotation` (counter-clockwise quarter turns,
/// as `SET_ROTATION` numbers them) as the shape card does: it is upright
/// after that turn. With `bottom_left` false, frame line `y` is GL row
/// `y`; with `bottom_left` true it is GL row `height - 1 - y`.
pub fn gl_card(size: Size, rotation: u32, bottom_left: bool) -> [ClearRect; CARD_RECTS] {
    let rotation = if rotation > 3 { 0 } else { rotation };
    let (w, h) = if rotation % 2 == 1 {
        (size.height, size.width)
    } else {
        (size.width, size.height)
    };
    let mut rects = [ClearRect::default(); CARD_RECTS];
    for (slot, part) in rects.iter_mut().zip(upright_parts(w, h)) {
        *slot = to_gl(part, size, rotation, bottom_left);
    }
    rects
}

/// The colour, `0xRRGGBB`, of pixel `(x, y)` of a frame of `size` that
/// shows the card, with line 0 at the top: the picture that a frontend
/// shows for the core, before the turn of `rotation`.
pub fn gl_card_pixel(size: Size, rotation: u32, x: u32, y: u32) -> u32 {
    gl_card(size, rotation, false)
        .iter()
        .rev()
        .find(|r| x >= r.x && x - r.x < r.width && y >= r.y && y - r.y < r.height)
        .map_or(CARD_BACKGROUND, |r| r.rgb)
}

/// The colours of the card, `0xRRGGBB`: those of the shape card.
const CARD_BACKGROUND: u32 = 0x20_20_20;
const CARD_BORDER: u32 = 0xFF_FF_FF;
const CARD_STAIRS: u32 = 0x00_FF_00;
const CARD_ARROW: u32 = 0xFF_FF_00;
const CARD_CORNER: u32 = 0x00_00_FF;

/// Steps of the staircase.
const STAIRS: usize = 16;
/// Rows of the arrow head.
const ARROW_ROWS: usize = 4;

/// A rectangle of the upright picture, top-left origin: `[x0, x1)` by
/// `[y0, y1)`.
#[derive(Debug, Clone, Copy)]
struct Part {
    x0: u32,
    y0: u32,
    x1: u32,
    y1: u32,
    rgb: u32,
}

/// The parts of the card on an upright picture of `w` by `h`, in drawing
/// order: a later part covers an earlier one.
fn upright_parts(w: u32, h: u32) -> impl Iterator<Item = Part> {
    let part = move |x0: u32, y0: u32, x1: u32, y1: u32, rgb| Part {
        x0: x0.min(w),
        y0: y0.min(h),
        x1: x1.min(w).max(x0.min(w)),
        y1: y1.min(h).max(y0.min(h)),
        rgb,
    };
    let line = (h / 60).max(1);
    let background = part(0, 0, w, h, CARD_BACKGROUND);
    let corner = part(0, 0, w / 6, h / 6, CARD_CORNER);
    let stairs = (0..STAIRS as u32).map(move |i| {
        let n = STAIRS as u32;
        part(
            i * w / n,
            i * h / n,
            (i + 1) * w / n,
            (i + 1) * h / n,
            CARD_STAIRS,
        )
    });
    let (tip, base, end) = (h / 20, h / 8, h / 6);
    let centre = w / 2;
    let head = (0..ARROW_ROWS as u32).map(move |row| {
        let n = ARROW_ROWS as u32;
        let half = (row + 1) * (w / 12) / n;
        part(
            centre.saturating_sub(half),
            tip + row * (base - tip) / n,
            centre + half.max(1),
            tip + (row + 1) * (base - tip) / n,
            CARD_ARROW,
        )
    });
    let stem_half = (w / 40).max(1);
    let stem = part(
        centre.saturating_sub(stem_half),
        base,
        centre + stem_half,
        end,
        CARD_ARROW,
    );
    let border = [
        part(0, 0, w, line, CARD_BORDER),
        part(0, h.saturating_sub(line), w, h, CARD_BORDER),
        part(0, 0, line, h, CARD_BORDER),
        part(w.saturating_sub(line), 0, w, h, CARD_BORDER),
    ];
    [background, corner]
        .into_iter()
        .chain(stairs)
        .chain(head)
        .chain([stem])
        .chain(border)
}

/// The rectangle of the frame of `size` that shows `part` of the upright
/// picture after the turn of `rotation`, in GL rows.
fn to_gl(part: Part, size: Size, rotation: u32, bottom_left: bool) -> ClearRect {
    let (fw, fh) = (size.width, size.height);
    // The inverse of the turn of the shape card: at 1 the right column of
    // the frame is the top row of the upright picture.
    let (x0, x1, y0, y1) = match rotation {
        1 => (fw - part.y1, fw - part.y0, part.x0, part.x1),
        2 => (fw - part.x1, fw - part.x0, fh - part.y1, fh - part.y0),
        3 => (part.y0, part.y1, fh - part.x1, fh - part.x0),
        _ => (part.x0, part.x1, part.y0, part.y1),
    };
    let (row0, row1) = if bottom_left {
        (fh - y1, fh - y0)
    } else {
        (y0, y1)
    };
    ClearRect {
        x: x0,
        y: row0,
        width: x1 - x0,
        height: row1 - row0,
        rgb: part.rgb,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const QVGA: Size = Size {
        width: 320,
        height: 240,
    };

    fn picture(size: Size, rotation: u32) -> Vec<u32> {
        (0..size.height)
            .flat_map(|y| (0..size.width).map(move |x| gl_card_pixel(size, rotation, x, y)))
            .collect()
    }

    #[test]
    fn the_card_shows_its_parts_where_they_belong() {
        let at = |x, y| gl_card_pixel(QVGA, 0, x, y);
        assert_eq!(at(0, 0), CARD_BORDER, "the border");
        assert_eq!(at(319, 239), CARD_BORDER);
        assert_eq!(at(30, 10), CARD_CORNER, "the top-left corner");
        assert_eq!(at(300, 10), CARD_BACKGROUND, "not the top-right corner");
        assert_eq!(
            at(160, 120),
            CARD_STAIRS,
            "the staircase crosses the centre"
        );
        assert_eq!(at(300, 230), CARD_STAIRS, "and ends bottom right");
        assert_eq!(at(40, 230), CARD_BACKGROUND, "not bottom left");
        assert_eq!(at(160, 13), CARD_ARROW, "the tip of the arrow");
        assert_eq!(at(160, 35), CARD_ARROW, "its stem");
        assert_eq!(at(140, 13), CARD_BACKGROUND, "the tip is narrow");
        assert_eq!(at(140, 28), CARD_ARROW, "the base is wide");
    }

    #[test]
    fn both_origins_give_the_same_picture() {
        for rotation in 0..4 {
            let top = gl_card(QVGA, rotation, false);
            let bottom = gl_card(QVGA, rotation, true);
            for (t, b) in top.iter().zip(&bottom) {
                assert_eq!(
                    (t.x, t.width, t.height, t.rgb),
                    (b.x, b.width, b.height, b.rgb)
                );
                assert_eq!(b.y, QVGA.height - t.y - t.height, "{rotation}");
            }
        }
    }

    #[test]
    fn a_turned_card_is_upright_after_the_turn() {
        // At 1 the right column of the frame is the top row of the upright
        // picture: the corner is at the top right of the frame.
        let side = Size {
            width: 320,
            height: 240,
        };
        // Upright pixel (35, 8) of the 240x320 picture at 1 and 3, and
        // (45, 10) of the 320x240 picture at 2: inside the corner, clear
        // of the staircase.
        let at = |x, y| gl_card_pixel(side, 1, x, y);
        assert_eq!(at(311, 35), CARD_CORNER);
        assert_eq!(at(8, 35), CARD_BACKGROUND);
        let at = |x, y| gl_card_pixel(side, 2, x, y);
        assert_eq!(at(274, 229), CARD_CORNER);
        let at = |x, y| gl_card_pixel(side, 3, x, y);
        assert_eq!(at(8, 204), CARD_CORNER);
        assert_ne!(picture(side, 1), picture(side, 3));
    }

    #[test]
    fn every_rect_fits_the_frame() {
        for size in [
            QVGA,
            Size {
                width: 1,
                height: 1,
            },
            Size {
                width: 7,
                height: 3,
            },
            Size {
                width: 4096,
                height: 4096,
            },
        ] {
            for rotation in 0..5 {
                for bottom_left in [false, true] {
                    for r in gl_card(size, rotation, bottom_left) {
                        assert!(r.x + r.width <= size.width, "{size:?} {r:?}");
                        assert!(r.y + r.height <= size.height, "{size:?} {r:?}");
                    }
                }
            }
        }
        let first = gl_card(QVGA, 0, false)[0];
        assert_eq!((first.width, first.height), (320, 240), "the background");
    }
}
