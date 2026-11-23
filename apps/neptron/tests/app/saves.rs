//! The per-game files of games in folders: two games of one name in two
//! folders keep two saves and two settings files, and a game in a folder
//! takes the flat files of the earlier layout only when no game of its
//! name is at the top of the system folder.

use neptron::NextRun;
use neptron::relaunch::GameRef;

use crate::rig::{Rig, in_children, test_name};

/// The settings file of `game`, a key below `games/nes`, with the picture
/// style `style`.
fn game_style(rig: &Rig, key: &str, style: &str) {
    let path = rig.paths.config().join(format!("games/nes/{key}.toml"));
    std::fs::create_dir_all(path.parent().expect("a parent")).expect("the folder");
    std::fs::write(
        &path,
        format!("schema_version = 1\n\n[display]\nstyle = \"{style}\"\n"),
    )
    .expect("the settings file");
}

fn rom(rig: &Rig, path: &str) {
    let path = rig.paths.roms().join(path);
    std::fs::create_dir_all(path.parent().expect("a parent")).expect("the folder");
    std::fs::write(path, b"test core content").expect("a game");
}

fn save(rig: &Rig, key: &str) -> Option<Vec<u8>> {
    std::fs::read(rig.paths.saves("nes").join(format!("{key}.srm"))).ok()
}

/// Plays the game at `path` until the core wrote its save RAM (byte 0 is
/// 7), then ends the run, which writes the save. Gives the game key and
/// the picture style of the run.
fn play(rig: &mut Rig, path: &str) -> (String, String) {
    rig.boot(Some(NextRun {
        pads: Vec::new(),
        game: Some(GameRef {
            system: "nes".to_owned(),
            path: path.to_owned(),
        }),
        notice: None,
    }));
    assert!(rig.shell().in_game(), "{path}");
    rig.frames(400);
    let key = rig.shell().config().context().expect("a game").game.clone();
    let style = rig.shell().config().text("display.style");
    rig.finish();
    (key, style)
}

#[test]
#[ignore = "tier 2"]
fn games_of_one_name_in_two_folders_keep_their_own_files() {
    let name = test_name(
        module_path!(),
        "games_of_one_name_in_two_folders_keep_their_own_files",
    );
    in_children(&name, 3, |dir, stage| {
        let mut rig = Rig::new(dir);
        // `nes/Test Game.nes` is at the top; its flat files are its own.
        let flat = [9_u8; 64];
        if stage == 0 {
            rom(&rig, "nes/hacks/Test Game.nes");
            rom(&rig, "nes/translations/fr/Test Game.nes");
            game_style(&rig, "Test Game", "2x");
            game_style(&rig, "hacks/Test Game", "4x");
            std::fs::create_dir_all(rig.paths.saves("nes")).expect("the save folder");
            std::fs::write(rig.paths.saves("nes").join("Test Game.srm"), flat).expect("save");
        }
        match stage {
            0 => {
                let (key, style) = play(&mut rig, "nes/hacks/Test Game.nes");
                assert_eq!(key, "hacks/Test Game");
                assert_eq!(style, "4x", "the settings file of the folder");
                let own = save(&rig, "hacks/Test Game").expect("its own save");
                assert_eq!(own[..2], [7, 0], "not a copy of the flat save");
            }
            1 => {
                let (key, style) = play(&mut rig, "nes/translations/fr/Test Game.nes");
                assert_eq!(key, "translations/fr/Test Game");
                assert!(style != "4x" && style != "2x", "no file: {style}");
                assert!(
                    !rig.paths
                        .config()
                        .join("games/nes/translations/fr/Test Game.toml")
                        .exists(),
                    "the flat settings of the game on top are not copied"
                );
                let own = save(&rig, "translations/fr/Test Game").expect("its own save");
                assert_eq!(own[..2], [7, 0]);
            }
            _ => {
                let (key, style) = play(&mut rig, "nes/Test Game.nes");
                assert_eq!(key, "Test Game");
                assert_eq!(style, "2x");
                let top = save(&rig, "Test Game").expect("the flat save");
                assert_eq!(top[..2], [7, 9], "the game on top loads its flat save");
                let hack = save(&rig, "hacks/Test Game").expect("the save of stage 0");
                assert_eq!(hack[..2], [7, 0], "untouched by the others");
            }
        }
    });
}

/// A game that sat in a folder before the files followed the folders: its
/// flat save and settings are copied once, and the flat files stay.
#[test]
#[ignore = "tier 2"]
fn a_game_in_a_folder_takes_a_copy_of_its_earlier_flat_files() {
    let name = test_name(
        module_path!(),
        "a_game_in_a_folder_takes_a_copy_of_its_earlier_flat_files",
    );
    in_children(&name, 1, |dir, _| {
        let mut rig = Rig::new(dir);
        rom(&rig, "nes/old/Other Game.nes");
        game_style(&rig, "Other Game", "4x");
        let saves = rig.paths.saves("nes");
        std::fs::create_dir_all(&saves).expect("the save folder");
        std::fs::write(saves.join("Other Game.srm"), [9_u8; 64]).expect("save");
        let flat_settings =
            std::fs::read(rig.paths.config().join("games/nes/Other Game.toml")).expect("flat");

        let (key, style) = play(&mut rig, "nes/old/Other Game.nes");
        assert_eq!(key, "old/Other Game");
        assert_eq!(style, "4x", "the copied settings");
        assert_eq!(
            std::fs::read(rig.paths.config().join("games/nes/old/Other Game.toml"))
                .expect("the copy"),
            flat_settings
        );
        let own = save(&rig, "old/Other Game").expect("the copied save");
        assert_eq!(own[..2], [7, 9], "the copy, then the game's write");
        assert_eq!(
            std::fs::read(saves.join("old/Other Game.srm.bak")).expect("the backup"),
            [9; 64]
        );
        assert_eq!(
            std::fs::read(saves.join("Other Game.srm")).expect("the flat save"),
            [9; 64],
            "never moved or changed"
        );
        assert_eq!(
            std::fs::read(rig.paths.config().join("games/nes/Other Game.toml")).expect("flat"),
            flat_settings
        );
    });
}
