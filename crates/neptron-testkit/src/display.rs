//! A fake display that behaves like the `Display` contract on the
//! simulated clock, and records every mode and every frame.

mod record;
mod screen;
mod timing;

pub use record::{FrameRecord, ModeRecord, ModeSwitch, SceneSummary};
pub use timing::Jitter;

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use neptron_backend::{Clock, Display, DisplayError};
use neptron_safety::ValidatedMode;
use neptron_types::{FlipEvent, GpuCaps, Modeline, Scene};

use crate::{RgbImage, SimClock, rng};

use screen::Screen;
use timing::{Grid, GridError};

/// The default time `wait_flip` waits for a lost flip before it fails:
/// 100 ms, six frames at 60 Hz.
pub const DEFAULT_FLIP_TIMEOUT_NS: u64 = 100_000_000;

/// A display on the simulated clock that records every `set_mode` and
/// every `present`.
///
/// # Time
///
/// After a `set_mode` at time `t`, vertical blank `k` comes at
/// `t + k * 10^12 / rate_mhz` nanoseconds, plus its [`Jitter`]. The rate
/// is the field rate of the mode read back, or
/// [`DisplayOptions::refresh_mhz`]. Each blank has a sequence number that
/// counts every blank since the display started, across modes.
///
/// A `present` queues one flip for the first blank after the time of the
/// call where a flip can land. That is every blank, except for an
/// interlaced mode on [`GpuCaps`] without `flip_per_field`: there the
/// driver completes one flip per frame, so a flip lands only on the blank
/// that starts a frame, every second field. The sequence numbers still
/// count fields. [`DisplayOptions::refresh_mhz`] overrides this, and every
/// blank of the programmed rate takes a flip. `wait_flip` moves the clock forward to that blank (it never moves
/// it back) and returns its time and sequence number. A frame that the
/// caller presents after its target blank has passed lands on a later
/// blank, and the sequence gap shows it.
///
/// # Contract
///
/// - `present` and `wait_flip` before the first successful `set_mode`
///   return [`DisplayError::NoMode`].
/// - `set_mode` returns the mode read back: the validated mode, unless a
///   [`ModeFault::ReadBack`] says otherwise.
/// - One flip can be queued at a time. A second `present`, or a `set_mode`,
///   while a flip is queued returns [`DisplayError::Device`], as a KMS
///   driver returns `EBUSY`. `wait_flip` with no flip queued returns
///   [`DisplayError::Device`], because a real driver would never send an
///   event.
/// - A scene that the reference renderer refuses returns
///   [`DisplayError::SceneRefused`].
/// - `caps` returns the capabilities given to the constructor.
///
/// # Faults
///
/// [`FakeDisplay::fault_mode`] and [`FakeDisplay::fault_flip`] script
/// failures by call number. [`DisplayOptions::late_flips`] adds seeded late
/// flips. A failed `set_mode` keeps the mode that was active.
///
/// # Sharing
///
/// Clones share one display, so a test keeps a clone and gives another to
/// the code under test. The records and the faults are behind a mutex,
/// which only makes the type `Send`; the fake starts no thread.
#[derive(Debug, Clone)]
pub struct FakeDisplay {
    caps: GpuCaps,
    clock: SimClock,
    state: Arc<Mutex<State>>,
}

/// Settings of a [`FakeDisplay`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisplayOptions {
    /// Whether `present` renders the scene.
    pub render: Render,
    /// How the vertical blanks move from their nominal times.
    pub jitter: Jitter,
    /// Seeded late flips, in addition to the scripted faults.
    pub late_flips: Option<LateFlips>,
    /// The refresh rate of the vertical blanks, in millihertz, instead of
    /// the field rate of the mode. It applies from the next `set_mode`.
    pub refresh_mhz: Option<u64>,
    /// How long `set_mode` takes on the simulated clock, in nanoseconds.
    /// The first blank of the new mode comes one period after its end.
    pub modeset_ns: u64,
    /// How long `wait_flip` waits for a lost flip before it fails, in
    /// nanoseconds.
    pub flip_timeout_ns: u64,
}

/// What the fake display does with each scene. Every mode checks the
/// scene with the reference renderer, so a broken scene is refused in each.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Render {
    /// Render each scene at the size of the mode with the reference
    /// renderer, and record the hash of the pixels. Two frame buffers of the
    /// mode size are kept. In the unoptimised test build, a 320x240 frame
    /// takes about 10 ms. For goldens of every frame.
    Pixels,
    /// Record the hash of the scene: the mode size, the rectangles, the
    /// filter, and the rows of each frame without padding. The renderer is
    /// a pure function of these, so two scenes with the same hash show the
    /// same pixels. The queued scene and the scene on screen are copied,
    /// and rendered only for [`FakeDisplay::screen`] and captures, with the
    /// same pixels as [`Render::Pixels`]. Well under 1 ms per frame. The
    /// default, for sessions.
    #[default]
    Scene,
    /// Check each scene but keep nothing: no hash, no screen. For long
    /// timing runs.
    Off,
}

/// Seeded late flips: each present has a chance to land late. The draw
/// depends only on the seed and the present number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LateFlips {
    /// The seed of the generator.
    pub seed: u64,
    /// The chance that a flip is late, in parts per million.
    pub per_million: u32,
    /// A late flip lands 1 to `max_vblanks` blanks after its target,
    /// rounded up to a blank where a flip can land.
    pub max_vblanks: u32,
}

/// A scripted result of one `set_mode` call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModeFault {
    /// The driver refuses the mode: [`DisplayError::ModeRefused`].
    Refuse {
        /// What the driver reports.
        reason: String,
    },
    /// The device fails: [`DisplayError::Device`].
    Device {
        /// What the device reports.
        reason: String,
    },
    /// The driver takes the mode but reads back this one, for example with
    /// a rounded pixel clock. The flip times follow it.
    ReadBack(Modeline),
}

/// A scripted fault of one frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlipFault {
    /// The flip lands this many blanks after its target, rounded up to a
    /// blank where a flip can land.
    Late {
        /// Extra blanks.
        vblanks: u32,
    },
    /// The flip never arrives: `wait_flip` moves the clock by the flip
    /// timeout and returns [`DisplayError::Device`].
    Lost,
    /// `present` returns [`DisplayError::Device`] and queues nothing.
    PresentFails {
        /// What the device reports.
        reason: String,
    },
    /// The device fails while `wait_flip` waits: it returns
    /// [`DisplayError::Device`] at once, and the queued flip is lost.
    WaitFails {
        /// What the device reports.
        reason: String,
    },
}

impl Default for DisplayOptions {
    fn default() -> Self {
        Self {
            render: Render::Scene,
            jitter: Jitter::None,
            late_flips: None,
            refresh_mhz: None,
            modeset_ns: 0,
            flip_timeout_ns: DEFAULT_FLIP_TIMEOUT_NS,
        }
    }
}

impl FakeDisplay {
    /// A display with the default options: scene hashes, no jitter, no
    /// faults.
    pub fn new(clock: SimClock, caps: GpuCaps) -> Self {
        Self::with_options(clock, caps, DisplayOptions::default())
    }

    /// A display with `options`.
    pub fn with_options(clock: SimClock, caps: GpuCaps, options: DisplayOptions) -> Self {
        Self {
            caps,
            clock,
            state: Arc::new(Mutex::new(State {
                options,
                active: None,
                next_k: 1,
                pending: None,
                mode_faults: BTreeMap::new(),
                flip_faults: BTreeMap::new(),
                modes: Vec::new(),
                frames: Vec::new(),
                screen: Screen::default(),
            })),
        }
    }

    /// Scripts the result of the `set_mode` call number `call` (from 0).
    pub fn fault_mode(&self, call: u64, fault: ModeFault) {
        self.lock().mode_faults.insert(call, fault);
    }

    /// Scripts a fault of the frame with present number `index` (from 0).
    pub fn fault_flip(&self, index: u64, fault: FlipFault) {
        self.lock().flip_faults.insert(index, fault);
    }

    /// The number the next `set_mode` call gets.
    pub fn next_mode_call(&self) -> u64 {
        self.lock().modes.len() as u64
    }

    /// The number the next `present` call gets.
    pub fn next_present_index(&self) -> u64 {
        self.lock().frames.len() as u64
    }

    /// Sets the refresh rate of the vertical blanks, in millihertz, from the
    /// next `set_mode` on. `None` follows the field rate of the mode.
    pub fn set_refresh_mhz(&self, refresh_mhz: Option<u64>) {
        self.lock().options.refresh_mhz = refresh_mhz;
    }

    /// Keeps a copy of the pixels of the frame with present number `index`,
    /// for a golden. Only frames presented after this call are kept, and
    /// none with [`Render::Off`].
    pub fn capture(&self, index: u64) {
        self.lock().screen.request_capture(index);
    }

    /// The pixels kept by [`FakeDisplay::capture`] for frame `index`.
    pub fn captured(&self, index: u64) -> Option<RgbImage> {
        self.lock().screen.captured(index)
    }

    /// The frame on screen now: the last frame that flipped in the active
    /// mode. `None` before that flip, or with [`Render::Off`].
    pub fn screen(&self) -> Option<RgbImage> {
        self.lock().screen.shown()
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        // A poisoned lock means a test thread panicked; the records are
        // still the best report of what happened.
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Display for FakeDisplay {
    fn caps(&self) -> &GpuCaps {
        &self.caps
    }

    fn set_mode(&mut self, mode: &ValidatedMode) -> Result<Modeline, DisplayError> {
        let clock = self.clock.clone();
        let mut state = self.lock();
        let time_ns = clock.now_ns();
        let call = state.modes.len() as u64;
        let result = state.set_mode(&clock, &self.caps, call, mode.mode());
        state.modes.push(ModeRecord {
            call,
            time_ns,
            requested: *mode.mode(),
            result: result.clone(),
        });
        result
    }

    fn present(&mut self, scene: &Scene<'_>) -> Result<(), DisplayError> {
        let present_ns = self.clock.now_ns();
        let mut state = self.lock();
        let index = state.frames.len() as u64;
        let outcome = state.present(index, present_ns, scene);
        let (present, hash, target_sequence, flip_interval) = match outcome {
            Ok(q) => (
                Ok(()),
                q.hash,
                Some(q.target_sequence),
                Some(q.flip_interval),
            ),
            Err(e) => (Err(e), None, None, None),
        };
        state.frames.push(FrameRecord {
            index,
            present_ns,
            scene: SceneSummary::of(scene),
            present: present.clone(),
            hash,
            target_sequence,
            flip_interval,
            flip: None,
            flip_error: None,
        });
        present
    }

    fn wait_flip(&mut self) -> Result<FlipEvent, DisplayError> {
        let clock = self.clock.clone();
        self.lock().wait_flip(&clock)
    }
}

#[derive(Debug)]
struct State {
    options: DisplayOptions,
    active: Option<Active>,
    /// The first blank of the active mode that a new flip can land on.
    next_k: u64,
    pending: Option<Pending>,
    mode_faults: BTreeMap<u64, ModeFault>,
    flip_faults: BTreeMap<u64, FlipFault>,
    modes: Vec<ModeRecord>,
    frames: Vec<FrameRecord>,
    screen: Screen,
}

#[derive(Debug)]
struct Active {
    mode: Modeline,
    grid: Grid,
    /// Blanks from one flip opportunity to the next: 2 for an interlaced
    /// mode when the driver flips once per frame, 1 otherwise.
    flip_interval: u64,
}

#[derive(Debug, Clone, Copy)]
struct Pending {
    index: u64,
    land_k: u64,
}

struct Queued {
    hash: Option<u64>,
    target_sequence: u64,
    flip_interval: u64,
}

impl State {
    fn set_mode(
        &mut self,
        clock: &SimClock,
        caps: &GpuCaps,
        call: u64,
        requested: &Modeline,
    ) -> Result<Modeline, DisplayError> {
        if self.pending.is_some() {
            return Err(busy("set_mode"));
        }
        let active = match self.mode_faults.get(&call) {
            None => *requested,
            Some(ModeFault::ReadBack(mode)) => *mode,
            Some(ModeFault::Refuse { reason }) => {
                return Err(DisplayError::ModeRefused {
                    mode: *requested,
                    reason: reason.clone(),
                });
            }
            Some(ModeFault::Device { reason }) => {
                return Err(device("set_mode", reason.clone()));
            }
        };
        let now = clock.now_ns();
        let sequence_base = self
            .active
            .as_ref()
            .map_or(0, |a| a.grid.sequence(a.grid.count_until(now)));
        let rate_mhz = self
            .options
            .refresh_mhz
            .unwrap_or_else(|| active.field_rate_mhz());
        let per_frame =
            active.interlace && !caps.flip_per_field && self.options.refresh_mhz.is_none();
        let flip_interval = if per_frame { 2 } else { 1 };
        let start_ns = now.saturating_add(self.options.modeset_ns);
        let grid = Grid::new(
            start_ns,
            rate_mhz,
            sequence_base,
            self.options.jitter.clone(),
        )
        .map_err(|e| grid_error(e, rate_mhz))?;
        clock.catch_up(start_ns);
        self.screen.set_mode(&active, self.options.render);
        self.active = Some(Active {
            mode: active,
            grid,
            flip_interval,
        });
        self.next_k = 1;
        Ok(active)
    }

    fn present(
        &mut self,
        index: u64,
        present_ns: u64,
        scene: &Scene<'_>,
    ) -> Result<Queued, DisplayError> {
        let Some(active) = &self.active else {
            return Err(DisplayError::NoMode);
        };
        if self.pending.is_some() {
            return Err(busy("present"));
        }
        let fault = self.flip_faults.get(&index).cloned();
        if let Some(FlipFault::PresentFails { reason }) = fault {
            return Err(device("present", reason));
        }
        let step = active.flip_interval;
        let target_k = active
            .grid
            .first_after(present_ns, self.next_k)
            .next_multiple_of(step);
        let target_sequence = active.grid.sequence(target_k);
        let late = match fault {
            Some(FlipFault::Late { vblanks }) => u64::from(vblanks),
            _ => self.random_late(index),
        }
        .next_multiple_of(step);
        let hash = self
            .screen
            .present(index, scene)
            .map_err(|e| DisplayError::SceneRefused {
                reason: e.to_string(),
            })?;
        self.pending = Some(Pending {
            index,
            land_k: target_k + late,
        });
        Ok(Queued {
            hash,
            target_sequence,
            flip_interval: step,
        })
    }

    fn wait_flip(&mut self, clock: &SimClock) -> Result<FlipEvent, DisplayError> {
        let Some(active) = &self.active else {
            return Err(DisplayError::NoMode);
        };
        let Some(pending) = self.pending.take() else {
            return Err(device("wait_flip", "no flip is queued".to_owned()));
        };
        let outcome = match self.flip_faults.get(&pending.index) {
            Some(FlipFault::Lost) => {
                clock.advance(self.options.flip_timeout_ns);
                Err(device(
                    "wait_flip",
                    format!("no flip event after {} ns", self.options.flip_timeout_ns),
                ))
            }
            Some(FlipFault::WaitFails { reason }) => Err(device("wait_flip", reason.clone())),
            _ => {
                let time_ns = active.grid.time_ns(pending.land_k);
                clock.catch_up(time_ns);
                self.next_k = pending.land_k + 1;
                self.screen.flip();
                Ok(FlipEvent {
                    time_ns,
                    sequence: active.grid.sequence(pending.land_k),
                })
            }
        };
        if let Some(frame) = usize::try_from(pending.index)
            .ok()
            .and_then(|i| self.frames.get_mut(i))
        {
            match &outcome {
                Ok(flip) => frame.flip = Some(*flip),
                Err(e) => frame.flip_error = Some(e.clone()),
            }
        }
        outcome
    }

    fn random_late(&self, index: u64) -> u64 {
        let Some(late) = self.options.late_flips else {
            return 0;
        };
        if late.max_vblanks == 0
            || rng::mix64(late.seed, index) % 1_000_000 >= u64::from(late.per_million)
        {
            return 0;
        }
        1 + rng::mix64(!late.seed, index) % u64::from(late.max_vblanks)
    }
}

fn busy(operation: &'static str) -> DisplayError {
    device(operation, "a flip is already queued".to_owned())
}

fn device(operation: &'static str, reason: String) -> DisplayError {
    DisplayError::Device { operation, reason }
}

fn grid_error(e: GridError, rate_mhz: u64) -> DisplayError {
    let reason = match e {
        GridError::NoRate => "the refresh rate is 0 mHz".to_owned(),
        GridError::JitterTooLarge {
            jitter_ns,
            period_ns,
        } => format!(
            "a jitter of {jitter_ns} ns needs a period above {} ns; {rate_mhz} mHz gives {period_ns} ns",
            jitter_ns.saturating_mul(2)
        ),
    };
    device("set_mode", reason)
}
