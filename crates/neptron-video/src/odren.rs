use neptron_backend::DisplayError;
use neptron_modeline::policy::{PolicySettings, Proposal, Switch, switch_for};
use neptron_safety::{Rejection, ValidatedMode, validate};
use neptron_types::{FlipEvent, Frame, GpuCaps, Modeline, Scene, SourceGeometry};

use crate::cache::Cache;
use crate::decision::{Decision, Inputs};
use crate::{
    Chosen, Monitor, Outcome, READ_BACK_RATE_NOTICE_MHZ, ScanlineSettings, SceneInputs,
    StatusInputs, SwitchRecord, SwitchTrace, Trigger, Unshowable, Veto, VideoStatusText,
    build_scene, write_video_status,
};

/// A source that asks for a new mode must stay the same for this many core
/// frames before the engine sets the mode. A core that flickers between
/// two sizes then causes no modeset.
pub const DEBOUNCE_FRAMES: u32 = 3;

/// How many times the engine asks for a modeset that a monitor, settings
/// or retry change needs, when `set_mode` fails on the device. A busy or
/// transient device error clears within three frames; a lasting one must
/// not cost a `set_mode` call on every frame. After the last attempt the
/// mode on screen stays, and `current_refused` tells whether the inputs
/// refuse it.
pub const MODESET_ATTEMPTS: u32 = 3;

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
    /// Scanlines change the scene only, never the mode.
    scanlines: ScanlineSettings,
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
    /// Device errors of the modeset that the last input change asked for.
    failed_attempts: u32,
    frame: u64,
    /// The mode on the display is one the inputs refuse, or one the engine
    /// does not know after a failed restore.
    refused_on_screen: bool,
    muted: bool,
    /// The last good mode that the executor sets again.
    restoring: Option<Modeline>,
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
            scanlines: ScanlineSettings::OFF,
            cache: Cache::new(),
            current: None,
            request: None,
            redo: None,
            commit: None,
            blocked: None,
            failed_attempts: 0,
            frame: 0,
            refused_on_screen: false,
            muted: false,
            restoring: None,
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

    /// Lifts the block of a source whose `set_mode` failed on the device,
    /// and the count of failed attempts. The next frame decides again with
    /// no debounce. The session calls it when the player asks (for example
    /// when the quick menu closes); the engine never retries a blocked
    /// source by itself.
    pub fn retry(&mut self) {
        self.blocked = None;
        self.failed_attempts = 0;
        self.redo = Some(Trigger::Retry);
    }

    /// New scanline settings. The next scene takes them; no modeset, no
    /// debounce and no trace record.
    pub fn set_scanlines(&mut self, scanlines: ScanlineSettings) {
        self.scanlines = scanlines;
    }

    /// The scanline settings.
    pub fn scanlines(&self) -> ScanlineSettings {
        self.scanlines
    }

    /// The scene of the mode on screen, or `None` before the first mode.
    pub fn scene<'a>(&self, game: Option<Frame<'a>>, ui: Option<Frame<'a>>) -> Option<Scene<'a>> {
        let current = self.current.as_ref()?;
        let inputs = SceneInputs {
            placement: &current.chosen.proposal.placement,
            source: &current.source,
            monitor: &self.inputs.monitor.profile().kind,
            mode: &current.read_back,
            scanlines: self.scanlines,
        };
        Some(build_scene(&inputs, game, ui))
    }

    /// The scene that `proposal` would show for `source`, for the preview
    /// of the Display panel: the same function, monitor and scanline
    /// settings as the run.
    pub fn preview_scene<'a>(
        &self,
        proposal: &Proposal,
        source: &SourceGeometry,
        game: Option<Frame<'a>>,
        ui: Option<Frame<'a>>,
    ) -> Scene<'a> {
        let inputs = SceneInputs {
            placement: &proposal.placement,
            source,
            monitor: &self.inputs.monitor.profile().kind,
            mode: &proposal.mode,
            scanlines: self.scanlines,
        };
        build_scene(&inputs, game, ui)
    }

    /// Writes the video status of the mode on screen into `out`, from its
    /// read back. `None` before the first mode is on screen.
    pub fn video_status<'o>(&self, out: &'o mut VideoStatusText) -> Option<&'o VideoStatusText> {
        let inputs = self.current.as_ref().map(StatusInputs::of);
        write_video_status(inputs.as_ref(), out)
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

    /// The mode on the display is one that the current inputs refuse, or
    /// one the engine does not know (a read back it refused, with no good
    /// mode to restore). It stays set until a valid mode is on screen. The
    /// session and the guard must read it: the engine cannot blank the
    /// output by itself.
    pub fn current_refused(&self) -> bool {
        self.refused_on_screen
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
        if let Some(stale) = self.commit.take() {
            self.keep_trigger(stale.record.trigger);
        }
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
                    outcome: Outcome::Kept(reason.unwrap_or(Unshowable::NoWinner)),
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
                self.refused_on_screen = false;
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
                let drift = requested
                    .field_rate_mhz()
                    .abs_diff(read_back.field_rate_mhz());
                self.trace.push(SwitchRecord {
                    read_back: Some(*read_back),
                    rate_drift_mhz: (drift >= READ_BACK_RATE_NOTICE_MHZ).then_some(drift),
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
                self.failed_attempts = 0;
                self.refused_on_screen = false;
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
                self.refused_on_screen = true;
                self.restore_order(commit.record.trigger)
            }
            Err(error) => {
                if matches!(error, DisplayError::ModeRefused { .. }) {
                    self.refused_by_driver(requested);
                } else {
                    self.after_device_error(commit.record.trigger, commit.source);
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

    /// The last good mode, validated again with the current inputs, to set
    /// after a refused read back. With no good mode, or one the inputs now
    /// refuse, the engine sets nothing and forgets its mode: the display
    /// holds a mode it does not know, and `current_refused` says so.
    fn restore_order(&mut self, trigger: Trigger) -> AfterModeSet {
        let Some(current) = &self.current else {
            return AfterModeSet::Done;
        };
        match self.inputs.check(current.chosen.token.mode()) {
            Ok(token) => {
                self.restoring = Some(*token.mode());
                AfterModeSet::Restore(ModeOrder {
                    token,
                    generation: self.generation,
                })
            }
            Err(refusal) => {
                let request = Request {
                    source: current.source,
                    asked_frame: self.frame,
                    stable: 1,
                };
                let record = self.record(trigger, request, None, None);
                self.trace.push(SwitchRecord {
                    outcome: Outcome::Kept(Unshowable::CurrentRefused(refusal)),
                    ..record
                });
                self.current = None;
                AfterModeSet::Done
            }
        }
    }

    /// Takes the result of setting the last good mode again. Its read back
    /// must hold as for a modeset. Otherwise the mode on the display is
    /// not known, so the engine forgets its mode and decides again on the
    /// next frame.
    pub(crate) fn on_restore(&mut self, result: &Result<Modeline, DisplayError>) {
        let restoring = self.restoring.take();
        let held = match (result, restoring) {
            (Ok(read_back), Some(asked)) => self.read_back_holds(&asked, read_back),
            _ => false,
        };
        if let (true, Ok(read_back), Some(current)) = (held, result, &mut self.current) {
            current.read_back = *read_back;
            self.awaiting_flip = Some(self.trace.total());
            self.refused_on_screen = false;
            return;
        }
        if let (Ok(read_back), Some(last)) = (result, self.trace.last()) {
            let record = SwitchRecord {
                read_back: Some(*read_back),
                outcome: Outcome::ReadBackRefused {
                    read_back: *read_back,
                },
                winner: None,
                dropped: Vec::new(),
                refused: Vec::new(),
                ..last.clone()
            };
            self.trace.push(record);
        }
        self.refused_on_screen = true;
        self.current = None;
        self.awaiting_flip = None;
        self.muted = false;
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

    /// A modeset of a monitor, settings or retry change that did not reach
    /// the display is decided again on the next frame. A geometry or
    /// launch modeset needs no help: its source asks again.
    fn keep_trigger(&mut self, trigger: Trigger) {
        if !matches!(trigger, Trigger::Geometry | Trigger::Launch) && self.redo.is_none() {
            self.redo = Some(trigger);
        }
    }

    /// A geometry or launch modeset blocks its source. Any other is tried
    /// again, up to [`MODESET_ATTEMPTS`] in all.
    fn after_device_error(&mut self, trigger: Trigger, source: SourceGeometry) {
        if matches!(trigger, Trigger::Geometry | Trigger::Launch) {
            self.blocked = Some(source);
            return;
        }
        self.failed_attempts += 1;
        if self.failed_attempts < MODESET_ATTEMPTS {
            self.keep_trigger(trigger);
        } else {
            self.blocked = Some(source);
        }
    }

    fn invalidate(&mut self, trigger: Trigger) {
        self.failed_attempts = 0;
        self.generation += 1;
        self.commit = None;
        self.cache.clear();
        self.blocked = None;
        self.request = None;
        self.redo = Some(trigger);
        self.check_on_screen(trigger);
    }

    /// Judges the mode on the display with the new inputs, and records a
    /// refusal.
    fn check_on_screen(&mut self, trigger: Trigger) {
        let Some(current) = &self.current else {
            return;
        };
        let Err(refusal) = self.inputs.check(&current.read_back) else {
            self.refused_on_screen = false;
            return;
        };
        self.refused_on_screen = true;
        let request = Request {
            source: current.source,
            asked_frame: self.frame,
            stable: 1,
        };
        let record = self.record(trigger, request, None, None);
        self.trace.push(SwitchRecord {
            outcome: Outcome::Kept(Unshowable::CurrentRefused(refusal)),
            ..record
        });
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
            count: 1,
            shown_frame: None,
            gap_ns: None,
            trigger,
            source: request.source,
            previous: self.current.as_ref().map(|c| *c.chosen.token.mode()),
            winner,
            read_back: None,
            rate_drift_mhz: None,
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
