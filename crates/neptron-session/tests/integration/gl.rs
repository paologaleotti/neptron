//! A GL core on the fake display and the reference core GL
//! (`docs/spec/hw-render.md`, sections 3 to 6): the launch and frame
//! order, the picture, the internal resolution of each display class, the
//! quick menu over a GPU picture, and the notices H1 to H4.
//!
//! With `NEPTRON_HW_PICTURES` set to a folder, the picture tests write the
//! screen of each main case there as a PPM file with a text file that
//! names the mode and the scale.

use neptron_backend::{ContextOffer, GlApi, GlVersion};
use neptron_libretro::{HwApi, HwRefusalReason};
use neptron_session::{EndReason, LaunchError, Machine, SessionError};
use neptron_testcore::{Size, gl_card_pixel};
use neptron_testkit::{CoreCall, DisplayOptions, RgbImage};
use neptron_types::{
    FrameOrigin, Modeline, MonitorKind, MonitorProfile, Orientation, PixelFormat, Rotation,
    ScanlineMode,
};
use neptron_video::ScanlineSettings;

use crate::harness::{Rig, Setup, test_core_scale};
use crate::with_script;

/// What a PC with Mesa `iris` offers.
const PC_OFFER: ContextOffer = ContextOffer {
    gles: Some(GlVersion::new(3, 2)),
    gl_core: Some(GlVersion::new(4, 6)),
    gl_compatibility: Some(GlVersion::new(4, 6)),
};

/// What a Raspberry Pi 3 with `vc4` offers.
const PI3_OFFER: ContextOffer = ContextOffer {
    gles: Some(GlVersion::new(2, 0)),
    gl_core: None,
    gl_compatibility: None,
};

/// A GLES 2 core with a 320x240 picture.
const GL_SCRIPT: &str = "neptron-testcore 5\nhw_render gles2 2 0\ngeometry 320 240 0\n";

/// 1920x1080 at 60 Hz, CEA-861 timing.
const MODE_1080P: Modeline = Modeline {
    pclock_hz: 148_500_000,
    hactive: 1_920,
    hsync_start: 2_008,
    hsync_end: 2_052,
    htotal: 2_200,
    vactive: 1_080,
    vsync_start: 1_084,
    vsync_end: 1_089,
    vtotal: 1_125,
    interlace: false,
    doublescan: false,
    hsync_positive: true,
    vsync_positive: true,
};

fn gl_setup(offer: ContextOffer) -> Setup {
    Setup {
        display: DisplayOptions {
            core_offer: Some(offer),
            ..DisplayOptions::default()
        },
        internal_scale: test_core_scale(240),
        ..Setup::default()
    }
}

fn panel_setup() -> Setup {
    Setup {
        monitor_profile: Some(MonitorProfile {
            id: "panel_1920x1080".to_owned(),
            kind: MonitorKind::FlatPanel {
                modes: vec![MODE_1080P],
            },
        }),
        ..gl_setup(PC_OFFER)
    }
}

/// The run frames of a test: enough for the debounce of a first mode.
const FRAMES: u64 = 30;

#[test]
fn a_gl_core_runs_in_the_launch_and_frame_order_of_the_spec() {
    with_script!(
        "a_gl_core_runs_in_the_launch_and_frame_order_of_the_spec",
        GL_SCRIPT,
        |dir| {
            let mut rig = Rig::launch(dir, gl_setup(PC_OFFER));
            rig.run(3);
            let calls = rig.display.core_calls();
            assert!(
                matches!(
                    calls[0].call,
                    CoreCall::Create {
                        max_width: 320,
                        max_height: 240,
                        result: Ok(_),
                        ..
                    }
                ),
                "{:?}",
                calls[0]
            );
            let settle = rig.session.counters().settle_frames;
            let pairs: Vec<_> = calls[1..].chunks(2).collect();
            assert_eq!(
                pairs.len() as u64,
                1 + settle + 3,
                "context_reset, settle, run"
            );
            for (i, pair) in pairs.iter().enumerate() {
                assert!(
                    matches!(
                        (&pair[0].call, &pair[1].call),
                        (CoreCall::Begin(Ok(())), CoreCall::End(Ok(_)))
                    ),
                    "pair {i}: {pair:?}"
                );
                assert_eq!(pair[0].presents_before, pair[1].presents_before);
                let loop_frame = (i as u64).saturating_sub(1 + settle);
                assert_eq!(
                    pair[0].presents_before, loop_frame,
                    "each core frame ends before its present"
                );
            }
            let frames = rig.display.frames();
            assert_eq!(frames.len(), 3);
            for frame in frames {
                assert_eq!(frame.scene.game, Some((320, 240, PixelFormat::Xrgb8888)));
                assert_eq!(frame.scene.gpu_origin, Some(FrameOrigin::TopLeft));
            }
            assert_eq!(rig.session.picture_scale(), 1);
            assert_eq!(rig.session.counters().frame_faults, 0);
        }
    );
}

#[test]
fn the_picture_of_a_gl_core_reads_back_as_its_card() {
    with_script!(
        "the_picture_of_a_gl_core_reads_back_as_its_card",
        GL_SCRIPT,
        |dir| {
            let mut rig = Rig::launch(dir, gl_setup(PC_OFFER));
            rig.run(2);
            assert!(
                rig.session.last_frame().is_none(),
                "the picture is on the GPU"
            );
            let frame = rig.session.read_picture().expect("a read back");
            assert_eq!((frame.width, frame.height), (320, 240));
            check_card(
                &frame,
                Size {
                    width: 320,
                    height: 240,
                },
            );
        }
    );
}

/// `frame` is the GL card of `size`, line 0 at the top.
fn check_card(frame: &neptron_types::Frame<'_>, size: Size) {
    for y in 0..u32::from(frame.height) {
        for x in 0..u32::from(frame.width) {
            let at = y as usize * frame.pitch + x as usize * 4;
            let got = u32::from_ne_bytes(frame.data[at..at + 4].try_into().expect("4 bytes"));
            assert_eq!(got, gl_card_pixel(size, 0, x, y), "({x}, {y})");
        }
    }
}

#[test]
fn a_crt_shows_a_gl_core_at_1x_in_its_native_mode() {
    with_script!(
        "a_crt_shows_a_gl_core_at_1x_in_its_native_mode",
        GL_SCRIPT,
        |dir| {
            let mut rig = Rig::launch(dir, gl_setup(PC_OFFER));
            rig.run(FRAMES);
            assert_eq!(rig.session.picture_scale(), 1);
            let source = rig.session.source();
            assert_eq!((source.width, source.height), (320, 240));
            let current = rig.session.video().current().expect("a mode");
            assert_eq!(current.read_back.vactive, 240, "the native lines");
            save(&rig, "crt_15khz_1x");
        }
    );
}

#[test]
fn a_pc_panel_takes_the_largest_whole_internal_resolution() {
    with_script!(
        "a_pc_panel_takes_the_largest_whole_internal_resolution",
        GL_SCRIPT,
        |dir| {
            let mut rig = Rig::launch(dir, panel_setup());
            rig.run(FRAMES);
            assert_eq!(rig.session.picture_scale(), 4, "4x of 240 is 960 of 1080");
            let source = rig.session.source();
            assert_eq!(
                (source.width, source.height),
                (320, 240),
                "the policy sees the native size"
            );
            let frame = rig.display.frames().pop().expect("a present");
            assert_eq!(frame.scene.game, Some((1_280, 960, PixelFormat::Xrgb8888)));
            assert_eq!(
                frame.scene.crop.h, 960,
                "the crop is 4 times the native crop"
            );
            let picture = rig.session.read_picture().expect("a read back");
            check_card(
                &picture,
                Size {
                    width: 1_280,
                    height: 960,
                },
            );
            assert_eq!(rig.session.counters().unscaled_frames, 0);
            save(&rig, "panel_1080p_4x");
        }
    );
}

#[test]
fn scanlines_on_a_panel_take_1x() {
    with_script!("scanlines_on_a_panel_take_1x", GL_SCRIPT, |dir| {
        let setup = Setup {
            scanlines: ScanlineSettings {
                mode: ScanlineMode::On,
                strength_percent: 50,
            },
            ..panel_setup()
        };
        let mut rig = Rig::launch(dir, setup);
        rig.run(FRAMES);
        assert_eq!(rig.session.picture_scale(), 1);
        let frame = rig.display.frames().pop().expect("a present");
        assert_eq!(frame.scene.game, Some((320, 240, PixelFormat::Xrgb8888)));
        save(&rig, "panel_1080p_scanlines_1x");
    });
}

#[test]
fn a_raspberry_pi_panel_takes_1x() {
    with_script!("a_raspberry_pi_panel_takes_1x", GL_SCRIPT, |dir| {
        let setup = Setup {
            machine: Machine::RaspberryPi,
            ..panel_setup()
        };
        let mut rig = Rig::launch(dir, setup);
        rig.run(4);
        assert_eq!(rig.session.picture_scale(), 1);
    });
}

#[test]
fn scanlines_turned_on_in_a_game_set_the_option_to_1x() {
    with_script!(
        "scanlines_turned_on_in_a_game_set_the_option_to_1x",
        GL_SCRIPT,
        |dir| {
            let mut rig = Rig::launch(dir, panel_setup());
            rig.run(4);
            rig.session.set_scanlines(ScanlineSettings {
                mode: ScanlineMode::On,
                strength_percent: 50,
            });
            let value = rig
                .session
                .host()
                .options()
                .iter()
                .find(|o| o.key() == "neptron_testcore_scale")
                .map(|o| o.value().to_owned());
            assert_eq!(value.as_deref(), Some("1"));
            rig.run(2);
            assert_eq!(
                rig.session.picture_scale(),
                4,
                "the test core takes the option at its next launch, and its frames stay 4x"
            );
        }
    );
}

#[test]
fn a_turned_screen_turns_the_gpu_picture() {
    with_script!("a_turned_screen_turns_the_gpu_picture", GL_SCRIPT, |dir| {
        let mut rig = Rig::launch(dir, gl_setup(PC_OFFER));
        rig.session.video_mut().set_orientation(Orientation {
            screen: Rotation::R90,
            game_override: None,
        });
        rig.run(FRAMES);
        let frame = rig.display.frames().pop().expect("a present");
        assert_eq!(frame.scene.game, Some((320, 240, PixelFormat::Xrgb8888)));
        assert_ne!(frame.scene.rotation, Rotation::R0, "the picture turns");
        save(&rig, "crt_15khz_turned_screen_1x");
    });
}

#[test]
fn the_quick_menu_shows_the_gpu_picture_under_the_ui() {
    with_script!(
        "the_quick_menu_shows_the_gpu_picture_under_the_ui",
        GL_SCRIPT,
        |dir| {
            let mut rig = Rig::launch(dir, panel_setup());
            rig.run(FRAMES);
            rig.open_menu();
            let begins = |rig: &Rig| {
                rig.display
                    .core_calls()
                    .iter()
                    .filter(|r| matches!(r.call, CoreCall::Begin(_)))
                    .count()
            };
            let before = begins(&rig);
            let ui_bytes = ui_canvas();
            let ui = neptron_types::Frame {
                data: &ui_bytes,
                width: 320,
                height: 240,
                pitch: 320 * 4,
                format: PixelFormat::Xrgb8888,
            };
            for _ in 0..3 {
                rig.session.run_frame(Some(ui)).expect("a menu frame");
            }
            assert_eq!(begins(&rig), before, "the core does not run under the menu");
            let frame = rig.display.frames().pop().expect("a present");
            assert_eq!(frame.scene.game, Some((1_280, 960, PixelFormat::Xrgb8888)));
            assert_eq!(frame.scene.ui, Some((320, 240)));
            save(&rig, "panel_1080p_quick_menu_4x");
        }
    );
}

/// A UI canvas with a dark frame and a clear middle, as a dialog over the
/// game covers part of it.
fn ui_canvas() -> Vec<u8> {
    let mut bytes = vec![0_u8; 320 * 240 * 4];
    for y in 0..240_usize {
        for x in 0..320_usize {
            let inside = (80..240).contains(&x) && (60..180).contains(&y);
            let px: u32 = if inside { 0x0020_4080 } else { 0x0010_1010 };
            bytes[(y * 320 + x) * 4..][..4].copy_from_slice(&px.to_ne_bytes());
        }
    }
    bytes
}

#[test]
fn a_type_the_display_lacks_refuses_the_game_with_the_request() {
    let script = "neptron-testcore 5\nhw_render glcore 4 6\ngeometry 320 240 0\n";
    with_script!(
        "a_type_the_display_lacks_refuses_the_game_with_the_request",
        script,
        |dir| {
            let failed = Rig::start(dir, gl_setup(PI3_OFFER))
                .err()
                .expect("a refusal");
            let LaunchError::Refused { hw_refusal, .. } = failed.error else {
                panic!("{:?}", failed.error);
            };
            let refusal = hw_refusal.expect("the request");
            assert_eq!(refusal.api, HwApi::OpenGl);
            assert_eq!((refusal.major, refusal.minor), (4, 6));
            assert_eq!(refusal.reason, HwRefusalReason::NotOffered);
        }
    );
}

#[test]
fn a_context_the_display_cannot_make_ends_the_launch() {
    with_script!(
        "a_context_the_display_cannot_make_ends_the_launch",
        GL_SCRIPT,
        |dir| {
            let setup = Setup {
                core_create_fault: Some("out of memory".to_owned()),
                ..gl_setup(PC_OFFER)
            };
            let failed = Rig::start(dir, setup).err().expect("a failed launch");
            let LaunchError::Graphics { request, .. } = failed.error else {
                panic!("{:?}", failed.error);
            };
            assert_eq!(request.api, GlApi::Gles);
            assert!(failed.display.frames().is_empty(), "no picture");
        }
    );
}

#[test]
fn a_gl_core_with_no_picture_ends_after_ten_seconds() {
    let mut script = GL_SCRIPT.to_owned();
    for frame in 0..=700 {
        script.push_str(&format!("at {frame} hw_none\n"));
    }
    with_script!(
        "a_gl_core_with_no_picture_ends_after_ten_seconds",
        &script,
        |dir| {
            let mut rig = Rig::launch(dir, gl_setup(PC_OFFER));
            let mut ended = None;
            for _ in 0..700 {
                let report = rig.session.run_frame(None).expect("a frame");
                if report.ended.is_some() {
                    ended = report.ended;
                    break;
                }
            }
            assert_eq!(ended, Some(EndReason::NoPicture));
            let counters = rig.session.counters();
            assert_eq!(
                counters.settle_frames + counters.core_frames,
                600,
                "10 s of core frames at 60 Hz"
            );
        }
    );
}

#[test]
fn a_lost_context_is_made_again_once_and_a_second_loss_ends_the_game() {
    with_script!(
        "a_lost_context_is_made_again_once_and_a_second_loss_ends_the_game",
        GL_SCRIPT,
        |dir| {
            let setup = Setup {
                core_losses: vec![20, 40],
                ..gl_setup(PC_OFFER)
            };
            let mut rig = Rig::launch(dir, setup);
            let mut error = None;
            for _ in 0..60 {
                if let Err(e) = rig.session.run_frame(None) {
                    error = Some(e);
                    break;
                }
            }
            let creates = rig
                .display
                .core_calls()
                .iter()
                .filter(|r| matches!(r.call, CoreCall::Create { result: Ok(_), .. }))
                .count();
            assert_eq!(creates, 2, "one new context after the first loss");
            assert!(
                matches!(error, Some(SessionError::ContextLost { losses: 2 })),
                "{error:?}"
            );
            assert_eq!(rig.session.counters().context_losses, 2);
        }
    );
}

#[test]
fn the_end_of_a_gl_game_destroys_its_context_inside_it() {
    with_script!(
        "the_end_of_a_gl_game_destroys_its_context_inside_it",
        GL_SCRIPT,
        |dir| {
            let mut rig = Rig::launch(dir, gl_setup(PC_OFFER));
            rig.run(2);
            let display = rig.display.clone();
            drop(rig.session.quit());
            let calls = display.core_calls();
            assert!(
                matches!(calls.last().map(|r| &r.call), Some(CoreCall::Begin(Ok(())))),
                "context_destroy and the unload run inside the context: {:?}",
                calls.last()
            );
        }
    );
}

/// Writes the screen of `rig` and its mode and scale to the folder of
/// `NEPTRON_HW_PICTURES`, when it is set.
fn save(rig: &Rig, name: &str) {
    let Some(dir) = std::env::var_os("NEPTRON_HW_PICTURES") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).expect("the picture folder");
    let screen: RgbImage = rig.display.screen().expect("a screen");
    let mut ppm = format!("P6\n{} {}\n255\n", screen.width(), screen.height()).into_bytes();
    ppm.extend_from_slice(screen.bytes());
    std::fs::write(dir.join(format!("{name}.ppm")), ppm).expect("write the picture");
    let current = rig.session.video().current().expect("a mode");
    let mode = current.read_back;
    let frame = rig.display.frames().pop().expect("a present");
    let text = format!(
        "mode {}x{}{} at {} mHz field rate\nscale {}x\nsource {}x{}\npicture {:?}\ncrop {:?}\ndest {:?}\nfilter {:?}\nrotation {:?}\n",
        mode.hactive,
        mode.vactive,
        if mode.interlace { "i" } else { "p" },
        mode.field_rate_mhz(),
        rig.session.picture_scale(),
        current.source.width,
        current.source.height,
        frame.scene.game,
        frame.scene.crop,
        frame.scene.dest,
        frame.scene.filter,
        frame.scene.rotation,
    );
    std::fs::write(dir.join(format!("{name}.txt")), text).expect("write the notes");
}
