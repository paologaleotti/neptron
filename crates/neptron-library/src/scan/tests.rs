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
        [
            "SNES/Mario World.sfc",
            "SNES/Mario World.smc",
            "SNES/Mario World.zip"
        ]
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
    // readme.txt, noextension, notes.txt in the root.
    assert_eq!(report.ignored_files, 3);
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

#[test]
fn zip_files_are_games_and_are_not_opened() {
    let dir = TestDir::new("zip");
    dir.file("nes/Contra.nes");
    // Not a valid archive: the scan must not care, because it never opens it.
    fs::write(dir.file("nes/Contra.zip"), b"not a zip").expect("write the archive");
    dir.file("nes/Bomberman.ZIP");
    dir.file("nes/Excitebike.7z");
    dir.file("snes/usa/Mario World.zip");
    let outcome = run(dir.path()).expect("the scan completes");
    let index = &outcome.index;
    assert_eq!(names(index, "nes"), ["Bomberman", "Contra", "Contra"]);
    assert_eq!(
        paths(index, "nes"),
        ["nes/Bomberman.ZIP", "nes/Contra.nes", "nes/Contra.zip"]
    );
    let zips: Vec<bool> = index
        .system("nes")
        .map_or(vec![], |s| s.games().iter().map(Game::is_zip).collect());
    assert_eq!(zips, [true, false, true]);
    assert_eq!(paths(index, "snes"), ["snes/usa/Mario World.zip"]);
    assert_eq!(outcome.report.ignored_files, 1);
}

/// The time `T0 + secs`, with `T0` an old, fixed time.
fn t(secs: u64) -> SystemTime {
    SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_600_000_000 + secs)
}

/// Sets the modification time of the folder `relative` of `dir`, so a test
/// does not depend on the time step of the file system.
fn set_mtime(dir: &TestDir, relative: &str, at: SystemTime) {
    fs::File::open(dir.path().join(relative))
        .and_then(|f| f.set_modified(at))
        .expect("set the folder time");
}

fn write(dir: &TestDir, relative: &str, bytes: &[u8]) {
    fs::write(dir.path().join(relative), bytes).expect("write the file");
}

/// The tree of the refresh tests, every folder at `t(0)`.
fn refresh_fixture() -> TestDir {
    let dir = TestDir::new("refresh");
    dir.file("nes/sub/b.nes");
    dir.dir("snes");
    write(&dir, "nes/a.nes", b"aaaa");
    write(&dir, "snes/c.sfc", b"cc");
    for folder in ["nes/sub", "nes", "snes"] {
        set_mtime(&dir, folder, t(0));
    }
    dir
}

fn refresh_at(
    root: &Path,
    previous: &Index,
    now: SystemTime,
) -> (Result<ScanOutcome, ScanError>, ScanProgress) {
    let mut last = ScanProgress::default();
    let result = refresh(
        root,
        &catalog(),
        previous,
        now,
        &AtomicBool::new(false),
        &mut |p| last = p,
    );
    (result, last)
}

fn sizes<'a>(index: &'a Index, system: &str) -> Vec<(&'a str, u64)> {
    index
        .system(system)
        .map(|s| s.games().iter().map(|g| (g.path(), g.size())).collect())
        .unwrap_or_default()
}

#[test]
fn a_scan_stamps_every_folder_and_sizes_every_game() {
    let dir = refresh_fixture();
    let (outcome, progress) = refresh_at(dir.path(), &Index::new(), t(3600));
    let index = outcome.expect("the scan").index;
    let stamped: Vec<(&str, bool)> = index
        .folders()
        .iter()
        .map(|f| (f.path.as_str(), f.mtime.is_some()))
        .collect();
    assert_eq!(stamped, [("nes", true), ("nes/sub", true), ("snes", true)]);
    assert_eq!(
        index.folder("nes").and_then(|f| f.mtime),
        Some(crate::Mtime::of(t(0)))
    );
    assert_eq!(
        sizes(&index, "nes"),
        [("nes/a.nes", 4), ("nes/sub/b.nes", 0)]
    );
    assert_eq!((progress.folders, progress.kept), (4, 0));
}

#[test]
fn a_refresh_reads_only_the_folders_that_changed() {
    let dir = refresh_fixture();
    let first = refresh_at(dir.path(), &Index::new(), t(3600))
        .0
        .expect("scan");
    write(&dir, "nes/d.nes", b"ddd");
    set_mtime(&dir, "nes", t(10));
    let (outcome, progress) = refresh_at(dir.path(), &first.index, t(7200));
    let index = outcome.expect("refresh").index;
    assert_eq!(
        sizes(&index, "nes"),
        [("nes/a.nes", 4), ("nes/sub/b.nes", 0), ("nes/d.nes", 3)]
    );
    assert_eq!(sizes(&index, "snes"), [("snes/c.sfc", 2)]);
    // The root and `nes` are read; `nes/sub` and `snes` are kept.
    assert_eq!((progress.folders, progress.kept, progress.games), (2, 2, 4));
    let again = refresh_at(dir.path(), &index, t(7300)).0.expect("refresh");
    assert_eq!(again.index, index);
}

#[test]
fn an_entry_in_a_subfolder_is_found_under_a_kept_folder() {
    let dir = refresh_fixture();
    let first = refresh_at(dir.path(), &Index::new(), t(3600))
        .0
        .expect("scan");
    dir.file("nes/sub/e.nes");
    set_mtime(&dir, "nes/sub", t(10));
    set_mtime(&dir, "nes", t(0));
    let (outcome, progress) = refresh_at(dir.path(), &first.index, t(7200));
    assert_eq!(
        names(&outcome.expect("refresh").index, "nes"),
        ["a", "b", "e"]
    );
    assert_eq!((progress.folders, progress.kept), (2, 2));
}

#[test]
fn a_new_folder_is_read_with_its_subfolders() {
    let dir = refresh_fixture();
    let first = refresh_at(dir.path(), &Index::new(), t(3600))
        .0
        .expect("scan");
    dir.file("nes/new/deep/f.nes");
    dir.file("megadrive/g.md");
    set_mtime(&dir, "nes", t(10));
    let index = refresh_at(dir.path(), &first.index, t(7200))
        .0
        .expect("refresh")
        .index;
    assert_eq!(names(&index, "nes"), ["a", "b", "f"]);
    assert_eq!(names(&index, "megadrive"), ["g"]);
    assert!(index.folder("nes/new/deep").is_some());
}

#[test]
fn a_time_inside_the_trust_window_is_read_again() {
    let dir = refresh_fixture();
    let first = refresh_at(dir.path(), &Index::new(), t(1)).0.expect("scan");
    assert!(first.index.folders().iter().all(|f| f.mtime.is_none()));
    // An entry made in the same time step as the scan: the time stays.
    dir.file("nes/h.nes");
    set_mtime(&dir, "nes", t(0));
    let (outcome, progress) = refresh_at(dir.path(), &first.index, t(3600));
    assert_eq!(
        names(&outcome.expect("refresh").index, "nes"),
        ["a", "b", "h"]
    );
    assert_eq!((progress.folders, progress.kept), (4, 0));
}

#[test]
fn removed_files_and_folders_lose_their_games() {
    let dir = refresh_fixture();
    let first = refresh_at(dir.path(), &Index::new(), t(3600))
        .0
        .expect("scan");
    fs::remove_file(dir.path().join("nes/a.nes")).expect("remove");
    set_mtime(&dir, "nes", t(10));
    fs::remove_dir_all(dir.path().join("snes")).expect("remove the system folder");
    let index = refresh_at(dir.path(), &first.index, t(7200))
        .0
        .expect("refresh")
        .index;
    assert_eq!(names(&index, "nes"), ["b"]);
    assert!(index.system("snes").is_none());
    assert!(index.folder("snes").is_none());
}

#[test]
fn a_folder_gone_under_a_kept_folder_loses_its_games() {
    // A RAM folder or a stick pulled and put back can lose a folder while
    // the time of the folder above stays.
    let dir = refresh_fixture();
    let first = refresh_at(dir.path(), &Index::new(), t(3600))
        .0
        .expect("scan");
    fs::remove_dir_all(dir.path().join("nes/sub")).expect("remove");
    set_mtime(&dir, "nes", t(0));
    let (outcome, progress) = refresh_at(dir.path(), &first.index, t(7200));
    let index = outcome.expect("refresh").index;
    assert_eq!(names(&index, "nes"), ["a"]);
    assert!(index.folder("nes/sub").is_none());
    assert_eq!((progress.folders, progress.kept), (1, 2));
}

#[test]
fn a_refresh_of_a_missing_root_is_an_error() {
    let dir = refresh_fixture();
    let first = refresh_at(dir.path(), &Index::new(), t(3600))
        .0
        .expect("scan");
    let (outcome, _) = refresh_at(&dir.path().join("absent"), &first.index, t(7200));
    assert!(matches!(outcome, Err(ScanError::Root { .. })));
}

#[test]
fn cancel_during_a_refresh_stops_it() {
    let dir = refresh_fixture();
    let first = refresh_at(dir.path(), &Index::new(), t(3600))
        .0
        .expect("scan");
    let cancel = AtomicBool::new(false);
    let mut reports = 0;
    let result = refresh(
        dir.path(),
        &catalog(),
        &first.index,
        t(7200),
        &cancel,
        &mut |_| {
            reports += 1;
            cancel.store(true, Ordering::Relaxed);
        },
    );
    assert!(matches!(result, Err(ScanError::Cancelled)));
    assert_eq!(reports, 1, "the refresh stops after the root");
}

#[test]
fn a_kept_folder_keeps_the_old_size_and_a_scan_finds_the_new_one() {
    // A network copy grows a file after the scan listed it. The time of
    // its folder does not change, so only a scan of every folder sees it.
    let dir = refresh_fixture();
    let first = refresh_at(dir.path(), &Index::new(), t(3600))
        .0
        .expect("scan");
    write(&dir, "nes/a.nes", b"aaaaaaaa");
    set_mtime(&dir, "nes", t(0));
    let kept = refresh_at(dir.path(), &first.index, t(7200))
        .0
        .expect("refresh");
    assert_eq!(sizes(&kept.index, "nes")[0], ("nes/a.nes", 4));
    let full = refresh_at(dir.path(), &Index::new(), t(7200))
        .0
        .expect("scan");
    assert_eq!(sizes(&full.index, "nes")[0], ("nes/a.nes", 8));
}

#[test]
fn a_partial_zip_is_listed_and_not_opened() {
    let dir = refresh_fixture();
    write(&dir, "nes/partial.zip", b"PK\x03\x04 cut here");
    let index = refresh_at(dir.path(), &Index::new(), t(3600))
        .0
        .expect("scan")
        .index;
    let game = index
        .system("nes")
        .and_then(|s| s.games().iter().find(|g| g.is_zip()))
        .expect("the zip is listed");
    assert_eq!(game.size(), 13);
}
