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

static BUILTIN: [Entry; 4] = [
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
    use crate::schema::Level;

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
