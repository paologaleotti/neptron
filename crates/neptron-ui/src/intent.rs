//! What goes out of the UI: requests that the app carries out. The UI never
//! acts itself.

use crate::snapshot::{ActionId, GameId, GameSource, Level, PageId, ProfileId, SettingKey};

/// A request from the UI to the app.
///
/// The enum is exhaustive on purpose: a new intent must break the build of
/// the app until the app handles it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Intent {
    /// Put the rows `first..first + count` of a game list in
    /// [`crate::Snapshot::games`]. The app may send more rows around them.
    LoadGames {
        /// The list.
        source: GameSource,
        /// The index of the first row needed.
        first: usize,
        /// The number of rows needed.
        count: usize,
    },
    /// Put the rows of a settings page in [`crate::Snapshot::settings`].
    LoadSettings {
        /// The page.
        page: PageId,
    },
    /// Start a game.
    Launch {
        /// The game.
        game: GameId,
    },
    /// Add a game to the favourites or remove it.
    ToggleFavourite {
        /// The game.
        game: GameId,
    },
    /// Write a setting value at a level.
    SetValue {
        /// The setting.
        key: SettingKey,
        /// The level to write.
        level: Level,
        /// The new value.
        value: Value,
    },
    /// Remove the value of a setting at a level, so that it inherits again.
    ResetValue {
        /// The setting.
        key: SettingKey,
        /// The level to clear.
        level: Level,
    },
    /// Run a menu action.
    Action(ActionId),
    /// Change the monitor profile.
    SelectProfile(ProfileId),
    /// Keep the signal change that waits for confirmation.
    KeepMode,
    /// Revert the signal change that waits for confirmation.
    RevertMode,
    /// Close the quick menu and continue the game.
    Resume,
    /// Stop the game and return to the game list.
    QuitGame,
}

/// A new setting value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Value {
    /// On or off.
    Toggle(bool),
    /// An integer.
    Number(i64),
    /// The index of an option.
    Choice(usize),
}
