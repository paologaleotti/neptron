//! Goldens of the turned output: the whole picture in scan-out pixels, as
//! the reference renderer of the fake display draws the scenes of the
//! engine. One table drives every case: four display classes, the screen
//! at 90, 180 and 270, and three scenes (the game list, a vertical game
//! under the quick menu, a horizontal game). The game frames are the test
//! card of the core: a border, a diagonal, a circle that is round on the
//! glass, an arrow to the top and a marked top-left corner. At 0 the picture equals the picture of a run with no
//! orientation setting.
//!
//! A missing or different golden fails, and the test writes the actual
//! picture to `turned-output/` in the Cargo test temporary directory. Look
//! at it before you copy it to `tests/golden/turned/`.

use std::path::{Path, PathBuf};

use neptron::NextRun;
use neptron_testkit::{Golden, RgbImage};
use neptron_types::Rotation;

use crate::panel::{PANEL_1080P, sysfs_with_panel};
use crate::rig::{Rig, in_children_with, test_name};
use crate::tv::{composite_rig, dismiss_notices};

/// The display classes of the goldens.
#[derive(Clone, Copy, Debug)]
enum Class {
    /// The preset of a tube, by its id.
    Crt(&'static str),
    /// The 1080p flat panel.
    Panel,
    /// NTSC on the composite output.
    TvNtsc,
}

impl Class {
    fn name(self) -> &'static str {
        match self {
            Class::Crt(id) => id,
            Class::Panel => "panel_1080",
            Class::TvNtsc => "tv_ntsc",
        }
    }
}

const CLASSES: [Class; 4] = [
    Class::Crt("generic_15"),
    Class::Crt("arcade_31"),
    Class::Panel,
    Class::TvNtsc,
];

/// The screen orientation of a stage, or `None` for no setting at all.
const SCREENS: [Option<Rotation>; 5] = [
    None,
    Some(Rotation::R0),
    Some(Rotation::R90),
    Some(Rotation::R180),
    Some(Rotation::R270),
];

/// A 320x240 board turned by the core by `rotation` quarter turns, as
/// FinalBurn Neo sends it: 1 is a vertical game (DoDonPachi: the aspect
/// 3:4 of the upright picture), 0 a horizontal one (4:3). The frames are
/// the test card of the core, so a picture shows its shape and its top.
fn board(rotation: u32) -> String {
    let aspect_milli = if rotation % 2 == 1 { 750 } else { 1333 };
    format!(
        "neptron-testcore 4\npattern shape\ngeometry 320 240 {aspect_milli}\ntiming 57550 48000\nrotation {rotation}\n"
    )
}

/// A rig under `dir/<class>-<screen>` on `class`, with the screen
/// orientation `screen` in the global settings.
fn rig_on(dir: &Path, class: Class, screen: Option<Rotation>) -> Rig {
    let root = dir.join(format!("{}-{}", class.name(), degrees(screen)));
    std::fs::create_dir_all(&root).expect("the folder");
    let mut rig = match class {
        Class::TvNtsc => composite_rig(&root),
        _ => Rig::new(&root),
    };
    let mut text = "schema_version = 1\n\n[display]\n".to_owned();
    if let Some(screen) = screen {
        text.push_str(&format!("orientation = \"{}\"\n", screen.degrees()));
    }
    match class {
        Class::Crt(id) => text.push_str(&format!("monitor = \"{id}\"\n")),
        Class::Panel => {
            sysfs_with_panel(&rig.paths.sysfs, PANEL_1080P);
            rig.output = "HDMI-A-1".to_owned();
        }
        Class::TvNtsc => text.push_str("tv = \"ntsc\"\n"),
    }
    let config = rig.paths.config();
    std::fs::create_dir_all(&config).expect("the config folder");
    std::fs::write(config.join("global.toml"), text).expect("the settings");
    rig
}

fn degrees(screen: Option<Rotation>) -> String {
    screen.map_or("none".to_owned(), |s| s.degrees().to_string())
}

fn game() -> NextRun {
    NextRun {
        pads: Vec::new(),
        game: Some(neptron::relaunch::GameRef {
            system: "nes".to_owned(),
            path: "nes/Test Game.nes".to_owned(),
        }),
        notice: None,
    }
}

/// The picture on the screen now.
fn screen(rig: &Rig) -> RgbImage {
    rig.display.screen().expect("a picture on the screen")
}

/// The game list after the scan, with the boot prompt kept and the
/// notices dismissed.
fn game_list(rig: &mut Rig) -> RgbImage {
    rig.boot(None);
    rig.frames(2);
    if rig.shell().guard().pending().is_some() {
        rig.keep();
    }
    rig.scanned();
    rig.frames(2);
    dismiss_notices(rig);
    rig.frames(2);
    let image = screen(rig);
    rig.finish();
    image
}

/// The quick menu over the game of the board, with the notices dismissed.
fn quick_menu(rig: &mut Rig) -> RgbImage {
    rig.boot(Some(game()));
    rig.frames(30);
    rig.open_menu();
    rig.frames(1);
    dismiss_notices(rig);
    rig.frames(2);
    assert_eq!(rig.screen(), "quick-menu");
    let image = screen(rig);
    rig.finish();
    image
}

/// The game of the board running, with no UI over it.
fn running(rig: &mut Rig) -> RgbImage {
    rig.boot(Some(game()));
    rig.frames(30);
    let last = rig.display.frames().last().expect("a frame").scene;
    assert_eq!(last.ui, None, "nothing over the running game");
    let image = screen(rig);
    rig.finish();
    image
}

/// The folder of the goldens, and the folder of the actual pictures.
fn goldens() -> Golden {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    Golden::new(
        manifest.join("tests/golden/turned"),
        PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("turned-output"),
    )
}

/// Checks the picture of one stage: a golden at 90, 180 and 270; at 0,
/// the picture of the stage with no setting, which the stage before
/// leaves in `dir`.
fn check(dir: &Path, class: Class, screen: Option<Rotation>, scene: &str, image: &RgbImage) {
    let name = format!("{}-{}-{scene}", class.name(), degrees(screen));
    let today = dir.join(format!("{}-{scene}.ppm", class.name()));
    match screen {
        None => {
            std::fs::write(&today, neptron_testkit::golden::encode_ppm(image))
                .expect("the picture with no setting");
        }
        Some(Rotation::R0) => {
            let bytes = std::fs::read(&today).expect("the picture with no setting");
            let before = neptron_testkit::golden::decode_ppm(&bytes).expect("a PPM");
            assert!(
                neptron_testkit::golden::difference(&before, image).is_none(),
                "{name}: the screen at 0 differs from the screen with no setting"
            );
        }
        Some(_) => {
            if let Err(e) = goldens().check(&name, image) {
                panic!("{name}: {e}");
            }
        }
    }
}

/// Runs the stages of `scenes` on the board `board`: every class at every
/// screen of [`SCREENS`], in that order, one game per child.
fn run(function: &str, board: &str, scenes: fn(&Path, Class, Option<Rotation>)) {
    let name = test_name(module_path!(), function);
    let stages = (CLASSES.len() * SCREENS.len()) as u32;
    in_children_with(&name, board, stages, |dir, stage| {
        let class = CLASSES[stage as usize / SCREENS.len()];
        let screen = SCREENS[stage as usize % SCREENS.len()];
        scenes(dir, class, screen);
    });
}

#[test]
#[ignore = "tier 2"]
fn the_game_list_and_a_vertical_game_under_the_quick_menu_match_their_goldens() {
    run(
        "the_game_list_and_a_vertical_game_under_the_quick_menu_match_their_goldens",
        &board(1),
        |dir, class, screen| {
            let mut rig = rig_on(dir, class, screen);
            let image = game_list(&mut rig);
            check(dir, class, screen, "list", &image);
            let image = quick_menu(&mut rig);
            check(dir, class, screen, "vertical-quick-menu", &image);
        },
    );
}

#[test]
#[ignore = "tier 2"]
fn a_horizontal_game_matches_its_golden() {
    run(
        "a_horizontal_game_matches_its_golden",
        &board(0),
        |dir, class, screen| {
            let image = running(&mut rig_on(dir, class, screen));
            check(dir, class, screen, "horizontal", &image);
        },
    );
}
