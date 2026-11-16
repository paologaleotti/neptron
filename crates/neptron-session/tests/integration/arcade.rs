//! The parts of the session that arcade games need: option values for a
//! core that declares its options at load, the rotation of a board drawn
//! on its side, a core with no save RAM, and the zip listing for the set
//! check.

use std::io::Cursor;

use neptron_session::{Flush, ZipError, zip_entries};
use neptron_types::Rotation;

use crate::harness::{Rig, Setup};
use crate::with_script;

/// A board drawn on its side that declares its options at load and has no
/// save RAM, as FinalBurn Neo runs DoDonPachi: 320x240 frames, the aspect
/// of the upright picture, a turn of 90 degrees at load and one of 270
/// degrees later.
const VERTICAL_SCRIPT: &str = "\
neptron-testcore 1
geometry 320 240 750
options load
rotation 1
at 40 rotation 3
at 60 shutdown
";

#[test]
fn held_values_reach_a_core_that_declares_its_options_at_load() {
    with_script!(
        "held_values_reach_a_core_that_declares_its_options_at_load",
        VERTICAL_SCRIPT,
        |dir| {
            let setup = Setup {
                core_options: vec![
                    ("neptron_testcore_dip".to_owned(), "off".to_owned()),
                    ("neptron_testcore_dip".to_owned(), "on".to_owned()),
                    ("neptron_testcore_speed".to_owned(), "2".to_owned()),
                ],
                ..Setup::default()
            };
            let rig = Rig::launch(dir, setup);
            let dip = rig
                .session
                .host()
                .options()
                .iter()
                .find(|o| o.key() == "neptron_testcore_dip")
                .map(|o| o.value().to_owned());
            assert_eq!(dip.as_deref(), Some("on"), "the later value wins");
            assert_eq!(rig.session.counters().refused_options, 1);
        }
    );
}

#[test]
fn the_rotation_of_the_core_reaches_the_host_of_the_session() {
    with_script!(
        "the_rotation_of_the_core_reaches_the_host_of_the_session",
        VERTICAL_SCRIPT,
        |dir| {
            let mut rig = Rig::launch(dir, Setup::default());
            assert_eq!(rig.session.host().rotation(), Rotation::R90);
            let geometry = rig.session.host().av_info().unwrap().geometry;
            assert_eq!(geometry.aspect_milli, 750);
            assert!(geometry.aspect_from_core);
            while rig.core_frame() < 40 {
                rig.run(1);
            }
            assert_eq!(rig.session.host().rotation(), Rotation::R270);
            let source = rig.session.source();
            assert_eq!((source.width, source.height), (320, 240));
        }
    );
}

#[test]
fn a_core_with_no_save_ram_writes_no_save_file() {
    with_script!(
        "a_core_with_no_save_ram_writes_no_save_file",
        VERTICAL_SCRIPT,
        |dir| {
            let mut rig = Rig::launch(dir, Setup::default());
            while !rig.session.ended() {
                rig.run(1);
            }
            assert!(rig.session.save_stats().is_none());
            let exit = rig.session.quit();
            assert_eq!(exit.save, Flush::NoSaveRam);
            let saves: Vec<_> = std::fs::read_dir(dir.join("saves"))
                .unwrap()
                .map(|e| e.unwrap().file_name())
                .collect();
            assert!(
                saves.is_empty(),
                "no .srm, no .bak, no temp file: {saves:?}"
            );
        }
    );
}

/// The smallest archive: an end record and no entry.
fn empty_zip() -> Vec<u8> {
    let mut zip = b"PK\x05\x06".to_vec();
    zip.extend_from_slice(&[0; 18]);
    zip
}

#[test]
fn the_zip_listing_is_reachable_through_the_session() {
    assert_eq!(zip_entries(&mut Cursor::new(empty_zip())).unwrap(), []);
    assert!(matches!(
        zip_entries(&mut Cursor::new(b"not a zip".to_vec())),
        Err(ZipError::NoEndOfCentralDirectory)
    ));
}
