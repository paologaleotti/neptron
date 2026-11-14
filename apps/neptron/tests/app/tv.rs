//! A composite TV output: a connector with the `TV mode` property, as
//! `vc4` gives it on a Raspberry Pi 3. The fake display refuses every mode
//! that does not come from a TV profile, as the KMS backend does, so every
//! path that sets a mode must build it from the TV profile of a standard.

use std::path::Path;

use neptron::NextRun;
use neptron::pages::NOT_USED_ON_A_TV;
use neptron_input::BLIND_RECOVERY_HOLD_NS;
use neptron_testkit::{DisplayOptions, FakeDisplay};
use neptron_types::{
    Control, GpuCaps, Modeline, Rect, TV_525_240P, TV_625_288P, TV_PCLOCK_HZ, TvStandard,
};
use neptron_video::{Phase, SafeReason};

use crate::rig::{Rig, in_child, in_children, test_name};

/// The caps of the composite entry of the capability table.
const COMPOSITE_CAPS: GpuCaps = GpuCaps {
    pclock_min_hz: TV_PCLOCK_HZ,
    pclock_max_hz: TV_PCLOCK_HZ,
    interlace: true,
    flip_per_field: false,
    max_width: 720,
    max_height: 576,
};

/// The confirm window and a margin, in frames at 60 Hz.
const CONFIRM_WINDOW_FRAMES: u32 = 660;

/// The frames of the blind recovery hold at 60 Hz, and a margin.
const HOLD_FRAMES: u32 = (BLIND_RECOVERY_HOLD_NS / 16_000_000) as u32 + 30;

/// A sysfs tree of a Pi 3 under `root`: the `vc4` card with HDMI
/// disconnected and the composite connector, whose status is `unknown`.
pub(crate) fn sysfs_with_composite(root: &Path) {
    let drm = root.join("class/drm");
    let card = drm.join("card1/device");
    std::fs::create_dir_all(&card).expect("the card");
    std::fs::write(
        card.join("uevent"),
        "DRIVER=vc4-drm\nOF_NAME=gpu\nOF_COMPATIBLE_0=brcm,bcm2835-vc4\nOF_COMPATIBLE_N=1\n",
    )
    .expect("the uevent");
    for (name, status) in [("HDMI-A-1", "disconnected"), ("Composite-1", "unknown")] {
        let connector = drm.join(format!("card1-{name}"));
        std::fs::create_dir_all(&connector).expect("the connector");
        std::fs::write(connector.join("status"), format!("{status}\n")).expect("the status");
    }
}

/// A rig on the composite connector of a Pi 3, whose `TV mode` property
/// lists every standard.
pub(crate) fn composite_rig(dir: &Path) -> Rig {
    let mut rig = Rig::new(dir);
    sysfs_with_composite(&rig.paths.sysfs);
    let standards = TvStandard::ALL.to_vec();
    rig.display = FakeDisplay::with_options(
        rig.clock.clone(),
        COMPOSITE_CAPS,
        DisplayOptions {
            tv_standards: Some(standards.clone()),
            ..DisplayOptions::default()
        },
    );
    rig.output = "Composite-1".to_owned();
    rig.tv_standards = Some(standards);
    rig
}

/// Every mode the run asked for carries a TV standard and is one of the
/// two modes of that standard, and the fake took each one.
fn assert_only_tv_modes(rig: &Rig) {
    let calls = rig.display.mode_calls();
    assert!(!calls.is_empty(), "no mode was set");
    for call in &calls {
        let standard = call
            .tv_standard
            .unwrap_or_else(|| panic!("no standard: {call:?}"));
        assert!(
            standard.modes().contains(&call.requested),
            "not a mode of {standard:?}: {call:?}"
        );
        assert!(call.result.is_ok(), "refused: {call:?}");
    }
}

fn on_wire(rig: &Rig) -> (Modeline, TvStandard) {
    (
        rig.display.active_mode().expect("a mode"),
        rig.display.active_tv_standard().expect("a standard"),
    )
}

fn last_ui_dest(rig: &Rig) -> Rect {
    rig.display.frames().last().expect("a frame").scene.ui_dest
}

/// Closes the notices on show with OK.
fn dismiss_notices(rig: &mut Rig) {
    for _ in 0..8 {
        if rig.screen() != "notice" {
            return;
        }
        rig.tap(&[Control::A]);
    }
}

/// Opens Settings, Display and the option list of `row`.
fn open_options(rig: &mut Rig, row: &str) {
    rig.focus("Settings");
    rig.tap(&[Control::A]);
    rig.focus("Display");
    rig.tap(&[Control::A]);
    rig.focus(row);
    rig.tap(&[Control::A]);
    assert_eq!(rig.screen(), "options");
}

fn game() -> NextRun {
    NextRun {
        game: Some(neptron::relaunch::GameRef {
            system: "nes".to_owned(),
            path: "nes/Test Game.nes".to_owned(),
        }),
        notice: None,
    }
}

#[test]
#[ignore = "tier 2"]
fn a_tv_output_starts_on_ntsc_240p_with_the_menu_at_twice_its_width() {
    in_child(
        &test_name(
            module_path!(),
            "a_tv_output_starts_on_ntsc_240p_with_the_menu_at_twice_its_width",
        ),
        |dir| {
            let mut rig = composite_rig(dir);
            rig.boot(None);
            rig.frames(2);
            assert!(matches!(rig.shell().guard().phase(), Phase::Pending(_)));
            assert_eq!(on_wire(&rig), (TV_525_240P, TvStandard::Ntsc));
            assert_eq!(
                last_ui_dest(&rig),
                Rect {
                    x: 40,
                    y: 0,
                    w: 640,
                    h: 240
                }
            );
            let display = rig.shell().snapshot().display.clone().expect("display");
            assert_eq!(display.output, "Composite-1");
            assert_eq!(display.mode, "NTSC 240p 60.05 Hz");
            assert_eq!(display.monitor, "Composite TV");
            rig.keep();
            assert_eq!(rig.shell().guard().phase(), Phase::Confirmed);
            assert_only_tv_modes(&rig);
            rig.finish();
            let record = rig.state_file("video/guard.toml");
            assert!(record.contains("id = \"tv_ntsc\""), "{record}");
            assert!(record.contains("output = \"Composite-1\""), "{record}");
        },
    );
}

#[test]
#[ignore = "tier 2"]
fn a_new_standard_reverts_unless_kept_and_is_written_when_kept() {
    in_child(
        &test_name(
            module_path!(),
            "a_new_standard_reverts_unless_kept_and_is_written_when_kept",
        ),
        |dir| {
            let mut rig = composite_rig(dir);
            rig.boot(None);
            rig.frames(2);
            rig.keep();
            rig.until(600, "the scan", |r| {
                !r.shell().snapshot().systems.is_empty()
            });

            open_options(&mut rig, "TV standard");
            rig.focus("PAL (50 Hz)");
            rig.tap(&[Control::A]);
            assert!(matches!(rig.shell().guard().phase(), Phase::Pending(_)));
            assert_eq!(on_wire(&rig), (TV_625_288P, TvStandard::Pal));
            assert_eq!(
                last_ui_dest(&rig),
                Rect {
                    x: 40,
                    y: 24,
                    w: 640,
                    h: 240
                }
            );
            rig.frames(CONFIRM_WINDOW_FRAMES);
            assert_eq!(rig.shell().guard().phase(), Phase::Confirmed);
            assert_eq!(on_wire(&rig), (TV_525_240P, TvStandard::Ntsc));
            assert_eq!(rig.shell().config().tv(), "auto");
            dismiss_notices(&mut rig);
            assert_eq!(rig.screen(), "display");
            rig.focus("TV standard");
            rig.tap(&[Control::A]);
            rig.focus("PAL (50 Hz)");
            rig.tap(&[Control::A]);
            rig.keep();
            assert_eq!(rig.shell().guard().phase(), Phase::Confirmed);
            assert_eq!(on_wire(&rig), (TV_625_288P, TvStandard::Pal));
            let display = rig.shell().snapshot().display.clone().expect("display");
            assert_eq!(display.mode, "PAL 288p 50.08 Hz");
            assert_only_tv_modes(&rig);
            rig.finish();
            let global = rig.state_file("config/global.toml");
            assert!(global.contains("tv = \"pal\""), "{global}");
            let record = rig.state_file("video/guard.toml");
            assert!(record.contains("id = \"tv_pal\""), "{record}");

            rig.boot(None);
            rig.frames(2);
            assert_eq!(rig.shell().guard().phase(), Phase::Confirmed);
            assert_eq!(on_wire(&rig), (TV_625_288P, TvStandard::Pal));
            rig.finish();
        },
    );
}

#[test]
#[ignore = "tier 2"]
fn the_blind_recovery_on_a_tv_gives_ntsc() {
    in_child(
        &test_name(module_path!(), "the_blind_recovery_on_a_tv_gives_ntsc"),
        |dir| {
            let mut rig = composite_rig(dir);
            std::fs::create_dir_all(rig.paths.config()).expect("config");
            std::fs::write(
                rig.paths.config().join("global.toml"),
                "schema_version = 1\n\n[display]\ntv = \"pal\"\n",
            )
            .expect("global.toml");
            rig.boot(None);
            rig.frames(2);
            rig.keep();
            assert_eq!(on_wire(&rig), (TV_625_288P, TvStandard::Pal));
            rig.hold(&[Control::Select, Control::Start]);
            rig.until(HOLD_FRAMES, "the recovery", |r| {
                r.shell().guard().phase() == Phase::Safe(SafeReason::Recovery)
            });
            rig.release(&[Control::Select, Control::Start]);
            rig.frames(2);
            assert_eq!(on_wire(&rig), (TV_525_240P, TvStandard::Ntsc));
            assert_only_tv_modes(&rig);
            rig.finish();

            // The next boot starts in safe mode, on NTSC too.
            rig.boot(None);
            rig.frames(2);
            assert!(matches!(rig.shell().guard().phase(), Phase::Safe(_)));
            assert_eq!(on_wire(&rig), (TV_525_240P, TvStandard::Ntsc));
            assert_only_tv_modes(&rig);
            rig.finish();
        },
    );
}

#[test]
#[ignore = "tier 2"]
fn a_monitor_preset_has_no_effect_on_a_tv_and_says_so() {
    in_child(
        &test_name(
            module_path!(),
            "a_monitor_preset_has_no_effect_on_a_tv_and_says_so",
        ),
        |dir| {
            let mut rig = composite_rig(dir);
            std::fs::create_dir_all(rig.paths.config()).expect("config");
            std::fs::write(
                rig.paths.config().join("global.toml"),
                "schema_version = 1\n\n[display]\nmonitor = \"pc_31_120\"\n",
            )
            .expect("global.toml");
            rig.boot(None);
            rig.frames(2);
            assert_eq!(on_wire(&rig), (TV_525_240P, TvStandard::Ntsc));
            let notices: Vec<String> = rig
                .shell()
                .snapshot()
                .notices
                .iter()
                .map(|n| n.text.clone())
                .collect();
            assert!(
                notices
                    .iter()
                    .any(|n| n.contains("no effect on a TV output")),
                "{notices:?}"
            );
            rig.keep();
            assert_eq!(rig.shell().guard().phase(), Phase::Confirmed);
            rig.until(600, "the scan", |r| {
                !r.shell().snapshot().systems.is_empty()
            });
            dismiss_notices(&mut rig);
            rig.focus("Settings");
            rig.tap(&[Control::A]);
            rig.focus("Display");
            rig.tap(&[Control::A]);
            let rows = rig.shell().snapshot().settings.rows.clone();
            let monitor = rows.iter().find(|r| r.label == "Monitor profile");
            assert_eq!(
                monitor.and_then(|r| r.unavailable.as_deref()),
                Some(NOT_USED_ON_A_TV)
            );
            let refresh = rows.iter().find(|r| r.label == "Refresh policy");
            assert_eq!(
                refresh.and_then(|r| r.unavailable.as_deref()),
                Some(NOT_USED_ON_A_TV)
            );
            for label in ["TV standard", "TV picture width", "Picture style"] {
                let row = rows.iter().find(|r| r.label == label).expect(label);
                assert!(row.is_enabled(), "{label}");
            }
            assert_only_tv_modes(&rig);
            rig.finish();

            // A relaunch is a run of the same boot: no notice again.
            rig.boot(Some(NextRun {
                game: None,
                notice: None,
            }));
            rig.frames(2);
            let again = rig
                .shell()
                .snapshot()
                .notices
                .iter()
                .any(|n| n.text.contains("no effect on a TV output"));
            assert!(!again, "the notice shows once per boot");
            rig.finish();
        },
    );
}

#[test]
#[ignore = "tier 2"]
fn a_game_on_a_tv_runs_in_the_modes_of_the_standard() {
    in_children(
        &test_name(
            module_path!(),
            "a_game_on_a_tv_runs_in_the_modes_of_the_standard",
        ),
        2,
        |dir, stage| {
            let mut rig = composite_rig(dir);
            if stage == 0 {
                rig.boot(None);
                rig.frames(2);
                rig.keep();
                rig.until(600, "the scan", |r| {
                    !r.shell().snapshot().systems.is_empty()
                });
                rig.finish();
                return;
            }
            rig.boot(Some(game()));
            rig.frames(400);
            assert_eq!(rig.shell().guard().phase(), Phase::Confirmed);
            // 256x224, then 256x240 at frame 300: both fit 240p, so the
            // mode stays and only the placement moves.
            assert_eq!(on_wire(&rig), (TV_525_240P, TvStandard::Ntsc));
            let current = rig.shell().engine().and_then(|e| e.current()).cloned();
            let current = current.expect("a mode");
            assert_eq!(current.chosen.proposal.placement.dest.h, 240);
            let mut status = neptron_video::VideoStatusText::default();
            let shell = rig.shell();
            let status = match shell.engine() {
                Some(engine) => engine.video_status(&mut status).cloned(),
                None => None,
            }
            .expect("a status");
            assert_eq!(status.result, "exact", "60.099 Hz on 60.054 Hz is silent");
            assert_eq!(status.mode, "NTSC 240p 15.73 kHz 60.054 Hz");
            assert!(!status.alert);
            assert_only_tv_modes(&rig);
            rig.finish();
        },
    );
}
