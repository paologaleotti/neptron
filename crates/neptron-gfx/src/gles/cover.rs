//! The parts of the game quad that the opaque UI does not cover.

/// A rectangle of output pixels from the top left, with no zero side.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Area {
    pub(super) x: u32,
    pub(super) y: u32,
    pub(super) w: u32,
    pub(super) h: u32,
}

impl Area {
    /// The area of the output pixels `[x, y, w, h]` of a quad, or `None`
    /// for a zero side. The quad holds whole numbers below 2^24, which a
    /// float holds exactly.
    pub(super) fn of(visible: [f32; 4]) -> Option<Self> {
        let [x, y, w, h] = visible.map(|v| v as u32);
        (w > 0 && h > 0).then_some(Self { x, y, w, h })
    }

    fn right(self) -> u32 {
        self.x + self.w
    }

    fn bottom(self) -> u32 {
        self.y + self.h
    }

    fn new(x: u32, y: u32, right: u32, bottom: u32) -> Option<Self> {
        (right > x && bottom > y).then_some(Self {
            x,
            y,
            w: right - x,
            h: bottom - y,
        })
    }
}

/// The parts of `area` outside `cover`: the rows above and below `cover`,
/// then the columns left and right of it in its rows. They do not overlap,
/// and with `cover` they hold every pixel of `area`.
///
/// The UI canvas is opaque and is drawn after the game, so a game pixel
/// under it never shows. On a TV the canvas covers 640 of 720 columns, so
/// a game drawn only outside it costs the GPU a fraction of a full quad.
pub(super) fn outside(area: Area, cover: Area) -> [Option<Area>; 4] {
    let top = cover.y.clamp(area.y, area.bottom());
    let bottom = cover.bottom().clamp(area.y, area.bottom());
    let left = cover.x.clamp(area.x, area.right());
    let right = cover.right().clamp(area.x, area.right());
    if top >= bottom || left >= right {
        return [Some(area), None, None, None];
    }
    [
        Area::new(area.x, area.y, area.right(), top),
        Area::new(area.x, bottom, area.right(), area.bottom()),
        Area::new(area.x, top, left, bottom),
        Area::new(right, top, area.right(), bottom),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area(x: u32, y: u32, w: u32, h: u32) -> Area {
        Area { x, y, w, h }
    }

    /// Every pixel of `area` is in `cover` or in exactly one part.
    fn assert_partition(a: Area, cover: Area) {
        let parts = outside(a, cover);
        let inside = |r: Area, x: u32, y: u32| {
            (r.x..r.right()).contains(&x) && (r.y..r.bottom()).contains(&y)
        };
        for y in 0..40 {
            for x in 0..40 {
                let in_parts = parts.iter().flatten().filter(|p| inside(**p, x, y)).count();
                let expected = usize::from(inside(a, x, y) && !inside(cover, x, y));
                assert_eq!(
                    in_parts, expected,
                    "({x}, {y}) of {a:?} under {cover:?}: {parts:?}"
                );
            }
        }
    }

    #[test]
    fn a_cover_inside_leaves_four_bands() {
        let parts = outside(area(0, 0, 40, 30), area(10, 5, 20, 15));
        assert_eq!(
            parts,
            [
                Some(area(0, 0, 40, 5)),
                Some(area(0, 20, 40, 10)),
                Some(area(0, 5, 10, 15)),
                Some(area(30, 5, 10, 15)),
            ]
        );
    }

    /// A game at 704 columns from 8 under a canvas at 640 columns from 40,
    /// as on a 720x240 TV mode: two side strips of 32 columns.
    #[test]
    fn the_canvas_on_a_tv_leaves_two_side_strips() {
        let parts = outside(area(8, 8, 704, 224), area(40, 0, 640, 240));
        assert_eq!(
            parts,
            [
                None,
                None,
                Some(area(8, 8, 32, 224)),
                Some(area(680, 8, 32, 224))
            ]
        );
    }

    #[test]
    fn a_whole_cover_leaves_nothing() {
        assert_eq!(outside(area(4, 4, 8, 8), area(0, 0, 20, 20)), [None; 4]);
        assert_eq!(outside(area(4, 4, 8, 8), area(4, 4, 8, 8)), [None; 4]);
    }

    #[test]
    fn a_cover_beside_the_area_leaves_it_whole() {
        let a = area(10, 10, 5, 5);
        for cover in [
            area(0, 0, 10, 40),
            area(15, 0, 5, 40),
            area(0, 0, 40, 10),
            area(0, 15, 40, 3),
        ] {
            assert_eq!(outside(a, cover), [Some(a), None, None, None], "{cover:?}");
        }
    }

    #[test]
    fn every_overlap_is_a_partition() {
        let covers = [
            area(0, 0, 40, 40),
            area(5, 5, 10, 10),
            area(0, 8, 40, 4),
            area(8, 0, 4, 40),
            area(12, 12, 20, 20),
            area(0, 0, 13, 13),
            area(30, 30, 10, 10),
        ];
        for a in [area(3, 3, 20, 20), area(10, 0, 5, 40), area(0, 10, 40, 5)] {
            for cover in covers {
                assert_partition(a, cover);
            }
        }
    }

    #[test]
    fn a_quad_area_comes_from_its_whole_numbers() {
        assert_eq!(
            Area::of([8.0, 0.0, 704.0, 240.0]),
            Some(area(8, 0, 704, 240))
        );
        assert_eq!(Area::of([8.0, 0.0, 0.0, 240.0]), None);
    }
}
