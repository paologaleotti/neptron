//! The arcade games of the list: the titles of the arcade index, and the
//! set check before a launch with its messages. Every set, ROM, size and
//! CRC here is invented; the zips hold a central directory and no data.

use std::path::{Path, PathBuf};

use neptron::{NextRun, Outcome};
use neptron_library::arcade::{self, ArcadeIndex, IndexSource, RomRecord, SetRecord};
use neptron_types::Control;

use crate::rig::Rig;

/// A new, empty folder for one test.
fn fresh(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("neptron-app-{label}-{}", std::process::id()));
    // A leftover of an earlier run with the same process id; absence is fine.
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("the folder");
    dir
}

/// A zip with a central directory of `entries` (name, size, CRC).
fn zip_bytes(entries: &[(&str, u32, u32)]) -> Vec<u8> {
    let mut directory = Vec::new();
    for (name, size, crc) in entries {
        directory.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
        directory.extend_from_slice(&[20, 0, 20, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        directory.extend_from_slice(&crc.to_le_bytes());
        directory.extend_from_slice(&size.to_le_bytes());
        directory.extend_from_slice(&size.to_le_bytes());
        directory.extend_from_slice(&(name.len() as u16).to_le_bytes());
        directory.extend_from_slice(&[0; 12]);
        directory.extend_from_slice(&0u32.to_le_bytes());
        directory.extend_from_slice(name.as_bytes());
    }
    let mut zip = directory.clone();
    zip.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    zip.extend_from_slice(&[0, 0, 0, 0]);
    let count = (entries.len() as u16).to_le_bytes();
    zip.extend_from_slice(&count);
    zip.extend_from_slice(&count);
    zip.extend_from_slice(&(directory.len() as u32).to_le_bytes());
    zip.extend_from_slice(&0u32.to_le_bytes());
    zip.extend_from_slice(&0u16.to_le_bytes());
    zip
}

fn write_zip(path: &Path, entries: &[(&str, u32, u32)]) {
    std::fs::create_dir_all(path.parent().expect("a folder")).expect("folder");
    std::fs::write(path, zip_bytes(entries)).expect("zip");
}

/// Gives the rig a data folder with the shipped data and an arcade index
/// of one BIOS and one game that needs it.
fn with_index(rig: &mut Rig, root: &Path) {
    let shipped = rig.paths.data.clone();
    let data = root.join("data");
    std::fs::create_dir_all(data.join("arcade")).expect("folder");
    for entry in std::fs::read_dir(&shipped).expect("the shipped data") {
        let entry = entry.expect("an entry");
        std::os::unix::fs::symlink(entry.path(), data.join(entry.file_name())).expect("a link");
    }
    let rom = |name: &str, size: u64, crc: u32| RomRecord {
        name: name.to_owned(),
        size,
        crc: Some(crc),
        merge: None,
    };
    let mut bios = SetRecord::new("zbios".to_owned(), "Zed BIOS".to_owned());
    bios.flags.bios = true;
    bios.roms = vec![rom("z1.bin", 128, 0xb1)];
    let mut game = SetRecord::new("starpat".to_owned(), "Star Patrol".to_owned());
    game.romof = Some("zbios".to_owned());
    game.flags.vertical = true;
    game.roms = vec![rom("sp1.bin", 1024, 0x11)];
    let source = IndexSource {
        core: "fbneo".to_owned(),
        ..IndexSource::default()
    };
    let index = ArcadeIndex::from_sets(source, vec![bios, game]).expect("valid sets");
    std::fs::write(
        data.join("arcade/fbneo.idx"),
        arcade::encode(&index).expect("encodes"),
    )
    .expect("the index");
    rig.paths.data = data;
}

/// Boots to the list, waits for the first scan, and opens the arcade list.
fn arcade_list(rig: &mut Rig) {
    rig.boot(None);
    rig.frames(2);
    rig.until(600, "the arcade games", |r| {
        r.shell()
            .snapshot()
            .systems
            .iter()
            .any(|s| s.name == "Arcade")
    });
    rig.focus("Arcade");
    rig.tap(&[Control::A]);
    assert_eq!(rig.screen(), "games");
}

fn notices(rig: &mut Rig) -> Vec<String> {
    rig.shell()
        .snapshot()
        .notices
        .iter()
        .map(|n| n.text.clone())
        .collect()
}

/// Runs frames until the run ends or a notice shows, at most 600.
fn until_notice_or_end(rig: &mut Rig) -> Option<Outcome> {
    for _ in 0..600 {
        if let Some(outcome) = rig.shell().step() {
            return Some(outcome);
        }
        if !notices(rig).is_empty() {
            return None;
        }
    }
    panic!("no notice and no end in 600 frames");
}

#[test]
#[ignore = "tier 2"]
fn with_no_index_the_player_chooses_to_start_or_not() {
    let root = fresh("arcade-no-index");
    let mut rig = Rig::new(&root);
    write_zip(&rig.paths.roms().join("arcade/starpat.zip"), &[]);
    arcade_list(&mut rig);
    rig.focus("starpat");
    rig.tap(&[Control::A]);
    assert_eq!(until_notice_or_end(&mut rig), None);
    assert_eq!(
        notices(&mut rig),
        ["NEPTRON cannot find its list of arcade games, so it cannot check this game."]
    );
    assert_eq!(rig.screen(), "notice");
    rig.tap(&[Control::A]);
    assert_eq!(rig.screen(), "games", "Cancel has the focus");
    assert!(notices(&mut rig).is_empty());

    rig.tap(&[Control::A]);
    assert_eq!(until_notice_or_end(&mut rig), None);
    rig.tap(&[Control::Right]);
    rig.hold(&[Control::A]);
    let outcome = rig.until_end(2);
    let Outcome::Relaunch(NextRun {
        game: Some(game), ..
    }) = outcome
    else {
        panic!("Start relaunches into the game: {outcome:?}");
    };
    assert_eq!(game.path, "arcade/starpat.zip");
    rig.release(&[Control::A]);
    rig.finish();
}

#[test]
#[ignore = "tier 2"]
fn the_set_check_stops_a_game_whose_bios_is_missing_and_starts_a_complete_one() {
    let root = fresh("arcade-check");
    let mut rig = Rig::new(&root);
    with_index(&mut rig, &root);
    write_zip(
        &rig.paths.roms().join("arcade/starpat.zip"),
        &[("sp1.bin", 1024, 0x11)],
    );
    arcade_list(&mut rig);
    rig.focus("Star Patrol");
    rig.tap(&[Control::A]);
    assert_eq!(until_notice_or_end(&mut rig), None);
    assert_eq!(
        notices(&mut rig),
        ["This game needs the BIOS file zbios.zip. Put it in bios, or in the folder of the game."]
    );
    rig.tap(&[Control::A]);
    assert_eq!(rig.screen(), "games");

    write_zip(
        &rig.paths.bios().join("fbneo/zbios.zip"),
        &[("z1.bin", 128, 0xb1)],
    );
    rig.hold(&[Control::A]);
    let outcome = until_notice_or_end(&mut rig);
    let Some(Outcome::Relaunch(NextRun {
        game: Some(game), ..
    })) = outcome
    else {
        panic!("a complete set starts: {outcome:?} {:?}", notices(&mut rig));
    };
    assert_eq!(game.path, "arcade/starpat.zip");
    rig.release(&[Control::A]);
    rig.finish();
}
