use std::time::SystemTime;

use super::*;
use crate::test_dir::TestDir;

fn catalog() -> Catalog {
    Catalog::builtin().expect("the built-in definitions are valid")
}

fn run(root: &Path) -> Result<ScanOutcome, ScanError> {
    scan(root, &catalog(), &AtomicBool::new(false), &mut |_| {})
}

fn names<'a>(index: &'a Index, system: &str) -> Vec<&'a str> {
    index
        .system(system)
        .map(|s| s.games().iter().map(Game::name).collect())
        .unwrap_or_default()
}

fn paths<'a>(index: &'a Index, system: &str) -> Vec<&'a str> {
    index
        .system(system)
        .map(|s| s.games().iter().map(Game::path).collect())
        .unwrap_or_default()
}

/// A tree in the Batocera and RetroPie layout, with the cases that the scan
/// must skip or report.
fn layout_fixture() -> TestDir {
    let dir = TestDir::new("layout");
    for file in [
        "nes/Super Mario Bros. (USA).nes",
        "nes/zelda.NES",
        "nes/Contra.nes",
        "nes/readme.txt",
        "nes/noextension",
        "nes/.nes",
        "nes/.hidden.nes",
        "nes/._zelda.NES",
        "nes/usa/Duck Hunt.nes",
        "nes/.git/objects.nes",
        "SNES/Mario World.sfc",
        "SNES/Mario World.smc",
        "SNES/Mario World.zip",
        "megadrive/Sonic.md",
        "genesis/Sonic.md",
        "genesis/Streets of Rage.gen",
        "psx/game.bin",
        "notes.txt",
        ".Trashes/x.nes",
    ] {
        dir.file(file);
    }
    dir.dir("snes9x-empty");
    dir
}

#[test]
fn layout_fixture_gives_the_expected_index() {
    let dir = layout_fixture();
    let outcome = run(dir.path()).expect("the scan completes");
    let index = &outcome.index;

    let systems: Vec<&str> = index.systems().iter().map(SystemGames::system).collect();
    assert_eq!(systems, ["nes", "snes", "megadrive"]);
    assert_eq!(
        names(index, "nes"),
        ["Contra", "Duck Hunt", "Super Mario Bros. (USA)", "zelda"]
    );
    assert_eq!(paths(index, "nes")[1], "nes/usa/Duck Hunt.nes");
    assert_eq!(
        paths(index, "snes"),
        ["SNES/Mario World.sfc", "SNES/Mario World.smc"]
    );
    assert_eq!(
        paths(index, "megadrive"),
        [
            "genesis/Sonic.md",
            "megadrive/Sonic.md",
            "genesis/Streets of Rage.gen"
        ]
    );

    let report = &outcome.report;
    assert_eq!(report.unknown_folders, ["psx", "snes9x-empty"]);
    assert_eq!(report.more_unknown_folders, 0);
    // readme.txt, noextension, .zip, notes.txt in the root.
    assert_eq!(report.ignored_files, 4);
    // .nes, .hidden.nes, ._zelda.NES, .git, .Trashes.
    assert_eq!(report.hidden_entries, 5);
    assert_eq!(report.symlinks, 0);
    assert!(report.unreadable.is_empty());
}

#[test]
fn scan_changes_no_file() {
    let dir = layout_fixture();
    let before = tree_state(dir.path());
    run(dir.path()).expect("the scan completes");
    assert_eq!(tree_state(dir.path()), before);
}

fn tree_state(root: &Path) -> Vec<(PathBuf, u64, Option<SystemTime>)> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).expect("read the fixture") {
            let entry = entry.expect("read the fixture entry");
            let meta = fs::symlink_metadata(entry.path()).expect("read the metadata");
            if meta.is_dir() {
                stack.push(entry.path());
            }
            out.push((entry.path(), meta.len(), meta.modified().ok()));
        }
    }
    out.sort();
    out
}

#[test]
fn empty_root_gives_an_empty_index() {
    let dir = TestDir::new("empty");
    let outcome = run(dir.path()).expect("the scan completes");
    assert_eq!(outcome.index.total(), 0);
    assert_eq!(outcome.report, ScanReport::default());
}

#[test]
fn missing_root_is_an_error() {
    let dir = TestDir::new("missing");
    let root = dir.path().join("roms");
    match run(&root) {
        Err(ScanError::Root { path, source }) => {
            assert_eq!(path, root);
            assert_eq!(source.kind(), io::ErrorKind::NotFound);
        }
        other => panic!("expected a root error, got {other:?}"),
    }
}

#[test]
fn folders_deeper_than_the_limit_are_skipped() {
    let dir = TestDir::new("deep");
    let mut relative = String::from("nes");
    for level in 1..=MAX_FOLDER_DEPTH + 1 {
        relative.push_str(&format!("/d{level}"));
        dir.file(&format!("{relative}/level {level}.nes"));
    }
    let outcome = run(dir.path()).expect("the scan completes");
    assert_eq!(outcome.index.count("nes"), MAX_FOLDER_DEPTH);
    assert_eq!(outcome.report.too_deep, 1);
}

#[test]
fn cancel_before_the_start_stops_the_scan() {
    let dir = layout_fixture();
    let cancel = AtomicBool::new(true);
    let result = scan(dir.path(), &catalog(), &cancel, &mut |_| {});
    assert!(matches!(result, Err(ScanError::Cancelled)));
}

#[test]
fn cancel_between_folders_stops_the_scan() {
    let dir = layout_fixture();
    let cancel = AtomicBool::new(false);
    let mut calls = 0;
    let result = scan(dir.path(), &catalog(), &cancel, &mut |_| {
        calls += 1;
        cancel.store(true, Ordering::Relaxed);
    });
    assert!(matches!(result, Err(ScanError::Cancelled)));
    assert_eq!(
        calls, 1,
        "the scan stops in the first folder after the cancel"
    );
}

#[test]
fn progress_grows_and_ends_at_the_totals() {
    let dir = layout_fixture();
    let mut seen = Vec::new();
    let outcome = scan(dir.path(), &catalog(), &AtomicBool::new(false), &mut |p| {
        seen.push(p);
    })
    .expect("the scan completes");
    // The root, nes, nes/usa, SNES, megadrive, genesis.
    assert_eq!(seen.len(), 6);
    assert!(
        seen.windows(2)
            .all(|w| w[0].folders < w[1].folders && w[0].games <= w[1].games)
    );
    let last = seen.last().copied().unwrap_or_default();
    assert_eq!(last.folders, 6);
    assert_eq!(last.games as usize, outcome.index.total());
}

#[test]
fn ensure_system_folders_makes_only_missing_folders() {
    let dir = TestDir::new("ensure");
    let roms = dir.path().join("share/roms");
    let rom = dir.file("share/roms/nes/Contra.nes");
    fs::write(&rom, b"ROM").expect("write the ROM");

    let made = ensure_system_folders(&roms, &catalog()).expect("the folders are made");
    assert_eq!(made, [roms.join("snes"), roms.join("megadrive")]);
    assert_eq!(fs::read(&rom).expect("read the ROM"), b"ROM");

    let made = ensure_system_folders(&roms, &catalog()).expect("the folders exist");
    assert!(made.is_empty());
}

#[test]
fn ensure_system_folders_reports_a_file_in_the_way() {
    let dir = TestDir::new("ensure-file");
    dir.file("roms");
    let err = ensure_system_folders(&dir.path().join("roms"), &catalog()).unwrap_err();
    assert_ne!(err.kind(), io::ErrorKind::NotFound);
}

#[cfg(unix)]
#[test]
fn symbolic_links_are_not_followed() {
    use std::os::unix::fs::symlink;

    let dir = TestDir::new("links");
    dir.file("megadrive/Sonic.md");
    dir.file("outside/Contra.nes");
    symlink(dir.path().join("megadrive"), dir.path().join("genesis")).expect("link a folder");
    dir.dir("nes");
    symlink(
        dir.path().join("outside/Contra.nes"),
        dir.path().join("nes/Contra.nes"),
    )
    .expect("link a file");
    symlink(dir.path().join("nes"), dir.path().join("nes/loop")).expect("link a loop");

    let outcome = run(dir.path()).expect("the scan completes");
    assert_eq!(paths(&outcome.index, "megadrive"), ["megadrive/Sonic.md"]);
    assert_eq!(outcome.index.count("nes"), 0);
    assert_eq!(outcome.report.symlinks, 3);
    assert_eq!(outcome.report.unknown_folders, ["outside"]);
}

#[cfg(unix)]
#[test]
fn unreadable_folder_is_reported_and_the_scan_goes_on() {
    use std::os::unix::fs::PermissionsExt;

    let dir = TestDir::new("unreadable");
    dir.file("snes/Mario.sfc");
    let locked = dir.dir("nes/locked");
    dir.file("nes/locked/Contra.nes");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).expect("lock the folder");
    // The superuser reads a folder with no permissions, so the error
    // cannot happen there.
    let can_fail = fs::read_dir(&locked).is_err();

    let outcome = run(dir.path());
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).expect("unlock the folder");
    let outcome = outcome.expect("the scan completes");

    assert_eq!(outcome.index.count("snes"), 1);
    if can_fail {
        assert_eq!(outcome.report.unreadable.len(), 1);
        assert_eq!(outcome.report.unreadable[0].path, locked);
        assert_eq!(outcome.index.count("nes"), 0);
    } else {
        assert_eq!(outcome.index.count("nes"), 1);
    }
}

#[cfg(target_os = "linux")]
#[test]
fn non_utf8_names_are_skipped() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    let dir = TestDir::new("utf8");
    let nes = dir.dir("nes");
    fs::write(nes.join(OsStr::from_bytes(b"bad\xff.nes")), b"").expect("write the file");
    dir.file("nes/good.nes");
    let outcome = run(dir.path()).expect("the scan completes");
    assert_eq!(names(&outcome.index, "nes"), ["good"]);
    assert_eq!(outcome.report.bad_names, 1);
}

#[test]
fn report_lists_are_bounded() {
    let mut list = Vec::new();
    let mut overflow = 0;
    for i in 0..MAX_REPORTED_ENTRIES + 3 {
        push_bounded(&mut list, i, &mut overflow);
    }
    assert_eq!(list.len(), MAX_REPORTED_ENTRIES);
    assert_eq!(overflow, 3);
}

#[test]
fn counters_saturate() {
    let mut n = u32::MAX - 1;
    bump(&mut n);
    bump(&mut n);
    assert_eq!(n, u32::MAX);
}
