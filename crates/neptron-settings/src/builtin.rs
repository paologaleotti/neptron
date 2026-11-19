use crate::schema::{AppliesTo, Entry, Kind, Levels, Safety, Schema};

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

/// The picture mode choices, in menu order. The list is append-only, so `sharp`, `speed` and
/// `fit` come after `4x`. "Native, double refresh" is the mode `native` with the refresh
/// policy `exact_x2`.
pub const PICTURE_STYLES: &[&str] = &["auto", "native", "2x", "3x", "4x", "sharp", "speed", "fit"];

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
/// The key of the picture mode: one of [`PICTURE_STYLES`]. It keeps the name of the picture
/// style that it was before the modes `sharp`, `speed` and `fit`.
pub const DISPLAY_STYLE: &str = "display.style";
/// The key of the refresh policy: the rate target that every picture mode works with.
pub const DISPLAY_REFRESH_POLICY: &str = "display.refresh_policy";
/// The key of the Start hold that opens the quick menu, in whole seconds.
///
/// Help text: "Hold Start alone on a controller for this number of seconds to open the quick
/// menu. 0 turns it off. Select and Start held together for half a second still open the
/// quick menu. Use it on an arcade panel that has no Select button."
pub const INPUT_MENU_HOLD_START_SECONDS: &str = "input.menu_hold_start_seconds";

/// The longest Start hold that [`INPUT_MENU_HOLD_START_SECONDS`] allows, in seconds.
pub const MENU_HOLD_START_MAX_SECONDS: i64 = 10;

/// The key of the scanline mode: one of [`SCANLINE_MODES`].
pub const DISPLAY_SCANLINES: &str = "display.scanlines";
/// The key of the scanline strength, in percent.
pub const DISPLAY_SCANLINE_STRENGTH: &str = "display.scanline_strength";

/// The key of the user volume, in whole percent: 100 is the core's own level, 0 is silence.
///
/// Help text: "The volume of the games. 100 plays the sound at the level the game makes it."
pub const AUDIO_VOLUME: &str = "audio.volume";

/// The highest volume, in percent: unity gain.
pub const AUDIO_VOLUME_MAX_PERCENT: i64 = 100;

/// The default volume, in percent: -5.8 dB. The image sets the mixer to 0 dB, so 100 is the
/// full output of the codec, loud for a game whose samples reach full scale; 80 starts lower
/// and leaves the player room to go up.
pub const AUDIO_VOLUME_DEFAULT_PERCENT: i64 = 80;

/// The key of the playback device: `auto` (the automatic choice) or the name of a device as
/// the audio backend lists it: on ALSA a PCM name such as `hw:CARD=PCH,DEV=0`.
pub const AUDIO_OUTPUT: &str = "audio.output";

/// The longest playback device name that [`AUDIO_OUTPUT`] holds, in bytes.
pub const AUDIO_OUTPUT_MAX_LEN: usize = 128;

/// The key of the TV standard of a composite output: `auto` (NTSC) or one of
/// [`TV_STANDARDS`].
pub const DISPLAY_TV: &str = "display.tv";

/// The choices of [`DISPLAY_TV`], in menu order: `auto`, then the keys of the ten
/// standards of the encoder in the order of `TvStandard::ALL`.
pub const TV_STANDARDS: &[&str] = &[
    "auto", "ntsc", "ntsc_j", "ntsc_443", "pal_60", "pal_m", "pal", "pal_n", "secam", "mono_60",
    "mono_50",
];

/// The key of the width of the game on a TV: one of [`TV_WIDTHS`].
pub const DISPLAY_TV_WIDTH: &str = "display.tv_width";

/// The choices of [`DISPLAY_TV_WIDTH`], in menu order. `aspect` gives the console's own
/// shape, `integer` a whole factor of the game width, `fill` the full 4:3 width.
pub const TV_WIDTHS: &[&str] = &["aspect", "integer", "fill"];

/// The key of how a vertical game shows on a horizontal display: one of
/// [`VERTICAL_MODES`]. An expert key: the picture modes `fit` and `sharp` are the menu choices.
///
/// Help text: "How a vertical game shows on a horizontal screen. Exact shows every pixel
/// exact, with black borders, and on a 15 kHz screen or a TV with flicker. Fit fills the
/// height, with a soft picture."
pub const DISPLAY_VERTICAL: &str = "display.vertical";

/// The choices of [`DISPLAY_VERTICAL`], in menu order.
pub const VERTICAL_MODES: &[&str] = &["exact", "fit"];

/// The key of how a flat panel scales a game: one of [`PANEL_SCALES`]. An expert key: the
/// picture modes `sharp` and `fit` are the menu choices.
///
/// Help text: "How a game fills a flat panel. Auto fills the height when the picture is
/// large, and uses whole steps when it is small. Integer always uses whole steps, with
/// black borders."
pub const DISPLAY_PANEL_SCALE: &str = "display.panel_scale";

/// The choices of [`DISPLAY_PANEL_SCALE`], in menu order.
pub const PANEL_SCALES: &[&str] = &["auto", "integer"];

static BUILTIN: [Entry; 13] = [
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
        applies_to: AppliesTo::Every,
        expert: false,
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
        applies_to: AppliesTo::NotTv,
        expert: false,
    },
    Entry {
        key: DISPLAY_STYLE,
        label: "Picture mode",
        kind: Kind::Choice {
            choices: PICTURE_STYLES,
            default: "auto",
        },
        levels: Levels::ALL,
        live: true,
        safety: Safety::Validated,
        applies_to: AppliesTo::Every,
        expert: false,
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
        applies_to: AppliesTo::CrtNotTv,
        expert: false,
    },
    Entry {
        key: INPUT_MENU_HOLD_START_SECONDS,
        label: "Hold Start for menu",
        kind: Kind::Int {
            min: 0,
            max: MENU_HOLD_START_MAX_SECONDS,
            default: 0,
        },
        levels: Levels::GLOBAL,
        live: true,
        safety: Safety::None,
        applies_to: AppliesTo::Every,
        expert: false,
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
        applies_to: AppliesTo::Every,
        expert: false,
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
        applies_to: AppliesTo::Every,
        expert: false,
    },
    Entry {
        key: AUDIO_VOLUME,
        label: "Volume",
        kind: Kind::Int {
            min: 0,
            max: AUDIO_VOLUME_MAX_PERCENT,
            default: AUDIO_VOLUME_DEFAULT_PERCENT,
        },
        levels: Levels::ALL,
        live: true,
        safety: Safety::None,
        applies_to: AppliesTo::Every,
        expert: false,
    },
    Entry {
        key: AUDIO_OUTPUT,
        label: "Audio output",
        kind: Kind::Text {
            max_len: AUDIO_OUTPUT_MAX_LEN,
            default: "auto",
        },
        levels: Levels::GLOBAL,
        live: false,
        safety: Safety::None,
        applies_to: AppliesTo::Every,
        expert: false,
    },
    // The standard is the monitor of a TV, so it is global as the monitor is: the guard
    // keeps the monitor of the boot for every game, and a per-game standard would revert.
    Entry {
        key: DISPLAY_TV,
        label: "TV standard",
        kind: Kind::Choice {
            choices: TV_STANDARDS,
            default: "auto",
        },
        levels: Levels::GLOBAL,
        live: true,
        safety: Safety::ConfirmOrRevert,
        applies_to: AppliesTo::Tv,
        expert: false,
    },
    Entry {
        key: DISPLAY_TV_WIDTH,
        label: "TV picture width",
        kind: Kind::Choice {
            choices: TV_WIDTHS,
            default: "aspect",
        },
        levels: Levels::ALL,
        live: true,
        safety: Safety::Validated,
        applies_to: AppliesTo::Tv,
        expert: false,
    },
    Entry {
        key: DISPLAY_VERTICAL,
        label: "Vertical games",
        kind: Kind::Choice {
            choices: VERTICAL_MODES,
            default: "exact",
        },
        levels: Levels::ALL,
        live: true,
        safety: Safety::Validated,
        applies_to: AppliesTo::Every,
        expert: true,
    },
    Entry {
        key: DISPLAY_PANEL_SCALE,
        label: "Panel scale",
        kind: Kind::Choice {
            choices: PANEL_SCALES,
            default: "auto",
        },
        levels: Levels::ALL,
        live: true,
        safety: Safety::Validated,
        applies_to: AppliesTo::FlatPanel,
        expert: true,
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
    use crate::schema::{DisplayClass, Invalid, Level, Source, Value, ValueType};
    use crate::{Change, Issue, IssueKind, Layer, Layers, edit, parse_layer, resolve_key};

    /// One line per key, with the parts that a file depends on: type, levels and choices.
    fn describe(entry: &Entry) -> String {
        let levels: Vec<_> = entry.levels.iter().map(Level::name).collect();
        let kind = match entry.kind {
            Kind::Bool { .. } => "bool".to_owned(),
            Kind::Int { .. } => "int".to_owned(),
            Kind::Choice { choices, .. } => format!("choice {}", choices.join("|")),
            Kind::Name { .. } => "name".to_owned(),
            Kind::Text { .. } => "text".to_owned(),
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
        assert_eq!(default(AUDIO_VOLUME), "80");
        assert_eq!(default(AUDIO_OUTPUT), "auto");
        assert_eq!(default(DISPLAY_TV), "auto");
        assert_eq!(default(DISPLAY_TV_WIDTH), "aspect");
    }

    #[test]
    fn the_tv_standard_is_global_and_the_width_is_at_every_level() {
        let schema = Schema::builtin();
        let tv = schema.get(DISPLAY_TV).unwrap();
        assert_eq!(tv.levels, Levels::GLOBAL);
        assert_eq!(tv.safety, Safety::ConfirmOrRevert);
        assert_eq!(tv.applies_to, AppliesTo::Tv);
        let width = schema.get(DISPLAY_TV_WIDTH).unwrap();
        assert_eq!(width.levels, Levels::ALL);
        assert_eq!(width.safety, Safety::Validated);
        assert_eq!(width.applies_to, AppliesTo::Tv);
        let parsed = parse_layer(
            &schema,
            Level::Global,
            "[display]\ntv = \"pal_60\"\ntv_width = \"integer\"\n",
        );
        assert_eq!(parsed.issues, []);
        assert_eq!(
            parsed.layer.get(DISPLAY_TV),
            Some(&Value::Text("pal_60".into()))
        );
        let game = parse_layer(
            &schema,
            Level::Game,
            "[display]\ntv = \"pal\"\ntv_width = \"fill\"\n",
        );
        assert_eq!(game.layer.get(DISPLAY_TV), None);
        assert_eq!(
            game.issues[0].kind,
            IssueKind::LevelNotAllowed { level: Level::Game }
        );
        assert_eq!(
            game.layer.get(DISPLAY_TV_WIDTH),
            Some(&Value::Text("fill".into()))
        );
        let bad = parse_layer(&schema, Level::Global, "[display]\ntv = \"pal_70\"\n");
        assert_eq!(bad.layer.get(DISPLAY_TV), None);
        assert!(matches!(
            bad.issues[0].kind,
            IssueKind::Invalid {
                reason: Invalid::NotAChoice { .. }
            }
        ));
    }

    #[test]
    fn a_tv_setting_shows_only_on_a_tv_and_the_monitor_and_the_rate_never_apply_there() {
        let schema = Schema::builtin();
        for entry in schema.entries() {
            let tv_only = [DISPLAY_TV, DISPLAY_TV_WIDTH].contains(&entry.key);
            // The row of an expert key never shows; this test reads where a row would show.
            let panel_only = entry.key == DISPLAY_PANEL_SCALE;
            for class in [DisplayClass::Crt, DisplayClass::FlatPanel] {
                let panel = class == DisplayClass::FlatPanel;
                assert_eq!(
                    entry.applies_to.shown(class),
                    !tv_only && (!panel_only || panel),
                    "{} {class:?}",
                    entry.key
                );
            }
            assert_eq!(
                entry.applies_to.shown(DisplayClass::Tv),
                !panel_only,
                "{}",
                entry.key
            );
            assert_eq!(
                entry.applies_to.applies(DisplayClass::Tv),
                ![DISPLAY_MONITOR, DISPLAY_REFRESH_POLICY, DISPLAY_PANEL_SCALE]
                    .contains(&entry.key),
                "{}: the monitor and the rate come from the TV standard, and a TV is no panel",
                entry.key
            );
        }
    }

    #[test]
    fn the_volume_is_a_whole_percent_at_every_level_and_live() {
        let schema = Schema::builtin();
        let entry = schema.get(AUDIO_VOLUME).unwrap();
        assert_eq!(
            entry.kind,
            Kind::Int {
                min: 0,
                max: 100,
                default: 80
            }
        );
        assert_eq!(entry.levels, Levels::ALL);
        assert_eq!(entry.safety, Safety::None);
        assert!(entry.live);
        for level in Level::ORDER {
            let parsed = parse_layer(&schema, level, "[audio]\nvolume = 35\n");
            assert_eq!(parsed.issues, [], "{level}");
            assert_eq!(parsed.layer.get(AUDIO_VOLUME), Some(&Value::Int(35)));
        }
        for (value, reason) in [
            (
                "101",
                Invalid::OutOfRange {
                    value: 101,
                    min: 0,
                    max: 100,
                },
            ),
            (
                "-5",
                Invalid::OutOfRange {
                    value: -5,
                    min: 0,
                    max: 100,
                },
            ),
            (
                "\"loud\"",
                Invalid::WrongType {
                    expected: ValueType::Int,
                    found: "string",
                },
            ),
        ] {
            let src = format!("[audio]\nvolume = {value}\n");
            let parsed = parse_layer(&schema, Level::Game, &src);
            assert_eq!(parsed.layer.get(AUDIO_VOLUME), None);
            assert_eq!(
                parsed.issues,
                [Issue {
                    line: Some(2),
                    key: Some(AUDIO_VOLUME.to_owned()),
                    kind: IssueKind::Invalid { reason },
                }],
                "{src}"
            );
        }
    }

    #[test]
    fn a_game_volume_wins_over_the_system_and_the_global_one() {
        let schema = Schema::builtin();
        let global = parse_layer(&schema, Level::Global, "[audio]\nvolume = 80\n");
        let system = parse_layer(&schema, Level::System, "[audio]\nvolume = 60\n");
        let game = parse_layer(&schema, Level::Game, "[audio]\nvolume = 40\n");
        let layers =
            Layers::try_new(Some(&global.layer), Some(&system.layer), Some(&game.layer)).unwrap();
        let resolved = resolve_key(&schema, &layers, AUDIO_VOLUME).unwrap();
        assert_eq!(resolved.value, Value::Int(40));
        assert_eq!(resolved.source, Source::Game);
        let layers = Layers::try_new(Some(&global.layer), Some(&system.layer), None).unwrap();
        let resolved = resolve_key(&schema, &layers, AUDIO_VOLUME).unwrap();
        assert_eq!(resolved.value, Value::Int(60));
        let none = Layers::try_new(None, None, None).unwrap();
        let resolved = resolve_key(&schema, &none, AUDIO_VOLUME).unwrap();
        assert_eq!(
            (resolved.value, resolved.source),
            (Value::Int(80), Source::Default)
        );
    }

    #[test]
    fn the_output_holds_a_device_name_as_the_backend_lists_it() {
        let schema = Schema::builtin();
        let entry = schema.get(AUDIO_OUTPUT).unwrap();
        assert_eq!(entry.levels, Levels::GLOBAL);
        assert!(!entry.live, "a new device opens at the next start");
        assert_eq!(entry.safety, Safety::None);
        let change = Change::Set {
            key: AUDIO_OUTPUT,
            value: Value::Text("hw:CARD=PCH,DEV=0".into()),
        };
        let out = edit(&schema, Level::Global, "", &[change]).unwrap();
        assert_eq!(
            out,
            "schema_version = 1\n\n[audio]\noutput = \"hw:CARD=PCH,DEV=0\"\n"
        );
        let parsed = parse_layer(&schema, Level::Global, &out);
        assert_eq!(parsed.issues, []);
        assert_eq!(
            parsed.layer.get(AUDIO_OUTPUT),
            Some(&Value::Text("hw:CARD=PCH,DEV=0".into()))
        );
        for level in [Level::System, Level::Game] {
            let parsed = parse_layer(&schema, level, "[audio]\noutput = \"auto\"\n");
            assert_eq!(parsed.layer.get(AUDIO_OUTPUT), None);
            assert_eq!(parsed.issues[0].kind, IssueKind::LevelNotAllowed { level });
        }
        let long = "x".repeat(AUDIO_OUTPUT_MAX_LEN + 1);
        for bad in [String::new(), long, "a\\u0007b".to_owned()] {
            let src = format!("[audio]\noutput = \"{bad}\"\n");
            let parsed = parse_layer(&schema, Level::Global, &src);
            assert_eq!(parsed.layer.get(AUDIO_OUTPUT), None, "{src}");
            assert!(
                matches!(
                    parsed.issues[..],
                    [Issue {
                        kind: IssueKind::Invalid {
                            reason: Invalid::BadText { .. }
                        },
                        ..
                    }]
                ),
                "{src}: {:?}",
                parsed.issues
            );
        }
        let unknown = parse_layer(&schema, Level::Global, "[audio]\nbalance = 3\n");
        assert_eq!(unknown.issues[0].kind, IssueKind::UnknownKey);
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
    fn the_vertical_games_and_the_panel_scale_are_choices_of_every_level() {
        let schema = Schema::builtin();
        let vertical = schema.get(DISPLAY_VERTICAL).unwrap();
        assert_eq!(vertical.levels, Levels::ALL);
        assert_eq!(vertical.safety, Safety::Validated);
        assert_eq!(vertical.applies_to, AppliesTo::Every);
        assert!(matches!(
            vertical.kind,
            Kind::Choice {
                choices: ["exact", "fit"],
                default: "exact"
            }
        ));
        let panel = schema.get(DISPLAY_PANEL_SCALE).unwrap();
        assert_eq!(panel.levels, Levels::ALL);
        assert_eq!(panel.applies_to, AppliesTo::FlatPanel);
        assert!(panel.applies_to.applies(DisplayClass::FlatPanel));
        assert!(!panel.applies_to.applies(DisplayClass::Crt));
        assert!(!panel.applies_to.applies(DisplayClass::Tv));
        let game = parse_layer(
            &schema,
            Level::Game,
            "[display]\npanel_scale = \"integer\"\n",
        );
        assert!(game.issues.is_empty(), "{:?}", game.issues);
    }

    #[test]
    fn only_the_refresh_policy_applies_to_a_crt_alone() {
        let schema = Schema::builtin();
        for entry in schema.entries() {
            if [AppliesTo::Tv, AppliesTo::FlatPanel].contains(&entry.applies_to) {
                continue;
            }
            let crt_only = entry.key == DISPLAY_REFRESH_POLICY;
            assert!(entry.applies_to.applies(DisplayClass::Crt), "{}", entry.key);
            assert_eq!(
                entry.applies_to.applies(DisplayClass::FlatPanel),
                !crt_only,
                "{}",
                entry.key
            );
        }
    }

    #[test]
    fn the_picture_mode_applies_to_every_display_and_appends_its_new_modes() {
        let schema = Schema::builtin();
        let mode = schema.get(DISPLAY_STYLE).unwrap();
        assert_eq!(mode.label, "Picture mode");
        assert_eq!(mode.applies_to, AppliesTo::Every);
        assert_eq!(mode.levels, Levels::ALL);
        assert!(!mode.expert);
        for class in [DisplayClass::Crt, DisplayClass::Tv, DisplayClass::FlatPanel] {
            assert!(mode.shown(class), "{class:?}");
        }
        assert_eq!(&PICTURE_STYLES[..5], ["auto", "native", "2x", "3x", "4x"]);
        assert_eq!(&PICTURE_STYLES[5..], ["sharp", "speed", "fit"]);
        for level in Level::ORDER {
            for value in PICTURE_STYLES {
                let src = format!("[display]\nstyle = \"{value}\"\n");
                let parsed = parse_layer(&schema, level, &src);
                assert_eq!(parsed.issues, [], "{src}");
                assert_eq!(
                    parsed.layer.get(DISPLAY_STYLE),
                    Some(&Value::Text((*value).into()))
                );
            }
        }
    }

    #[test]
    fn the_keys_the_picture_mode_covers_are_expert_keys_that_keep_their_values() {
        let schema = Schema::builtin();
        for entry in schema.entries() {
            let expert = [DISPLAY_VERTICAL, DISPLAY_PANEL_SCALE].contains(&entry.key);
            assert_eq!(entry.expert, expert, "{}", entry.key);
            for class in [DisplayClass::Crt, DisplayClass::Tv, DisplayClass::FlatPanel] {
                if expert {
                    assert!(!entry.shown(class), "{} {class:?}", entry.key);
                } else {
                    assert_eq!(entry.shown(class), entry.applies_to.shown(class));
                }
            }
        }
        // A file of the picture style era: every value keeps its meaning.
        let old = "schema_version = 1\n\n[display]\nstyle = \"2x\"\nrefresh_policy = \"force_60\"\n\
                   vertical = \"fit\"\npanel_scale = \"integer\"\n";
        let parsed = parse_layer(&schema, Level::Game, old);
        assert_eq!(parsed.issues, []);
        let layers = Layers::try_new(None, None, Some(&parsed.layer)).unwrap();
        for (key, value) in [
            (DISPLAY_STYLE, "2x"),
            (DISPLAY_REFRESH_POLICY, "force_60"),
            (DISPLAY_VERTICAL, "fit"),
            (DISPLAY_PANEL_SCALE, "integer"),
        ] {
            let resolved = resolve_key(&schema, &layers, key).unwrap();
            assert_eq!(resolved.value, Value::Text(value.into()), "{key}");
            assert_eq!(resolved.source, Source::Game, "{key}");
        }
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
