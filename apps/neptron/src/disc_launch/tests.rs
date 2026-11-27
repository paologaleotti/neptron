use super::*;

use crate::test_disc::{Dir, EU, US, catalog};

fn run(dir: &Dir, content: &Path, game: &str) -> Result<DiscLaunch, Stop> {
    let catalog = catalog();
    let system = catalog.system("testdisc").unwrap().clone();
    prepare(
        &system,
        content,
        game,
        (&dir.0.join("bios"), &dir.0.join("state/system/testdisc")),
    )
}

#[test]
fn a_missing_bios_names_the_file_of_the_region_and_the_folder() {
    let dir = Dir::new("missing");
    let game = dir.put("roms/Quest (Europe).chd", b"");
    let (notice, log) = run(&dir, &game, "Quest (Europe)").unwrap_err();
    assert_eq!(
        notice,
        "Quest (Europe) did not start: the Test Disc BIOS is missing. Put td-eu.bin in the bios folder."
    );
    assert_eq!(log, ["BIOS td-us.bin: missing", "BIOS td-eu.bin: missing"]);
}

#[test]
fn a_wrong_file_under_the_right_name_is_named() {
    let dir = Dir::new("wrong");
    dir.put("bios/sub/TD-US.BIN", b"not the bios 16b");
    let game = dir.put("roms/Quest (USA).chd", b"");
    let (notice, _) = run(&dir, &game, "Quest (USA)").unwrap_err();
    assert_eq!(
        notice,
        "Quest (USA) did not start: td-us.bin is not a good Test Disc BIOS. Put a good copy of td-us.bin in the bios folder."
    );
}

#[test]
fn a_good_copy_under_any_name_goes_to_the_system_folder_of_the_core() {
    let dir = Dir::new("found");
    dir.put("bios/psx/my europe bios.bin", EU);
    dir.put("state/system/testdisc/old.bin", b"a file of the core");
    dir.put("state/system/testdisc/td-us.bin", b"an old copy");
    let game = dir.put("roms/Quest (USA).chd", b"");
    let launch = run(&dir, &game, "Quest (USA)").expect("any region serves");
    let folder = dir.0.join("state/system/testdisc");
    assert_eq!(launch.system_dir, folder);
    assert_eq!(fs::read(folder.join("td-eu.bin")).unwrap(), EU);
    assert_eq!(
        fs::read(folder.join("old.bin")).unwrap(),
        b"a file of the core",
        "a file of another name stays"
    );
    assert!(
        !folder.join("td-us.bin").exists(),
        "an old copy of a BIOS file that is gone goes"
    );
    assert_eq!(launch.numbers, [1]);
    let survey = launch.survey.expect("the survey");
    assert!(survey.entries("testdisc")[1].is_found());
    assert_eq!(
        fs::read(dir.0.join("bios/psx/my europe bios.bin")).unwrap(),
        EU,
        "the user's file is as it was"
    );
}

#[test]
fn the_disc_files_are_checked_before_the_bios() {
    let dir = Dir::new("discs");
    dir.put("bios/us.bin", US);
    let cue = dir.put("roms/Saga (Disc 1).cue", b"FILE \"s1.bin\" BINARY\n");
    dir.put("roms/Saga (Disc 2).cue", b"FILE \"s2.bin\" BINARY\n");
    dir.put("roms/s1.bin", b"");
    let (notice, _) = run(&dir, &cue, "Saga").unwrap_err();
    assert_eq!(
        notice,
        "Saga did not start: the disc file s2.bin is missing. Put it beside Saga (Disc 2).cue."
    );
    dir.put("roms/s2.bin", b"");
    let launch = run(&dir, &cue, "Saga").expect("the launch");
    assert_eq!(launch.more_discs, [dir.0.join("roms/Saga (Disc 2).cue")]);
    assert_eq!(launch.numbers, [1, 2]);
    let m3u = dir.put("roms/Empty.m3u", b"# nothing\n");
    let (notice, _) = run(&dir, &m3u, "Empty").unwrap_err();
    assert_eq!(notice, "Empty did not start: Empty.m3u names no disc file.");
}

#[test]
fn a_folder_that_cannot_be_made_stops_the_launch() {
    let dir = Dir::new("blocked");
    dir.put("bios/us.bin", US);
    dir.put("state/system", b"a file where the folder goes");
    let game = dir.put("roms/Quest (USA).chd", b"");
    let (notice, log) = run(&dir, &game, "Quest (USA)").unwrap_err();
    assert_eq!(
        notice,
        "Quest (USA) did not start: the BIOS files could not be prepared."
    );
    assert!(
        log.last()
            .unwrap()
            .starts_with("cannot write the BIOS copies")
    );
}

#[test]
fn a_system_with_no_discs_and_no_bios_keeps_the_bios_folder() {
    let dir = Dir::new("plain");
    let catalog = neptron_library::Catalog::builtin().unwrap();
    let nes = catalog.system("nes").unwrap();
    let game = dir.put("roms/Game.nes", b"");
    let launch = prepare(
        nes,
        &game,
        "Game",
        (&dir.0.join("bios"), &dir.0.join("sys")),
    )
    .unwrap();
    assert_eq!(launch.system_dir, dir.0.join("bios"));
    assert_eq!(launch.survey, None);
    assert!(launch.numbers.is_empty());
}

#[test]
fn an_equal_copy_is_not_written_again() {
    let dir = Dir::new("same-copy");
    dir.put("bios/eu.bin", EU);
    let game = dir.put("roms/Quest (Europe).chd", b"");
    run(&dir, &game, "Quest (Europe)").expect("the first launch");
    let copy = dir.0.join("state/system/testdisc/td-eu.bin");
    let first = fs::metadata(&copy).unwrap();
    // A hard link shares the inode: a write by rename would replace it.
    let witness = dir.0.join("witness");
    fs::hard_link(&copy, &witness).unwrap();
    run(&dir, &game, "Quest (Europe)").expect("the second launch");
    let again = fs::metadata(&copy).unwrap();
    use std::os::unix::fs::MetadataExt;
    assert_eq!(again.ino(), first.ino(), "no new file");
    fs::write(&copy, b"damaged copy 16b").unwrap();
    run(&dir, &game, "Quest (Europe)").expect("the third launch");
    assert_eq!(fs::read(&copy).unwrap(), EU, "a different copy is written");
}

#[test]
fn a_source_that_changed_after_the_check_is_not_copied() {
    let dir = Dir::new("changed");
    let source = dir.put("bios/eu.bin", EU);
    let catalog = catalog();
    let system = catalog.system("testdisc").unwrap();
    let survey = bios::survey_systems(
        &dir.0.join("bios"),
        &[system],
        &std::sync::atomic::AtomicBool::new(false),
    );
    fs::write(&source, b"other content 16").unwrap();
    let folder = dir.0.join("sys");
    let err = fill_system_folder(&folder, survey.entries("testdisc")).unwrap_err();
    assert!(err.to_string().contains("changed after the check"), "{err}");
    assert!(!folder.join("td-eu.bin").exists());
}
