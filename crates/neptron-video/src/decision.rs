use std::fmt;

use neptron_modeline::policy::{
    Dropped, Notice, Plan, PolicyError, PolicySettings, Proposal, RefreshTarget, Refused, Rung,
    StyleRow, choose, decide, notice, style_rows,
};
use neptron_safety::{Rejection, ValidatedMode, validate};
use neptron_types::{GpuCaps, Modeline, Rotation, SourceGeometry};

use crate::Monitor;

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
    /// The game is turned. The policy can swap its axes, but `Scene` has no
    /// rotation, so the engine does not show it.
    Rotated(Rotation),
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

    /// Steps 1 to 6 for `source`.
    pub(crate) fn decide(&self, source: &SourceGeometry) -> Decision {
        if source.rotation != Rotation::R0 {
            return Decision::unshowable(Unshowable::Rotated(source.rotation));
        }
        let plan = match decide(source, &self.settings, self.monitor.profile(), &self.caps) {
            Ok(plan) => plan,
            Err(error) => return Decision::unshowable(Unshowable::Policy(error)),
        };
        let (winner, refused) = {
            let choice = choose(&plan, |mode| self.check(mode));
            let winner = choice.winner.map(|w| Chosen {
                index: w.index,
                rung: w.considered.rung,
                target: w.considered.target,
                proposal: *w.proposal,
                notice: notice(&plan, w.considered),
                token: w.token,
            });
            (winner, choice.refused)
        };
        Decision {
            plan: Ok(plan),
            winner,
            refused,
        }
    }

    /// The picture style rows of the Display panel for `source`, from the
    /// same policy, caps and validator that the engine uses.
    pub(crate) fn style_rows(
        &self,
        source: &SourceGeometry,
    ) -> Result<Vec<StyleRow<Refusal>>, Unshowable> {
        if source.rotation != Rotation::R0 {
            return Err(Unshowable::Rotated(source.rotation));
        }
        style_rows(
            source,
            &self.settings,
            self.monitor.profile(),
            &self.caps,
            |mode| self.check(mode).map(drop),
        )
        .map_err(Unshowable::Policy)
    }
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
            Self::Rotated(rotation) => write!(
                f,
                "the game is turned ({rotation:?}) and the renderer does not rotate"
            ),
            Self::Policy(error) => write!(f, "no plan: {error}"),
            Self::NoWinner => f.write_str("every proposal was refused"),
            Self::CurrentRefused(refusal) => {
                write!(f, "the inputs refuse the mode on screen: {refusal}")
            }
        }
    }
}
