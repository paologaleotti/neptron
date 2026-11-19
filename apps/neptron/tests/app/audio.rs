//! The audio settings from the menus: the audio panel at home writes the
//! global file, and the volume row of the quick menu reaches the running
//! game at once and keeps its value for the next game.

use neptron::NextRun;
use neptron_settings::{AUDIO_OUTPUT, AUDIO_VOLUME, Level, Value};
use neptron_types::Control;

use crate::rig::{Rig, in_child, in_children, test_name};

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

#[test]
#[ignore = "tier 2"]
fn the_audio_panel_writes_the_volume_and_the_output_to_the_global_file() {
    in_child(
        &test_name(
            module_path!(),
            "the_audio_panel_writes_the_volume_and_the_output_to_the_global_file",
        ),
        |dir| {
            let mut rig = Rig::new(dir);
            rig.boot(None);
            rig.frames(2);
            rig.until(600, "the scan", |r| {
                !r.shell().snapshot().systems.is_empty()
            });
            rig.focus("Settings");
            rig.tap(&[Control::A]);
            assert_eq!(rig.screen(), "settings");
            rig.focus("Audio");
            rig.tap(&[Control::A]);
            assert_eq!(rig.screen(), "audio");
            rig.focus("Volume");
            rig.tap(&[Control::Left]);
            rig.tap(&[Control::Left]);
            rig.focus("Audio output");
            rig.tap(&[Control::A]);
            assert_eq!(rig.screen(), "options");
            rig.focus("Analog (USB Audio)");
            rig.tap(&[Control::A]);
            rig.frames(2);
            let config = rig.shell().config().clone();
            assert_eq!(
                config.get(Level::Global, AUDIO_VOLUME),
                Some(&Value::Int(70))
            );
            assert_eq!(
                config.get(Level::Global, AUDIO_OUTPUT),
                Some(&Value::Text("hw:CARD=Device,DEV=0".to_owned()))
            );
            rig.finish();
            let file = std::fs::read_to_string(rig.paths.config().join("global.toml"))
                .expect("the global file");
            assert!(file.contains("volume = 70"), "{file}");
            assert!(file.contains("output = \"hw:CARD=Device,DEV=0\""), "{file}");
        },
    );
}

#[test]
#[ignore = "tier 2"]
fn the_quick_menu_volume_reaches_the_game_at_once() {
    // One process hosts one core, so each run is a stage of its own, as
    // the launcher starts a process for each run.
    in_children(
        &test_name(
            module_path!(),
            "the_quick_menu_volume_reaches_the_game_at_once",
        ),
        3,
        |dir, stage| {
            let mut rig = Rig::new(dir);
            match stage {
                // A first boot with nothing confirmed: keep the display,
                // so no prompt or notice stands in the way of the game.
                0 => {
                    rig.boot(None);
                    rig.frames(2);
                }
                1 => {
                    rig.boot(Some(game()));
                    rig.frames(30);
                    assert_eq!(rig.shell().game_volume_percent(), Some(80));
                    rig.open_menu();
                    rig.frames(1);
                    assert_eq!(rig.screen(), "quick-menu");
                    rig.focus("Volume");
                    rig.tap(&[Control::Left]);
                    assert_eq!(rig.shell().game_volume_percent(), Some(75));
                    rig.tap(&[Control::Left]);
                    assert_eq!(rig.shell().game_volume_percent(), Some(70));
                    assert_eq!(
                        rig.shell().config().get(Level::Game, AUDIO_VOLUME),
                        Some(&Value::Int(70))
                    );
                }
                _ => {
                    rig.boot(Some(game()));
                    rig.frames(30);
                    assert_eq!(
                        rig.shell().game_volume_percent(),
                        Some(70),
                        "the next run of the game resolves its volume"
                    );
                }
            }
            rig.finish();
        },
    );
}

/// The `audio_underruns` stat counts the times the audio queue ran empty
/// in play: here the frame loop stalls for 300 ms, longer than the queue.
#[test]
#[ignore = "tier 2"]
fn the_stats_count_an_audio_underrun() {
    in_child(
        &test_name(module_path!(), "the_stats_count_an_audio_underrun"),
        |dir| {
            let mut rig = crate::pause::in_game(dir);
            rig.frames(30);
            assert_eq!(rig.shell().stats_now().audio_underruns, 0);
            rig.clock.advance(300_000_000);
            rig.frames(2);
            assert!(!rig.audio.underruns().is_empty());
            assert_eq!(rig.shell().stats_now().audio_underruns, 1);
            rig.finish();
        },
    );
}

/// The `fields_missed` stat counts the fields in which no new frame reached
/// the display while the core ran: none in steady play, and each blank
/// that a 300 ms stall of the frame loop let pass with no flip.
#[test]
#[ignore = "tier 2"]
fn the_stats_count_the_fields_with_no_new_frame() {
    in_child(
        &test_name(
            module_path!(),
            "the_stats_count_the_fields_with_no_new_frame",
        ),
        |dir| {
            let mut rig = crate::pause::in_game(dir);
            rig.frames(30);
            let stats = rig.shell().stats_now();
            assert_eq!(stats.fields_missed, Some(0));
            let dropped = stats.frames_dropped;
            let before = rig.display.flips().len();
            rig.clock.advance(300_000_000);
            rig.frames(2);
            let flips = rig.display.flips();
            let late: u64 = flips[before - 1..]
                .windows(2)
                .map(|pair| pair[1].sequence - pair[0].sequence - 1)
                .sum();
            let stats = rig.shell().stats_now();
            assert!(
                late >= 17,
                "a 300 ms stall passes 17 or more blanks: {late}"
            );
            assert_eq!(stats.fields_missed, Some(late));
            assert_eq!(
                stats.frames_dropped, dropped,
                "frames_dropped keeps its meaning"
            );
            rig.finish();
        },
    );
}

/// The `input_to_frame_ns` stat is the delay from the poll of an input
/// to the flip of the frame that ran the core with it: none before an
/// input, one flip interval for a game that fits in its field.
#[test]
#[ignore = "tier 2"]
fn the_stats_give_the_delay_from_an_input_to_its_flip() {
    in_child(
        &test_name(
            module_path!(),
            "the_stats_give_the_delay_from_an_input_to_its_flip",
        ),
        |dir| {
            let mut rig = crate::pause::in_game(dir);
            rig.frames(30);
            assert_eq!(rig.shell().stats_now().input_to_frame_ns, 0);
            rig.tap(&[Control::A]);
            let flips = rig.display.flips();
            let interval = flips[flips.len() - 1].time_ns - flips[flips.len() - 2].time_ns;
            let delay = rig.shell().stats_now().input_to_frame_ns;
            assert!(
                delay.abs_diff(interval) <= 1,
                "{delay} ns for {interval} ns"
            );
            rig.finish();
        },
    );
}

/// A 300 ms stall of the frame loop is a late frame: the loop goes to
/// late pacing, which the stats count and the log says, and back to wait
/// pacing after the hold, which the log says too.
#[test]
#[ignore = "tier 2"]
fn the_stats_and_the_log_show_the_late_pacing() {
    in_child(
        &test_name(module_path!(), "the_stats_and_the_log_show_the_late_pacing"),
        |dir| {
            let mut rig = crate::pause::in_game(dir);
            rig.frames(30);
            let stats = rig.shell().stats_now();
            assert_eq!((stats.late_frames, stats.late_flips), (Some(0), Some(0)));
            rig.clock.advance(300_000_000);
            rig.frames(2);
            assert!(rig.shell().stats_now().late_frames > Some(0));
            rig.frames(60);
            let lines = rig.shell().log_lines(20);
            let late = lines.iter().position(|l| l.starts_with("late pacing:"));
            let wait = lines.iter().position(|l| l.starts_with("wait pacing:"));
            assert!(late.is_some() && wait > late, "{lines:?}");
            rig.finish();
        },
    );
}
