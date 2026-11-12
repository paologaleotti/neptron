use std::fmt::Write as _;

use neptron_modeline::policy::{Proposal, REFRESH_MATCH_MHZ, RefreshKind, Rung};
use neptron_types::{Filter, Modeline, SourceGeometry};

use crate::Current;

/// The video status of the running game as text, for the video block of the
/// quick menu and the display panel. The UI draws the strings as they are
/// and parses nothing.
///
/// Make one at launch and pass it to [`write_video_status`] again: it
/// clears and rewrites the strings, so it allocates only until they reach
/// their longest text.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VideoStatusText {
    /// What the core asks for: the size padded to 10 characters, 10 spaces,
    /// then the field rate with 3 decimals: `256x224             60.099 Hz`.
    pub game: String,
    /// The mode on screen as the driver read it back: the size with its
    /// scan suffix padded to 10, the line rate padded to 10, the field
    /// rate, two spaces and the rung: `768x448p  30.05 kHz 60.099 Hz  2x`.
    pub mode: String,
    /// The result in words, joined with `, `, in the order `exact`,
    /// `sharp`, `double refresh`, `cropped N lines`, `small picture N%`,
    /// `filtered`, `speed (X to Y Hz)`. An alert result starts with
    /// [`ALERT_MARK`].
    ///
    /// On a flat panel the result is `exact` when both axes scale by a
    /// whole factor and `sharp` when one does not (sharp bilinear, or
    /// nearest with uneven pixels at a scale from 1 to 2): the panel's way
    /// to keep the game's aspect, never blurry and never an alert. `filtered` is a CRT word only.
    pub result: String,
    /// The player must notice the result: `filtered` or `speed`, which a
    /// flat panel never gives.
    pub alert: bool,
}

/// What the status is made from: the source of the core, the mode on
/// screen and the winner that gave it.
///
/// [`StatusInputs::of`] takes the read back of the mode on screen. Inputs
/// built by hand from a proposal show a requested mode: that is for the
/// preview of the Display panel only, and the app must never put such a
/// status in the running game's status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatusInputs<'a> {
    /// The source the core asks for.
    pub source: &'a SourceGeometry,
    /// The mode on screen: the read back of the driver. A preview passes
    /// the proposal's mode.
    pub mode: &'a Modeline,
    /// The rung of the winner.
    pub rung: Rung,
    /// The winner's proposal.
    pub proposal: &'a Proposal,
}

/// The width of the size column and of the line rate column, in
/// characters.
pub const STATUS_COLUMN_CHARS: usize = 10;

/// The start of an alert result. It marks the alert also on a screen or a
/// skin where its colour is hard to tell apart.
pub const ALERT_MARK: &str = "[!] ";

impl<'a> StatusInputs<'a> {
    /// The inputs of the mode on screen.
    pub fn of(current: &'a Current) -> Self {
        Self {
            source: &current.source,
            mode: &current.read_back,
            rung: current.chosen.rung,
            proposal: &current.chosen.proposal,
        }
    }
}

/// Writes the video status into `out` and returns it, or returns `None`
/// with `out` unchanged when `inputs` is `None`: before the driver reads a
/// mode back there is no status, and the UI shows no block. The run and the
/// preview both come here. For the running game, pass inputs from
/// [`StatusInputs::of`] (or call `Odren::video_status`): inputs from a
/// proposal show a requested mode and are for the preview only.
pub fn write_video_status<'o>(
    inputs: Option<&StatusInputs<'_>>,
    out: &'o mut VideoStatusText,
) -> Option<&'o VideoStatusText> {
    let inputs = inputs?;
    write_game(inputs.source, &mut out.game);
    write_mode(inputs.mode, inputs.rung, &mut out.mode);
    out.alert = write_result(inputs, &mut out.result);
    Some(out)
}

fn write_game(source: &SourceGeometry, out: &mut String) {
    out.clear();
    column(out, |s| write!(s, "{}x{}", source.width, source.height));
    column(out, |_| Ok(()));
    hz(out, u64::from(source.refresh_mhz));
}

fn write_mode(mode: &Modeline, rung: Rung, out: &mut String) {
    out.clear();
    column(out, |s| {
        let scan = if mode.interlace { 'i' } else { 'p' };
        write!(s, "{}x{}{scan}", mode.hactive, mode.vactive)
    });
    column(out, |s| khz(s, mode.line_rate_mhz()));
    hz(out, mode.field_rate_mhz());
    out.push_str("  ");
    // Writing into a String cannot fail.
    let _ = write_rung(out, rung);
}

/// Writes the result words and returns the alert flag.
fn write_result(inputs: &StatusInputs<'_>, out: &mut String) -> bool {
    out.clear();
    let proposal = inputs.proposal;
    let timing = &proposal.timing;
    let repeat = u64::from(timing.repeat().max(1));
    let panel = timing.target.kind == RefreshKind::Panel;
    let fitted = inputs.rung == Rung::Fitted && !panel;
    let scale = proposal.placement.scale;
    let sharp = panel
        && (proposal.placement.filter != Filter::Nearest || scale.x.is_none() || scale.y.is_none());
    let cropped = proposal.overscan_crop.map_or(0, |crop| crop.lines());
    let small = proposal.padding.map(|pad| pad.fill_permille());
    let game_mhz = u64::from(timing.game_mhz);
    let shown_mhz = inputs.mode.field_rate_mhz() / repeat;
    let speed_changed = !panel && shown_mhz.abs_diff(game_mhz) >= REFRESH_MATCH_MHZ;
    let exact = !fitted && !sharp && cropped == 0 && small.is_none() && !speed_changed;
    let alert = fitted || speed_changed;
    if alert {
        out.push_str(ALERT_MARK);
    }

    if exact {
        push_word(out, |s| s.write_str("exact"));
    }
    if sharp {
        push_word(out, |s| s.write_str("sharp"));
    }
    if repeat >= 2 {
        push_word(out, |s| s.write_str("double refresh"));
    }
    if cropped > 0 {
        push_word(out, |s| write!(s, "cropped {cropped} lines"));
    }
    if let Some(permille) = small {
        push_word(out, |s| write!(s, "small picture {}%", (permille + 5) / 10));
    }
    if fitted {
        push_word(out, |s| s.write_str("filtered"));
    }
    if speed_changed {
        push_word(out, |s| {
            s.write_str("speed (")?;
            hz_number(s, game_mhz)?;
            s.write_str(" to ")?;
            hz_number(s, shown_mhz)?;
            s.write_str(" Hz)")
        });
    }
    alert
}

/// Appends one result word, after `, ` when a word is there already. The
/// alert mark is not a word.
fn push_word(out: &mut String, write: impl FnOnce(&mut String) -> std::fmt::Result) {
    if !out.is_empty() && out != ALERT_MARK {
        out.push_str(", ");
    }
    // Writing into a String cannot fail.
    let _ = write(out);
}

/// The short name of a rung on the mode line. Interlaced 1:1 shows the
/// game's own lines, so it is `native`; the `i` of the mode says the scan.
/// A multiple is `{n}x`, at most `255x`, so no label is longer than
/// `native`.
fn write_rung(out: &mut String, rung: Rung) -> std::fmt::Result {
    match rung {
        Rung::Native | Rung::Interlaced => out.write_str("native"),
        Rung::Multiple(n) => write!(out, "{n}x"),
        Rung::Fitted => out.write_str("fit"),
        Rung::Panel => out.write_str("panel"),
    }
}

/// Writes one column and pads it with spaces to [`STATUS_COLUMN_CHARS`].
/// A text that fills the column gets one space, so a panel size such as
/// `1920x1080p` stays apart from the next column. Every character the
/// formatter writes is ASCII, so bytes count characters.
fn column(out: &mut String, write: impl FnOnce(&mut String) -> std::fmt::Result) {
    let start = out.len();
    // Writing into a String cannot fail.
    let _ = write(out);
    while out.len() - start < STATUS_COLUMN_CHARS {
        out.push(' ');
    }
    if out.len() - start == STATUS_COLUMN_CHARS && !out.ends_with(' ') {
        out.push(' ');
    }
}

/// `60.099 Hz` from millihertz.
fn hz(out: &mut String, mhz: u64) {
    // Writing into a String cannot fail.
    let _ = hz_number(out, mhz).and_then(|()| out.write_str(" Hz"));
}

fn hz_number(out: &mut String, mhz: u64) -> std::fmt::Result {
    write!(out, "{}.{:03}", mhz / 1_000, mhz % 1_000)
}

/// `15.73 kHz` from millihertz, rounded to 10 Hz, a half up. From
/// 100 kHz up, one decimal (`133.3 kHz`, rounded to 100 Hz), so the
/// column stays 9 characters and a 2160p panel mode line fits 37.
fn khz(out: &mut String, mhz: u64) -> std::fmt::Result {
    let centi_khz = (mhz + 5_000) / 10_000;
    if centi_khz >= CENTI_KHZ_ONE_DECIMAL {
        let deci_khz = (mhz + 50_000) / 100_000;
        return write!(out, "{}.{} kHz", deci_khz / 10, deci_khz % 10);
    }
    write!(out, "{}.{:02} kHz", centi_khz / 100, centi_khz % 100)
}

/// 100 kHz in units of 10 Hz: from here the line rate has one decimal.
const CENTI_KHZ_ONE_DECIMAL: u64 = 10_000;

#[cfg(test)]
mod tests {
    use neptron_modeline::policy::{
        OverscanCrop, Padding, PolicySettings, RefreshTarget, choose, decide,
    };
    use neptron_safety::validate;
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

    /// A real winner: SNES on generic_15, with its mode as the read back.
    fn winner() -> (Proposal, Rung) {
        let profile = monitor_preset("generic_15").unwrap();
        let plan = decide(&SNES, &PolicySettings::default(), &profile, &CAPS_LOW_CLOCK).unwrap();
        let choice = choose(&plan, |m| validate(m, &profile, &CAPS_LOW_CLOCK));
        let w = choice.winner.unwrap();
        (*w.proposal, w.considered.rung)
    }

    /// The mode with its field rate moved to about `field_mhz`, through
    /// its total lines.
    fn at_rate(mode: Modeline, field_mhz: u64) -> Modeline {
        let vtotal = (mode.line_rate_mhz() + field_mhz / 2) / field_mhz;
        Modeline {
            vtotal: u16::try_from(vtotal).unwrap(),
            ..mode
        }
    }

    fn result(proposal: &Proposal, rung: Rung, mode: &Modeline) -> (String, bool) {
        let inputs = StatusInputs {
            source: &SNES,
            mode,
            rung,
            proposal,
        };
        let mut out = VideoStatusText::default();
        write_video_status(Some(&inputs), &mut out).unwrap();
        (out.result, out.alert)
    }

    /// The game rate of a proposal set to the rate its mode shows, so that
    /// the speed does not change.
    fn same_speed(mut p: Proposal) -> Proposal {
        p.timing.game_mhz =
            u32::try_from(p.mode.field_rate_mhz() / u64::from(p.timing.repeat())).unwrap();
        p
    }

    fn crop(lines_per_edge: u16) -> OverscanCrop {
        OverscanCrop {
            lines_per_edge,
            asked_lines: 480,
            capacity_lines: 478,
        }
    }

    fn padding(game_lines: u32, picture_lines: u32) -> Padding {
        Padding {
            game_lines,
            picture_lines,
            line_rate_min_mhz: 30_000_000,
            field_rate_mhz: 60_099,
        }
    }

    fn double(mut p: Proposal) -> Proposal {
        p.timing.target = RefreshTarget {
            repeat: 2,
            ..p.timing.target
        };
        p
    }

    #[test]
    fn each_result_word_alone() {
        let (base, rung) = winner();
        let base = same_speed(base);
        let mode = base.mode;
        let mut cropped = base;
        cropped.overscan_crop = Some(crop(8));
        let mut small = base;
        small.padding = Some(padding(224, 480));
        let mut fast = base;
        fast.timing.game_mhz = 59_940;
        let doubled = same_speed(double(base));
        let table: [(&str, Proposal, Rung, &str, bool); 6] = [
            ("exact", base, rung, "exact", false),
            ("double", doubled, rung, "exact, double refresh", false),
            ("cropped", cropped, rung, "cropped 16 lines", false),
            ("small", small, rung, "small picture 47%", false),
            ("filtered", base, Rung::Fitted, "[!] filtered", true),
            ("speed", fast, rung, "", true),
        ];
        for (name, proposal, rung, words, alert) in table {
            let (text, flag) = result(&proposal, rung, &mode);
            if name == "speed" {
                let shown = mode.field_rate_mhz();
                let expected = format!(
                    "[!] speed (59.940 to {}.{:03} Hz)",
                    shown / 1_000,
                    shown % 1_000
                );
                assert_eq!(text, expected, "{name}");
            } else {
                assert_eq!(text, words, "{name}");
            }
            assert_eq!(flag, alert, "{name}");
        }
    }

    #[test]
    fn combined_words_keep_their_order() {
        let (base, rung) = winner();
        let mode = at_rate(base.mode, 59_940);
        let mut p = base;
        p.timing.game_mhz = 60_099;
        p.overscan_crop = Some(crop(1));
        let (text, alert) = result(&p, rung, &mode);
        let shown = mode.field_rate_mhz();
        assert_eq!(
            text,
            format!(
                "[!] cropped 2 lines, speed (60.099 to {}.{:03} Hz)",
                shown / 1_000,
                shown % 1_000
            )
        );
        assert!(alert);
        let mut p = same_speed(double(base));
        p.padding = Some(padding(224, 300));
        assert_eq!(
            result(&p, rung, &p.mode).0,
            "double refresh, small picture 75%"
        );
        let (text, alert) = result(&p, Rung::Fitted, &p.mode);
        assert_eq!(text, "[!] double refresh, small picture 75%, filtered");
        assert!(alert);
    }

    #[test]
    fn the_speed_changes_at_the_match_bound_and_never_on_a_panel() {
        let (base, rung) = winner();
        let shown = base.mode.field_rate_mhz();
        let mut p = base;
        p.timing.game_mhz = u32::try_from(shown - (REFRESH_MATCH_MHZ - 1)).unwrap();
        assert_eq!(result(&p, rung, &base.mode).0, "exact");
        p.timing.game_mhz = u32::try_from(shown - REFRESH_MATCH_MHZ).unwrap();
        assert!(result(&p, rung, &base.mode).0.starts_with("[!] speed ("));
        p.timing.target.kind = RefreshKind::Panel;
        assert_eq!(
            result(&p, Rung::Panel, &base.mode),
            ("exact".to_owned(), false)
        );
    }

    #[test]
    fn a_panel_is_exact_or_sharp_and_never_an_alert() {
        let (base, _) = winner();
        let mut p = same_speed(base);
        p.timing.target.kind = RefreshKind::Panel;
        p.placement.filter = Filter::Nearest;
        assert_eq!(
            result(&p, Rung::Panel, &p.mode),
            ("exact".to_owned(), false)
        );
        let mut uneven = p;
        uneven.placement.scale.x = None;
        assert_eq!(
            result(&uneven, Rung::Panel, &uneven.mode),
            ("sharp".to_owned(), false),
            "nearest at a scale from 1 to 2 is not whole"
        );
        p.placement.filter = Filter::SharpBilinear;
        assert_eq!(
            result(&p, Rung::Panel, &p.mode),
            ("sharp".to_owned(), false)
        );
        assert_eq!(
            result(&p, Rung::Fitted, &p.mode),
            ("sharp".to_owned(), false),
            "a game larger than the panel"
        );
        p.timing.target.kind = RefreshKind::Exact;
        let (text, alert) = result(&p, Rung::Fitted, &p.mode);
        assert_eq!(text, "[!] filtered", "a CRT keeps its alert");
        assert!(alert);
    }

    #[test]
    fn a_1080p_panel_mode_line_keeps_its_columns_apart() {
        let (p, _) = winner();
        // 148.5 MHz over 2200 clocks is 67.50 kHz; over 1125 lines, 60 Hz.
        let shown = mode(1920, 1080, 148_500_000, 2_200, 1_125);
        let inputs = StatusInputs {
            source: &SNES,
            mode: &shown,
            rung: Rung::Panel,
            proposal: &p,
        };
        let mut out = VideoStatusText::default();
        write_video_status(Some(&inputs), &mut out).unwrap();
        assert_eq!(out.mode, "1920x1080p 67.50 kHz 60.000 Hz  panel");
    }

    #[test]
    fn a_2160p_mode_line_fits_37_characters() {
        let (p, _) = winner();
        // 533.25 MHz over 4000 clocks is 133.3125 kHz: one decimal from
        // 100 kHz up keeps the column at 9 characters.
        let shown = mode(3840, 2160, 533_250_000, 4_000, 2_222);
        let inputs = StatusInputs {
            source: &SNES,
            mode: &shown,
            rung: Rung::Panel,
            proposal: &p,
        };
        let mut out = VideoStatusText::default();
        write_video_status(Some(&inputs), &mut out).unwrap();
        assert_eq!(out.mode, "3840x2160p 133.3 kHz 59.997 Hz  panel");
        assert_eq!(out.mode.len(), 37);
        let mut text = String::new();
        khz(&mut text, 99_995_000).unwrap();
        assert_eq!(
            text, "100.0 kHz",
            "99.995 kHz rounds up to the one-decimal form"
        );
    }

    #[test]
    fn the_speed_compares_the_read_back_rate() {
        let (base, rung) = winner();
        let p = same_speed(base);
        let drifted = at_rate(p.mode, 59_000);
        assert!(result(&p, rung, &drifted).0.starts_with("[!] speed ("));
    }

    fn mode(hactive: u16, vactive: u16, pclock_hz: u32, htotal: u16, vtotal: u16) -> Modeline {
        Modeline {
            pclock_hz,
            hactive,
            hsync_start: hactive + 1,
            hsync_end: hactive + 2,
            htotal,
            vactive,
            vsync_start: vactive + 1,
            vsync_end: vactive + 2,
            vtotal,
            interlace: false,
            doublescan: false,
            hsync_positive: false,
            vsync_positive: false,
        }
    }

    #[test]
    fn the_game_and_mode_lines_have_fixed_columns() {
        let (p, _) = winner();
        // 30.05 MHz over 1000 clocks is 30.05 kHz; over 500 lines, 60.100 Hz.
        let shown = mode(768, 448, 30_050_000, 1_000, 500);
        let inputs = StatusInputs {
            source: &SNES,
            mode: &shown,
            rung: Rung::Multiple(2),
            proposal: &p,
        };
        let mut out = VideoStatusText::default();
        write_video_status(Some(&inputs), &mut out).unwrap();
        assert_eq!(out.game, "256x224             60.099 Hz");
        assert_eq!(out.mode, "768x448p  30.05 kHz 60.100 Hz  2x");
        assert_eq!(out.game.find("60."), out.mode.find("60."), "rates align");
        let interlaced = Modeline {
            interlace: true,
            ..mode(640, 480, 13_400_000, 852, 525)
        };
        let inputs = StatusInputs {
            mode: &interlaced,
            rung: Rung::Interlaced,
            ..inputs
        };
        write_video_status(Some(&inputs), &mut out).unwrap();
        assert!(out.mode.starts_with("640x480i  15.73 kHz "), "{}", out.mode);
        assert!(out.mode.ends_with("  native"));
    }

    #[test]
    fn the_worst_mode_line_fits_37_characters() {
        let (p, _) = winner();
        // 1920x238p at 15.73 kHz and about 59.9 Hz.
        let worst = mode(1_920, 238, 31_460_000, 2_000, 262);
        let inputs = StatusInputs {
            source: &SNES,
            mode: &worst,
            rung: Rung::Native,
            proposal: &p,
        };
        let mut out = VideoStatusText::default();
        write_video_status(Some(&inputs), &mut out).unwrap();
        assert!(
            out.mode.starts_with("1920x238p 15.73 kHz 60.0"),
            "{}",
            out.mode
        );
        assert_eq!(out.mode.len(), 37, "{}", out.mode);
    }

    #[test]
    fn every_rung_label_is_at_most_as_long_as_native() {
        let label = |rung| {
            let mut out = String::new();
            write_rung(&mut out, rung).unwrap();
            out
        };
        assert_eq!(label(Rung::Multiple(2)), "2x");
        assert_eq!(label(Rung::Multiple(5)), "5x");
        assert_eq!(label(Rung::Interlaced), "native");
        assert_eq!(label(Rung::Fitted), "fit");
        assert_eq!(label(Rung::Panel), "panel");
        let rungs = [
            Rung::Native,
            Rung::Interlaced,
            Rung::Multiple(u8::MAX),
            Rung::Fitted,
            Rung::Panel,
        ];
        assert!(rungs.into_iter().all(|r| label(r).len() <= "native".len()));
    }

    #[test]
    fn the_line_rate_rounds_to_10_hz_half_up() {
        let mut out = String::new();
        khz(&mut out, 15_734_999).unwrap();
        assert_eq!(out, "15.73 kHz");
        out.clear();
        khz(&mut out, 15_735_000).unwrap();
        assert_eq!(out, "15.74 kHz");
        out.clear();
        khz(&mut out, 100_000_000).unwrap();
        assert_eq!(out, "100.0 kHz", "one decimal from 100 kHz up");
    }

    #[test]
    fn no_status_before_a_read_back() {
        let mut out = VideoStatusText {
            game: "kept".to_owned(),
            ..VideoStatusText::default()
        };
        assert!(write_video_status(None, &mut out).is_none());
        assert_eq!(out.game, "kept");
    }

    #[test]
    fn a_second_write_reuses_the_strings() {
        let (p, rung) = winner();
        let inputs = StatusInputs {
            source: &SNES,
            mode: &p.mode,
            rung,
            proposal: &p,
        };
        let mut out = VideoStatusText::default();
        write_video_status(Some(&inputs), &mut out).unwrap();
        let before = (out.game.as_ptr(), out.mode.as_ptr(), out.result.as_ptr());
        write_video_status(Some(&inputs), &mut out).unwrap();
        assert_eq!(
            before,
            (out.game.as_ptr(), out.mode.as_ptr(), out.result.as_ptr())
        );
    }

    /// Wraps at spaces into lines of `width`, as the UI does.
    fn wrap(text: &str, width: usize) -> Vec<String> {
        let mut lines: Vec<String> = Vec::new();
        for word in text.split(' ') {
            match lines.last_mut() {
                Some(line) if line.len() + 1 + word.len() <= width => {
                    line.push(' ');
                    line.push_str(word);
                }
                _ => lines.push(word.to_owned()),
            }
        }
        lines
    }

    #[test]
    fn the_longest_real_results_fit_two_lines_of_37() {
        let (base, rung) = winner();
        let fast = at_rate(base.mode, 120_000);
        let mut cropped_double = double(base);
        cropped_double.timing.game_mhz = 100_000;
        cropped_double.overscan_crop = Some(crop(8));
        let mut small_double = double(base);
        small_double.timing.game_mhz = 57_500;
        small_double.padding = Some(padding(144, 480));
        // A fill of 995 per mille or more rounds to the widest percent.
        let mut full_double = small_double;
        full_double.padding = Some(padding(478, 480));
        let mut fitted_double = double(base);
        fitted_double.timing.game_mhz = 100_000;
        let cases = [
            (cropped_double, rung, fast),
            (small_double, rung, at_rate(base.mode, 100_000)),
            (full_double, rung, at_rate(base.mode, 100_000)),
            (fitted_double, Rung::Fitted, fast),
        ];
        let (full, _) = result(&full_double, rung, &at_rate(base.mode, 100_000));
        assert!(full.contains("small picture 100%,"), "{full}");
        for (proposal, rung, mode) in cases {
            let (text, alert) = result(&proposal, rung, &mode);
            assert!(alert && text.starts_with(ALERT_MARK), "{text}");
            assert!(text.len() <= 74, "{text}");
            let lines = wrap(&text, 37);
            assert!(lines.len() <= 2, "{lines:?}");
            assert!(lines.iter().all(|l| l.len() <= 37), "{lines:?}");
        }
    }
}
