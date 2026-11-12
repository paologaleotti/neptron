//! What the fake display records, and the queries on it.

use neptron_backend::DisplayError;
use neptron_types::{Filter, FlipEvent, Modeline, PixelFormat, Rect, Scene};

use super::FakeDisplay;

/// One call of `set_mode`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModeRecord {
    /// The call number, from 0.
    pub call: u64,
    /// When the call started, in nanoseconds.
    pub time_ns: u64,
    /// The mode the caller asked for.
    pub requested: Modeline,
    /// The mode read back, or the error the call returned.
    pub result: Result<Modeline, DisplayError>,
}

/// One mode that the display took.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModeSwitch {
    /// When the call started, in nanoseconds.
    pub time_ns: u64,
    /// The mode the caller asked for.
    pub requested: Modeline,
    /// The mode read back, which sets the flip times.
    pub active: Modeline,
}

/// What a scene held, without its pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SceneSummary {
    /// Width, height and format of the game frame, if any.
    pub game: Option<(u16, u16, PixelFormat)>,
    /// The crop rectangle.
    pub crop: Rect,
    /// The destination rectangle.
    pub dest: Rect,
    /// The scaling filter.
    pub filter: Filter,
    /// Width and height of the UI canvas, if any.
    pub ui: Option<(u16, u16)>,
    /// The UI destination rectangle.
    pub ui_dest: Rect,
}

/// One call of `present`, and the flip that followed it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameRecord {
    /// The present number, from 0. Failed calls count too.
    pub index: u64,
    /// When `present` was called, in nanoseconds.
    pub present_ns: u64,
    /// The scene, without its pixels.
    pub scene: SceneSummary,
    /// What `present` returned.
    pub present: Result<(), DisplayError>,
    /// With [`Render::Pixels`](crate::Render::Pixels), the
    /// [`frame_hash`](crate::frame_hash) of the rendered XRGB8888 rows,
    /// no padding. With [`Render::Scene`](crate::Render::Scene), the hash
    /// of the scene. `None` when rendering is off or `present` failed.
    pub hash: Option<u64>,
    /// The sequence number of the first vertical blank after the present:
    /// the flip is on time when it lands there. `None` when `present`
    /// failed.
    pub target_sequence: Option<u64>,
    /// Blanks from one flip opportunity to the next in the mode of the
    /// present: 2 for an interlaced mode when the driver flips once per
    /// frame, 1 otherwise. `None` when `present` failed.
    pub flip_interval: Option<u64>,
    /// The completed flip, once `wait_flip` returned it.
    pub flip: Option<FlipEvent>,
    /// The last error `wait_flip` returned for this frame, if any. The
    /// flip stays queued after it, so `flip` can still come later.
    pub flip_error: Option<DisplayError>,
}

impl FrameRecord {
    /// Vertical blanks between the target and the flip: 0 for a flip on
    /// time, `None` for a frame with no flip.
    pub fn late_by(&self) -> Option<u64> {
        let flip = self.flip?;
        Some(flip.sequence.saturating_sub(self.target_sequence?))
    }
}

impl SceneSummary {
    pub(crate) fn of(scene: &Scene<'_>) -> Self {
        Self {
            game: scene.game.map(|f| (f.width, f.height, f.format)),
            crop: scene.crop,
            dest: scene.dest,
            filter: scene.filter,
            ui: scene.ui.map(|f| (f.width, f.height)),
            ui_dest: scene.ui_dest,
        }
    }
}

impl FakeDisplay {
    /// Every `set_mode` call, in order, with its result.
    pub fn mode_calls(&self) -> Vec<ModeRecord> {
        self.lock().modes.clone()
    }

    /// Every mode the display took, in order.
    pub fn mode_switches(&self) -> Vec<ModeSwitch> {
        self.lock()
            .modes
            .iter()
            .filter_map(|r| {
                let active = r.result.as_ref().ok()?;
                Some(ModeSwitch {
                    time_ns: r.time_ns,
                    requested: r.requested,
                    active: *active,
                })
            })
            .collect()
    }

    /// The mode on the display now, as read back.
    pub fn active_mode(&self) -> Option<Modeline> {
        self.lock().active.as_ref().map(|a| a.mode)
    }

    /// Every `present` call, in order, with its flip.
    pub fn frames(&self) -> Vec<FrameRecord> {
        self.lock().frames.clone()
    }

    /// Every completed flip, in order.
    pub fn flips(&self) -> Vec<FlipEvent> {
        self.lock().frames.iter().filter_map(|f| f.flip).collect()
    }

    /// The time of every completed flip, in nanoseconds.
    pub fn flip_times(&self) -> Vec<u64> {
        self.flips().iter().map(|f| f.time_ns).collect()
    }

    /// The hash of every frame that flipped, in order. Empty when
    /// rendering is off.
    pub fn frame_hashes(&self) -> Vec<u64> {
        self.lock()
            .frames
            .iter()
            .filter(|f| f.flip.is_some())
            .filter_map(|f| f.hash)
            .collect()
    }

    /// The frames that flipped one or more vertical blanks after their
    /// target.
    pub fn late_frames(&self) -> Vec<FrameRecord> {
        self.lock()
            .frames
            .iter()
            .filter(|f| f.late_by().is_some_and(|n| n > 0))
            .cloned()
            .collect()
    }

    /// The frames whose `present` succeeded but that never flipped: the
    /// flip did not land yet, or nobody waited for it.
    pub fn dropped_frames(&self) -> Vec<FrameRecord> {
        self.lock()
            .frames
            .iter()
            .filter(|f| f.present.is_ok() && f.flip.is_none())
            .cloned()
            .collect()
    }

    /// Vertical blanks that showed the previous frame again beyond the flip
    /// interval: the sum, over each pair of flips that follow each other, of
    /// the sequence gap less the flip interval of the later frame. 0 when
    /// every flip opportunity brought a new frame.
    pub fn repeated_vblanks(&self) -> u64 {
        let state = self.lock();
        let flipped: Vec<(u64, u64)> = state
            .frames
            .iter()
            .filter_map(|f| Some((f.flip?.sequence, f.flip_interval?)))
            .collect();
        flipped
            .windows(2)
            .map(|pair| {
                pair[1]
                    .0
                    .saturating_sub(pair[0].0)
                    .saturating_sub(pair[1].1)
            })
            .sum()
    }
}
