//! The read-only snapshot of what the UI shows. The app converts the library,
//! the settings schema and the session state into these types, so a change
//! inside those crates cannot break a screen.

/// The number of a system, given by the app.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SystemId(pub u32);

/// The number of a game, given by the app.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct GameId(pub u64);

/// The number of a setting in the settings schema, given by the app.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SettingKey(pub u32);

/// The number of a monitor profile, given by the app.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ProfileId(pub u32);

/// The number of a menu action (a row that does something instead of
/// holding a value), given by the app.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ActionId(pub u32);

/// The number of a settings page (a group of rows, such as the display
/// panel or the quick menu), given by the app.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PageId(pub u32);

/// Which games a game list shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GameSource {
    /// The games of one system.
    System(SystemId),
    /// The favourite games of every system.
    Favourites,
    /// The games played last, newest first.
    Recents,
}

/// A settings level. A value comes from the most specific level that has
/// one: game, then system, then global, then the built-in default.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Level {
    /// The built-in default.
    Default,
    /// Every game.
    Global,
    /// Every game of one system.
    System,
    /// One game.
    Game,
}

/// The levels at which a setting can hold a value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct Scopes {
    /// The global level.
    pub global: bool,
    /// The system level.
    pub system: bool,
    /// The game level.
    pub game: bool,
}

/// Everything the UI can show in one frame.
///
/// The app keeps one snapshot and passes it by reference each frame; the UI
/// never copies it. The systems are complete. The games and the settings
/// rows are the data of one view each: the UI asks for a view with
/// [`crate::Intent::LoadGames`] or [`crate::Intent::LoadSettings`], and the
/// app answers in a later snapshot. [`GameWindow`] and [`SettingsPage`] say
/// which view their data belongs to, so a screen can tell old data from
/// current data.
///
/// The type is `#[non_exhaustive]` so that a new field does not break the
/// app: start from [`Snapshot::default`] and set the fields.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct Snapshot {
    /// Every system that contains games, in display order.
    pub systems: Vec<SystemEntry>,
    /// A window of rows of one game list.
    pub games: GameWindow,
    /// The rows of one settings page.
    pub settings: SettingsPage,
    /// The monitor profiles the player can pick.
    pub profiles: Vec<ProfileEntry>,
    /// The active monitor profile.
    pub active_profile: Option<ProfileId>,
    /// The output and the mode on screen now.
    pub display: Option<DisplayInfo>,
    /// A signal change that waits for the player to keep it.
    pub confirm: Option<PendingConfirm>,
    /// The game that runs now, if any.
    pub session: Option<SessionInfo>,
    /// Messages for the player, oldest first.
    pub notices: Vec<Notice>,
    /// What the running game asks for and what the screen shows, as text.
    /// The screens show it only while [`Snapshot::session`] holds a game.
    pub video: Option<VideoStatus>,
}

/// A window of rows of one game list: `rows[0]` is the game at index
/// `first` of a list of `total` games. A list of 10,000 games needs only
/// the rows around the cursor in the snapshot.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct GameWindow {
    /// The list the rows belong to. `None` before the app loaded a list.
    pub source: Option<GameSource>,
    /// The index of `rows[0]` in the whole list.
    pub first: usize,
    /// The number of games in the whole list.
    pub total: usize,
    /// The rows, in display order.
    pub rows: Vec<GameEntry>,
}

/// The rows of one settings page.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct SettingsPage {
    /// The page the rows belong to. `None` before the app loaded a page.
    pub page: Option<PageId>,
    /// The title of the page.
    pub title: String,
    /// The rows, in display order.
    pub rows: Vec<MenuItem>,
}

/// One system of the home screen.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct SystemEntry {
    /// The system.
    pub id: SystemId,
    /// The display name.
    pub name: String,
    /// The number of games.
    pub game_count: u32,
}

/// One game of a game list.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct GameEntry {
    /// The game.
    pub id: GameId,
    /// The system of the game.
    pub system: SystemId,
    /// The display title.
    pub title: String,
    /// True when the player marked the game as a favourite.
    pub favourite: bool,
}

/// One row of a settings page.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct MenuItem {
    /// The setting, or the action's key for an action row.
    pub key: SettingKey,
    /// The label.
    pub label: String,
    /// The current value and its kind.
    pub value: SettingValue,
    /// The level that the current value comes from.
    pub source: Level,
    /// The levels that can hold a value of this setting.
    pub scopes: Scopes,
    /// True when a change applies at once, with no restart of the game.
    pub live: bool,
    /// Why the setting has no effect now, for example `Not used on a flat
    /// panel`. The row shows the reason in place of its value and takes no
    /// change. `None` when the row is enabled.
    pub unavailable: Option<String>,
}

/// The value of a settings row, with what the UI needs to change it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SettingValue {
    /// On or off.
    Toggle(bool),
    /// An integer in a range.
    Number {
        /// The value.
        value: i64,
        /// The smallest value.
        min: i64,
        /// The largest value.
        max: i64,
        /// The change for one press. Zero or less means 1.
        step: i64,
        /// The unit shown after the value, for example `Hz`. It can be empty.
        unit: String,
    },
    /// One option of a list.
    Choice {
        /// The selected option.
        index: usize,
        /// The options, in display order.
        options: Vec<ChoiceOption>,
    },
    /// A row that does something when the player accepts it.
    Action(ActionId),
    /// A row that opens a page when the player accepts it: a settings
    /// page, or a screen of the UI with a reserved page id such as
    /// [`crate::screens::GRID_PAGE`].
    Page(PageId),
}

/// One option of a [`SettingValue::Choice`]: its label, what it gives, and
/// whether the player can choose it now.
///
/// The display panel uses the detail for the line rate and the field rate
/// that a picture style gives, and the reason for a style that the monitor
/// cannot show.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct ChoiceOption {
    /// The label, for example `2x`.
    pub label: String,
    /// What the option gives, for example `31.5 kHz 59.94 Hz`. It can be
    /// empty.
    pub detail: String,
    /// Why the player cannot choose the option now, for example
    /// `Needs a 45 kHz monitor`. `None` when the player can choose it.
    pub unavailable: Option<String>,
    /// A short form of the label for the value column of a settings row,
    /// for example `HDMI/DP 1` for `HDMI/DP 1 (HDA Intel PCH)`. `None`
    /// shows the label. The option list always shows the label.
    pub short: Option<String>,
}

/// One monitor profile.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct ProfileEntry {
    /// The profile.
    pub id: ProfileId,
    /// The display name.
    pub name: String,
}

/// The output and the mode on screen.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct DisplayInfo {
    /// The output, for example `VGA-1`.
    pub output: String,
    /// The mode as the player reads it, for example `320x240 59.92 Hz`.
    pub mode: String,
    /// The name of the monitor when it is not one of the profiles (a
    /// monitor of its EDID, for example `NPT FHD 24`); empty otherwise.
    pub monitor: String,
}

/// A signal change that reverts unless the player keeps it.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct PendingConfirm {
    /// What changed, as the player reads it.
    pub description: String,
    /// Time left before the revert, in nanoseconds.
    pub remaining_ns: u64,
    /// The confirmation. Each signal change that waits for the player gets
    /// a new id, also one that replaces another at once.
    pub id: ConfirmId,
}

/// The number of a confirmation, given by the app.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ConfirmId(pub u32);

/// The game that runs.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct SessionInfo {
    /// The game.
    pub game: GameId,
    /// Its display title.
    pub title: String,
    /// True while the core is paused.
    pub paused: bool,
}

/// The video status of the running game, as text that the app makes. The
/// UI draws the text and decides nothing about it.
///
/// The app keeps one value and writes new text into the same strings when
/// the status changes, so a frame allocates nothing.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct VideoStatus {
    /// What the core asks for: the source size and the field rate, for
    /// example `256x224  60.099 Hz`.
    pub game: String,
    /// The mode on screen as the kernel reports it, and the picture style,
    /// for example `768x448p 30.05 kHz 60.099 Hz  2x`.
    pub mode: String,
    /// The result in words, for example `exact`, `filtered` or
    /// `[!] cropped 2 lines, speed (59.940 to 60.000 Hz)`.
    pub result: String,
    /// True when the player must notice the result: the UI draws it with
    /// emphasis.
    pub alert: bool,
}

/// The number of a notice, given by the app. Two notices in one snapshot
/// have different numbers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NoticeId(pub u32);

/// A message for the player. It stays in the snapshot until the player
/// dismisses it ([`crate::Intent::DismissNotice`]) or the app removes it.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Notice {
    /// The notice.
    pub id: NoticeId,
    /// The text.
    pub text: String,
    /// How serious it is.
    pub kind: NoticeKind,
}

/// How serious a notice is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NoticeKind {
    /// For information.
    Info,
    /// Something the player should know about.
    Warning,
    /// Something failed.
    Error,
}

impl Scopes {
    /// No level: the setting cannot be changed.
    pub const NONE: Self = Self {
        global: false,
        system: false,
        game: false,
    };
    /// Every level.
    pub const ALL: Self = Self {
        global: true,
        system: true,
        game: true,
    };

    /// True when `level` can hold a value. The default level never can.
    pub fn allows(&self, level: Level) -> bool {
        match level {
            Level::Default => false,
            Level::Global => self.global,
            Level::System => self.system,
            Level::Game => self.game,
        }
    }
}

impl GameWindow {
    /// A window of `rows` that starts at index `first` of a list of `total`
    /// games from `source`.
    pub fn new(source: GameSource, first: usize, total: usize, rows: Vec<GameEntry>) -> Self {
        Self {
            source: Some(source),
            first,
            total,
            rows,
        }
    }

    /// True when the rows belong to the list of `source`.
    pub fn is_for(&self, source: GameSource) -> bool {
        self.source == Some(source)
    }

    /// The game at `index` of the whole list, when the window holds it.
    pub fn get(&self, index: usize) -> Option<&GameEntry> {
        self.rows.get(index.checked_sub(self.first)?)
    }

    /// True when the window holds the rows `first..first + count` of the
    /// list of `source`, as far as the list has them.
    pub fn covers(&self, source: GameSource, first: usize, count: usize) -> bool {
        let end = first.saturating_add(count).min(self.total);
        let held = self.first.saturating_add(self.rows.len());
        self.is_for(source) && (first >= end || (first >= self.first && end <= held))
    }
}

impl SettingsPage {
    /// The rows of `page`, under `title`.
    pub fn new(page: PageId, title: impl Into<String>, rows: Vec<MenuItem>) -> Self {
        Self {
            page: Some(page),
            title: title.into(),
            rows,
        }
    }

    /// True when the rows belong to `page`.
    pub fn is_for(&self, page: PageId) -> bool {
        self.page == Some(page)
    }
}

impl SystemEntry {
    /// A system with its name and game count.
    pub fn new(id: SystemId, name: impl Into<String>, game_count: u32) -> Self {
        Self {
            id,
            name: name.into(),
            game_count,
        }
    }
}

impl GameEntry {
    /// A game that is not a favourite.
    pub fn new(id: GameId, system: SystemId, title: impl Into<String>) -> Self {
        Self {
            id,
            system,
            title: title.into(),
            favourite: false,
        }
    }
}

impl MenuItem {
    /// A row with the value from the default level, no level the player
    /// can write, and no live change. Set `source`, `scopes` and `live`
    /// after.
    pub fn new(key: SettingKey, label: impl Into<String>, value: SettingValue) -> Self {
        Self {
            key,
            label: label.into(),
            value,
            source: Level::Default,
            scopes: Scopes::NONE,
            live: false,
            unavailable: None,
        }
    }

    /// The same row, disabled for `reason`.
    pub fn unavailable(mut self, reason: impl Into<String>) -> Self {
        self.unavailable = Some(reason.into());
        self
    }

    /// True when the row takes a change.
    pub fn is_enabled(&self) -> bool {
        self.unavailable.is_none()
    }
}

impl ChoiceOption {
    /// An option that the player can choose, with a label and no detail.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            detail: String::new(),
            unavailable: None,
            short: None,
        }
    }

    /// The same option with `short` in the value column of its row.
    pub fn with_short(mut self, short: impl Into<String>) -> Self {
        self.short = Some(short.into());
        self
    }

    /// The text of the option in the value column of a settings row: the
    /// short form, or the label.
    pub fn value_text(&self) -> &str {
        self.short.as_deref().unwrap_or(&self.label)
    }

    /// The same option with a detail text.
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = detail.into();
        self
    }

    /// The same option, marked as one the player cannot choose, for
    /// `reason`.
    pub fn unavailable(mut self, reason: impl Into<String>) -> Self {
        self.unavailable = Some(reason.into());
        self
    }

    /// True when the player can choose the option.
    pub fn is_enabled(&self) -> bool {
        self.unavailable.is_none()
    }
}

impl From<&str> for ChoiceOption {
    fn from(label: &str) -> Self {
        Self::new(label)
    }
}

impl From<String> for ChoiceOption {
    fn from(label: String) -> Self {
        Self::new(label)
    }
}

impl ProfileEntry {
    /// A profile with its name.
    pub fn new(id: ProfileId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
        }
    }
}

impl DisplayInfo {
    /// An output and its mode.
    pub fn new(output: impl Into<String>, mode: impl Into<String>) -> Self {
        Self {
            output: output.into(),
            mode: mode.into(),
            monitor: String::new(),
        }
    }

    /// The same with the name of a monitor that is not one of the
    /// profiles.
    pub fn monitor(mut self, name: impl Into<String>) -> Self {
        self.monitor = name.into();
        self
    }
}

impl PendingConfirm {
    /// A change with its id and the time left before it reverts.
    pub fn new(id: ConfirmId, description: impl Into<String>, remaining_ns: u64) -> Self {
        Self {
            description: description.into(),
            remaining_ns,
            id,
        }
    }
}

impl SessionInfo {
    /// A running game that is not paused.
    pub fn new(game: GameId, title: impl Into<String>) -> Self {
        Self {
            game,
            title: title.into(),
            paused: false,
        }
    }
}

impl VideoStatus {
    /// A status with its three texts and no alert.
    pub fn new(
        game: impl Into<String>,
        mode: impl Into<String>,
        result: impl Into<String>,
    ) -> Self {
        Self {
            game: game.into(),
            mode: mode.into(),
            result: result.into(),
            alert: false,
        }
    }

    /// The same status with the alert set to `alert`.
    pub fn with_alert(mut self, alert: bool) -> Self {
        self.alert = alert;
        self
    }
}

impl Notice {
    /// A message of a kind, with its number.
    pub fn new(id: NoticeId, text: impl Into<String>, kind: NoticeKind) -> Self {
        Self {
            id,
            text: text.into(),
            kind,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scopes_never_allow_the_default_level() {
        let all = Scopes {
            global: true,
            system: true,
            game: true,
        };
        assert!(!all.allows(Level::Default));
        assert!(all.allows(Level::Game));
        let global_only = Scopes {
            global: true,
            ..Scopes::default()
        };
        assert!(global_only.allows(Level::Global));
        assert!(!global_only.allows(Level::System));
    }

    fn games(first: usize, n: usize) -> Vec<GameEntry> {
        (first..first + n)
            .map(|i| GameEntry::new(GameId(i as u64), SystemId(1), format!("Game {i}")))
            .collect()
    }

    #[test]
    fn a_game_window_maps_list_indices_to_rows() {
        let nes = GameSource::System(SystemId(1));
        let w = GameWindow::new(nes, 100, 10_000, games(100, 50));
        assert!(w.is_for(nes));
        assert!(!w.is_for(GameSource::Recents));
        assert_eq!(w.get(100).map(|g| g.id), Some(GameId(100)));
        assert_eq!(w.get(149).map(|g| g.id), Some(GameId(149)));
        assert_eq!(w.get(99), None);
        assert_eq!(w.get(150), None);
        assert_eq!(w.get(usize::MAX), None);
    }

    #[test]
    fn a_game_window_knows_what_it_covers() {
        let nes = GameSource::System(SystemId(1));
        let w = GameWindow::new(nes, 100, 120, games(100, 20));
        assert!(w.covers(nes, 100, 20));
        assert!(w.covers(nes, 110, 50));
        assert!(!w.covers(nes, 90, 20));
        assert!(!w.covers(GameSource::Favourites, 100, 5));
        assert!(w.covers(nes, usize::MAX, usize::MAX));
        assert!(!GameWindow::default().covers(nes, 0, 1));
    }

    #[test]
    fn a_settings_page_names_its_page() {
        let page = SettingsPage::new(PageId(3), "Display", Vec::new());
        assert!(page.is_for(PageId(3)));
        assert!(!page.is_for(PageId(4)));
        assert!(!SettingsPage::default().is_for(PageId(3)));
    }

    #[test]
    fn constructors_take_safe_defaults() {
        let item = MenuItem::new(SettingKey(1), "Interlace", SettingValue::Toggle(false));
        assert_eq!(
            (item.source, item.scopes, item.live),
            (Level::Default, Scopes::NONE, false)
        );
        assert!(!GameEntry::new(GameId(1), SystemId(1), "x").favourite);
        assert!(!SessionInfo::new(GameId(1), "x").paused);
        let notice = Notice::new(NoticeId(7), "Cannot start the game.", NoticeKind::Error);
        assert_eq!((notice.id, notice.kind), (NoticeId(7), NoticeKind::Error));
    }

    #[test]
    fn a_choice_option_is_enabled_until_it_has_a_reason() {
        let option = ChoiceOption::new("2x").with_detail("31.5 kHz 59.94 Hz");
        assert!(option.is_enabled());
        assert_eq!(option.detail, "31.5 kHz 59.94 Hz");
        let option = option.unavailable("Needs a 31 kHz monitor");
        assert!(!option.is_enabled());
        assert_eq!(ChoiceOption::from("Auto"), ChoiceOption::new("Auto"));
        assert_eq!(ChoiceOption::from(String::from("Auto")).detail, "");
    }

    #[test]
    fn a_choice_option_shows_its_short_form_in_the_value_column() {
        let option = ChoiceOption::new("HDMI/DP 1 (HDA Intel PCH)");
        assert_eq!(option.value_text(), "HDMI/DP 1 (HDA Intel PCH)");
        let option = option.with_short("HDMI/DP 1");
        assert_eq!(option.value_text(), "HDMI/DP 1");
        assert_eq!(option.label, "HDMI/DP 1 (HDA Intel PCH)");
    }

    #[test]
    fn a_video_status_has_no_alert_until_it_is_set() {
        let status = VideoStatus::new("256x224  60.099 Hz", "768x448p", "exact");
        assert!(!status.alert);
        assert_eq!(status.result, "exact");
        assert!(status.with_alert(true).alert);
        assert_eq!(Snapshot::default().video, None);
    }

    #[test]
    fn levels_order_from_least_to_most_specific() {
        assert!(Level::Default < Level::Global);
        assert!(Level::Global < Level::System);
        assert!(Level::System < Level::Game);
    }
}
