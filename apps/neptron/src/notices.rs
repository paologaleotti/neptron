//! The notices that the frontend shows only once: the ids of those shown
//! already, in `state/config/notices.toml`.
//!
//! The file is not user data: a deleted or damaged file shows the notices
//! again and loses nothing, so a bad file reads as an empty list and never
//! stops the boot. The writer thread writes it with the write rule of user
//! data (temp file, fsync, rename).

use serde::{Deserialize, Serialize};

/// The notice of a vertical game that shows interlaced, with every pixel
/// exact, on a 15 kHz CRT or a TV.
pub const VERTICAL_INTERLACED: &str = "vertical_interlaced";

/// The text of [`VERTICAL_INTERLACED`].
pub const VERTICAL_INTERLACED_TEXT: &str = "This is a vertical game on a horizontal screen. It shows interlaced, with every pixel exact. Settings > Display > Vertical games, or Video in the quick menu, can show it as a soft picture with no flicker. A screen turned on its side (TATE) comes in a later version.";

/// The version of the file format. The format only grows.
const FORMAT: u32 = 1;

/// The ids of the one-time notices shown already.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ShownNotices {
    ids: Vec<String>,
}

#[derive(Serialize, Deserialize)]
struct File {
    format: u32,
    #[serde(default)]
    shown: Vec<String>,
}

impl ShownNotices {
    /// The list in the text of a notices file. A text that does not parse,
    /// or a later format, gives the empty list and the reason.
    pub fn parse(text: &str) -> (Self, Option<String>) {
        match toml::from_str::<File>(text) {
            Ok(file) if file.format == FORMAT => (Self { ids: file.shown }, None),
            Ok(file) => (
                Self::default(),
                Some(format!(
                    "the notices file has format {}, not {FORMAT}; the one-time notices show again",
                    file.format
                )),
            ),
            Err(e) => (
                Self::default(),
                Some(format!(
                    "the notices file does not parse ({e}); the one-time notices show again"
                )),
            ),
        }
    }

    /// The list of the file at `path`: empty when the file is missing, and
    /// empty with the reason when it cannot be read or parsed.
    pub fn load(path: &std::path::Path) -> (Self, Option<String>) {
        match std::fs::read_to_string(path) {
            Ok(text) => Self::parse(&text),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (Self::default(), None),
            Err(e) => (
                Self::default(),
                Some(format!("cannot read {}: {e}", path.display())),
            ),
        }
    }

    /// True when the notice `id` was shown.
    pub fn contains(&self, id: &str) -> bool {
        self.ids.iter().any(|i| i == id)
    }

    /// Records `id` as shown. False when it was there already.
    pub fn insert(&mut self, id: &str) -> bool {
        if self.contains(id) {
            return false;
        }
        self.ids.push(id.to_owned());
        true
    }

    /// The text of the file.
    pub fn to_text(&self) -> String {
        let file = File {
            format: FORMAT,
            shown: self.ids.clone(),
        };
        // A struct of a number and a list of strings always serializes.
        toml::to_string(&file).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_list_goes_through_its_text() {
        let mut shown = ShownNotices::default();
        assert!(!shown.contains(VERTICAL_INTERLACED));
        assert!(shown.insert(VERTICAL_INTERLACED));
        assert!(!shown.insert(VERTICAL_INTERLACED), "once");
        let text = shown.to_text();
        assert!(text.starts_with("format = 1\n"), "{text}");
        assert_eq!(ShownNotices::parse(&text), (shown, None));
    }

    #[test]
    fn an_unknown_id_is_kept() {
        let (shown, issue) = ShownNotices::parse("format = 1\nshown = [\"later\"]\n");
        assert_eq!(issue, None);
        assert!(shown.contains("later"));
        assert!(shown.to_text().contains("later"));
    }

    #[test]
    fn a_bad_file_shows_the_notices_again() {
        for text in ["", "format = 2\nshown = []\n", "shown = 3", "\u{0}"] {
            let (shown, issue) = ShownNotices::parse(text);
            assert_eq!(shown, ShownNotices::default(), "{text:?}");
            assert!(issue.is_some(), "{text:?}");
        }
        let (shown, issue) = ShownNotices::parse("format = 1\n");
        assert_eq!((shown, issue), (ShownNotices::default(), None));
    }

    #[test]
    fn a_missing_file_is_an_empty_list_with_no_issue() {
        let path = std::env::temp_dir().join(format!(
            "neptron-notices-missing-{}.toml",
            std::process::id()
        ));
        assert_eq!(ShownNotices::load(&path), (ShownNotices::default(), None));
    }
}
