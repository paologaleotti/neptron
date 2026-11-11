use neptron_types::{Filter, Frame, PixelFormat, Rect, ScanlineKind, Scanlines, Scene};

use super::{Image, MAX_RECT_SIDE_PX, OutputBuffer, RectKind, ReferenceRenderer, RenderError};

const BLACK: u32 = 0xFF00_0000;

/// An XRGB8888 frame with one `u32` per pixel.
fn xrgb(pixels: &[u32]) -> Vec<u8> {
    pixels.iter().flat_map(|p| p.to_ne_bytes()).collect()
}

fn frame(data: &[u8], width: u16, height: u16) -> Frame<'_> {
    Frame {
        data,
        width,
        height,
        pitch: usize::from(width) * 4,
        format: PixelFormat::Xrgb8888,
    }
}

fn rect(x: i32, y: i32, w: u32, h: u32) -> Rect {
    Rect { x, y, w, h }
}

fn scene<'a>(game: Option<Frame<'a>>, crop: Rect, dest: Rect, filter: Filter) -> Scene<'a> {
    Scene {
        game,
        crop,
        dest,
        filter,
        scanlines: Scanlines::NONE,
        ui: None,
        ui_dest: rect(0, 0, 0, 0),
    }
}

/// Renders into a `width` by `height` output with no row padding and
/// returns the pixels as `u32`.
fn render(scene: &Scene<'_>, width: u32, height: u32) -> Vec<u32> {
    let mut data = vec![0_u8; (width * height * 4) as usize];
    let mut out = OutputBuffer {
        data: &mut data,
        width,
        height,
        pitch: width as usize * 4,
    };
    ReferenceRenderer::new()
        .render(scene, &mut out)
        .expect("valid scene");
    data.chunks_exact(4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .collect()
}

#[test]
fn no_game_gives_black_with_an_opaque_x_byte() {
    let s = scene(None, rect(0, 0, 1, 1), rect(0, 0, 4, 4), Filter::Nearest);
    assert_eq!(render(&s, 3, 2), [BLACK; 6]);
}

#[test]
fn row_padding_is_not_written() {
    let mut data = vec![0xAB_u8; 2 * 12 + 8];
    let mut out = OutputBuffer {
        data: &mut data,
        width: 2,
        height: 3,
        pitch: 12,
    };
    let s = scene(None, rect(0, 0, 0, 0), rect(0, 0, 0, 0), Filter::Nearest);
    ReferenceRenderer::new()
        .render(&s, &mut out)
        .expect("valid scene");
    for y in 0..3 {
        assert_eq!(&data[y * 12..y * 12 + 8], &[0, 0, 0, 0xFF, 0, 0, 0, 0xFF]);
    }
    assert_eq!(&data[8..12], &[0xAB; 4]);
    assert_eq!(&data[20..24], &[0xAB; 4]);
}

#[test]
fn identity_copies_the_frame_and_ignores_the_x_byte() {
    let data = xrgb(&[0x00_112233, 0x7F_445566, 0xFF_778899, 0x01_AABBCC]);
    let g = frame(&data, 2, 2);
    let s = scene(Some(g), rect(0, 0, 2, 2), rect(0, 0, 2, 2), Filter::Nearest);
    assert_eq!(
        render(&s, 2, 2),
        [0xFF_112233, 0xFF_445566, 0xFF_778899, 0xFF_AABBCC]
    );
}

#[test]
fn nearest_scales_each_axis_on_its_own() {
    let data = xrgb(&[1, 2]);
    let g = frame(&data, 2, 1);
    let s = scene(Some(g), rect(0, 0, 2, 1), rect(1, 0, 4, 2), Filter::Nearest);
    let b = BLACK;
    let (p, q) = (BLACK | 1, BLACK | 2);
    assert_eq!(
        render(&s, 6, 3),
        [b, p, p, q, q, b, b, p, p, q, q, b, b, b, b, b, b, b]
    );
}

#[test]
fn destination_outside_the_output_is_clipped_without_a_scale_change() {
    let data = xrgb(&[1, 2, 3]);
    let g = frame(&data, 3, 1);
    let s = scene(
        Some(g),
        rect(0, 0, 3, 1),
        rect(-3, 0, 6, 1),
        Filter::Nearest,
    );
    // Destination pixels 0 to 5 show 1 1 2 2 3 3; the output shows 3 to 5.
    assert_eq!(render(&s, 4, 1), [BLACK | 2, BLACK | 3, BLACK | 3, BLACK]);
    let s = scene(
        Some(g),
        rect(0, 0, 3, 1),
        rect(10, 5, 6, 1),
        Filter::Nearest,
    );
    assert_eq!(render(&s, 4, 1), [BLACK; 4]);
}

#[test]
fn crop_selects_a_part_of_the_frame() {
    let data = xrgb(&[1, 2, 3, 4, 5, 6]);
    let g = frame(&data, 3, 2);
    let s = scene(Some(g), rect(1, 1, 2, 1), rect(0, 0, 2, 1), Filter::Nearest);
    assert_eq!(render(&s, 2, 1), [BLACK | 5, BLACK | 6]);
}

#[test]
fn crop_outside_the_frame_shows_black() {
    let data = xrgb(&[1, 2]);
    let g = frame(&data, 2, 1);
    let s = scene(
        Some(g),
        rect(-1, 0, 4, 2),
        rect(0, 0, 4, 2),
        Filter::Nearest,
    );
    let b = BLACK;
    assert_eq!(render(&s, 4, 2), [b, b | 1, b | 2, b, b, b, b, b]);
}

#[test]
fn zero_size_rectangles_show_no_game() {
    let data = xrgb(&[0x00FF_FFFF]);
    let g = frame(&data, 1, 1);
    for (crop, dest) in [
        (rect(0, 0, 0, 1), rect(0, 0, 2, 2)),
        (rect(0, 0, 1, 0), rect(0, 0, 2, 2)),
        (rect(0, 0, 1, 1), rect(0, 0, 0, 2)),
        (rect(0, 0, 1, 1), rect(0, 0, 2, 0)),
    ] {
        let s = scene(Some(g), crop, dest, Filter::SharpBilinear);
        assert_eq!(render(&s, 2, 2), [BLACK; 4], "crop {crop:?} dest {dest:?}");
    }
}

#[test]
fn an_empty_game_frame_shows_black() {
    let g = frame(&[], 0, 0);
    let s = scene(
        Some(g),
        rect(0, 0, 2, 2),
        rect(0, 0, 2, 2),
        Filter::SharpBilinear,
    );
    assert_eq!(render(&s, 2, 2), [BLACK; 4]);
}

#[test]
fn sharp_bilinear_rounds_a_half_up() {
    // Scale 2.5: destination pixel 2 is half pixel 0 and half pixel 1.
    let data = xrgb(&[0x00_000000, 0x00_010203]);
    let g = frame(&data, 2, 1);
    let s = scene(
        Some(g),
        rect(0, 0, 2, 1),
        rect(0, 0, 5, 1),
        Filter::SharpBilinear,
    );
    assert_eq!(render(&s, 5, 1)[2], 0xFF_010102);
}

#[test]
fn sharp_bilinear_blends_four_pixels() {
    // Scale 2.5 on both axes: destination pixel (2, 2) has wx = wy = 1/2.
    let data = xrgb(&[0x00_000000, 0x00_FF0000, 0x00_FF0000, 0x00_FF0000]);
    let g = frame(&data, 2, 2);
    let s = scene(
        Some(g),
        rect(0, 0, 2, 2),
        rect(0, 0, 5, 5),
        Filter::SharpBilinear,
    );
    let out = render(&s, 5, 5);
    // 255 * 3 / 4 = 191.25.
    assert_eq!(out[2 * 5 + 2], 0xFF_BF0000);
    // (0, 0) has wx = wy = 1/10 toward the clamped pixel 0 itself.
    assert_eq!(out[0], BLACK);
    // (1, 1) has no blend.
    assert_eq!(out[5 + 1], BLACK);
}

#[test]
fn sharp_bilinear_at_an_integer_scale_equals_nearest() {
    let pixels: Vec<u32> = (0..12).map(|i| i * 0x0011_1111).collect();
    let data = xrgb(&pixels);
    let g = frame(&data, 4, 3);
    let nearest = scene(
        Some(g),
        rect(0, 0, 4, 3),
        rect(1, 2, 12, 6),
        Filter::Nearest,
    );
    let sharp = Scene {
        filter: Filter::SharpBilinear,
        scanlines: Scanlines::NONE,
        ..nearest
    };
    assert_eq!(render(&nearest, 14, 9), render(&sharp, 14, 9));
}

#[test]
fn the_same_renderer_draws_many_scenes() {
    let data = xrgb(&[1, 2, 3, 4]);
    let g = frame(&data, 2, 2);
    let mut renderer = ReferenceRenderer::new();
    let mut big = vec![0_u8; 8 * 8 * 4];
    let mut small = vec![0_u8; 2 * 2 * 4];
    let first = scene(
        Some(g),
        rect(0, 0, 2, 2),
        rect(0, 0, 8, 8),
        Filter::SharpBilinear,
    );
    let second = scene(Some(g), rect(0, 0, 2, 2), rect(0, 0, 2, 2), Filter::Nearest);
    for (s, buf, side) in [(&first, &mut big, 8), (&second, &mut small, 2)] {
        let mut out = OutputBuffer {
            data: buf,
            width: side,
            height: side,
            pitch: side as usize * 4,
        };
        renderer.render(s, &mut out).expect("valid scene");
    }
    let expected: Vec<u8> = [1, 2, 3, 4]
        .iter()
        .flat_map(|p| (BLACK | p).to_le_bytes())
        .collect();
    assert_eq!(small, expected);
}

fn render_err(
    scene: &Scene<'_>,
    data: &mut [u8],
    width: u32,
    height: u32,
    pitch: usize,
) -> RenderError {
    let before = data.to_vec();
    let mut out = OutputBuffer {
        data,
        width,
        height,
        pitch,
    };
    let err = ReferenceRenderer::new()
        .render(scene, &mut out)
        .expect_err("invalid input");
    assert_eq!(out.data, &before[..], "output changed after {err}");
    err
}

#[test]
fn a_short_output_is_refused() {
    let s = scene(None, rect(0, 0, 0, 0), rect(0, 0, 0, 0), Filter::Nearest);
    let mut data = vec![7_u8; 15];
    assert_eq!(
        render_err(&s, &mut data, 2, 2, 8),
        RenderError::BufferTooSmall {
            image: Image::Output,
            len: 15,
            min: 16
        }
    );
}

#[test]
fn a_small_output_pitch_is_refused() {
    let s = scene(None, rect(0, 0, 0, 0), rect(0, 0, 0, 0), Filter::Nearest);
    let mut data = vec![7_u8; 64];
    assert_eq!(
        render_err(&s, &mut data, 2, 2, 7),
        RenderError::PitchTooSmall {
            image: Image::Output,
            width: 2,
            pitch: 7,
            min: 8
        }
    );
}

#[test]
fn a_short_game_frame_is_refused() {
    let data = [0_u8; 7];
    let g = Frame {
        data: &data,
        width: 2,
        height: 2,
        pitch: 4,
        format: PixelFormat::Rgb565,
    };
    let s = scene(Some(g), rect(0, 0, 2, 2), rect(0, 0, 2, 2), Filter::Nearest);
    let mut out = vec![7_u8; 16];
    assert_eq!(
        render_err(&s, &mut out, 2, 2, 8),
        RenderError::BufferTooSmall {
            image: Image::Game,
            len: 7,
            min: 8
        }
    );
}

#[test]
fn a_small_game_pitch_is_refused() {
    let data = [0_u8; 64];
    let g = Frame {
        data: &data,
        width: 4,
        height: 2,
        pitch: 6,
        format: PixelFormat::Xrgb1555,
    };
    let s = scene(Some(g), rect(0, 0, 4, 2), rect(0, 0, 2, 2), Filter::Nearest);
    let mut out = vec![7_u8; 16];
    assert_eq!(
        render_err(&s, &mut out, 2, 2, 8),
        RenderError::PitchTooSmall {
            image: Image::Game,
            width: 4,
            pitch: 6,
            min: 8
        }
    );
}

#[test]
fn a_huge_rectangle_is_refused() {
    let data = xrgb(&[1]);
    let g = frame(&data, 1, 1);
    let mut out = vec![7_u8; 16];
    let s = scene(
        Some(g),
        rect(0, 0, MAX_RECT_SIDE_PX + 1, 1),
        rect(0, 0, 2, 2),
        Filter::Nearest,
    );
    assert_eq!(
        render_err(&s, &mut out, 2, 2, 8),
        RenderError::RectTooLarge {
            rect: RectKind::Crop,
            w: MAX_RECT_SIDE_PX + 1,
            h: 1,
            limit: MAX_RECT_SIDE_PX
        }
    );
    let s = scene(
        Some(g),
        rect(0, 0, 1, 1),
        rect(0, 0, 2, u32::MAX),
        Filter::Nearest,
    );
    assert_eq!(
        render_err(&s, &mut out, 2, 2, 8),
        RenderError::RectTooLarge {
            rect: RectKind::Dest,
            w: 2,
            h: u32::MAX,
            limit: MAX_RECT_SIDE_PX
        }
    );
}

#[test]
fn rectangles_are_not_checked_without_a_game() {
    let s = scene(
        None,
        rect(0, 0, u32::MAX, 1),
        rect(0, 0, u32::MAX, 1),
        Filter::Nearest,
    );
    assert_eq!(render(&s, 1, 1), [BLACK]);
}

#[test]
fn the_largest_rectangles_render() {
    let data = xrgb(&[0x00_FFFFFF, 0]);
    let g = frame(&data, 2, 1);
    let crop = rect(0, 0, MAX_RECT_SIDE_PX, 1);
    let dest = rect(-1000, 0, MAX_RECT_SIDE_PX, 1);
    let s = scene(Some(g), crop, dest, Filter::SharpBilinear);
    // Scale 1: output pixel 0 is destination pixel 1000, past the frame.
    assert_eq!(render(&s, 4, 1), [BLACK; 4]);
    let dest = rect(0, 0, MAX_RECT_SIDE_PX, 1);
    let s = scene(Some(g), crop, dest, Filter::SharpBilinear);
    assert_eq!(render(&s, 3, 1), [0xFF_FFFFFF, BLACK, BLACK]);
}

/// A scene with only the canvas `ui` at `ui_dest`.
fn ui_scene(ui: Frame<'_>, ui_dest: Rect) -> Scene<'_> {
    Scene {
        ui: Some(ui),
        ui_dest,
        ..scene(None, rect(0, 0, 0, 0), rect(0, 0, 0, 0), Filter::Nearest)
    }
}

#[test]
fn ui_is_drawn_opaque_over_the_game() {
    let game = xrgb(&[0x00_0000FF]);
    let ui = xrgb(&[0x7F_FF0000, 0x00_000000]);
    let s = Scene {
        ui: Some(frame(&ui, 2, 1)),
        ui_dest: rect(0, 0, 4, 2),
        ..scene(
            Some(frame(&game, 1, 1)),
            rect(0, 0, 1, 1),
            rect(0, 0, 4, 3),
            Filter::Nearest,
        )
    };
    // The black pixel of the canvas and its X byte do not let the game
    // show. Below the canvas, the game shows.
    let (g, red) = (0xFF_0000FF, 0xFF_FF0000);
    #[rustfmt::skip]
    let expected = [
        red, red, BLACK, BLACK,
        red, red, BLACK, BLACK,
        g, g, g, g,
    ];
    assert_eq!(render(&s, 4, 3), expected);
}

#[test]
fn ui_scales_each_axis_on_its_own() {
    let ui = xrgb(&[1, 2, 3, 4]);
    let s = ui_scene(frame(&ui, 2, 2), rect(1, 0, 6, 2));
    let (b, p, q, r, t) = (BLACK, BLACK | 1, BLACK | 2, BLACK | 3, BLACK | 4);
    #[rustfmt::skip]
    let expected = [
        b, p, p, p, q, q, q, b,
        b, r, r, r, t, t, t, b,
        b, b, b, b, b, b, b, b,
    ];
    assert_eq!(render(&s, 8, 3), expected);
}

#[test]
fn ui_fills_a_super_width_output() {
    let ui = xrgb(&[1, 2, 3, 4]);
    let s = ui_scene(frame(&ui, 2, 2), rect(0, 0, 12, 2));
    let row = |a, c| [[BLACK | a; 6], [BLACK | c; 6]].concat();
    assert_eq!(render(&s, 12, 2), [row(1, 2), row(3, 4)].concat());
}

#[test]
fn ui_uses_the_sample_rule_of_the_game() {
    // Centres of 5 pixels over 2 canvas pixels: 0.2, 0.6, 1.0, 1.4, 1.8.
    // A centre on a pixel edge takes the pixel on the right.
    let ui = xrgb(&[1, 2]);
    let s = ui_scene(frame(&ui, 2, 1), rect(0, 0, 5, 1));
    let (p, q) = (BLACK | 1, BLACK | 2);
    assert_eq!(render(&s, 5, 1), [p, p, q, q, q]);
    // Centres at 1.0 and 3.0.
    let ui = xrgb(&[1, 2, 3, 4]);
    let s = ui_scene(frame(&ui, 4, 1), rect(0, 0, 2, 1));
    assert_eq!(render(&s, 2, 1), [BLACK | 2, BLACK | 4]);
}

#[test]
fn ui_outside_the_output_is_clipped_without_a_scale_change() {
    let ui = xrgb(&[1, 2, 3, 4, 5, 6, 7, 8, 9]);
    let s = ui_scene(frame(&ui, 3, 3), rect(-1, -1, 3, 3));
    assert_eq!(
        render(&s, 2, 2),
        [BLACK | 5, BLACK | 6, BLACK | 8, BLACK | 9]
    );
    let s = ui_scene(frame(&ui, 3, 3), rect(-1, 1, 6, 6));
    // Destination column 1 shows canvas column 0; line 0 is above it.
    assert_eq!(render(&s, 2, 2), [BLACK, BLACK, BLACK | 1, BLACK | 2]);
    let s = ui_scene(frame(&ui, 3, 3), rect(2, 0, 3, 3));
    assert_eq!(render(&s, 2, 2), [BLACK; 4]);
}

#[test]
fn an_empty_ui_draws_nothing() {
    let game = xrgb(&[7]);
    let ui = xrgb(&[1]);
    for (canvas, ui_dest) in [
        (frame(&[], 0, 0), rect(0, 0, 1, 1)),
        (frame(&ui, 1, 1), rect(0, 0, 0, 1)),
        (frame(&ui, 1, 1), rect(0, 0, 1, 0)),
    ] {
        let s = Scene {
            ui: Some(canvas),
            ui_dest,
            ..scene(
                Some(frame(&game, 1, 1)),
                rect(0, 0, 1, 1),
                rect(0, 0, 1, 1),
                Filter::Nearest,
            )
        };
        assert_eq!(render(&s, 1, 1), [BLACK | 7], "ui_dest {ui_dest:?}");
    }
}

#[test]
fn ui_at_extreme_positions_renders() {
    let ui = xrgb(&[1, 2]);
    let side = MAX_RECT_SIDE_PX;
    for ui_dest in [
        rect(i32::MAX, i32::MAX, side, side),
        rect(i32::MIN, i32::MIN, side, side),
        rect(i32::MAX, 0, side, 1),
        rect(0, i32::MIN, 1, side),
    ] {
        let s = ui_scene(frame(&ui, 2, 1), ui_dest);
        assert_eq!(render(&s, 3, 2), [BLACK; 6], "ui_dest {ui_dest:?}");
    }
    // Output pixel 0 is the last pixel of the destination: canvas column 1.
    let last = 1 - i32::try_from(side).expect("the limit fits an i32");
    let s = ui_scene(frame(&ui, 2, 1), rect(last, 0, side, side));
    assert_eq!(render(&s, 2, 1), [BLACK | 2, BLACK]);
    let s = ui_scene(frame(&ui, 2, 1), rect(0, 0, side, side));
    assert_eq!(render(&s, 2, 1), [BLACK | 1, BLACK | 1]);
}

#[test]
fn a_huge_ui_destination_is_refused() {
    let ui = xrgb(&[1]);
    let mut out = vec![7_u8; 16];
    for (w, h) in [
        (MAX_RECT_SIDE_PX + 1, 1),
        (1, MAX_RECT_SIDE_PX + 1),
        (u32::MAX, u32::MAX),
    ] {
        let s = ui_scene(frame(&ui, 1, 1), rect(i32::MIN, i32::MAX, w, h));
        assert_eq!(
            render_err(&s, &mut out, 2, 2, 8),
            RenderError::RectTooLarge {
                rect: RectKind::UiDest,
                w,
                h,
                limit: MAX_RECT_SIDE_PX
            }
        );
    }
}

#[test]
fn the_ui_destination_is_not_checked_without_a_canvas() {
    let s = Scene {
        ui_dest: rect(0, 0, u32::MAX, u32::MAX),
        ..scene(None, rect(0, 0, 0, 0), rect(0, 0, 0, 0), Filter::Nearest)
    };
    assert_eq!(render(&s, 1, 1), [BLACK]);
}

#[test]
fn a_second_render_keeps_the_tables() {
    let game = xrgb(&[1, 2, 3, 4]);
    let ui = xrgb(&[5, 6]);
    let s = Scene {
        ui: Some(frame(&ui, 2, 1)),
        ui_dest: rect(-3, 1, 9, 2),
        ..scene(
            Some(frame(&game, 2, 2)),
            rect(0, 0, 2, 2),
            rect(1, 0, 4, 4),
            Filter::SharpBilinear,
        )
    };
    let mut renderer = ReferenceRenderer::new();
    let mut data = vec![0_u8; 6 * 4 * 4];
    let mut out = OutputBuffer {
        data: &mut data,
        width: 6,
        height: 4,
        pitch: 24,
    };
    renderer.render(&s, &mut out).expect("valid scene");
    let first = (
        renderer.columns.as_ptr(),
        renderer.columns.capacity(),
        renderer.lines.as_ptr(),
        renderer.lines.capacity(),
    );
    renderer.render(&s, &mut out).expect("valid scene");
    let second = (
        renderer.columns.as_ptr(),
        renderer.columns.capacity(),
        renderer.lines.as_ptr(),
        renderer.lines.capacity(),
    );
    assert_eq!(first, second);
}

#[test]
fn a_short_ui_canvas_is_refused() {
    let data = [0_u8; 9];
    let ui = Frame {
        data: &data,
        width: 2,
        height: 2,
        pitch: 6,
        format: PixelFormat::Xrgb1555,
    };
    let s = ui_scene(ui, rect(0, 0, 2, 2));
    let mut out = vec![7_u8; 16];
    assert_eq!(
        render_err(&s, &mut out, 2, 2, 8),
        RenderError::BufferTooSmall {
            image: Image::Ui,
            len: 9,
            min: 10
        }
    );
}

fn black_lines(strength_percent: u8) -> Scanlines {
    Scanlines {
        kind: ScanlineKind::Black,
        strength_percent,
    }
}

/// A one-column game of `lines` source lines, each its own colour, scaled
/// to `dest` with black scanlines at `strength_percent`.
fn scanline_scene<'a>(game: &'a [u8], lines: u16, dest: Rect, strength_percent: u8) -> Scene<'a> {
    Scene {
        scanlines: black_lines(strength_percent),
        ..scene(
            Some(frame(game, 1, lines)),
            rect(0, 0, 1, u32::from(lines)),
            dest,
            Filter::Nearest,
        )
    }
}

#[test]
fn two_x_black_scanlines_darken_the_second_line_of_each_pair() {
    let data = xrgb(&[0x00_FF8001, 0x00_030405]);
    let s = scanline_scene(&data, 2, rect(0, 0, 1, 4), 100);
    assert_eq!(render(&s, 1, 4), [0xFF_FF8001, BLACK, 0xFF_030405, BLACK]);
    // 50 %: (c * 50 + 50) / 100, a half rounded up.
    let s = scanline_scene(&data, 2, rect(0, 0, 1, 4), 50);
    assert_eq!(
        render(&s, 1, 4),
        [0xFF_FF8001, 0xFF_804001, 0xFF_030405, 0xFF_020203]
    );
}

#[test]
fn zero_strength_and_none_give_the_same_pixels() {
    let data = xrgb(&[0x00_FF8001, 0x00_030405]);
    let plain = scene(
        Some(frame(&data, 1, 2)),
        rect(0, 0, 1, 2),
        rect(0, 1, 1, 4),
        Filter::Nearest,
    );
    let zero = scanline_scene(&data, 2, rect(0, 1, 1, 4), 0);
    assert_eq!(render(&zero, 1, 6), render(&plain, 1, 6));
}

#[test]
fn three_x_darkens_one_line_in_three_and_four_x_two_in_four() {
    let data = xrgb(&[0x00_FFFFFF, 0x00_808080]);
    let (w, g) = (0xFF_FFFFFF, 0xFF_808080);
    let s = scanline_scene(&data, 2, rect(0, 0, 1, 6), 100);
    assert_eq!(render(&s, 1, 6), [w, w, BLACK, g, g, BLACK]);
    let s = scanline_scene(&data, 2, rect(0, 0, 1, 8), 100);
    assert_eq!(render(&s, 1, 8), [w, w, BLACK, BLACK, g, g, BLACK, BLACK]);
}

#[test]
fn a_fractional_or_unit_scale_has_no_scanlines() {
    let data = xrgb(&[0x00_FFFFFF, 0x00_808080]);
    for dest in [rect(0, 0, 1, 5), rect(0, 0, 1, 2), rect(0, 0, 1, 3)] {
        let plain = scene(
            Some(frame(&data, 1, 2)),
            rect(0, 0, 1, 2),
            dest,
            Filter::Nearest,
        );
        let lined = scanline_scene(&data, 2, dest, 100);
        assert_eq!(render(&lined, 1, 5), render(&plain, 1, 5), "{dest:?}");
    }
}

#[test]
fn the_pattern_follows_the_destination_not_the_output_parity() {
    let data = xrgb(&[0x00_FFFFFF, 0x00_808080]);
    let (w, g) = (0xFF_FFFFFF, 0xFF_808080);
    let s = scanline_scene(&data, 2, rect(0, 1, 1, 4), 100);
    assert_eq!(render(&s, 1, 6), [BLACK, w, BLACK, g, BLACK, BLACK]);
    // A destination that starts above the output keeps its phase.
    let s = scanline_scene(&data, 2, rect(0, -1, 1, 4), 100);
    assert_eq!(render(&s, 1, 3), [BLACK, g, BLACK]);
}

#[test]
fn a_crop_offset_keeps_the_pattern_on_whole_source_lines() {
    let data = xrgb(&[0x00_000001, 0x00_000002, 0x00_000003]);
    let s = Scene {
        scanlines: black_lines(100),
        ..scene(
            Some(frame(&data, 1, 3)),
            rect(0, 1, 1, 2),
            rect(0, 0, 1, 4),
            Filter::Nearest,
        )
    };
    assert_eq!(render(&s, 1, 4), [BLACK | 2, BLACK, BLACK | 3, BLACK]);
}

#[test]
fn sharp_bilinear_scales_the_exact_blend_before_rounding() {
    // Two source pixels over three columns: the middle one is the exact
    // half of 0 and 1. Rounded alone it is 1; at 50 % it is 0.25, so 0.
    // Rounding twice would give 1.
    let data = xrgb(&[0x00_000000, 0x00_000001]);
    let s = Scene {
        scanlines: black_lines(50),
        ..scene(
            Some(frame(&data, 2, 1)),
            rect(0, 0, 2, 1),
            rect(0, 0, 3, 2),
            Filter::SharpBilinear,
        )
    };
    assert_eq!(
        render(&s, 3, 2),
        [BLACK, BLACK | 1, BLACK | 1, BLACK, BLACK, BLACK | 1]
    );
}

#[test]
fn the_ui_and_the_borders_are_not_darkened() {
    let game = xrgb(&[0x00_FFFFFF]);
    let ui = xrgb(&[0x00_123456]);
    let s = Scene {
        ui: Some(frame(&ui, 1, 1)),
        ui_dest: rect(1, 0, 1, 4),
        ..scanline_scene(&game, 1, rect(0, 0, 2, 2), 100)
    };
    let (w, u) = (0xFF_FFFFFF, 0xFF_123456);
    assert_eq!(
        render(&s, 3, 4),
        [
            w, u, BLACK, BLACK, u, BLACK, BLACK, u, BLACK, BLACK, u, BLACK
        ]
    );
}

#[test]
fn a_scanline_strength_above_100_is_refused() {
    let data = xrgb(&[1, 2]);
    let s = scanline_scene(&data, 2, rect(0, 0, 1, 4), 101);
    let mut out = vec![7_u8; 16];
    assert_eq!(
        render_err(&s, &mut out, 1, 4, 4),
        RenderError::ScanlineStrengthTooHigh {
            strength_percent: 101,
            limit: 100
        }
    );
    // Without a game frame the strength is not used, and not checked.
    let s = Scene { game: None, ..s };
    assert_eq!(render(&s, 1, 4), [BLACK; 4]);
}
