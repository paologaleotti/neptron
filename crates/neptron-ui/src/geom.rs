//! Colours and rectangles on the canvas.

/// An XRGB8888 colour. The unused top byte is always zero.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct Color(u32);

impl Color {
    /// Black.
    pub const BLACK: Self = Self::rgb(0, 0, 0);
    /// White.
    pub const WHITE: Self = Self::rgb(0xFF, 0xFF, 0xFF);

    /// A colour from its red, green and blue values.
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self(((r as u32) << 16) | ((g as u32) << 8) | b as u32)
    }

    /// A colour from an XRGB8888 word. The top byte is dropped.
    pub const fn from_xrgb(xrgb: u32) -> Self {
        Self(xrgb & 0x00FF_FFFF)
    }

    /// The XRGB8888 word, with a zero top byte.
    pub const fn xrgb(self) -> u32 {
        self.0
    }

    /// The red value.
    pub const fn r(self) -> u8 {
        (self.0 >> 16) as u8
    }

    /// The green value.
    pub const fn g(self) -> u8 {
        (self.0 >> 8) as u8
    }

    /// The blue value.
    pub const fn b(self) -> u8 {
        self.0 as u8
    }
}

/// A rectangle in canvas pixels. `x` and `y` are the top-left corner.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct Rect {
    /// Left edge.
    pub x: i32,
    /// Top edge.
    pub y: i32,
    /// Width.
    pub w: u32,
    /// Height.
    pub h: u32,
}

impl Rect {
    /// A rectangle from its corner and size.
    pub const fn new(x: i32, y: i32, w: u32, h: u32) -> Self {
        Self { x, y, w, h }
    }

    /// The first column to the right of the rectangle.
    pub const fn right(&self) -> i64 {
        self.x as i64 + self.w as i64
    }

    /// The first row below the rectangle.
    pub const fn bottom(&self) -> i64 {
        self.y as i64 + self.h as i64
    }

    /// True when the rectangle covers no pixel.
    pub const fn is_empty(&self) -> bool {
        self.w == 0 || self.h == 0
    }

    /// The pixels that both rectangles cover, or `None` when there are none.
    pub fn intersect(&self, other: &Self) -> Option<Self> {
        let x0 = i64::from(self.x).max(i64::from(other.x));
        let y0 = i64::from(self.y).max(i64::from(other.y));
        let x1 = self.right().min(other.right());
        let y1 = self.bottom().min(other.bottom());
        if x1 <= x0 || y1 <= y0 {
            return None;
        }
        Some(Self {
            x: clamp_i32(x0),
            y: clamp_i32(y0),
            w: clamp_u32(x1 - x0),
            h: clamp_u32(y1 - y0),
        })
    }

    /// True when every pixel of `other` is inside this rectangle.
    pub fn contains(&self, other: &Self) -> bool {
        other.is_empty()
            || (other.x >= self.x
                && other.y >= self.y
                && other.right() <= self.right()
                && other.bottom() <= self.bottom())
    }

    /// The rectangle made smaller by `by` pixels on each side. It does not
    /// go below a zero size.
    pub fn inset(&self, by: u32) -> Self {
        let twice = by.saturating_mul(2);
        Self {
            x: clamp_i32(i64::from(self.x) + i64::from(by)),
            y: clamp_i32(i64::from(self.y) + i64::from(by)),
            w: self.w.saturating_sub(twice),
            h: self.h.saturating_sub(twice),
        }
    }
}

fn clamp_i32(v: i64) -> i32 {
    i32::try_from(v).unwrap_or(if v < 0 { i32::MIN } else { i32::MAX })
}

fn clamp_u32(v: i64) -> u32 {
    u32::try_from(v).unwrap_or(if v < 0 { 0 } else { u32::MAX })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colour_drops_the_top_byte() {
        let c = Color::from_xrgb(0xAB12_3456);
        assert_eq!(c.xrgb(), 0x0012_3456);
        assert_eq!((c.r(), c.g(), c.b()), (0x12, 0x34, 0x56));
        assert_eq!(Color::rgb(0x12, 0x34, 0x56), c);
    }

    #[test]
    fn intersection_of_overlapping_rects() {
        let a = Rect::new(0, 0, 10, 10);
        let b = Rect::new(5, -3, 10, 6);
        assert_eq!(a.intersect(&b), Some(Rect::new(5, 0, 5, 3)));
    }

    #[test]
    fn touching_rects_do_not_intersect() {
        let a = Rect::new(0, 0, 10, 10);
        assert_eq!(a.intersect(&Rect::new(10, 0, 5, 5)), None);
        assert_eq!(a.intersect(&Rect::new(0, 0, 0, 5)), None);
    }

    #[test]
    fn extreme_coordinates_do_not_overflow() {
        let a = Rect::new(i32::MAX, i32::MAX, u32::MAX, u32::MAX);
        let b = Rect::new(i32::MIN, i32::MIN, u32::MAX, u32::MAX);
        assert_eq!(a.intersect(&b), None);
        assert_eq!(a.right(), i64::from(i32::MAX) + i64::from(u32::MAX));
    }

    #[test]
    fn containment() {
        let outer = Rect::new(16, 12, 288, 216);
        assert!(outer.contains(&Rect::new(16, 12, 288, 216)));
        assert!(!outer.contains(&Rect::new(15, 12, 10, 10)));
        assert!(!outer.contains(&Rect::new(300, 12, 5, 10)));
        assert!(outer.contains(&Rect::new(0, 0, 0, 0)));
    }

    #[test]
    fn inset_stops_at_zero() {
        assert_eq!(Rect::new(0, 0, 10, 4).inset(2), Rect::new(2, 2, 6, 0));
        assert_eq!(Rect::new(0, 0, 3, 3).inset(5), Rect::new(5, 5, 0, 0));
    }
}
