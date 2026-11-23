//! `neptron.toml` on the boot partition: an import into the global
//! settings when the file changes, and nothing when it does not.

use neptron::boot_file::PI_DAC_NOTICE;
use neptron_types::{TV_525_240P, TV_625_288P, TvStandard};
use neptron_video::Phase;

use crate::rig::{Rig, in_child, test_name};
use crate::tv::{composite_rig, dismiss_notices, notice_texts, on_wire, open_options};

fn write_boot_file(rig: &Rig, text: &str) {
    std::fs::create_dir_all(&rig.paths.boot).expect("the boot folder");
    std::fs::write(rig.paths.boot_file(), text).expect("neptron.toml");
}

/// Waits until the scan fills the game list, so the menu has its rows.
fn settle(rig: &mut Rig) {
    rig.frames(2);
    rig.until(600, "the scan", |r| {
        !r.shell().snapshot().systems.is_empty()
    });
}

#[test]
#[ignore = "tier 2"]
fn the_file_is_imported_once_per_change_and_a_bad_file_changes_nothing() {
    in_child(
        &test_name(
            module_path!(),
            "the_file_is_imported_once_per_change_and_a_bad_file_changes_nothing",
        ),
        |dir| {
            let mut rig = composite_rig(dir);

            // The first start copies the file and boots on its standard.
            write_boot_file(&rig, "[display]\ntv = \"pal\"\npi_dac = \"none\"\n");
            rig.boot(None);
            settle(&mut rig);
            assert_eq!(rig.shell().config().tv(), "pal");
            assert_eq!(rig.shell().guard().phase(), Phase::Confirmed);
            assert_eq!(on_wire(&rig), (TV_625_288P, TvStandard::Pal));
            assert_eq!(notice_texts(&mut rig), Vec::<String>::new());
            assert!(
                rig.state_file("config/global.toml")
                    .contains("tv = \"pal\"")
            );
            assert_eq!(rig.state_file("neptron.toml.sha256").trim().len(), 64);

            // A menu change, kept through the guard.
            open_options(&mut rig, "TV standard");
            rig.focus("NTSC (60 Hz)");
            rig.tap(&[neptron_types::Control::A]);
            rig.keep();
            assert_eq!(on_wire(&rig), (TV_525_240P, TvStandard::Ntsc));
            rig.finish();
            assert!(
                rig.state_file("config/global.toml")
                    .contains("tv = \"ntsc\"")
            );

            // The same file at the next start leaves the menu change.
            rig.boot(None);
            settle(&mut rig);
            assert_eq!(rig.shell().config().tv(), "ntsc");
            assert_eq!(on_wire(&rig), (TV_525_240P, TvStandard::Ntsc));
            let lines = rig.shell().log_lines(200);
            assert!(
                lines
                    .iter()
                    .any(|l| l.contains("neptron.toml has not changed")),
                "{lines:?}"
            );
            rig.finish();

            // A changed file is imported again; its bad lines are listed.
            write_boot_file(
                &rig,
                "[display]\ntv = \"pal\"\nwifi = \"home\"\noutput = 7\npi_dac = \"vga666\"\n",
            );
            rig.boot(None);
            settle(&mut rig);
            assert_eq!(rig.shell().config().tv(), "pal");
            assert_eq!(on_wire(&rig), (TV_625_288P, TvStandard::Pal));
            let notices = notice_texts(&mut rig);
            assert_eq!(notices.len(), 3, "{notices:?}");
            assert!(
                notices.contains(
                    &"neptron.toml, line 3: display.wifi is not a setting. It is ignored."
                        .to_owned()
                ),
                "{notices:?}"
            );
            assert!(
                notices
                    .iter()
                    .any(|n| n.starts_with("neptron.toml, line 4: display.output: ")),
                "{notices:?}"
            );
            assert!(
                notices.contains(&format!(
                    "neptron.toml, line 5: display.pi_dac: {PI_DAC_NOTICE}."
                )),
                "{notices:?}"
            );
            dismiss_notices(&mut rig);
            rig.finish();
            let global = rig.state_file("config/global.toml");
            assert!(
                global.contains("tv = \"pal\"") && !global.contains("output"),
                "{global}"
            );

            // A file that does not parse changes nothing, and each new boot
            // lists it until it is fixed.
            write_boot_file(&rig, "[display\ntv = \"ntsc\"\n");
            for _ in 0..2 {
                rig.boot(None);
                settle(&mut rig);
                assert_eq!(rig.shell().config().tv(), "pal");
                assert_eq!(on_wire(&rig), (TV_625_288P, TvStandard::Pal));
                let notices = notice_texts(&mut rig);
                assert!(
                    notices
                        .iter()
                        .any(|n| n.starts_with("neptron.toml does not parse (")),
                    "{notices:?}"
                );
                dismiss_notices(&mut rig);
                rig.finish();
            }
            assert_eq!(rig.state_file("config/global.toml"), global);
        },
    );
}

#[test]
#[ignore = "tier 2"]
fn a_missing_boot_file_changes_nothing_and_shows_no_notice() {
    in_child(
        &test_name(
            module_path!(),
            "a_missing_boot_file_changes_nothing_and_shows_no_notice",
        ),
        |dir| {
            let mut rig = Rig::new(dir);
            rig.boot(None);
            rig.frames(2);
            assert_eq!(notice_texts(&mut rig), Vec::<String>::new());
            let lines = rig.shell().log_lines(200);
            assert!(
                lines
                    .iter()
                    .any(|l| l == "no neptron.toml on the boot partition; the settings stand"),
                "{lines:?}"
            );
            rig.finish();
            assert_eq!(rig.state_file("config/global.toml"), "");
            assert_eq!(rig.state_file("neptron.toml.sha256"), "");
        },
    );
}

#[test]
#[ignore = "tier 2"]
fn a_relaunch_logs_the_problems_of_the_file_and_shows_no_notice() {
    in_child(
        &test_name(
            module_path!(),
            "a_relaunch_logs_the_problems_of_the_file_and_shows_no_notice",
        ),
        |dir| {
            let mut rig = Rig::new(dir);
            write_boot_file(&rig, "[display\n");
            let relaunch = neptron::NextRun {
                pads: Vec::new(),
                game: None,
                notice: None,
            };
            rig.boot(Some(relaunch));
            rig.frames(2);
            assert_eq!(notice_texts(&mut rig), Vec::<String>::new());
            let lines = rig.shell().log_lines(200);
            assert!(
                lines
                    .iter()
                    .any(|l| l.starts_with("neptron.toml does not parse (")),
                "{lines:?}"
            );
            rig.finish();
        },
    );
}
