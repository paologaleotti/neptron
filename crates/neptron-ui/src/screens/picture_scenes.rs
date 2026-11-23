//! The options of the picture rows that the scenes show, as the app builds
//! them. A test of the app builds the same rows with its own code and
//! holds them equal to these, so a scene cannot show what the app does not.

use crate::snapshot::ChoiceOption;

/// The labels and the help lines of the picture modes, in the order of the
/// menus of the app.
pub const PICTURE_MODE_TEXTS: [(&str, &str); 8] = [
    (
        "Auto",
        "Exact lines first, at the game's speed. A notice names each compromise.",
    ),
    (
        "Sharp lines",
        "Exact lines over the game's speed. The picture can flicker or be small.",
    ),
    (
        "Game speed",
        "The game's own speed first. The picture can be soft.",
    ),
    (
        "Fill",
        "The largest picture with no flicker where the screen allows it. The picture can be soft.",
    ),
    (
        "Native",
        "Each game line on one screen line. The picture can be small, with borders.",
    ),
    ("2x", "Each game line on two screen lines."),
    ("3x", "Each game line on three screen lines."),
    ("4x", "Each game line on four screen lines."),
];

/// The labels and the help lines of the refresh choices on Settings >
/// Display, in the order of the schema.
pub const REFRESH_RATE_TEXTS: [(&str, &str); 4] = [
    (
        "Auto",
        "The game's own rate, or the nearest rate the monitor takes.",
    ),
    (
        "Double rate",
        "Each frame shows twice, at twice the game's rate. The game speed does not change.",
    ),
    (
        "50 Hz",
        "Games run at 50 Hz. A game made for 60 Hz runs slower.",
    ),
    (
        "60 Hz",
        "Games run at 60 Hz. A game made for 50 Hz runs faster.",
    ),
];

/// The reasons and tags of the picture rows that the scenes show.
pub mod picture_texts {
    /// `NotShownHere`.
    pub const NOT_SHOWN_HERE: &str = "Not possible with this screen and video output";
    /// `OneRateOnTv` and `RateFixedByTv`.
    pub const ONE_RATE_ON_TV: &str = "A TV has one rate";
    /// `RateOfPanel`.
    pub const PANEL_RATES: &str = "A flat panel uses its own rates";
    /// `SpeedNotHeld`.
    pub const SPEED_NOT_HELD: &str = "This screen cannot hold the game's speed";
    /// `RateNotCovered` of Double rate for every reference game.
    pub const NO_DOUBLE_RATE: &str = "This monitor does not take twice the rate of any game";
    /// A row that the display does not read on a TV.
    pub const NOT_USED_ON_A_TV: &str = "Not used on a TV";
    /// A row that the display does not read on a flat panel.
    pub const NOT_USED_ON_A_PANEL: &str = "Not used on a flat panel";
    /// The tag of an option that some reference games do not get.
    pub const NOT_ALL_GAMES: &str = "Not all games";
    /// An option whose answer is in work. The font has the full stop, not
    /// the ellipsis character.
    pub const JUDGING: &str = "...";
    /// The tag of a picture mode that also sets the refresh rate to Auto.
    pub const SETS_RATE_AUTO: &str = "Sets rate Auto";
    /// The longest picture notice of one picture: a stored pair that the
    /// display declines, the speed change and the scanlines, in one
    /// dialog, as the app joins them.
    pub const LONGEST_PICTURE_NOTICE: &str = "Picture mode Game speed with Refresh rate 50 Hz: the refresh rate changes the game's speed. Shown: 256x224p 15.65 kHz 50.000 Hz. The game runs at 50.000 Hz, not 60.099 Hz: -16.80 % speed. No scanlines: they need each game line on two or more screen lines.";
}

/// The help line of the scanline mode Auto on a CRT, the longest help
/// line of the display rows.
pub const SCANLINES_AUTO_CRT_HELP: &str =
    "Drawn above 15 kHz when each game line is on two or more screen lines, with no interlace.";

use picture_texts as texts;

/// A display of the scenes of Settings > Display, judged for every game
/// with the default settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SceneDisplay {
    /// `generic_15`, a 15 kHz tube.
    Crt15,
    /// `pc_31_120`, a 31 kHz PC tube that takes 120 Hz.
    Crt31,
    /// A 1080p flat panel at 60 Hz.
    Panel,
    /// NTSC on the composite output.
    Tv,
}

/// The picture modes of Settings > Display on `display`: no rate detail,
/// the tag of an option that some games do not get, and the greyed options
/// with their reasons.
pub fn settings_modes(display: SceneDisplay) -> Vec<ChoiceOption> {
    PICTURE_MODE_TEXTS
        .iter()
        .map(|&(label, help)| {
            let option = ChoiceOption::new(label).with_help(help);
            match (display, label) {
                (SceneDisplay::Crt15 | SceneDisplay::Tv, "2x" | "3x" | "4x")
                | (SceneDisplay::Crt31, "4x") => option.unavailable(texts::NOT_SHOWN_HERE),
                (SceneDisplay::Tv, "Game speed") => option.unavailable(texts::ONE_RATE_ON_TV),
                (SceneDisplay::Crt15 | SceneDisplay::Tv, "Native")
                | (SceneDisplay::Crt31, "2x" | "3x")
                | (SceneDisplay::Panel, "Game speed" | "3x" | "4x") => {
                    option.with_detail(texts::NOT_ALL_GAMES)
                }
                _ => option,
            }
        })
        .collect()
}

/// The refresh choices of Settings > Display on `display`.
pub fn settings_rates(display: SceneDisplay) -> Vec<ChoiceOption> {
    REFRESH_RATE_TEXTS
        .iter()
        .map(|&(label, help)| {
            let option = ChoiceOption::new(label).with_help(help);
            match (display, label) {
                (_, "Auto") => option,
                (SceneDisplay::Crt15, "Double rate") => option.unavailable(texts::NO_DOUBLE_RATE),
                (SceneDisplay::Crt31, "Double rate") => option.with_detail(texts::NOT_ALL_GAMES),
                (SceneDisplay::Tv, _) => option.unavailable(texts::ONE_RATE_ON_TV),
                (SceneDisplay::Panel, _) => option.unavailable(texts::PANEL_RATES),
                _ => option,
            }
        })
        .collect()
}

/// The picture modes of the quick menu for a DOS game in 640x400 at
/// 70.086 Hz on `generic_15`: Game speed and the fixed scales greyed.
pub fn quick_dos_modes() -> Vec<ChoiceOption> {
    PICTURE_MODE_TEXTS
        .iter()
        .map(|&(label, help)| {
            let option = ChoiceOption::new(label).with_help(help);
            match label {
                "Auto" | "Sharp lines" => option.with_detail("15.72 kHz  64.285 Hz"),
                "Fill" => option.with_detail("15.67 kHz  65.000 Hz"),
                "Game speed" => option
                    .with_detail("15.67 kHz  65.000 Hz")
                    .unavailable(texts::SPEED_NOT_HELD),
                _ => option
                    .with_detail("15.72 kHz  64.285 Hz")
                    .unavailable(texts::NOT_SHOWN_HERE),
            }
        })
        .collect()
}

/// The picture modes of the quick menu for the SNES on `generic_15` with
/// the refresh rate at 50 Hz: Game speed holds the speed only at the
/// game's own rate, so its tag says that a pick also sets the rate to Auto.
pub fn quick_reset_modes() -> Vec<ChoiceOption> {
    PICTURE_MODE_TEXTS
        .iter()
        .map(|&(label, help)| {
            let option = ChoiceOption::new(label).with_help(help);
            match label {
                "Game speed" => option.with_detail(texts::SETS_RATE_AUTO),
                "2x" | "3x" | "4x" => option
                    .with_detail("15.63 kHz  60.099 Hz")
                    .unavailable(texts::NOT_SHOWN_HERE),
                _ => option.with_detail("15.65 kHz  50.000 Hz"),
            }
        })
        .collect()
}

/// The picture modes of the quick menu while their first answers are in
/// work: every option greyed with [`texts::JUDGING`].
pub fn quick_judging_modes() -> Vec<ChoiceOption> {
    PICTURE_MODE_TEXTS
        .iter()
        .map(|&(label, help)| {
            ChoiceOption::new(label)
                .with_help(help)
                .unavailable(texts::JUDGING)
        })
        .collect()
}

/// The picture modes of the quick menu after a pick, while the new answers
/// are in work: the last settled answers ([`quick_reset_modes`]) stay on
/// show, and each offered option takes no pick until the new answers are
/// there.
pub fn quick_last_modes() -> Vec<ChoiceOption> {
    quick_reset_modes()
        .into_iter()
        .map(|option| {
            if option.is_enabled() {
                option.unavailable(texts::JUDGING)
            } else {
                option
            }
        })
        .collect()
}
