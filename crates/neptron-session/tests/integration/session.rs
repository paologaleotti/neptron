//! The scripted session: launch, play, the quick menu, save RAM, double
//! refresh and exit, on the fakes with the test core.

use neptron_backend::Clock;
use neptron_session::{EndReason, Flush, SAVE_RAM_FLUSH_INTERVAL_NS, SaveError, SessionError};
use neptron_testkit::{DisplayOptions, Render};
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
            rig.press(&[Control::Select, Control::Start]);
            let report = rig.session.run_frame(None).unwrap();
            assert!(!report.menu_opened, "the menu opens at the release");
            assert!(report.ran_core);
            let frozen = rig.core_frame();
            rig.release(&[Control::Select, Control::Start]);
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
            rig.press(&[Control::Select, Control::Start]);
            let report = rig.session.run_frame(Some(canvas())).unwrap();
            assert!(!report.menu_opened);
            rig.release(&[Control::Select, Control::Start]);
            let report = rig.session.run_frame(Some(canvas())).unwrap();
            assert!(!report.menu_opened);
            rig.press(&[Control::Select, Control::Start]);
            rig.session.run_frame(Some(canvas())).unwrap();
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
            assert!(!report.menu_opened);
            assert_frame(&rig.session, frozen + 1, 0);
            rig.release(&[Control::Select, Control::Start, Control::A]);
            let report = rig.session.run_frame(None).unwrap();
            assert!(
                !report.menu_opened,
                "a combination held across the resume does not open it again"
            );
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
            rig.run(1);
            rig.release(&[Control::Select, Control::Start]);
            assert!(rig.session.run_frame(None).unwrap().menu_opened);
            rig.press(&[Control::Select, Control::Start]);
            let mut recovery = None;
            for frame in 0..400 {
                let report = rig.session.run_frame(None).unwrap();
                assert!(!report.menu_opened);
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

const SAVE_SCRIPT: &str = "\
neptron-testcore 1
pixel_format rgb565
geometry 256 224 1333
timing 60000 48000
save_ram 64
at 20 sram_write 0 42
";

fn write_old_save(dir: &std::path::Path) -> std::path::PathBuf {
    let saves = dir.join("saves");
    std::fs::create_dir_all(&saves).unwrap();
    let path = saves.join("game.srm");
    std::fs::write(&path, [7_u8; 64]).unwrap();
    path
}

#[test]
fn the_save_ram_is_loaded_at_launch_and_written_at_exit_with_a_backup() {
    with_script!(
        "the_save_ram_is_loaded_at_launch_and_written_at_exit_with_a_backup",
        SAVE_SCRIPT,
        |dir| {
            let path = write_old_save(dir);
            let mut rig = Rig::launch(dir, Setup::default());
            rig.run(30);
            assert_eq!(rig.session.save_stats().unwrap().sent, 0);
            let exit = rig.session.quit();
            assert_eq!(exit.save, Flush::Written);
            assert_eq!(exit.save_stats.written, 1);
            assert_eq!(exit.save_stats.failures, 0);
            let mut expected = [7_u8; 64];
            expected[0] = 42;
            assert_eq!(std::fs::read(&path).unwrap(), expected);
            assert_eq!(
                std::fs::read(dir.join("saves/game.srm.bak")).unwrap(),
                [7; 64]
            );
        }
    );
}

/// A save file longer than the core's save RAM: the core gets the first
/// 64 bytes, and the write keeps the other 16 after them.
#[test]
fn a_save_file_longer_than_the_save_ram_is_never_cut() {
    with_script!(
        "a_save_file_longer_than_the_save_ram_is_never_cut",
        SAVE_SCRIPT,
        |dir| {
            let saves = dir.join("saves");
            std::fs::create_dir_all(&saves).unwrap();
            let old: Vec<u8> = (0..80).collect();
            std::fs::write(saves.join("game.srm"), &old).unwrap();
            let mut rig = Rig::launch(dir, Setup::default());
            assert_eq!(rig.session.save_stats().unwrap().kept_tail_bytes, 16);
            rig.run(30);
            let exit = rig.session.quit();
            assert_eq!(exit.save, Flush::Written);
            let mut expected = old.clone();
            expected[0] = 42;
            assert_eq!(std::fs::read(saves.join("game.srm")).unwrap(), expected);
            assert_eq!(std::fs::read(saves.join("game.srm.bak")).unwrap(), old);
        }
    );
}

#[test]
fn a_failed_write_at_exit_is_reported_and_the_old_file_kept() {
    with_script!(
        "a_failed_write_at_exit_is_reported_and_the_old_file_kept",
        SAVE_SCRIPT,
        |dir| {
            let path = write_old_save(dir);
            let mut rig = Rig::launch(dir, Setup::default());
            rig.run(30);
            // A folder in place of the backup makes the write fail before
            // the new file replaces the old one.
            let bak = dir.join("saves/game.srm.bak");
            std::fs::create_dir(&bak).unwrap();
            std::fs::write(bak.join("inside"), [0]).unwrap();
            let exit = rig.session.quit();
            assert_eq!(exit.save, Flush::WriteFailed);
            assert_eq!(exit.save_stats.written, 0);
            assert!(matches!(
                exit.save_stats.last_error,
                Some(SaveError::Write(_))
            ));
            assert_eq!(std::fs::read(&path).unwrap(), [7; 64], "the old file stays");
        }
    );
}

#[test]
fn the_save_ram_is_flushed_on_the_timer_when_it_changed() {
    // More than 30 s of frames: a small frame and no scene record keep it
    // fast, and nothing here reads a picture.
    let script = SAVE_SCRIPT.replace("geometry 256 224", "geometry 128 112");
    with_script!(
        "the_save_ram_is_flushed_on_the_timer_when_it_changed",
        &script,
        |dir| {
            write_old_save(dir);
            let setup = Setup {
                display: DisplayOptions {
                    render: Render::Off,
                    ..DisplayOptions::default()
                },
                ..Setup::default()
            };
            let mut rig = Rig::launch(dir, setup);
            let due = rig.clock.now_ns() + SAVE_RAM_FLUSH_INTERVAL_NS;
            while rig.clock.now_ns() < due {
                rig.run(1);
            }
            assert_eq!(rig.session.save_stats().unwrap().sent, 0, "not due yet");
            rig.run(1);
            assert_eq!(rig.session.save_stats().unwrap().sent, 1);
            rig.press(&[Control::Select, Control::Start]);
            rig.run(1);
            rig.release(&[Control::Select, Control::Start]);
            assert!(rig.session.run_frame(None).unwrap().menu_opened);
            let stats = rig.session.save_stats().unwrap();
            assert_eq!((stats.sent, stats.unchanged), (1, 1), "unchanged: no write");
            let exit = rig.session.quit();
            assert_eq!(exit.save, Flush::Unchanged);
            assert_eq!(exit.save_stats.written, 1);
        }
    );
}

#[test]
fn opening_the_menu_flushes_the_save_ram() {
    with_script!(
        "opening_the_menu_flushes_the_save_ram",
        SAVE_SCRIPT,
        |dir| {
            write_old_save(dir);
            let mut rig = Rig::launch(dir, Setup::default());
            rig.run(30);
            rig.press(&[Control::Select, Control::Start]);
            rig.run(1);
            rig.release(&[Control::Select, Control::Start]);
            assert!(rig.session.run_frame(None).unwrap().menu_opened);
            assert_eq!(rig.session.save_stats().unwrap().sent, 1);
        }
    );
}

#[test]
fn a_core_shutdown_ends_the_session_and_the_save_ram_is_flushed() {
    let script = format!("{SAVE_SCRIPT}at 40 shutdown\n");
    with_script!(
        "a_core_shutdown_ends_the_session_and_the_save_ram_is_flushed",
        &script,
        |dir| {
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
            let exit = rig.session.quit();
            assert_eq!(exit.save, Flush::Written);
            assert_eq!(std::fs::read(dir.join("saves/game.srm")).unwrap()[0], 42);
        }
    );
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

/// A source whose modeset failed on the device, then for one frame a
/// source that needs no new mode, then the failed source again: its block
/// holds, and the scale-only record hides the failure.
const BLOCKED_SCRIPT: &str = "\
neptron-testcore 1
pixel_format rgb565
geometry 256 224 1333
max_size 512 480
timing 60099 32040
at 20 geometry 512 448 1333
at 40 geometry 256 224 1500
at 41 geometry 512 448 1333
";

#[test]
fn closing_the_menu_retries_a_blocked_source_whatever_the_last_record() {
    with_script!(
        "closing_the_menu_retries_a_blocked_source_whatever_the_last_record",
        BLOCKED_SCRIPT,
        |dir| {
            let mut rig = Rig::launch(dir, Setup::default());
            rig.display.fault_mode(
                1,
                neptron_testkit::ModeFault::Device {
                    reason: "EIO".to_owned(),
                },
            );
            while rig.core_frame() < 70 {
                let _ = rig.session.run_frame(None);
            }
            assert!(rig.session.video().source_blocked());
            assert!(
                !matches!(
                    rig.session.video().trace().last().map(|r| &r.outcome),
                    Some(neptron_video::Outcome::SetModeFailed(_))
                ),
                "a later record hides the failure"
            );
            let calls = rig.display.mode_calls().len();
            rig.press(&[Control::Select, Control::Start]);
            rig.run(1);
            rig.release(&[Control::Select, Control::Start]);
            rig.run(1);
            assert!(rig.session.paused());
            rig.session.resume();
            rig.run(1);
            assert_eq!(
                rig.display.mode_calls().len(),
                calls + 1,
                "the blocked source is tried again"
            );
        }
    );
}

#[test]
fn a_recovery_hold_never_reaches_the_core() {
    with_script!("a_recovery_hold_never_reaches_the_core", SCRIPT, |dir| {
        let mut rig = Rig::launch(dir, Setup::default());
        rig.run(5);
        rig.press(&[Control::Select, Control::Start]);
        let mut recovered = false;
        for _ in 0..330 {
            let report = rig.session.run_frame(None).unwrap();
            assert!(report.ran_core);
            assert_frame(&rig.session, rig.core_frame(), 0);
            recovered |= report.blind_recovery.is_some();
        }
        assert!(recovered, "the 5 s hold fired the recovery");
        // Both stay held back until both are up: Start alone is no press.
        rig.release(&[Control::Select]);
        rig.run(1);
        assert_frame(&rig.session, rig.core_frame(), 0);
        rig.release(&[Control::Start]);
        rig.run(1);
        assert!(!rig.session.paused(), "the hold opens no menu");
        rig.press(&[Control::Start]);
        rig.run(1);
        assert_frame(&rig.session, rig.core_frame(), START);
    });
}
