//! Text layout with the built-in font: fitting into a box, alignment and
//! word wrap. Nothing here allocates.

use core::fmt;

use crate::font;
use crate::geom::{Color, Rect};
use crate::scene::UiScene;
use crate::snapshot::{ChoiceOption, SettingValue};

/// The text drawn after text that the layout cut.
pub const ELLIPSIS: &str = "...";

/// Horizontal alignment in a box.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum Align {
    /// Against the left edge.
    #[default]
    Left,
    /// In the middle. An odd spare pixel goes to the right.
    Center,
    /// Against the right edge.
    Right,
}

/// The part of a text that fits into a width.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fit<'a> {
    /// The characters to draw.
    pub shown: &'a str,
    /// True when [`ELLIPSIS`] follows `shown`.
    pub ellipsis: bool,
    /// True when `shown` is not the whole text.
    pub clipped: bool,
}

/// Cuts `text` to fit `width_px`. A cut text ends with [`ELLIPSIS`] when
/// the width has room for it; spaces before the ellipsis are dropped.
pub fn fit(text: &str, width_px: u32) -> Fit<'_> {
    let room = font::chars_in(width_px);
    let count = text.chars().count();
    if count <= room {
        return Fit {
            shown: text,
            ellipsis: false,
            clipped: false,
        };
    }
    let dots = ELLIPSIS.len();
    let (keep, ellipsis) = if room >= dots {
        (room - dots, true)
    } else {
        (room, false)
    };
    let shown = prefix(text, keep);
    Fit {
        shown: if ellipsis {
            shown.trim_end_matches(' ')
        } else {
            shown
        },
        ellipsis,
        clipped: true,
    }
}

/// Draws `text` in `rect`, cut to fit, aligned, and centred vertically.
/// Returns true when the text was cut.
pub fn draw_in(scene: &mut UiScene, rect: Rect, align: Align, color: Color, text: &str) -> bool {
    let fitted = fit(text, rect.w);
    let dots = if fitted.ellipsis {
        font::text_width(ELLIPSIS)
    } else {
        0
    };
    let width = font::text_width(fitted.shown).saturating_add(dots);
    let spare = rect.w.saturating_sub(width);
    let dx = match align {
        Align::Left => 0,
        Align::Center => spare / 2,
        Align::Right => spare,
    };
    let x = offset(rect.x, dx);
    let y = offset(rect.y, rect.h.saturating_sub(font::GLYPH_HEIGHT) / 2);
    scene.text_run(x, y, color, fitted.shown);
    if fitted.ellipsis {
        let after = offset(x, font::text_width(fitted.shown));
        scene.text_run(after, y, color, ELLIPSIS);
    }
    fitted.clipped
}

/// Splits `text` into lines of at most `width_px`, at spaces where it can
/// and inside a word where it must. A `\n` always ends a line. Each line is
/// a slice of `text` without the spaces at its end. A line that starts
/// after a `\n` keeps its leading spaces, so a message can indent a line;
/// a line that starts after a break at a space does not.
pub fn wrap(text: &str, width_px: u32) -> Wrap<'_> {
    Wrap {
        rest: text,
        room: font::chars_in(width_px).max(1),
        after_space: false,
    }
}

/// The lines of [`wrap`].
#[derive(Clone, Debug)]
pub struct Wrap<'a> {
    rest: &'a str,
    room: usize,
    after_space: bool,
}

impl<'a> Iterator for Wrap<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<&'a str> {
        if self.rest.trim_start_matches(' ').is_empty() {
            return None;
        }
        let rest = if self.after_space {
            self.rest.trim_start_matches(' ')
        } else {
            self.rest
        };
        let (line, next, at_newline) = take_line(rest, self.room);
        self.rest = next;
        self.after_space = !at_newline;
        Some(line.trim_end_matches(' '))
    }
}

/// A short text in a fixed buffer, for formatted values that must not
/// allocate. Text past the capacity is dropped at a character boundary.
#[derive(Clone, Copy, Debug)]
pub struct StackText<const N: usize> {
    bytes: [u8; N],
    len: usize,
}

impl<const N: usize> StackText<N> {
    /// An empty text.
    pub const fn new() -> Self {
        Self {
            bytes: [0; N],
            len: 0,
        }
    }

    /// The text.
    pub fn as_str(&self) -> &str {
        // Only whole characters are copied in, so the bytes are UTF-8.
        core::str::from_utf8(self.bytes.get(..self.len).unwrap_or(&[])).unwrap_or("")
    }

    /// Removes the text.
    pub fn clear(&mut self) {
        self.len = 0;
    }
}

impl<const N: usize> PartialEq for StackText<N> {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}

impl<const N: usize> Eq for StackText<N> {}

impl<const N: usize> Default for StackText<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> fmt::Write for StackText<N> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for c in s.chars() {
            let mut utf8 = [0; 4];
            let encoded = c.encode_utf8(&mut utf8).as_bytes();
            let end = self.len + encoded.len();
            let Some(slot) = self.bytes.get_mut(self.len..end) else {
                return Ok(());
            };
            slot.copy_from_slice(encoded);
            self.len = end;
        }
        Ok(())
    }
}

/// The text that shows a setting value. A number goes into `buf`.
pub fn value_text<'a, const N: usize>(
    value: &'a SettingValue,
    buf: &'a mut StackText<N>,
) -> &'a str {
    match value {
        SettingValue::Toggle(true) => "On",
        SettingValue::Toggle(false) => "Off",
        SettingValue::Number { value, unit, .. } => {
            buf.clear();
            // `StackText` never returns an error; it drops what does not fit.
            let _ = if unit.is_empty() {
                fmt::Write::write_fmt(buf, format_args!("{value}"))
            } else {
                fmt::Write::write_fmt(buf, format_args!("{value} {unit}"))
            };
            buf.as_str()
        }
        SettingValue::Choice { index, options } => {
            options.get(*index).map_or("?", ChoiceOption::value_text)
        }
        SettingValue::Action(_) | SettingValue::Page(_) => "",
    }
}

fn prefix(text: &str, chars: usize) -> &str {
    match text.char_indices().nth(chars) {
        Some((at, _)) => text.get(..at).unwrap_or(text),
        None => text,
    }
}

/// The first line of `text` that fits `room` characters, the text after
/// it, and true when a `\n` ended the line.
fn take_line(text: &str, room: usize) -> (&str, &str, bool) {
    let mut last_space = None;
    for (count, (at, c)) in text.char_indices().enumerate() {
        if c == '\n' {
            return (&text[..at], &text[at + 1..], true);
        }
        if count == room {
            if c == ' ' {
                return (&text[..at], &text[at..], false);
            }
            return match last_space {
                Some(space) => (&text[..space], &text[space..], false),
                None => (&text[..at], &text[at..], false),
            };
        }
        if c == ' ' {
            last_space = Some(at);
        }
    }
    (text, "", false)
}

fn offset(x: i32, by: u32) -> i32 {
    i32::try_from(i64::from(x) + i64::from(by)).unwrap_or(i32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::fmt::Write;

    #[test]
    fn short_text_is_not_cut() {
        let f = fit("Home", 24);
        assert_eq!((f.shown, f.ellipsis, f.clipped), ("Home", false, false));
    }

    #[test]
    fn long_text_ends_with_an_ellipsis() {
        let f = fit("Super Mario World", 60);
        assert_eq!((f.shown, f.ellipsis, f.clipped), ("Super M", true, true));
    }

    #[test]
    fn no_space_comes_before_the_ellipsis() {
        let f = fit("Ganbare Goemon", 66);
        assert_eq!((f.shown, f.ellipsis, f.clipped), ("Ganbare", true, true));
        assert_eq!(fit("a          b", 36).shown, "a");
    }

    #[test]
    fn a_tiny_box_cuts_without_an_ellipsis() {
        let f = fit("Hello", 12);
        assert_eq!((f.shown, f.ellipsis, f.clipped), ("He", false, true));
        assert_eq!(fit("Hello", 0).shown, "");
    }

    #[test]
    fn cutting_respects_character_boundaries() {
        let f = fit("\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}", 24);
        assert_eq!(f.shown, "\u{e9}");
    }

    #[test]
    fn alignment_moves_the_run() {
        let rect = Rect::new(10, 20, 60, 16);
        for (align, x) in [
            (Align::Left, 10),
            (Align::Center, 10 + 21),
            (Align::Right, 10 + 42),
        ] {
            let mut scene = UiScene::new();
            assert!(!draw_in(&mut scene, rect, align, Color::WHITE, "abc"));
            let Some(crate::DrawCommand::Text { x: got, y, .. }) =
                scene.commands().first().copied()
            else {
                panic!("no text");
            };
            assert_eq!((got, y), (x, 22), "{align:?}");
        }
    }

    #[test]
    fn a_cut_text_draws_the_ellipsis_after_it() {
        let mut scene = UiScene::new();
        assert!(draw_in(
            &mut scene,
            Rect::new(0, 0, 36, 10),
            Align::Left,
            Color::WHITE,
            "abcdefgh"
        ));
        let texts: Vec<_> = scene
            .commands()
            .iter()
            .filter_map(|c| match *c {
                crate::DrawCommand::Text { x, span, .. } => Some((x, scene.text(span))),
                _ => None,
            })
            .collect();
        assert_eq!(texts, [(0, "abc"), (18, "...")]);
    }

    #[test]
    fn wrap_breaks_at_spaces() {
        let lines: Vec<_> = wrap("Keep this video mode? It reverts soon.", 60).collect();
        assert_eq!(
            lines,
            ["Keep this", "video", "mode? It", "reverts", "soon."]
        );
    }

    #[test]
    fn wrap_breaks_long_words_and_newlines() {
        let lines: Vec<_> = wrap("abcdefghijkl\nxy", 30).collect();
        assert_eq!(lines, ["abcde", "fghij", "kl", "xy"]);
        assert_eq!(wrap("", 30).count(), 0);
        assert_eq!(wrap("a\n\nb", 30).collect::<Vec<_>>(), ["a", "", "b"]);
    }

    #[test]
    fn a_line_after_a_newline_keeps_its_indent() {
        let lines: Vec<_> = wrap("Folders:\n  roms/nes  roms/snes", 300).collect();
        assert_eq!(lines, ["Folders:", "  roms/nes  roms/snes"]);
        let lines: Vec<_> = wrap("  one two three", 60).collect();
        assert_eq!(lines, ["  one two", "three"]);
        assert_eq!(wrap("a\n   ", 60).collect::<Vec<_>>(), ["a"]);
    }

    #[test]
    fn stack_text_drops_what_does_not_fit() {
        let mut t = StackText::<5>::new();
        write!(t, "ab\u{e9}cd").expect("never fails");
        assert_eq!(t.as_str(), "ab\u{e9}c");
        t.clear();
        assert_eq!(t.as_str(), "");
    }

    #[test]
    fn values_as_text() {
        let mut buf = StackText::<32>::new();
        assert_eq!(value_text(&SettingValue::Toggle(true), &mut buf), "On");
        let number = SettingValue::Number {
            value: -3,
            min: -10,
            max: 10,
            step: 1,
            unit: "px".into(),
        };
        assert_eq!(value_text(&number, &mut buf), "-3 px");
        let choice = SettingValue::Choice {
            index: 1,
            options: vec!["Auto".into(), "60 Hz".into()],
        };
        assert_eq!(value_text(&choice, &mut buf), "60 Hz");
        let broken = SettingValue::Choice {
            index: 7,
            options: Vec::new(),
        };
        assert_eq!(value_text(&broken, &mut buf), "?");
    }
}
