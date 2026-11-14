//! The mode families of the video self-test, and the one test mode of each
//! family that a monitor profile supports. Every mode comes from the policy
//! and passes `validate`; nothing here touches a display.

use neptron_modeline::Settings;
use neptron_modeline::policy::{
    self, Choice, HorizontalScale, HorizontalStrategy, PictureStyle, PolicySettings, RefreshPolicy,
    Rung, TvWidth, Winner,
};
use neptron_safety::{Rejection, ValidatedMode, validate};
use neptron_types::{GpuCaps, Modeline, MonitorProfile, Rotation, SourceGeometry};

/// One mode family: a game source and the policy settings that ask for it,
/// and the test that the winner of the policy belongs to the family.
#[derive(Debug, Clone, Copy)]
pub struct Family {
    /// The stable id, used in the report: `240p`, `480i`, `super_width_fixed`.
    pub id: &'static str,
    /// The text on the test pattern and in the summary.
    pub title: &'static str,
    source: SourceGeometry,
    style: PictureStyle,
    refresh: RefreshPolicy,
    horizontal: Option<HorizontalStrategy>,
    accept: fn(&Facts) -> bool,
}

/// What the plan gives for one family.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The family has a test mode of its own.
    Planned(PlannedMode),
    /// The family gives the same mode as an earlier family, which tests it.
    SameAs {
        /// The family.
        family: &'static str,
        /// The earlier family with the same mode.
        same_as: &'static str,
    },
    /// The profile and the GPU give no mode of this family.
    Unsupported {
        /// The family.
        family: &'static str,
        /// Why, with the numbers.
        reason: String,
    },
}

/// The test mode of one family.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedMode {
    /// The family id.
    pub family: &'static str,
    /// The family title.
    pub title: &'static str,
    /// The mode, as `validate` approved it. Only this reaches a display.
    pub token: ValidatedMode,
    /// The policy's reasons for the mode.
    pub policy: PolicyFacts,
}

/// Where the policy took the mode from, for the report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyFacts {
    /// The picture style asked for.
    pub style: &'static str,
    /// The refresh policy asked for.
    pub refresh_policy: &'static str,
    /// The width strategy asked for, `None` for the GPU default.
    pub horizontal: Option<String>,
    /// The rung of the winner: `native`, `2x`, `interlaced 1:1`.
    pub rung: String,
    /// Where the field rate comes from: `exact`, `double refresh`.
    pub refresh_kind: String,
    /// Fields per game frame.
    pub repeat: u8,
    /// How the width is filled: `multiple 3`, `fixed 1920`, or `None`.
    pub width_rule: Option<String>,
    /// The game source of the family, as `WxH@mHz` with `i` for interlace.
    pub source: String,
}

/// The families in the order the self-test shows them. Each mode passes
/// `validate` before it is planned, so the order only decides what the
/// person sees first.
pub const FAMILIES: [Family; 9] = [
    Family {
        id: "240p",
        title: "240P",
        source: source(320, 240, 60_000, false),
        style: PictureStyle::Native,
        refresh: RefreshPolicy::Exact,
        horizontal: None,
        accept: |f| f.progressive_lines(240) && f.repeat == 1,
    },
    Family {
        id: "288p",
        title: "288P",
        source: source(384, 288, 50_000, false),
        style: PictureStyle::Native,
        refresh: RefreshPolicy::Exact,
        horizontal: None,
        accept: |f| f.progressive_lines(288) && f.repeat == 1,
    },
    Family {
        id: "240p_double_refresh",
        title: "240P DOUBLE REFRESH",
        source: source(320, 240, 60_000, false),
        style: PictureStyle::Native,
        refresh: RefreshPolicy::ExactX2,
        horizontal: None,
        accept: |f| f.progressive_lines(240) && f.repeat == 2,
    },
    Family {
        id: "480i",
        title: "480I",
        source: source(640, 480, 59_940, true),
        style: PictureStyle::Native,
        refresh: RefreshPolicy::Exact,
        horizontal: None,
        accept: |f| f.mode.interlace && f.mode.vactive == 480,
    },
    Family {
        id: "576i",
        title: "576I",
        source: source(720, 576, 50_000, true),
        style: PictureStyle::Native,
        refresh: RefreshPolicy::Exact,
        horizontal: None,
        accept: |f| f.mode.interlace && f.mode.vactive == 576,
    },
    Family {
        id: "480p",
        title: "480P",
        source: source(640, 480, 60_000, false),
        style: PictureStyle::Native,
        refresh: RefreshPolicy::Exact,
        horizontal: None,
        accept: |f| f.rung == Rung::Native && f.progressive_lines(480) && f.repeat == 1,
    },
    Family {
        id: "2x",
        title: "2X",
        source: source(320, 240, 60_000, false),
        style: PictureStyle::X2,
        refresh: RefreshPolicy::Exact,
        horizontal: None,
        accept: |f| f.rung == Rung::Multiple(2) && !f.mode.interlace,
    },
    Family {
        id: "super_width_dynamic",
        title: "DYNAMIC SUPER WIDTH",
        source: source(256, 224, 60_099, false),
        style: PictureStyle::Native,
        refresh: RefreshPolicy::Exact,
        horizontal: Some(HorizontalStrategy::DynamicSuperWidth),
        accept: |f| matches!(f.width, Some(HorizontalScale::Multiple(k)) if k >= 2),
    },
    Family {
        id: "super_width_fixed",
        title: "FIXED SUPER WIDTH",
        source: source(256, 224, 60_099, false),
        style: PictureStyle::Native,
        refresh: RefreshPolicy::Exact,
        // A width of 0 is the widest fixed width the GPU sends.
        horizontal: Some(HorizontalStrategy::FixedSuperWidth { width: 0 }),
        accept: |f| matches!(f.width, Some(HorizontalScale::Fixed(_))),
    },
];

/// Plans one test mode per family for `profile` on `caps`, in the order of
/// [`FAMILIES`]. A family whose mode equals the mode of an earlier family
/// is [`Outcome::SameAs`], so no mode shows twice.
pub fn plan(profile: &MonitorProfile, caps: &GpuCaps) -> Vec<Outcome> {
    let mut outcomes: Vec<Outcome> = Vec::with_capacity(FAMILIES.len());
    for family in &FAMILIES {
        let outcome = match plan_family(family, profile, caps) {
            Ok(planned) => match earlier_with(&outcomes, planned.token.mode()) {
                Some(same_as) => Outcome::SameAs {
                    family: family.id,
                    same_as,
                },
                None => Outcome::Planned(planned),
            },
            Err(reason) => Outcome::Unsupported {
                family: family.id,
                reason,
            },
        };
        outcomes.push(outcome);
    }
    outcomes
}

/// The planned modes of `outcomes`, in order.
pub fn planned(outcomes: &[Outcome]) -> impl Iterator<Item = &PlannedMode> {
    outcomes.iter().filter_map(|o| match o {
        Outcome::Planned(p) => Some(p),
        _ => None,
    })
}

/// The facts of a policy winner that the family tests read.
struct Facts {
    mode: Modeline,
    rung: Rung,
    repeat: u8,
    width: Option<HorizontalScale>,
}

impl Facts {
    fn progressive_lines(&self, lines: u16) -> bool {
        !self.mode.interlace && self.mode.vactive == lines
    }
}

const fn source(width: u16, height: u16, refresh_mhz: u32, interlaced: bool) -> SourceGeometry {
    SourceGeometry {
        width,
        height,
        refresh_mhz,
        aspect_milli: ASPECT_4_3_MILLI,
        interlaced,
        rotation: Rotation::R0,
    }
}

/// 4:3, in thousandths.
const ASPECT_4_3_MILLI: u32 = 1_333;

fn plan_family(
    family: &Family,
    profile: &MonitorProfile,
    caps: &GpuCaps,
) -> Result<PlannedMode, String> {
    let settings = PolicySettings {
        style: family.style,
        refresh: family.refresh,
        horizontal: family.horizontal,
        solver: Settings::default(),
        tv_width: TvWidth::default(),
    };
    let plan = policy::decide(&family.source, &settings, profile, caps)
        .map_err(|e| format!("the policy gives no plan: {e}"))?;
    let choice: Choice<'_, ValidatedMode, Rejection> =
        policy::choose(&plan, |m| validate(m, profile, caps));
    let Some(winner) = choice.winner else {
        return Err(match choice.refused.first() {
            Some(first) => format!(
                "the validator refuses every proposal ({} in all); the first: {}",
                choice.refused.len(),
                first.reason
            ),
            None => "the policy proposes no mode".to_owned(),
        });
    };
    let facts = facts_of(&winner);
    if !(family.accept)(&facts) {
        return Err(format!(
            "the policy chooses {} ({}), which is not of this family",
            describe(&facts.mode),
            winner.considered
        ));
    }
    Ok(PlannedMode {
        family: family.id,
        title: family.title,
        policy: PolicyFacts {
            style: family.style.key(),
            refresh_policy: family.refresh.key(),
            horizontal: family.horizontal.map(strategy_text),
            rung: winner.considered.rung.to_string(),
            refresh_kind: winner.proposal.timing.target.kind.to_string(),
            repeat: facts.repeat,
            width_rule: facts.width.map(|w| match w {
                HorizontalScale::Multiple(k) => format!("multiple {k}"),
                HorizontalScale::Fixed(w) => format!("fixed {w}"),
            }),
            source: format!(
                "{}x{}{}@{}",
                family.source.width,
                family.source.height,
                if family.source.interlaced { "i" } else { "p" },
                family.source.refresh_mhz
            ),
        },
        token: winner.token,
    })
}

fn strategy_text(strategy: HorizontalStrategy) -> String {
    match strategy {
        HorizontalStrategy::FixedSuperWidth { width: 0 } => {
            "fixed super width, the widest the GPU sends".to_owned()
        }
        other => other.to_string(),
    }
}

fn facts_of(winner: &Winner<'_, ValidatedMode>) -> Facts {
    Facts {
        mode: *winner.token.mode(),
        rung: winner.considered.rung,
        repeat: winner.proposal.timing.repeat(),
        width: winner.proposal.horizontal,
    }
}

fn earlier_with(outcomes: &[Outcome], mode: &Modeline) -> Option<&'static str> {
    planned(outcomes)
        .find(|p| p.token.mode() == mode)
        .map(|p| p.family)
}

/// A mode in short: `1280x240p 15.73 kHz 59.94 Hz 25.123 MHz`.
pub fn describe(m: &Modeline) -> String {
    let line = m.line_rate_mhz();
    let field = m.field_rate_mhz();
    format!(
        "{}x{}{} {}.{:02} kHz {}.{:03} Hz {}.{:03} MHz",
        m.hactive,
        m.vactive,
        if m.interlace { "i" } else { "p" },
        line / 1_000_000,
        (line / 10_000) % 100,
        field / 1_000,
        field % 1_000,
        m.pclock_hz / 1_000_000,
        (m.pclock_hz / 1_000) % 1_000
    )
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use neptron_platform::UNKNOWN_GPU_CAPS;

    use super::*;

    pub(crate) fn preset(id: &str) -> MonitorProfile {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/monitors")
            .join(format!("{id}.toml"));
        let text = std::fs::read_to_string(&path).unwrap();
        toml::from_str(&text).unwrap()
    }

    fn no_interlace() -> GpuCaps {
        UNKNOWN_GPU_CAPS
    }

    fn with_interlace() -> GpuCaps {
        GpuCaps {
            pclock_min_hz: 0,
            interlace: true,
            ..UNKNOWN_GPU_CAPS
        }
    }

    fn ids(outcomes: &[Outcome]) -> Vec<&'static str> {
        planned(outcomes).map(|p| p.family).collect()
    }

    fn find<'a>(outcomes: &'a [Outcome], id: &str) -> &'a Outcome {
        outcomes
            .iter()
            .find(|o| match o {
                Outcome::Planned(p) => p.family == id,
                Outcome::SameAs { family, .. } | Outcome::Unsupported { family, .. } => {
                    *family == id
                }
            })
            .unwrap()
    }

    #[test]
    fn every_family_has_an_outcome_in_order() {
        let outcomes = plan(&preset("pc_31_120"), &no_interlace());
        assert_eq!(outcomes.len(), FAMILIES.len());
        let order: Vec<&str> = outcomes
            .iter()
            .map(|o| match o {
                Outcome::Planned(p) => p.family,
                Outcome::SameAs { family, .. } | Outcome::Unsupported { family, .. } => family,
            })
            .collect();
        let expected: Vec<&str> = FAMILIES.iter().map(|f| f.id).collect();
        assert_eq!(order, expected);
    }

    #[test]
    fn a_31_khz_monitor_gets_double_refresh_480p_and_both_super_widths() {
        let outcomes = plan(&preset("pc_31_120"), &no_interlace());
        assert_eq!(
            ids(&outcomes),
            [
                "240p_double_refresh",
                "480p",
                "super_width_dynamic",
                "super_width_fixed"
            ],
            "{outcomes:#?}"
        );
        assert!(matches!(
            find(&outcomes, "240p"),
            Outcome::Unsupported { reason, .. } if reason.contains("not of this family")
        ));
        assert!(matches!(
            find(&outcomes, "480i"),
            Outcome::Unsupported { .. }
        ));
        assert_eq!(
            find(&outcomes, "2x"),
            &Outcome::SameAs {
                family: "2x",
                same_as: "480p"
            }
        );
        for p in planned(&outcomes) {
            let m = p.token.mode();
            assert!(!m.interlace, "{}", p.family);
            let rate = m.line_rate_mhz();
            assert!((31_400_000..=31_600_000).contains(&rate), "{}", p.family);
        }
        let double = planned(&outcomes).next().unwrap();
        assert_eq!(double.policy.repeat, 2);
        assert!(double.token.mode().field_rate_mhz() > 100_000);
    }

    #[test]
    fn a_15_khz_monitor_with_interlace_gets_the_low_resolution_families() {
        let outcomes = plan(&preset("generic_15"), &with_interlace());
        let got = ids(&outcomes);
        for id in ["240p", "288p", "480i", "576i"] {
            assert!(got.contains(&id), "{id}: {outcomes:#?}");
        }
        for p in planned(&outcomes) {
            let rate = p.token.mode().line_rate_mhz();
            assert!(rate < 17_000_000, "{}: {rate}", p.family);
        }
    }

    #[test]
    fn without_interlace_the_interlaced_families_are_unsupported() {
        let outcomes = plan(&preset("generic_15"), &no_interlace());
        for id in ["480i", "576i"] {
            assert!(
                matches!(find(&outcomes, id), Outcome::Unsupported { .. }),
                "{id}: {outcomes:#?}"
            );
        }
        assert!(!ids(&outcomes).is_empty(), "{outcomes:#?}");
    }

    #[test]
    fn every_planned_mode_passes_validate_again() {
        for id in [
            "pc_31_120",
            "generic_15",
            "arcade_31",
            "ntsc",
            "pal",
            "arcade_15_25_31",
        ] {
            let profile = preset(id);
            for caps in [no_interlace(), with_interlace()] {
                for p in planned(&plan(&profile, &caps)) {
                    assert_eq!(
                        validate(p.token.mode(), &profile, &caps).as_ref(),
                        Ok(&p.token),
                        "{id} {}",
                        p.family
                    );
                }
            }
        }
    }

    #[test]
    fn no_mode_is_planned_twice() {
        for id in ["pc_31_120", "generic_15", "arcade_31", "arcade_15_25_31"] {
            let outcomes = plan(&preset(id), &with_interlace());
            let modes: Vec<&Modeline> = planned(&outcomes).map(|p| p.token.mode()).collect();
            for (i, m) in modes.iter().enumerate() {
                assert!(!modes[..i].contains(m), "{id}: {m:?}");
            }
        }
    }

    /// The caps of the composite entry of the capability table.
    const COMPOSITE: GpuCaps = GpuCaps {
        pclock_min_hz: neptron_types::TV_PCLOCK_HZ,
        pclock_max_hz: neptron_types::TV_PCLOCK_HZ,
        interlace: true,
        flip_per_field: false,
        max_width: 720,
        max_height: 576,
    };

    /// On a TV the self-test shows the two table modes of the standard and
    /// nothing else: the progressive mode in the 240p or the 288p family,
    /// the interlaced mode in the 480i or the 576i family.
    #[test]
    fn a_tv_profile_plans_only_the_table_modes_of_its_standard() {
        for standard in neptron_types::TvStandard::ALL {
            let profile = standard.profile();
            let outcomes = plan(&profile, &COMPOSITE);
            let [progressive, interlaced] = standard.modes();
            let (p_family, i_family) = if standard.lines() == 525 {
                ("240p", "480i")
            } else {
                ("288p", "576i")
            };
            let got: Vec<(&str, Modeline)> = planned(&outcomes)
                .map(|p| (p.family, *p.token.mode()))
                .collect();
            assert_eq!(
                got,
                [(p_family, progressive), (i_family, interlaced)],
                "{standard:?}: {outcomes:#?}"
            );
            for p in planned(&outcomes) {
                assert_eq!(p.token.tv_standard(), Some(standard), "{}", p.family);
            }
        }
    }

    #[test]
    fn an_empty_profile_plans_nothing_and_says_why() {
        let empty = MonitorProfile {
            id: "empty".to_owned(),
            kind: neptron_types::MonitorKind::Crt { ranges: Vec::new() },
        };
        let outcomes = plan(&empty, &no_interlace());
        assert!(ids(&outcomes).is_empty());
        for o in &outcomes {
            assert!(
                matches!(o, Outcome::Unsupported { reason, .. } if reason.contains("no plan")),
                "{o:?}"
            );
        }
    }

    #[test]
    fn a_mode_reads_in_short() {
        let m = Modeline {
            pclock_hz: 25_175_000,
            hactive: 640,
            hsync_start: 656,
            hsync_end: 752,
            htotal: 800,
            vactive: 480,
            vsync_start: 490,
            vsync_end: 492,
            vtotal: 525,
            interlace: false,
            doublescan: false,
            hsync_positive: false,
            vsync_positive: false,
        };
        assert_eq!(describe(&m), "640x480p 31.46 kHz 59.940 Hz 25.175 MHz");
    }
}
