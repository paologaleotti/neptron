//! The names of the TV standards in the menus and on the mode line. The
//! names NTSC and PAL appear only for a composite output: every other
//! output shows lines and rates.

use neptron_types::TvStandard;

/// The name of `standard` with no rate: `NTSC`, `PAL 60`, `Mono`. The
/// video status of a game uses the same names.
pub fn name(standard: TvStandard) -> &'static str {
    neptron_video::tv_standard_name(standard)
}

/// The short name of `standard` for the value column of a settings row:
/// [`name`], and the rate of the two monochrome standards, which share a
/// name: `PAL 60`, `Mono 50`.
pub fn short(standard: TvStandard) -> String {
    match standard {
        TvStandard::Mono60 | TvStandard::Mono50 => {
            format!("{} {}", name(standard), field_rate_hz(standard))
        }
        _ => name(standard).to_owned(),
    }
}

/// The name of `standard` with its field rate, for the menus:
/// `NTSC (60 Hz)`, `PAL (50 Hz)`, `Mono (50 Hz)`.
pub fn label(standard: TvStandard) -> String {
    format!("{} ({} Hz)", name(standard), field_rate_hz(standard))
}

/// The field rate of the line system in whole hertz, as the menus name it:
/// 60 for 525 lines, 50 for 625 lines.
fn field_rate_hz(standard: TvStandard) -> u16 {
    if standard.lines() == 525 { 60 } else { 50 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_standard_has_a_label_with_its_rate() {
        let labels: Vec<String> = TvStandard::ALL.into_iter().map(label).collect();
        assert_eq!(
            labels,
            [
                "NTSC (60 Hz)",
                "NTSC-J (60 Hz)",
                "NTSC 4.43 (60 Hz)",
                "PAL 60 (60 Hz)",
                "PAL-M (60 Hz)",
                "PAL (50 Hz)",
                "PAL-N (50 Hz)",
                "SECAM (50 Hz)",
                "Mono (60 Hz)",
                "Mono (50 Hz)",
            ]
        );
        let shorts: Vec<String> = TvStandard::ALL.into_iter().map(short).collect();
        assert_eq!(
            shorts,
            [
                "NTSC",
                "NTSC-J",
                "NTSC 4.43",
                "PAL 60",
                "PAL-M",
                "PAL",
                "PAL-N",
                "SECAM",
                "Mono 60",
                "Mono 50",
            ]
        );
    }
}
