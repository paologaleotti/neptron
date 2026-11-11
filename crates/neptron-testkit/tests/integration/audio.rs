//! The fake audio sink: playback on the simulated clock, underruns and
//! overruns.

use neptron_backend::AudioSink;
use neptron_testkit::{AudioOptions, AudioWrite, FakeAudio, SimClock, Underrun};

const MS: u64 = 1_000_000;

#[test]
fn the_device_plays_at_its_rate_on_the_simulated_clock() {
    let clock = SimClock::starting_at(5 * MS);
    let mut audio = FakeAudio::new(clock.clone(), 48_000, 4_096);
    assert_eq!(audio.rate_hz(), 48_000);
    // Nothing plays before the first write.
    clock.advance(100 * MS);
    assert_eq!(audio.queued_frames(), 0);
    assert!(audio.underruns().is_empty());

    assert_eq!(audio.write(&[1; 1_600]), 800);
    clock.advance(10 * MS);
    assert_eq!(audio.queued_frames(), 320);
    assert_eq!(audio.played_frames(), 480);
    assert_eq!(audio.written_frames(), 800);
}

#[test]
fn an_empty_queue_is_one_underrun_until_the_next_write() {
    let clock = SimClock::new();
    let mut audio = FakeAudio::new(clock.clone(), 48_000, 4_096);
    audio.write(&[0; 1_600]);
    clock.advance(20 * MS);
    assert_eq!(audio.queued_frames(), 0);
    clock.advance(10 * MS);
    // 801 frames at 48 kHz take 16.6875 ms; 1440 frames were asked for.
    assert_eq!(
        audio.underruns(),
        [Underrun {
            time_ns: 16_687_500,
            missing_frames: 640
        }]
    );
    audio.write(&[0; 96]);
    clock.advance(2 * MS);
    let underruns = audio.underruns();
    assert_eq!(underruns.len(), 2);
    // The 48 frames run out at frame 1489: 31.020 833 ms, rounded up.
    assert_eq!(underruns[1].time_ns, 31_020_834);
    assert_eq!(underruns[1].missing_frames, 48);
}

#[test]
fn a_full_queue_takes_what_fits() {
    let clock = SimClock::new();
    let mut audio = FakeAudio::new(clock.clone(), 48_000, 1_000);
    assert_eq!(audio.write(&[0; 1_600]), 800);
    assert_eq!(audio.write(&[0; 1_600]), 200);
    assert_eq!(audio.write(&[0; 3]), 0);
    clock.advance(MS);
    assert_eq!(audio.write(&[0; 3]), 1);
    let overruns = audio.overruns();
    assert_eq!(overruns.len(), 2);
    assert_eq!(overruns[0].dropped_frames(), 600);
    assert_eq!(
        audio.writes()[3],
        AudioWrite {
            time_ns: MS,
            offered_frames: 1,
            taken_frames: 1,
            queued_after: 953
        }
    );
}

#[test]
fn a_device_rate_offset_changes_the_playback_speed() {
    let clock = SimClock::new();
    let options = AudioOptions {
        capacity_frames: 100_000,
        device_rate_mhz: Some(48_048_000),
        keep_samples: false,
    };
    let mut audio = FakeAudio::with_options(clock.clone(), 48_000, options);
    audio.write(&[0; 200_000]);
    clock.advance(1_000 * MS);
    assert_eq!(audio.played_frames(), 48_048);
    assert_eq!(audio.rate_hz(), 48_000);
}

#[test]
fn samples_are_hashed_and_kept_on_request() {
    let run = |keep_samples| {
        let clock = SimClock::new();
        let options = AudioOptions {
            capacity_frames: 4,
            device_rate_mhz: None,
            keep_samples,
        };
        let mut audio = FakeAudio::with_options(clock, 48_000, options);
        audio.write(&[1, -1, 2, -2, 3, -3, 4, -4, 5, -5]);
        audio
    };
    let kept = run(true);
    assert_eq!(kept.samples(), [1, -1, 2, -2, 3, -3, 4, -4]);
    let plain = run(false);
    assert!(plain.samples().is_empty());
    assert_eq!(kept.sample_hash(), plain.sample_hash());

    let clock = SimClock::new();
    let mut other = FakeAudio::new(clock, 48_000, 4);
    other.write(&[1, -1, 2, -2, 3, -3, 4, 4]);
    assert_ne!(other.sample_hash(), plain.sample_hash());
}

#[test]
fn clones_share_the_device() {
    let clock = SimClock::new();
    let probe = FakeAudio::new(clock, 48_000, 16);
    let mut boxed: Box<dyn AudioSink + Send> = Box::new(probe.clone());
    boxed.write(&[0; 8]);
    assert_eq!(probe.queued_frames(), 4);
}
