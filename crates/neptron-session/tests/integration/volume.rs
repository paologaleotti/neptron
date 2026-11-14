//! The user volume through the session, and a sound device lost during a
//! game.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use neptron_backend::AudioSink;
use neptron_testkit::FakeAudio;

use crate::harness::{DEVICE_RATE_HZ, Rig, SINK_CAPACITY_FRAMES, Setup, launch_on};
use crate::with_script;

const SCRIPT: &str = "\
neptron-testcore 1
pixel_format rgb565
geometry 256 224 1333
max_size 320 240
timing 60000 48000
";

/// The largest sample, in magnitude, of `samples`.
fn peak(samples: &[i16]) -> i32 {
    samples
        .iter()
        .map(|s| i32::from(*s).abs())
        .max()
        .unwrap_or(0)
}

fn setup(volume_percent: u32) -> Setup {
    Setup {
        volume_percent,
        keep_samples: true,
        ..Setup::default()
    }
}

#[test]
fn the_volume_of_the_launch_reaches_the_pipeline_with_no_ramp() {
    with_script!(
        "the_volume_of_the_launch_reaches_the_pipeline_with_no_ramp",
        SCRIPT,
        |dir| {
            let mut rig = Rig::launch(dir, setup(0));
            rig.run(120);
            assert_eq!(rig.session.volume_percent(), Some(0));
            let samples = rig.audio.samples();
            assert!(samples.len() > 48_000);
            assert_eq!(peak(&samples), 0, "volume 0 is silence from the start");
        }
    );
}

#[test]
fn a_live_volume_change_ramps_down_and_keeps_the_frames_going() {
    with_script!(
        "a_live_volume_change_ramps_down_and_keeps_the_frames_going",
        SCRIPT,
        |dir| {
            let mut rig = Rig::launch(dir, setup(100));
            rig.run(120);
            let before = rig.audio.samples().len();
            let loud = peak(&rig.audio.samples()[before / 2..]);
            assert!(loud > 1_000, "the test core plays at {loud}");
            rig.session.set_volume_percent(30);
            rig.run(120);
            assert_eq!(rig.session.volume_percent(), Some(30));
            let after = rig.audio.samples();
            // 20 ms of ramp is 960 frames; the second half of the new
            // samples is far past it.
            let quiet = peak(&after[before + (after.len() - before) / 2..]);
            let limit = loud * 27 / 1_000 + 1;
            assert!(quiet <= limit, "{quiet} above {limit}");
            assert!(quiet > 0);
            rig.session.set_volume_percent(250);
            assert_eq!(rig.session.volume_percent(), Some(100));
            assert_eq!(rig.session.counters().core_frames, 240);
        }
    );
}

#[test]
fn a_timing_change_keeps_the_volume() {
    let script = format!("{SCRIPT}at 300 av_info 256 224 1333 320 240 50007 32040\n");
    with_script!("a_timing_change_keeps_the_volume", &script, |dir| {
        let mut rig = Rig::launch(dir, setup(40));
        rig.run(600);
        let stats = rig.session.audio_stats().unwrap();
        assert!(stats.frames < 600, "a new pipeline after the change");
        assert_eq!(rig.session.volume_percent(), Some(40));
    });
}

/// A sound device that can be lost: from then on it takes nothing and
/// reports a full queue, as `Sdl3Sink` does after an SDL error.
struct Losable {
    inner: FakeAudio,
    lost: Arc<AtomicBool>,
}

impl AudioSink for Losable {
    fn rate_hz(&self) -> u32 {
        self.inner.rate_hz()
    }

    fn queued_frames(&self) -> usize {
        if self.lost.load(Ordering::Relaxed) {
            SINK_CAPACITY_FRAMES
        } else {
            self.inner.queued_frames()
        }
    }

    fn write(&mut self, stereo: &[i16]) -> usize {
        if self.lost.load(Ordering::Relaxed) {
            0
        } else {
            self.inner.write(stereo)
        }
    }
}

#[test]
fn a_sound_device_lost_during_a_game_stops_nothing() {
    with_script!(
        "a_sound_device_lost_during_a_game_stops_nothing",
        SCRIPT,
        |dir| {
            let lost = Arc::new(AtomicBool::new(false));
            let flag = lost.clone();
            let (mut session, _clock) = launch_on(dir, Setup::default(), move |clock, _| Losable {
                inner: FakeAudio::new(clock.clone(), DEVICE_RATE_HZ, SINK_CAPACITY_FRAMES),
                lost: flag,
            });
            for frame in 0..600 {
                if frame == 200 {
                    lost.store(true, Ordering::Relaxed);
                }
                if frame == 400 {
                    session.set_volume_percent(50);
                }
                let report = session.run_frame(None).expect("the frame runs");
                assert_eq!(report.flips, 1, "frame {frame} flips");
            }
            let stats = session.audio_stats().expect("the pipeline stays");
            assert!(stats.overrun_frames > 300 * 700, "{stats:?}");
            assert_eq!(session.counters().core_frames, 600);
            assert!(!session.counters().audio_lost);
        }
    );
}
