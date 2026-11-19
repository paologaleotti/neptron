use std::fmt;

use neptron_modeline::policy::{
    Dropped, Notice, Offer, PictureChoices, Plan, PolicyError, PolicySettings, Proposal,
    RefreshTarget, Refused, Rung, decide_shown, offer, picture_choices,
};
use neptron_safety::{Rejection, ValidatedMode, validate};
use neptron_types::{GpuCaps, Modeline, SourceGeometry};

use crate::{Monitor, ScanlineSettings};

/// An extra check that runs before `neptron_safety::validate`. It can only
/// refuse more: a mode it accepts must still pass the validator.
pub type Veto = Box<dyn Fn(&Modeline) -> Result<(), Rejection> + Send>;

/// Why a proposal of the plan was not set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// The validator, or the veto before it, refused the mode.
    Validator(Rejection),
    /// The display driver refused this mode earlier, or read back a mode
    /// that differs from it. The trace holds the error of that call.
    Driver,
}

/// The proposal that won, with the token to set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chosen {
    /// Index of the winner in [`Plan::considered`].
    pub index: usize,
    /// Its rung.
    pub rung: Rung,
    /// Its refresh target.
    pub target: RefreshTarget,
    /// The mode the policy proposed, and the game's place in it.
    pub proposal: Proposal,
    /// What the validator approved: the only mode the engine sets.
    pub token: ValidatedMode,
    /// The notice of step 8.
    pub notice: Option<Notice>,
}

/// Why the engine has no mode for a source.
#[derive(Debug, Clone, PartialEq)]
pub enum Unshowable {
    /// The policy gives no plan.
    Policy(PolicyError),
    /// Every proposal was refused.
    NoWinner,
    /// The inputs changed, and they refuse the mode on screen. The engine
    /// sets a valid mode when one exists, and else turns the output off
    /// with `Display::blank`.
    CurrentRefused(Refusal),
}

/// The engine's inputs that do not change from frame to frame.
pub(crate) struct Inputs {
    pub(crate) monitor: Monitor,
    pub(crate) settings: PolicySettings,
    /// The display's caps after the monitor's limit.
    pub(crate) caps: GpuCaps,
    pub(crate) veto: Option<Veto>,
    /// Modes the driver refused since the last monitor change.
    pub(crate) driver_refused: Vec<Modeline>,
}

/// Steps 1 to 6 for one source, owned so that the engine can cache it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Decision {
    /// The plan, or why there is none.
    pub(crate) plan: Result<Plan, Unshowable>,
    pub(crate) winner: Option<Chosen>,
    pub(crate) refused: Vec<Refused<Refusal>>,
}

impl Inputs {
    /// The validator of the engine: the veto, then `validate`, then the
    /// modes the driver refused.
    pub(crate) fn check(&self, mode: &Modeline) -> Result<ValidatedMode, Refusal> {
        if let Some(veto) = &self.veto {
            veto(mode).map_err(Refusal::Validator)?;
        }
        let token =
            validate(mode, self.monitor.profile(), &self.caps).map_err(Refusal::Validator)?;
        if self.driver_refused.contains(mode) {
            return Err(Refusal::Driver);
        }
        Ok(token)
    }

    /// Steps 1 to 6 for `source`, with the fallback of a picture mode that
    /// has no picture (`neptron_modeline::policy::decide_shown`).
    pub(crate) fn decide(&self, source: &SourceGeometry) -> Decision {
        let shown = match decide_shown(
            source,
            &self.settings,
            self.monitor.profile(),
            &self.caps,
            |mode| self.check(mode),
        ) {
            Ok(shown) => shown,
            Err(error) => return Decision::unshowable(Unshowable::Policy(error)),
        };
        let notice = shown.notice();
        let winner = shown.winner.and_then(|(index, token)| {
            let considered = shown.plan.considered.get(index)?;
            Some(Chosen {
                index,
                rung: considered.rung,
                target: considered.target,
                proposal: *considered.outcome.as_ref().ok()?,
                notice,
                token,
            })
        });
        Decision {
            plan: Ok(shown.plan),
            winner,
            refused: shown.refused,
        }
    }

    /// The answer of [`offer`] for the settings of the engine: whether the
    /// display gives the stored picture mode and refresh choice.
    pub(crate) fn offer(
        &self,
        source: &SourceGeometry,
        scanlines: ScanlineSettings,
    ) -> Result<Offer, Unshowable> {
        offer(
            source,
            &self.settings,
            scanlines,
            self.monitor.profile(),
            &self.caps,
            |mode| self.check(mode).map(drop),
        )
        .map_err(Unshowable::Policy)
    }
}

/// The answers of `neptron_modeline::policy::picture_choices` for a
/// display with the validator alone, with no veto and no driver refusal:
/// the menu rows a display gives before an engine runs on it. The engine
/// asks [`crate::VideoEngine::picture_choices`], which adds its veto.
///
/// # Errors
///
/// [`PolicyError`] when the source or the profile gives no plan.
pub fn picture_choices_on(
    source: &SourceGeometry,
    settings: &PolicySettings,
    scanlines: ScanlineSettings,
    profile: &neptron_types::MonitorProfile,
    caps: &GpuCaps,
) -> Result<PictureChoices, PolicyError> {
    picture_choices(source, settings, scanlines, profile, caps, |mode| {
        validate(mode, profile, caps).map(drop)
    })
}

impl Decision {
    fn unshowable(reason: Unshowable) -> Self {
        Self {
            plan: Err(reason),
            winner: None,
            refused: Vec::new(),
        }
    }

    /// Why there is no winner, or `None` when there is one.
    pub(crate) fn unshowable_reason(&self) -> Option<Unshowable> {
        match (&self.plan, &self.winner) {
            (_, Some(_)) => None,
            (Err(reason), None) => Some(reason.clone()),
            (Ok(_), None) => Some(Unshowable::NoWinner),
        }
    }

    /// The candidates the policy dropped before the winner, or all of them
    /// when nothing won, with their index in the plan.
    pub(crate) fn dropped(&self) -> Vec<(usize, Rung, Dropped)> {
        let Ok(plan) = &self.plan else {
            return Vec::new();
        };
        let end = self
            .winner
            .as_ref()
            .map_or(plan.considered.len(), |w| w.index);
        plan.considered[..end]
            .iter()
            .enumerate()
            .filter_map(|(i, c)| c.outcome.err().map(|d| (i, c.rung, d)))
            .collect()
    }
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Validator(rejection) => write!(f, "validator: {rejection}"),
            Self::Driver => f.write_str("the display driver refused it"),
        }
    }
}

impl fmt::Display for Unshowable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Policy(error) => write!(f, "no plan: {error}"),
            Self::NoWinner => f.write_str("every proposal was refused"),
            Self::CurrentRefused(refusal) => {
                write!(f, "the inputs refuse the mode on screen: {refusal}")
            }
        }
    }
}
