use std::fmt;

/// A level at which a settings file stores a value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Level {
    /// One game: `games/<system>/<game>.toml`.
    Game,
    /// One system: `systems/<system>.toml`.
    System,
    /// Every game: `global.toml`.
    Global,
}

impl Level {
    /// Every level, in resolution order: the first level that holds a value wins.
    pub const ORDER: [Level; 3] = [Level::Game, Level::System, Level::Global];

    /// The lower-case name that the control API and the diagnostics show.
    pub fn name(self) -> &'static str {
        match self {
            Level::Game => "game",
            Level::System => "system",
            Level::Global => "global",
        }
    }
}

impl fmt::Display for Level {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Where a resolved value comes from: one of the three levels, or the built-in default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Source {
    /// The game file.
    Game,
    /// The system file.
    System,
    /// The global file.
    Global,
    /// The schema default. No file holds a valid value.
    Default,
}

impl Source {
    /// The lower-case name that the control API and the UI show.
    pub fn name(self) -> &'static str {
        match self {
            Source::Game => "game",
            Source::System => "system",
            Source::Global => "global",
            Source::Default => "default",
        }
    }
}

impl From<Level> for Source {
    fn from(level: Level) -> Self {
        match level {
            Level::Game => Source::Game,
            Level::System => Source::System,
            Level::Global => Source::Global,
        }
    }
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// The set of levels at which a key may be set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Levels {
    game: bool,
    system: bool,
    global: bool,
}

impl Levels {
    /// Game, system and global.
    pub const ALL: Levels = Levels::new(true, true, true);
    /// Global only: a setting of the machine, not of a game.
    pub const GLOBAL: Levels = Levels::new(false, false, true);

    /// A set from one flag per level. [`Schema::from_entries`] rejects an empty set.
    pub const fn new(game: bool, system: bool, global: bool) -> Self {
        Levels {
            game,
            system,
            global,
        }
    }

    /// True when a value may be set at `level`.
    pub fn allows(self, level: Level) -> bool {
        match level {
            Level::Game => self.game,
            Level::System => self.system,
            Level::Global => self.global,
        }
    }

    /// True when no level is in the set.
    pub fn is_empty(self) -> bool {
        !(self.game || self.system || self.global)
    }

    /// The levels in the set, in resolution order.
    pub fn iter(self) -> impl Iterator<Item = Level> {
        Level::ORDER.into_iter().filter(move |l| self.allows(*l))
    }
}

/// How a change to a setting meets the CRT safety rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Safety {
    /// The setting has no effect on the video signal.
    None,
    /// The setting changes the video mode. Every mode it gives passes the safety validator.
    Validated,
    /// The setting changes the output or the monitor profile. A user change also applies
    /// under confirm-or-revert, and only a confirmed value becomes last known good.
    ConfirmOrRevert,
}

/// The type of a setting, with its allowed values and its built-in default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// `true` or `false`.
    Bool {
        /// The built-in default.
        default: bool,
    },
    /// A whole number in `min..=max`.
    Int {
        /// The smallest allowed value.
        min: i64,
        /// The largest allowed value.
        max: i64,
        /// The built-in default.
        default: i64,
    },
    /// One string from a fixed list. The list is append-only.
    Choice {
        /// The allowed strings, in menu order.
        choices: &'static [&'static str],
        /// The built-in default.
        default: &'static str,
    },
    /// An identifier from an open set, such as a connector or a monitor profile id. The
    /// consumer checks that the named thing exists; the schema checks only the form: 1 to
    /// `max_len` ASCII letters, digits, `-` and `_`.
    Name {
        /// The longest allowed name, in bytes.
        max_len: usize,
        /// The built-in default.
        default: &'static str,
    },
}

impl Kind {
    /// The built-in default as a value.
    pub fn default_value(&self) -> Value {
        match *self {
            Kind::Bool { default } => Value::Bool(default),
            Kind::Int { default, .. } => Value::Int(default),
            Kind::Choice { default, .. } | Kind::Name { default, .. } => {
                Value::Text(default.to_owned())
            }
        }
    }

    /// The value type that a file must hold for this kind.
    pub fn value_type(&self) -> ValueType {
        match self {
            Kind::Bool { .. } => ValueType::Bool,
            Kind::Int { .. } => ValueType::Int,
            Kind::Choice { .. } | Kind::Name { .. } => ValueType::Text,
        }
    }

    /// Checks the type and the range of `value`.
    pub fn check(&self, value: &Value) -> Result<(), Invalid> {
        match (*self, value) {
            (Kind::Bool { .. }, Value::Bool(_)) => Ok(()),
            (Kind::Int { min, max, .. }, Value::Int(v)) => {
                if (min..=max).contains(v) {
                    Ok(())
                } else {
                    Err(Invalid::OutOfRange {
                        value: *v,
                        min,
                        max,
                    })
                }
            }
            (Kind::Choice { choices, .. }, Value::Text(v)) => {
                if choices.contains(&v.as_str()) {
                    Ok(())
                } else {
                    Err(Invalid::NotAChoice {
                        value: v.clone(),
                        choices,
                    })
                }
            }
            (Kind::Name { max_len, .. }, Value::Text(v)) => {
                if is_name(v, max_len) {
                    Ok(())
                } else {
                    Err(Invalid::BadName {
                        value: v.clone(),
                        max_len,
                    })
                }
            }
            (kind, value) => Err(Invalid::WrongType {
                expected: kind.value_type(),
                found: value.value_type().name(),
            }),
        }
    }
}

/// A setting value.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Value {
    /// A boolean.
    Bool(bool),
    /// A whole number.
    Int(i64),
    /// A string: a choice or a name.
    Text(String),
}

impl Value {
    /// The type of this value.
    pub fn value_type(&self) -> ValueType {
        match self {
            Value::Bool(_) => ValueType::Bool,
            Value::Int(_) => ValueType::Int,
            Value::Text(_) => ValueType::Text,
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Bool(v) => write!(f, "{v}"),
            Value::Int(v) => write!(f, "{v}"),
            Value::Text(v) => f.write_str(v),
        }
    }
}

/// The type of a value, with the TOML name of the type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ValueType {
    /// A TOML boolean.
    Bool,
    /// A TOML integer.
    Int,
    /// A TOML string.
    Text,
}

impl ValueType {
    /// The TOML name of the type.
    pub fn name(self) -> &'static str {
        match self {
            ValueType::Bool => "boolean",
            ValueType::Int => "integer",
            ValueType::Text => "string",
        }
    }
}

impl fmt::Display for ValueType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Why a value is not valid for a setting.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Invalid {
    /// The value has a different type.
    #[error("expected a {expected}, found a {found}")]
    WrongType {
        /// The type the setting needs.
        expected: ValueType,
        /// The TOML name of the type found.
        found: &'static str,
    },
    /// The number is outside the allowed range.
    #[error("{value} is outside {min}..={max}")]
    OutOfRange {
        /// The value found.
        value: i64,
        /// The smallest allowed value.
        min: i64,
        /// The largest allowed value.
        max: i64,
    },
    /// The string is not in the list of choices.
    #[error("\"{value}\" is not one of: {}", choices.join(", "))]
    NotAChoice {
        /// The value found.
        value: String,
        /// The allowed strings.
        choices: &'static [&'static str],
    },
    /// The string is not a valid name.
    #[error("\"{value}\" is not a name of 1 to {max_len} ASCII letters, digits, '-' and '_'")]
    BadName {
        /// The value found.
        value: String,
        /// The longest allowed name, in bytes.
        max_len: usize,
    },
}

/// One setting: everything the resolution, the files and the UI crawler need.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Entry {
    /// The dotted key, such as `display.style`. In a file, every segment but the last is a
    /// table. Keys are append-only: a key is never renamed or removed.
    pub key: &'static str,
    /// The label that the menu shows.
    pub label: &'static str,
    /// The type, the allowed values and the default.
    pub kind: Kind,
    /// The levels at which a value may be set.
    pub levels: Levels,
    /// True when a change takes effect at once, with no restart of the game.
    pub live: bool,
    /// How a change meets the CRT safety rules.
    pub safety: Safety,
}

/// Why a list of entries is not a valid schema.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SchemaError {
    /// The key is not two or more dot-separated segments of `a-z`, `0-9` and `_`.
    #[error("key \"{key}\" is not two or more dot-separated segments of a-z, 0-9 and '_'")]
    BadKey {
        /// The key.
        key: &'static str,
    },
    /// Two entries have the same key.
    #[error("key \"{key}\" is declared twice")]
    DuplicateKey {
        /// The key.
        key: &'static str,
    },
    /// One key is a table that holds the other, so a file cannot store both.
    #[error("key \"{key}\" is a table that holds key \"{inner}\"")]
    KeyClash {
        /// The shorter key.
        key: &'static str,
        /// The longer key, inside the shorter one.
        inner: &'static str,
    },
    /// The entry has an empty label.
    #[error("key \"{key}\" has an empty label")]
    EmptyLabel {
        /// The key.
        key: &'static str,
    },
    /// The entry allows no level.
    #[error("key \"{key}\" allows no level")]
    NoLevel {
        /// The key.
        key: &'static str,
    },
    /// The integer range is empty.
    #[error("key \"{key}\" has an empty range {min}..={max}")]
    EmptyRange {
        /// The key.
        key: &'static str,
        /// The smallest value.
        min: i64,
        /// The largest value.
        max: i64,
    },
    /// A choice is empty or listed twice.
    #[error("key \"{key}\" has an empty or repeated choice \"{choice}\"")]
    BadChoice {
        /// The key.
        key: &'static str,
        /// The choice.
        choice: &'static str,
    },
    /// The longest allowed name is zero bytes.
    #[error("key \"{key}\" allows no name length")]
    ZeroLength {
        /// The key.
        key: &'static str,
    },
    /// The default is not a valid value of the kind.
    #[error("key \"{key}\" has an invalid default: {reason}")]
    BadDefault {
        /// The key.
        key: &'static str,
        /// Why the default is invalid.
        reason: Invalid,
    },
}

/// The typed schema: every setting, in menu order. The UI crawler walks [`Schema::entries`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Schema {
    entries: Vec<Entry>,
}

impl Schema {
    /// Makes a schema from entries in menu order, after a check of every entry.
    pub fn from_entries(entries: Vec<Entry>) -> Result<Self, SchemaError> {
        for (i, entry) in entries.iter().enumerate() {
            check_entry(entry)?;
            for other in &entries[..i] {
                check_pair(other, entry)?;
            }
        }
        Ok(Schema { entries })
    }

    /// Makes a schema with no check. Only for entries that a test proves valid.
    pub(crate) fn from_entries_unchecked(entries: Vec<Entry>) -> Self {
        Schema { entries }
    }

    /// Every entry, in menu order.
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// The entry for `key`.
    pub fn get(&self, key: &str) -> Option<&Entry> {
        self.entries.iter().find(|e| e.key == key)
    }
}

/// True when `segment` is a valid key segment: `a-z`, `0-9` and `_`, at least one byte.
pub(crate) fn is_segment(segment: &str) -> bool {
    !segment.is_empty()
        && segment
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

fn is_name(value: &str, max_len: usize) -> bool {
    !value.is_empty()
        && value.len() <= max_len
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

fn check_entry(entry: &Entry) -> Result<(), SchemaError> {
    let key = entry.key;
    if key.split('.').count() < 2 || !key.split('.').all(is_segment) {
        return Err(SchemaError::BadKey { key });
    }
    if entry.label.trim().is_empty() {
        return Err(SchemaError::EmptyLabel { key });
    }
    if entry.levels.is_empty() {
        return Err(SchemaError::NoLevel { key });
    }
    match entry.kind {
        Kind::Int { min, max, .. } if min > max => {
            return Err(SchemaError::EmptyRange { key, min, max });
        }
        Kind::Choice { choices, .. } => {
            for (i, choice) in choices.iter().enumerate() {
                if choice.is_empty() || choices[..i].contains(choice) {
                    return Err(SchemaError::BadChoice { key, choice });
                }
            }
        }
        Kind::Name { max_len: 0, .. } => return Err(SchemaError::ZeroLength { key }),
        _ => {}
    }
    entry
        .kind
        .check(&entry.kind.default_value())
        .map_err(|reason| SchemaError::BadDefault { key, reason })
}

fn check_pair(earlier: &Entry, later: &Entry) -> Result<(), SchemaError> {
    if earlier.key == later.key {
        return Err(SchemaError::DuplicateKey { key: later.key });
    }
    for (outer, inner) in [(earlier.key, later.key), (later.key, earlier.key)] {
        if inner
            .strip_prefix(outer)
            .is_some_and(|rest| rest.starts_with('.'))
        {
            return Err(SchemaError::KeyClash { key: outer, inner });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const CHOICES: &[&str] = &["a", "b"];

    fn entry(key: &'static str, kind: Kind) -> Entry {
        Entry {
            key,
            label: "Label",
            kind,
            levels: Levels::ALL,
            live: true,
            safety: Safety::None,
        }
    }

    fn int(min: i64, max: i64, default: i64) -> Kind {
        Kind::Int { min, max, default }
    }

    #[test]
    fn a_valid_schema_keeps_menu_order_and_finds_keys() {
        let schema = Schema::from_entries(vec![
            entry("b.second", Kind::Bool { default: true }),
            entry("a.first", int(0, 9, 3)),
        ])
        .unwrap();
        let keys: Vec<_> = schema.entries().iter().map(|e| e.key).collect();
        assert_eq!(keys, ["b.second", "a.first"]);
        assert_eq!(schema.get("a.first").unwrap().kind, int(0, 9, 3));
        assert!(schema.get("a").is_none());
        assert!(schema.get("a.first.x").is_none());
    }

    #[test]
    fn bad_keys_are_rejected() {
        for key in [
            "single",
            "",
            ".a",
            "a.",
            "a..b",
            "A.b",
            "a.b-c",
            "a. b",
            "schema_version",
        ] {
            let err =
                Schema::from_entries(vec![entry(key, Kind::Bool { default: false })]).unwrap_err();
            assert_eq!(err, SchemaError::BadKey { key }, "{key}");
        }
    }

    #[test]
    fn duplicate_and_nested_keys_are_rejected() {
        let b = Kind::Bool { default: false };
        assert_eq!(
            Schema::from_entries(vec![entry("a.b", b), entry("a.b", b)]).unwrap_err(),
            SchemaError::DuplicateKey { key: "a.b" }
        );
        assert_eq!(
            Schema::from_entries(vec![entry("a.b", b), entry("a.b.c", b)]).unwrap_err(),
            SchemaError::KeyClash {
                key: "a.b",
                inner: "a.b.c"
            }
        );
        assert_eq!(
            Schema::from_entries(vec![entry("a.b.c", b), entry("a.b", b)]).unwrap_err(),
            SchemaError::KeyClash {
                key: "a.b",
                inner: "a.b.c"
            }
        );
        assert!(Schema::from_entries(vec![entry("a.b", b), entry("a.bc", b)]).is_ok());
    }

    #[test]
    fn bad_entries_are_rejected() {
        let mut e = entry("a.b", Kind::Bool { default: false });
        e.label = " ";
        assert_eq!(
            Schema::from_entries(vec![e]).unwrap_err(),
            SchemaError::EmptyLabel { key: "a.b" }
        );

        let mut e = entry("a.b", Kind::Bool { default: false });
        e.levels = Levels::new(false, false, false);
        assert_eq!(
            Schema::from_entries(vec![e]).unwrap_err(),
            SchemaError::NoLevel { key: "a.b" }
        );

        assert_eq!(
            Schema::from_entries(vec![entry("a.b", int(5, 4, 5))]).unwrap_err(),
            SchemaError::EmptyRange {
                key: "a.b",
                min: 5,
                max: 4
            }
        );
        assert_eq!(
            Schema::from_entries(vec![entry("a.b", int(0, 4, 5))]).unwrap_err(),
            SchemaError::BadDefault {
                key: "a.b",
                reason: Invalid::OutOfRange {
                    value: 5,
                    min: 0,
                    max: 4
                }
            }
        );
        for (choices, bad) in [(&["a", "a"][..], "a"), (&["a", ""][..], "")] {
            let kind = Kind::Choice {
                choices,
                default: "a",
            };
            assert_eq!(
                Schema::from_entries(vec![entry("a.b", kind)]).unwrap_err(),
                SchemaError::BadChoice {
                    key: "a.b",
                    choice: bad
                }
            );
        }
        let kind = Kind::Choice {
            choices: CHOICES,
            default: "c",
        };
        assert!(matches!(
            Schema::from_entries(vec![entry("a.b", kind)]).unwrap_err(),
            SchemaError::BadDefault { .. }
        ));
        let kind = Kind::Name {
            max_len: 0,
            default: "",
        };
        assert_eq!(
            Schema::from_entries(vec![entry("a.b", kind)]).unwrap_err(),
            SchemaError::ZeroLength { key: "a.b" }
        );
        let kind = Kind::Name {
            max_len: 3,
            default: "abcd",
        };
        assert!(matches!(
            Schema::from_entries(vec![entry("a.b", kind)]).unwrap_err(),
            SchemaError::BadDefault { .. }
        ));
    }

    #[test]
    fn check_accepts_valid_values_and_rejects_the_rest() {
        let b = Kind::Bool { default: false };
        assert_eq!(b.check(&Value::Bool(true)), Ok(()));
        assert_eq!(
            b.check(&Value::Int(1)),
            Err(Invalid::WrongType {
                expected: ValueType::Bool,
                found: "integer"
            })
        );

        let i = int(-2, 2, 0);
        for v in -2..=2 {
            assert_eq!(i.check(&Value::Int(v)), Ok(()));
        }
        for v in [-3, 3, i64::MIN, i64::MAX] {
            assert_eq!(
                i.check(&Value::Int(v)),
                Err(Invalid::OutOfRange {
                    value: v,
                    min: -2,
                    max: 2
                })
            );
        }
        assert!(matches!(
            i.check(&Value::Text("1".into())),
            Err(Invalid::WrongType {
                found: "string",
                ..
            })
        ));

        let c = Kind::Choice {
            choices: CHOICES,
            default: "a",
        };
        assert_eq!(c.check(&Value::Text("b".into())), Ok(()));
        assert!(matches!(
            c.check(&Value::Text("A".into())),
            Err(Invalid::NotAChoice { .. })
        ));
        assert!(matches!(
            c.check(&Value::Bool(true)),
            Err(Invalid::WrongType {
                found: "boolean",
                ..
            })
        ));

        let n = Kind::Name {
            max_len: 9,
            default: "auto",
        };
        for ok in ["VGA-1", "HDMI-A-1", "pc_31_120", "x", "123456789"] {
            assert_eq!(n.check(&Value::Text(ok.into())), Ok(()), "{ok}");
        }
        for bad in ["", "1234567890", "a b", "a/b", "a.b", "é", "../x"] {
            assert!(
                matches!(
                    n.check(&Value::Text(bad.into())),
                    Err(Invalid::BadName { .. })
                ),
                "{bad}"
            );
        }
    }

    #[test]
    fn invalid_messages_carry_the_value_and_the_limit() {
        let msg = Invalid::OutOfRange {
            value: 9,
            min: 0,
            max: 4,
        }
        .to_string();
        assert_eq!(msg, "9 is outside 0..=4");
        let msg = Invalid::NotAChoice {
            value: "x".into(),
            choices: CHOICES,
        }
        .to_string();
        assert_eq!(msg, "\"x\" is not one of: a, b");
    }

    #[test]
    fn levels_and_sources_have_stable_names() {
        assert_eq!(Level::ORDER, [Level::Game, Level::System, Level::Global]);
        let names: Vec<_> = Level::ORDER
            .iter()
            .map(|l| Source::from(*l).name())
            .collect();
        assert_eq!(names, ["game", "system", "global"]);
        assert_eq!(Source::Default.to_string(), "default");
        assert_eq!(Levels::GLOBAL.iter().collect::<Vec<_>>(), [Level::Global]);
        assert_eq!(Levels::ALL.iter().count(), 3);
    }

    #[test]
    fn values_print_as_the_menu_shows_them() {
        assert_eq!(Value::Bool(true).to_string(), "true");
        assert_eq!(Value::Int(-4).to_string(), "-4");
        assert_eq!(Value::Text("2x".into()).to_string(), "2x");
    }
}
