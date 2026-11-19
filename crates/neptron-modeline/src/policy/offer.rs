use neptron_types::{
    GpuCaps, Modeline, MonitorKind, MonitorProfile, ScanlineKind, ScanlineMode, ScreenClass,
    SourceGeometry, scanlines_for,
};

use super::choose::mode_refusal;
use super::{
    Considered, Dropped, Notice, PictureStyle, Plan, PolicyError, PolicySettings, RefreshKind,
    RefreshPolicy, Rung, choose, decide, notice, walk_ladder,
};

/// The scanline settings of a candidate: the resolved values of
/// `display.scanlines` and `display.scanline_strength`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ScanlineChoice {
    /// The scanline mode.
    pub mode: ScanlineMode,
    /// How much a dark line loses, in whole percent.
    pub strength_percent: u8,
}

impl ScanlineChoice {
    /// No scanlines: what the engine draws until the session passes the
    /// settings.
    pub const OFF: Self = Self {
        mode: ScanlineMode::Off,
        strength_percent: 0,
    };
}

/// Why the policy does not offer a candidate on the connected display.
/// The menus grey the option and the app maps each reason to its text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeclineReason {
    /// Nothing validates: the display shows no picture for the candidate.
    NoPicture,
    /// A TV standard fixes the field rate: only `exact` applies.
    RateFixedByTv {
        /// The refresh choice that was asked.
        asked: RefreshPolicy,
    },
    /// A flat panel runs at the rates of its own modes: only `exact`
    /// applies.
    RateOfPanel {
        /// The refresh choice that was asked.
        asked: RefreshPolicy,
    },
    /// No range of the CRT profile covers the asked rate.
    RateNotCovered {
        /// The refresh choice that was asked.
        asked: RefreshPolicy,
        /// The asked field rate, in millihertz.
        rate_mhz: u64,
    },
    /// A range covers the asked rate, but no exact mode at it validates.
    RateNotHeld {
        /// The refresh choice that was asked.
        asked: RefreshPolicy,
    },
    /// `native` or a multiple on a CRT or a TV: the display cannot show
    /// that rung, and the winner is another one.
    NeedsAnotherMonitor {
        /// The picture mode that was asked.
        asked: PictureStyle,
        /// The rung that the display shows in its place.
        shown: Rung,
    },
    /// `native` or a multiple on a flat panel: the factor does not fit the
    /// panel, and the winner is a smaller one or the fit.
    TooLargeForScreen {
        /// The picture mode that was asked.
        asked: PictureStyle,
        /// The rung that the panel shows in its place.
        shown: Rung,
    },
    /// `native` or a multiple: the rung of the mode fails for a limit of
    /// the video output (the GPU), not of the monitor: no interlace on the
    /// output, more lines than it sends, or a pixel clock below its floor.
    OutputCannotSend {
        /// The picture mode that was asked.
        asked: PictureStyle,
        /// The rung that the display shows in its place.
        shown: Rung,
    },
    /// `sharp`: no exact mode validates, the winner is the fit.
    NoExactMode,
    /// `speed` on a TV: its one field rate leaves no speed to choose.
    OneRateOnTv,
}

/// What the scanline settings give on the winner of a candidate. The
/// renderer draws by the rule of `neptron_types::scanlines_for`; this is
/// that rule read back, so the app can say why none show.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScanlineOutcome {
    /// The renderer draws scanlines.
    Drawn,
    /// The settings ask for none here: `off`, or `auto` on a screen where
    /// `auto` draws none.
    NotAsked,
    /// The strength is 0.
    NoStrength,
    /// The mode is interlaced: no scanline is drawn on one.
    Interlaced,
    /// The picture has no whole vertical multiple of 2 or more.
    NoWholeMultiple,
}

/// A candidate that the display gives: the plan, the winner the validator
/// accepted, its notice and its scanlines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offered {
    /// The plan of the candidate, from [`decide`].
    pub plan: Plan,
    /// Index of the winner in [`Plan::considered`].
    pub winner: usize,
    /// The notice of the winner, from [`notice`].
    pub notice: Option<Notice>,
    /// What the scanline settings give on the winner.
    pub scanlines: ScanlineOutcome,
}

/// A candidate that the display does not give, and what it would show in
/// its place. A stored candidate of this kind still runs: the engine shows
/// the winner of the same plan, which the validator accepted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Declined {
    /// Why.
    pub reason: DeclineReason,
    /// The plan of the candidate.
    pub plan: Plan,
    /// Index of the winner the display shows in its place, or `None` when
    /// nothing validates.
    pub winner: Option<usize>,
}

/// The answer for one candidate.
pub type Offer = Result<Offered, Declined>;

/// The answers for the options of the two menu rows: each picture mode
/// with the refresh choice of the settings, and each refresh choice with
/// the picture mode of the settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PictureChoices {
    /// Each picture mode, in the order of [`PictureStyle::ALL`].
    pub modes: Vec<(PictureStyle, Offer)>,
    /// Each refresh choice, in the order of [`RefreshPolicy::ALL`].
    pub refresh: Vec<(RefreshPolicy, Offer)>,
}

/// Scanlines show on a flat panel: the mode `on` with a strength above 0.
/// `auto` draws none on a panel. The policy then keeps a whole vertical
/// multiple. The engine and [`offer`] read this one rule.
pub fn panel_scanlines(kind: &MonitorKind, scanlines: ScanlineChoice) -> bool {
    matches!(kind, MonitorKind::FlatPanel { .. })
        && scanlines.mode == ScanlineMode::On
        && scanlines.strength_percent > 0
}

/// The one decision of a candidate (the picture mode and the refresh
/// choice of `settings`, with `scanlines`) on the connected display: the
/// same [`decide`] and [`choose`] as the engine, then the refusal rules.
/// The menus build every option from it, and the app names a stored
/// candidate that it refuses.
///
/// `validate` is the validator of the engine. The refresh choice is
/// checked first, because it holds for every picture mode.
///
/// # Errors
///
/// [`PolicyError`] when the source or the profile gives no plan.
pub fn offer<E>(
    source: &SourceGeometry,
    settings: &PolicySettings,
    scanlines: ScanlineChoice,
    profile: &MonitorProfile,
    caps: &GpuCaps,
    mut validate: impl FnMut(&Modeline) -> Result<(), E>,
) -> Result<Offer, PolicyError> {
    let settings = PolicySettings {
        panel_scanlines: panel_scanlines(&profile.kind, scanlines),
        ..*settings
    };
    let plan = decide(source, &settings, profile, caps)?;
    let winner = choose(&plan, &mut validate)
        .winner
        .map(|w| (w.index, w.considered.rung, w.considered.target.kind));
    let Some((index, rung, kind, proposal)) = winner.and_then(|(index, rung, kind)| {
        let proposal = plan.considered.get(index)?.outcome.as_ref().ok().copied()?;
        Some((index, rung, kind, proposal))
    }) else {
        return Ok(Err(Declined {
            reason: DeclineReason::NoPicture,
            plan,
            winner: None,
        }));
    };
    let reason = refresh_refusal(&plan, &profile.kind, settings.refresh, kind).or_else(|| {
        mode_refusal(&plan, settings.style, rung).map(|reason| output_limit(&plan, caps, reason))
    });
    if let Some(reason) = reason {
        return Ok(Err(Declined {
            reason,
            plan,
            winner: Some(index),
        }));
    }
    let multiple = proposal.placement.vertical_multiple(source.rotation);
    let scanlines = scanline_outcome(scanlines, multiple, &profile.kind, &proposal.mode);
    let notice = plan
        .considered
        .get(index)
        .and_then(|considered| notice(&plan, considered));
    Ok(Ok(Offered {
        winner: index,
        notice,
        scanlines,
        plan,
    }))
}

/// [`offer`] for each option of the two menu rows.
///
/// # Errors
///
/// [`PolicyError`] when the source or the profile gives no plan.
pub fn picture_choices<E>(
    source: &SourceGeometry,
    settings: &PolicySettings,
    scanlines: ScanlineChoice,
    profile: &MonitorProfile,
    caps: &GpuCaps,
    mut validate: impl FnMut(&Modeline) -> Result<(), E>,
) -> Result<PictureChoices, PolicyError> {
    let mut one = |settings: PolicySettings| {
        offer(source, &settings, scanlines, profile, caps, &mut validate)
    };
    let modes = PictureStyle::ALL
        .into_iter()
        .map(|style| Ok((style, one(PolicySettings { style, ..*settings })?)))
        .collect::<Result<_, PolicyError>>()?;
    let refresh = RefreshPolicy::ALL
        .into_iter()
        .map(|refresh| {
            Ok((
                refresh,
                one(PolicySettings {
                    refresh,
                    ..*settings
                })?,
            ))
        })
        .collect::<Result<_, PolicyError>>()?;
    Ok(PictureChoices { modes, refresh })
}

/// Why the display does not give the refresh choice `asked`, whose plan
/// has a winner at a target of `kind`. `exact` is always given: it is the
/// game's rate, or the nearest the display reaches.
fn refresh_refusal(
    plan: &Plan,
    kind: &MonitorKind,
    asked: RefreshPolicy,
    won: RefreshKind,
) -> Option<DeclineReason> {
    let wanted = match asked {
        RefreshPolicy::Exact => return None,
        RefreshPolicy::ExactX2 => RefreshKind::Double,
        RefreshPolicy::Force50 => RefreshKind::Forced50,
        RefreshPolicy::Force60 => RefreshKind::Forced60,
    };
    match kind {
        MonitorKind::Tv { .. } => Some(DeclineReason::RateFixedByTv { asked }),
        MonitorKind::FlatPanel { .. } => Some(DeclineReason::RateOfPanel { asked }),
        MonitorKind::Crt { .. } if won == wanted => None,
        MonitorKind::Crt { .. } => Some(match plan.skipped.iter().find(|s| s.kind == wanted) {
            Some(skipped) => DeclineReason::RateNotCovered {
                asked,
                rate_mhz: skipped.rate_mhz,
            },
            None => DeclineReason::RateNotHeld { asked },
        }),
    }
}

/// `NeedsAnotherMonitor` becomes `OutputCannotSend` when every candidate
/// on the first rung of the asked mode dropped for a limit of the GPU:
/// another monitor would not help there.
fn output_limit(plan: &Plan, caps: &GpuCaps, reason: DeclineReason) -> DeclineReason {
    let DeclineReason::NeedsAnotherMonitor { asked, shown } = reason else {
        return reason;
    };
    let first = walk_ladder(asked).first().copied();
    let mut own = plan
        .considered
        .iter()
        .filter(|c| Some(c.rung) == first)
        .peekable();
    let gpu = |c: &Considered| match &c.outcome {
        Err(Dropped::NoInterlace) => !caps.interlace,
        Err(Dropped::TallerThanGpu { .. } | Dropped::PixelClockFloor { .. }) => true,
        Ok(_) | Err(_) => false,
    };
    if own.peek().is_some() && own.all(gpu) {
        DeclineReason::OutputCannotSend { asked, shown }
    } else {
        reason
    }
}

/// The scanline rule read back for the winner: `scanlines_for` with the
/// winner's multiple, screen class and scan, and with each condition
/// lifted in turn to find the one that stops it.
fn scanline_outcome(
    scanlines: ScanlineChoice,
    multiple: Option<u16>,
    kind: &MonitorKind,
    mode: &Modeline,
) -> ScanlineOutcome {
    let screen = ScreenClass::of(kind, mode);
    let draws = |strength, multiple, interlaced| {
        scanlines_for(scanlines.mode, strength, multiple, screen, interlaced).kind
            != ScanlineKind::None
    };
    if draws(scanlines.strength_percent, multiple, mode.interlace) {
        ScanlineOutcome::Drawn
    } else if !draws(100, Some(2), false) {
        ScanlineOutcome::NotAsked
    } else if !draws(scanlines.strength_percent, Some(2), false) {
        ScanlineOutcome::NoStrength
    } else if mode.interlace {
        ScanlineOutcome::Interlaced
    } else {
        ScanlineOutcome::NoWholeMultiple
    }
}
