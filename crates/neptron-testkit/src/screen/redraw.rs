//! Draws a placement that the engine did not choose: the frame buffer of a
//! mode for a scene given by hand, with the reference renderer of the app.
//! The option sheets of a TV and the wrong placements of the mutation
//! tests use it. It adds no code path to the product.

use neptron_gfx::{OutputBuffer, ReferenceRenderer};
use neptron_types::{Filter, Frame, GamePicture, Modeline, PixelFormat, Rect, Rotation, Scene};

use super::image::Rgb;
use super::wire::{SceneFacts, WireError};

/// The frame buffer of `mode` for the scene `facts` over `frame`.
///
/// # Errors
///
/// [`WireError::Engine`] when the renderer refuses the scene.
pub fn draw(mode: &Modeline, frame: &Rgb, facts: &SceneFacts) -> Result<Rgb, WireError> {
    let data = frame.to_xrgb8888();
    let game = Frame {
        data: &data,
        width: u16::try_from(frame.width).unwrap_or(u16::MAX),
        height: u16::try_from(frame.height).unwrap_or(u16::MAX),
        pitch: frame.width as usize * 4,
        format: PixelFormat::Xrgb8888,
    };
    let scene = Scene {
        game: Some(GamePicture::Memory(game)),
        crop: facts.crop,
        dest: facts.dest,
        rotation: facts.rotation,
        filter: facts.filter,
        scanlines: facts.scanlines,
        ui: None,
        ui_dest: Rect {
            x: 0,
            y: 0,
            w: 0,
            h: 0,
        },
        ui_rotation: Rotation::R0,
        ui_filter: Filter::Nearest,
    };
    let (width, height) = (u32::from(mode.hactive), u32::from(mode.vactive));
    let mut bytes = vec![0_u8; width as usize * height as usize * 4];
    let mut out = OutputBuffer {
        data: &mut bytes,
        width,
        height,
        pitch: width as usize * 4,
    };
    ReferenceRenderer::new()
        .render(&scene, &mut out)
        .map_err(|e| WireError::Engine(e.to_string()))?;
    let rgb = bytes
        .chunks_exact(4)
        .flat_map(|p| [p[2], p[1], p[0]])
        .collect();
    Rgb::from_rgb(width, height, rgb).ok_or(WireError::NoScreen)
}

#[cfg(test)]
mod tests {
    use neptron_types::{Scanlines, TV_525_240P};

    use super::*;

    /// A white frame drawn into the middle of a TV mode lights only its
    /// destination.
    #[test]
    fn a_frame_lands_in_its_destination() {
        let frame = Rgb::new(4, 2, [255; 3]);
        let dest = Rect {
            x: 8,
            y: 2,
            w: 8,
            h: 4,
        };
        let crop = Rect {
            x: 0,
            y: 0,
            w: 4,
            h: 2,
        };
        let facts = SceneFacts {
            crop,
            dest,
            rotation: Rotation::R0,
            filter: Filter::Nearest,
            scanlines: Scanlines::NONE,
        };
        let fb = draw(&TV_525_240P, &frame, &facts).unwrap();
        assert_eq!((fb.width, fb.height), (720, 240));
        assert_eq!(fb.get(8, 2), [255; 3]);
        assert_eq!(fb.get(15, 5), [255; 3]);
        assert_eq!(fb.get(7, 2), [0; 3]);
        assert_eq!(fb.get(8, 6), [0; 3]);
    }
}
