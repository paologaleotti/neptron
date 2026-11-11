use neptron_backend::DisplayError;
use neptron_modeline::policy::{PolicySettings, Switch, switch_for};
use neptron_safety::{Rejection, ValidatedMode, validate};
use neptron_types::{FlipEvent, GpuCaps, Modeline, SourceGeometry};

use crate::cache::Cache;
use crate::decision::{Decision, Inputs};
use crate::{Chosen, Monitor, Outcome, SwitchRecord, SwitchTrace, Trigger, Veto};

/// A source that asks for a new mode must stay the same for this many core
/// frames before the engine sets the mode. A core that flickers between
/// two sizes then causes no modeset.
pub const DEBOUNCE_FRAMES: u32 = 3;

/// The mode on screen and the game's place in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Current {
    /// The winner that the engine set, with its token and its placement.
    pub chosen: Chosen,
    /// The mode the driver read back. Its field rate is the real rate for
    /// frame pacing and audio rate control.
    pub read_back: Modeline,
    /// The source the placement is for.
    pub source: SourceGeometry,
}

/// What a core frame does to the display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Directive {
    /// Nothing changes.
    Steady,
    /// The winner equals the mode on screen: only crop, destination and
    /// filter change, at once.
    Rescale,
    /// The source asks for a new mode, and the debounce waits.
    Waiting {
        /// Core frames the source has been the same, this one included.
        stable_frames: u32,
    },
    /// The next present sets a new mode. Mute the audio first.
    Modeset,
    /// The engine has no mode for the source, and the mode on screen stays.
    /// The trace has the reason.
    Kept,
}

/// What the display does after a `set_mode` result.
#[derive(Debug, Clone)]
pub(crate) enum AfterModeSet {
    Done,
    /// Set this mode again: the last good one.
    Restore(ModeOrder),
}

/// A token and the generation of the inputs it was validated for. Only
/// [`Odren::authorize`] gives the token out, and only while the inputs are
/// that generation, so a token from before a change of the monitor, the
/// settings, the veto or the driver refusals never reaches `set_mode`.
#[derive(Debug, Clone)]
pub(crate) struct ModeOrder {
    token: ValidatedMode,
    generation: u64,
}

/// The ODREN state machine: debounce, decision, commit and trace. It does
/// no I/O and holds no display: it takes the source of each core frame and
/// the results of the display calls, and says what to do next.
pub struct Odren {
    inputs: Inputs,
    /// Counts the changes of the inputs that judge a mode.
    generation: u64,
    cache: Cache,
    current: Option<Current>,
    request: Option<Request>,
    /// Decide on the next frame with no debounce, for this reason.
    redo: Option<Trigger>,
    commit: Option<Commit>,
    /// A source whose `set_mode` failed on the device. The engine does not
    /// try it again until the source on screen comes back, or the monitor
    /// or the settings change.
    blocked: Option<SourceGeometry>,
    frame: u64,
    muted: bool,
    /// The trace number of the modeset that waits for its first flip.
    awaiting_flip: Option<u64>,
    last_flip_ns: Option<u64>,
    trace: SwitchTrace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Request {
    source: SourceGeometry,
    asked_frame: u64,
    stable: u32,
}

#[derive(Debug)]
struct Commit {
    chosen: Chosen,
    generation: u64,
    source: SourceGeometry,
    record: SwitchRecord,
}

impl Odren {
    /// A machine with no mode on screen. `gpu` is the display's caps; the
    /// monitor's limit is applied here.
    pub fn new(gpu: &GpuCaps, monitor: Monitor, settings: PolicySettings) -> Self {
        Self {
            inputs: Inputs {
                caps: monitor.caps_for(gpu),
                monitor,
                settings,
                veto: None,
                driver_refused: Vec::new(),
            },
            generation: 0,
            cache: Cache::new(),
            current: None,
            request: None,
            redo: None,
            commit: None,
            blocked: None,
            frame: 0,
            muted: false,
            awaiting_flip: None,
            last_flip_ns: None,
            trace: SwitchTrace::new(),
        }
    }

    /// Adds a check that runs before the validator. It can only refuse
    /// more modes.
    pub fn set_veto(&mut self, veto: Veto) {
        self.inputs.veto = Some(veto);
        self.invalidate(Trigger::Settings);
    }

    /// A new monitor. The next frame decides again with no debounce.
    pub fn set_monitor(&mut self, gpu: &GpuCaps, monitor: Monitor) {
        self.inputs.caps = monitor.caps_for(gpu);
        self.inputs.monitor = monitor;
        self.inputs.driver_refused.clear();
        self.invalidate(Trigger::Monitor);
    }

    /// New policy settings. The next frame decides again with no debounce.
    pub fn set_settings(&mut self, settings: PolicySettings) {
        if settings != self.inputs.settings {
            self.inputs.settings = settings;
            self.invalidate(Trigger::Settings);
        }
    }

    /// The monitor.
    pub fn monitor(&self) -> &Monitor {
        &self.inputs.monitor
    }

    /// The policy settings.
    pub fn settings(&self) -> &PolicySettings {
        &self.inputs.settings
    }

    /// The caps the policy and the validator get.
    pub fn caps(&self) -> &GpuCaps {
        &self.inputs.caps
    }

    /// The mode on screen, or `None` before the first modeset.
    pub fn current(&self) -> Option<&Current> {
        self.current.as_ref()
    }

    /// The switch trace.
    pub fn trace(&self) -> &SwitchTrace {
        &self.trace
    }

    /// The number of the last core frame, from 1.
    pub fn frame(&self) -> u64 {
        self.frame
    }

    /// Audio must be silent now: a modeset is due or the new mode has not
    /// flipped yet.
    pub fn muted(&self) -> bool {
        self.muted
    }

    /// The token of the modeset that the next present sets.
    pub fn pending_modeset(&self) -> Option<&ValidatedMode> {
        self.commit.as_ref().map(|c| &c.chosen.token)
    }

    /// The modeset that the next present sets, for the executor.
    pub(crate) fn pending_order(&self) -> Option<ModeOrder> {
        self.commit.as_ref().map(|c| ModeOrder {
            token: c.chosen.token.clone(),
            generation: c.generation,
        })
    }

    /// The token of `order`, or `None` when the inputs changed since it was
    /// validated.
    pub(crate) fn authorize<'o>(&self, order: &'o ModeOrder) -> Option<&'o ValidatedMode> {
        (order.generation == self.generation).then_some(&order.token)
    }

    /// Takes the source of one core frame: debounce, decision, and what the
    /// display does. Call it once per core frame, not once per flip.
    pub fn on_frame(&mut self, source: SourceGeometry) -> Directive {
        self.frame += 1;
        self.commit = None;
        let directive = self.step(source);
        if directive == Directive::Modeset {
            self.muted = true;
        } else if self.awaiting_flip.is_none() {
            self.muted = false;
        }
        directive
    }

    fn step(&mut self, source: SourceGeometry) -> Directive {
        let trigger = match (self.redo.take(), &self.current) {
            (Some(trigger), _) => trigger,
            (None, None) => Trigger::Launch,
            (None, Some(current)) if current.source == source => {
                self.request = None;
                self.blocked = None;
                return Directive::Steady;
            }
            (None, Some(_)) => Trigger::Geometry,
        };
        let request = match &mut self.request {
            Some(r) if r.source == source => {
                r.stable += 1;
                *r
            }
            slot => *slot.insert(Request {
                source,
                asked_frame: self.frame,
                stable: 1,
            }),
        };
        let debounced = matches!(trigger, Trigger::Geometry | Trigger::Launch);
        if debounced && self.blocked == Some(source) {
            return Directive::Kept;
        }
        let inputs = &self.inputs;
        let index = self.cache.find_or_insert(source, || inputs.decide(&source));
        let winner = self.cache.get(index).and_then(|d| d.winner.clone());
        let Some(winner) = winner else {
            if !debounced || request.stable == 1 {
                let reason = self.decision(index).and_then(Decision::unshowable_reason);
                let record = self.record(trigger, request, None, Some(index));
                self.trace.push(SwitchRecord {
                    outcome: Outcome::Kept(reason.unwrap_or(crate::Unshowable::NoWinner)),
                    ..record
                });
            }
            return Directive::Kept;
        };
        let current = self.current.as_ref().map(|c| c.chosen.token.mode());
        match switch_for(current, winner.token.mode()) {
            Switch::ScaleOnly => {
                let record = self.record(trigger, request, Some(winner.clone()), None);
                if let Some(current) = &mut self.current {
                    current.chosen = winner;
                    current.source = source;
                }
                self.request = None;
                self.trace.push(SwitchRecord {
                    outcome: Outcome::ScaleOnly,
                    ..record
                });
                Directive::Rescale
            }
            Switch::Modeset if trigger == Trigger::Geometry && request.stable < DEBOUNCE_FRAMES => {
                Directive::Waiting {
                    stable_frames: request.stable,
                }
            }
            Switch::Modeset => {
                let record = self.record(trigger, request, Some(winner.clone()), Some(index));
                self.commit = Some(Commit {
                    chosen: winner,
                    generation: self.generation,
                    source,
                    record,
                });
                Directive::Modeset
            }
        }
    }

    /// Takes the result of `set_mode` for the pending modeset.
    pub(crate) fn on_mode_set(&mut self, result: &Result<Modeline, DisplayError>) -> AfterModeSet {
        let Some(commit) = self.commit.take() else {
            return AfterModeSet::Done;
        };
        let requested = *commit.chosen.token.mode();
        match result {
            Ok(read_back) if self.read_back_holds(&requested, read_back) => {
                self.trace.push(SwitchRecord {
                    read_back: Some(*read_back),
                    outcome: Outcome::Modeset,
                    ..commit.record
                });
                self.awaiting_flip = Some(self.trace.total());
                self.current = Some(Current {
                    chosen: commit.chosen,
                    read_back: *read_back,
                    source: commit.source,
                });
                self.request = None;
                self.blocked = None;
                AfterModeSet::Done
            }
            Ok(read_back) => {
                self.refused_by_driver(requested);
                self.trace.push(SwitchRecord {
                    read_back: Some(*read_back),
                    outcome: Outcome::ReadBackRefused {
                        read_back: *read_back,
                    },
                    ..commit.record
                });
                match &self.current {
                    Some(current) => AfterModeSet::Restore(ModeOrder {
                        token: current.chosen.token.clone(),
                        generation: self.generation,
                    }),
                    None => AfterModeSet::Done,
                }
            }
            Err(error) => {
                if matches!(error, DisplayError::ModeRefused { .. }) {
                    self.refused_by_driver(requested);
                } else {
                    self.blocked = Some(commit.source);
                }
                self.trace.push(SwitchRecord {
                    outcome: Outcome::SetModeFailed(error.clone()),
                    ..commit.record
                });
                self.muted = false;
                AfterModeSet::Done
            }
        }
    }

    /// Takes the result of setting the last good mode again. On an error
    /// the mode on the display is not known, so the engine forgets its
    /// mode and decides again on the next frame.
    pub(crate) fn on_restore(&mut self, result: &Result<Modeline, DisplayError>) {
        match (result, &mut self.current) {
            (Ok(read_back), Some(current)) => {
                current.read_back = *read_back;
                self.awaiting_flip = Some(self.trace.total());
            }
            _ => {
                self.current = None;
                self.awaiting_flip = None;
                self.muted = false;
            }
        }
    }

    /// Takes a completed flip. The first flip in a new mode ends the mute.
    pub(crate) fn on_flip(&mut self, event: &FlipEvent) {
        if let Some(id) = self.awaiting_flip.take() {
            let gap_ns = self.last_flip_ns.map(|t| event.time_ns.saturating_sub(t));
            let frame = self.frame;
            if let Some(record) = self.trace.by_number_mut(id) {
                record.shown_frame = Some(frame);
                record.gap_ns = gap_ns;
            }
            if self.commit.is_none() {
                self.muted = false;
            }
        }
        self.last_flip_ns = Some(event.time_ns);
    }

    /// The picture style rows of the Display panel, from the same policy,
    /// caps and validator.
    pub fn style_rows(
        &self,
        source: &SourceGeometry,
    ) -> Result<Vec<neptron_modeline::policy::StyleRow<crate::Refusal>>, crate::Unshowable> {
        self.inputs.style_rows(source)
    }

    fn invalidate(&mut self, trigger: Trigger) {
        self.generation += 1;
        self.commit = None;
        self.cache.clear();
        self.blocked = None;
        self.request = None;
        self.redo = Some(trigger);
    }

    fn refused_by_driver(&mut self, mode: Modeline) {
        if !self.inputs.driver_refused.contains(&mode) {
            self.inputs.driver_refused.push(mode);
        }
        self.generation += 1;
        self.commit = None;
        self.cache.clear();
        self.redo = Some(Trigger::Retry);
    }

    /// The driver may round the pixel clock. It must keep the size and the
    /// scan, and the mode it reports must still pass the validator.
    fn read_back_holds(&self, requested: &Modeline, read_back: &Modeline) -> bool {
        let same_shape = read_back.hactive == requested.hactive
            && read_back.vactive == requested.vactive
            && read_back.interlace == requested.interlace
            && read_back.doublescan == requested.doublescan;
        same_shape && self.check_read_back(read_back).is_ok()
    }

    fn check_read_back(&self, read_back: &Modeline) -> Result<ValidatedMode, Rejection> {
        if let Some(veto) = &self.inputs.veto {
            veto(read_back)?;
        }
        validate(read_back, self.inputs.monitor.profile(), &self.inputs.caps)
    }

    fn decision(&self, index: usize) -> Option<&Decision> {
        self.cache.get(index)
    }

    fn record(
        &self,
        trigger: Trigger,
        request: Request,
        winner: Option<Chosen>,
        reasons_from: Option<usize>,
    ) -> SwitchRecord {
        let decision = reasons_from.and_then(|i| self.decision(i));
        SwitchRecord {
            asked_frame: request.asked_frame,
            decided_frame: self.frame,
            shown_frame: None,
            gap_ns: None,
            trigger,
            source: request.source,
            previous: self.current.as_ref().map(|c| *c.chosen.token.mode()),
            winner,
            read_back: None,
            outcome: Outcome::ScaleOnly,
            dropped: decision.map(Decision::dropped).unwrap_or_default(),
            refused: decision.map(|d| d.refused.clone()).unwrap_or_default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use neptron_testkit::preset::{CAPS_LOW_CLOCK, monitor_preset};
    use neptron_types::Rotation;

    use super::*;

    const SNES: SourceGeometry = SourceGeometry {
        width: 256,
        height: 224,
        refresh_mhz: 60_099,
        aspect_milli: 1_333,
        interlaced: false,
        rotation: Rotation::R0,
    };

    fn monitor(id: &str) -> Monitor {
        Monitor::Profile(monitor_preset(id).unwrap())
    }

    #[test]
    fn an_order_from_older_inputs_is_not_authorized() {
        let mut odren = Odren::new(
            &CAPS_LOW_CLOCK,
            monitor("pc_31_120"),
            PolicySettings::default(),
        );
        assert_eq!(odren.on_frame(SNES), Directive::Modeset);
        let order = odren.pending_order().unwrap();
        assert!(odren.authorize(&order).is_some());
        odren.set_monitor(&CAPS_LOW_CLOCK, monitor("pc_31_120"));
        assert!(odren.authorize(&order).is_none());
        assert!(odren.pending_order().is_none());
    }
}
