//! The skin file: colours and sizes of the templates, in a small versioned
//! text file. The app reads the file and passes its text to [`parse`]; this
//! crate does no I/O.
//!
//! The file is a subset of TOML: comments, a top-level `version`, and the
//! tables `[colors]` and `[sizes]` with `key = value` lines. A colour is a
//! string `"#RRGGBB"`; a size is a decimal integer. Every key of the
//! version is required, and an unknown key is refused, so a typing error
//! never passes silently.
//!
//! The format is append-only: a later version adds keys and never renames
//! or removes one. Version 1 has eight colours. Version 2 adds `warning` and
//! `error`; a version 1 file gets their values from [`Palette::default`].

use crate::geom::Color;
use crate::theme::{Palette, Theme, ThemeError};

/// The newest skin format version that [`parse`] reads.
pub const SKIN_VERSION: u32 = 2;
/// The oldest skin format version that [`parse`] reads.
pub const SKIN_VERSION_OLDEST: u32 = 1;

/// Why a skin file was refused. Each error names the line or the key.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SkinError {
    /// A line is not a comment, a table header or `key = value`.
    #[error("line {line}: expected `key = value` or `[table]`")]
    Syntax {
        /// The line number, from 1.
        line: usize,
    },
    /// A table that the format does not have.
    #[error("line {line}: unknown table [{name}]")]
    UnknownTable {
        /// The line number, from 1.
        line: usize,
        /// The table name.
        name: String,
    },
    /// A key that the format does not have in its table.
    #[error("line {line}: unknown key {key:?}")]
    UnknownKey {
        /// The line number, from 1.
        line: usize,
        /// The key.
        key: String,
    },
    /// A key that is set twice.
    #[error("line {line}: {key:?} is set a second time")]
    DuplicateKey {
        /// The line number, from 1.
        line: usize,
        /// The key.
        key: String,
    },
    /// A colour that is not a string `"#RRGGBB"`.
    #[error("line {line}: {value} is not a colour \"#RRGGBB\"")]
    BadColor {
        /// The line number, from 1.
        line: usize,
        /// The value as written.
        value: String,
    },
    /// A number that is not a decimal integer that fits in 32 bits.
    #[error("line {line}: {value} is not a whole number")]
    BadNumber {
        /// The line number, from 1.
        line: usize,
        /// The value as written.
        value: String,
    },
    /// A required key is missing.
    #[error("the key {key:?} is missing")]
    MissingKey {
        /// The key, with its table, for example `colors.focus`.
        key: &'static str,
    },
    /// The file has a format version that this build cannot read.
    #[error(
        "skin version {found} is not supported; this build reads versions {oldest} to {newest}"
    )]
    Version {
        /// The version in the file.
        found: u32,
        /// The oldest version this build reads.
        oldest: u32,
        /// The newest version this build reads.
        newest: u32,
    },
    /// A size is outside the limits of the theme.
    #[error(transparent)]
    Theme(#[from] ThemeError),
}

/// Reads a skin file and makes its [`Theme`] through [`Theme::new`], so a
/// size outside the limits is refused here, once, and not at each frame.
pub fn parse(text: &str) -> Result<Theme, SkinError> {
    let mut fields = Fields::default();
    let mut table = Table::Top;
    for (index, raw) in text.lines().enumerate() {
        let line = index + 1;
        let content = strip_comment(raw).trim();
        if content.is_empty() {
            continue;
        }
        if let Some(name) = content
            .strip_prefix('[')
            .and_then(|rest| rest.strip_suffix(']'))
        {
            table = Table::named(name.trim()).ok_or_else(|| SkinError::UnknownTable {
                line,
                name: name.trim().to_owned(),
            })?;
            continue;
        }
        let (key, value) = content
            .split_once('=')
            .map(|(k, v)| (k.trim(), v.trim()))
            .filter(|(k, v)| !k.is_empty() && !v.is_empty())
            .ok_or(SkinError::Syntax { line })?;
        fields.set(table, key, value, line)?;
    }
    fields.finish()
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Table {
    Top,
    Colors,
    Sizes,
}

impl Table {
    fn named(name: &str) -> Option<Self> {
        match name {
            "colors" => Some(Self::Colors),
            "sizes" => Some(Self::Sizes),
            _ => None,
        }
    }
}

/// A colour key: its name in `[colors]`, its path for an error, and the
/// first format version that has it.
struct ColorKey {
    name: &'static str,
    path: &'static str,
    since: u32,
}

const fn key(name: &'static str, path: &'static str, since: u32) -> ColorKey {
    ColorKey { name, path, since }
}

const COLOR_KEYS: [ColorKey; 10] = [
    key("background", "colors.background", 1),
    key("panel", "colors.panel", 1),
    key("border", "colors.border", 1),
    key("text", "colors.text", 1),
    key("text_dim", "colors.text_dim", 1),
    key("focus", "colors.focus", 1),
    key("focus_text", "colors.focus_text", 1),
    key("accent", "colors.accent", 1),
    key("warning", "colors.warning", 2),
    key("error", "colors.error", 2),
];

#[derive(Default)]
struct Fields {
    version: Option<u32>,
    colors: [Option<Color>; COLOR_KEYS.len()],
    row_height_px: Option<u32>,
    padding_px: Option<u32>,
}

impl Fields {
    fn set(&mut self, table: Table, key: &str, value: &str, line: usize) -> Result<(), SkinError> {
        let unknown = || SkinError::UnknownKey {
            line,
            key: key.to_owned(),
        };
        match table {
            Table::Top if key == "version" => {
                let version = number(value, line)?;
                put(&mut self.version, version, key, line)?;
                supported(version)
            }
            Table::Colors => {
                // TOML puts top-level keys before the first table, so the
                // version is known here. Without one, `finish` reports it.
                let version = self.version.unwrap_or(SKIN_VERSION);
                let slot = COLOR_KEYS
                    .iter()
                    .position(|k| k.name == key && k.since <= version)
                    .and_then(|i| self.colors.get_mut(i))
                    .ok_or_else(unknown)?;
                put(slot, color(value, line)?, key, line)
            }
            Table::Sizes if key == "row_height_px" => {
                put(&mut self.row_height_px, number(value, line)?, key, line)
            }
            Table::Sizes if key == "padding_px" => {
                put(&mut self.padding_px, number(value, line)?, key, line)
            }
            _ => Err(unknown()),
        }
    }

    fn finish(self) -> Result<Theme, SkinError> {
        let version = self
            .version
            .ok_or(SkinError::MissingKey { key: "version" })?;
        supported(version)?;
        let defaults = Palette::default();
        let mut colors = [
            defaults.background,
            defaults.panel,
            defaults.border,
            defaults.text,
            defaults.text_dim,
            defaults.focus,
            defaults.focus_text,
            defaults.accent,
            defaults.warning,
            defaults.error,
        ];
        for ((slot, value), key) in colors.iter_mut().zip(self.colors).zip(&COLOR_KEYS) {
            match value {
                Some(value) => *slot = value,
                None if key.since <= version => {
                    return Err(SkinError::MissingKey { key: key.path });
                }
                None => {}
            }
        }
        let [
            background,
            panel,
            border,
            text,
            text_dim,
            focus,
            focus_text,
            accent,
            warning,
            error,
        ] = colors;
        let palette = Palette {
            background,
            panel,
            border,
            text,
            text_dim,
            focus,
            focus_text,
            accent,
            warning,
            error,
        };
        let row = self.row_height_px.ok_or(SkinError::MissingKey {
            key: "sizes.row_height_px",
        })?;
        let padding = self.padding_px.ok_or(SkinError::MissingKey {
            key: "sizes.padding_px",
        })?;
        Ok(Theme::new(palette, row, padding)?)
    }
}

fn supported(version: u32) -> Result<(), SkinError> {
    if (SKIN_VERSION_OLDEST..=SKIN_VERSION).contains(&version) {
        Ok(())
    } else {
        Err(SkinError::Version {
            found: version,
            oldest: SKIN_VERSION_OLDEST,
            newest: SKIN_VERSION,
        })
    }
}

fn put<T>(slot: &mut Option<T>, value: T, key: &str, line: usize) -> Result<(), SkinError> {
    if slot.is_some() {
        return Err(SkinError::DuplicateKey {
            line,
            key: key.to_owned(),
        });
    }
    *slot = Some(value);
    Ok(())
}

fn strip_comment(line: &str) -> &str {
    let mut quoted = false;
    for (at, c) in line.char_indices() {
        match c {
            '"' => quoted = !quoted,
            '#' if !quoted => return &line[..at],
            _ => {}
        }
    }
    line
}

fn number(value: &str, line: usize) -> Result<u32, SkinError> {
    let bad = || SkinError::BadNumber {
        line,
        value: value.to_owned(),
    };
    if !value.bytes().all(|b| b.is_ascii_digit()) {
        return Err(bad());
    }
    value.parse().map_err(|_| bad())
}

fn color(value: &str, line: usize) -> Result<Color, SkinError> {
    let bad = || SkinError::BadColor {
        line,
        value: value.to_owned(),
    };
    let hex = value
        .strip_prefix("\"#")
        .and_then(|v| v.strip_suffix('"'))
        .filter(|h| h.len() == 6 && h.bytes().all(|b| b.is_ascii_hexdigit()))
        .ok_or_else(bad)?;
    let xrgb = u32::from_str_radix(hex, 16).map_err(|_| bad())?;
    Ok(Color::from_xrgb(xrgb))
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEFAULT_SKIN: &str = include_str!("../../../data/skins/default/skin.toml");

    #[test]
    fn the_default_skin_file_is_the_built_in_theme() {
        assert_eq!(parse(DEFAULT_SKIN), Ok(Theme::default()));
    }

    fn replace(from: &str, to: &str) -> String {
        assert!(DEFAULT_SKIN.contains(from), "{from:?}");
        DEFAULT_SKIN.replacen(from, to, 1)
    }

    #[test]
    fn a_colour_is_read_as_rgb() {
        let skin = replace("focus = \"#F0C030\"", "focus = \"#a1B2c3\" # amber");
        let theme = parse(&skin).expect("valid");
        assert_eq!(theme.colors.focus, Color::rgb(0xA1, 0xB2, 0xC3));
    }

    #[test]
    fn a_size_outside_the_theme_limits_is_refused() {
        let skin = replace("row_height_px = 12", "row_height_px = 40");
        assert_eq!(
            parse(&skin),
            Err(SkinError::Theme(ThemeError::RowHeight {
                value: 40,
                min: 12,
                max: 24
            }))
        );
        let skin = replace("padding_px = 4", "padding_px = 99999999999");
        assert!(matches!(parse(&skin), Err(SkinError::BadNumber { .. })));
    }

    #[test]
    fn errors_name_the_line() {
        let cases = [
            (
                replace("accent = \"#50C8F0\"", "accent = \"#50C8F\""),
                SkinError::BadColor {
                    line: 23,
                    value: "\"#50C8F\"".into(),
                },
            ),
            (
                replace("accent = \"#50C8F0\"", "accent = #50C8F0"),
                SkinError::Syntax { line: 23 },
            ),
            (
                replace("accent =", "accnet ="),
                SkinError::UnknownKey {
                    line: 23,
                    key: "accnet".into(),
                },
            ),
            (
                replace("[sizes]", "[layout]"),
                SkinError::UnknownTable {
                    line: 27,
                    name: "layout".into(),
                },
            ),
            (
                replace("padding_px = 4", "padding_px = 4\nrow_height_px = 12"),
                SkinError::DuplicateKey {
                    line: 30,
                    key: "row_height_px".into(),
                },
            ),
            (
                replace("padding_px = 4", "padding_px = -4"),
                SkinError::BadNumber {
                    line: 29,
                    value: "-4".into(),
                },
            ),
            (
                replace("padding_px = 4", "padding_px"),
                SkinError::Syntax { line: 29 },
            ),
        ];
        for (skin, expected) in cases {
            assert_eq!(parse(&skin), Err(expected));
        }
    }

    #[test]
    fn a_missing_key_or_another_version_is_refused() {
        assert_eq!(
            parse(&replace("text_dim = \"#9098B0\"\n", "")),
            Err(SkinError::MissingKey {
                key: "colors.text_dim"
            })
        );
        for found in [0, 3] {
            assert_eq!(
                parse(&replace("version = 2", &format!("version = {found}"))),
                Err(SkinError::Version {
                    found,
                    oldest: 1,
                    newest: 2
                })
            );
        }
        assert_eq!(parse(""), Err(SkinError::MissingKey { key: "version" }));
    }

    /// A version 1 file as it was committed before version 2.
    const VERSION_1_SKIN: &str = "\
version = 1

[colors]
background = \"#101428\"
panel = \"#202848\"
border = \"#6070A0\"
text = \"#E8E8E8\"
text_dim = \"#9098B0\"
focus = \"#F0C030\"
focus_text = \"#101428\"
accent = \"#50C8F0\"

[sizes]
row_height_px = 12
padding_px = 4
";

    #[test]
    fn a_version_1_file_gets_the_default_warning_and_error_colours() {
        assert_eq!(parse(VERSION_1_SKIN), Ok(Theme::default()));
        let skin = VERSION_1_SKIN.replace("#E8E8E8", "#FFFFFF");
        let theme = parse(&skin).expect("valid");
        assert_eq!(theme.colors.text, Color::rgb(0xFF, 0xFF, 0xFF));
        assert_eq!(theme.colors.warning, Palette::default().warning);
        assert_eq!(theme.colors.error, Palette::default().error);
    }

    #[test]
    fn a_version_1_file_has_no_warning_or_error_key() {
        for key in ["warning", "error"] {
            let skin = VERSION_1_SKIN.replace(
                "accent = \"#50C8F0\"",
                &format!("accent = \"#50C8F0\"\n{key} = \"#FF8020\""),
            );
            assert_eq!(
                parse(&skin),
                Err(SkinError::UnknownKey {
                    line: 12,
                    key: key.into(),
                })
            );
        }
    }

    #[test]
    fn a_version_2_file_gives_the_warning_and_error_colours() {
        let skin = replace("warning = \"#FF8020\"", "warning = \"#123456\"");
        let skin = skin.replacen("error = \"#FF6060\"", "error = \"#ABCDEF\"", 1);
        let theme = parse(&skin).expect("valid");
        assert_eq!(theme.colors.warning, Color::rgb(0x12, 0x34, 0x56));
        assert_eq!(theme.colors.error, Color::rgb(0xAB, 0xCD, 0xEF));
    }

    #[test]
    fn a_version_2_file_needs_valid_warning_and_error_colours() {
        assert_eq!(
            parse(&replace("warning = \"#FF8020\"\n", "")),
            Err(SkinError::MissingKey {
                key: "colors.warning"
            })
        );
        assert_eq!(
            parse(&replace("error = \"#FF6060\"\n", "")),
            Err(SkinError::MissingKey {
                key: "colors.error"
            })
        );
        assert_eq!(
            parse(&replace("error = \"#FF6060\"", "error = \"red\"")),
            Err(SkinError::BadColor {
                line: 25,
                value: "\"red\"".into(),
            })
        );
    }

    #[test]
    fn a_newer_version_is_refused_before_its_keys() {
        let skin =
            replace("version = 2", "version = 3").replacen("[sizes]", "[sizes]\nglow_px = 2", 1);
        assert_eq!(
            parse(&skin),
            Err(SkinError::Version {
                found: 3,
                oldest: 1,
                newest: 2
            })
        );
    }

    #[test]
    fn a_version_key_inside_a_table_is_unknown() {
        let skin = replace("[sizes]", "[sizes]\nversion = 2");
        assert!(matches!(parse(&skin), Err(SkinError::UnknownKey { .. })));
    }
}
