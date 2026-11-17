//! Every set, ROM, size and CRC here is invented for the tests.

use super::*;
use crate::arcade::{IndexSource, SetRecord};

fn rom(name: &str, size: u64, crc: Option<u32>) -> RomRecord {
    RomRecord {
        name: name.to_owned(),
        size,
        crc,
        merge: None,
    }
}

/// A BIOS with two ROMs, a parent that uses it, a clone with one ROM of
/// its own and one of the parent, and a set with no BIOS.
fn index() -> ArcadeIndex {
    let mut bios = SetRecord::new("zbios".to_owned(), "Zed BIOS".to_owned());
    bios.flags.bios = true;
    bios.roms = vec![
        rom("z1.bin", 128, Some(0xb1)),
        rom("z2.bin", 128, Some(0xb2)),
    ];
    let mut parent = SetRecord::new("starpat".to_owned(), "Star Patrol".to_owned());
    parent.romof = Some("zbios".to_owned());
    parent.roms = vec![
        rom("sp1.bin", 1024, Some(0x11)),
        rom("sp2.bin", 2048, Some(0x12)),
        rom("sp3.bin", 512, Some(0x13)),
        rom("sp4.bin", 512, Some(0x14)),
        rom("dead.bin", 64, None),
        rom("z1.bin", 128, Some(0xb1)),
    ];
    let mut clone = SetRecord::new("starpatj".to_owned(), "Star Patrol".to_owned());
    clone.parent = Some("starpat".to_owned());
    clone.romof = Some("starpat".to_owned());
    clone.roms = vec![
        rom("sp1.bin", 1024, Some(0x11)),
        rom("spj2.bin", 2048, Some(0x22)),
    ];
    let mut alone = SetRecord::new("alone".to_owned(), "Alone".to_owned());
    alone.roms = vec![rom("a.bin", 16, Some(0xa1))];
    let source = IndexSource {
        commit: "0123456789abcdef0123456789abcdef01234567".to_owned(),
        dat_version: "9.9.9".to_owned(),
        ..IndexSource::default()
    };
    ArcadeIndex::from_sets(source, vec![bios, parent, clone, alone]).expect("valid sets")
}

fn zip(file: &str, entries: &[(&str, u64, u32)]) -> FoundZip {
    FoundZip {
        file: file.to_owned(),
        entries: Ok(entries
            .iter()
            .map(|(name, size, crc)| ZipEntry {
                name: (*name).to_owned(),
                size: *size,
                crc: *crc,
            })
            .collect()),
    }
}

fn bios_zip() -> FoundZip {
    zip("zbios.zip", &[("z1.bin", 128, 0xb1), ("z2.bin", 128, 0xb2)])
}

fn parent_zip() -> FoundZip {
    zip(
        "starpat.zip",
        &[
            ("sp1.bin", 1024, 0x11),
            ("sp2.bin", 2048, 0x12),
            ("sp3.bin", 512, 0x13),
            ("sp4.bin", 512, 0x14),
        ],
    )
}

#[test]
fn the_wanted_names_follow_the_parent_and_the_romof_chain() {
    let index = index();
    let w = wanted(&index, "StarPatJ").expect("a set");
    assert_eq!(w.set, "starpatj");
    assert_eq!(w.parent, Some("starpat"));
    assert_eq!(w.bios, Some("zbios"));
    let w = wanted(&index, "alone").expect("a set");
    assert_eq!((w.parent, w.bios), (None, None));
    assert!(wanted(&index, "nothing").is_none());
}

#[test]
fn a_complete_split_set_passes() {
    let found = FoundZips {
        set: vec![zip("starpatj.zip", &[("spj2.bin", 2048, 0x22)])],
        parent: vec![parent_zip()],
        bios: vec![bios_zip()],
    };
    assert_eq!(
        check_set(&index(), "starpatj", &found),
        Verdict::Pass {
            bios_differences: Vec::new()
        }
    );
}

#[test]
fn a_non_merged_clone_needs_no_parent_zip() {
    let found = FoundZips {
        set: vec![zip(
            "starpatj.zip",
            &[("other-name.bin", 1024, 0x11), ("spj2.bin", 2048, 0x22)],
        )],
        bios: vec![bios_zip()],
        ..FoundZips::default()
    };
    assert!(matches!(
        check_set(&index(), "starpatj", &found),
        Verdict::Pass { .. }
    ));
}

#[test]
fn an_unknown_set_gives_message_1() {
    let verdict = check_set(&index(), "mystery", &FoundZips::default());
    assert_eq!(
        verdict.message("Core X").as_deref(),
        Some("mystery.zip is not a set name of Core X. Check the file name.")
    );
    assert!(verdict.stops());
}

#[test]
fn a_missing_bios_zip_gives_message_2() {
    let found = FoundZips {
        set: vec![parent_zip()],
        ..FoundZips::default()
    };
    let verdict = check_set(&index(), "starpat", &found);
    assert_eq!(
        verdict,
        Verdict::MissingBios {
            bios: "zbios".to_owned()
        }
    );
    assert_eq!(
        verdict.message("Core X").as_deref(),
        Some(
            "This game needs the BIOS file zbios.zip. Put it in bios, or in the folder of the game."
        )
    );
}

#[test]
fn a_clone_that_needs_its_parent_gives_message_3() {
    let found = FoundZips {
        set: vec![zip("starpatj.zip", &[("spj2.bin", 2048, 0x22)])],
        bios: vec![bios_zip()],
        ..FoundZips::default()
    };
    let verdict = check_set(&index(), "starpatj", &found);
    assert_eq!(
        verdict.message("Core X").as_deref(),
        Some("This game needs the file starpat.zip in the same folder.")
    );
}

const WANTED: &str = " This image needs sets for Core X 9.9.9 (commit 0123456).";

#[test]
fn missing_and_wrong_size_roms_give_message_4_with_their_names() {
    let found = FoundZips {
        set: vec![zip(
            "starpat.zip",
            &[
                ("sp1.bin", 1000, 0x11),
                ("sp3.bin", 512, 0x13),
                ("x", 512, 0x14),
            ],
        )],
        bios: vec![bios_zip()],
        ..FoundZips::default()
    };
    let verdict = check_set(&index(), "starpat", &found);
    assert_eq!(
        verdict,
        Verdict::Roms {
            problems: vec![
                RomProblem {
                    rom: "sp1.bin".to_owned(),
                    problem: Problem::Size {
                        expected: 1024,
                        found: 1000
                    },
                },
                RomProblem {
                    rom: "sp2.bin".to_owned(),
                    problem: Problem::Missing,
                },
            ],
            checked: 4,
            wanted: "9.9.9 (commit 0123456)".to_owned(),
        }
    );
    assert!(!verdict.wrong_set(), "2 of 4 is not more than half");
    assert_eq!(
        verdict.message("Core X"),
        Some(format!(
            "2 files of this game are missing or differ: sp1.bin and sp2.bin.{WANTED} The log lists every file."
        ))
    );
    assert_eq!(
        verdict.details(),
        ["sp1.bin: 1000 bytes, expected 1024", "sp2.bin: missing"]
    );
}

#[test]
fn message_4_names_two_files_and_counts_the_rest() {
    let problem = |rom: &str| RomProblem {
        rom: rom.to_owned(),
        problem: Problem::Missing,
    };
    let roms = |names: &[&str]| Verdict::Roms {
        problems: names.iter().map(|n| problem(n)).collect(),
        checked: 100,
        wanted: String::new(),
    };
    assert_eq!(
        roms(&["gg1.bin"]).message("C").as_deref(),
        Some("1 file of this game is missing or differs: gg1.bin. The log lists every file.")
    );
    assert_eq!(
        roms(&["gg1.bin", "gg2.bin", "gg3.bin", "gg4.bin"])
            .message("C")
            .as_deref(),
        Some(
            "4 files of this game are missing or differ: gg1.bin, gg2.bin and 2 more. The log lists every file."
        )
    );
    let long = "a".repeat(NAMED_ROM_CHARS_MAX + 10);
    let message = roms(&[&long]).message("C").expect("a message");
    assert!(message.contains(&"a".repeat(NAMED_ROM_CHARS_MAX)));
    assert!(!message.contains(&"a".repeat(NAMED_ROM_CHARS_MAX + 1)));
}

#[test]
fn a_set_with_most_files_wrong_is_for_another_emulator() {
    let found = FoundZips {
        set: vec![zip("starpat.zip", &[("sp1.bin", 1024, 0x11)])],
        bios: vec![bios_zip()],
        ..FoundZips::default()
    };
    let verdict = check_set(&index(), "starpat", &found);
    assert!(verdict.wrong_set(), "3 of 4 is more than half");
    assert!(verdict.stops());
    assert_eq!(
        verdict.message("Core X"),
        Some(format!(
            "Most files of this game do not match. The set is for another emulator or another version.{WANTED}"
        ))
    );
    assert_eq!(verdict.details().len(), 3, "the log still lists each file");
}

#[test]
fn the_wanted_version_names_the_dat_and_the_commit() {
    let source = |version: &str, commit: &str| IndexSource {
        dat_version: version.to_owned(),
        commit: commit.to_owned(),
        ..IndexSource::default()
    };
    assert_eq!(
        wanted_version(&source("1.0", "abcdef0123")),
        "1.0 (commit abcdef0)"
    );
    assert_eq!(wanted_version(&source("1.0", "")), "1.0");
    assert_eq!(wanted_version(&source("", "abc")), "commit abc");
    assert_eq!(wanted_version(&source("", "")), "");
}

#[test]
fn the_first_zip_with_the_crc_gives_the_rom() {
    // The first zip holds the CRC with a wrong size; a later one is right.
    // The core takes the first, so the check does too.
    let found = FoundZips {
        set: vec![zip("starpat.zip", &[("sp1.bin", 1, 0x11)]), parent_zip()],
        bios: vec![bios_zip()],
        ..FoundZips::default()
    };
    assert!(matches!(
        check_set(&index(), "starpat", &found),
        Verdict::Roms { problems, .. } if problems.len() == 1
    ));
}

#[test]
fn a_bios_that_holds_nothing_gives_message_5() {
    let found = FoundZips {
        set: vec![parent_zip()],
        bios: vec![zip("zbios.zip", &[("z1.bin", 128, 0xff)])],
        ..FoundZips::default()
    };
    let verdict = check_set(&index(), "starpat", &found);
    assert_eq!(
        verdict.message("Core X").as_deref(),
        Some(format!(
            "The BIOS file zbios.zip does not match this version of Core X.{WANTED}"
        ))
        .as_deref()
    );
}

#[test]
fn a_bios_that_holds_some_files_passes_and_lists_the_rest() {
    let found = FoundZips {
        set: vec![parent_zip()],
        bios: vec![zip("zbios.zip", &[("z1.bin", 128, 0xb1)])],
        ..FoundZips::default()
    };
    let verdict = check_set(&index(), "starpat", &found);
    assert!(!verdict.stops());
    assert_eq!(verdict.message("Core X"), None);
    assert_eq!(verdict.details(), ["z2.bin: missing"]);
}

#[test]
fn a_bios_rom_listed_in_the_game_set_is_not_asked_twice() {
    // starpat lists z1.bin with the CRC of the BIOS; the BIOS zip lacks
    // it, so only the BIOS line names it.
    let found = FoundZips {
        set: vec![parent_zip()],
        bios: vec![zip("zbios.zip", &[("z2.bin", 128, 0xb2)])],
        ..FoundZips::default()
    };
    let verdict = check_set(&index(), "starpat", &found);
    assert!(matches!(verdict, Verdict::Pass { .. }), "{verdict:?}");
    assert_eq!(verdict.details(), ["z1.bin: missing"]);
}

#[test]
fn an_unreadable_zip_gives_message_6() {
    let found = FoundZips {
        set: vec![parent_zip()],
        bios: vec![
            bios_zip(),
            FoundZip {
                file: "zbios.zip".to_owned(),
                entries: Err("the archive is ZIP64".to_owned()),
            },
        ],
        ..FoundZips::default()
    };
    let verdict = check_set(&index(), "starpat", &found);
    assert_eq!(
        verdict.message("Core X").as_deref(),
        Some("The file zbios.zip cannot be read: the archive is ZIP64.")
    );
}

#[test]
fn no_index_gives_message_7_and_lets_the_player_choose() {
    let verdict = Verdict::NoIndex;
    assert!(!verdict.stops());
    assert_eq!(
        verdict.message("Core X").as_deref(),
        Some("NEPTRON cannot find its list of arcade games, so it cannot check this game.")
    );
}

#[test]
fn a_set_with_no_bios_checks_its_own_zip_only() {
    let found = FoundZips {
        set: vec![zip("alone.zip", &[("a.bin", 16, 0xa1)])],
        ..FoundZips::default()
    };
    assert!(matches!(
        check_set(&index(), "ALONE", &found),
        Verdict::Pass { .. }
    ));
    let empty = FoundZips {
        set: vec![zip("alone.zip", &[])],
        ..FoundZips::default()
    };
    assert!(matches!(
        check_set(&index(), "alone", &empty),
        Verdict::Roms { .. }
    ));
}
