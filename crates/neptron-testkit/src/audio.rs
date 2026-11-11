//! A fake audio sink that plays its queue at a fixed rate on the simulated
//! clock.

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use neptron_backend::{AudioSink, Clock};

use crate::SimClock;

/// Nanoseconds times millihertz in one second: `10^9 * 10^3`.
const NS_MHZ_PER_S: u128 = 1_000_000_000_000;
/// FNV-1a 64 offset basis.
const FNV_OFFSET: u64 = 0xCBF2_9CE4_8422_2325;
/// FNV-1a 64 prime.
const FNV_PRIME: u64 = 0x0000_0100_0000_01B3;

/// An audio device on the simulated clock.
///
/// The device starts to play at the first write that queues a frame. From
/// then on it takes `device_rate_mhz` stereo frames per second from the
/// queue: by time `t` it has asked for `(t - start) * device_rate_mhz /
/// 10^12` frames, rounded down. When the queue is empty, the device plays
/// silence, and those frames are lost: an underrun. A write takes what fits
/// in the free part of the queue; the rest is an overrun.
///
/// Time never moves here. The queue is brought up to date on each write and
/// each query, from the clock.
///
/// Clones share one device, so a test keeps a clone to read the records.
#[derive(Debug, Clone)]
pub struct FakeAudio {
    rate_hz: u32,
    clock: SimClock,
    state: Arc<Mutex<State>>,
}

/// Settings of a [`FakeAudio`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioOptions {
    /// The size of the queue, in stereo frames.
    pub capacity_frames: usize,
    /// The real rate of the device, in millihertz, when it differs from the
    /// nominal rate, for example 48 000 500 for a device 0.001% fast.
    /// `None` plays at the nominal rate.
    pub device_rate_mhz: Option<u64>,
    /// Keep every sample the device took, for a test that checks the
    /// signal. Off by default: the sample hash is always kept.
    pub keep_samples: bool,
}

/// One call of `write`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioWrite {
    /// When the call happened, in nanoseconds.
    pub time_ns: u64,
    /// Stereo frames the caller offered.
    pub offered_frames: usize,
    /// Stereo frames the queue took.
    pub taken_frames: usize,
    /// Stereo frames in the queue after the call.
    pub queued_after: usize,
}

/// A stretch of time in which the device found its queue empty.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Underrun {
    /// When the device first asked for a frame that was not there, in
    /// nanoseconds.
    pub time_ns: u64,
    /// Frames the device played as silence before the next write.
    pub missing_frames: u64,
}

impl AudioWrite {
    /// Frames the queue refused because it was full.
    pub fn dropped_frames(&self) -> usize {
        self.offered_frames.saturating_sub(self.taken_frames)
    }
}

impl FakeAudio {
    /// A device at `rate_hz` with a queue of `capacity_frames` stereo
    /// frames.
    pub fn new(clock: SimClock, rate_hz: u32, capacity_frames: usize) -> Self {
        let options = AudioOptions {
            capacity_frames,
            device_rate_mhz: None,
            keep_samples: false,
        };
        Self::with_options(clock, rate_hz, options)
    }

    /// A device at the nominal rate `rate_hz` with `options`.
    pub fn with_options(clock: SimClock, rate_hz: u32, options: AudioOptions) -> Self {
        let device_rate_mhz = options
            .device_rate_mhz
            .unwrap_or(u64::from(rate_hz) * 1_000);
        Self {
            rate_hz,
            clock,
            state: Arc::new(Mutex::new(State {
                capacity: options.capacity_frames,
                device_rate_mhz,
                keep_samples: options.keep_samples,
                start_ns: None,
                asked: 0,
                queued: 0,
                played: 0,
                written: 0,
                starving: false,
                sample_hash: FNV_OFFSET,
                samples: Vec::new(),
                writes: Vec::new(),
                underruns: Vec::new(),
            })),
        }
    }

    /// Every `write` call, in order.
    pub fn writes(&self) -> Vec<AudioWrite> {
        self.settled().writes.clone()
    }

    /// The writes that the queue did not take in full.
    pub fn overruns(&self) -> Vec<AudioWrite> {
        self.settled()
            .writes
            .iter()
            .filter(|w| w.taken_frames < w.offered_frames)
            .copied()
            .collect()
    }

    /// Every underrun up to now, in order.
    pub fn underruns(&self) -> Vec<Underrun> {
        self.settled().underruns.clone()
    }

    /// Frames the device played from the queue up to now.
    pub fn played_frames(&self) -> u64 {
        self.settled().played
    }

    /// Frames the queue took, in total.
    pub fn written_frames(&self) -> u64 {
        self.settled().written
    }

    /// FNV-1a 64 over every sample the queue took, as 16-bit values, in
    /// order. It is the same on every host and in every run.
    pub fn sample_hash(&self) -> u64 {
        self.settled().sample_hash
    }

    /// Every sample the queue took, interleaved, when
    /// [`AudioOptions::keep_samples`] is on. Empty otherwise.
    pub fn samples(&self) -> Vec<i16> {
        self.settled().samples.clone()
    }

    fn settled(&self) -> MutexGuard<'_, State> {
        let now = self.clock.now_ns();
        // A poisoned lock means a test thread panicked; the records are
        // still the best report of what happened.
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        state.settle(now);
        state
    }
}

impl AudioSink for FakeAudio {
    fn rate_hz(&self) -> u32 {
        self.rate_hz
    }

    fn queued_frames(&self) -> usize {
        self.settled().queued
    }

    fn write(&mut self, stereo: &[i16]) -> usize {
        let time_ns = self.clock.now_ns();
        let mut state = self.settled();
        let offered = stereo.len() / 2;
        let taken = offered.min(state.capacity - state.queued);
        state.take(time_ns, &stereo[..taken * 2]);
        let queued_after = state.queued;
        state.writes.push(AudioWrite {
            time_ns,
            offered_frames: offered,
            taken_frames: taken,
            queued_after,
        });
        taken
    }
}

#[derive(Debug)]
struct State {
    capacity: usize,
    device_rate_mhz: u64,
    keep_samples: bool,
    /// When the device started to play.
    start_ns: Option<u64>,
    /// Frames the device asked for since it started.
    asked: u64,
    queued: usize,
    played: u64,
    written: u64,
    /// The queue was empty when the device last asked, and nothing came
    /// since.
    starving: bool,
    sample_hash: u64,
    samples: Vec<i16>,
    writes: Vec<AudioWrite>,
    underruns: Vec<Underrun>,
}

impl State {
    /// Plays the frames the device asked for up to `now_ns`.
    fn settle(&mut self, now_ns: u64) {
        let Some(start_ns) = self.start_ns else {
            return;
        };
        let due = self.frames_due(now_ns.saturating_sub(start_ns));
        let demand = due.saturating_sub(self.asked);
        if demand == 0 {
            return;
        }
        let from_queue = demand.min(self.queued as u64);
        // `from_queue` is at most `queued`, a usize.
        self.queued -= from_queue as usize;
        self.played += from_queue;
        let missing = demand - from_queue;
        if missing > 0 {
            match self.underruns.last_mut() {
                Some(last) if self.starving => last.missing_frames += missing,
                _ => {
                    let time_ns = start_ns + self.time_of(self.asked + from_queue + 1);
                    self.underruns.push(Underrun {
                        time_ns,
                        missing_frames: missing,
                    });
                }
            }
            self.starving = true;
        }
        self.asked = due;
    }

    /// Queues the interleaved `samples`, which fit.
    fn take(&mut self, now_ns: u64, samples: &[i16]) {
        let frames = samples.len() / 2;
        if frames == 0 {
            return;
        }
        if self.start_ns.is_none() {
            self.start_ns = Some(now_ns);
        }
        self.queued += frames;
        self.written += frames as u64;
        self.starving = false;
        for sample in samples {
            self.sample_hash =
                (self.sample_hash ^ u64::from(*sample as u16)).wrapping_mul(FNV_PRIME);
        }
        if self.keep_samples {
            self.samples.extend_from_slice(samples);
        }
    }

    /// Frames the device asks for in `elapsed_ns` after its start.
    fn frames_due(&self, elapsed_ns: u64) -> u64 {
        let due = u128::from(elapsed_ns) * u128::from(self.device_rate_mhz) / NS_MHZ_PER_S;
        u64::try_from(due).unwrap_or(u64::MAX)
    }

    /// The first time, after the start, at which the device has asked for
    /// `frames` frames.
    fn time_of(&self, frames: u64) -> u64 {
        let rate = u128::from(self.device_rate_mhz.max(1));
        let ns = (u128::from(frames) * NS_MHZ_PER_S).div_ceil(rate);
        u64::try_from(ns).unwrap_or(u64::MAX)
    }
}
