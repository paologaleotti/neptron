//! A contact sheet of glass pictures with lines of text under each: the
//! gate, the TV options and the sequence use it. The writer draws the
//! text with the UI font (see [`super::survey::Label`]).

use super::image::Rgb;
use super::survey::{LABEL_ADVANCE_PX, LABEL_LINE_PX, Label};

/// One cell of a sheet.
pub struct Cell<'a> {
    /// The picture, or `None` for no picture.
    pub picture: Option<&'a Rgb>,
    /// The lines of text under the picture, each with its colour.
    pub lines: Vec<(String, [u8; 3])>,
    /// A frame of this colour around the picture, or `None`.
    pub mark: Option<[u8; 3]>,
}

/// The gap between two cells, in pixels.
const GUTTER_PX: u32 = 8;
const BACKGROUND: [u8; 3] = [0x30, 0x30, 0x30];
const NO_PICTURE: [u8; 3] = [0x50, 0x10, 0x10];
const LEGEND_COLOR: [u8; 3] = [0xD0, 0xD0, 0xA0];

/// The legend of a sheet of the test card: what each part shows.
pub fn card_legend() -> Vec<String> {
    vec![
        "Red border: the edge of the frame. Circle: round when the shape is right under the reference named under the picture (ref: console = the width of the real console, ref: core 4:3 = the aspect the core gives).".to_owned(),
        "White corner mark: the top left of the game. On a PAL 50 Hz case, the bars above and below are the picture of today for a 60 Hz game.".to_owned(),
    ]
}

/// `cells` in rows of `columns`, each picture centred in a box of `tile`
/// pixels with `lines` lines of text under it, under the lines of
/// `legend`. Returns the sheet and the text to draw.
pub fn sheet(
    cells: &[Cell<'_>],
    columns: usize,
    tile: (u32, u32),
    lines: u32,
    legend: &[String],
) -> (Rgb, Vec<Label>) {
    let (tw, th) = tile;
    let cell_h = th + lines * LABEL_LINE_PX + 4;
    let columns = columns.max(1);
    let rows = cells.len().div_ceil(columns).max(1) as u32;
    let cols = columns.min(cells.len().max(1)) as u32;
    let top = legend.len() as u32 * LABEL_LINE_PX;
    let width = cols * tw + (cols + 1) * GUTTER_PX;
    let height = top + rows * cell_h + (rows + 1) * GUTTER_PX;
    let mut out = Rgb::new(width, height, BACKGROUND);
    let mut labels: Vec<Label> = legend
        .iter()
        .enumerate()
        .map(|(n, text)| Label {
            x: GUTTER_PX,
            y: GUTTER_PX / 2 + n as u32 * LABEL_LINE_PX,
            text: text
                .chars()
                .take(((width - GUTTER_PX) / LABEL_ADVANCE_PX) as usize)
                .collect(),
            color: LEGEND_COLOR,
        })
        .collect();
    let chars = (tw / LABEL_ADVANCE_PX) as usize;
    for (i, cell) in cells.iter().enumerate() {
        let x = GUTTER_PX + (i % columns) as u32 * (tw + GUTTER_PX);
        let y = top + GUTTER_PX + (i / columns) as u32 * (cell_h + GUTTER_PX);
        if let Some(color) = cell.mark {
            frame(&mut out, (x, y), tile, color);
        }
        match cell.picture {
            Some(p) => {
                let dx = tw.saturating_sub(p.width) / 2;
                let dy = th.saturating_sub(p.height) / 2;
                out.paste(p, x + dx, y + dy);
            }
            None => out.paste(&Rgb::new(tw, th, NO_PICTURE), x, y),
        }
        for (n, (text, color)) in cell.lines.iter().take(lines as usize).enumerate() {
            labels.push(Label {
                x,
                y: y + th + 3 + n as u32 * LABEL_LINE_PX,
                text: text.chars().take(chars).collect(),
                color: *color,
            });
        }
    }
    (out, labels)
}

/// A 2-pixel frame just outside the box at `at` of `size`.
fn frame(out: &mut Rgb, at: (u32, u32), size: (u32, u32), color: [u8; 3]) {
    let (x, y) = at;
    let (w, h) = size;
    for d in 1..=2 {
        for i in 0..w + 2 * d {
            out.put(x + i - d, y - d, color);
            out.put(x + i - d, y + h + d - 1, color);
        }
        for j in 0..h + 2 * d {
            out.put(x - d, y + j - d, color);
            out.put(x + w + d - 1, y + j - d, color);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sheet_places_its_cells_and_their_text() {
        let p = Rgb::new(10, 10, [200; 3]);
        let cells = [
            Cell {
                picture: Some(&p),
                lines: vec![("one".into(), [1; 3]), ("two".into(), [2; 3])],
                mark: Some([9; 3]),
            },
            Cell {
                picture: None,
                lines: vec![("three".into(), [3; 3])],
                mark: None,
            },
        ];
        let (img, labels) = sheet(&cells, 2, (20, 20), 2, &[]);
        assert_eq!(img.width, 2 * 20 + 3 * GUTTER_PX);
        assert_eq!(img.get(GUTTER_PX + 5, GUTTER_PX + 5), [200; 3]);
        assert_eq!(img.get(GUTTER_PX - 1, GUTTER_PX), [9; 3]);
        assert_eq!(labels.len(), 3);
        assert_eq!(labels[2].text, "thr", "20 pixels hold 3 characters");
    }
}
