//! The screens: which template shows which snapshot data, and the screen
//! stack. [`Screens`] is the router that the app owns and calls once per
//! frame. Each screen registers its scenes in [`SCENES`].
//!
//! The UI and the app agree on a few numbers: the pages of the display
//! panel and of the quick menu, and the menu actions that the UI carries out
//! itself. They are at [`RESERVED_ID_MIN`] and above; the app numbers its
//! own pages and actions below it.

mod chrome;
mod display;
mod game_list;
mod home;
mod options;
mod overlays;
mod page;
mod quick_menu;
mod router;
mod scenes;

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
/// refresh policy, and a row with [`GRID_PATTERN`].
pub const DISPLAY_PAGE: PageId = PageId(RESERVED_ID_MIN + 1);
/// The page of the quick menu: the rows a player can change during a game,
/// written at the game level, with rows for [`RESUME`], [`OPEN_DISPLAY`]
/// and [`QUIT_GAME`].
pub const QUICK_MENU_PAGE: PageId = PageId(RESERVED_ID_MIN + 2);

/// A menu row that closes the quick menu and continues the game.
pub const RESUME: ActionId = ActionId(RESERVED_ID_MIN + 1);
/// A menu row that stops the game.
pub const QUIT_GAME: ActionId = ActionId(RESERVED_ID_MIN + 2);
/// A menu row that opens the display panel.
pub const OPEN_DISPLAY: ActionId = ActionId(RESERVED_ID_MIN + 3);
/// A menu row that shows the grid test pattern.
pub const GRID_PATTERN: ActionId = ActionId(RESERVED_ID_MIN + 4);

/// The scenes of the screens, added to the registry after the template
/// scenes.
pub static SCENES: &[SceneDef] = scenes::SCENES;
