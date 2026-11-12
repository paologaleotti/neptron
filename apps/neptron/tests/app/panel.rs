//! A fake 1080p HDMI panel: the boot picks the panel profile in its
//! native mode, the menu sits at one whole scale, a game gets a whole
//! vertical scale with black borders, and a change to a CRT preset is
//! taken back with no answer.

use std::path::Path;

use neptron::NextRun;
use neptron_types::{Control, Filter, Rect};
use neptron_ui::snapshot::SettingValue;
use neptron_video::Phase;

use crate::rig::{Rig, in_child, test_name};

/// Frames of 10 s at 60 Hz, the confirm window, and a margin.
const CONFIRM_WINDOW_FRAMES: u32 = 660;

/// The EDID of a 1080p HDMI monitor that also lists 1080p50, 720p and
/// 576p50, made from the specifications.
const PANEL_1080P: &[u8] = include_bytes!("../fixtures/edid/panel_1080p.bin");

/// A sysfs tree under `root` with one GPU and an HDMI connector that
/// reports `edid`.
fn sysfs_with_panel(root: &Path, edid: &[u8]) {
    let drm = root.join("class/drm");
    let card = drm.join("card0/device");
    std::fs::create_dir_all(&card).expect("the card");
    std::fs::write(card.join("uevent"), "DRIVER=fake\n").expect("the uevent");
    let connector = drm.join("card0-HDMI-A-1");
    std::fs::create_dir_all(&connector).expect("the connector");
    std::fs::write(connector.join("status"), "connected\n").expect("the status");
    std::fs::write(connector.join("edid"), edid).expect("the EDID");
}

/// Every pixel outside `inside` is black, and some pixel inside is not.
fn assert_black_outside(screen: &neptron_testkit::RgbImage, inside: Rect) {
    let mut lit = false;
    for y in 0..screen.height() {
        for x in 0..screen.width() {
            let pixel = screen.pixel(x, y).expect("a pixel");
            let within = i64::from(x) >= i64::from(inside.x)
                && i64::from(x) < i64::from(inside.x) + i64::from(inside.w)
                && i64::from(y) >= i64::from(inside.y)
                && i64::from(y) < i64::from(inside.y) + i64::from(inside.h);
            if within {
                lit |= pixel != [0, 0, 0];
            } else {
                assert_eq!(pixel, [0, 0, 0], "pixel ({x}, {y}) outside {inside:?}");
            }
        }
    }
    assert!(lit, "nothing inside {inside:?}");
}

fn rect(x: i32, y: i32, w: u32, h: u32) -> Rect {
    Rect { x, y, w, h }
}

#[test]
#[ignore = "tier 2"]
fn a_1080p_panel_shows_the_menu_and_a_game_at_a_whole_scale() {
    in_child(
        &test_name(
            module_path!(),
            "a_1080p_panel_shows_the_menu_and_a_game_at_a_whole_scale",
        ),
        |dir| {
            let mut rig = Rig::new(dir);
            sysfs_with_panel(&rig.paths.sysfs, PANEL_1080P);
            rig.output = "HDMI-A-1".to_owned();

            // A new boot on the panel: its native mode, under the prompt.
            rig.boot(None);
            rig.frames(2);
            assert!(matches!(rig.shell().guard().phase(), Phase::Pending(_)));
            let mode = rig.display.active_mode().expect("a mode");
            assert_eq!(
                (mode.hactive, mode.vactive, mode.field_rate_mhz()),
                (1920, 1080, 60_000)
            );
            // The 320x240 menu at 4x, centred: 1920 / 320 = 6 and
            // 1080 / 240 = 4, so 4; (1920 - 1280) / 2 = 320 and
            // (1080 - 960) / 2 = 60.
            let last = rig.display.frames().last().expect("a frame").scene;
            assert_eq!(last.ui_dest, rect(320, 60, 1280, 960));
            let screen = rig.display.screen().expect("the screen");
            assert_eq!((screen.width(), screen.height()), (1920, 1080));
            assert_black_outside(&screen, rect(320, 60, 1280, 960));
            rig.keep();
            assert_eq!(rig.shell().guard().phase(), Phase::Confirmed);

            // The display panel names the panel and its native mode.
            rig.until(600, "the scan", |r| {
                !r.shell().snapshot().systems.is_empty()
            });
            rig.focus("Display");
            rig.tap(&[Control::A]);
            let monitor = rig
                .shell()
                .snapshot()
                .settings
                .rows
                .iter()
                .find(|r| r.label == "Monitor profile")
                .map(|r| r.value.clone());
            let Some(SettingValue::Choice { options, index }) = monitor else {
                panic!("the monitor row: {monitor:?}");
            };
            assert_eq!(options[index].label, "auto");
            assert_eq!(options[index].detail, "NPT FHD 24  1920x1080 60 Hz");
            let display = rig.shell().snapshot().display.clone().expect("display");
            assert_eq!(display.output, "HDMI-A-1");
            assert_eq!(display.mode, "1920x1080p 60.00 Hz");

            // A CRT preset with no answer goes back to the panel.
            rig.focus("Monitor profile");
            rig.tap(&[Control::A]);
            assert_eq!(rig.screen(), "options");
            rig.focus("pc_31_120");
            rig.tap(&[Control::A]);
            assert!(matches!(rig.shell().guard().phase(), Phase::Pending(_)));
            let crt = rig.display.active_mode().expect("a mode");
            assert!(crt.line_rate_mhz() < 40_000_000, "a 31 kHz mode: {crt:?}");
            rig.frames(CONFIRM_WINDOW_FRAMES);
            assert_eq!(rig.shell().guard().phase(), Phase::Confirmed);
            assert_eq!(rig.shell().config().text("display.monitor"), "auto");
            let back = rig.display.active_mode().expect("a mode");
            assert_eq!((back.hactive, back.vactive), (1920, 1080));
            rig.finish();
            let record = rig.state_file("video/guard.toml");
            assert!(record.contains("id = \"panel_npt_1080\""), "{record}");
            assert!(record.contains("output = \"HDMI-A-1\""), "{record}");

            // The game: 256x224 at 4:3. 1080 / 224 = 4 lines each, 896
            // lines; the width at 4:3 is 896 * 1333 / 1000 = 1194.4, so
            // 1194, a fraction of 256: sharp bilinear. (1920 - 1194) / 2 =
            // 363 and (1080 - 896) / 2 = 92.
            rig.boot(Some(NextRun {
                game: Some(neptron::relaunch::GameRef {
                    system: "nes".to_owned(),
                    path: "nes/Test Game.nes".to_owned(),
                }),
                notice: None,
            }));
            assert!(rig.shell().in_game());
            assert_eq!(rig.shell().guard().phase(), Phase::Confirmed);
            rig.frames(60);
            let switches = rig.display.mode_switches().len();
            let scene = rig.display.frames().last().expect("a frame").scene;
            assert_eq!(scene.dest, rect(363, 92, 1194, 896));
            let screen = rig.display.screen().expect("the screen");
            assert_black_outside(&screen, scene.dest);
            assert_eq!(scene.filter, Filter::SharpBilinear);
            let mode = rig.display.active_mode().expect("a mode");
            assert_eq!((mode.hactive, mode.vactive), (1920, 1080));
            let video = rig.shell().snapshot().video.clone().expect("the status");
            assert_eq!(video.mode, "1920x1080p 67.50 kHz 60.000 Hz  panel");
            assert_eq!(video.result, "sharp");
            assert!(!video.alert);
            assert!(
                rig.shell().snapshot().notices.is_empty(),
                "{:?}",
                rig.shell().snapshot().notices
            );

            // At core frame 300 the game is 256x240: 4 lines each, 960
            // lines, 1280 wide, 5 times 256, so nearest on both axes, at
            // (320, 60). The panel keeps its mode.
            rig.frames(300);
            let scene = rig.display.frames().last().expect("a frame").scene;
            assert_eq!(scene.dest, rect(320, 60, 1280, 960));
            assert_eq!(scene.filter, Filter::Nearest);
            assert_eq!(
                rig.display.mode_switches().len(),
                switches,
                "no modeset for the size change"
            );
            assert_eq!(
                rig.shell().snapshot().video.clone().expect("status").result,
                "exact"
            );
            rig.finish();
        },
    );
}
