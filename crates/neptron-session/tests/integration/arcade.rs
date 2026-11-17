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

/// The source of a board drawn on its side has the turn of the core and
/// the aspect of the frame as drawn: the inverse of the 3:4 of the
/// upright picture that the core reports.
#[test]
fn a_board_on_its_side_gives_a_turned_source() {
    with_script!(
        "a_board_on_its_side_gives_a_turned_source",
        VERTICAL_SCRIPT,
        |dir| {
            let mut rig = Rig::launch(dir, Setup::default());
            let source = rig.session.source();
            assert_eq!(source.rotation, Rotation::R90);
            assert_eq!(source.aspect_milli, 1_333);
            while rig.core_frame() < 40 {
                rig.run(1);
            }
            let source = rig.session.source();
            assert_eq!(source.rotation, Rotation::R270);
            assert_eq!(source.aspect_milli, 1_333);
        }
    );
}

/// On a 15 kHz CRT the upright 320 lines show 1:1 in an interlaced mode,
/// and every flip turns the frame: 90 degrees, then, after the debounce
/// of the new source, 270 degrees.
#[test]
fn a_board_on_its_side_shows_turned_upright() {
    with_script!(
        "a_board_on_its_side_shows_turned_upright",
        VERTICAL_SCRIPT,
        |dir| {
            let mut rig = Rig::launch(dir, Setup::default());
            rig.run(1);
            let mode = rig.display.active_mode().expect("a mode");
            assert!(mode.interlace, "{mode:?}");
            let shown = rig.display.frames().last().unwrap().scene;
            assert_eq!(shown.rotation, Rotation::R90);
            assert_eq!(shown.crop.w, 320, "the crop is in frame pixels");
            assert_eq!(shown.dest.h, 320, "the 320 upright lines 1:1");
            assert!(shown.dest.w < shown.dest.h, "{:?}", shown.dest);
            while rig.core_frame() < 45 {
                rig.run(1);
            }
            let shown = rig.display.frames().last().unwrap().scene;
            assert_eq!(shown.rotation, Rotation::R270);
            assert_eq!(shown.dest.h, 320);
        }
    );
}

/// A half turn keeps the aspect and the shape of the frame.
#[test]
fn a_flipped_board_keeps_its_aspect() {
    with_script!(
        "a_flipped_board_keeps_its_aspect",
        "neptron-testcore 1\ngeometry 320 240 1333\nrotation 2\nat 30 shutdown\n",
        |dir| {
            let mut rig = Rig::launch(dir, Setup::default());
            rig.run(1);
            let source = rig.session.source();
            assert_eq!(
                (source.rotation, source.aspect_milli),
                (Rotation::R180, 1_333)
            );
            let shown = rig.display.frames().last().unwrap().scene;
            assert_eq!(shown.rotation, Rotation::R180);
        }
    );
}

/// With no aspect from the core, the host's fallback is the shape of the
/// frame as drawn, and the session does not invert it.
#[test]
fn a_turned_board_with_no_aspect_keeps_the_fallback() {
    with_script!(
        "a_turned_board_with_no_aspect_keeps_the_fallback",
        "neptron-testcore 1\ngeometry 320 240 0\nrotation 1\nat 30 shutdown\n",
        |dir| {
            let rig = Rig::launch(dir, Setup::default());
            assert!(
                !rig.session
                    .host()
                    .av_info()
                    .unwrap()
                    .geometry
                    .aspect_from_core
            );
            let source = rig.session.source();
            assert_eq!(
                (source.rotation, source.aspect_milli),
                (Rotation::R90, 1_333)
            );
        }
    );
}

/// A board of 384 lines, as Sega System 24 draws.
const TALL: &str = "neptron-testcore 1\ngeometry 496 384 1333\nat 30 shutdown\n";

/// A tall board of a system whose frames are always progressive: 384
/// lines are not interlaced.
#[test]
fn an_always_progressive_system_has_no_interlaced_source() {
    with_script!(
        "an_always_progressive_system_has_no_interlaced_source",
        TALL,
        |dir| {
            let setup = Setup {
                always_progressive: true,
                ..Setup::default()
            };
            let mut rig = Rig::launch(dir, setup);
            for _ in 0..10 {
                rig.run(1);
                assert!(!rig.session.source().interlaced);
            }
            let mode = rig.display.active_mode().expect("a mode");
            assert!(mode.vactive >= 384, "{mode:?}");
        }
    );
}

/// The same frames from a console system: the line count decides.
#[test]
fn a_console_system_takes_a_tall_frame_as_interlaced() {
    with_script!(
        "a_console_system_takes_a_tall_frame_as_interlaced",
        TALL,
        |dir| {
            let rig = Rig::launch(dir, Setup::default());
            assert!(rig.session.source().interlaced);
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
