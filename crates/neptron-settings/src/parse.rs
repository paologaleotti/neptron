use std::fmt;

use toml_edit::{Document, Item, TableLike};

use crate::builtin::VERSION_KEY;
use crate::layer::{Layer, SetError};
use crate::schema::{Entry, Invalid, Level, Schema, Value, is_segment};

/// The result of reading one settings file: the valid values, and a report of everything
/// else. Parsing never fails: a problem drops only the value it touches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parsed {
    /// The valid values.
    pub layer: Layer,
    /// Every problem found, in file order.
    pub issues: Vec<Issue>,
    /// The schema version recorded in the file, when it holds a valid one.
    pub version: Option<i64>,
}

/// One problem in a settings file, for the diagnostics page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    /// The line, counted from 1, when the problem has one.
    pub line: Option<usize>,
    /// The dotted key, when the problem is about one key.
    pub key: Option<String>,
    /// What is wrong and what NEPTRON does about it.
    pub kind: IssueKind,
}

/// What is wrong with a settings file or one of its values.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum IssueKind {
    /// The file exists but cannot be read.
    #[error("the file cannot be read ({message}); it is treated as empty")]
    Unreadable {
        /// The error from the system.
        message: String,
    },
    /// The file is not UTF-8 text.
    #[error("the file is not UTF-8 text; it is treated as empty and left unchanged")]
    NotUtf8,
    /// The file is not valid TOML.
    #[error("the file does not parse ({message}); it is treated as empty and left unchanged")]
    Unparseable {
        /// The message of the TOML parser.
        message: String,
    },
    /// The schema has no such key. A newer build may have written it.
    #[error("unknown key; it is kept in the file and ignored")]
    UnknownKey,
    /// The key may not be set at the level of this file.
    #[error("cannot be set at the {level} level; the level below applies")]
    LevelNotAllowed {
        /// The level of the file.
        level: Level,
    },
    /// The value has the wrong type or is out of range.
    #[error("{reason}; the level below applies")]
    Invalid {
        /// Why the value is invalid.
        reason: Invalid,
    },
    /// The schema version is not a positive integer. A write raises an integer below 1 and
    /// refuses any other value, so the content under the reserved key is never lost.
    #[error("the schema version must be a positive integer, found {found}")]
    BadVersion {
        /// The value found, as written.
        found: String,
    },
}

impl fmt::Display for Issue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(line) = self.line {
            write!(f, "line {line}: ")?;
        }
        if let Some(key) = &self.key {
            write!(f, "{key}: ")?;
        }
        write!(f, "{}", self.kind)
    }
}

/// Reads the text of a settings file for `level`.
///
/// An unknown key is reported and ignored. A value at a level its key does not allow, or of
/// the wrong type, or out of range, is reported and left out, so the level below applies. A
/// file that does not parse is reported as a whole and gives an empty layer.
pub fn parse_layer(schema: &Schema, level: Level, text: &str) -> Parsed {
    let text = strip_bom(text);
    let mut parser = Parser {
        schema,
        lines: LineIndex::new(text),
        parsed: Parsed {
            layer: Layer::new(level),
            issues: Vec::new(),
            version: None,
        },
    };
    match Document::parse(text) {
        Ok(doc) => parser.walk(doc.as_table(), None),
        Err(err) => parser.parsed.issues.push(Issue {
            line: err.span().map(|s| parser.lines.line(s.start)),
            key: None,
            kind: IssueKind::Unparseable {
                message: err.message().trim().to_owned(),
            },
        }),
    }
    parser.parsed
}

/// Removes a UTF-8 byte order mark, which some Windows editors write.
pub(crate) fn strip_bom(text: &str) -> &str {
    text.strip_prefix('\u{feff}').unwrap_or(text)
}

/// The start of each line of a text, so that a byte offset maps to its line in
/// logarithmic time. A file has one index, built in one pass.
pub(crate) struct LineIndex {
    starts: Vec<usize>,
}

impl LineIndex {
    /// Indexes the line starts of `text`.
    pub(crate) fn new(text: &str) -> Self {
        #[cfg(test)]
        tests::BYTES_SCANNED.with(|n| n.set(n.get() + text.len()));
        let starts = std::iter::once(0)
            .chain(
                text.bytes()
                    .enumerate()
                    .filter(|(_, b)| *b == b'\n')
                    .map(|(i, _)| i + 1),
            )
            .collect();
        LineIndex { starts }
    }

    /// The line, counted from 1, of byte `offset`.
    pub(crate) fn line(&self, offset: usize) -> usize {
        self.starts.partition_point(|start| *start <= offset)
    }
}

struct Parser<'a> {
    schema: &'a Schema,
    lines: LineIndex,
    parsed: Parsed,
}

impl Parser<'_> {
    fn walk(&mut self, table: &dyn TableLike, prefix: Option<&str>) {
        for (name, item) in table.iter() {
            let line = table
                .key(name)
                .and_then(|k| k.span())
                .or_else(|| item.span())
                .map(|s| self.lines.line(s.start));
            let path = match prefix {
                Some(prefix) => format!("{prefix}.{name}"),
                None => name.to_owned(),
            };
            if prefix.is_none() && name == VERSION_KEY {
                self.version(item, line);
            } else if !is_segment(name) {
                self.issue(line, path, IssueKind::UnknownKey);
            } else if let Some(entry) = self.schema.get(&path) {
                self.value(entry, item, line);
            } else if let Some(inner) = item.as_table_like() {
                self.walk(inner, Some(&path));
            } else {
                self.issue(line, path, IssueKind::UnknownKey);
            }
        }
    }

    fn version(&mut self, item: &Item, line: Option<usize>) {
        match item.as_integer() {
            Some(v) if v >= 1 => self.parsed.version = Some(v),
            _ => {
                let found = match item.as_value() {
                    Some(v) => v.to_string().trim().to_owned(),
                    None => item.type_name().to_owned(),
                };
                self.issue(
                    line,
                    VERSION_KEY.to_owned(),
                    IssueKind::BadVersion { found },
                );
            }
        }
    }

    fn value(&mut self, entry: &Entry, item: &Item, line: Option<usize>) {
        let level = self.parsed.layer.level();
        let kind = if !entry.levels.allows(level) {
            IssueKind::LevelNotAllowed { level }
        } else {
            match to_value(entry, item) {
                Err(reason) => IssueKind::Invalid { reason },
                Ok(value) => match self.parsed.layer.set(self.schema, entry.key, value) {
                    Ok(_) => return,
                    Err(SetError::Invalid { reason, .. }) => IssueKind::Invalid { reason },
                    Err(SetError::LevelNotAllowed { level, .. }) => {
                        IssueKind::LevelNotAllowed { level }
                    }
                    Err(SetError::UnknownKey { .. }) => IssueKind::UnknownKey,
                },
            }
        };
        self.issue(line, entry.key.to_owned(), kind);
    }

    fn issue(&mut self, line: Option<usize>, key: String, kind: IssueKind) {
        self.parsed.issues.push(Issue {
            line,
            key: Some(key),
            kind,
        });
    }
}

fn to_value(entry: &Entry, item: &Item) -> Result<Value, Invalid> {
    let wrong_type = || Invalid::WrongType {
        expected: entry.kind.value_type(),
        found: item.type_name(),
    };
    match item.as_value().ok_or_else(wrong_type)? {
        toml_edit::Value::Boolean(v) => Ok(Value::Bool(*v.value())),
        toml_edit::Value::Integer(v) => Ok(Value::Int(*v.value())),
        toml_edit::Value::String(v) => Ok(Value::Text(v.value().clone())),
        _ => Err(wrong_type()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{Kind, Levels, Safety, ValueType};

    thread_local! {
        /// The bytes that `LineIndex::new` scanned on this thread.
        pub(super) static BYTES_SCANNED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    }
    use crate::{DISPLAY_MONITOR, DISPLAY_OUTPUT, DISPLAY_STYLE};

    fn text(v: &str) -> Value {
        Value::Text(v.to_owned())
    }

    fn issue(line: usize, key: &str, kind: IssueKind) -> Issue {
        Issue {
            line: Some(line),
            key: Some(key.to_owned()),
            kind,
        }
    }

    fn mixed_schema() -> Schema {
        let entry = |key, kind| Entry {
            key,
            label: "Test",
            kind,
            levels: Levels::ALL,
            live: true,
            safety: Safety::None,
        };
        Schema::from_entries(vec![
            entry("a.flag", Kind::Bool { default: false }),
            entry(
                "a.count",
                Kind::Int {
                    min: 0,
                    max: 10,
                    default: 0,
                },
            ),
            entry(
                "a.deep.name",
                Kind::Name {
                    max_len: 8,
                    default: "x",
                },
            ),
        ])
        .unwrap()
    }

    #[test]
    fn an_empty_file_gives_an_empty_layer() {
        let parsed = parse_layer(&Schema::builtin(), Level::Global, "");
        assert!(parsed.layer.is_empty());
        assert!(parsed.issues.is_empty());
        assert_eq!(parsed.version, None);
    }

    #[test]
    fn a_valid_file_gives_every_value_and_the_version() {
        let src = "schema_version = 1\n\n# my CRT\n[display]\noutput = \"VGA-1\"\nmonitor = \"generic_15\" # set by hand\nstyle = \"2x\"\n";
        let parsed = parse_layer(&Schema::builtin(), Level::Global, src);
        assert_eq!(parsed.issues, []);
        assert_eq!(parsed.version, Some(1));
        assert_eq!(parsed.layer.get(DISPLAY_OUTPUT), Some(&text("VGA-1")));
        assert_eq!(parsed.layer.get(DISPLAY_MONITOR), Some(&text("generic_15")));
        assert_eq!(parsed.layer.get(DISPLAY_STYLE), Some(&text("2x")));
        assert_eq!(parsed.layer.len(), 3);
    }

    #[test]
    fn dotted_and_inline_tables_are_read_like_sections() {
        let schema = mixed_schema();
        for src in [
            "a.flag = true\na.count = 3\na.deep.name = \"n\"\n",
            "a = { flag = true, count = 3, deep = { name = \"n\" } }\n",
            "[a]\nflag = true\ncount = 3\n[a.deep]\nname = \"n\"\n",
        ] {
            let parsed = parse_layer(&schema, Level::Game, src);
            assert_eq!(parsed.issues, [], "{src}");
            assert_eq!(parsed.layer.get("a.flag"), Some(&Value::Bool(true)));
            assert_eq!(parsed.layer.get("a.count"), Some(&Value::Int(3)));
            assert_eq!(parsed.layer.get("a.deep.name"), Some(&text("n")));
        }
    }

    #[test]
    fn unknown_keys_are_reported_with_their_line() {
        let src = "[display]\nstyle = \"2x\"\nshader = \"crt\"\n\n[network]\nwifi_ssid = \"home\"\n\"odd.key\" = 1\n[core.fceumm]\nfceumm_palette = \"raw\"\n";
        let parsed = parse_layer(&Schema::builtin(), Level::Global, src);
        assert_eq!(parsed.layer.get(DISPLAY_STYLE), Some(&text("2x")));
        assert_eq!(
            parsed.issues,
            [
                issue(3, "display.shader", IssueKind::UnknownKey),
                issue(6, "network.wifi_ssid", IssueKind::UnknownKey),
                issue(7, "network.odd.key", IssueKind::UnknownKey),
                issue(9, "core.fceumm.fceumm_palette", IssueKind::UnknownKey),
            ]
        );
    }

    #[test]
    fn a_quoted_key_cannot_pose_as_a_dotted_key() {
        let parsed = parse_layer(
            &Schema::builtin(),
            Level::Global,
            "\"display.style\" = \"2x\"\n",
        );
        assert!(parsed.layer.is_empty());
        assert_eq!(
            parsed.issues,
            [issue(1, "display.style", IssueKind::UnknownKey)]
        );
    }

    #[test]
    fn wrong_types_and_ranges_are_reported_and_left_out() {
        let src = "[a]\nflag = 1\ncount = 11\ndeep = { name = \"no spaces\" }\n";
        let parsed = parse_layer(&mixed_schema(), Level::Global, src);
        assert!(parsed.layer.is_empty());
        assert_eq!(
            parsed.issues,
            [
                issue(
                    2,
                    "a.flag",
                    IssueKind::Invalid {
                        reason: Invalid::WrongType {
                            expected: ValueType::Bool,
                            found: "integer"
                        }
                    }
                ),
                issue(
                    3,
                    "a.count",
                    IssueKind::Invalid {
                        reason: Invalid::OutOfRange {
                            value: 11,
                            min: 0,
                            max: 10
                        }
                    }
                ),
                issue(
                    4,
                    "a.deep.name",
                    IssueKind::Invalid {
                        reason: Invalid::BadName {
                            value: "no spaces".into(),
                            max_len: 8
                        }
                    }
                ),
            ]
        );
    }

    #[test]
    fn every_toml_type_that_is_not_the_setting_type_is_reported() {
        let schema = mixed_schema();
        for (value, found) in [
            ("1.5", "float"),
            ("\"3\"", "string"),
            ("true", "boolean"),
            ("1979-05-27", "datetime"),
            ("[1, 2]", "array"),
            ("{ x = 1 }", "inline table"),
        ] {
            let src = format!("a.count = {value}\n");
            let parsed = parse_layer(&schema, Level::Global, &src);
            assert!(parsed.layer.is_empty(), "{value}");
            assert_eq!(
                parsed.issues,
                [issue(
                    1,
                    "a.count",
                    IssueKind::Invalid {
                        reason: Invalid::WrongType {
                            expected: ValueType::Int,
                            found
                        }
                    }
                )],
                "{value}"
            );
        }
        let parsed = parse_layer(&schema, Level::Global, "[a.count]\nx = 1\n");
        assert!(matches!(
            parsed.issues[0].kind,
            IssueKind::Invalid {
                reason: Invalid::WrongType { found: "table", .. }
            }
        ));
        let parsed = parse_layer(&schema, Level::Global, "[[a.count]]\nx = 1\n");
        assert!(matches!(
            parsed.issues[0].kind,
            IssueKind::Invalid {
                reason: Invalid::WrongType {
                    found: "array of tables",
                    ..
                }
            }
        ));
    }

    #[test]
    fn a_key_at_a_level_it_does_not_allow_is_reported() {
        let src = "[display]\noutput = \"VGA-1\"\nstyle = \"native\"\n";
        let parsed = parse_layer(&Schema::builtin(), Level::Game, src);
        assert_eq!(parsed.layer.get(DISPLAY_STYLE), Some(&text("native")));
        assert_eq!(parsed.layer.len(), 1);
        assert_eq!(
            parsed.issues,
            [issue(
                2,
                DISPLAY_OUTPUT,
                IssueKind::LevelNotAllowed { level: Level::Game }
            )]
        );
    }

    #[test]
    fn a_file_that_does_not_parse_gives_an_empty_layer_and_one_issue() {
        for (src, line) in [
            ("[display]\nstyle = \"2x\"\nmonitor = \n", 3),
            ("[display]\nstyle = \"2x\"\nstyle = \"3x\"\n", 3),
            ("[display\n", 1),
        ] {
            let parsed = parse_layer(&Schema::builtin(), Level::Global, src);
            assert!(parsed.layer.is_empty(), "{src}");
            assert_eq!(parsed.issues.len(), 1, "{src}");
            let issue = &parsed.issues[0];
            assert_eq!(issue.line, Some(line), "{src}");
            assert_eq!(issue.key, None);
            assert!(
                matches!(&issue.kind, IssueKind::Unparseable { message } if !message.is_empty())
            );
        }
    }

    #[test]
    fn a_bad_version_is_reported_and_the_values_still_count() {
        for (version, found) in [("0", "0"), ("\"2\"", "\"2\""), ("-1", "-1")] {
            let src = format!("schema_version = {version}\ndisplay.style = \"3x\"\n");
            let parsed = parse_layer(&Schema::builtin(), Level::Global, &src);
            assert_eq!(parsed.version, None);
            assert_eq!(parsed.layer.get(DISPLAY_STYLE), Some(&text("3x")));
            assert_eq!(
                parsed.issues,
                [issue(
                    1,
                    VERSION_KEY,
                    IssueKind::BadVersion {
                        found: found.into()
                    }
                )]
            );
        }
        let parsed = parse_layer(&Schema::builtin(), Level::Global, "[schema_version]\n");
        assert_eq!(
            parsed.issues[0].kind,
            IssueKind::BadVersion {
                found: "table".into()
            }
        );
    }

    #[test]
    fn a_newer_version_is_kept_and_its_unknown_keys_are_reported() {
        let src = "schema_version = 7\n[display]\nstyle = \"4x\"\nfuture = true\n";
        let parsed = parse_layer(&Schema::builtin(), Level::Global, src);
        assert_eq!(parsed.version, Some(7));
        assert_eq!(parsed.layer.get(DISPLAY_STYLE), Some(&text("4x")));
        assert_eq!(
            parsed.issues,
            [issue(4, "display.future", IssueKind::UnknownKey)]
        );
    }

    #[test]
    fn a_byte_order_mark_is_ignored() {
        let parsed = parse_layer(
            &Schema::builtin(),
            Level::Global,
            "\u{feff}display.style = \"2x\"\n",
        );
        assert_eq!(parsed.issues, []);
        assert_eq!(parsed.layer.get(DISPLAY_STYLE), Some(&text("2x")));
    }

    #[test]
    fn issues_print_with_line_key_and_action() {
        let i = issue(
            4,
            "display.style",
            IssueKind::Invalid {
                reason: Invalid::NotAChoice {
                    value: "5x".into(),
                    choices: &["a", "b"],
                },
            },
        );
        assert_eq!(
            i.to_string(),
            "line 4: display.style: \"5x\" is not one of: a, b; the level below applies"
        );
        let whole = Issue {
            line: None,
            key: None,
            kind: IssueKind::NotUtf8,
        };
        assert_eq!(
            whole.to_string(),
            "the file is not UTF-8 text; it is treated as empty and left unchanged"
        );
    }

    #[test]
    fn line_index_counts_from_one() {
        let lines = LineIndex::new("a\nb\nc");
        for (offset, line) in [(0, 1), (1, 1), (2, 2), (3, 2), (4, 3), (99, 3)] {
            assert_eq!(lines.line(offset), line, "{offset}");
        }
        assert_eq!(LineIndex::new("").line(0), 1);
        assert_eq!(LineIndex::new("\n\n").line(2), 3);
    }

    /// A large file is scanned for line starts once, not once per key, so the parse stays
    /// linear in the file size.
    #[test]
    fn a_large_file_is_scanned_for_lines_once() {
        const KEYS: usize = 20_000;
        let mut src = String::from("[display]\nstyle = \"2x\"\n[junk]\n");
        for i in 0..KEYS {
            src.push_str(&format!("k{i} = {i}\n"));
        }
        BYTES_SCANNED.with(|n| n.set(0));
        let parsed = parse_layer(&Schema::builtin(), Level::Global, &src);
        assert_eq!(BYTES_SCANNED.with(|n| n.get()), src.len());
        assert_eq!(parsed.layer.get(DISPLAY_STYLE), Some(&text("2x")));
        assert_eq!(parsed.issues.len(), KEYS);
        assert_eq!(parsed.issues[0], issue(4, "junk.k0", IssueKind::UnknownKey));
        assert_eq!(
            parsed.issues[KEYS - 1],
            issue(
                KEYS + 3,
                &format!("junk.k{}", KEYS - 1),
                IssueKind::UnknownKey
            )
        );
    }
}
