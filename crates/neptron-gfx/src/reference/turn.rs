use neptron_types::{Filter, Rect, Rotation};

use super::axis::{AxisMap, Tap};

/// One axis of the turned picture `T` of a scene, as a crop of one axis of
/// the game frame.
///
/// A reversed axis runs against its frame axis. It is the crop of the
/// mirrored frame axis: the crop starts at `frame_len - (start + len)`, and
/// a mirrored coordinate `c` is the frame coordinate `frame_len - 1 - c`.
/// So every rule of [`AxisMap`] (the scale, the filter, the test that a
/// pixel is inside the frame, the clamp of the far tap to the crop) holds
/// on the mirrored axis with no change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TurnedAxis {
    /// The first pixel of the crop on this axis, mirrored when reversed.
    pub(crate) crop_start: i64,
    /// The pixels of the crop on this axis.
    pub(crate) crop_len: u32,
    /// The pixels of the frame on this axis.
    pub(crate) frame_len: u32,
    /// The axis runs against its frame axis.
    pub(crate) reversed: bool,
}

/// The two axes of `T`, and which frame axis each one reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Turn {
    /// The columns of `T`.
    pub(crate) columns: TurnedAxis,
    /// The lines of `T`.
    pub(crate) lines: TurnedAxis,
    /// The columns of `T` read the lines of the frame, and the lines of
    /// `T` read its columns: `R90` and `R270`.
    pub(crate) swap: bool,
}

impl Turn {
    /// The axes of `crop` of a `width` by `height` frame turned by
    /// `rotation`, with the rule of `Scene::rotation`.
    pub(crate) fn of(crop: Rect, (width, height): (u16, u16), rotation: Rotation) -> Self {
        let x = |reversed| TurnedAxis::of(crop.x, crop.w, width, reversed);
        let y = |reversed| TurnedAxis::of(crop.y, crop.h, height, reversed);
        match rotation {
            Rotation::R0 => Self {
                columns: x(false),
                lines: y(false),
                swap: false,
            },
            // fx = w - 1 - ty, fy = tx.
            Rotation::R90 => Self {
                columns: y(false),
                lines: x(true),
                swap: true,
            },
            Rotation::R180 => Self {
                columns: x(true),
                lines: y(true),
                swap: false,
            },
            // fx = ty, fy = h - 1 - tx.
            Rotation::R270 => Self {
                columns: y(true),
                lines: x(false),
                swap: true,
            },
        }
    }
}

impl TurnedAxis {
    fn of(start: i32, len: u32, frame_len: u16, reversed: bool) -> Self {
        let frame_len = u32::from(frame_len);
        let start = i64::from(start);
        Self {
            crop_start: if reversed {
                i64::from(frame_len) - (start + i64::from(len))
            } else {
                start
            },
            crop_len: len,
            frame_len,
            reversed,
        }
    }

    /// The map of this axis into `dest_len` pixels with `filter`.
    pub(crate) fn map(&self, dest_len: u32, filter: Filter) -> AxisMap {
        AxisMap {
            crop_start: self.crop_start,
            crop_len: self.crop_len,
            dest_len,
            frame_len: self.frame_len,
            filter,
        }
    }

    /// The frame coordinate of coordinate `c` of this axis, which is
    /// inside the frame.
    pub(crate) fn frame_coord(&self, c: u32) -> u32 {
        if self.reversed {
            self.frame_len - 1 - c
        } else {
            c
        }
    }

    /// `tap` with its pixels as frame coordinates.
    pub(crate) fn tap_in_frame(&self, tap: Tap) -> Tap {
        match tap {
            Tap::Outside => Tap::Outside,
            Tap::Inside { near, far, weight } => Tap::Inside {
                near: self.frame_coord(near),
                far: self.frame_coord(far),
                weight,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: i32, y: i32, w: u32, h: u32) -> Rect {
        Rect { x, y, w, h }
    }

    /// The frame pixel of pixel `(tx, ty)` of `T`, through the axes.
    fn pixel(turn: &Turn, tx: u32, ty: u32) -> (i64, i64) {
        let coord = |axis: &TurnedAxis, t: u32| {
            let c = axis.crop_start + i64::from(t);
            if axis.reversed {
                i64::from(axis.frame_len) - 1 - c
            } else {
                c
            }
        };
        let (c, l) = (coord(&turn.columns, tx), coord(&turn.lines, ty));
        if turn.swap { (l, c) } else { (c, l) }
    }

    /// The axes give the pixel rule of `Scene::rotation` for every pixel
    /// of a crop that is not at the origin of the frame.
    #[test]
    fn the_axes_follow_the_pixel_rule() {
        let crop = rect(2, 1, 5, 3);
        let (w, h) = (5_i64, 3_i64);
        let rule = |rotation, tx: i64, ty: i64| match rotation {
            Rotation::R0 => (tx, ty),
            Rotation::R90 => (w - 1 - ty, tx),
            Rotation::R180 => (w - 1 - tx, h - 1 - ty),
            Rotation::R270 => (ty, h - 1 - tx),
        };
        for rotation in [Rotation::R0, Rotation::R90, Rotation::R180, Rotation::R270] {
            let turn = Turn::of(crop, (10, 8), rotation);
            let (tw, th) = if turn.swap { (h, w) } else { (w, h) };
            assert_eq!(
                (turn.columns.crop_len, turn.lines.crop_len),
                (tw as u32, th as u32),
                "{rotation:?}"
            );
            for ty in 0..th {
                for tx in 0..tw {
                    let (fx, fy) = rule(rotation, tx, ty);
                    let at = pixel(&turn, tx as u32, ty as u32);
                    assert_eq!(at, (2 + fx, 1 + fy), "{rotation:?} ({tx}, {ty})");
                }
            }
        }
    }

    /// A crop that reaches past the frame keeps the part outside the
    /// frame on the same side of the turned picture.
    #[test]
    fn a_reversed_axis_mirrors_the_crop() {
        let axis = TurnedAxis::of(-2, 6, 10, true);
        assert_eq!(axis.crop_start, 6);
        let (start, end) = axis.map(6, Filter::Nearest).inside();
        assert_eq!(
            (start, end),
            (0, 4),
            "the last two pixels are left of the frame"
        );
        assert_eq!(axis.frame_coord(6), 3);
        assert_eq!(axis.frame_coord(9), 0);
    }
}
