//! The screens: which template shows which snapshot data, and the screen
//! stack. [`Screens`] is the router that the app owns and calls once per
//! frame. Each screen registers its scenes in [`SCENES`].
//!
//! The UI and the app agree on a few numbers: the pages of the display
//! panel, the audio panel and the quick menu, the pages that the UI shows
//! itself, and the menu actions that the UI carries out itself. They are at
//! [`RESERVED_ID_MIN`] and above; the app numbers its own pages and actions
//! below it.

mod chrome;
mod display;
mod game_list;
mod home;
mod options;
mod overlays;
mod page;
mod panel;
mod pattern;
mod quick_menu;
mod router;
mod scenes;
mod settings;
mod video;

#[cfg(test)]
mod tests;

pub use quick_menu::MAX_CHANGES;
pub use router::{MAX_DEPTH, Screens};

use crate::registry::SceneDef;
use crate::snapshot::{ActionId, PageId};

/// The first page and action number that the UI names. The app uses
/// numbers below it for its own pages and actions.
pub const RESERVED_ID_MIN: u32 = 0xFFFF_0000;

/// The page of the display panel: output, monitor profile, picture style,
/// refresh policy, and a row that opens [`GRID_PAGE`].
pub const DISPLAY_PAGE: PageId = PageId(RESERVED_ID_MIN + 1);
/// The page of the quick menu: the rows a player can change during a game,
/// written at the game level, with rows for [`RESUME`] and [`QUIT_GAME`].
pub const QUICK_MENU_PAGE: PageId = PageId(RESERVED_ID_MIN + 2);

/// The page of the audio panel: the volume and the output device, written
/// at the global level.
pub const AUDIO_PAGE: PageId = PageId(RESERVED_ID_MIN + 3);

/// The grid test pattern. It is not a settings page and the app never
/// loads it: a [`crate::snapshot::SettingValue::Page`] row with this id
/// opens the pattern.
pub const GRID_PAGE: PageId = PageId(RESERVED_ID_MIN + 4);

/// The page that lists the settings panels ([`SETTINGS_PANELS`]). The UI
/// shows it itself and the app never loads it: a
/// [`crate::snapshot::SettingValue::Page`] row with this id opens it.
pub const SETTINGS_PAGE: PageId = PageId(RESERVED_ID_MIN + 5);

/// The video page of the quick menu: the video block, and the display
/// settings of the game, written at the game level.
pub const VIDEO_PAGE: PageId = PageId(RESERVED_ID_MIN + 6);

/// One row of the Settings page: a panel and the page it opens.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Panel {
    /// The label of the row, and the part of the breadcrumb of the panel.
    pub label: &'static str,
    /// The page of the panel. The app answers
    /// [`crate::Intent::LoadSettings`] for it, except for the reserved
    /// pages that the UI shows itself.
    pub page: PageId,
}

/// The rows of the Settings page, in display order. A new panel is one
/// entry here and a page that the app loads; a page other than
/// [`DISPLAY_PAGE`] shows its rows in the generic page screen.
pub static SETTINGS_PANELS: &[Panel] = &[
    Panel {
        label: "Display",
        page: DISPLAY_PAGE,
    },
    Panel {
        label: "Audio",
        page: AUDIO_PAGE,
    },
];

/// A menu row that closes the quick menu and continues the game.
pub const RESUME: ActionId = ActionId(RESERVED_ID_MIN + 1);
/// A menu row that stops the game.
pub const QUIT_GAME: ActionId = ActionId(RESERVED_ID_MIN + 2);

/// The label of the tree row with the video status line of the game: what
/// the core asks for. Its value is [`crate::snapshot::VideoStatus::game`].
pub const VIDEO_GAME_ROW: &str = "video/game";
/// The label of the tree row with the mode on screen. Its value is
/// [`crate::snapshot::VideoStatus::mode`].
pub const VIDEO_MODE_ROW: &str = "video/mode";
/// The label of the tree row with the result in words. Its value is
/// [`crate::snapshot::VideoStatus::result`].
pub const VIDEO_RESULT_ROW: &str = "video/result";

/// The scenes of the screens, added to the registry after the template
/// scenes.
pub static SCENES: &[SceneDef] = scenes::SCENES;
