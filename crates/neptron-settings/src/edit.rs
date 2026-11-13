use toml_edit::{DocumentMut, InlineTable, Item, Table, TableLike};

use crate::builtin::{SCHEMA_VERSION, VERSION_KEY};
use crate::layer::{Layer, SetError};
use crate::parse::{LineIndex, strip_bom};
use crate::schema::{Level, Schema, Value};

/// One change to a settings file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change<'a> {
    /// Sets the key to the value.
    Set {
        /// The dotted key.
        key: &'a str,
        /// The new value.
        value: Value,
    },
    /// Removes the key, so the value is inherited from the level below.
    Unset {
        /// The dotted key.
        key: &'a str,
    },
}

/// Why a settings file cannot be edited. The file text stays as it was.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EditError {
    /// The current text does not parse, so an edit could lose what the user wrote.
    #[error("the file does not parse{} ({message}); fix it by hand first", line.map(|l| format!(" at line {l}")).unwrap_or_default())]
    Unparseable {
        /// The line of the error, counted from 1.
        line: Option<usize>,
        /// The message of the TOML parser.
        message: String,
    },
    /// The change does not pass the schema check.
    #[error(transparent)]
    Rejected(#[from] SetError),
    /// The file holds something else where the key goes, such as a value where a table must
    /// be, or a table where the value must be.
    #[error(
        "key \"{key}\": the file holds a {found} at \"{path}\", so the change would overwrite it"
    )]
    Conflict {
        /// The key of the change.
        key: String,
        /// The dotted path of the item in the way.
        path: String,
        /// The TOML type of the item in the way.
        found: &'static str,
    },
}

/// Applies `changes` to the text of a settings file for `level` and returns the new text.
///
/// The edit keeps comments, key order, formatting and every key that the schema does not
/// know. A new value keeps the comment after the value it replaces. The file records
/// [`SCHEMA_VERSION`], or the version already in it when that is newer. Nothing changes when
/// one change fails.
pub fn edit(
    schema: &Schema,
    level: Level,
    text: &str,
    changes: &[Change<'_>],
) -> Result<String, EditError> {
    for change in changes {
        check(schema, level, change)?;
    }
    let text = strip_bom(text);
    let mut doc: DocumentMut =
        text.parse()
            .map_err(|err: toml_edit::TomlError| EditError::Unparseable {
                line: err.span().map(|s| LineIndex::new(text).line(s.start)),
                message: err.message().trim().to_owned(),
            })?;
    for change in changes {
        match change {
            Change::Set { key, value } => set(doc.as_table_mut(), key, value)?,
            Change::Unset { key } => unset(doc.as_table_mut(), key)?,
        }
    }
    record_version(doc.as_table_mut())?;
    Ok(doc.to_string())
}

/// A set must pass the full check. An unset needs only a known key: removing a value never
/// makes a file less valid, and it is the way to clear a value at a level it does not allow.
fn check(schema: &Schema, level: Level, change: &Change<'_>) -> Result<(), SetError> {
    match change {
        Change::Set { key, value } => Layer::new(level)
            .set(schema, key, value.clone())
            .map(|_| ()),
        Change::Unset { key } => schema
            .get(key)
            .map(|_| ())
            .ok_or_else(|| SetError::UnknownKey {
                key: (*key).to_owned(),
            }),
    }
}

fn set(root: &mut Table, key: &str, value: &Value) -> Result<(), EditError> {
    let (parents, last) = split(key);
    let mut table: &mut dyn TableLike = root;
    let mut inline = false;
    let mut path = String::new();
    for segment in parents {
        push_segment(&mut path, segment);
        if !table.contains_key(segment) {
            table.insert(segment, new_table(inline, table.is_dotted()));
        }
        let (child, child_inline) = child_table(table, segment, key, &path)?;
        table = child;
        inline |= child_inline;
    }
    push_segment(&mut path, last);
    let new = to_toml(value);
    match table.get_mut(last) {
        None => {
            table.insert(last, Item::Value(new));
        }
        Some(Item::Value(old)) if !old.is_inline_table() => replace_value(old, new),
        Some(other) => {
            return Err(EditError::Conflict {
                key: key.to_owned(),
                path,
                found: other.type_name(),
            });
        }
    }
    Ok(())
}

/// True when a file that held `old` and holds `new` after an edit has nothing to keep, so
/// the storage removes it instead of a write. Each test keeps the file when in doubt:
///
/// - `old` holds no `#` at all, so no comment of the user goes with the file. A `#` in a
///   string also keeps it.
/// - `new` parses, and its version is absent or an integer no newer than
///   [`SCHEMA_VERSION`]: a newer build may need the file.
/// - Apart from the version, `new` holds only empty tables.
pub(crate) fn nothing_to_keep(old: &str, new: &str) -> bool {
    if old.contains('#') || new.contains('#') {
        return false;
    }
    let Ok(mut doc) = strip_bom(new).parse::<DocumentMut>() else {
        return false;
    };
    let known_version = match doc.get(VERSION_KEY) {
        None => true,
        Some(Item::Value(v)) => v.as_integer().is_some_and(|n| n <= SCHEMA_VERSION),
        Some(_) => false,
    };
    doc.as_table_mut().remove(VERSION_KEY);
    known_version && only_empty_tables(doc.as_table())
}

fn only_empty_tables(table: &dyn TableLike) -> bool {
    table
        .iter()
        .all(|(_, item)| item.as_table_like().is_some_and(only_empty_tables))
}

fn unset(root: &mut Table, key: &str) -> Result<(), EditError> {
    let segments: Vec<&str> = key.split('.').collect();
    unset_in(root, &segments, key, &mut String::new())
}

/// Removes the key below `table`, then each table on its path that the removal left with no
/// item and no comment, so an unset never leaves an empty table behind.
fn unset_in(
    table: &mut dyn TableLike,
    segments: &[&str],
    key: &str,
    path: &mut String,
) -> Result<(), EditError> {
    let Some((segment, rest)) = segments.split_first() else {
        return Ok(());
    };
    push_segment(path, segment);
    if rest.is_empty() {
        return match table.get(segment) {
            None => Ok(()),
            Some(Item::Value(v)) if !v.is_inline_table() => {
                table.remove(segment);
                Ok(())
            }
            Some(other) => Err(EditError::Conflict {
                key: key.to_owned(),
                path: path.clone(),
                found: other.type_name(),
            }),
        };
    }
    if !table.contains_key(segment) {
        return Ok(());
    }
    let child = child_table(table, segment, key, path)?.0;
    unset_in(child, rest, key, path)?;
    if child.is_empty() && !has_comment(table, segment) {
        table.remove(segment);
    }
    Ok(())
}

/// True when the key `segment` of `table` or its item carries a comment: in the decor of
/// the key (a dotted key keeps its comment there) or of the table or inline table.
fn has_comment(table: &dyn TableLike, segment: &str) -> bool {
    let key = table
        .key(segment)
        .map(|k| [k.leaf_decor(), k.dotted_decor()]);
    let item = match table.get(segment) {
        Some(Item::Table(t)) => Some(t.decor()),
        Some(Item::Value(v)) => Some(v.decor()),
        _ => None,
    };
    key.into_iter()
        .flatten()
        .chain(item)
        .flat_map(|d| [d.prefix(), d.suffix()])
        .flatten()
        .any(|raw| raw.as_str().is_none_or(|s| s.contains('#')))
}

/// Raises an older integer version and keeps a newer one. Anything else under the reserved
/// key is someone's data, so the edit refuses to replace it.
fn record_version(root: &mut Table) -> Result<(), EditError> {
    let new = toml_edit::Value::from(SCHEMA_VERSION);
    match root.get_mut(VERSION_KEY) {
        None => {
            root.insert(VERSION_KEY, Item::Value(new));
        }
        Some(Item::Value(old)) if old.is_integer() => {
            if old.as_integer().is_some_and(|v| v < SCHEMA_VERSION) {
                replace_value(old, new);
            }
        }
        Some(other) => {
            return Err(EditError::Conflict {
                key: VERSION_KEY.to_owned(),
                path: VERSION_KEY.to_owned(),
                found: other.type_name(),
            });
        }
    }
    Ok(())
}

/// Replaces a value and keeps the spaces and the comment around it.
fn replace_value(old: &mut toml_edit::Value, new: toml_edit::Value) {
    let decor = old.decor().clone();
    *old = new;
    *old.decor_mut() = decor;
}

/// Splits a schema key into its table segments and its last segment.
fn split(key: &str) -> (impl Iterator<Item = &str>, &str) {
    let (parents, last) = key.rsplit_once('.').unwrap_or(("", key));
    (parents.split('.').filter(|s| !s.is_empty()), last)
}

fn push_segment(path: &mut String, segment: &str) {
    if !path.is_empty() {
        path.push('.');
    }
    path.push_str(segment);
}

/// A new table in the style of its parent: inside an inline table it must be inline, and
/// inside a dotted table it stays dotted, so the file keeps its shape.
fn new_table(parent_inline: bool, parent_dotted: bool) -> Item {
    if parent_inline {
        Item::Value(toml_edit::Value::InlineTable(InlineTable::new()))
    } else {
        let mut table = Table::new();
        table.set_dotted(parent_dotted);
        Item::Table(table)
    }
}

/// The child table at `segment`, and whether it is an inline table.
fn child_table<'t>(
    table: &'t mut dyn TableLike,
    segment: &str,
    key: &str,
    path: &str,
) -> Result<(&'t mut dyn TableLike, bool), EditError> {
    let conflict = |found| EditError::Conflict {
        key: key.to_owned(),
        path: path.to_owned(),
        found,
    };
    let item = table.get_mut(segment).ok_or_else(|| conflict("nothing"))?;
    let found = item.type_name();
    let inline = item.is_inline_table();
    item.as_table_like_mut()
        .map(|t| (t, inline))
        .ok_or_else(|| conflict(found))
}

fn to_toml(value: &Value) -> toml_edit::Value {
    match value {
        Value::Bool(v) => (*v).into(),
        Value::Int(v) => (*v).into(),
        Value::Text(v) => v.as_str().into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse_layer;
    use crate::schema::{AppliesTo, Entry, Invalid, Kind, Levels, Safety};
    use crate::{DISPLAY_MONITOR, DISPLAY_OUTPUT, DISPLAY_REFRESH_POLICY, DISPLAY_STYLE};

    fn text(v: &str) -> Value {
        Value::Text(v.to_owned())
    }

    fn set_style(v: &str) -> Change<'static> {
        Change::Set {
            key: DISPLAY_STYLE,
            value: text(v),
        }
    }

    fn global(src: &str, changes: &[Change<'_>]) -> Result<String, EditError> {
        edit(&Schema::builtin(), Level::Global, src, changes)
    }

    #[test]
    fn an_empty_file_gets_the_version_and_a_section() {
        let out = global("", &[set_style("2x")]).unwrap();
        assert_eq!(out, "schema_version = 1\n\n[display]\nstyle = \"2x\"\n");
        let parsed = parse_layer(&Schema::builtin(), Level::Global, &out);
        assert_eq!(parsed.issues, []);
        assert_eq!(parsed.version, Some(1));
    }

    #[test]
    fn comments_order_and_unknown_keys_are_kept() {
        let src = "\
# My settings. Do not touch.
schema_version = 1

[display]
# the tube in the bedroom
monitor = \"generic_15\"
style = \"native\" # sharp
future_key = [1, 2]

[network]
wifi_ssid = \"home\"

[core.fceumm]
fceumm_palette = \"raw\"
";
        let out = global(src, &[set_style("2x")]).unwrap();
        assert_eq!(out, src.replace("\"native\" # sharp", "\"2x\" # sharp"));
    }

    #[test]
    fn a_new_key_goes_into_the_existing_section_in_its_style() {
        let schema = Schema::builtin();
        for (src, expected) in [
            (
                "schema_version = 1\n[display]\nstyle = \"2x\"\n",
                "schema_version = 1\n[display]\nstyle = \"2x\"\nrefresh_policy = \"force_60\"\n",
            ),
            (
                "schema_version = 1\ndisplay.style = \"2x\"\n",
                "schema_version = 1\ndisplay.style = \"2x\"\ndisplay.refresh_policy = \"force_60\"\n",
            ),
            (
                "schema_version = 1\ndisplay = { style = \"2x\" }\n",
                "schema_version = 1\ndisplay = { style = \"2x\", refresh_policy = \"force_60\" }\n",
            ),
        ] {
            let change = Change::Set {
                key: DISPLAY_REFRESH_POLICY,
                value: text("force_60"),
            };
            let out = edit(&schema, Level::Game, src, &[change]).unwrap();
            // toml_edit keeps the space that ended the old last inline value.
            assert_eq!(out.replace(' ', ""), expected.replace(' ', ""));
        }
    }

    #[test]
    fn deep_keys_follow_the_shape_of_their_parent() {
        let entry = Entry {
            key: "a.b.c",
            label: "Deep",
            kind: Kind::Bool { default: false },
            levels: Levels::ALL,
            live: true,
            safety: Safety::None,
            applies_to: AppliesTo::Every,
        };
        let schema = Schema::from_entries(vec![entry]).unwrap();
        let change = [Change::Set {
            key: "a.b.c",
            value: Value::Bool(true),
        }];
        for src in [
            "schema_version = 1\n",
            "schema_version = 1\na = { x = 1 }\n",
            "schema_version = 1\na.x = 1\n",
            "schema_version = 1\n[a]\nx = 1\n",
        ] {
            let out = edit(&schema, Level::Global, src, &change).unwrap();
            let parsed = parse_layer(&schema, Level::Global, &out);
            assert_eq!(parsed.layer.get("a.b.c"), Some(&Value::Bool(true)), "{out}");
            assert_eq!(parsed.issues.len(), usize::from(src.contains('x')), "{out}");
        }
    }

    #[test]
    fn unset_removes_only_the_key() {
        let src = "schema_version = 1\n[display]\n# keep me\nstyle = \"2x\"\nrefresh_policy = \"exact\"\nextra = 1\n";
        let out = global(src, &[Change::Unset { key: DISPLAY_STYLE }]).unwrap();
        assert_eq!(
            out,
            "schema_version = 1\n[display]\nrefresh_policy = \"exact\"\nextra = 1\n"
        );
        let out = global(&out, &[Change::Unset { key: DISPLAY_STYLE }]).unwrap();
        assert!(out.contains("refresh_policy"));
        assert_eq!(
            global("", &[Change::Unset { key: DISPLAY_STYLE }]).unwrap(),
            "schema_version = 1\n"
        );
    }

    #[test]
    fn unset_of_the_last_key_removes_its_empty_table() {
        for src in [
            "schema_version = 1\n[display]\nstyle = \"2x\"\n",
            "schema_version = 1\ndisplay.style = \"2x\"\n",
            "schema_version = 1\ndisplay = { style = \"2x\" }\n",
        ] {
            let out = global(src, &[Change::Unset { key: DISPLAY_STYLE }]).unwrap();
            assert_eq!(out, "schema_version = 1\n", "{src}");
            assert!(nothing_to_keep(src, &out), "{src}");
        }
    }

    #[test]
    fn an_empty_table_with_a_comment_stays() {
        let src = "schema_version = 1\n# the tube\n[display]\nstyle = \"2x\"\n";
        let out = global(src, &[Change::Unset { key: DISPLAY_STYLE }]).unwrap();
        assert!(out.contains("# the tube"), "{out}");
        assert!(!nothing_to_keep(src, &out), "{out}");
    }

    #[test]
    fn nothing_to_keep_is_true_only_with_no_value_no_comment_and_a_known_version() {
        let old = "schema_version = 1\n[display]\nstyle = \"2x\"\n";
        for new in [
            "",
            "\n",
            "schema_version = 1\n",
            "schema_version = 0\n",
            "schema_version = 1\n\n[display]\n",
        ] {
            assert!(nothing_to_keep(old, new), "{new:?}");
        }
        for new in [
            "schema_version = 1\n# note\n",
            "schema_version = 1\n[display]\nstyle = \"2x\"\n",
            "[network]\nwifi_ssid = \"home\"\n",
            "schema_version = 2\n",
            "schema_version = \"x\"\n",
            "not toml",
        ] {
            assert!(!nothing_to_keep(old, new), "{new:?}");
        }
        for old in [
            "# my CRT notes, keep\nschema_version = 1\n[display]\nstyle = \"2x\"\n",
            "[display]\nstyle = \"2x\" # sharp\n",
        ] {
            assert!(!nothing_to_keep(old, "schema_version = 1\n"), "{old:?}");
        }
    }

    #[test]
    fn a_comment_above_a_dotted_key_keeps_the_file() {
        let src = "schema_version = 1\n# my tube\ndisplay.style = \"2x\"\n";
        let out = global(src, &[Change::Unset { key: DISPLAY_STYLE }]).unwrap();
        assert!(!nothing_to_keep(src, &out), "{out}");
    }

    #[test]
    fn unset_clears_a_key_at_a_level_it_does_not_allow() {
        let src = "[display]\noutput = \"VGA-1\"\n";
        let out = edit(
            &Schema::builtin(),
            Level::Game,
            src,
            &[Change::Unset {
                key: DISPLAY_OUTPUT,
            }],
        )
        .unwrap();
        assert!(!out.contains("output"));
    }

    #[test]
    fn several_changes_go_in_one_edit() {
        let out = global(
            "",
            &[
                Change::Set {
                    key: DISPLAY_OUTPUT,
                    value: text("VGA-1"),
                },
                Change::Set {
                    key: DISPLAY_MONITOR,
                    value: text("generic_15"),
                },
                set_style("3x"),
                Change::Unset { key: DISPLAY_STYLE },
            ],
        )
        .unwrap();
        let parsed = parse_layer(&Schema::builtin(), Level::Global, &out);
        assert_eq!(parsed.layer.get(DISPLAY_OUTPUT), Some(&text("VGA-1")));
        assert_eq!(parsed.layer.get(DISPLAY_MONITOR), Some(&text("generic_15")));
        assert_eq!(parsed.layer.get(DISPLAY_STYLE), None);
    }

    #[test]
    fn a_rejected_change_stops_the_whole_edit() {
        let schema = Schema::builtin();
        assert_eq!(
            global("", &[set_style("2x"), set_style("9x")]).map_err(|e| matches!(
                e,
                EditError::Rejected(SetError::Invalid {
                    reason: Invalid::NotAChoice { .. },
                    ..
                })
            )),
            Err(true)
        );
        assert_eq!(
            edit(
                &schema,
                Level::Game,
                "",
                &[Change::Set {
                    key: DISPLAY_OUTPUT,
                    value: text("VGA-1")
                }]
            ),
            Err(EditError::Rejected(SetError::LevelNotAllowed {
                key: DISPLAY_OUTPUT,
                level: Level::Game
            }))
        );
        for change in [
            Change::Unset {
                key: "display.extra",
            },
            Change::Set {
                key: "display.extra",
                value: Value::Int(1),
            },
        ] {
            assert_eq!(
                global("display.extra = 1\n", &[change]),
                Err(EditError::Rejected(SetError::UnknownKey {
                    key: "display.extra".into()
                }))
            );
        }
    }

    #[test]
    fn a_file_that_does_not_parse_is_not_edited() {
        let err = global("[display]\nstyle = \n", &[set_style("2x")]).unwrap_err();
        assert!(
            matches!(err, EditError::Unparseable { line: Some(2), ref message } if !message.is_empty()),
            "{err:?}"
        );
        assert!(
            err.to_string()
                .starts_with("the file does not parse at line 2 (")
        );
    }

    #[test]
    fn a_change_never_overwrites_a_table_or_a_value_in_the_way() {
        for (src, path, found) in [
            ("display = 3\n", "display", "integer"),
            ("display = \"x\"\n", "display", "string"),
            (
                "[[display]]\nstyle = \"2x\"\n",
                "display",
                "array of tables",
            ),
            ("[display.style]\nx = 1\n", "display.style", "table"),
            (
                "display.style = { x = 1 }\n",
                "display.style",
                "inline table",
            ),
        ] {
            for change in [set_style("2x"), Change::Unset { key: DISPLAY_STYLE }] {
                assert_eq!(
                    global(src, &[change]),
                    Err(EditError::Conflict {
                        key: DISPLAY_STYLE.into(),
                        path: path.into(),
                        found
                    }),
                    "{src}"
                );
            }
        }
    }

    #[test]
    fn the_version_is_raised_but_never_lowered() {
        for (src, expected) in [
            ("schema_version = 0 # old\n", "schema_version = 1 # old\n"),
            ("schema_version = 1\n", "schema_version = 1\n"),
            ("schema_version = 9\n", "schema_version = 9\n"),
        ] {
            assert_eq!(global(src, &[]).unwrap(), expected, "{src}");
        }
    }

    #[test]
    fn a_version_that_is_not_an_integer_is_never_replaced() {
        for (src, found) in [
            ("schema_version = \"x\"\n", "string"),
            ("schema_version = 1.5\n", "float"),
            ("schema_version = [1]\n", "array"),
            ("schema_version = { a = 1 }\n", "inline table"),
            ("[schema_version]\nnote = \"mine\"\n", "table"),
        ] {
            assert_eq!(
                global(src, &[set_style("2x")]),
                Err(EditError::Conflict {
                    key: VERSION_KEY.into(),
                    path: VERSION_KEY.into(),
                    found
                }),
                "{src}"
            );
        }
    }

    #[test]
    fn a_byte_order_mark_is_dropped() {
        let out = global("\u{feff}display.style = \"2x\"\n", &[set_style("3x")]).unwrap();
        assert!(!out.starts_with('\u{feff}'));
        assert!(out.contains("display.style = \"3x\""));
    }

    #[test]
    fn every_value_type_round_trips() {
        let entry = |key, kind| Entry {
            key,
            label: "Test",
            kind,
            levels: Levels::ALL,
            live: true,
            safety: Safety::None,
            applies_to: AppliesTo::Every,
        };
        let schema = Schema::from_entries(vec![
            entry("a.flag", Kind::Bool { default: false }),
            entry(
                "a.count",
                Kind::Int {
                    min: -5,
                    max: 5,
                    default: 0,
                },
            ),
            entry(
                "a.name",
                Kind::Name {
                    max_len: 8,
                    default: "x",
                },
            ),
        ])
        .unwrap();
        let changes = [
            Change::Set {
                key: "a.flag",
                value: Value::Bool(true),
            },
            Change::Set {
                key: "a.count",
                value: Value::Int(-5),
            },
            Change::Set {
                key: "a.name",
                value: text("VGA-1"),
            },
        ];
        let out = edit(&schema, Level::System, "", &changes).unwrap();
        let parsed = parse_layer(&schema, Level::System, &out);
        assert_eq!(parsed.issues, []);
        for change in &changes {
            let Change::Set { key, value } = change else {
                unreachable!()
            };
            assert_eq!(parsed.layer.get(key), Some(value));
        }
    }
}
