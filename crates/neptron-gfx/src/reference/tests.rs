use neptron_types::{Filter, Frame, PixelFormat, Rect, Scene};

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
        ui: None,
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
