//! The UI font: Spleen 6x12 by Frederic Cambus (BSD-2-Clause), printable
//! ASCII and the Latin-1 Supplement. Capitals and digits are 8 pixels tall.
//!
//! `cargo xtask font` generates the glyph table from the BDF file in
//! `assets/fonts/spleen`; that directory has the licence and the source.
//! To change the font, change the generator and the module below.

mod spleen_6x12;

use spleen_6x12 as table;

/// Width of a glyph box in pixels.
pub const GLYPH_WIDTH: u32 = table::WIDTH;
/// Height of a glyph box in pixels, descenders included.
pub const GLYPH_HEIGHT: u32 = table::HEIGHT as u32;
/// Horizontal distance from one glyph to the next, in pixels. The glyph box
/// already holds the space between glyphs.
pub const ADVANCE: u32 = table::WIDTH;
/// Rows of the glyph box above the baseline.
pub const ASCENT: u32 = table::ASCENT;
/// Height of capitals and digits in pixels.
pub const CAP_HEIGHT: u32 = 8;

/// The character drawn for every character that the font does not have.
pub const REPLACEMENT: char = '?';

/// The rows of one glyph, top first. Bit 7 is the left column.
pub type Glyph = [u8; table::HEIGHT];

/// The glyph of `c`, or the glyph of [`REPLACEMENT`] when the font does not
/// have `c`.
pub fn glyph(c: char) -> &'static Glyph {
    lookup(c).or_else(|| lookup(REPLACEMENT)).unwrap_or(&BLANK)
}

/// True when the font has a glyph for `c`.
pub fn has_glyph(c: char) -> bool {
    lookup(c).is_some()
}

/// The width of `text` in pixels: one advance per character.
pub fn text_width(text: &str) -> u32 {
    char_count_width(text.chars().count())
}

/// The width of `count` characters in pixels.
pub fn char_count_width(count: usize) -> u32 {
    u32::try_from(count)
        .unwrap_or(u32::MAX)
        .saturating_mul(ADVANCE)
}

/// How many characters fit in `width` pixels.
pub fn chars_in(width: u32) -> usize {
    (width / ADVANCE) as usize
}

static BLANK: Glyph = [0; table::HEIGHT];

fn lookup(c: char) -> Option<&'static Glyph> {
    let code = u32::from(c);
    let index = table::GLYPHS.binary_search_by_key(&code, |g| g.0).ok()?;
    table::GLYPHS.get(index).map(|g| &g.1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ink(g: &Glyph) -> u32 {
        g.iter().map(|row| row.count_ones()).sum()
    }

    fn printable() -> impl Iterator<Item = char> {
        (' '..='~').chain('\u{a0}'..='\u{ff}')
    }

    #[test]
    fn ascii_and_latin_1_have_glyphs() {
        for c in printable() {
            assert!(has_glyph(c), "{c:?}");
        }
        for c in printable().filter(|c| !matches!(c, ' ' | '\u{a0}' | '\u{ad}')) {
            assert!(ink(glyph(c)) > 0, "{c:?} has no ink");
        }
        assert_eq!(ink(glyph(' ')), 0);
    }

    #[test]
    fn ascii_glyphs_are_distinct() {
        let all: Vec<(char, &Glyph)> = (' '..='~').map(|c| (c, glyph(c))).collect();
        for (i, (a, ga)) in all.iter().enumerate() {
            for (b, gb) in &all[i + 1..] {
                assert_ne!(ga, gb, "{a:?} and {b:?} look the same");
            }
        }
    }

    #[test]
    fn glyphs_stay_inside_the_box() {
        let outside = !(0xFF_u8 << (8 - GLYPH_WIDTH));
        for c in printable() {
            assert!(glyph(c).iter().all(|row| row & outside == 0), "{c:?}");
        }
    }

    #[test]
    fn capitals_and_digits_stand_on_the_baseline() {
        for c in ('A'..='Z').chain('0'..='9') {
            let g = glyph(c);
            assert_ne!(
                g[ASCENT as usize - 1],
                0,
                "{c:?} does not reach the baseline"
            );
            let top = g.iter().position(|r| *r != 0).unwrap_or(g.len());
            assert!(
                ASCENT as usize - top >= CAP_HEIGHT as usize,
                "{c:?} is under 8 pixels tall"
            );
        }
    }

    #[test]
    fn descenders_go_below_the_baseline() {
        for c in ['g', 'j', 'p', 'q', 'y'] {
            assert!(glyph(c)[ASCENT as usize..].iter().any(|r| *r != 0), "{c:?}");
        }
    }

    #[test]
    fn unknown_characters_draw_the_replacement() {
        assert!(has_glyph('\u{e9}'));
        assert!(!has_glyph('\u{3a9}'));
        assert!(!has_glyph('\n'));
        assert_eq!(glyph('\u{3a9}'), glyph(REPLACEMENT));
    }

    #[test]
    fn widths_count_characters_not_bytes() {
        assert_eq!(text_width(""), 0);
        assert_eq!(text_width("ab"), 12);
        assert_eq!(text_width("\u{e9}\u{e9}"), 12);
        assert_eq!(chars_in(17), 2);
        assert_eq!(char_count_width(usize::MAX), u32::MAX);
    }
}
