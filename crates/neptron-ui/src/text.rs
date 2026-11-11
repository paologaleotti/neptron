//! Text layout with the built-in font: fitting into a box, alignment and
//! word wrap. Nothing here allocates.

use core::fmt;

use crate::font;
use crate::geom::{Color, Rect};
use crate::scene::UiScene;
use crate::snapshot::SettingValue;

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
/// the width has room for it.
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
    Fit {
        shown: prefix(text, keep),
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
/// a slice of `text` without its outer spaces.
pub fn wrap(text: &str, width_px: u32) -> Wrap<'_> {
    Wrap {
        rest: text,
        room: font::chars_in(width_px).max(1),
    }
}

/// The lines of [`wrap`].
#[derive(Clone, Debug)]
pub struct Wrap<'a> {
    rest: &'a str,
    room: usize,
}

impl<'a> Iterator for Wrap<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<&'a str> {
        let rest = self.rest.trim_start_matches(' ');
        if rest.is_empty() {
            return None;
        }
        let (line, next) = take_line(rest, self.room);
        self.rest = next;
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
            options.get(*index).map_or("?", |o| o.label.as_str())
        }
        SettingValue::Action(_) => "",
    }
}

fn prefix(text: &str, chars: usize) -> &str {
    match text.char_indices().nth(chars) {
        Some((at, _)) => text.get(..at).unwrap_or(text),
        None => text,
    }
}

fn take_line(text: &str, room: usize) -> (&str, &str) {
    let mut last_space = None;
    for (count, (at, c)) in text.char_indices().enumerate() {
        if c == '\n' {
            return (&text[..at], &text[at + 1..]);
        }
        if count == room {
            if c == ' ' {
                return (&text[..at], &text[at..]);
            }
            return match last_space {
                Some(space) => (&text[..space], &text[space..]),
                None => (&text[..at], &text[at..]),
            };
        }
        if c == ' ' {
            last_space = Some(at);
        }
    }
    (text, "")
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
