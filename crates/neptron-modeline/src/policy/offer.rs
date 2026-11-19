use neptron_types::{
    GpuCaps, Modeline, MonitorKind, MonitorProfile, Rotation, ScanlineKind, ScanlineMode,
    ScreenClass, SourceGeometry, scanlines_for,
};

use super::choose::mode_refusal;
use super::{
    Considered, Notice, PictureStyle, Plan, PolicyError, PolicySettings, RefreshKind,
    RefreshPolicy, Rung, decide_shown,
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
    /// The plan of the candidate has no picture: nothing validates. A
    /// stored candidate shows the picture of `auto`, or else of `fit`
    /// ([`super::decide_shown`]), when one has a picture.
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
    /// A range covers the asked rate, but the winner runs at another rate:
    /// no mode at the asked rate validates, or a mode at another rate (an
    /// exact picture or a fit) ranks before every mode at the asked one.
    /// Where the winner runs at the asked rate, also as a soft fit, the
    /// option is offered: it gives the rate it names, and the notice names
    /// the soft picture.
    RateNotHeld {
        /// The refresh choice that was asked.
        asked: RefreshPolicy,
    },
    /// `native` or a multiple on a CRT or a TV: the display (the monitor
    /// with its video output) cannot show that rung whole at the rate of
    /// the winner, and the winner is another one. The policy does not tell the limit of the monitor from
    /// the limit of the output: that would need a second decision with
    /// other limits.
    NotShownHere {
        /// The picture mode that was asked.
        asked: PictureStyle,
        /// The rung that the display shows in its place.
        shown: Rung,
    },
    /// `native` or a multiple on a CRT or a TV: the asked rung is reachable
    /// whole at the rate of the winner, but a smaller step ranks first and
    /// fills more of the screen (a padded 2x of a lower line rate range
    /// before a padded 3x). The order of the plan is the policy of `auto`'s
    /// walk and does not change.
    SmallerFillsMore {
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
    /// `sharp`: no exact mode validates, the winner is the fit.
    NoExactMode,
    /// `speed` on a TV: its one field rate leaves no speed to choose.
    OneRateOnTv,
    /// `speed`: the winner still changes the game's speed by more than
    /// [`super::SPEED_ALERT_MIN_PPM`], so the mode does not give what it
    /// names.
    SpeedNotHeld,
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
    /// What the scanline settings give on that winner, or `None` with no
    /// winner.
    pub scanlines: Option<ScanlineOutcome>,
}

/// The answer for one candidate.
pub type Offer = Result<Offered, Declined>;

/// The answer for one option of the picture mode row or the refresh row.
/// A pick stores the option at a level; the engine then runs the pair of
/// that option and the stored value of the other row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RowAnswer {
    /// The pair of the option and the stored value of the other row is
    /// offered: a pick sets this row only.
    Plain(Offered),
    /// The pair with the stored value of the other row is declined, and
    /// the pair with the other row at Auto is offered: a pick sets this row
    /// and the other row to Auto, at the same level, in the same write.
    ResetsOther {
        /// The answer for the pair with the other row at Auto.
        offered: Offered,
        /// Why the pair with the stored value of the other row is
        /// declined.
        stored_pair: DeclineReason,
    },
    /// The pair with the other row at Auto is declined too: the option is
    /// greyed with this reason, which is a reason of this row alone.
    Declined(Declined),
}

impl RowAnswer {
    /// The answer of the pair that a pick runs: the offered pair, or the
    /// declined pair with the other row at Auto.
    pub fn offer(&self) -> Result<&Offered, &Declined> {
        match self {
            Self::Plain(offered) | Self::ResetsOther { offered, .. } => Ok(offered),
            Self::Declined(declined) => Err(declined),
        }
    }
}

/// An option of a row, with what is not given for every game when the
/// answer covers more than one source ([`for_every_game`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RowChoice {
    /// The answer.
    pub answer: RowAnswer,
    /// For a selectable option judged for more than one source: the reason
    /// of the first source whose pair the pick stores is declined. `None`
    /// when every source gives it, or for one source.
    pub not_for_every_game: Option<DeclineReason>,
}

/// The answers for the options of the two menu rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PictureChoices {
    /// Each picture mode, in the order of [`PictureStyle::ALL`].
    pub modes: Vec<(PictureStyle, RowChoice)>,
    /// Each refresh choice, in the order of [`RefreshPolicy::ALL`].
    pub refresh: Vec<(RefreshPolicy, RowChoice)>,
}

/// The sources that Settings > Display judges its options for: that page
/// writes the global level, which every game reads, and the policy has no
/// list of every game. Each entry is a real system shape that the others
/// do not cover:
///
/// - Game Boy, 160x144 at 59.728 Hz, aspect 10:9: a small frame, which
///   needs the largest multiples.
/// - SNES, 256x224 at 60.099 Hz: the common 60 Hz console frame.
/// - PlayStation, 320x240 at 59.826 Hz: a full 240-line frame at 60 Hz.
/// - NES PAL, 256x240 at 50.007 Hz: a 50 Hz console.
/// - Amiga PAL, 320x256 at 49.920 Hz: a tall 50 Hz frame, more lines than
///   a 60 Hz mode holds at 15 kHz.
/// - PlayStation 480i, 640x480 interlaced at 59.940 Hz: an interlaced
///   source.
pub const REFERENCE_SOURCES: [SourceGeometry; 6] = [
    reference(160, 144, 59_728, 1_111, false),
    reference(256, 224, 60_099, 1_333, false),
    reference(320, 240, 59_826, 1_333, false),
    reference(256, 240, 50_007, 1_333, false),
    reference(320, 256, 49_920, 1_333, false),
    reference(640, 480, 59_940, 1_333, true),
];

/// Scanlines show on a flat panel: the mode `on` with a strength above 0.
/// `auto` draws none on a panel. The policy then keeps a whole vertical
/// multiple. The engine and [`offer`] read this one rule.
pub fn panel_scanlines(kind: &MonitorKind, scanlines: ScanlineChoice) -> bool {
    matches!(kind, MonitorKind::FlatPanel { .. })
        && scanlines.mode == ScanlineMode::On
        && scanlines.strength_percent > 0
}

/// Which reasons a decision of [`judge`] checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Judge {
    /// A stored candidate, or an option of the picture mode row: the
    /// refresh reasons where the display reads the refresh choice (a CRT),
    /// then the mode reasons.
    Candidate,
    /// An option of the refresh row: the refresh reasons on every display,
    /// then the mode reasons. A TV and a flat panel do not read the
    /// refresh choice, so they give none but `exact`.
    RefreshOption,
}

/// The one decision of a candidate (the picture mode and the refresh
/// choice of `settings`, with `scanlines`) on the connected display: the
/// same [`decide`] and [`choose`] as the engine, then the refusal rules.
/// The app names a stored candidate that it declines. A TV and a flat
/// panel do not read the refresh choice, so it declines nothing there.
///
/// `validate` is the validator of the engine.
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
    validate: impl FnMut(&Modeline) -> Result<(), E>,
) -> Result<Offer, PolicyError> {
    let display = Display {
        source,
        scanlines,
        profile,
        caps,
    };
    judge(&display, settings, Judge::Candidate, validate)
}

/// [`offer`] for each option of the two menu rows. An option is judged
/// as the pair that a pick stores and the engine runs: the option with the
/// stored value of the other row ([`RowAnswer::Plain`]); where that pair is
/// declined, the option with the other row at Auto
/// ([`RowAnswer::ResetsOther`]); where that pair is declined too, the
/// option is greyed with the reason of that pair, a reason of the row
/// itself ([`RowAnswer::Declined`]). Auto of a row is offered with the
/// other row at Auto wherever the display shows a picture, so it is never
/// greyed there.
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
    let display = Display {
        source,
        scanlines,
        profile,
        caps,
    };
    let mut row = |pair: PolicySettings, at_auto: PolicySettings, which: Judge| {
        let answer = match judge(&display, &pair, which, &mut validate)? {
            Ok(offered) => RowAnswer::Plain(offered),
            Err(stored) if pair == at_auto => RowAnswer::Declined(stored),
            Err(stored) => match judge(&display, &at_auto, which, &mut validate)? {
                Ok(offered) => RowAnswer::ResetsOther {
                    offered,
                    stored_pair: stored.reason,
                },
                Err(declined) => RowAnswer::Declined(declined),
            },
        };
        Ok::<_, PolicyError>(RowChoice {
            answer,
            not_for_every_game: None,
        })
    };
    let modes = PictureStyle::ALL
        .into_iter()
        .map(|style| {
            let pair = PolicySettings { style, ..*settings };
            let at_auto = PolicySettings {
                refresh: RefreshPolicy::Exact,
                ..pair
            };
            Ok((style, row(pair, at_auto, Judge::Candidate)?))
        })
        .collect::<Result<_, PolicyError>>()?;
    let refresh = RefreshPolicy::ALL
        .into_iter()
        .map(|refresh| {
            let pair = PolicySettings {
                refresh,
                ..*settings
            };
            let at_auto = PolicySettings {
                style: PictureStyle::Auto,
                ..pair
            };
            Ok((refresh, row(pair, at_auto, Judge::RefreshOption)?))
        })
        .collect::<Result<_, PolicyError>>()?;
    Ok(PictureChoices { modes, refresh })
}

/// The answers of [`picture_choices`] for several sources, as one answer
/// for a page that writes for every game: an option declined for every
/// source is greyed with the reason of the first; else it is selectable,
/// plain where some source gives the pair with the stored value of the
/// other row, else with the reset of the other row, and with the reason of
/// the first source that the stored pair does not give
/// ([`RowChoice::not_for_every_game`]). The detail of the option is the
/// answer of the first source that gives it. `None` for no source.
pub fn for_every_game(per_source: &[PictureChoices]) -> Option<PictureChoices> {
    let first = per_source.first()?;
    let modes = (0..first.modes.len())
        .map(|i| {
            let answers: Vec<&RowAnswer> = per_source
                .iter()
                .filter_map(|c| c.modes.get(i))
                .map(|(_, r)| &r.answer)
                .collect();
            (
                first.modes[i].0,
                combine(&first.modes[i].1.answer, &answers),
            )
        })
        .collect();
    let refresh = (0..first.refresh.len())
        .map(|i| {
            let answers: Vec<&RowAnswer> = per_source
                .iter()
                .filter_map(|c| c.refresh.get(i))
                .map(|(_, r)| &r.answer)
                .collect();
            (
                first.refresh[i].0,
                combine(&first.refresh[i].1.answer, &answers),
            )
        })
        .collect();
    Some(PictureChoices { modes, refresh })
}

/// One answer for the answers of an option on several sources, of which
/// `first` is the first.
fn combine(first: &RowAnswer, answers: &[&RowAnswer]) -> RowChoice {
    let plain = answers.iter().find_map(|a| match a {
        RowAnswer::Plain(offered) => Some(offered),
        RowAnswer::ResetsOther { .. } | RowAnswer::Declined(_) => None,
    });
    let reset = answers.iter().find_map(|a| match a {
        RowAnswer::ResetsOther {
            offered,
            stored_pair,
        } => Some((offered, *stored_pair)),
        RowAnswer::Plain(_) | RowAnswer::Declined(_) => None,
    });
    let declined = answers.iter().find_map(|a| match a {
        RowAnswer::Declined(declined) => Some(declined),
        RowAnswer::Plain(_) | RowAnswer::ResetsOther { .. } => None,
    });
    match (plain, reset) {
        (Some(offered), _) => RowChoice {
            answer: RowAnswer::Plain(offered.clone()),
            not_for_every_game: answers.iter().find_map(|a| match a {
                RowAnswer::Plain(_) => None,
                RowAnswer::ResetsOther { stored_pair, .. } => Some(*stored_pair),
                RowAnswer::Declined(declined) => Some(declined.reason),
            }),
        },
        (None, Some((offered, stored_pair))) => RowChoice {
            answer: RowAnswer::ResetsOther {
                offered: offered.clone(),
                stored_pair,
            },
            not_for_every_game: declined.map(|d| d.reason),
        },
        (None, None) => RowChoice {
            answer: first.clone(),
            not_for_every_game: None,
        },
    }
}

/// The inputs of a decision that every candidate shares.
struct Display<'a> {
    source: &'a SourceGeometry,
    scanlines: ScanlineChoice,
    profile: &'a MonitorProfile,
    caps: &'a GpuCaps,
}

/// The decision of one candidate, with the reasons of `which`.
fn judge<E>(
    display: &Display<'_>,
    settings: &PolicySettings,
    which: Judge,
    mut validate: impl FnMut(&Modeline) -> Result<(), E>,
) -> Result<Offer, PolicyError> {
    let Display {
        source,
        scanlines,
        profile,
        caps,
    } = *display;
    let settings = PolicySettings {
        panel_scanlines: panel_scanlines(&profile.kind, scanlines),
        ..*settings
    };
    let shown = decide_shown(source, &settings, profile, caps, &mut validate)?;
    let notice = shown.notice();
    let fallback = shown.fallback;
    let plan = shown.plan;
    let winner = shown.winner.map(|(index, ())| index);
    let Some((index, considered, proposal)) = winner.and_then(|index| {
        let considered = plan.considered.get(index)?;
        let proposal = *considered.outcome.as_ref().ok()?;
        Some((index, considered, proposal))
    }) else {
        return Ok(Err(Declined {
            reason: DeclineReason::NoPicture,
            plan,
            winner: None,
            scanlines: None,
        }));
    };
    let refresh = || refresh_refusal(&plan, &profile.kind, settings.refresh, considered);
    let mode = || match fallback {
        // A picture of another mode: the mode of the settings has none.
        // Auto has none of its own, and takes the Fill picture.
        Some(_) if settings.style != PictureStyle::Auto => Some(DeclineReason::NoPicture),
        Some(_) => None,
        None => mode_refusal(&plan, settings.style, considered),
    };
    let reason = match which {
        Judge::Candidate => match profile.kind {
            MonitorKind::Crt { .. } => refresh().or_else(mode),
            MonitorKind::Tv { .. } | MonitorKind::FlatPanel { .. } => mode(),
        },
        Judge::RefreshOption => refresh().or_else(mode),
    };
    let reason = reason.map(|reason| ranked_after(reason, &plan, considered, &mut validate));
    let multiple = proposal.placement.vertical_multiple(source.rotation);
    let outcome = scanline_outcome(scanlines, multiple, &profile.kind, &proposal.mode);
    if let Some(reason) = reason {
        return Ok(Err(Declined {
            reason,
            plan,
            winner: Some(index),
            scanlines: Some(outcome),
        }));
    }
    Ok(Ok(Offered {
        winner: index,
        notice,
        scanlines: outcome,
        plan,
    }))
}

/// `reason`, or [`DeclineReason::SmallerFillsMore`] where it is
/// `NotShownHere` and the asked rung is reachable whole at the target of
/// the winner: a proposal of it there, with no crop, passes `validate`, but
/// ranks after the winner, a smaller step that fills more of the screen.
/// The order of the plan does not change; only the reason is true.
fn ranked_after<E>(
    reason: DeclineReason,
    plan: &Plan,
    won: &Considered,
    mut validate: impl FnMut(&Modeline) -> Result<(), E>,
) -> DeclineReason {
    let DeclineReason::NotShownHere { asked, shown } = reason else {
        return reason;
    };
    let own = super::walk_ladder(asked).first().copied();
    let reachable = plan.proposals().any(|(_, c, p)| {
        Some(c.rung) == own
            && c.target.kind == won.target.kind
            && p.overscan_crop.is_none()
            && validate(&p.mode).is_ok()
    });
    if reachable {
        DeclineReason::SmallerFillsMore { asked, shown }
    } else {
        reason
    }
}

/// Why the display does not give the refresh choice `asked`, whose plan
/// has the winner `won`. `exact` is always given: it is the game's rate,
/// or the nearest the display reaches. A winner at the asked target is
/// given, a soft fit too: the option gives the rate it names.
fn refresh_refusal(
    plan: &Plan,
    kind: &MonitorKind,
    asked: RefreshPolicy,
    won: &Considered,
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
        MonitorKind::Crt { .. } if won.target.kind == wanted => None,
        MonitorKind::Crt { .. } => Some(match plan.skipped.iter().find(|s| s.kind == wanted) {
            Some(skipped) => DeclineReason::RateNotCovered {
                asked,
                rate_mhz: skipped.rate_mhz,
            },
            None => DeclineReason::RateNotHeld { asked },
        }),
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

/// A progressive or interlaced source with no turn, for
/// [`REFERENCE_SOURCES`].
const fn reference(
    width: u16,
    height: u16,
    refresh_mhz: u32,
    aspect_milli: u32,
    interlaced: bool,
) -> SourceGeometry {
    SourceGeometry {
        width,
        height,
        refresh_mhz,
        aspect_milli,
        interlaced,
        rotation: Rotation::R0,
    }
}
