//! File tests for the settings storage, each in its own temporary directory.

#![forbid(unsafe_code)]
#![cfg(unix)]

use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};

use neptron_settings::{
    Change, DISPLAY_MONITOR, DISPLAY_OUTPUT, DISPLAY_STYLE, EditError, IssueKind, Layers, Level,
    NEW_FILE_MODE, Schema, Source, Storage, StorageError, Target, Value, WriteStep, resolve_key,
};

/// A directory that is removed when the test ends.
struct TempDir(PathBuf);

impl TempDir {
    fn create(test: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("neptron-settings-{}-{test}", std::process::id()));
        if path.exists() {
            fs::remove_dir_all(&path).unwrap();
        }
        fs::create_dir_all(&path).unwrap();
        TempDir(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        // A leftover directory in the system temp folder does no harm.
        let _ = fs::remove_dir_all(&self.0);
    }
}

const NES: Target<'static> = Target::System { system: "nes" };
const SMB: Target<'static> = Target::Game {
    system: "nes",
    game: "Super Mario Bros. (World)",
};

fn text(v: &str) -> Value {
    Value::Text(v.to_owned())
}

fn set<'a>(key: &'a str, value: &str) -> Change<'a> {
    Change::Set {
        key,
        value: text(value),
    }
}

fn storage(dir: &TempDir) -> Storage {
    Storage::new(dir.path().join("config"))
}

fn files_in(dir: &Path) -> Vec<String> {
    let mut names: Vec<_> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn paths_follow_the_config_layout() {
    let s = Storage::new("/state/config");
    assert_eq!(s.root(), Path::new("/state/config"));
    assert_eq!(
        s.path(Target::Global).unwrap(),
        Path::new("/state/config/global.toml")
    );
    assert_eq!(
        s.path(NES).unwrap(),
        Path::new("/state/config/systems/nes.toml")
    );
    assert_eq!(
        s.path(SMB).unwrap(),
        Path::new("/state/config/games/nes/Super Mario Bros. (World).toml")
    );
    assert_eq!(Target::Global.level(), Level::Global);
    assert_eq!(NES.level(), Level::System);
    assert_eq!(SMB.level(), Level::Game);
}

#[test]
fn names_that_leave_their_folder_are_rejected() {
    let s = Storage::new("/state/config");
    for bad in ["", ".", "..", ".hidden", "a/b", "a\\b", "a\0b", "../../etc"] {
        for target in [
            Target::System { system: bad },
            Target::Game {
                system: bad,
                game: "ok",
            },
            Target::Game {
                system: "nes",
                game: bad,
            },
        ] {
            let err = s.path(target).unwrap_err();
            assert!(
                matches!(&err, StorageError::BadName { name, .. } if name == bad),
                "{bad:?}"
            );
            assert!(s.load(&Schema::builtin(), target).is_err());
        }
    }
}

#[test]
fn missing_files_load_as_empty_with_no_issue() {
    let dir = TempDir::create("missing");
    let s = storage(&dir);
    let schema = Schema::builtin();
    for target in [Target::Global, NES, SMB] {
        let loaded = s.load(&schema, target).unwrap();
        assert!(loaded.parsed.layer.is_empty());
        assert_eq!(loaded.parsed.layer.level(), target.level());
        assert_eq!(loaded.parsed.issues, []);
        assert_eq!(loaded.path, s.path(target).unwrap());
    }
}

#[test]
fn apply_makes_the_folders_and_the_values_load_back() {
    let dir = TempDir::create("round-trip");
    let s = storage(&dir);
    let schema = Schema::builtin();
    s.apply(
        &schema,
        Target::Global,
        &[set(DISPLAY_OUTPUT, "VGA-1"), set(DISPLAY_STYLE, "2x")],
    )
    .unwrap();
    s.apply(&schema, NES, &[set(DISPLAY_STYLE, "native")])
        .unwrap();
    s.apply(&schema, SMB, &[set(DISPLAY_STYLE, "3x")]).unwrap();

    let global = s.load(&schema, Target::Global).unwrap().parsed;
    let system = s.load(&schema, NES).unwrap().parsed;
    let game = s.load(&schema, SMB).unwrap().parsed;
    for parsed in [&global, &system, &game] {
        assert_eq!(parsed.issues, []);
        assert_eq!(parsed.version, Some(neptron_settings::SCHEMA_VERSION));
    }
    let layers =
        Layers::try_new(Some(&global.layer), Some(&system.layer), Some(&game.layer)).unwrap();
    let style = resolve_key(&schema, &layers, DISPLAY_STYLE).unwrap();
    assert_eq!((style.value, style.source), (text("3x"), Source::Game));
    let output = resolve_key(&schema, &layers, DISPLAY_OUTPUT).unwrap();
    assert_eq!(
        (output.value, output.source),
        (text("VGA-1"), Source::Global)
    );
    let monitor = resolve_key(&schema, &layers, DISPLAY_MONITOR).unwrap();
    assert_eq!(
        (monitor.value, monitor.source),
        (text("auto"), Source::Default)
    );

    s.apply(&schema, SMB, &[Change::Unset { key: DISPLAY_STYLE }])
        .unwrap();
    let game = s.load(&schema, SMB).unwrap().parsed;
    let layers =
        Layers::try_new(Some(&global.layer), Some(&system.layer), Some(&game.layer)).unwrap();
    let style = resolve_key(&schema, &layers, DISPLAY_STYLE).unwrap();
    assert_eq!(
        (style.value, style.source),
        (text("native"), Source::System)
    );
}

#[test]
fn a_write_leaves_no_temporary_file() {
    let dir = TempDir::create("no-temp");
    let s = storage(&dir);
    s.apply(
        &Schema::builtin(),
        Target::Global,
        &[set(DISPLAY_STYLE, "3x")],
    )
    .unwrap();
    assert_eq!(files_in(s.root()), ["global.toml"]);
}

#[test]
fn a_hand_edit_made_after_the_load_is_kept() {
    let dir = TempDir::create("hand-edit");
    let s = storage(&dir);
    let schema = Schema::builtin();
    s.apply(&schema, Target::Global, &[set(DISPLAY_STYLE, "2x")])
        .unwrap();
    let path = s.path(Target::Global).unwrap();
    let mut by_hand = fs::read_to_string(&path).unwrap();
    by_hand.push_str("# my monitor\nmonitor = \"generic_15\"\n[network]\nwifi_ssid = \"home\"\n");
    fs::write(&path, &by_hand).unwrap();

    s.apply(&schema, Target::Global, &[set(DISPLAY_STYLE, "3x")])
        .unwrap();
    let after = fs::read_to_string(&path).unwrap();
    assert_eq!(after, by_hand.replace("\"2x\"", "\"3x\""));
    let loaded = s.load(&schema, Target::Global).unwrap().parsed;
    assert_eq!(loaded.layer.get(DISPLAY_MONITOR), Some(&text("generic_15")));
    assert_eq!(loaded.issues.len(), 1);
    assert_eq!(loaded.issues[0].kind, IssueKind::UnknownKey);
    assert_eq!(loaded.issues[0].key.as_deref(), Some("network.wifi_ssid"));
}

#[test]
fn a_file_that_does_not_parse_is_reported_and_never_overwritten() {
    let dir = TempDir::create("unparseable");
    let s = storage(&dir);
    let schema = Schema::builtin();
    let path = s.path(Target::Global).unwrap();
    fs::create_dir_all(s.root()).unwrap();
    let broken = "[display]\nstyle = \"2x\"\nmonitor = \n";
    fs::write(&path, broken).unwrap();

    let loaded = s.load(&schema, Target::Global).unwrap().parsed;
    assert!(loaded.layer.is_empty());
    assert_eq!(loaded.issues.len(), 1);
    assert_eq!(loaded.issues[0].line, Some(3));
    assert!(matches!(
        loaded.issues[0].kind,
        IssueKind::Unparseable { .. }
    ));

    let err = s
        .apply(&schema, Target::Global, &[set(DISPLAY_STYLE, "3x")])
        .unwrap_err();
    assert!(
        matches!(
            &err,
            StorageError::Edit {
                source: EditError::Unparseable { line: Some(3), .. },
                ..
            }
        ),
        "{err}"
    );
    assert_eq!(fs::read_to_string(&path).unwrap(), broken);
    assert_eq!(files_in(s.root()), ["global.toml"]);
}

#[test]
fn a_file_that_is_not_utf8_is_reported_and_never_overwritten() {
    let dir = TempDir::create("not-utf8");
    let s = storage(&dir);
    let schema = Schema::builtin();
    let path = s.path(Target::Global).unwrap();
    fs::create_dir_all(s.root()).unwrap();
    let bytes = b"[display]\nstyle = \"\xff\"\n";
    fs::write(&path, bytes).unwrap();

    let loaded = s.load(&schema, Target::Global).unwrap().parsed;
    assert!(loaded.layer.is_empty());
    assert_eq!(loaded.issues.len(), 1);
    assert_eq!(loaded.issues[0].kind, IssueKind::NotUtf8);

    let err = s
        .apply(&schema, Target::Global, &[set(DISPLAY_STYLE, "3x")])
        .unwrap_err();
    assert!(matches!(err, StorageError::NotUtf8 { .. }), "{err}");
    assert_eq!(fs::read(&path).unwrap(), bytes);
}

#[test]
fn a_file_that_cannot_be_read_is_reported_and_never_overwritten() {
    let dir = TempDir::create("unreadable");
    let s = storage(&dir);
    let schema = Schema::builtin();
    let path = s.path(Target::Global).unwrap();
    // A directory in place of the file fails the read for every user, root included.
    fs::create_dir_all(&path).unwrap();
    fs::write(path.join("inside"), "keep").unwrap();

    let loaded = s.load(&schema, Target::Global).unwrap().parsed;
    assert!(loaded.layer.is_empty());
    assert_eq!(loaded.issues.len(), 1);
    assert!(matches!(
        loaded.issues[0].kind,
        IssueKind::Unreadable { .. }
    ));

    let err = s
        .apply(&schema, Target::Global, &[set(DISPLAY_STYLE, "3x")])
        .unwrap_err();
    assert!(matches!(err, StorageError::Read { .. }), "{err}");
    assert_eq!(fs::read_to_string(path.join("inside")).unwrap(), "keep");
}

#[test]
fn a_bad_value_falls_back_and_the_file_keeps_it() {
    let dir = TempDir::create("bad-value");
    let s = storage(&dir);
    let schema = Schema::builtin();
    let path = s.path(NES).unwrap();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(
        &path,
        "[display]\nstyle = \"9x\"\nrefresh_policy = \"force_50\"\n",
    )
    .unwrap();

    let system = s.load(&schema, NES).unwrap().parsed;
    assert_eq!(system.issues.len(), 1);
    assert_eq!(system.issues[0].key.as_deref(), Some(DISPLAY_STYLE));
    let layers = Layers::try_new(None, Some(&system.layer), None).unwrap();
    let style = resolve_key(&schema, &layers, DISPLAY_STYLE).unwrap();
    assert_eq!(style.source, Source::Default);

    s.apply(&schema, NES, &[set("display.refresh_policy", "exact")])
        .unwrap();
    let after = fs::read_to_string(&path).unwrap();
    assert!(after.contains("style = \"9x\""), "{after}");
    assert!(after.contains("refresh_policy = \"exact\""), "{after}");
}

#[test]
fn a_rejected_change_leaves_the_file_unchanged() {
    let dir = TempDir::create("rejected");
    let s = storage(&dir);
    let schema = Schema::builtin();
    s.apply(&schema, SMB, &[set(DISPLAY_STYLE, "2x")]).unwrap();
    let path = s.path(SMB).unwrap();
    let before = fs::read(&path).unwrap();
    let err = s
        .apply(
            &schema,
            SMB,
            &[set(DISPLAY_STYLE, "3x"), set(DISPLAY_OUTPUT, "VGA-1")],
        )
        .unwrap_err();
    assert!(matches!(err, StorageError::Edit { .. }), "{err}");
    assert_eq!(fs::read(&path).unwrap(), before);
}

#[test]
fn a_stale_temporary_file_from_a_crash_is_ignored_and_replaced() {
    let dir = TempDir::create("stale-temp");
    let s = storage(&dir);
    let schema = Schema::builtin();
    s.apply(&schema, Target::Global, &[set(DISPLAY_STYLE, "2x")])
        .unwrap();
    let path = s.path(Target::Global).unwrap();
    let old = fs::read_to_string(&path).unwrap();
    // A crash between the temporary write and the rename leaves this file.
    fs::write(s.root().join("global.toml.tmp"), "display.style = \"half").unwrap();

    let loaded = s.load(&schema, Target::Global).unwrap().parsed;
    assert_eq!(loaded.issues, []);
    assert_eq!(fs::read_to_string(&path).unwrap(), old);

    s.apply(&schema, Target::Global, &[set(DISPLAY_STYLE, "4x")])
        .unwrap();
    assert_eq!(files_in(s.root()), ["global.toml"]);
    let loaded = s.load(&schema, Target::Global).unwrap().parsed;
    assert_eq!(loaded.layer.get(DISPLAY_STYLE), Some(&text("4x")));
}

#[test]
fn a_failed_temporary_write_keeps_the_old_file() {
    let dir = TempDir::create("temp-fails");
    let s = storage(&dir);
    let schema = Schema::builtin();
    s.apply(&schema, Target::Global, &[set(DISPLAY_STYLE, "2x")])
        .unwrap();
    let path = s.path(Target::Global).unwrap();
    let old = fs::read(&path).unwrap();
    // A directory where the temporary file goes makes its creation fail.
    fs::create_dir(s.root().join("global.toml.tmp")).unwrap();

    let err = s
        .apply(&schema, Target::Global, &[set(DISPLAY_STYLE, "4x")])
        .unwrap_err();
    assert!(
        matches!(
            err,
            StorageError::Write {
                step: WriteStep::WriteTemp,
                ..
            }
        ),
        "{err}"
    );
    assert_eq!(fs::read(&path).unwrap(), old);
}

#[test]
fn a_parent_that_is_a_file_is_never_replaced() {
    let dir = TempDir::create("parent-file");
    let s = storage(&dir);
    fs::create_dir_all(s.root()).unwrap();
    fs::write(s.root().join("games"), "not a folder").unwrap();
    let schema = Schema::builtin();

    let loaded = s.load(&schema, SMB).unwrap().parsed;
    assert!(matches!(
        loaded.issues[0].kind,
        IssueKind::Unreadable { .. }
    ));
    let err = s
        .apply(&schema, SMB, &[set(DISPLAY_STYLE, "2x")])
        .unwrap_err();
    assert!(matches!(err, StorageError::Read { .. }), "{err}");
    assert_eq!(
        fs::read_to_string(s.root().join("games")).unwrap(),
        "not a folder"
    );
}

#[test]
fn errors_name_the_path_and_the_step() {
    let dir = TempDir::create("messages");
    let s = storage(&dir);
    fs::create_dir_all(s.root().join("global.toml.tmp")).unwrap();
    let err = s
        .apply(
            &Schema::builtin(),
            Target::Global,
            &[set(DISPLAY_STYLE, "2x")],
        )
        .unwrap_err()
        .to_string();
    assert!(err.contains("global.toml"), "{err}");
    assert!(err.contains("writing the temporary file failed"), "{err}");
}

#[test]
fn a_stale_temporary_symlink_is_removed_and_never_written_through() {
    let dir = TempDir::create("temp-symlink");
    let s = storage(&dir);
    let schema = Schema::builtin();
    s.apply(&schema, Target::Global, &[set(DISPLAY_STYLE, "2x")])
        .unwrap();
    let victim = dir.path().join("victim.txt");
    fs::write(&victim, "precious").unwrap();
    symlink(&victim, s.root().join("global.toml.tmp")).unwrap();

    s.apply(&schema, Target::Global, &[set(DISPLAY_STYLE, "3x")])
        .unwrap();
    assert_eq!(fs::read_to_string(&victim).unwrap(), "precious");
    let path = s.path(Target::Global).unwrap();
    assert!(fs::symlink_metadata(&path).unwrap().file_type().is_file());
    assert_eq!(files_in(s.root()), ["global.toml"]);
    let loaded = s.load(&schema, Target::Global).unwrap().parsed;
    assert_eq!(loaded.layer.get(DISPLAY_STYLE), Some(&text("3x")));
}

#[test]
fn a_settings_file_that_is_a_symlink_becomes_a_regular_file() {
    let dir = TempDir::create("file-symlink");
    let s = storage(&dir);
    let schema = Schema::builtin();
    fs::create_dir_all(s.root()).unwrap();
    let target = dir.path().join("elsewhere.toml");
    fs::write(&target, "[display]\nmonitor = \"generic_15\"\n").unwrap();
    let path = s.path(Target::Global).unwrap();
    symlink(&target, &path).unwrap();

    let loaded = s.load(&schema, Target::Global).unwrap().parsed;
    assert_eq!(loaded.layer.get(DISPLAY_MONITOR), Some(&text("generic_15")));
    s.apply(&schema, Target::Global, &[set(DISPLAY_STYLE, "2x")])
        .unwrap();
    assert_eq!(
        fs::read_to_string(&target).unwrap(),
        "[display]\nmonitor = \"generic_15\"\n"
    );
    assert!(fs::symlink_metadata(&path).unwrap().file_type().is_file());
    let loaded = s.load(&schema, Target::Global).unwrap().parsed;
    assert_eq!(loaded.layer.get(DISPLAY_MONITOR), Some(&text("generic_15")));
    assert_eq!(loaded.layer.get(DISPLAY_STYLE), Some(&text("2x")));
}

#[test]
fn a_write_keeps_the_permission_bits_of_the_old_file() {
    let dir = TempDir::create("permissions");
    let s = storage(&dir);
    let schema = Schema::builtin();
    s.apply(&schema, Target::Global, &[set(DISPLAY_STYLE, "2x")])
        .unwrap();
    let path = s.path(Target::Global).unwrap();
    let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode & !NEW_FILE_MODE, 0, "{mode:o}");
    assert_eq!(mode & 0o600, 0o600, "{mode:o}");

    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    s.apply(&schema, Target::Global, &[set(DISPLAY_STYLE, "3x")])
        .unwrap();
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
}

#[test]
fn a_version_that_is_not_an_integer_is_reported_and_never_overwritten() {
    let dir = TempDir::create("bad-version");
    let s = storage(&dir);
    let schema = Schema::builtin();
    fs::create_dir_all(s.root()).unwrap();
    let path = s.path(Target::Global).unwrap();
    let src = "[schema_version]\nnote = \"mine\"\n";
    fs::write(&path, src).unwrap();

    let loaded = s.load(&schema, Target::Global).unwrap().parsed;
    assert!(matches!(
        loaded.issues[0].kind,
        IssueKind::BadVersion { .. }
    ));
    let err = s
        .apply(&schema, Target::Global, &[set(DISPLAY_STYLE, "2x")])
        .unwrap_err();
    assert!(
        matches!(
            err,
            StorageError::Edit {
                source: EditError::Conflict { .. },
                ..
            }
        ),
        "{err}"
    );
    assert_eq!(fs::read_to_string(&path).unwrap(), src);
}
