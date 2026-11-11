use std::fmt::Write as _;

use crate::RgbImage;

/// The most pixels that a report lists one by one.
const LISTED_PIXELS: u32 = 10;

/// A report of how `actual` differs from `expected`, or `None` when the
/// two images are equal.
///
/// The report gives the count of pixels that differ, the box around them,
/// the largest difference of one channel, and the first pixels that differ
/// with both values.
pub fn difference(expected: &RgbImage, actual: &RgbImage) -> Option<String> {
    if (expected.width(), expected.height()) != (actual.width(), actual.height()) {
        return Some(format!(
            "size {}x{} differs from the golden size {}x{}",
            actual.width(),
            actual.height(),
            expected.width(),
            expected.height()
        ));
    }
    let mut listed = String::new();
    let mut count: u64 = 0;
    let mut max_channel = 0;
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (u32::MAX, u32::MAX, 0, 0);
    let width = u64::from(actual.width().max(1));
    let pixels = expected
        .bytes()
        .chunks_exact(3)
        .zip(actual.bytes().chunks_exact(3));
    for (i, (e, a)) in (0_u64..).zip(pixels) {
        if e == a {
            continue;
        }
        // The pixel index is below width * height, so both values fit in u32.
        let (x, y) = ((i % width) as u32, (i / width) as u32);
        count += 1;
        (min_x, min_y) = (min_x.min(x), min_y.min(y));
        (max_x, max_y) = (max_x.max(x), max_y.max(y));
        for (ec, ac) in e.iter().zip(a) {
            max_channel = max_channel.max(ec.abs_diff(*ac));
        }
        if count <= u64::from(LISTED_PIXELS) {
            // Writing to a String cannot fail.
            let _ = writeln!(
                listed,
                "  ({x}, {y}): golden #{:02X}{:02X}{:02X}, actual #{:02X}{:02X}{:02X}",
                e[0], e[1], e[2], a[0], a[1], a[2]
            );
        }
    }
    (count > 0).then(|| {
        format!(
            "{count} pixels differ, in x {min_x} to {max_x} and y {min_y} to {max_y}; \
             largest channel difference {max_channel}\n{listed}"
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn black(width: u32, height: u32) -> RgbImage {
        RgbImage::from_rgb(width, height, vec![0; (width * height * 3) as usize]).unwrap()
    }

    #[test]
    fn equal_images_have_no_report() {
        assert_eq!(difference(&black(2, 2), &black(2, 2)), None);
    }

    #[test]
    fn the_report_names_the_pixels() {
        let mut bytes = vec![0; 12];
        bytes[9] = 3;
        let actual = RgbImage::from_rgb(2, 2, bytes).unwrap();
        let report = difference(&black(2, 2), &actual).unwrap();
        assert_eq!(
            report,
            "1 pixels differ, in x 1 to 1 and y 1 to 1; largest channel difference 3\n  \
             (1, 1): golden #000000, actual #030000\n"
        );
    }

    #[test]
    fn the_report_lists_ten_pixels_at_most() {
        let actual = RgbImage::from_rgb(4, 4, vec![9; 48]).unwrap();
        let report = difference(&black(4, 4), &actual).unwrap();
        assert!(report.starts_with("16 pixels differ, in x 0 to 3 and y 0 to 3;"));
        assert_eq!(report.lines().count(), 11);
    }

    #[test]
    fn a_size_change_is_reported() {
        assert_eq!(
            difference(&black(2, 2), &black(3, 2)).as_deref(),
            Some("size 3x2 differs from the golden size 2x2")
        );
    }
}
