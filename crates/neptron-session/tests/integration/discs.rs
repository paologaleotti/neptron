//! Disc games: discs added after the load, the change of disc with the
//! tray open for some frames, and the memory card as the save RAM of the
//! game under the name of the game.

use neptron_session::{DISC_TRAY_OPEN_FRAMES, DiscState, Flush};

use crate::harness::{Rig, Setup};
use crate::with_script;

const DISC_SCRIPT: &str = "\
neptron-testcore 6
pixel_format rgb565
geometry 320 240 1333
timing 60000 48000
save_ram 128
discs 1
at 20 sram_write 0 9
";

fn group_setup(dir: &std::path::Path) -> Setup {
    Setup {
        more_discs: vec![dir.join("Saga (Disc 2).chd"), dir.join("Saga (Disc 3).chd")],
        save_name: Some("Saga".to_owned()),
        save_folders: vec!["rpg".to_owned()],
        core_saves_in_game_folder: true,
        ..Setup::default()
    }
}

fn core_lines(rig: &Rig) -> Vec<String> {
    rig.session
        .core_logs()
        .iter()
        .map(|r| r.text.clone())
        .collect()
}

#[test]
fn the_other_discs_of_a_group_are_added_after_the_load() {
    with_script!(
        "the_other_discs_of_a_group_are_added_after_the_load",
        DISC_SCRIPT,
        |dir| {
            let mut rig = Rig::launch(dir, group_setup(dir));
            assert!(rig.session.disc_issues().is_empty());
            assert_eq!(
                rig.session.discs(),
                Some(DiscState {
                    count: 3,
                    index: 0,
                    tray_open: false
                })
            );
            assert!(
                dir.join("saves/rpg").is_dir(),
                "the core gets the save folder of the game"
            );
        }
    );
}

#[test]
fn a_change_of_disc_keeps_the_tray_open_for_some_frames_of_play() {
    with_script!(
        "a_change_of_disc_keeps_the_tray_open_for_some_frames_of_play",
        DISC_SCRIPT,
        |dir| {
            let mut rig = Rig::launch(dir, group_setup(dir));
            rig.run(5);
            rig.open_menu();
            rig.session.change_disc(2).unwrap();
            assert_eq!(core_lines(&rig), ["disc 2 in the drive"]);
            let open = DiscState {
                count: 3,
                index: 2,
                tray_open: true,
            };
            assert_eq!(rig.session.discs(), Some(open));
            rig.run(DISC_TRAY_OPEN_FRAMES.into());
            assert_eq!(rig.session.discs(), Some(open), "no core frame in the menu");
            rig.session.change_disc(1).unwrap();
            rig.session.resume();
            rig.run(u64::from(DISC_TRAY_OPEN_FRAMES) - 1);
            assert!(rig.session.discs().unwrap().tray_open);
            rig.run(1);
            assert_eq!(
                rig.session.discs(),
                Some(DiscState {
                    count: 3,
                    index: 1,
                    tray_open: false
                })
            );
            assert!(rig.session.disc_issues().is_empty());
            assert!(rig.session.change_disc(3).is_err(), "no disc 3");
        }
    );
}

#[test]
fn every_disc_shares_one_memory_card_under_the_name_of_the_game() {
    with_script!(
        "every_disc_shares_one_memory_card_under_the_name_of_the_game",
        DISC_SCRIPT,
        |dir| {
            let card = dir.join("saves/rpg/Saga.srm");
            std::fs::create_dir_all(card.parent().unwrap()).unwrap();
            std::fs::write(&card, [3_u8; 128]).unwrap();
            let mut rig = Rig::launch(dir, group_setup(dir));
            rig.run(30);
            let exit = rig.session.quit();
            assert_eq!(exit.save, Flush::Written);
            let mut expected = [3_u8; 128];
            expected[0] = 9;
            assert_eq!(std::fs::read(&card).unwrap(), expected);
            assert_eq!(
                std::fs::read(dir.join("saves/rpg/Saga.srm.bak")).unwrap(),
                [3; 128]
            );
            assert!(!dir.join("saves/rpg/game.srm").exists());
        }
    );
}

#[test]
fn a_core_with_no_disc_interface_plays_the_first_disc() {
    with_script!(
        "a_core_with_no_disc_interface_plays_the_first_disc",
        "neptron-testcore 6\nsave_ram 16\n",
        |dir| {
            let mut rig = Rig::launch(dir, group_setup(dir));
            assert_eq!(rig.session.discs(), None);
            assert_eq!(rig.session.disc_issues().len(), 1, "logged once");
            assert!(rig.session.change_disc(1).is_err());
        }
    );
}

#[test]
fn a_bad_save_name_falls_back_to_the_content() {
    with_script!(
        "a_bad_save_name_falls_back_to_the_content",
        "neptron-testcore 6\nsave_ram 16\nat 2 sram_write 0 1\n",
        |dir| {
            let setup = Setup {
                save_name: Some("../escape".to_owned()),
                ..Setup::default()
            };
            let mut rig = Rig::launch(dir, setup);
            rig.run(5);
            assert_eq!(rig.session.quit().save, Flush::Written);
            assert!(dir.join("saves/game.srm").exists());
        }
    );
}
