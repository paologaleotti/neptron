//! The scripted session: launch, play, the quick menu, double refresh and
//! exit, on the fakes with the test core.

use neptron_session::{EndReason, SessionError};
use neptron_types::{Control, Frame, PixelFormat};
use neptron_video::{
    PictureStyle, PolicySettings, RefreshPolicy, VideoStatusText, scanline_settings,
};

use crate::harness::{Rig, Setup, TestSession, expected, frame_hash};
use crate::with_script;

const SCRIPT: &str = "\
neptron-testcore 1
pixel_format rgb565
geometry 256 224 1333
max_size 320 240
timing 60099 32040
save_ram 64
";

/// The RetroPad bit of each button, as the test core reads it.
const START: u16 = 1 << 3;
const A: u16 = 1 << 8;

/// The UI canvas: 320x240 XRGB8888.
static CANVAS: [u8; 320 * 240 * 4] = [0; 320 * 240 * 4];

fn canvas() -> Frame<'static> {
    Frame {
        data: &CANVAS,
        width: 320,
        height: 240,
        pitch: 320 * 4,
        format: PixelFormat::Xrgb8888,
    }
}

/// The last game frame shows test core frame `n` with `buttons`.
fn assert_frame(session: &TestSession, n: u64, buttons: u16) {
    let frame = session.last_frame().expect("a game frame");
    assert_eq!(
        frame_hash(&frame),
        expected(&frame, n, buttons),
        "core frame {n} with buttons {buttons:#x}"
    );
}

#[test]
fn the_core_runs_one_frame_per_flip() {
    with_script!("the_core_runs_one_frame_per_flip", SCRIPT, |dir| {
        let mut rig = Rig::launch(dir, Setup::default());
        rig.run(600);
        let counters = *rig.session.counters();
        assert_eq!(counters.core_frames, 600);
        assert_eq!(counters.flips, 600);
        assert_eq!(counters.flipless_frames, 0);
        assert_eq!(rig.display.mode_calls().len(), 1);
        assert!(rig.display.dropped_frames().is_empty());
        assert_eq!(rig.display.repeated_vblanks(), 0);
        assert_frame(&rig.session, rig.core_frame(), 0);
        let mode = rig.display.active_mode().unwrap();
        assert_eq!(rig.session.run_rate_mhz(), Some(mode.field_rate_mhz()));
        assert!(
            rig.audio.underruns().is_empty(),
            "{:?}",
            rig.audio.underruns()
        );
        assert!(rig.audio.overruns().is_empty());
        let level = rig.session.audio_stats().unwrap().level_frames;
        assert!((1_382..=1_690).contains(&level), "queue level {level}");
    });
}

#[test]
fn the_core_reads_the_input_of_its_frame() {
    with_script!("the_core_reads_the_input_of_its_frame", SCRIPT, |dir| {
        let mut rig = Rig::launch(dir, Setup::default());
        rig.run(5);
        rig.press(&[Control::A]);
        rig.run(1);
        assert_frame(&rig.session, rig.core_frame(), A);
        rig.release(&[Control::A]);
        rig.run(1);
        assert_frame(&rig.session, rig.core_frame(), 0);
    });
}

#[test]
fn the_quick_menu_pauses_the_core_and_is_never_a_toggle() {
    with_script!(
        "the_quick_menu_pauses_the_core_and_is_never_a_toggle",
        SCRIPT,
        |dir| {
            let mut rig = Rig::launch(dir, Setup::default());
            rig.run(10);
            let frozen = rig.core_frame();
            rig.press(&[Control::Select, Control::Start]);
            let report = rig.session.run_frame(Some(canvas())).unwrap();
            assert!(report.menu_opened);
            assert!(!report.ran_core);
            assert_eq!(report.flips, 1);
            assert!(rig.session.paused());
            let audio_frames = rig.session.audio_stats().unwrap().frames;

            for _ in 0..60 {
                let report = rig.session.run_frame(Some(canvas())).unwrap();
                assert!(!report.ran_core);
                assert_eq!(report.flips, 1, "the display keeps flipping");
            }
            assert_eq!(rig.core_frame(), frozen, "the core is paused");
            assert_frame(&rig.session, frozen, 0);
            assert_eq!(
                rig.session.audio_stats().unwrap().frames,
                audio_frames,
                "no audio frame while paused"
            );
            let last = rig.display.frames().last().unwrap().clone();
            assert_eq!(last.scene.ui, Some((320, 240)), "the menu is drawn");
            assert!(last.scene.game.is_some(), "over the frozen frame");

            // The combination again opens nothing and closes nothing.
            rig.release(&[Control::Select, Control::Start]);
            rig.session.run_frame(Some(canvas())).unwrap();
            rig.press(&[Control::Select, Control::Start]);
            let report = rig.session.run_frame(Some(canvas())).unwrap();
            assert!(!report.menu_opened);
            assert!(rig.session.paused());
            assert_eq!(rig.display.mode_calls().len(), 1, "no mode switch");

            // Resume with Select, Start and A held: the core sees them up
            // until each one is released.
            rig.press(&[Control::A]);
            rig.session.run_frame(Some(canvas())).unwrap();
            rig.session.resume();
            assert!(!rig.session.paused());
            let report = rig.session.run_frame(None).unwrap();
            assert!(report.ran_core);
            assert!(
                !report.menu_opened,
                "a held combination does not open it again"
            );
            assert_frame(&rig.session, frozen + 1, 0);
            rig.release(&[Control::Select, Control::Start, Control::A]);
            rig.run(1);
            assert_frame(&rig.session, frozen + 2, 0);
            rig.press(&[Control::A]);
            rig.run(1);
            assert_frame(&rig.session, frozen + 3, A);
            assert!(rig.audio.overruns().is_empty());
        }
    );
}

#[test]
fn the_start_hold_opens_the_menu_and_the_core_sees_start_released() {
    with_script!(
        "the_start_hold_opens_the_menu_and_the_core_sees_start_released",
        SCRIPT,
        |dir| {
            let setup = Setup {
                start_hold_ns: 2_000_000_000,
                ..Setup::default()
            };
            let mut rig = Rig::launch(dir, setup);
            rig.run(5);
            rig.press(&[Control::Start]);
            let mut frames = 0;
            loop {
                let report = rig.session.run_frame(None).unwrap();
                frames += 1;
                if report.menu_opened {
                    break;
                }
                assert_frame(&rig.session, rig.core_frame(), START);
                assert!(frames < 200, "the hold never fired");
            }
            assert!((119..=122).contains(&frames), "{frames} frames for 2 s");
            rig.session.resume();
            rig.run(1);
            assert_frame(&rig.session, rig.core_frame(), 0);
            rig.release(&[Control::Start]);
            rig.run(1);
            rig.press(&[Control::Start]);
            rig.run(1);
            assert_frame(&rig.session, rig.core_frame(), START);
        }
    );
}

#[test]
fn the_blind_recovery_is_reported_also_under_the_menu() {
    with_script!(
        "the_blind_recovery_is_reported_also_under_the_menu",
        SCRIPT,
        |dir| {
            let mut rig = Rig::launch(dir, Setup::default());
            rig.run(5);
            rig.press(&[Control::Select, Control::Start]);
            let mut recovery = None;
            for frame in 0..400 {
                let report = rig.session.run_frame(None).unwrap();
                assert_eq!(report.menu_opened, frame == 0);
                if report.blind_recovery.is_some() {
                    recovery = Some(frame);
                    assert_eq!(report.blind_recovery, Some(0));
                    break;
                }
            }
            let frame = recovery.expect("the recovery hold fired");
            assert!((299..=302).contains(&frame), "after 5 s: frame {frame}");
            assert!(rig.session.paused());
        }
    );
}

#[test]
fn a_core_shutdown_ends_the_session() {
    let script = format!("{SCRIPT}at 40 shutdown\n");
    with_script!("a_core_shutdown_ends_the_session", &script, |dir| {
        let mut rig = Rig::launch(dir, Setup::default());
        let mut ended = None;
        for _ in 0..100 {
            let report = rig.session.run_frame(None).unwrap();
            if report.ended.is_some() {
                ended = report.ended;
                break;
            }
        }
        assert_eq!(ended, Some(EndReason::CoreShutdown));
        assert_eq!(rig.core_frame(), 40);
        assert!(rig.session.ended());
        assert!(matches!(
            rig.session.run_frame(None),
            Err(SessionError::Ended)
        ));
        let core_frames = rig.session.counters().core_frames;
        let exit = rig.session.quit();
        assert_eq!(exit.counters.core_frames, core_frames);
    });
}

#[test]
fn double_refresh_presents_each_frame_twice_at_the_game_rate() {
    with_script!(
        "double_refresh_presents_each_frame_twice_at_the_game_rate",
        SCRIPT,
        |dir| {
            let setup = Setup {
                monitor: "pc_31_120",
                policy: PolicySettings {
                    style: PictureStyle::Native,
                    refresh: RefreshPolicy::ExactX2,
                    ..PolicySettings::default()
                },
                ..Setup::default()
            };
            let mut rig = Rig::launch(dir, setup);
            for _ in 0..600 {
                let report = rig.session.run_frame(None).unwrap();
                assert_eq!(report.flips, 2);
            }
            let counters = *rig.session.counters();
            assert_eq!(counters.core_frames, 600);
            assert_eq!(counters.flips, 1_200);
            let mode = rig.display.active_mode().unwrap();
            let field = mode.field_rate_mhz();
            assert!(field.abs_diff(2 * 60_099) < 10, "field rate {field}");
            assert_eq!(rig.session.run_rate_mhz(), Some(field / 2));
            assert_eq!(rig.display.repeated_vblanks(), 0);
            let hashes = rig.display.frame_hashes();
            assert!(hashes.chunks(2).all(|pair| pair[0] == pair[1]));
            assert!(rig.audio.underruns().is_empty());
            assert!(rig.audio.overruns().is_empty());
            let mut status = VideoStatusText::default();
            let status = rig.session.video_status(&mut status).unwrap();
            assert!(status.result.contains("double refresh"), "{status:?}");
        }
    );
}

#[test]
fn the_engine_gets_the_scanline_settings_and_the_status_is_the_read_back() {
    with_script!(
        "the_engine_gets_the_scanline_settings_and_the_status_is_the_read_back",
        SCRIPT,
        |dir| {
            let scanlines = scanline_settings("on", 50).unwrap();
            let setup = Setup {
                scanlines,
                ..Setup::default()
            };
            let mut rig = Rig::launch(dir, setup);
            let mut status = VideoStatusText::default();
            assert!(
                rig.session.video_status(&mut status).is_none(),
                "no mode yet"
            );
            rig.run(1);
            assert_eq!(rig.session.video().odren().scanlines(), scanlines);
            let status = rig.session.video_status(&mut status).unwrap();
            assert!(status.game.starts_with("256x224"), "{status:?}");
            let mode = rig.display.active_mode().unwrap();
            let size = format!("{}x{}", mode.hactive, mode.vactive);
            assert!(status.mode.starts_with(&size), "{status:?}");
        }
    );
}

#[test]
fn core_options_from_the_settings_are_set_and_unknown_ones_counted() {
    with_script!(
        "core_options_from_the_settings_are_set_and_unknown_ones_counted",
        SCRIPT,
        |dir| {
            let setup = Setup {
                core_options: vec![
                    ("neptron_testcore_mode".to_owned(), "beta".to_owned()),
                    ("neptron_testcore_level".to_owned(), "9".to_owned()),
                    ("no_such_key".to_owned(), "x".to_owned()),
                ],
                ..Setup::default()
            };
            let rig = Rig::launch(dir, setup);
            assert_eq!(rig.session.counters().refused_options, 2);
            let mode = rig
                .session
                .host()
                .options()
                .iter()
                .find(|o| o.key() == "neptron_testcore_mode")
                .unwrap();
            assert_eq!(mode.value(), "beta");
        }
    );
}

#[test]
fn a_timing_change_makes_a_new_pipeline_and_a_debounced_modeset() {
    let script = format!("{SCRIPT}at 300 av_info 256 224 1333 320 240 50007 32040\n");
    with_script!(
        "a_timing_change_makes_a_new_pipeline_and_a_debounced_modeset",
        &script,
        |dir| {
            let mut rig = Rig::launch(dir, Setup::default());
            rig.run(900);
            assert_eq!(rig.display.mode_calls().len(), 2);
            let mode = rig.display.active_mode().unwrap();
            assert!(mode.field_rate_mhz().abs_diff(50_007) < 10);
            assert_eq!(rig.session.run_rate_mhz(), Some(mode.field_rate_mhz()));
            let counters = rig.session.counters();
            assert_eq!(counters.refused_timings, 0);
            assert!(!counters.audio_lost);
            assert_eq!(counters.flips, counters.core_frames);
            assert!(rig.audio.overruns().is_empty());
            let stats = rig.session.audio_stats().unwrap();
            assert!(stats.frames < 900, "a new pipeline counts from the change");
        }
    );
}
