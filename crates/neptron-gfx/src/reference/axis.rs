use neptron_types::Filter;

/// Where one destination pixel samples on one axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tap {
    /// The sample point is outside the source frame. The pixel is black.
    Outside,
    /// The colour is `(near * (den - weight) + far * weight) / den`, with
    /// `den` from [`AxisMap::denominator`]. `near` and `far` are source
    /// coordinates inside the frame.
    Inside { near: u32, far: u32, weight: u64 },
}

/// The map from destination pixels to source pixels on one axis.
///
/// The caller makes sure that `crop_len` and `dest_len` are between 1 and
/// [`MAX_RECT_SIDE_PX`](super::MAX_RECT_SIDE_PX). Every product below then
/// stays below 2^50.
#[derive(Debug, Clone, Copy)]
pub(crate) struct AxisMap {
    pub(crate) crop_start: i64,
    pub(crate) crop_len: u32,
    pub(crate) dest_len: u32,
    pub(crate) frame_len: u32,
    pub(crate) filter: Filter,
}

impl AxisMap {
    /// The denominator of every [`Tap::Inside`] weight of this axis.
    pub(crate) fn denominator(&self) -> u64 {
        match self.filter {
            Filter::Nearest => 1,
            Filter::SharpBilinear => 4 * u64::from(self.dest_len),
        }
    }

    /// The tap of destination pixel `i`, counted from the start of the
    /// destination rectangle. `i` is below `dest_len`.
    pub(crate) fn tap(&self, i: u32) -> Tap {
        let n = u64::from(self.crop_len);
        let m = u64::from(self.dest_len);
        // The centre of destination pixel i maps to the source coordinate
        // crop_start + num / den.
        let num = (2 * u64::from(i) + 1) * n;
        let den = 2 * m;
        let whole = num / den;
        let rem = num % den;

        let frame_len = i64::from(self.frame_len);
        let near = self.crop_start + whole as i64;
        if near < 0 || near >= frame_len {
            return Tap::Outside;
        }
        let near_u32 = near as u32;
        match self.filter {
            Filter::Nearest => Tap::Inside {
                near: near_u32,
                far: near_u32,
                weight: 0,
            },
            Filter::SharpBilinear => {
                let prescale = (m / n).max(1);
                let dist = (2 * rem).abs_diff(den);
                let weight = (dist * prescale).saturating_sub((prescale - 1) * den);
                let first = self.crop_start.max(0);
                let last = (self.crop_start + n as i64).min(frame_len) - 1;
                let far = if 2 * rem < den { near - 1 } else { near + 1 };
                Tap::Inside {
                    near: near_u32,
                    far: far.clamp(first, last) as u32,
                    weight,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(
        crop_start: i64,
        crop_len: u32,
        dest_len: u32,
        frame_len: u32,
        filter: Filter,
    ) -> AxisMap {
        AxisMap {
            crop_start,
            crop_len,
            dest_len,
            frame_len,
            filter,
        }
    }

    fn nearest(m: &AxisMap) -> Vec<Option<u32>> {
        (0..m.dest_len)
            .map(|i| match m.tap(i) {
                Tap::Outside => None,
                Tap::Inside { near, far, weight } => {
                    assert_eq!((far, weight), (near, 0));
                    Some(near)
                }
            })
            .collect()
    }

    #[test]
    fn nearest_integer_upscale_repeats_each_pixel() {
        let m = map(0, 3, 9, 3, Filter::Nearest);
        let expected = [0, 0, 0, 1, 1, 1, 2, 2, 2].map(Some);
        assert_eq!(nearest(&m), expected);
    }

    #[test]
    fn nearest_fractional_upscale_uses_pixel_centres() {
        // Centres of 5 pixels over 2 source pixels: 0.2, 0.6, 1.0, 1.4, 1.8.
        // A centre on a pixel edge (1.0) goes to the pixel on the right.
        let m = map(0, 2, 5, 2, Filter::Nearest);
        assert_eq!(nearest(&m), [0, 0, 1, 1, 1].map(Some));
    }

    #[test]
    fn nearest_downscale_by_two_takes_the_right_pixel_of_each_pair() {
        // Centres at 1.0, 3.0 and 5.0: each on an edge.
        let m = map(0, 6, 3, 6, Filter::Nearest);
        assert_eq!(nearest(&m), [1, 3, 5].map(Some));
    }

    #[test]
    fn crop_start_offsets_the_source() {
        let m = map(4, 2, 4, 10, Filter::Nearest);
        assert_eq!(nearest(&m), [4, 4, 5, 5].map(Some));
    }

    #[test]
    fn samples_outside_the_frame_are_outside() {
        let m = map(-1, 4, 4, 2, Filter::Nearest);
        assert_eq!(nearest(&m), [None, Some(0), Some(1), None]);
        let m = map(0, 4, 4, 0, Filter::Nearest);
        assert_eq!(nearest(&m), [None; 4]);
    }

    #[test]
    fn sharp_bilinear_at_an_integer_scale_is_nearest() {
        for scale in 1..6 {
            let sharp = map(0, 7, 7 * scale, 7, Filter::SharpBilinear);
            let plain = map(0, 7, 7 * scale, 7, Filter::Nearest);
            for i in 0..7 * scale {
                let Tap::Inside { near, weight, .. } = sharp.tap(i) else {
                    panic!("pixel {i} at scale {scale} is outside");
                };
                assert_eq!(weight, 0, "pixel {i} at scale {scale}");
                assert_eq!(
                    Tap::Inside {
                        near,
                        far: near,
                        weight
                    },
                    plain.tap(i)
                );
            }
        }
    }

    #[test]
    fn sharp_bilinear_blends_only_across_a_pixel_edge() {
        // 2 source pixels over 5: scale 2.5, prescale 2. Centres at 0.2,
        // 0.6, 1.0, 1.4, 1.8. Weight = max(0, |frac - 1/2| * 2 - 1/2), over
        // a denominator of 20.
        let m = map(0, 2, 5, 2, Filter::SharpBilinear);
        assert_eq!(m.denominator(), 20);
        let taps: Vec<Tap> = (0..5).map(|i| m.tap(i)).collect();
        assert_eq!(
            taps,
            [
                // frac 0.2: |-0.3| * 2 - 0.5 = 0.1, toward pixel -1, clamped.
                Tap::Inside {
                    near: 0,
                    far: 0,
                    weight: 2
                },
                // frac 0.6: 0.1 * 2 - 0.5 < 0.
                Tap::Inside {
                    near: 0,
                    far: 1,
                    weight: 0
                },
                // frac 0.0: 0.5 * 2 - 0.5 = 0.5, toward pixel 0.
                Tap::Inside {
                    near: 1,
                    far: 0,
                    weight: 10
                },
                Tap::Inside {
                    near: 1,
                    far: 0,
                    weight: 0
                },
                Tap::Inside {
                    near: 1,
                    far: 1,
                    weight: 2
                },
            ]
        );
    }

    #[test]
    fn sharp_bilinear_downscale_is_bilinear() {
        // 3 source pixels over 2: centres at 0.75 and 2.25.
        let m = map(0, 3, 2, 3, Filter::SharpBilinear);
        assert_eq!(m.denominator(), 8);
        assert_eq!(
            m.tap(0),
            Tap::Inside {
                near: 0,
                far: 1,
                weight: 2
            }
        );
        assert_eq!(
            m.tap(1),
            Tap::Inside {
                near: 2,
                far: 1,
                weight: 2
            }
        );
    }

    #[test]
    fn sharp_bilinear_taps_stay_inside_the_crop() {
        // Crop [2, 4) of a 6 pixel frame at scale 1.5. The first and last
        // pixels blend toward pixels 1 and 4, which are outside the crop.
        let m = map(2, 2, 3, 6, Filter::SharpBilinear);
        for i in 0..3 {
            let Tap::Inside { near, far, .. } = m.tap(i) else {
                panic!("pixel {i} is outside");
            };
            assert!((2..4).contains(&near) && (2..4).contains(&far), "pixel {i}");
        }
    }

    #[test]
    fn sharp_bilinear_taps_stay_inside_the_frame() {
        let m = map(-2, 6, 9, 3, Filter::SharpBilinear);
        for i in 0..9 {
            if let Tap::Inside { near, far, .. } = m.tap(i) {
                assert!(near < 3 && far < 3, "pixel {i}");
            }
        }
    }

    #[test]
    fn weight_is_at_most_half() {
        for (n, m) in [
            (7, 9),
            (9, 7),
            (256, 1920),
            (320, 1080),
            (1, 65_536),
            (65_536, 1),
        ] {
            let axis = map(0, n, m, n, Filter::SharpBilinear);
            let den = axis.denominator();
            for i in [0, m / 3, m / 2, m - 1] {
                let Tap::Inside { weight, .. } = axis.tap(i) else {
                    panic!("pixel {i} of {n} to {m} is outside");
                };
                assert!(2 * weight <= den, "pixel {i} of {n} to {m}");
            }
        }
    }
}
