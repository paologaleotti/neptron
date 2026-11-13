use std::collections::VecDeque;
use std::fmt;

use neptron_backend::DisplayError;
use neptron_modeline::policy::{Dropped, Refused, Rung};
use neptron_types::{Modeline, SourceGeometry};

use crate::{Chosen, Refusal, Unshowable};

/// A read back whose field rate differs from the requested mode's by this
/// many millihertz or more is noted in its record. It is the policy's
/// bound for "matches to two decimals" (`REFRESH_MATCH_MHZ`, 5 mHz): from
/// there on the rate on the wire is no longer the rate the policy chose.
/// Pacing and audio already follow the read back; the note tells the
/// player and the log.
pub const READ_BACK_RATE_NOTICE_MHZ: u64 = neptron_modeline::policy::REFRESH_MATCH_MHZ;

/// How many records the trace keeps. The oldest goes first.
pub const TRACE_CAPACITY: usize = 64;

/// What made the engine decide again.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Trigger {
    /// The first frame: no mode is set yet.
    Launch,
    /// The core's geometry or rate changed.
    Geometry,
    /// The monitor changed.
    Monitor,
    /// The policy settings changed.
    Settings,
    /// The driver refused the last mode, so the engine tries the next
    /// proposal.
    Retry,
}

/// What came of a decision.
#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    /// The winner equals the mode on screen: only crop, destination and
    /// filter changed.
    ScaleOnly,
    /// The winner was set. The mode read back is in the record.
    Modeset,
    /// No mode for the source: the mode on screen stays.
    Kept(Unshowable),
    /// `set_mode` failed. The mode on screen stays.
    SetModeFailed(DisplayError),
    /// The driver read back a mode that differs from the winner in its
    /// size or scan, or that the validator refuses. The engine set the last
    /// good mode again, when there is one.
    ReadBackRefused {
        /// The mode the driver reported.
        read_back: Modeline,
    },
    /// The last good mode was set again after a refused read back, and
    /// its read back holds. The record holds that read back, and the flip
    /// that showed it.
    Restored,
    /// No valid mode exists for the inputs, and the output was turned off
    /// with `blank`. No mode is on the display.
    Blanked,
    /// `blank` failed. The refused mode stays on the display.
    BlankFailed(DisplayError),
}

/// One decision that changed something or failed: what was asked, what
/// won, what was dropped and why, and how long it took.
#[derive(Debug, Clone, PartialEq)]
pub struct SwitchRecord {
    /// Number of the core frame that first asked for the source, from 1.
    pub asked_frame: u64,
    /// Number of the core frame of the decision. For merged scale-only
    /// records, the last one.
    pub decided_frame: u64,
    /// How many decisions the record holds: 1, or more for consecutive
    /// scale-only changes, which share one record so that a flicker under
    /// a fixed super width does not push the modesets out of the trace.
    pub count: u32,
    /// Number of the core frame whose flip first showed the new mode.
    /// `None` until that flip, and for an outcome other than a modeset.
    pub shown_frame: Option<u64>,
    /// Time from the last flip in the old mode to the first flip in the
    /// new one, in nanoseconds: how long the picture was gone. `None` at
    /// launch and until the first flip.
    pub gap_ns: Option<u64>,
    /// What made the engine decide.
    pub trigger: Trigger,
    /// The source that was asked for.
    pub source: SourceGeometry,
    /// The mode on screen before, as the engine set it.
    pub previous: Option<Modeline>,
    /// The winner, if any.
    pub winner: Option<Chosen>,
    /// The mode the driver read back, after a modeset.
    pub read_back: Option<Modeline>,
    /// The difference of field rate between the requested mode and the
    /// read back, in millihertz, when it is
    /// [`READ_BACK_RATE_NOTICE_MHZ`] or more.
    pub rate_drift_mhz: Option<u64>,
    /// What came of it.
    pub outcome: Outcome,
    /// The candidates the policy dropped before the winner (all of them
    /// when nothing won), with their index in the plan. Empty for
    /// [`Outcome::ScaleOnly`].
    pub dropped: Vec<(usize, Rung, Dropped)>,
    /// The proposals the validator or the driver refused before the
    /// winner. Empty for [`Outcome::ScaleOnly`].
    pub refused: Vec<Refused<Refusal>>,
}

/// The last [`TRACE_CAPACITY`] switch records, oldest first. The control
/// protocol and the tests read it.
#[derive(Debug, Clone, PartialEq)]
pub struct SwitchTrace {
    records: VecDeque<SwitchRecord>,
    total: u64,
    modesets: u64,
}

impl SwitchRecord {
    /// Core frames from the first frame that asked for the source to the
    /// first flip in the new mode. 0 when the new mode showed on the frame
    /// that asked for it.
    pub fn took_frames(&self) -> Option<u64> {
        self.shown_frame
            .map(|shown| shown.saturating_sub(self.asked_frame))
    }
}

impl SwitchTrace {
    /// An empty trace with room for [`TRACE_CAPACITY`] records.
    pub fn new() -> Self {
        Self {
            records: VecDeque::with_capacity(TRACE_CAPACITY),
            total: 0,
            modesets: 0,
        }
    }

    /// The records, oldest first.
    pub fn records(&self) -> impl ExactSizeIterator<Item = &SwitchRecord> {
        self.records.iter()
    }

    /// The newest record.
    pub fn last(&self) -> Option<&SwitchRecord> {
        self.records.back()
    }

    /// How many records were ever pushed, including the ones that left. A
    /// scale-only decision merged into the record before it adds none.
    pub fn total(&self) -> u64 {
        self.total
    }

    /// How many [`Outcome::Modeset`] records were ever pushed, including
    /// the ones that left the trace.
    pub fn modeset_total(&self) -> u64 {
        self.modesets
    }

    /// The modesets in the trace, oldest first.
    pub fn modesets(&self) -> impl Iterator<Item = &SwitchRecord> {
        self.records
            .iter()
            .filter(|r| r.outcome == Outcome::Modeset)
    }

    pub(crate) fn push(&mut self, record: SwitchRecord) {
        if record.outcome == Outcome::ScaleOnly
            && let Some(last) = self.records.back_mut()
            && last.outcome == Outcome::ScaleOnly
        {
            let count = last.count.saturating_add(record.count);
            *last = SwitchRecord { count, ..record };
            return;
        }
        if self.records.len() == TRACE_CAPACITY {
            self.records.pop_front();
        }
        if record.outcome == Outcome::Modeset {
            self.modesets += 1;
        }
        self.records.push_back(record);
        self.total += 1;
    }

    /// The record that was pushed as number `number` (from 1, the value of
    /// [`SwitchTrace::total`] after its push), if it is still in the trace.
    pub(crate) fn by_number_mut(&mut self, number: u64) -> Option<&mut SwitchRecord> {
        let back = usize::try_from(self.total.checked_sub(number)?).ok()?;
        let len = self.records.len();
        let index = len.checked_sub(back.checked_add(1)?)?;
        self.records.get_mut(index)
    }
}

impl Default for SwitchTrace {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for Trigger {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Launch => "launch",
            Self::Geometry => "geometry",
            Self::Monitor => "monitor",
            Self::Settings => "settings",
            Self::Retry => "retry",
        })
    }
}

impl fmt::Display for Outcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ScaleOnly => f.write_str("scale only"),
            Self::Modeset => f.write_str("modeset"),
            Self::Kept(reason) => write!(f, "kept the mode: {reason}"),
            Self::SetModeFailed(error) => write!(f, "set_mode failed: {error}"),
            Self::ReadBackRefused { read_back } => write!(
                f,
                "read back {}x{}{} refused",
                read_back.hactive,
                read_back.vactive,
                scan(read_back)
            ),
            Self::Restored => f.write_str("last good mode set again"),
            Self::Blanked => f.write_str("output off: no valid mode"),
            Self::BlankFailed(error) => write!(f, "blank failed: {error}"),
        }
    }
}

/// One line for the log: the frames, the trigger, the source, the outcome
/// and the winner.
impl fmt::Display for SwitchRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = &self.source;
        write!(
            f,
            "frame {} (asked {}): {} {}x{}{} at {} mHz: {}",
            self.decided_frame,
            self.asked_frame,
            self.trigger,
            s.width,
            s.height,
            if s.interlaced { "i" } else { "p" },
            s.refresh_mhz,
            self.outcome
        )?;
        if let Some(w) = &self.winner {
            let m = w.token.mode();
            write!(
                f,
                ", {} {}x{}{} at {} mHz",
                w.rung,
                m.hactive,
                m.vactive,
                scan(m),
                m.field_rate_mhz()
            )?;
        }
        if let Some(drift) = self.rate_drift_mhz {
            write!(f, ", read back {drift} mHz off")?;
        }
        if self.count > 1 {
            write!(f, ", {} decisions", self.count)?;
        }
        if let Some(took) = self.took_frames() {
            write!(f, ", shown after {took} frames")?;
        }
        Ok(())
    }
}

fn scan(mode: &Modeline) -> &'static str {
    if mode.interlace { "i" } else { "p" }
}
