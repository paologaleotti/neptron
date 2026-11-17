use neptron_types::{Filter, Rotation, Scanlines, Scene};

use super::{BLACK, black_lines, frame, rect, render, scene, xrgb};

const ALL: [Rotation; 4] = [Rotation::R0, Rotation::R90, Rotation::R180, Rotation::R270];

/// A 3x2 frame with a different value in each pixel, so each turn and each
/// mirror of it gives a different picture:
///
/// ```text
/// 1 2 3
/// 4 5 6
/// ```
fn asymmetric() -> Vec<u8> {
    xrgb(&[1, 2, 3, 4, 5, 6])
}

fn turned<'a>(s: Scene<'a>, rotation: Rotation) -> Scene<'a> {
    Scene { rotation, ..s }
}

fn opaque(values: &[u32]) -> Vec<u32> {
    values.iter().map(|v| BLACK | v).collect()
}

#[test]
fn each_turn_gives_its_own_picture() {
    let data = asymmetric();
    let g = frame(&data, 3, 2);
    let cases = [
        (Rotation::R0, (3, 2), vec![1, 2, 3, 4, 5, 6]),
        (Rotation::R90, (2, 3), vec![3, 6, 2, 5, 1, 4]),
        (Rotation::R180, (3, 2), vec![6, 5, 4, 3, 2, 1]),
        (Rotation::R270, (2, 3), vec![4, 1, 5, 2, 6, 3]),
    ];
    for (rotation, (w, h), expected) in cases {
        let s = scene(Some(g), rect(0, 0, 3, 2), rect(0, 0, w, h), Filter::Nearest);
        assert_eq!(
            render(&turned(s, rotation), w, h),
            opaque(&expected),
            "{rotation:?}"
        );
    }
}

/// libretro counts the turn counter-clockwise. A vertical game that the
/// core draws on its side has its upright top on the right of the frame,
/// and a line of text reads from the top of the frame to the bottom. `R90`
/// shows that line from left to right on the top line of the screen.
#[test]
fn r90_shows_a_column_read_downward_as_a_line_read_left_to_right() {
    // The text is the last column of a 4x3 frame: 7, 8, 9 from the top.
    let data = xrgb(&[0, 0, 0, 7, 0, 0, 0, 8, 0, 0, 0, 9]);
    let s = scene(
        Some(frame(&data, 4, 3)),
        rect(0, 0, 4, 3),
        rect(0, 0, 3, 4),
        Filter::Nearest,
    );
    let out = render(&turned(s, Rotation::R90), 3, 4);
    assert_eq!(out[..3], opaque(&[7, 8, 9]), "the top line of the screen");
    assert_eq!(out[3..], opaque(&[0; 9]));
    let out = render(&turned(s, Rotation::R270), 3, 4);
    assert_eq!(out[9..], opaque(&[9, 8, 7]), "R270 puts it upside down");
}

#[test]
fn a_turned_picture_scales_each_axis_of_the_turn() {
    let data = asymmetric();
    let s = scene(
        Some(frame(&data, 3, 2)),
        rect(0, 0, 3, 2),
        rect(1, 0, 4, 3),
        Filter::Nearest,
    );
    let b = 0;
    #[rustfmt::skip]
    let expected = [
        b, 3, 3, 6, 6,
        b, 2, 2, 5, 5,
        b, 1, 1, 4, 4,
    ];
    assert_eq!(render(&turned(s, Rotation::R90), 5, 3), opaque(&expected));
}

#[test]
fn a_crop_is_in_frame_pixels_before_the_turn() {
    // A 4x2 frame; the crop leaves out the first column.
    let data = xrgb(&[9, 1, 2, 3, 9, 4, 5, 6]);
    let s = scene(
        Some(frame(&data, 4, 2)),
        rect(1, 0, 3, 2),
        rect(0, 0, 2, 3),
        Filter::Nearest,
    );
    assert_eq!(
        render(&turned(s, Rotation::R90), 2, 3),
        opaque(&[3, 6, 2, 5, 1, 4])
    );
}

/// A crop that reaches past the left edge of the frame shows black where
/// that edge goes after the turn: the bottom for `R90`.
#[test]
fn a_crop_outside_the_frame_shows_black_on_the_turned_side() {
    let data = asymmetric();
    let s = scene(
        Some(frame(&data, 3, 2)),
        rect(-1, 0, 4, 2),
        rect(0, 0, 2, 4),
        Filter::Nearest,
    );
    let out = render(&turned(s, Rotation::R90), 2, 4);
    assert_eq!(out, opaque(&[3, 6, 2, 5, 1, 4, 0, 0]));
    let out = render(&turned(s, Rotation::R270), 2, 4);
    assert_eq!(out, opaque(&[0, 0, 4, 1, 5, 2, 6, 3]));
    let s = scene(
        Some(frame(&data, 3, 2)),
        rect(-1, 0, 4, 2),
        rect(0, 0, 4, 2),
        Filter::Nearest,
    );
    let out = render(&turned(s, Rotation::R180), 4, 2);
    assert_eq!(out, opaque(&[6, 5, 4, 0, 3, 2, 1, 0]));
}

/// The scanlines follow the lines of the turned picture: a 3x2 crop turned
/// by 90 degrees has 3 lines, so 6 destination lines are 2x.
#[test]
fn scanlines_follow_the_lines_of_the_turned_picture() {
    let data = xrgb(&[0x00_FFFFFF; 6]);
    let base = scene(
        Some(frame(&data, 3, 2)),
        rect(0, 0, 3, 2),
        rect(0, 0, 1, 6),
        Filter::Nearest,
    );
    let s = Scene {
        scanlines: black_lines(100),
        ..turned(base, Rotation::R90)
    };
    let w = 0xFF_FFFFFF;
    assert_eq!(render(&s, 1, 6), [w, BLACK, w, BLACK, w, BLACK]);
    // Unturned, the same 6 lines are 3x of 2 lines.
    let s = Scene {
        scanlines: black_lines(100),
        ..base
    };
    assert_eq!(render(&s, 1, 6), [w, w, BLACK, w, w, BLACK]);
    // Turned, 4 lines are not a whole multiple of 3: no scanlines.
    let s = Scene {
        scanlines: black_lines(100),
        dest: rect(0, 0, 1, 4),
        ..turned(base, Rotation::R270)
    };
    assert_eq!(render(&s, 1, 4), [w; 4]);
}

/// The turn of the frame on the CPU, by the pixel rule, as the picture `T`.
fn turn_on_cpu(pixels: &[u32], (w, h): (usize, usize), rotation: Rotation) -> Vec<u32> {
    let (tw, th) = match rotation {
        Rotation::R0 | Rotation::R180 => (w, h),
        Rotation::R90 | Rotation::R270 => (h, w),
    };
    let mut out = Vec::with_capacity(w * h);
    for ty in 0..th {
        for tx in 0..tw {
            let (fx, fy) = match rotation {
                Rotation::R0 => (tx, ty),
                Rotation::R90 => (w - 1 - ty, tx),
                Rotation::R180 => (w - 1 - tx, h - 1 - ty),
                Rotation::R270 => (ty, h - 1 - tx),
            };
            out.push(pixels[fy * w + fx]);
        }
    }
    out
}

/// Every filter, scale and scanline rule gives the turned scene the
/// pixels of the unturned scene of the frame turned on the CPU: the turn
/// is a permutation and adds no rounding.
#[test]
fn a_turn_equals_the_frame_turned_before_the_render() {
    let (w, h) = (5_usize, 3_usize);
    let pixels: Vec<u32> = (0..(w * h) as u32)
        .map(|i| (i * 0x0011_2F37) & 0x00FF_FFFF)
        .collect();
    let data = xrgb(&pixels);
    let dests = [
        rect(0, 0, 7, 11),
        rect(2, 1, 9, 6),
        rect(-2, -1, 12, 20),
        rect(0, 0, 2, 3),
    ];
    let lines = [Scanlines::NONE, black_lines(40)];
    for rotation in ALL {
        let t = turn_on_cpu(&pixels, (w, h), rotation);
        let (tw, th) = match rotation {
            Rotation::R0 | Rotation::R180 => (w, h),
            Rotation::R90 | Rotation::R270 => (h, w),
        };
        let t_data = xrgb(&t);
        for filter in [Filter::Nearest, Filter::SharpBilinear] {
            for dest in dests {
                for scanlines in lines {
                    let s = Scene {
                        scanlines,
                        rotation,
                        ..scene(
                            Some(frame(&data, w as u16, h as u16)),
                            rect(0, 0, w as u32, h as u32),
                            dest,
                            filter,
                        )
                    };
                    let plain = Scene {
                        scanlines,
                        ..scene(
                            Some(frame(&t_data, tw as u16, th as u16)),
                            rect(0, 0, tw as u32, th as u32),
                            dest,
                            filter,
                        )
                    };
                    let at = (rotation, filter, dest, scanlines);
                    assert_eq!(render(&s, 12, 22), render(&plain, 12, 22), "{at:?}");
                }
            }
        }
    }
}
