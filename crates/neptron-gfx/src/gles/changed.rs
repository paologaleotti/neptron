//! The rows of a source image that changed since the texture got it.

use std::ops::Range;

use neptron_types::{Frame, PixelFormat};

/// A copy of the source rows that a texture holds, so that the next upload
/// of the same image sends only the rows that changed.
///
/// The UI canvas is the same in most frames: a menu or a dialog changes
/// only on input or once a second. An upload of the whole 320x240 canvas
/// takes about 0.6 ms on a Raspberry Pi 3, and a comparison of its rows
/// costs a fraction of that.
#[derive(Debug, Default)]
pub(super) struct Shown {
    /// Width, height and format of the image held, or `None` when the
    /// texture holds no known image.
    layout: Option<(u16, u16, PixelFormat)>,
    /// The rows held, packed: `width * bytes_per_pixel` bytes each.
    rows: Vec<u8>,
}

impl Shown {
    /// The rows of `frame` that differ from the rows held: every row when
    /// the layout differs or no image is held, `None` when no row differs.
    pub(super) fn changed(&self, frame: &Frame<'_>) -> Option<Range<u16>> {
        if self.layout != Some(layout(frame)) {
            return Some(0..frame.height);
        }
        let row_bytes = row_bytes(frame);
        let differs = |r: &u16| {
            let held = self
                .rows
                .get(usize::from(*r) * row_bytes..)
                .and_then(|h| h.get(..row_bytes));
            source_row(frame, *r, row_bytes).is_none_or(|row| held != Some(row))
        };
        let first = (0..frame.height).find(differs)?;
        // `first` differs, so the search from the end finds a row.
        let last = (first..frame.height).rev().find(differs).unwrap_or(first);
        Some(first..last + 1)
    }

    /// Records that the texture now holds `rows` of `frame`. A frame of a
    /// new layout must come with every row. The copy grows only when a
    /// larger image comes, so a steady image does not allocate.
    pub(super) fn record(&mut self, frame: &Frame<'_>, rows: Range<u16>) {
        let row_bytes = row_bytes(frame);
        if self.layout != Some(layout(frame)) {
            self.rows.clear();
            self.rows.resize(row_bytes * usize::from(frame.height), 0);
            self.layout = Some(layout(frame));
        }
        for r in rows {
            let start = usize::from(r) * row_bytes;
            let held = self.rows.get_mut(start..start + row_bytes);
            match (held, source_row(frame, r, row_bytes)) {
                (Some(held), Some(row)) => held.copy_from_slice(row),
                // A short frame: the next upload sends every row again.
                _ => {
                    self.forget();
                    return;
                }
            }
        }
    }

    /// Forgets the image: the next upload sends every row.
    pub(super) fn forget(&mut self) {
        self.layout = None;
    }
}

fn layout(frame: &Frame<'_>) -> (u16, u16, PixelFormat) {
    (frame.width, frame.height, frame.format)
}

fn row_bytes(frame: &Frame<'_>) -> usize {
    usize::from(frame.width) * frame.format.bytes_per_pixel()
}

/// The bytes of row `r` of `frame`, or `None` when the frame is too short.
fn source_row<'a>(frame: &Frame<'a>, r: u16, row_bytes: usize) -> Option<&'a [u8]> {
    let start = usize::from(r).checked_mul(frame.pitch)?;
    frame.data.get(start..start.checked_add(row_bytes)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    const WIDTH: u16 = 3;
    const HEIGHT: u16 = 4;
    const PITCH: usize = 14;

    /// A 3x4 XRGB8888 image with 2 bytes of padding after each row, and
    /// row `y` filled with the byte `values[y]`.
    fn image(values: [u8; HEIGHT as usize]) -> Vec<u8> {
        let mut data = vec![0xEE; PITCH * usize::from(HEIGHT)];
        for (y, value) in values.iter().enumerate() {
            data[y * PITCH..y * PITCH + 12].fill(*value);
        }
        data
    }

    fn frame(data: &[u8]) -> Frame<'_> {
        Frame {
            data,
            width: WIDTH,
            height: HEIGHT,
            pitch: PITCH,
            format: PixelFormat::Xrgb8888,
        }
    }

    fn shown(data: &[u8]) -> Shown {
        let mut shown = Shown::default();
        shown.record(&frame(data), 0..HEIGHT);
        shown
    }

    #[test]
    fn nothing_held_sends_every_row() {
        let data = image([1, 2, 3, 4]);
        assert_eq!(Shown::default().changed(&frame(&data)), Some(0..HEIGHT));
    }

    #[test]
    fn an_unchanged_image_sends_no_row() {
        let data = image([1, 2, 3, 4]);
        assert_eq!(shown(&data).changed(&frame(&data)), None);
    }

    #[test]
    fn the_padding_of_a_row_is_not_compared() {
        let held = image([1, 2, 3, 4]);
        let mut data = held.clone();
        data[12] = 0x00;
        data[PITCH + 13] = 0x11;
        assert_eq!(shown(&held).changed(&frame(&data)), None);
    }

    #[test]
    fn the_rows_from_the_first_to_the_last_change_are_sent() {
        let held = image([1, 2, 3, 4]);
        let one = image([1, 9, 3, 4]);
        assert_eq!(shown(&held).changed(&frame(&one)), Some(1..2));
        let two = image([1, 9, 3, 9]);
        assert_eq!(shown(&held).changed(&frame(&two)), Some(1..4));
        let mut pixel = held.clone();
        pixel[PITCH * 2 + 5] = 0;
        assert_eq!(shown(&held).changed(&frame(&pixel)), Some(2..3));
    }

    #[test]
    fn a_band_that_is_recorded_is_held() {
        let held = image([1, 2, 3, 4]);
        let next = image([1, 7, 8, 4]);
        let mut shown = shown(&held);
        let rows = shown.changed(&frame(&next)).expect("a change");
        shown.record(&frame(&next), rows);
        assert_eq!(shown.changed(&frame(&next)), None);
        assert_eq!(shown.changed(&frame(&held)), Some(1..3));
    }

    #[test]
    fn a_new_layout_sends_every_row() {
        let data = image([1, 2, 3, 4]);
        let shown = shown(&data);
        let narrow = Frame {
            width: 2,
            ..frame(&data)
        };
        assert_eq!(shown.changed(&narrow), Some(0..HEIGHT));
        let short = Frame {
            height: 3,
            ..frame(&data)
        };
        assert_eq!(shown.changed(&short), Some(0..3));
        let format = Frame {
            format: PixelFormat::Rgb565,
            ..frame(&data)
        };
        assert_eq!(shown.changed(&format), Some(0..HEIGHT));
    }

    #[test]
    fn a_forgotten_image_sends_every_row() {
        let data = image([1, 2, 3, 4]);
        let mut shown = shown(&data);
        shown.forget();
        assert_eq!(shown.changed(&frame(&data)), Some(0..HEIGHT));
    }

    #[test]
    fn a_frame_shorter_than_its_size_is_sent_whole_and_not_held() {
        let data = image([1, 2, 3, 4]);
        let cut = &data[..PITCH * 3];
        let mut shown = shown(&data);
        assert_eq!(shown.changed(&frame(cut)), Some(3..4));
        shown.record(&frame(cut), 0..HEIGHT);
        assert_eq!(shown.changed(&frame(&data)), Some(0..HEIGHT));
    }
}
