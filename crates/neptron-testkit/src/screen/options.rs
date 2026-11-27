//! The TV options side by side, for the owner to decide OP-122: for a few
//! sources, the picture of today and candidate placements on an NTSC and
//! a PAL TV. A candidate is drawn with a placement given by hand
//! ([`super::redraw`]); the product has no code path for it.

use neptron_types::{Filter, Rect, TvStandard};

use super::gate::SHAPE_MAX_PERMILLE;
use super::image::{Rgb, Turn};
use super::measure::{Measures, milli_text, scan_letter};
use super::redraw;
use super::run::{
    GlassOptions, Outcome, RunError, Tables, finish, measure_card, simulate_with_reference_card,
};
use super::sheet::{Cell, card_legend, sheet};
use super::survey::{LABEL_COLOR, Label, NOTE_COLOR};
use neptron_video::TV_SAFE_WIDTH_SAMPLES;

/// The sources of the option sheets: a 320x240 game at 60 Hz, a 224-line
/// game, a 256-line board at 55 Hz and a 640x480 interlaced game.
pub const OPTION_SOURCES: [&str; 4] = ["ps1_320", "snes_256", "rtype", "ps1_480i"];

/// A candidate placement on a TV.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TvOption {
    /// The picture of the product today.
    Today,
    /// The lines of today at the full 4:3 width of the standard: 704
    /// samples.
    FullWidth,
    /// On a PAL TV that takes a 60 Hz signal: the 525-line mode of today
    /// with PAL colour, so the lines fill the height at the game's rate.
    Pal60,
    /// [`Self::Pal60`] at the full 4:3 width.
    Pal60FullWidth,
}

/// One cell of an option sheet.
#[derive(Debug)]
pub struct OptionCell {
    /// The source.
    pub source: String,
    /// The candidate.
    pub option: TvOption,
    /// Its outcome.
    pub outcome: Outcome,
}

impl TvOption {
    /// The candidates of a standard.
    pub fn of(standard: TvStandard) -> &'static [Self] {
        if standard.lines() == 525 {
            &[Self::Today, Self::FullWidth]
        } else {
            &[
                Self::Today,
                Self::FullWidth,
                Self::Pal60,
                Self::Pal60FullWidth,
            ]
        }
    }

    /// The name on the sheet.
    pub fn name(self) -> &'static str {
        match self {
            Self::Today => "today",
            Self::FullWidth => "full 4:3 width",
            Self::Pal60 => "60 Hz, 525 lines",
            Self::Pal60FullWidth => "60 Hz, 525 lines, full 4:3 width",
        }
    }
}

/// The cells of the option sheet of `standard`, a glass of `tile` pixels
/// each, with the card measured on the glass.
///
/// # Errors
///
/// [`RunError`] when a case does not run.
pub fn option_cells(
    tables: &Tables,
    standard: TvStandard,
    tile: (u32, u32),
) -> Result<Vec<OptionCell>, RunError> {
    let options = GlassOptions {
        overscan_permille: None,
        size: tile,
        turn: Turn::R0,
    };
    let mut cells = Vec::new();
    for source in OPTION_SOURCES {
        for &option in TvOption::of(standard) {
            // A 60 Hz mode with PAL colour has the geometry of the 525-line
            // system: the NTSC TV of the simulator.
            let display = match option {
                TvOption::Pal60 | TvOption::Pal60FullWidth => TvStandard::Ntsc.key(),
                TvOption::Today | TvOption::FullWidth => standard.key(),
            };
            let mut case = tables.named_case(display, source, &[])?;
            let (mut outcome, frame) = simulate_with_reference_card(&mut case, &options)?;
            if matches!(option, TvOption::FullWidth | TvOption::Pal60FullWidth) {
                let orientation = case.settings.orientation()?;
                let mut wire = outcome.wire;
                if let Some(shown) = &mut wire.shown {
                    let dest = full_width(shown.scene.dest, shown.mode.hactive);
                    shown.scene.dest = dest;
                    shown.scene.filter = filter_for(shown.scene.crop, dest);
                    shown.frame_buffer = redraw::draw(&shown.mode, &frame, &shown.scene)?;
                }
                outcome = finish(&case, &frame, &options, orientation, wire);
            }
            measure_card(&case, &frame, None, &mut outcome);
            cells.push(OptionCell {
                source: source.to_owned(),
                option,
                outcome,
            });
        }
    }
    Ok(cells)
}

/// `dest` at the full 4:3 width of a TV line of `hactive` samples,
/// centred.
fn full_width(dest: Rect, hactive: u16) -> Rect {
    let w = u32::from(TV_SAFE_WIDTH_SAMPLES.min(hactive));
    Rect {
        x: i32::try_from((u32::from(hactive) - w) / 2).unwrap_or(0),
        w,
        ..dest
    }
}

/// Nearest where both axes scale by whole steps, else sharp bilinear: the
/// rule of the renderer of the product.
fn filter_for(crop: Rect, dest: Rect) -> Filter {
    let whole = |from: u32, to: u32| from > 0 && to.is_multiple_of(from);
    if whole(crop.w, dest.w) && whole(crop.h, dest.h) {
        Filter::Nearest
    } else {
        Filter::SharpBilinear
    }
}

/// The option sheet: one row per source, one column per candidate.
pub fn option_sheet(cells: &[OptionCell], tile: (u32, u32)) -> (Rgb, Vec<Label>) {
    let columns = cells.iter().filter(|c| c.source == cells[0].source).count();
    let list: Vec<Cell<'_>> = cells
        .iter()
        .map(|c| Cell {
            picture: c.outcome.glass_image.as_ref(),
            lines: option_lines(c)
                .into_iter()
                .enumerate()
                .map(|(i, l)| (l, if i == 0 { NOTE_COLOR } else { LABEL_COLOR }))
                .collect(),
            mark: None,
        })
        .collect();
    sheet(&list, columns, tile, 4, &card_legend())
}

/// The label of a cell: the source and the candidate; the mode; the shape
/// against both references; the lines cut and hidden and the speed.
pub fn option_lines(c: &OptionCell) -> Vec<String> {
    let m: &Measures = &c.outcome.measures;
    let Some(p) = &m.picture else {
        return vec![
            format!("{} {}", c.source, c.option.name()),
            "no picture".into(),
        ];
    };
    let rate = milli_text(p.mode.field_rate_mhz);
    let shape = p.glass_shape.map_or("ref: no card read".to_owned(), |s| {
        let error = s.verdict_error_permille();
        let judged = if error.abs() <= SHAPE_MAX_PERMILLE {
            "within 1 %"
        } else {
            "OFF"
        };
        match s.console {
            Some(_) => format!(
                "ref: console {} {judged} (core 4:3 {})",
                signed(error),
                signed(s.core_error_permille)
            ),
            None => format!("ref: core 4:3 {} {judged}", signed(error)),
        }
    });
    let hidden = p.overscan_hidden.unwrap_or_default();
    vec![
        format!("{} {}", c.source, c.option.name()),
        format!(
            "{}x{}{} {}Hz  dest {}x{}",
            p.mode.hactive,
            p.mode.vactive,
            scan_letter(p.mode.interlace, p.mode.doublescan),
            &rate[..rate.len() - 1],
            p.dest.w,
            p.dest.h
        ),
        shape,
        format!(
            "cut {} lines, overscan hides {}; speed {}",
            p.cropped.top + p.cropped.bottom,
            hidden.top + hidden.bottom,
            signed(p.mode_speed_ppm / 1000)
        ),
    ]
}

/// Per mille as a signed percent.
fn signed(permille: i64) -> String {
    let sign = if permille < 0 { "-" } else { "+" };
    let a = permille.abs();
    format!("{sign}{}.{}%", a / 10, a % 10)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::golden::workspace_root;

    fn shape(c: &OptionCell) -> i64 {
        let p = c.outcome.measures.picture.as_ref().unwrap();
        p.glass_shape.unwrap().core_error_permille
    }

    fn find<'a>(cells: &'a [OptionCell], source: &str, option: TvOption) -> &'a OptionCell {
        cells
            .iter()
            .find(|c| c.source == source && c.option == option)
            .unwrap()
    }

    /// On NTSC the full 4:3 width gives a 240-line game the shape of the
    /// core; today it is 9.1 % narrow. On PAL the 60 Hz mode keeps the
    /// speed of the game and the NTSC shape.
    #[test]
    fn the_options_place_the_picture_as_they_say() {
        let tables = Tables::load(&workspace_root()).unwrap();
        let ntsc = option_cells(&tables, TvStandard::Ntsc, (64, 48)).unwrap();
        assert_eq!(ntsc.len(), 2 * OPTION_SOURCES.len());
        assert_eq!(shape(find(&ntsc, "ps1_320", TvOption::Today)), -91);
        assert!(shape(find(&ntsc, "ps1_320", TvOption::FullWidth)).abs() <= 3);
        assert!(shape(find(&ntsc, "ps1_480i", TvOption::FullWidth)).abs() <= 3);
        let pal = option_cells(&tables, TvStandard::Pal, (64, 48)).unwrap();
        assert_eq!(pal.len(), 4 * OPTION_SOURCES.len());
        let today = find(&pal, "snes_256", TvOption::Today);
        let pal60 = find(&pal, "snes_256", TvOption::Pal60);
        let speed = |c: &OptionCell| c.outcome.measures.picture.as_ref().unwrap().mode_speed_ppm;
        assert!(speed(today) < -160_000, "{}", speed(today));
        assert!(speed(pal60).abs() < 5_000, "{}", speed(pal60));
        assert_eq!(
            shape(pal60),
            shape(find(&ntsc, "snes_256", TvOption::Today))
        );
    }

    #[test]
    fn full_width_is_704_samples_centred() {
        let dest = Rect {
            x: 40,
            y: 8,
            w: 640,
            h: 224,
        };
        assert_eq!(
            full_width(dest, 720),
            Rect {
                x: 8,
                y: 8,
                w: 704,
                h: 224
            }
        );
        let crop = Rect {
            x: 0,
            y: 0,
            w: 352,
            h: 224,
        };
        assert_eq!(filter_for(crop, full_width(dest, 720)), Filter::Nearest);
        assert_eq!(TvOption::of(TvStandard::Ntsc).len(), 2);
        assert_eq!(TvOption::of(TvStandard::Pal).len(), 4);
    }
}
