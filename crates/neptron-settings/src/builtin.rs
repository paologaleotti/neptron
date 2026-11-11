use crate::schema::{Entry, Kind, Levels, Safety, Schema};

/// The version of the file layout that this build writes into each settings file.
///
/// The layout is how a key maps to TOML tables and which TOML type each kind uses. The
/// version never changes for an added key or an added choice: an older reader reports a new
/// key as unknown, keeps it in the file and uses the level below. The version changes only
/// for a layout change that an older reader would misread, and the append-only rule forbids
/// such a change for the existing keys.
pub const SCHEMA_VERSION: i64 = 1;

/// The top-level key that holds the schema version in each settings file.
///
/// No setting can use it: every setting key has two or more segments.
pub const VERSION_KEY: &str = "schema_version";

/// The picture style choices, in menu order. "Native, double refresh" is the style `native`
/// with the refresh policy `exact_x2`.
pub const PICTURE_STYLES: &[&str] = &["auto", "native", "2x", "3x", "4x"];

/// The refresh policy choices, in menu order.
pub const REFRESH_POLICIES: &[&str] = &["exact", "exact_x2", "force_50", "force_60"];

/// The scanline choices, in menu order. `auto` draws scanlines where the tube cannot show
/// real ones: a whole vertical multiple of at least 2 on a CRT that is not in a 15 kHz mode.
/// Later scanline kinds are appended.
pub const SCANLINE_MODES: &[&str] = &["auto", "off", "on"];

/// The strongest scanline, in percent: black lines.
pub const SCANLINE_STRENGTH_MAX_PERCENT: i64 = 100;

/// The default scanline strength, in percent. MiSTer offers 25, 50 and 75 for VGA monitors
/// and no 100, and users on PC CRTs mostly pick 75. Black lines on every second line halve
/// the light, and an older tube is dim.
pub const SCANLINE_STRENGTH_DEFAULT_PERCENT: i64 = 75;

/// The longest connector name or monitor profile id, in bytes.
pub const NAME_MAX_LEN: usize = 32;

/// The key of the output connector: `auto` or a DRM connector name such as `VGA-1`.
pub const DISPLAY_OUTPUT: &str = "display.output";
/// The key of the monitor profile: `auto` or a profile id such as `generic_15`.
pub const DISPLAY_MONITOR: &str = "display.monitor";
/// The key of the picture style.
pub const DISPLAY_STYLE: &str = "display.style";
/// The key of the refresh policy.
pub const DISPLAY_REFRESH_POLICY: &str = "display.refresh_policy";
/// The key of the Start hold that opens the quick menu, in whole seconds.
///
/// Help text: "Hold Start alone on a controller for this number of seconds to open the quick
/// menu. 0 turns it off. Select and Start together still open the quick menu at once. Use it
/// on an arcade panel that has no Select button."
pub const INPUT_MENU_HOLD_START_SECONDS: &str = "input.menu_hold_start_seconds";

/// The longest Start hold that [`INPUT_MENU_HOLD_START_SECONDS`] allows, in seconds.
pub const MENU_HOLD_START_MAX_SECONDS: i64 = 10;

/// The key of the scanline mode: one of [`SCANLINE_MODES`].
pub const DISPLAY_SCANLINES: &str = "display.scanlines";
/// The key of the scanline strength, in percent.
pub const DISPLAY_SCANLINE_STRENGTH: &str = "display.scanline_strength";

static BUILTIN: [Entry; 7] = [
    Entry {
        key: DISPLAY_OUTPUT,
        label: "Output",
        kind: Kind::Name {
            max_len: NAME_MAX_LEN,
            default: "auto",
        },
        levels: Levels::GLOBAL,
        live: true,
        safety: Safety::ConfirmOrRevert,
    },
    Entry {
        key: DISPLAY_MONITOR,
        label: "Monitor profile",
        kind: Kind::Name {
            max_len: NAME_MAX_LEN,
            default: "auto",
        },
        levels: Levels::GLOBAL,
        live: true,
        safety: Safety::ConfirmOrRevert,
    },
    Entry {
        key: DISPLAY_STYLE,
        label: "Picture style",
        kind: Kind::Choice {
            choices: PICTURE_STYLES,
            default: "auto",
        },
        levels: Levels::ALL,
        live: true,
        safety: Safety::Validated,
    },
    Entry {
        key: DISPLAY_REFRESH_POLICY,
        label: "Refresh policy",
        kind: Kind::Choice {
            choices: REFRESH_POLICIES,
            default: "exact",
        },
        levels: Levels::ALL,
        live: true,
        safety: Safety::Validated,
    },
    Entry {
        key: INPUT_MENU_HOLD_START_SECONDS,
        label: "Hold Start for quick menu",
        kind: Kind::Int {
            min: 0,
            max: MENU_HOLD_START_MAX_SECONDS,
            default: 0,
        },
        levels: Levels::GLOBAL,
        live: true,
        safety: Safety::None,
    },
    Entry {
        key: DISPLAY_SCANLINES,
        label: "Scanlines",
        kind: Kind::Choice {
            choices: SCANLINE_MODES,
            default: "auto",
        },
        levels: Levels::ALL,
        live: true,
        safety: Safety::None,
    },
    Entry {
        key: DISPLAY_SCANLINE_STRENGTH,
        label: "Scanline strength",
        kind: Kind::Int {
            min: 0,
            max: SCANLINE_STRENGTH_MAX_PERCENT,
            default: SCANLINE_STRENGTH_DEFAULT_PERCENT,
        },
        levels: Levels::ALL,
        live: true,
        safety: Safety::None,
    },
];

impl Schema {
    /// The built-in schema: every setting that NEPTRON knows, in menu order.
    pub fn builtin() -> Self {
        // The test `builtin_entries_pass_the_schema_check` proves these entries valid.
        Schema::from_entries_unchecked(BUILTIN.to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{Invalid, Level, Source, Value, ValueType};
    use crate::{Change, Issue, IssueKind, Layer, Layers, edit, parse_layer, resolve_key};

    /// One line per key, with the parts that a file depends on: type, levels and choices.
    fn describe(entry: &Entry) -> String {
        let levels: Vec<_> = entry.levels.iter().map(Level::name).collect();
        let kind = match entry.kind {
            Kind::Bool { .. } => "bool".to_owned(),
            Kind::Int { .. } => "int".to_owned(),
            Kind::Choice { choices, .. } => format!("choice {}", choices.join("|")),
            Kind::Name { .. } => "name".to_owned(),
        };
        format!("{} {} {}", entry.key, levels.join(","), kind)
    }

    #[test]
    fn builtin_entries_pass_the_schema_check() {
        assert_eq!(
            Schema::from_entries(BUILTIN.to_vec()),
            Ok(Schema::builtin())
        );
    }

    #[test]
    fn builtin_keys_match_the_append_only_key_list() {
        let fixture = include_str!("../tests/fixtures/keys.txt");
        let mut lines = fixture
            .lines()
            .filter(|l| !l.is_empty() && !l.starts_with('#'));
        assert_eq!(
            lines.next(),
            Some(format!("{VERSION_KEY} {SCHEMA_VERSION}").as_str()),
            "SCHEMA_VERSION changed. It changes only with the file layout, never for an \
             added key or choice. If the layout really changed, update the first line of \
             tests/fixtures/keys.txt."
        );
        let expected: Vec<_> = lines.collect();
        let actual: Vec<_> = Schema::builtin().entries().iter().map(describe).collect();
        assert_eq!(
            actual, expected,
            "The schema does not match tests/fixtures/keys.txt. Keys and choices are \
             append-only: to add a key, append its line; to add a choice, append it to the \
             end of its line. Never rename or remove a key or a choice, and do not change \
             SCHEMA_VERSION for an addition."
        );
    }

    #[test]
    fn builtin_defaults_are_the_first_boot_defaults() {
        let schema = Schema::builtin();
        let default = |key| schema.get(key).unwrap().kind.default_value().to_string();
        assert_eq!(default(DISPLAY_OUTPUT), "auto");
        assert_eq!(default(DISPLAY_MONITOR), "auto");
        assert_eq!(default(DISPLAY_STYLE), "auto");
        assert_eq!(default(DISPLAY_REFRESH_POLICY), "exact");
        assert_eq!(default(INPUT_MENU_HOLD_START_SECONDS), "0");
        assert_eq!(default(DISPLAY_SCANLINES), "auto");
        assert_eq!(default(DISPLAY_SCANLINE_STRENGTH), "75");
    }

    #[test]
    fn scanline_settings_change_no_signal_and_take_whole_percent() {
        let schema = Schema::builtin();
        for key in [DISPLAY_SCANLINES, DISPLAY_SCANLINE_STRENGTH] {
            let entry = schema.get(key).unwrap();
            assert_eq!(entry.safety, Safety::None, "{key}");
            assert_eq!(entry.levels, Levels::ALL, "{key}");
            assert!(entry.live, "{key}");
        }
        let strength = schema.get(DISPLAY_SCANLINE_STRENGTH).unwrap();
        assert_eq!(strength.kind.check(&Value::Int(0)), Ok(()));
        assert_eq!(strength.kind.check(&Value::Int(100)), Ok(()));
        assert!(strength.kind.check(&Value::Int(101)).is_err());
        assert!(strength.kind.check(&Value::Int(-1)).is_err());
        let mode = schema.get(DISPLAY_SCANLINES).unwrap();
        assert!(mode.kind.check(&Value::Text("black".into())).is_err());
    }

    #[test]
    fn the_start_hold_is_a_global_whole_number_of_seconds_from_0_to_10() {
        let schema = Schema::builtin();
        let entry = schema
            .entries()
            .iter()
            .find(|entry| entry.key == INPUT_MENU_HOLD_START_SECONDS)
            .unwrap();
        assert_eq!(
            entry.kind,
            Kind::Int {
                min: 0,
                max: 10,
                default: 0
            }
        );
        assert_eq!(entry.levels, Levels::GLOBAL);
        assert_eq!(entry.safety, Safety::None);
        assert!(entry.live);
        assert!(!entry.label.is_empty());
    }

    #[test]
    fn the_start_hold_accepts_0_to_10_seconds_in_a_global_file() {
        let schema = Schema::builtin();
        for seconds in [0, 1, 2, MENU_HOLD_START_MAX_SECONDS] {
            let src = format!("[input]\nmenu_hold_start_seconds = {seconds}\n");
            let parsed = parse_layer(&schema, Level::Global, &src);
            assert_eq!(parsed.issues, [], "{src}");
            assert_eq!(
                parsed.layer.get(INPUT_MENU_HOLD_START_SECONDS),
                Some(&Value::Int(seconds))
            );
        }
    }

    #[test]
    fn the_start_hold_rejects_values_out_of_range_or_of_another_type() {
        let schema = Schema::builtin();
        for (value, reason) in [
            (
                "-1",
                Invalid::OutOfRange {
                    value: -1,
                    min: 0,
                    max: 10,
                },
            ),
            (
                "11",
                Invalid::OutOfRange {
                    value: 11,
                    min: 0,
                    max: 10,
                },
            ),
            (
                "\"2\"",
                Invalid::WrongType {
                    expected: ValueType::Int,
                    found: "string",
                },
            ),
            (
                "true",
                Invalid::WrongType {
                    expected: ValueType::Int,
                    found: "boolean",
                },
            ),
        ] {
            let src = format!("[input]\nmenu_hold_start_seconds = {value}\n");
            let parsed = parse_layer(&schema, Level::Global, &src);
            assert_eq!(parsed.layer.get(INPUT_MENU_HOLD_START_SECONDS), None);
            assert_eq!(
                parsed.issues,
                [Issue {
                    line: Some(2),
                    key: Some(INPUT_MENU_HOLD_START_SECONDS.to_owned()),
                    kind: IssueKind::Invalid { reason },
                }],
                "{src}"
            );
        }
    }

    #[test]
    fn the_start_hold_cannot_be_set_for_a_system_or_a_game() {
        let schema = Schema::builtin();
        for level in [Level::System, Level::Game] {
            let parsed = parse_layer(&schema, level, "[input]\nmenu_hold_start_seconds = 2\n");
            assert_eq!(parsed.layer.get(INPUT_MENU_HOLD_START_SECONDS), None);
            assert_eq!(parsed.issues.len(), 1);
            assert_eq!(parsed.issues[0].kind, IssueKind::LevelNotAllowed { level });
            let mut layer = Layer::new(level);
            assert!(
                layer
                    .set(&schema, INPUT_MENU_HOLD_START_SECONDS, Value::Int(2))
                    .is_err()
            );
        }
    }

    #[test]
    fn the_start_hold_survives_an_edit_and_a_parse() {
        let schema = Schema::builtin();
        let change = Change::Set {
            key: INPUT_MENU_HOLD_START_SECONDS,
            value: Value::Int(2),
        };
        let out = edit(&schema, Level::Global, "", &[change]).unwrap();
        assert_eq!(
            out,
            "schema_version = 1\n\n[input]\nmenu_hold_start_seconds = 2\n"
        );
        let parsed = parse_layer(&schema, Level::Global, &out);
        assert_eq!(parsed.issues, []);
        assert_eq!(
            parsed.layer.get(INPUT_MENU_HOLD_START_SECONDS),
            Some(&Value::Int(2))
        );
        let layers = Layers::try_new(Some(&parsed.layer), None, None).unwrap();
        let resolved = resolve_key(&schema, &layers, INPUT_MENU_HOLD_START_SECONDS).unwrap();
        assert_eq!(resolved.value, Value::Int(2));
        assert_eq!(resolved.source, Source::Global);
    }

    #[test]
    fn output_and_profile_changes_need_confirm_or_revert() {
        let schema = Schema::builtin();
        for key in [DISPLAY_OUTPUT, DISPLAY_MONITOR] {
            assert_eq!(schema.get(key).unwrap().safety, Safety::ConfirmOrRevert);
            assert_eq!(schema.get(key).unwrap().levels, Levels::GLOBAL);
        }
        for key in [DISPLAY_STYLE, DISPLAY_REFRESH_POLICY] {
            assert_eq!(schema.get(key).unwrap().safety, Safety::Validated);
            assert_eq!(schema.get(key).unwrap().levels, Levels::ALL);
        }
    }
}
