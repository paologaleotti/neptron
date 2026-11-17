//! Every set, ROM, size and CRC here is invented for the tests. The zips
//! hold a central directory only: the check reads nothing else.

use neptron_library::arcade::{ArcadeIndex, IndexSource, RomRecord, SetRecord};

use super::*;

/// A zip with a central directory of `entries` (name, size, CRC) and no
/// data.
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

/// A share with `roms/arcade/` and `bios/`, removed on drop.
struct Share(PathBuf);

impl Share {
    fn new(label: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("neptron-set-check-{label}-{}", std::process::id()));
        // A leftover of an earlier run with the same process id.
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("roms/arcade")).expect("folder");
        std::fs::create_dir_all(dir.join("bios/fbneo")).expect("folder");
        Self(dir)
    }

    fn zip(&self, path: &str, entries: &[(&str, u32, u32)]) {
        std::fs::write(self.0.join(path), zip_bytes(entries)).expect("zip");
    }

    fn job(&self, stem: &str, arcade: ArcadeState) -> CheckJob {
        CheckJob {
            stem: stem.to_owned(),
            game_folder: self.0.join("roms/arcade"),
            bios: self.0.join("bios"),
            arcade,
            arcade_file: self.0.join("missing.idx"),
        }
    }
}

impl Drop for Share {
    fn drop(&mut self) {
        // A folder left in the temporary directory does no harm.
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn rom(name: &str, size: u64, crc: u32) -> RomRecord {
    RomRecord {
        name: name.to_owned(),
        size,
        crc: Some(crc),
        merge: None,
    }
}

fn ready() -> ArcadeState {
    let mut bios = SetRecord::new("zbios".to_owned(), "Zed BIOS".to_owned());
    bios.flags.bios = true;
    bios.roms = vec![rom("z1.bin", 128, 0xb1)];
    let mut parent = SetRecord::new("starpat".to_owned(), "Star Patrol".to_owned());
    parent.romof = Some("zbios".to_owned());
    parent.roms = vec![rom("sp1.bin", 1024, 0x11)];
    let mut clone = SetRecord::new("starpatj".to_owned(), "Star Patrol".to_owned());
    clone.parent = Some("starpat".to_owned());
    clone.romof = Some("starpat".to_owned());
    clone.roms = vec![rom("sp1.bin", 1024, 0x11), rom("spj.bin", 64, 0x22)];
    let index = ArcadeIndex::from_sets(IndexSource::default(), vec![bios, parent, clone])
        .expect("valid sets");
    ArcadeState::Ready(Arc::new(index))
}

fn verdict(job: CheckJob) -> Verdict {
    run(job, &AtomicBool::new(false)).expect("not cancelled").0
}

#[test]
fn a_split_clone_finds_its_parent_and_its_bios_in_the_core_places() {
    let share = Share::new("split");
    share.zip("roms/arcade/starpatj.zip", &[("spj.bin", 64, 0x22)]);
    share.zip("roms/arcade/starpat.zip", &[("sp1.bin", 1024, 0x11)]);
    share.zip("bios/fbneo/zbios.zip", &[("z1.bin", 128, 0xb1)]);
    assert_eq!(
        verdict(share.job("starpatj", ready())),
        Verdict::Pass {
            bios_differences: Vec::new()
        }
    );
}

#[test]
fn every_found_zip_counts_in_the_core_order() {
    let share = Share::new("order");
    share.zip("roms/arcade/starpat.zip", &[("sp1.bin", 1024, 0x11)]);
    // A damaged copy of the BIOS in `bios/` is opened too, as the core
    // opens it.
    share.zip("bios/fbneo/zbios.zip", &[("z1.bin", 128, 0xb1)]);
    std::fs::write(share.0.join("bios/zbios.zip"), b"not a zip").expect("file");
    let found = verdict(share.job("starpat", ready()));
    assert!(
        matches!(&found, Verdict::Unreadable { file, .. } if file == "zbios.zip"),
        "{found:?}"
    );
}

#[test]
fn a_missing_bios_stops_the_launch() {
    let share = Share::new("no-bios");
    share.zip("roms/arcade/starpat.zip", &[("sp1.bin", 1024, 0x11)]);
    assert_eq!(
        verdict(share.job("starpat", ready())),
        Verdict::MissingBios {
            bios: "zbios".to_owned()
        }
    );
}

#[test]
fn an_unknown_stem_and_a_missing_index_have_their_verdicts() {
    let share = Share::new("unknown");
    share.zip("roms/arcade/mystery.zip", &[]);
    assert!(matches!(
        verdict(share.job("mystery", ready())),
        Verdict::UnknownSet { .. }
    ));
    let (found, state) = run(
        share.job("mystery", ArcadeState::Unread),
        &AtomicBool::new(false),
    )
    .expect("not cancelled");
    assert_eq!(found, Verdict::NoIndex);
    assert!(matches!(state, ArcadeState::Failed(_)), "the file was read");
}

#[test]
fn a_cancelled_check_gives_nothing() {
    let share = Share::new("cancel");
    share.zip("roms/arcade/starpat.zip", &[("sp1.bin", 1024, 0x11)]);
    assert!(run(share.job("starpat", ready()), &AtomicBool::new(true)).is_none());
}

#[test]
fn the_check_thread_gives_its_verdict_and_drop_joins_it() {
    let share = Share::new("thread");
    share.zip("roms/arcade/starpat.zip", &[("sp1.bin", 1024, 0x11)]);
    share.zip("bios/zbios.zip", &[("z1.bin", 128, 0xb1)]);
    let checking = Checking::start(GameId(7), share.job("starpat", ready())).expect("starts");
    while !checking.is_finished() {
        std::thread::yield_now();
    }
    assert_eq!(checking.game, GameId(7));
    let (found, _) = checking.join().expect("a verdict");
    assert!(matches!(found, Verdict::Pass { .. }), "{found:?}");
    let dropped = Checking::start(GameId(8), share.job("starpat", ready())).expect("starts");
    drop(dropped);
}

#[test]
fn the_places_follow_the_core() {
    let [game, core, bios] = places(
        Path::new("/share/roms/arcade/sub"),
        Path::new("/share/bios"),
    );
    assert_eq!(game, Path::new("/share/roms/arcade/sub"));
    assert_eq!(core, Path::new("/share/bios/fbneo"));
    assert_eq!(bios, Path::new("/share/bios"));
}
