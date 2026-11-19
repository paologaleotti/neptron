//! The texts of the picture notice: why the picture of a game is not
//! exact, from the reasons that the policy returns as data.

use neptron_types::ScanlineMode;
use neptron_types::{Modeline, MonitorKind, Rotation, ScreenClass, SourceGeometry};
use neptron_video::{
    DeclineReason, Notice, NoticeReason, PolicySettings, RefreshKind, ScanlineOutcome,
    ScanlineSettings, SoftReason, VerticalMode, WholeReason,
};

use crate::pages;

/// True for the case of the one-time notice of a vertical game: a game
/// turned by 90 or 270 degrees, in an interlaced mode on a 15 kHz CRT or
/// a TV, with `display.vertical = exact`.
pub(super) fn vertical_interlaced(
    source: &SourceGeometry,
    mode: &Modeline,
    kind: &MonitorKind,
    vertical: VerticalMode,
) -> bool {
    let turned = matches!(source.rotation, Rotation::R90 | Rotation::R270);
    turned
        && mode.interlace
        && ScreenClass::of(kind, mode) == ScreenClass::Crt15khz
        && vertical == VerticalMode::Exact
}

/// The text of the notice for the picture of `notice` on an output of
/// `kind`, or `None` for an exact picture. Every reason gives one
/// sentence, in the order of [`Notice::reasons`]. A fit or a crop with no
/// reason of its own (a fit that the picture style asks for) gives the
/// words of the video status, `status`.
pub(super) fn picture_text(kind: RefreshKind, notice: &Notice, status: &str) -> Option<String> {
    let mut text = String::new();
    for reason in notice.reasons() {
        if !text.is_empty() {
            text.push(' ');
        }
        text.push_str(&reason_text(reason));
    }
    if text.is_empty() && worth_status_words(kind, notice) {
        text = format!("Picture: {status}.");
    }
    (!text.is_empty()).then_some(text)
}

/// The sentence of one reason.
fn reason_text(reason: NoticeReason) -> String {
    match reason {
        NoticeReason::FillFallback => {
            "Fill picture: no other picture mode has a picture for this game here.".to_owned()
        }
        NoticeReason::Soft(SoftReason::TooTall { lines }) => {
            format!("Soft picture: this screen cannot show the game's {lines} lines.")
        }
        NoticeReason::Soft(SoftReason::NoInterlace { lines }) => format!(
            "Soft picture: the game needs {lines} lines, and this monitor profile has no interlaced mode."
        ),
        NoticeReason::Soft(SoftReason::NoRate) => {
            "Soft picture: no sharp mode at this game's rate.".to_owned()
        }
        NoticeReason::Soft(SoftReason::Chosen) => {
            "Soft picture: the setting display.vertical is set to fit.".to_owned()
        }
        NoticeReason::Soft(SoftReason::FillMode) => {
            "Soft picture: Picture mode is set to Fill.".to_owned()
        }
        NoticeReason::SmallPicture { fill_permille } => format!(
            "Small picture: the game uses {} % of the screen height, to stay sharp.",
            (u32::from(fill_permille) + 5) / 10
        ),
        NoticeReason::SpeedChange {
            game_mhz,
            shown_mhz,
        } => format!(
            "The game runs at {} Hz, not {} Hz: {} % speed.",
            hz(shown_mhz),
            hz(u64::from(game_mhz)),
            speed_percent(u64::from(game_mhz), shown_mhz)
        ),
        NoticeReason::DoubleRefresh { field_mhz } => format!(
            "Each frame shows twice at {} Hz. The game speed does not change.",
            hz(field_mhz)
        ),
        NoticeReason::WholeSteps(WholeReason::Scanlines) => {
            "Whole steps, with borders: scanlines are on, and they need whole steps on a flat panel."
                .to_owned()
        }
        NoticeReason::WholeSteps(WholeReason::PanelScaleKey) => {
            "Whole steps, with borders: the setting display.panel_scale is set to integer."
                .to_owned()
        }
    }
}

/// The sentence for a stored picture mode or refresh choice that the
/// display does not give: what was asked, why not, and the mode `shown`
/// in its place, as the status line names it.
pub(super) fn declined_text(policy: PolicySettings, reason: DeclineReason, shown: &str) -> String {
    let asked = match reason {
        DeclineReason::RateFixedByTv { asked }
        | DeclineReason::RateOfPanel { asked }
        | DeclineReason::RateNotCovered { asked, .. }
        | DeclineReason::RateNotHeld { asked } => {
            format!("Refresh rate {}", pages::refresh_label(asked))
        }
        DeclineReason::NotShownHere { asked, .. }
        | DeclineReason::TooLargeForScreen { asked, .. } => {
            format!("Picture mode {}", pages::mode_label(asked))
        }
        DeclineReason::NoPicture => format!(
            "Picture mode {} with Refresh rate {}",
            pages::mode_label(policy.style),
            pages::refresh_label(policy.refresh)
        ),
        DeclineReason::NoExactMode | DeclineReason::OneRateOnTv | DeclineReason::SpeedNotHeld => {
            format!("Picture mode {}", pages::mode_label(policy.style))
        }
    };
    format!(
        "{asked}: {}. Shown: {shown}.",
        pages::lower_first(&pages::decline_text(reason))
    )
}

/// The sentence for scanlines that are on but not drawn on this picture,
/// or `None`: the settings ask for none, or the renderer draws them.
pub(super) fn scanline_text(
    scanlines: ScanlineSettings,
    outcome: ScanlineOutcome,
) -> Option<&'static str> {
    if scanlines.mode != ScanlineMode::On {
        return None;
    }
    match outcome {
        ScanlineOutcome::Drawn | ScanlineOutcome::NotAsked => None,
        ScanlineOutcome::NoStrength => Some("No scanlines: the scanline strength is 0."),
        ScanlineOutcome::Interlaced => {
            Some("No scanlines: they are not drawn on an interlaced picture.")
        }
        ScanlineOutcome::NoWholeMultiple => {
            Some("No scanlines: they need each game line on two or more screen lines.")
        }
    }
}

/// A fit off a panel, or a crop: the picture is not exact, and no reason
/// of the policy says why.
fn worth_status_words(kind: RefreshKind, notice: &Notice) -> bool {
    let panel = kind == RefreshKind::Panel;
    (notice.fitted && !panel) || notice.cropped_lines > 0 || notice.padded_fill_permille.is_some()
}

/// Millihertz as hertz with three decimals: `57.550`.
fn hz(mhz: u64) -> String {
    format!("{}.{:03}", mhz / 1_000, mhz % 1_000)
}

/// The speed change, `shown / game - 1`, in percent with a sign and two
/// decimals, rounded half away from zero: `+4.15`, `-13.11`. A game rate
/// of 0 gives `+0.00`.
fn speed_percent(game_mhz: u64, shown_mhz: u64) -> String {
    if game_mhz == 0 {
        return "+0.00".to_owned();
    }
    let sign = if shown_mhz >= game_mhz { '+' } else { '-' };
    // Hundredths of a percent, rounded: 10_000 * |shown - game| / game.
    let off = u128::from(shown_mhz.abs_diff(game_mhz));
    let game = u128::from(game_mhz);
    let hundredths = (off * 20_000 + game) / (2 * game);
    format!("{sign}{}.{:02}", hundredths / 100, hundredths % 100)
}

#[cfg(test)]
mod tests {
    use neptron_types::{TV_525_240P, TV_525_480I, TvStandard};

    use super::*;

    const DODONPACHI: SourceGeometry = SourceGeometry {
        width: 320,
        height: 240,
        refresh_mhz: 57_550,
        aspect_milli: 1_333,
        interlaced: false,
        rotation: Rotation::R90,
    };

    #[test]
    fn the_vertical_notice_is_for_a_turned_game_interlaced_at_15_khz() {
        let crt = MonitorKind::Crt { ranges: Vec::new() };
        let tv = MonitorKind::Tv {
            standard: TvStandard::Ntsc,
        };
        let exact = VerticalMode::Exact;
        assert!(vertical_interlaced(&DODONPACHI, &TV_525_480I, &crt, exact));
        assert!(vertical_interlaced(&DODONPACHI, &TV_525_480I, &tv, exact));
        let r270 = SourceGeometry {
            rotation: Rotation::R270,
            ..DODONPACHI
        };
        assert!(vertical_interlaced(&r270, &TV_525_480I, &tv, exact));
        for rotation in [Rotation::R0, Rotation::R180] {
            let flat = SourceGeometry {
                rotation,
                ..DODONPACHI
            };
            assert!(
                !vertical_interlaced(&flat, &TV_525_480I, &crt, exact),
                "{rotation:?}"
            );
        }
        assert!(
            !vertical_interlaced(&DODONPACHI, &TV_525_240P, &crt, exact),
            "progressive"
        );
        assert!(!vertical_interlaced(
            &DODONPACHI,
            &TV_525_480I,
            &crt,
            VerticalMode::Fit
        ));
        let panel = MonitorKind::FlatPanel { modes: Vec::new() };
        assert!(!vertical_interlaced(
            &DODONPACHI,
            &TV_525_480I,
            &panel,
            exact
        ));
        let mode_31 = Modeline {
            pclock_hz: 2 * TV_525_480I.pclock_hz,
            ..TV_525_480I
        };
        assert!(
            !vertical_interlaced(&DODONPACHI, &mode_31, &crt, exact),
            "31 kHz"
        );
    }

    fn exact() -> Notice {
        Notice {
            fitted: false,
            game_mhz: 57_550,
            speed_change: false,
            shown_mhz: 57_550,
            strategy_replaced: None,
            double_refresh: false,
            style_fallback: false,
            cropped_lines: 0,
            padded_fill_permille: None,
            soft: None,
            small_fill_permille: None,
            whole_steps: None,
            fill_fallback: false,
        }
    }

    #[test]
    fn an_exact_picture_has_no_notice() {
        for kind in [RefreshKind::Exact, RefreshKind::Panel, RefreshKind::Tv] {
            assert_eq!(picture_text(kind, &exact(), "exact"), None, "{kind:?}");
        }
    }

    #[test]
    fn each_soft_reason_has_its_sentence() {
        let cases = [
            (
                SoftReason::TooTall { lines: 640 },
                "Soft picture: this screen cannot show the game's 640 lines.",
            ),
            (
                SoftReason::NoInterlace { lines: 640 },
                "Soft picture: the game needs 640 lines, and this monitor profile has no interlaced mode.",
            ),
            (
                SoftReason::NoRate,
                "Soft picture: no sharp mode at this game's rate.",
            ),
            (
                SoftReason::Chosen,
                "Soft picture: the setting display.vertical is set to fit.",
            ),
            (
                SoftReason::FillMode,
                "Soft picture: Picture mode is set to Fill.",
            ),
        ];
        for (soft, text) in cases {
            let notice = Notice {
                fitted: true,
                soft: Some(soft),
                ..exact()
            };
            let shown = picture_text(RefreshKind::Exact, &notice, "filtered");
            assert_eq!(shown.as_deref(), Some(text), "{soft:?}");
        }
    }

    /// The 480x640 board on a 1080p panel: 640 of 1080 lines.
    #[test]
    fn a_small_picture_names_its_share_of_the_height() {
        let notice = Notice {
            small_fill_permille: Some(593),
            ..exact()
        };
        assert_eq!(
            picture_text(RefreshKind::Panel, &notice, "small picture 59%").as_deref(),
            Some("Small picture: the game uses 59 % of the screen height, to stay sharp.")
        );
    }

    /// DoDonPachi on NTSC: 57.550 Hz shown at 59.940 Hz.
    #[test]
    fn a_speed_change_names_both_rates_and_the_change() {
        let notice = Notice {
            speed_change: true,
            shown_mhz: 59_940,
            ..exact()
        };
        assert_eq!(
            picture_text(RefreshKind::Tv, &notice, "").as_deref(),
            Some("The game runs at 59.940 Hz, not 57.550 Hz: +4.15 % speed.")
        );
        assert_eq!(speed_percent(57_550, 50_000), "-13.12");
        assert_eq!(speed_percent(60_000, 60_000), "+0.00");
        assert_eq!(speed_percent(0, 60_000), "+0.00");
    }

    #[test]
    fn double_refresh_names_the_field_rate() {
        let notice = Notice {
            game_mhz: 30_000,
            shown_mhz: 30_000,
            double_refresh: true,
            ..exact()
        };
        assert_eq!(
            picture_text(RefreshKind::NativeDouble, &notice, "").as_deref(),
            Some("Each frame shows twice at 60.000 Hz. The game speed does not change.")
        );
    }

    #[test]
    fn several_reasons_give_several_sentences_in_order() {
        let notice = Notice {
            fitted: true,
            soft: Some(SoftReason::Chosen),
            speed_change: true,
            shown_mhz: 59_940,
            ..exact()
        };
        let text = picture_text(RefreshKind::Tv, &notice, "").unwrap();
        assert!(
            text.starts_with(
                "Soft picture: the setting display.vertical is set to fit. The game runs at"
            ),
            "{text}"
        );
    }

    /// A fit that the picture style asks for has no reason of the policy:
    /// the notice carries the words of the video status, off a panel.
    #[test]
    fn a_fit_or_a_crop_with_no_reason_gives_the_status_words() {
        let fitted = Notice {
            fitted: true,
            ..exact()
        };
        assert_eq!(
            picture_text(RefreshKind::Exact, &fitted, "filtered").as_deref(),
            Some("Picture: filtered.")
        );
        assert_eq!(picture_text(RefreshKind::Panel, &fitted, "sharp"), None);
        let cropped = Notice {
            cropped_lines: 16,
            ..exact()
        };
        assert_eq!(
            picture_text(RefreshKind::Panel, &cropped, "cropped 16 lines").as_deref(),
            Some("Picture: cropped 16 lines.")
        );
    }
}
