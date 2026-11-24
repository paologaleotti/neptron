use neptron_types::{Filter, Frame, Rect, Rotation, Scene};

use super::super::{MAX_RECT_SIDE_PX, RectKind, RenderError};
use super::turn::turn_on_cpu;
use super::{BLACK, frame, rect, render, render_err, scene, ui_scene, xrgb};

const ALL: [Rotation; 4] = [Rotation::R0, Rotation::R90, Rotation::R180, Rotation::R270];

/// A 3x2 canvas with a different value in each pixel, so each corner and
/// each turn of it is distinct:
///
/// ```text
/// 1 2 3
/// 4 5 6
/// ```
fn asymmetric() -> Vec<u8> {
    xrgb(&[1, 2, 3, 4, 5, 6])
}

/// A scene with only the canvas `ui` at `ui_dest`, turned and filtered.
fn turned_ui<'a>(ui: Frame<'a>, ui_dest: Rect, turn: Rotation, filter: Filter) -> Scene<'a> {
    Scene {
        ui_rotation: turn,
        ui_filter: filter,
        ..ui_scene(ui, ui_dest)
    }
}

fn opaque(values: &[u32]) -> Vec<u32> {
    values.iter().map(|v| BLACK | v).collect()
}

#[test]
fn each_ui_turn_at_scale_1_gives_the_pixel_rule() {
    let data = asymmetric();
    let cases = [
        (Rotation::R0, (3, 2), [1, 2, 3, 4, 5, 6]),
        (Rotation::R90, (2, 3), [3, 6, 2, 5, 1, 4]),
        (Rotation::R180, (3, 2), [6, 5, 4, 3, 2, 1]),
        (Rotation::R270, (2, 3), [4, 1, 5, 2, 6, 3]),
    ];
    for (turn, (w, h), expected) in cases {
        for filter in [Filter::Nearest, Filter::SharpBilinear] {
            let s = turned_ui(frame(&data, 3, 2), rect(0, 0, w, h), turn, filter);
            assert_eq!(render(&s, w, h), opaque(&expected), "{turn:?} {filter:?}");
        }
    }
}

#[test]
fn each_ui_turn_at_scale_2_repeats_each_pixel() {
    let data = asymmetric();
    #[rustfmt::skip]
    let cases: [(Rotation, (u32, u32), Vec<u32>); 3] = [
        (Rotation::R90, (4, 6), vec![
            3, 3, 6, 6,
            3, 3, 6, 6,
            2, 2, 5, 5,
            2, 2, 5, 5,
            1, 1, 4, 4,
            1, 1, 4, 4,
        ]),
        (Rotation::R180, (6, 4), vec![
            6, 6, 5, 5, 4, 4,
            6, 6, 5, 5, 4, 4,
            3, 3, 2, 2, 1, 1,
            3, 3, 2, 2, 1, 1,
        ]),
        (Rotation::R270, (4, 6), vec![
            4, 4, 1, 1,
            4, 4, 1, 1,
            5, 5, 2, 2,
            5, 5, 2, 2,
            6, 6, 3, 3,
            6, 6, 3, 3,
        ]),
    ];
    for (turn, (w, h), expected) in cases {
        for filter in [Filter::Nearest, Filter::SharpBilinear] {
            let s = turned_ui(frame(&data, 3, 2), rect(0, 0, w, h), turn, filter);
            assert_eq!(render(&s, w, h), opaque(&expected), "{turn:?} {filter:?}");
        }
    }
}

/// A 4x2 canvas, black, with a white vertical stroke of one pixel in
/// canvas column `column`.
fn stroke(column: usize) -> Vec<u8> {
    let mut pixels = [0_u32; 8];
    pixels[column] = 0x00_FFFFFF;
    pixels[4 + column] = 0x00_FFFFFF;
    xrgb(&pixels)
}

/// The long side of the turned canvas, 4 pixels, scales to 3: a scale of
/// 0.75, as a turned 320x240 canvas on 240 lines. Each stroke sits at
/// pixel 1 of the long side of `T`. The centres of the 3 output pixels map
/// to 2/3, 2 and 10/3 on that side. Nearest takes pixels 0, 2 and 3 and
/// drops the stroke. Sharp bilinear at a scale below 1 is a bilinear blend:
/// 1/6 of the stroke, 1/2 of it, and none, so 255/6 = 42.5 rounds to 43 and
/// 127.5 to 128. The short side stays at scale 1 and does not blend.
#[test]
fn sharp_bilinear_keeps_a_stroke_that_nearest_drops_below_scale_1() {
    let (soft, half) = (BLACK | 0x2B_2B2B, BLACK | 0x80_8080);
    // (turn, canvas column of the stroke, output size, sharp output)
    #[rustfmt::skip]
    let cases = [
        (Rotation::R0, 1, (3, 2), [soft, half, BLACK, soft, half, BLACK]),
        (Rotation::R180, 2, (3, 2), [soft, half, BLACK, soft, half, BLACK]),
        (Rotation::R90, 2, (2, 3), [soft, soft, half, half, BLACK, BLACK]),
        (Rotation::R270, 1, (2, 3), [soft, soft, half, half, BLACK, BLACK]),
    ];
    for (turn, column, (w, h), expected) in cases {
        let data = stroke(column);
        let dest = rect(0, 0, w, h);
        let sharp = turned_ui(frame(&data, 4, 2), dest, turn, Filter::SharpBilinear);
        assert_eq!(render(&sharp, w, h), expected, "{turn:?}");
        let nearest = turned_ui(frame(&data, 4, 2), dest, turn, Filter::Nearest);
        assert_eq!(render(&nearest, w, h), [BLACK; 6], "{turn:?}");
    }
}

/// Every filter and scale gives the turned canvas the pixels of the
/// upright canvas turned on the CPU: the turn is a permutation and adds no
/// rounding. The destinations scale each axis by 1, by a whole number, and
/// below and above 1 by a fraction, and clip at the edges of the output.
#[test]
fn a_turned_canvas_equals_the_canvas_turned_before_the_render() {
    let (w, h) = (5_usize, 3_usize);
    let pixels: Vec<u32> = (0..(w * h) as u32)
        .map(|i| (i * 0x0013_2F37) & 0x00FF_FFFF)
        .collect();
    let data = xrgb(&pixels);
    let dests = [
        rect(0, 0, 5, 5),
        rect(1, 2, 6, 10),
        rect(0, 0, 2, 4),
        rect(2, 1, 7, 11),
        rect(-2, -1, 12, 20),
        rect(4, 13, 9, 13),
    ];
    for turn in ALL {
        let t = turn_on_cpu(&pixels, (w, h), turn);
        let (tw, th) = match turn {
            Rotation::R0 | Rotation::R180 => (w, h),
            Rotation::R90 | Rotation::R270 => (h, w),
        };
        let t_data = xrgb(&t);
        for filter in [Filter::Nearest, Filter::SharpBilinear] {
            for dest in dests {
                let s = turned_ui(frame(&data, w as u16, h as u16), dest, turn, filter);
                let plain = turned_ui(
                    frame(&t_data, tw as u16, th as u16),
                    dest,
                    Rotation::R0,
                    filter,
                );
                let at = (turn, filter, dest);
                assert_eq!(render(&s, 12, 22), render(&plain, 12, 22), "{at:?}");
            }
        }
    }
}

/// A turned canvas clipped at each edge of the output shows the part of
/// the same canvas drawn whole in a larger output.
#[test]
fn a_turned_canvas_clips_at_each_edge() {
    let data = asymmetric();
    let (out_w, out_h) = (5_u32, 6_u32);
    for turn in [Rotation::R90, Rotation::R180, Rotation::R270] {
        let (tw, th) = match turn {
            Rotation::R180 => (6_u32, 4_u32),
            _ => (4, 6),
        };
        let (left, top) = (2 - tw as i32, 2 - th as i32);
        let (right, bottom) = (out_w as i32 - 2, out_h as i32 - 2);
        // Two columns or two lines stay inside at each edge.
        for (x, y) in [(left, 0), (0, top), (right, 1), (1, bottom)] {
            let clipped = turned_ui(
                frame(&data, 3, 2),
                rect(x, y, tw, th),
                turn,
                Filter::Nearest,
            );
            // The same canvas in an output with a margin of 8 on each side.
            let whole = turned_ui(
                frame(&data, 3, 2),
                rect(x + 8, y + 8, tw, th),
                turn,
                Filter::Nearest,
            );
            let big = render(&whole, out_w + 16, out_h + 16);
            let window: Vec<u32> = (0..out_h)
                .flat_map(|row| {
                    let start = ((row + 8) * (out_w + 16) + 8) as usize;
                    big[start..start + out_w as usize].to_vec()
                })
                .collect();
            let shown = render(&clipped, out_w, out_h);
            assert_eq!(shown, window, "{turn:?} at ({x}, {y})");
            assert!(shown.iter().any(|&p| p != BLACK), "{turn:?} at ({x}, {y})");
        }
    }
}

#[test]
fn a_huge_turned_ui_destination_is_refused() {
    let ui = xrgb(&[1, 2]);
    let mut out = vec![7_u8; 16];
    for turn in ALL {
        for (w, h) in [(MAX_RECT_SIDE_PX + 1, 1), (1, MAX_RECT_SIDE_PX + 1)] {
            let s = turned_ui(frame(&ui, 2, 1), rect(0, 0, w, h), turn, Filter::Nearest);
            assert_eq!(
                render_err(&s, &mut out, 2, 2, 8),
                RenderError::RectTooLarge {
                    rect: RectKind::UiDest,
                    w,
                    h,
                    limit: MAX_RECT_SIDE_PX
                },
                "{turn:?}"
            );
        }
    }
}

/// The game picture and the canvas turn on their own. The game frame and
/// the canvas are the same asymmetric 3x2 picture, the game with 0x100
/// added, and each goes to its own 2x3 or 3x2 rectangle of a 6x3 output.
#[test]
fn the_game_and_the_canvas_turn_on_their_own() {
    let game = xrgb(&[0x101, 0x102, 0x103, 0x104, 0x105, 0x106]);
    let ui = asymmetric();
    let b = 0;
    // (game turn, game dest, canvas turn, canvas dest, output)
    #[rustfmt::skip]
    let cases = [
        (Rotation::R90, rect(0, 0, 2, 3), Rotation::R270, rect(4, 0, 2, 3), [
            0x103, 0x106, b, b, 4, 1,
            0x102, 0x105, b, b, 5, 2,
            0x101, 0x104, b, b, 6, 3,
        ]),
        (Rotation::R0, rect(0, 0, 3, 2), Rotation::R90, rect(4, 0, 2, 3), [
            0x101, 0x102, 0x103, b, 3, 6,
            0x104, 0x105, 0x106, b, 2, 5,
            b, b, b, b, 1, 4,
        ]),
        (Rotation::R270, rect(0, 0, 2, 3), Rotation::R0, rect(3, 0, 3, 2), [
            0x104, 0x101, b, 1, 2, 3,
            0x105, 0x102, b, 4, 5, 6,
            0x106, 0x103, b, b, b, b,
        ]),
        (Rotation::R180, rect(0, 0, 3, 2), Rotation::R180, rect(3, 1, 3, 2), [
            0x106, 0x105, 0x104, b, b, b,
            0x103, 0x102, 0x101, 6, 5, 4,
            b, b, b, 3, 2, 1,
        ]),
    ];
    for (game_turn, dest, ui_turn, ui_dest, expected) in cases {
        let s = Scene {
            rotation: game_turn,
            ui: Some(frame(&ui, 3, 2)),
            ui_dest,
            ui_rotation: ui_turn,
            ..scene(
                Some(frame(&game, 3, 2)),
                rect(0, 0, 3, 2),
                dest,
                Filter::Nearest,
            )
        };
        let at = (game_turn, ui_turn);
        assert_eq!(render(&s, 6, 3), opaque(&expected), "{at:?}");
    }
}

/// A turned canvas over a game covers the game inside its destination and
/// shows black outside it, opaque in both parts. The game is one colour in
/// a 3x4 destination; the canvas turned by `R90` covers columns 2 and 3 of
/// lines 1 to 3, so column 2 is over the game and column 3 is not.
#[test]
fn a_turned_canvas_covers_the_game_inside_and_outside_its_destination() {
    let game = xrgb(&[0x00_AA00]);
    let ui = asymmetric();
    let g = 0xAA00;
    let b = 0;
    for filter in [Filter::Nearest, Filter::SharpBilinear] {
        let s = Scene {
            ui: Some(frame(&ui, 3, 2)),
            ui_dest: rect(2, 1, 2, 3),
            ui_rotation: Rotation::R90,
            ui_filter: filter,
            ..scene(
                Some(frame(&game, 1, 1)),
                rect(0, 0, 1, 1),
                rect(0, 0, 3, 4),
                Filter::Nearest,
            )
        };
        #[rustfmt::skip]
        let expected = [
            g, g, g, b, b,
            g, g, 3, 6, b,
            g, g, 2, 5, b,
            g, g, 1, 4, b,
        ];
        assert_eq!(render(&s, 5, 4), opaque(&expected), "{filter:?}");
    }
}

/// The game takes the filter of the scene and the canvas its own: a game
/// with sharp bilinear under a canvas with nearest, and the reverse.
#[test]
fn the_canvas_filter_does_not_change_the_game() {
    let game = xrgb(&[0x00_0000, 0x00_FFFFFF]);
    let ui = stroke(1);
    let base = Scene {
        ui: Some(frame(&ui, 4, 2)),
        ui_dest: rect(0, 1, 3, 2),
        ..scene(
            Some(frame(&game, 2, 1)),
            rect(0, 0, 2, 1),
            rect(0, 0, 3, 1),
            Filter::SharpBilinear,
        )
    };
    let soft = |filter| {
        let s = Scene {
            ui_filter: filter,
            ..base
        };
        render(&s, 3, 3)
    };
    let (nearest, sharp) = (soft(Filter::Nearest), soft(Filter::SharpBilinear));
    assert_eq!(nearest[..3], sharp[..3], "the game line is the same");
    assert_eq!(nearest[3..], [BLACK; 6]);
    let (s, h) = (BLACK | 0x2B_2B2B, BLACK | 0x80_8080);
    assert_eq!(sharp[3..], [s, h, BLACK, s, h, BLACK]);
}
