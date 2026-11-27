//! Disc games through the frontend with a test system of discs and BIOS
//! files (`docs/spec/disc-games.md`): one entry for each game in the list,
//! the BIOS check before launch with its notice, the system folder of the
//! core, and the change of disc from the quick menu. Every file is
//! synthetic.

use neptron::NextRun;
use neptron::relaunch::GameRef;
use neptron_library::{Catalog, Source, bios};
use neptron_types::Control;
use neptron_ui::snapshot::SettingValue;

use crate::rig::{Rig, core_path, in_child_with, test_name};

/// The content of the BIOS file of the test system.
const BIOS: &[u8] = b"synthetic bios!!";

/// The test core with two discs, as an m3u file of two would give.
const DISC_SCRIPT: &str = "\
neptron-testcore 6
pixel_format rgb565
geometry 256 224 1333
max_size 320 240
timing 60099 48000
save_ram 64
discs 1
at 10 sram_write 0 5
";

fn catalog() -> Catalog {
    let system = format!(
        "format = 1\nfolder = \"testdisc\"\naliases = []\nname = \"Test Disc\"\nextensions = [\"chd\", \"cue\", \"m3u\"]\ndiscs = [\"chd\", \"cue\", \"m3u\"]\n\n[[bios]]\nfile = \"td-us.bin\"\nregion = \"ntsc-u\"\nsize = 16\ncrc32 = [\"{:08x}\"]\n",
        bios::crc32(BIOS)
    );
    let core = "format = 1\nid = \"testdisc\"\nname = \"Test Disc Core\"\nlibrary = \"testdisc_libretro.so\"\nsystems = [\"testdisc\"]\nlicence = \"MIT\"\n";
    Catalog::from_sources(
        &[Source {
            name: "testdisc",
            text: &system,
        }],
        &[Source {
            name: "testdisc",
            text: core,
        }],
    )
    .expect("the test catalog")
}

/// A rig with the test system, its core, and a game of two cue discs.
fn disc_rig(dir: &std::path::Path) -> Rig {
    let mut rig = Rig::new(dir);
    rig.catalog = Some(catalog());
    let link = rig.paths.cores.join("testdisc_libretro.so");
    if !link.exists() {
        std::os::unix::fs::symlink(core_path(), link).expect("the core link");
    }
    let roms = rig.paths.roms().join("testdisc");
    std::fs::create_dir_all(&roms).expect("the folder");
    for disc in 1..=2 {
        std::fs::write(
            roms.join(format!("Saga (USA) (Disc {disc}).cue")),
            format!("FILE \"saga{disc}.bin\" BINARY\n"),
        )
        .expect("a cue");
        std::fs::write(roms.join(format!("saga{disc}.bin")), b"track").expect("a bin");
    }
    rig
}

fn saga() -> NextRun {
    NextRun {
        pads: Vec::new(),
        game: Some(GameRef {
            system: "testdisc".to_owned(),
            path: "testdisc/Saga (USA) (Disc 1).cue".to_owned(),
        }),
        notice: None,
    }
}

#[test]
#[ignore = "tier 2"]
fn a_game_with_no_bios_does_not_start_and_says_which_file_to_put_where() {
    let name = test_name(
        module_path!(),
        "a_game_with_no_bios_does_not_start_and_says_which_file_to_put_where",
    );
    in_child_with(&name, DISC_SCRIPT, |dir| {
        let mut rig = disc_rig(dir);
        let (next, log) = rig.try_boot(Some(saga())).expect("no start");
        assert_eq!(
            next.notice.as_deref(),
            Some(
                "Saga (USA) did not start: the Test Disc BIOS is missing. Put td-us.bin in the bios folder."
            )
        );
        assert!(
            log.iter().any(|l| l == "BIOS td-us.bin: missing"),
            "{log:?}"
        );
        assert_eq!(next.game, None, "the run goes back to the list");
    });
}

#[test]
#[ignore = "tier 2"]
fn the_list_shows_one_entry_and_the_bios_page_finds_the_bios_by_content() {
    let name = test_name(
        module_path!(),
        "the_list_shows_one_entry_and_the_bios_page_finds_the_bios_by_content",
    );
    in_child_with(&name, DISC_SCRIPT, |dir| {
        let mut rig = disc_rig(dir);
        let bios_folder = rig.paths.bios().join("sony");
        std::fs::create_dir_all(&bios_folder).expect("the folder");
        std::fs::write(bios_folder.join("my bios.rom"), BIOS).expect("the BIOS");
        rig.boot(None);
        rig.scanned();
        let index = rig.shell().library().index();
        let names: Vec<&str> = index
            .system("testdisc")
            .expect("the system")
            .games()
            .iter()
            .map(|g| g.name())
            .collect();
        assert_eq!(names, ["Saga (USA)"]);
        let survey = rig.shell().library().bios().expect("a survey");
        assert!(survey.entries("testdisc")[0].is_found());
        rig.finish();
    });
}

#[test]
#[ignore = "tier 2"]
fn the_quick_menu_changes_the_disc_of_a_game_of_two_discs() {
    let name = test_name(
        module_path!(),
        "the_quick_menu_changes_the_disc_of_a_game_of_two_discs",
    );
    in_child_with(&name, DISC_SCRIPT, |dir| {
        let mut rig = disc_rig(dir);
        std::fs::create_dir_all(rig.paths.bios()).expect("the folder");
        std::fs::write(rig.paths.bios().join("other name.bin"), BIOS).expect("the BIOS");
        let own = rig.paths.core_system("testdisc").join("card.mcr");
        std::fs::create_dir_all(own.parent().expect("a parent")).expect("the folder");
        std::fs::write(&own, b"a file of the core").expect("a file of the core");
        rig.boot(Some(saga()));
        assert!(rig.shell().in_game());
        let copy = rig.paths.core_system("testdisc").join("td-us.bin");
        assert_eq!(std::fs::read(&copy).expect("the copy"), BIOS);
        assert_eq!(
            std::fs::read(&own).expect("the file of the core stays"),
            b"a file of the core"
        );
        rig.frames(30);
        rig.open_menu();
        rig.focus("Disc");
        rig.tap(&[Control::A]);
        let rows: Vec<(String, Option<String>)> = rig
            .shell()
            .snapshot()
            .settings
            .rows
            .iter()
            .map(|r| (r.label.clone(), r.unavailable.clone()))
            .collect();
        assert_eq!(
            rows,
            [
                ("Disc 1".to_owned(), Some("In the drive".to_owned())),
                ("Disc 2".to_owned(), None),
            ]
        );
        rig.focus("Disc 2");
        rig.tap(&[Control::A]);
        let disc_two = &rig.shell().snapshot().settings.rows[1];
        assert_eq!(disc_two.unavailable.as_deref(), Some("In the drive"));
        assert!(matches!(disc_two.value, SettingValue::Action(_)));
        assert!(
            rig.shell()
                .log_lines(50)
                .iter()
                .any(|l| l.contains("disc 1 put in the drive")),
            "{:?}",
            rig.shell().log_lines(50)
        );
        rig.finish();
        let card = rig.paths.saves("testdisc").join("Saga (USA).srm");
        assert_eq!(std::fs::read(card).expect("one card for the game")[0], 5);
    });
}

#[test]
#[ignore = "tier 2"]
fn the_bios_page_of_a_game_run_surveys_when_it_opens() {
    let name = test_name(
        module_path!(),
        "the_bios_page_of_a_game_run_surveys_when_it_opens",
    );
    in_child_with(&name, DISC_SCRIPT, |dir| {
        let mut rig = disc_rig(dir);
        std::fs::create_dir_all(rig.paths.bios()).expect("the folder");
        std::fs::write(rig.paths.bios().join("x.bin"), BIOS).expect("the BIOS");
        rig.boot(Some(saga()));
        assert!(rig.shell().in_game());
        assert!(
            rig.shell().library().bios().is_none(),
            "no survey of the folder after the launch"
        );
        rig.frames(30);
        rig.open_menu();
        rig.focus("Settings");
        rig.tap(&[Control::A]);
        rig.focus("System");
        rig.tap(&[Control::A]);
        rig.focus("BIOS files");
        rig.tap(&[Control::A]);
        let shown = |rig: &mut Rig| {
            rig.shell()
                .snapshot()
                .settings
                .rows
                .first()
                .map(|r| r.value.clone())
        };
        // The survey thread runs in real time, so the bound is real time.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
        while shown(&mut rig) != Some(SettingValue::Text("Found: td-us.bin".to_owned())) {
            assert!(
                std::time::Instant::now() < deadline,
                "the page shows {:?}",
                shown(&mut rig)
            );
            rig.frames(1);
        }
        rig.finish();
    });
}
