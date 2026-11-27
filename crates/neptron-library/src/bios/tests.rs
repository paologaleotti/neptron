use std::fs;
use std::sync::atomic::AtomicBool;

use super::*;
use crate::catalog::{Catalog, Source};
use crate::test_dir::TestDir;

const GOOD_US: &[u8] = b"test bios usa 01";
const GOOD_EU: &[u8] = b"test bios eur 01";
const SHARED: &[u8] = b"shared firmware!";

fn catalog() -> Catalog {
    let system = format!(
        r#"
format = 1
folder = "testdisc"
aliases = []
name = "Test Disc"
extensions = ["chd", "cue", "m3u"]
discs = ["chd", "cue", "m3u"]

[[bios]]
file = "fw.bin"
size = 16
crc32 = ["{fw:08x}"]

[[bios]]
file = "test-us.bin"
region = "ntsc-u"
size = 16
crc32 = ["00000000", "{us:08x}"]

[[bios]]
file = "test-eu.bin"
region = "pal"
size = 16
crc32 = ["{eu:08x}"]
"#,
        fw = crc32(SHARED),
        us = crc32(GOOD_US),
        eu = crc32(GOOD_EU),
    );
    let nes =
        "format = 1\nfolder = \"nes\"\naliases = []\nname = \"NES\"\nextensions = [\"nes\"]\n";
    let core = |id: &str, system: &str| {
        format!(
            "format = 1\nid = \"{id}\"\nname = \"X\"\nlibrary = \"x.so\"\nsystems = [\"{system}\"]\nlicence = \"MIT\"\n"
        )
    };
    let (a, b) = (core("a", "nes"), core("b", "testdisc"));
    Catalog::from_sources(
        &[
            Source {
                name: "nes",
                text: nes,
            },
            Source {
                name: "testdisc",
                text: &system,
            },
        ],
        &[
            Source {
                name: "a",
                text: &a,
            },
            Source {
                name: "b",
                text: &b,
            },
        ],
    )
    .expect("the test catalog")
}

fn put(dir: &TestDir, relative: &str, bytes: &[u8]) -> PathBuf {
    let path = dir.file(relative);
    fs::write(&path, bytes).unwrap();
    path
}

fn statuses(survey: &BiosSurvey) -> Vec<BiosStatus> {
    survey
        .entries("testdisc")
        .iter()
        .map(|e| e.status.clone())
        .collect()
}

#[test]
fn a_good_copy_is_found_by_content_under_any_name_and_in_a_subfolder() {
    let dir = TestDir::new("bios-found");
    let fw = put(&dir, "fw.bin", SHARED);
    let us = put(&dir, "psx/My USA bios.BIN", GOOD_US);
    put(&dir, "psx/readme.txt", b"not a bios");
    let survey = survey(dir.path(), &catalog(), &AtomicBool::new(false));
    assert_eq!(
        statuses(&survey),
        [
            BiosStatus::Found(fw),
            BiosStatus::Found(us),
            BiosStatus::Missing
        ]
    );
    assert_eq!(survey.systems().len(), 1, "only systems with BIOS files");
    assert_eq!(survey.systems()[0].name, "Test Disc");
    assert!(survey.entries("nes").is_empty());
}

#[test]
fn a_wrong_file_under_the_right_name_is_refused() {
    let dir = TestDir::new("bios-wrong");
    let wrong = put(&dir, "sub/TEST-EU.bin", b"wrong content 16");
    put(&dir, "fw.bin", b"short");
    let survey = survey(dir.path(), &catalog(), &AtomicBool::new(false));
    let fw = dir.path().join("fw.bin");
    assert_eq!(
        statuses(&survey),
        [
            BiosStatus::Wrong(fw),
            BiosStatus::Missing,
            BiosStatus::Wrong(wrong)
        ]
    );
}

#[test]
fn the_top_folder_comes_first_then_each_level_in_byte_order() {
    let dir = TestDir::new("bios-order");
    put(&dir, "b/x.bin", SHARED);
    put(&dir, "a/z/y.bin", SHARED);
    put(&dir, "a/w.bin", SHARED);
    let first = survey(dir.path(), &catalog(), &AtomicBool::new(false));
    assert_eq!(
        statuses(&first)[0],
        BiosStatus::Found(dir.path().join("a/w.bin"))
    );
    let top = put(&dir, "zz.bin", SHARED);
    let second = survey(dir.path(), &catalog(), &AtomicBool::new(false));
    assert_eq!(statuses(&second)[0], BiosStatus::Found(top));
}

#[test]
fn the_walk_stops_at_its_depth_and_skips_hidden_entries_and_links() {
    let dir = TestDir::new("bios-limits");
    put(&dir, "1/2/3/4/deep.bin", SHARED);
    put(&dir, ".hidden/fw.bin", SHARED);
    put(&dir, "._fw.bin", SHARED);
    let target = put(&dir, "../outside-bios-link-target.bin", GOOD_US);
    std::os::unix::fs::symlink(&target, dir.path().join("link.bin")).unwrap();
    let survey = survey(dir.path(), &catalog(), &AtomicBool::new(false));
    assert_eq!(
        statuses(&survey),
        [
            BiosStatus::Missing,
            BiosStatus::Missing,
            BiosStatus::Missing
        ]
    );
    fs::remove_file(target).unwrap();
    let three = put(&dir, "1/2/3/ok.bin", SHARED);
    let survey = super::survey(dir.path(), &catalog(), &AtomicBool::new(false));
    assert_eq!(statuses(&survey)[0], BiosStatus::Found(three));
}

#[test]
fn a_missing_folder_finds_nothing_and_a_cancel_stops() {
    let dir = TestDir::new("bios-missing");
    let gone = dir.path().join("nothing");
    let survey = survey(&gone, &catalog(), &AtomicBool::new(false));
    assert_eq!(
        statuses(&survey),
        [
            BiosStatus::Missing,
            BiosStatus::Missing,
            BiosStatus::Missing
        ]
    );
    assert!(survey.unreadable.is_empty());
    put(&dir, "fw.bin", SHARED);
    let cancelled = super::survey(dir.path(), &catalog(), &AtomicBool::new(true));
    assert_eq!(statuses(&cancelled)[0], BiosStatus::Missing);
}

fn entry(file: &str, region: Option<Region>, found: bool) -> BiosEntry {
    BiosEntry {
        file: BiosFile {
            file: file.to_owned(),
            region,
            size: 1,
            crc32: vec![0],
        },
        status: if found {
            BiosStatus::Found(PathBuf::from(file))
        } else {
            BiosStatus::Missing
        },
    }
}

#[test]
fn a_file_with_no_region_is_needed_by_every_game() {
    let entries = [
        entry("fw.bin", None, false),
        entry("us.bin", Some(Region::NtscU), true),
    ];
    assert_eq!(
        check(&entries, Some(Region::NtscU)),
        Err(BiosProblem::Missing {
            file: "fw.bin".to_owned()
        })
    );
    assert_eq!(check(&[], None), Ok(()));
}

#[test]
fn any_region_serves_when_the_own_one_is_missing() {
    let entries = [
        entry("jp.bin", Some(Region::NtscJ), false),
        entry("us.bin", Some(Region::NtscU), false),
        entry("eu.bin", Some(Region::Pal), true),
    ];
    assert_eq!(check(&entries, Some(Region::NtscU)), Ok(()));
    assert_eq!(check(&entries, None), Ok(()));
}

#[test]
fn the_notice_names_the_file_of_the_region_or_the_first() {
    let mut entries = [
        entry("jp.bin", Some(Region::NtscJ), false),
        entry("eu.bin", Some(Region::Pal), false),
    ];
    let missing = |file: &str| {
        Err(BiosProblem::Missing {
            file: file.to_owned(),
        })
    };
    assert_eq!(check(&entries, Some(Region::Pal)), missing("eu.bin"));
    assert_eq!(check(&entries, Some(Region::NtscU)), missing("jp.bin"));
    assert_eq!(check(&entries, None), missing("jp.bin"));
    entries[1].status = BiosStatus::Wrong(PathBuf::from("x/EU.BIN"));
    assert_eq!(
        check(&entries, Some(Region::Pal)),
        Err(BiosProblem::Wrong {
            file: "eu.bin".to_owned()
        })
    );
}

#[test]
fn a_survey_of_one_system_reads_only_its_files() {
    let dir = TestDir::new("bios-one-system");
    put(&dir, "fw.bin", SHARED);
    let catalog = catalog();
    let nes = catalog.system("nes").unwrap();
    let none = survey_systems(dir.path(), &[nes], &AtomicBool::new(false));
    assert!(none.systems().is_empty(), "nes has no BIOS files");
    let disc = catalog.system("testdisc").unwrap();
    let one = survey_systems(dir.path(), &[disc], &AtomicBool::new(false));
    assert_eq!(one.systems().len(), 1);
    assert!(one.entries("testdisc")[0].is_found());
}
