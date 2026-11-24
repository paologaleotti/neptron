use neptron_backend::{Display, DisplayError};
use neptron_modeline::policy::{Offer, PictureChoices, PolicySettings, Proposal};

use crate::answers::AnswerRequest;
use neptron_types::{FlipEvent, Frame, Modeline, Orientation, Scene, SourceGeometry, Turns};

use crate::odren::{AfterModeSet, ModeOrder};
use crate::{
    Content, Current, Directive, Monitor, Odren, Outcome, ScanlineSettings, SwitchTrace,
    Unshowable, Veto, VideoStatusText,
};

/// An error of the engine. The trace has the details of every failed
/// switch.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum VideoError {
    /// No mode is set, so there is nothing to present on: no frame came
    /// yet, the engine has no mode for the source, the last modeset
    /// failed with no mode before it, or the output is off after a blank.
    #[error("no mode is set")]
    NoMode,
    /// `wait_flip` was called with no flip queued.
    #[error("no flip is queued")]
    NoFlipPending,
    /// The display refused or failed the present. No flip is queued.
    #[error("present failed: {0}")]
    Present(DisplayError),
    /// The wait for the flip failed. The flip stays queued until a wait
    /// returns its event: the display never drops a queued flip.
    #[error("wait_flip failed: {0}")]
    WaitFlip(DisplayError),
    /// The flip of the last present is still queued, and the wait for it
    /// failed. Nothing was set or queued; a modeset waits for the next
    /// frame.
    #[error("the queued flip did not complete: {0}")]
    PendingFlip(DisplayError),
    /// The driver read back a mode that the engine refuses, and setting
    /// the last good mode again failed. The mode on the display is not
    /// known; the engine decides again on the next frame.
    #[error("the driver read back {read_back:?}, and the last good mode failed: {error}")]
    RestoreFailed {
        /// The mode the driver read back.
        read_back: Modeline,
        /// The error of setting the last good mode.
        error: DisplayError,
    },
}

/// What [`VideoEngine::present`] did.
#[derive(Debug, Clone, PartialEq)]
pub struct Presented {
    /// A flip of the previous present that the engine waited for first,
    /// because the caller had not.
    pub drained_flip: Option<FlipEvent>,
    /// The outcome of the modeset of this present, if there was one:
    /// [`Outcome::Modeset`], [`Outcome::SetModeFailed`] or
    /// [`Outcome::ReadBackRefused`].
    pub modeset: Option<Outcome>,
}

/// The ODREN engine on a display: the [`Odren`] state machine, and the
/// calls it asks for on a [`Display`].
///
/// One core frame is: [`VideoEngine::begin_frame`] with the source, then
/// for each of its flips (two for double refresh) [`VideoEngine::present`]
/// and [`VideoEngine::wait_flip`]. Read [`VideoEngine::muted`] after each
/// call and set the audio to it: it turns on in `begin_frame`, before the
/// modeset in `present`, and off at the first flip in the new mode.
pub struct VideoEngine<D: Display> {
    display: D,
    odren: Odren,
    /// A flip is queued on the display. It stays queued until a wait
    /// returns its event, because the display refuses `set_mode` and
    /// `present` as busy until then.
    flip_pending: bool,
}

impl<D: Display> VideoEngine<D> {
    /// An engine with no mode set. The first frame sets one with no
    /// debounce.
    pub fn new(display: D, monitor: Monitor, settings: PolicySettings) -> Self {
        let odren = Odren::new(display.caps(), monitor, settings);
        Self {
            display,
            odren,
            flip_pending: false,
        }
    }

    /// Adds a check that runs before the validator. It can only refuse
    /// more modes.
    pub fn set_veto(&mut self, veto: Veto) {
        self.odren.set_veto(veto);
    }

    /// A new monitor, for example a profile from the Display panel. The
    /// next frame decides again with no debounce.
    pub fn set_monitor(&mut self, monitor: Monitor) {
        self.odren.set_monitor(self.display.caps(), monitor);
    }

    /// New policy settings. The next frame decides again with no debounce.
    pub fn set_settings(&mut self, settings: PolicySettings) {
        self.odren.set_settings(settings);
    }

    /// New scanline settings. The next scene takes them, with no modeset.
    pub fn set_scanlines(&mut self, scanlines: ScanlineSettings) {
        self.odren.set_scanlines(scanlines);
    }

    /// The screen orientation and the game override, for this engine and
    /// for a test harness with no app. The default is
    /// [`Orientation::NORMAL`], with which every picture is the picture of
    /// a display with no orientation. See [`Odren::set_orientation`].
    pub fn set_orientation(&mut self, orientation: Orientation) {
        self.odren.set_orientation(orientation);
    }

    /// The screen orientation and the game override.
    pub fn orientation(&self) -> Orientation {
        self.odren.orientation()
    }

    /// What the engine shows. The menu engine sets
    /// `Content::Menu(TURNED_MENU_MODE)`. See [`Odren::set_content`].
    pub fn set_content(&mut self, content: Content) {
        self.odren.set_content(content);
    }

    /// The turns of a game whose core source is `source`. See
    /// [`Odren::turns`].
    pub fn turns(&self, source: &SourceGeometry) -> Turns {
        self.odren.turns(source)
    }

    /// The scene that `proposal` would show for `source`, for the preview
    /// of the Display panel. See [`Odren::preview_scene`].
    pub fn preview_scene<'a>(
        &self,
        proposal: &Proposal,
        source: &SourceGeometry,
        game: Option<Frame<'a>>,
        ui: Option<Frame<'a>>,
    ) -> Scene<'a> {
        self.odren.preview_scene(proposal, source, game, ui)
    }

    /// Writes the video status of the mode on screen into `out`. See
    /// [`Odren::video_status`].
    pub fn video_status<'o>(&self, out: &'o mut VideoStatusText) -> Option<&'o VideoStatusText> {
        self.odren.video_status(out)
    }

    /// Lifts the block of a source after a device error. See
    /// [`Odren::retry`].
    pub fn retry(&mut self) {
        self.odren.retry();
    }

    /// Takes the source of one core frame. Does no display call.
    pub fn begin_frame(&mut self, source: SourceGeometry) -> Directive {
        self.odren.on_frame(source)
    }

    /// Sets the mode that `begin_frame` asked for, if any, then queues one
    /// flip of the game frame and the UI canvas.
    ///
    /// A failed modeset keeps the mode on screen; the result and the trace
    /// say so, and the frame is presented in the old mode. When no valid
    /// mode exists ([`Directive::Blank`], or a refused read back with no good
    /// restore), it turns the output off with `Display::blank` and returns
    /// [`VideoError::NoMode`]; [`VideoEngine::blanked`] and the trace say
    /// so.
    ///
    /// # Errors
    ///
    /// [`VideoError::NoMode`] when no mode is set,
    /// [`VideoError::PendingFlip`] or [`VideoError::RestoreFailed`]
    /// as their docs say, and [`VideoError::Present`] when the display
    /// refuses the scene.
    pub fn present(
        &mut self,
        game: Option<Frame<'_>>,
        ui: Option<Frame<'_>>,
    ) -> Result<Presented, VideoError> {
        let mut presented = Presented {
            drained_flip: None,
            modeset: None,
        };
        presented.drained_flip = self.drain_flip()?;
        let modeset = self.modeset();
        self.blank_if_due();
        presented.modeset = modeset?;
        let scene = self.odren.scene(game, ui).ok_or(VideoError::NoMode)?;
        self.display.present(&scene).map_err(VideoError::Present)?;
        self.flip_pending = true;
        Ok(presented)
    }

    /// Waits for the flip of the last present.
    ///
    /// # Errors
    ///
    /// [`VideoError::NoFlipPending`] when no flip is queued, and
    /// [`VideoError::WaitFlip`] when the display fails.
    pub fn wait_flip(&mut self) -> Result<FlipEvent, VideoError> {
        if !self.flip_pending {
            return Err(VideoError::NoFlipPending);
        }
        self.wait_queued().map_err(VideoError::WaitFlip)
    }

    /// Audio must be silent now.
    pub fn muted(&self) -> bool {
        self.odren.muted()
    }

    /// The mode on the display is refused by the current inputs or not
    /// known. See [`Odren::current_refused`].
    pub fn current_refused(&self) -> bool {
        self.odren.current_refused()
    }

    /// A source is blocked after a device error of its `set_mode`. See
    /// [`Odren::source_blocked`].
    pub fn source_blocked(&self) -> bool {
        self.odren.source_blocked()
    }

    /// The output is off: no valid mode existed, and the engine called
    /// `Display::blank`. See [`Odren::blanked`].
    pub fn blanked(&self) -> bool {
        self.odren.blanked()
    }

    /// The mode on screen.
    pub fn current(&self) -> Option<&Current> {
        self.odren.current()
    }

    /// The switch trace.
    pub fn trace(&self) -> &SwitchTrace {
        self.odren.trace()
    }

    /// The state machine.
    pub fn odren(&self) -> &Odren {
        &self.odren
    }

    /// The display.
    pub fn display(&self) -> &D {
        &self.display
    }

    /// The answers of `neptron_modeline::policy::offer` for the options of
    /// the picture mode and the refresh rows, from the same policy, caps,
    /// validator and scanlines as the run.
    ///
    /// # Errors
    ///
    /// [`Unshowable`] when the policy gives no plan.
    pub fn picture_choices(&self, source: &SourceGeometry) -> Result<PictureChoices, Unshowable> {
        self.odren.picture_choices(source)
    }

    /// [`Self::picture_choices`] for other settings and scanlines than the
    /// run's: the menu of a level whose values the engine does not run.
    ///
    /// # Errors
    ///
    /// [`Unshowable`] when the policy gives no plan.
    pub fn picture_choices_with(
        &self,
        source: &SourceGeometry,
        settings: &PolicySettings,
        scanlines: ScanlineSettings,
    ) -> Result<PictureChoices, Unshowable> {
        self.odren.answers_now(&AnswerRequest {
            sources: vec![*source],
            settings: *settings,
            scanlines,
        })
    }

    /// The answers for every game of `sources`, as one answer by
    /// `neptron_modeline::policy::for_every_game`: the menu of a level that
    /// every game reads. A source with no plan is left out. `None` when no
    /// source has a plan.
    pub fn picture_choices_for_every_game(
        &self,
        sources: &[SourceGeometry],
        settings: &PolicySettings,
        scanlines: ScanlineSettings,
    ) -> Option<PictureChoices> {
        self.odren
            .answers_now(&AnswerRequest {
                sources: sources.to_vec(),
                settings: *settings,
                scanlines,
            })
            .ok()
    }

    /// The request of the answers for `source` with the settings and the
    /// scanlines of the run.
    pub fn request_for(&self, source: &SourceGeometry) -> AnswerRequest {
        self.odren.request_for(source)
    }

    /// The answers of `request` from the cache of the engine, or `None`
    /// while they are in work ([`Self::step_answers`]).
    pub fn answers(&self, request: &AnswerRequest) -> Option<Result<PictureChoices, Unshowable>> {
        self.odren.answers(request)
    }

    /// Makes at most `budget` decisions of the answers in work, the first
    /// request of `waited` in work first ([`Odren::step_answers`]). Returns
    /// whether a request got its answers.
    pub fn step_answers<'r>(
        &self,
        waited: impl IntoIterator<Item = &'r AnswerRequest> + Clone,
        budget: usize,
    ) -> bool {
        self.odren.step_answers(waited, budget)
    }

    /// No decision is due, and the mode on screen was placed by a decision
    /// on the current inputs ([`Odren::settled`]).
    pub fn settled(&self) -> bool {
        self.odren.settled()
    }

    /// The steps of [`Self::step_answers`] that ran work so far
    /// ([`Odren::answer_runs`]).
    pub fn answer_runs(&self) -> u64 {
        self.odren.answer_runs()
    }

    /// Whether answers are in work.
    pub fn answers_in_work(&self) -> bool {
        self.odren.answers_in_work()
    }

    /// The answers of `request`, with its work finished at once.
    ///
    /// # Errors
    ///
    /// [`Unshowable`] when the policy gives no plan.
    pub fn answers_now(&self, request: &AnswerRequest) -> Result<PictureChoices, Unshowable> {
        self.odren.answers_now(request)
    }

    /// The answer of `neptron_modeline::policy::offer` for the settings of
    /// the run: whether the display gives the stored picture mode and
    /// refresh choice.
    ///
    /// # Errors
    ///
    /// [`Unshowable`] when the policy gives no plan.
    pub fn offer(&self, source: &SourceGeometry) -> Result<Offer, Unshowable> {
        self.odren.offer(source)
    }

    /// The display refuses `set_mode` and `present` while a flip is
    /// queued, so the engine waits for it first.
    fn drain_flip(&mut self) -> Result<Option<FlipEvent>, VideoError> {
        if !self.flip_pending {
            return Ok(None);
        }
        self.wait_queued()
            .map(Some)
            .map_err(VideoError::PendingFlip)
    }

    /// The one call of `Display::wait_flip`. A failed wait keeps the flip
    /// queued.
    fn wait_queued(&mut self) -> Result<FlipEvent, DisplayError> {
        let event = self.display.wait_flip()?;
        self.flip_pending = false;
        self.odren.on_flip(&event);
        Ok(event)
    }

    fn modeset(&mut self) -> Result<Option<Outcome>, VideoError> {
        let Some(order) = self.odren.pending_order() else {
            return Ok(None);
        };
        let Some(result) = self.set_mode(&order) else {
            return Ok(None);
        };
        let after = self.odren.on_mode_set(&result);
        let outcome = self.odren.trace().last().map(|r| r.outcome.clone());
        if let AfterModeSet::Restore(good) = after {
            let Some(restored) = self.set_mode(&good) else {
                return Ok(outcome);
            };
            self.odren.on_restore(&restored);
            if let (Err(error), Ok(read_back)) = (restored, result) {
                return Err(VideoError::RestoreFailed { read_back, error });
            }
        }
        Ok(outcome)
    }

    /// The one call of `Display::blank`. A queued flip was waited for
    /// before, in the same present.
    fn blank_if_due(&mut self) {
        if self.odren.blank_due() {
            let result = self.display.blank();
            self.odren.on_blank(&result);
        }
    }

    /// The one call of `Display::set_mode`. It sets nothing for an order
    /// whose inputs changed since its token was validated.
    fn set_mode(&mut self, order: &ModeOrder) -> Option<Result<Modeline, DisplayError>> {
        let token = self.odren.authorize(order)?;
        Some(self.display.set_mode(token))
    }
}
