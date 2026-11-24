//! A plain RGB image for the screen simulator: the frame, the glass and
//! the contact sheet.

/// An RGB image, rows first, three bytes per pixel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rgb {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// The pixels.
    pub rgb: Vec<u8>,
}

/// A turn of the glass, clockwise, as a monitor on its side shows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Turn {
    /// Upright.
    R0,
    /// The top of the screen is at the right.
    R90,
    /// Upside down.
    R180,
    /// The top of the screen is at the left.
    R270,
}

impl Rgb {
    /// An image of one colour.
    pub fn new(width: u32, height: u32, fill: [u8; 3]) -> Self {
        let rgb = fill.repeat(width as usize * height as usize);
        Self { width, height, rgb }
    }

    /// The image of packed RGB bytes. `None` when the length is wrong.
    pub fn from_rgb(width: u32, height: u32, rgb: Vec<u8>) -> Option<Self> {
        (rgb.len() == width as usize * height as usize * 3).then_some(Self { width, height, rgb })
    }

    /// Sets one pixel; a pixel outside the image is ignored.
    pub fn put(&mut self, x: u32, y: u32, color: [u8; 3]) {
        if x < self.width && y < self.height {
            let at = (y as usize * self.width as usize + x as usize) * 3;
            self.rgb[at..at + 3].copy_from_slice(&color);
        }
    }

    /// One pixel; black outside the image.
    pub fn get(&self, x: u32, y: u32) -> [u8; 3] {
        if x >= self.width || y >= self.height {
            return [0; 3];
        }
        let at = (y as usize * self.width as usize + x as usize) * 3;
        [self.rgb[at], self.rgb[at + 1], self.rgb[at + 2]]
    }

    /// The frame as a core gives it: XRGB8888, one native-endian `u32`
    /// per pixel, rows packed.
    pub fn to_xrgb8888(&self) -> Vec<u8> {
        self.rgb
            .chunks_exact(3)
            .flat_map(|c| {
                (u32::from(c[0]) << 16 | u32::from(c[1]) << 8 | u32::from(c[2])).to_ne_bytes()
            })
            .collect()
    }

    /// `w` by `h` pixels from `(x0, y0)`; black outside the image.
    pub fn crop(&self, x0: u32, y0: u32, w: u32, h: u32) -> Self {
        let mut out = Self::new(w, h, [0; 3]);
        for y in 0..h {
            for x in 0..w {
                out.put(x, y, self.get(x0 + x, y0 + y));
            }
        }
        out
    }

    /// Copies `image` with its top-left corner at `(x0, y0)`.
    pub fn paste(&mut self, image: &Self, x0: u32, y0: u32) {
        for y in 0..image.height {
            for x in 0..image.width {
                self.put(x0 + x, y0 + y, image.get(x, y));
            }
        }
    }

    /// The image turned clockwise by `turn`.
    pub fn turned(&self, turn: Turn) -> Self {
        let (w, h) = (self.width, self.height);
        let (ow, oh) = match turn {
            Turn::R0 | Turn::R180 => (w, h),
            Turn::R90 | Turn::R270 => (h, w),
        };
        let mut out = Self::new(ow, oh, [0; 3]);
        for y in 0..oh {
            for x in 0..ow {
                let (sx, sy) = match turn {
                    Turn::R0 => (x, y),
                    Turn::R90 => (y, h - 1 - x),
                    Turn::R180 => (w - 1 - x, h - 1 - y),
                    Turn::R270 => (w - 1 - y, x),
                };
                out.put(x, y, self.get(sx, sy));
            }
        }
        out
    }
}

impl Turn {
    /// The turn of `degrees`: 0, 90, 180 or 270.
    pub fn from_degrees(degrees: u32) -> Option<Self> {
        match degrees {
            0 => Some(Self::R0),
            90 => Some(Self::R90),
            180 => Some(Self::R180),
            270 => Some(Self::R270),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn two_by_one() -> Rgb {
        Rgb::from_rgb(2, 1, vec![1, 1, 1, 2, 2, 2]).unwrap()
    }

    #[test]
    fn a_quarter_turn_puts_the_top_at_the_right() {
        let t = two_by_one().turned(Turn::R90);
        assert_eq!((t.width, t.height), (1, 2));
        assert_eq!(t.get(0, 0), [1; 3], "the left pixel goes to the top");
        assert_eq!(t.get(0, 1), [2; 3]);
        let t = two_by_one().turned(Turn::R270);
        assert_eq!(t.get(0, 0), [2; 3]);
        let t = two_by_one().turned(Turn::R180);
        assert_eq!(t.get(0, 0), [2; 3]);
    }

    #[test]
    fn xrgb_has_red_in_bits_16_to_23() {
        let img = Rgb::from_rgb(1, 1, vec![0x12, 0x34, 0x56]).unwrap();
        assert_eq!(img.to_xrgb8888(), 0x0012_3456_u32.to_ne_bytes());
    }

    #[test]
    fn a_wrong_length_is_no_image() {
        assert!(Rgb::from_rgb(2, 2, vec![0; 3]).is_none());
        assert!(Turn::from_degrees(45).is_none());
    }
}
