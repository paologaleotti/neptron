//! What comes into the UI from the player.

/// One navigation action. Every screen works with these alone: a D-pad and
/// two buttons, plus the menu button.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum UiAction {
    /// D-pad up.
    Up,
    /// D-pad down.
    Down,
    /// D-pad left.
    Left,
    /// D-pad right.
    Right,
    /// The accept button.
    Accept,
    /// The back button.
    Back,
    /// The menu button.
    Menu,
}
