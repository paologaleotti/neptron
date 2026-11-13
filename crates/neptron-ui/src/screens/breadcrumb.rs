//! The breadcrumb of the title band: where the player is, as the labels of
//! the rows that opened each screen, cut to the width of the band.

use core::fmt::Write;

use crate::text::{ELLIPSIS, StackText};

/// The text between two parts. The font has `>`.
pub(super) const SEPARATOR: &str = " > ";

/// The longest breadcrumb that a frame formats, in bytes: more than the
/// widest title band holds, so a cut never runs out of room.
pub(super) const BREADCRUMB_BYTES: usize = 256;

/// The fewest characters that a cut part keeps: one character and the
/// ellipsis. A part with less room goes.
const CUT_PART_MIN_CHARS: usize = 1 + ELLIPSIS.len();

/// A breadcrumb formatted with no allocation.
pub(super) type Breadcrumb = StackText<BREADCRUMB_BYTES>;

/// Writes `parts`, from the root, into `out` so that the text has at most
/// `room` characters, and returns true when the text is shorter than the
/// whole path. The rule:
///
/// 1. The whole path, when it fits.
/// 2. Else, with three parts or more: `... > ` and the last two parts.
/// 3. Else the second last part, cut at its end with `...`, before the last
///    part; with three parts or more after `... > `.
/// 4. When that leaves the second last part fewer than 4 characters,
///    `... > ` and the last part.
/// 5. Else the last part alone, cut at its end with `...`.
pub(super) fn fit(parts: &[&str], room: usize, out: &mut Breadcrumb) -> bool {
    out.clear();
    let Some((&last, before)) = parts.split_last() else {
        return false;
    };
    let sep = SEPARATOR.len();
    let whole = parts.iter().map(|p| chars(p)).sum::<usize>() + sep * before.len();
    if whole <= room {
        write_joined(out, parts);
        return false;
    }
    if let Some((&second, rest)) = before.split_last() {
        let lead = if rest.is_empty() { "" } else { LEAD };
        if !rest.is_empty() && chars(LEAD) + chars(second) + sep + chars(last) <= room {
            write_all(out, &[LEAD, second, SEPARATOR, last]);
            return true;
        }
        let left = room.saturating_sub(chars(lead) + sep + chars(last));
        if left >= CUT_PART_MIN_CHARS {
            write_all(out, &[lead]);
            write_cut(out, second, left);
            write_all(out, &[SEPARATOR, last]);
            return true;
        }
        if chars(LEAD) + chars(last) <= room {
            write_all(out, &[LEAD, last]);
            return true;
        }
    }
    write_cut(out, last, room);
    true
}

/// Writes the whole path, `parts` joined by the separator.
pub(super) fn write_joined(out: &mut Breadcrumb, parts: &[&str]) {
    for (i, part) in parts.iter().enumerate() {
        if i > 0 {
            write_all(out, &[SEPARATOR]);
        }
        write_all(out, &[part]);
    }
}

/// The text before the last parts when the path is cut.
const LEAD: &str = "... > ";

fn chars(text: &str) -> usize {
    text.chars().count()
}

fn write_all(out: &mut Breadcrumb, texts: &[&str]) {
    for text in texts {
        // `StackText` never fails; it drops what does not fit.
        let _ = out.write_str(text);
    }
}

/// Writes `text` in at most `room` characters: whole, or its head with the
/// spaces at its end dropped, then `...`.
fn write_cut(out: &mut Breadcrumb, text: &str, room: usize) {
    if chars(text) <= room {
        write_all(out, &[text]);
        return;
    }
    let dots = room >= ELLIPSIS.len();
    let keep = if dots { room - ELLIPSIS.len() } else { room };
    let end = text
        .char_indices()
        .nth(keep)
        .map_or(text.len(), |(at, _)| at);
    let head = text.get(..end).unwrap_or("");
    if dots {
        write_all(out, &[head.trim_end_matches(' '), ELLIPSIS]);
    } else {
        write_all(out, &[head]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The quick menu title band with `Paused` on its right, in characters.
    const GAME_ROOM: usize = 38;
    /// The title band of every other page, in characters.
    const PAGE_ROOM: usize = 46;

    fn fitted(parts: &[&str], room: usize) -> (String, bool) {
        let mut out = Breadcrumb::new();
        let short = fit(parts, room, &mut out);
        let text = out.as_str().to_owned();
        assert!(
            text.chars().count() <= room,
            "{text:?} is longer than {room}"
        );
        (text, short)
    }

    #[test]
    fn a_path_that_fits_is_whole() {
        assert_eq!(
            fitted(&["Settings", "Audio", "Audio output"], PAGE_ROOM),
            ("Settings > Audio > Audio output".to_owned(), false)
        );
        assert_eq!(
            fitted(&["Astro Racer"], GAME_ROOM),
            ("Astro Racer".to_owned(), false)
        );
        assert_eq!(fitted(&[], GAME_ROOM), (String::new(), false));
        let exact = "x".repeat(PAGE_ROOM);
        assert_eq!(fitted(&[&exact], PAGE_ROOM), (exact.clone(), false));
    }

    #[test]
    fn a_long_path_keeps_its_last_two_parts() {
        assert_eq!(
            fitted(
                &["Astro Racer", "Settings", "Display", "Monitor profile"],
                GAME_ROOM
            ),
            ("... > Display > Monitor profile".to_owned(), true)
        );
    }

    #[test]
    fn a_long_game_title_is_cut_at_its_end() {
        let title = "Super Mario Bros. 3 (USA) (Virtual Console)";
        assert_eq!(
            fitted(&[title, "Video"], GAME_ROOM),
            ("Super Mario Bros. 3 (USA) (... > Video".to_owned(), true)
        );
        assert_eq!(
            fitted(&[title, "Video", "Picture style"], PAGE_ROOM),
            ("... > Video > Picture style".to_owned(), true),
            "the last two parts fit, so no part is cut"
        );
        assert_eq!(
            fitted(&[title], GAME_ROOM),
            ("Super Mario Bros. 3 (USA) (Virtual...".to_owned(), true)
        );
    }

    #[test]
    fn a_long_second_last_part_is_cut_after_the_lead() {
        let long = "A panel with a very long name for its row";
        assert_eq!(
            fitted(&["Settings", "Network", long, "Options"], PAGE_ROOM),
            (
                "... > A panel with a very long na... > Options".to_owned(),
                true
            )
        );
    }

    #[test]
    fn with_no_room_for_the_second_last_part_only_the_last_part_stays() {
        assert_eq!(
            fitted(&["Settings", "Display", "Picture style"], 20),
            ("... > Picture style".to_owned(), true)
        );
        assert_eq!(
            fitted(&["Settings", "Display", "A far too long last part"], 20),
            ("A far too long la...".to_owned(), true)
        );
        assert_eq!(fitted(&["Settings", "Display"], 2), ("Di".to_owned(), true));
    }

    #[test]
    fn a_cut_drops_the_spaces_before_the_ellipsis() {
        assert_eq!(
            fitted(&["Ab cd ef", "Video"], 13),
            ("Ab... > Video".to_owned(), true)
        );
    }

    #[test]
    fn every_width_and_every_path_stays_in_its_room() {
        let parts = [
            "Super Mario Bros. 3 (USA) (Virtual Console)",
            "Settings",
            "Display",
            "Monitor profile",
            "\u{e9}t\u{e9} \u{e0} la plage",
        ];
        for n in 0..=parts.len() {
            for room in 0..80 {
                fitted(&parts[..n], room);
            }
        }
    }
}
