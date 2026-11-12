//! Navigation tests: sequences of actions against the intents and the
//! widget tree they give. No test looks at pixels.

use crate::frame::UiFrame;
use crate::input::UiAction::{self, Accept, Back, Down, Left, Menu, Right, Up};
use crate::intent::{Intent, Value};
use crate::snapshot::{
    ConfirmId, GameId, GameSource, GameWindow, Level, Notice, NoticeId, NoticeKind, PendingConfirm,
    SessionInfo, SettingsPage, Snapshot, VideoStatus,
};
use crate::theme::Theme;

use super::game_list::{PREFETCH_ROWS, WINDOW_ROWS};
use super::overlays::QUIET_FRAMES_BEFORE_MOVE;
use super::scenes::{
    LONG_LIST, MEGA_DRIVE, REFRESH, SNES, STYLE, TO_DISPLAY, exact_status, filtered_status,
    in_game, in_game_with, long_status, window, with_display, with_systems,
};
use super::{
    DISPLAY_PAGE, QUICK_MENU_PAGE, Screens, VIDEO_GAME_ROW, VIDEO_MODE_ROW, VIDEO_RESULT_ROW,
};

pub(super) struct Run {
    pub screens: Screens,
    pub frame: UiFrame,
}

impl Run {
    pub fn new() -> Self {
        Self {
            screens: Screens::new(Theme::default()),
            frame: UiFrame::new(),
        }
    }

    pub fn step(&mut self, snap: &Snapshot, actions: &[UiAction]) -> &UiFrame {
        self.screens.update(snap, actions, &mut self.frame);
        &self.frame
    }

    pub fn screen(&self) -> &'static str {
        self.frame.tree.screen()
    }

    pub fn focused(&self) -> Option<&str> {
        self.frame.tree.focused().map(|r| r.label)
    }

    pub fn labels(&self) -> Vec<&str> {
        self.frame.tree.rows().map(|r| r.label).collect()
    }

    pub fn intents(&self) -> &[Intent] {
        &self.frame.intents
    }

    pub fn loads(&self) -> Vec<(usize, usize)> {
        self.frame
            .intents
            .iter()
            .filter_map(|i| match *i {
                Intent::LoadGames { first, count, .. } => Some((first, count)),
                _ => None,
            })
            .collect()
    }
}

const SNES_LIST: GameSource = GameSource::System(SNES);

/// A run on the game list of the Super Nintendo, with its first window.
fn snes_list() -> (Run, Snapshot) {
    let mut run = Run::new();
    let mut snap = with_systems();
    run.step(&snap, &[Down, Accept]);
    snap.games = window(SNES_LIST, 0, LONG_LIST, WINDOW_ROWS);
    run.step(&snap, &[]);
    (run, snap)
}

#[test]
fn home_lists_the_systems_then_favourites_recents_and_display() {
    let mut run = Run::new();
    run.step(&with_systems(), &[]);
    assert_eq!(run.screen(), "home");
    let rows: Vec<_> = run
        .frame
        .tree
        .rows()
        .filter(|r| r.widget == crate::WidgetKind::List)
        .map(|r| (r.label, r.value))
        .collect();
    assert_eq!(
        rows,
        [
            ("Nintendo Entertainment System", Some("312 games")),
            ("Super Nintendo", Some("10000 games")),
            ("Mega Drive", Some("1 game")),
            ("Favourites", None),
            ("Recent games", None),
            ("Display", None),
        ]
    );
    assert_eq!(run.focused(), Some("Nintendo Entertainment System"));
    assert!(run.intents().is_empty());
}

#[test]
fn home_with_no_systems_says_where_the_games_go() {
    let mut run = Run::new();
    run.step(&Snapshot::default(), &[Down, Up, Back, Menu]);
    assert_eq!(run.screen(), "home");
    assert_eq!(run.focused(), Some("Display"));
    let text = run.labels().join(" ");
    for folder in ["roms/nes", "roms/snes", "roms/megadrive", ".zip"] {
        assert!(text.contains(folder), "{folder} is missing from {text:?}");
    }
    assert!(
        run.frame.tree.rows().all(|r| !r.clipped),
        "{:?}",
        run.labels()
    );
}

#[test]
fn a_system_opens_its_game_list_and_back_returns_home() {
    let mut run = Run::new();
    let snap = with_systems();
    run.step(&snap, &[Down, Accept]);
    assert_eq!(run.screen(), "games");
    assert_eq!(
        run.intents(),
        [Intent::LoadGames {
            source: SNES_LIST,
            first: 0,
            count: WINDOW_ROWS
        }]
    );
    run.step(&snap, &[Back]);
    assert_eq!(run.screen(), "home");
    assert_eq!(run.focused(), Some("Super Nintendo"));
    run.step(&snap, &[Back, Back]);
    assert_eq!(run.screen(), "home");
}

#[test]
fn favourites_and_recents_open_their_lists() {
    let mut run = Run::new();
    let snap = with_systems();
    run.step(&snap, &[Down, Down, Down, Accept]);
    assert_eq!(run.loads(), [(0, WINDOW_ROWS)]);
    assert!(matches!(
        run.intents()[0],
        Intent::LoadGames {
            source: GameSource::Favourites,
            ..
        }
    ));
    run.step(&snap, &[Back, Down, Accept]);
    assert!(matches!(
        run.intents()[0],
        Intent::LoadGames {
            source: GameSource::Recents,
            ..
        }
    ));
}

#[test]
fn the_game_list_asks_for_nothing_while_the_window_covers_the_need() {
    let (mut run, snap) = snes_list();
    assert!(run.loads().is_empty());
    assert_eq!(run.focused(), Some(super::scenes::title(0).as_str()));
    let visible = run
        .frame
        .tree
        .rows()
        .filter(|r| r.widget == crate::WidgetKind::List)
        .count();
    // The need is the visible rows plus the margin; the window of 128
    // rows covers it until the top row passes 128 - visible - margin.
    let quiet = WINDOW_ROWS - visible - PREFETCH_ROWS;
    for step in 0..quiet + visible - 1 {
        run.step(&snap, &[Down]);
        assert!(run.loads().is_empty(), "step {step}");
    }
    run.step(&snap, &[Down]);
    let top = quiet + 1;
    let first = top + visible / 2 - WINDOW_ROWS / 2;
    assert_eq!(run.loads(), [(first, WINDOW_ROWS)]);
}

#[test]
fn the_game_list_asks_once_per_frame_and_never_shows_a_blank_row() {
    let (mut run, mut snap) = snes_list();
    for _ in 0..200 {
        run.step(&snap, &[Right, Right, Down, Right]);
        let loads = run.loads();
        assert!(loads.len() <= 1, "{loads:?}");
        assert!(
            run.labels().iter().all(|l| *l != "..."),
            "a blank row at {:?}",
            run.focused()
        );
        if let Some(&(first, count)) = loads.first() {
            snap.games = window(SNES_LIST, first, LONG_LIST, count);
        }
    }
}

#[test]
fn the_end_of_a_long_list_asks_for_a_full_window() {
    let (mut run, snap) = snes_list();
    if let Some(super::router::Screen::Games(list)) = run.screens.top_mut() {
        list.select(LONG_LIST - 1, LONG_LIST);
    }
    run.step(&snap, &[]);
    assert_eq!(run.loads(), [(LONG_LIST - WINDOW_ROWS, WINDOW_ROWS)]);
    run.step(&snap, &[Down, Right]);
    assert_eq!(run.focused(), Some("..."));
}

#[test]
fn the_cursor_survives_a_window_of_a_different_list() {
    let (mut run, mut snap) = snes_list();
    run.step(&snap, &[Down, Down, Down]);
    snap.games = GameWindow::new(GameSource::Recents, 0, 0, Vec::new());
    run.step(&snap, &[Down]);
    assert_eq!(run.loads(), [(0, WINDOW_ROWS)]);
    snap.games = window(SNES_LIST, 0, LONG_LIST, WINDOW_ROWS);
    run.step(&snap, &[]);
    assert_eq!(run.focused(), Some(super::scenes::title(3).as_str()));
}

#[test]
fn accept_launches_and_menu_marks_a_favourite() {
    let (mut run, snap) = snes_list();
    run.step(&snap, &[Down, Accept]);
    assert_eq!(run.intents(), [Intent::Launch { game: GameId(1) }]);
    run.step(&snap, &[Menu, Left]);
    assert_eq!(run.intents(), [Intent::ToggleFavourite { game: GameId(1) }]);
}

#[test]
fn the_ui_hides_while_a_game_runs_and_returns_to_the_list() {
    let (mut run, mut snap) = snes_list();
    run.step(&snap, &[Down, Down, Accept]);
    snap.session = Some(SessionInfo::new(GameId(2), "Astro Saga"));
    run.step(&snap, &[]);
    assert_eq!(run.screen(), "game");
    assert!(!run.screens.shows_ui());
    assert!(run.frame.scene.commands().is_empty());
    assert!(run.intents().is_empty());
    run.step(&snap, &[Down, Accept, Back]);
    assert_eq!(run.screen(), "game");
    snap.session = None;
    run.step(&snap, &[]);
    assert!(run.screens.shows_ui());
    assert_eq!(run.screen(), "games");
    assert_eq!(run.focused(), Some(super::scenes::title(2).as_str()));
}

#[test]
fn a_list_with_one_game_and_an_empty_list() {
    let mut run = Run::new();
    let mut snap = with_systems();
    run.step(&snap, &[Down, Down, Accept]);
    snap.games = window(GameSource::System(MEGA_DRIVE), 0, 1, WINDOW_ROWS);
    run.step(&snap, &[Down, Down, Right, Up]);
    assert_eq!(run.focused(), Some(super::scenes::title(0).as_str()));
    assert!(run.loads().is_empty());

    snap.games = window(GameSource::System(MEGA_DRIVE), 0, 0, WINDOW_ROWS);
    run.step(&snap, &[Accept, Menu]);
    assert!(run.intents().is_empty());
    assert_eq!(run.focused(), None);
    assert!(run.labels().iter().any(|l| l.contains("no games")));
}

fn field<'a>(run: &'a Run, name: &str) -> Option<&'a str> {
    run.frame
        .tree
        .rows()
        .find(|r| r.label == name)
        .and_then(|r| r.value)
}

#[test]
fn the_display_panel_asks_for_its_page_and_shows_the_mode() {
    let mut run = Run::new();
    let mut snap = with_display();
    let page = core::mem::take(&mut snap.settings);
    run.step(&snap, &TO_DISPLAY);
    assert_eq!(run.screen(), "display");
    assert_eq!(run.intents(), [Intent::LoadSettings { page: DISPLAY_PAGE }]);
    run.step(&snap, &[Down, Accept]);
    assert_eq!(run.intents(), [Intent::LoadSettings { page: DISPLAY_PAGE }]);
    snap.settings = page;
    run.step(&snap, &[]);
    assert!(run.intents().is_empty());
    assert_eq!(field(&run, "Output"), Some("VGA-1"));
    assert_eq!(field(&run, "Mode"), Some("640x480 59.94 Hz"));
    assert_eq!(field(&run, "Monitor"), Some("pc_31_120"));
    assert_eq!(run.focused(), Some("Output"));
    run.step(&snap, &[Back]);
    assert_eq!(run.screen(), "home");
}

#[test]
fn the_display_panel_never_changes_a_choice_in_place() {
    let mut run = Run::new();
    let snap = with_display();
    run.step(&snap, &TO_DISPLAY);
    run.step(&snap, &[Down, Down]);
    assert_eq!(run.focused(), Some("Picture style"));
    for action in [Left, Right, Left, Right] {
        run.step(&snap, &[action]);
        assert!(run.intents().is_empty(), "{action:?}");
        assert_eq!(run.screen(), "display");
    }
}

#[test]
fn a_picture_style_applies_only_from_its_option_list() {
    let mut run = Run::new();
    let snap = with_display();
    run.step(&snap, &TO_DISPLAY);
    run.step(&snap, &[Down, Down, Accept]);
    assert_eq!(run.screen(), "options");
    assert_eq!(run.focused(), Some("Auto"));
    assert_eq!(field(&run, "In use"), Some("Auto"));
    let rows: Vec<_> = run
        .frame
        .tree
        .rows()
        .filter(|r| r.widget == crate::WidgetKind::List)
        .map(|r| (r.label, r.value))
        .collect();
    assert_eq!(
        rows,
        [
            ("Auto", Some("31.5 kHz  59.94 Hz")),
            ("Native", Some("Needs a 15 kHz monitor")),
            ("2x", Some("31.5 kHz  59.94 Hz")),
            ("3x", Some("Needs a 45 kHz monitor")),
            ("4x", Some("Needs a 60 kHz monitor")),
        ]
    );

    run.step(&snap, &[Down, Accept]);
    assert_eq!(run.screen(), "options");
    assert!(run.intents().is_empty());
    assert!(
        run.labels()
            .contains(&"Not available. Needs a 15 kHz monitor")
    );

    run.step(&snap, &[Down, Accept]);
    assert_eq!(
        run.intents(),
        [Intent::SetValue {
            key: STYLE,
            level: Level::Global,
            value: Value::Choice(2)
        }]
    );
    assert_eq!(run.screen(), "display");
}

#[test]
fn the_option_in_use_or_back_closes_the_list_with_no_change() {
    let mut run = Run::new();
    let snap = with_display();
    run.step(&snap, &TO_DISPLAY);
    run.step(&snap, &[Down, Down, Accept]);
    run.step(&snap, &[Accept]);
    assert!(run.intents().is_empty());
    assert_eq!(run.screen(), "display");
    run.step(&snap, &[Accept, Down, Down, Back]);
    assert!(run.intents().is_empty());
    assert_eq!(run.screen(), "display");
}

#[test]
fn the_option_list_waits_for_its_page() {
    let mut run = Run::new();
    let mut snap = with_display();
    run.step(&snap, &TO_DISPLAY);
    run.step(&snap, &[Down, Down, Accept]);
    snap.settings = SettingsPage::default();
    run.step(&snap, &[Down, Accept]);
    assert_eq!(run.intents(), [Intent::LoadSettings { page: DISPLAY_PAGE }]);
    assert_eq!(run.screen(), "options");
    snap = with_display();
    run.step(&snap, &[]);
    assert!(run.intents().is_empty());
    assert_eq!(run.focused(), Some("Auto"));
}

const SECOND_NS: u64 = 1_000_000_000;

/// A run on the display panel with a confirmation of `remaining_ns`.
fn confirming(remaining_ns: u64) -> (Run, Snapshot) {
    let mut run = Run::new();
    let mut snap = with_display();
    run.step(&snap, &TO_DISPLAY);
    snap.confirm = Some(PendingConfirm::new(
        ConfirmId(1),
        "640x480 at 59.94 Hz",
        remaining_ns,
    ));
    run.step(&snap, &[]);
    (run, snap)
}

/// Steps the frames with no action after which the confirm dialog takes
/// left and right.
fn release(run: &mut Run, snap: &Snapshot) {
    for _ in 0..QUIET_FRAMES_BEFORE_MOVE {
        run.step(snap, &[]);
        assert!(run.intents().is_empty());
    }
}

fn message(run: &Run) -> &str {
    run.frame
        .tree
        .rows()
        .find(|r| r.widget == crate::WidgetKind::Dialog)
        .map_or("", |r| r.label)
}

#[test]
fn a_blind_press_reverts_the_signal_change() {
    let (mut run, snap) = confirming(10 * SECOND_NS);
    assert_eq!(run.screen(), "confirm");
    assert_eq!(run.focused(), Some("Revert"));
    run.step(&snap, &[Accept]);
    assert_eq!(run.intents(), [Intent::RevertMode]);

    let (mut run, snap) = confirming(10 * SECOND_NS);
    run.step(&snap, &[Back]);
    assert_eq!(run.intents(), [Intent::RevertMode]);

    let (mut run, snap) = confirming(10 * SECOND_NS);
    for action in [Up, Down, Menu, Left] {
        run.step(&snap, &[action]);
        assert!(run.intents().is_empty(), "{action:?}");
    }
    assert_eq!(run.focused(), Some("Revert"));
}

#[test]
fn keep_needs_a_move_to_keep_then_accept() {
    let (mut run, snap) = confirming(10 * SECOND_NS);
    release(&mut run, &snap);
    run.step(&snap, &[Right]);
    assert!(run.intents().is_empty());
    assert_eq!(run.focused(), Some("Keep"));
    run.step(&snap, &[Accept]);
    assert_eq!(run.intents(), [Intent::KeepMode]);
    run.step(&snap, &[Accept, Back, Left, Accept]);
    assert!(run.intents().is_empty());
    assert!(message(&run).contains("One moment."));
}

#[test]
fn a_held_right_never_moves_the_focus_to_keep() {
    let (mut run, snap) = confirming(10 * SECOND_NS);
    for _ in 0..200 {
        run.step(&snap, &[Right]);
        assert_eq!(run.focused(), Some("Revert"));
    }
    run.step(&snap, &[Accept]);
    assert_eq!(run.intents(), [Intent::RevertMode]);
}

#[test]
fn key_repeat_with_gaps_never_moves_the_focus_to_keep() {
    let mut run = Run::new();
    let mut snap = with_display();
    run.step(&snap, &TO_DISPLAY);
    run.step(&snap, &[Right]);
    snap.confirm = Some(PendingConfirm::new(ConfirmId(4), "2x", 10 * SECOND_NS));
    for _ in 0..23 {
        run.step(&snap, &[]);
    }
    for frame in 0..300 {
        let actions: &[UiAction] = if frame % 4 == 0 { &[Right] } else { &[] };
        run.step(&snap, actions);
        assert_eq!(run.focused(), Some("Revert"), "frame {frame}");
    }
    run.step(&snap, &[Accept]);
    assert_eq!(run.intents(), [Intent::RevertMode]);
}

#[test]
fn up_and_down_also_count_as_a_held_key() {
    let (mut run, snap) = confirming(10 * SECOND_NS);
    // `confirming` already stepped one quiet frame.
    for _ in 0..QUIET_FRAMES_BEFORE_MOVE - 2 {
        run.step(&snap, &[]);
    }
    run.step(&snap, &[Down]);
    run.step(&snap, &[Right]);
    assert_eq!(run.focused(), Some("Revert"));
    release(&mut run, &snap);
    run.step(&snap, &[Right, Accept]);
    assert_eq!(run.intents(), [Intent::KeepMode]);
}

#[test]
fn every_new_confirmation_starts_on_revert() {
    let (mut run, mut snap) = confirming(10 * SECOND_NS);
    release(&mut run, &snap);
    run.step(&snap, &[Right]);
    snap.confirm = None;
    run.step(&snap, &[]);
    assert_eq!(run.screen(), "display");
    snap.confirm = Some(PendingConfirm::new(
        ConfirmId(1),
        "640x480 at 59.94 Hz",
        9 * SECOND_NS,
    ));
    run.step(&snap, &[Accept]);
    assert_eq!(run.intents(), [Intent::RevertMode]);
}

#[test]
fn a_confirmation_that_replaces_another_at_once_starts_again() {
    let (mut run, mut snap) = confirming(4 * SECOND_NS);
    release(&mut run, &snap);
    run.step(&snap, &[Right]);
    assert_eq!(run.focused(), Some("Keep"));
    snap.confirm = Some(PendingConfirm::new(
        ConfirmId(2),
        "320x240 at 60 Hz",
        2 * SECOND_NS,
    ));
    run.step(&snap, &[]);
    assert_eq!(run.focused(), Some("Revert"));

    run.step(&snap, &[Back]);
    assert_eq!(run.intents(), [Intent::RevertMode]);
    snap.confirm = Some(PendingConfirm::new(
        ConfirmId(3),
        "288 lines at 50 Hz",
        10 * SECOND_NS,
    ));
    run.step(&snap, &[Accept]);
    assert_eq!(
        run.intents(),
        [Intent::RevertMode],
        "the new one takes an answer"
    );
}

#[test]
fn more_time_left_with_the_same_id_is_the_same_confirmation() {
    let (mut run, mut snap) = confirming(4 * SECOND_NS);
    release(&mut run, &snap);
    run.step(&snap, &[Right]);
    snap.confirm = Some(PendingConfirm::new(
        ConfirmId(1),
        "640x480 at 59.94 Hz",
        9 * SECOND_NS,
    ));
    run.step(&snap, &[]);
    assert_eq!(run.focused(), Some("Keep"));
}

#[test]
fn the_countdown_comes_from_the_snapshot() {
    for (remaining_ns, text) in [
        (10 * SECOND_NS, "comes back in 10 s."),
        (2_400_000_000, "comes back in 3 s."),
        (1, "comes back in 1 s."),
        (0, "comes back now."),
    ] {
        let (run, _) = confirming(remaining_ns);
        assert!(
            message(&run).contains(text),
            "{remaining_ns}: {:?}",
            message(&run)
        );
        assert!(message(&run).starts_with("640x480 at 59.94 Hz"));
    }
}

#[test]
fn a_confirmation_shows_over_a_running_game() {
    let (mut run, mut snap) = snes_list();
    snap.session = Some(SessionInfo::new(GameId(0), "Astro Quest"));
    run.step(&snap, &[]);
    assert!(!run.screens.shows_ui());
    snap.confirm = Some(PendingConfirm::new(
        ConfirmId(7),
        "288 lines at 50 Hz",
        10 * SECOND_NS,
    ));
    run.step(&snap, &[]);
    assert!(run.screens.shows_ui());
    assert_eq!(run.screen(), "confirm");
    run.step(&snap, &[Accept]);
    assert_eq!(run.intents(), [Intent::RevertMode]);
    snap.confirm = None;
    run.step(&snap, &[]);
    assert!(!run.screens.shows_ui());
}

fn notice(id: u32, text: &str) -> Notice {
    Notice::new(NoticeId(id), text, NoticeKind::Error)
}

#[test]
fn notices_show_one_at_a_time_until_dismissed() {
    let (mut run, mut snap) = snes_list();
    snap.notices = vec![notice(4, "First"), notice(9, "Second")];
    run.step(&snap, &[]);
    assert_eq!(run.screen(), "notice");
    assert_eq!(message(&run), "First");
    run.step(&snap, &[Down, Right, Menu]);
    assert!(
        run.intents()
            .iter()
            .all(|i| matches!(i, Intent::LoadGames { .. }))
    );
    assert_eq!(message(&run), "First");
    run.step(&snap, &[Accept]);
    assert_eq!(run.intents(), [Intent::DismissNotice(NoticeId(4))]);
    assert_eq!(message(&run), "Second");
    run.step(&snap, &[Back]);
    assert_eq!(run.intents(), [Intent::DismissNotice(NoticeId(9))]);
    assert_eq!(run.screen(), "games");
    snap.notices.clear();
    run.step(&snap, &[]);
    assert_eq!(run.screen(), "games");
    snap.notices = vec![notice(4, "First again")];
    run.step(&snap, &[]);
    assert_eq!(message(&run), "First again");
}

#[test]
fn notices_wait_while_a_game_runs_and_yield_to_a_confirmation() {
    let (mut run, mut snap) = snes_list();
    snap.session = Some(SessionInfo::new(GameId(0), "Astro Quest"));
    snap.notices = vec![notice(1, "Forced to 60 Hz")];
    run.step(&snap, &[Accept]);
    assert_eq!(run.screen(), "game");
    assert!(run.intents().is_empty());
    snap.confirm = Some(PendingConfirm::new(
        ConfirmId(7),
        "288 lines at 50 Hz",
        SECOND_NS,
    ));
    run.step(&snap, &[]);
    assert_eq!(run.screen(), "confirm");
    snap.session = None;
    snap.confirm = None;
    run.step(&snap, &[]);
    assert_eq!(run.screen(), "notice");
}

#[test]
fn the_title_bar_of_a_notice_has_the_colour_of_its_kind() {
    let theme = Theme::default();
    let kinds = [
        (NoticeKind::Info, "Note", theme.colors.accent),
        (NoticeKind::Warning, "Warning", theme.colors.warning),
        (NoticeKind::Error, "Error", theme.colors.error),
    ];
    for (kind, title, colour) in kinds {
        let (mut run, mut snap) = snes_list();
        snap.notices = vec![Notice::new(NoticeId(1), "Text", kind)];
        run.step(&snap, &[]);
        let bar = run
            .frame
            .tree
            .rows()
            .find(|r| r.label == title)
            .map(|r| r.rect)
            .expect("title row");
        let fill = run.frame.scene.commands().iter().find_map(|c| match *c {
            crate::DrawCommand::Rect { rect, color } if rect == bar => Some(color),
            _ => None,
        });
        assert_eq!(fill, Some(colour), "{title}");
    }
}

/// A run with the quick menu open over a running game.
fn quick_menu() -> (Run, Snapshot) {
    let mut run = Run::new();
    let snap = in_game();
    run.step(&snap, &[]);
    run.step(&snap, &[Menu]);
    (run, snap)
}

/// The path from the top of the quick menu to "Force 60 Hz".
const TO_FORCE_60: [UiAction; 7] = [Down, Down, Accept, Down, Down, Down, Accept];

fn force_60(level: Level) -> Intent {
    Intent::SetValue {
        key: REFRESH,
        level,
        value: Value::Choice(3),
    }
}

#[test]
fn menu_opens_the_quick_menu_over_the_game_and_back_resumes() {
    let mut run = Run::new();
    let mut snap = in_game();
    snap.settings = SettingsPage::default();
    run.step(&snap, &[Up, Accept, Back]);
    assert!(!run.screens.shows_ui());
    run.step(&snap, &[Menu]);
    assert!(run.screens.shows_ui());
    assert_eq!(run.screen(), "quick-menu");
    assert_eq!(
        run.intents(),
        [Intent::LoadSettings {
            page: QUICK_MENU_PAGE
        }]
    );
    snap = in_game();
    run.step(&snap, &[]);
    assert_eq!(run.focused(), Some("Resume"));
    run.step(&snap, &[Back]);
    assert_eq!(run.intents(), [Intent::Resume]);
    assert!(!run.screens.shows_ui());
    run.step(&snap, &[Menu, Accept]);
    assert_eq!(run.intents(), [Intent::Resume]);
    assert_eq!(run.screen(), "game");
}

#[test]
fn quit_stops_the_game_and_the_list_comes_back() {
    let (mut run, mut snap) = quick_menu();
    run.step(&snap, &[Up, Down, Down, Down, Down]);
    assert_eq!(run.focused(), Some("Quit game"));
    run.step(&snap, &[Accept]);
    assert_eq!(run.intents(), [Intent::QuitGame]);
    run.step(&snap, &[Accept, Back]);
    assert!(run.intents().is_empty());
    assert_eq!(run.screen(), "quick-menu");
    snap.session = None;
    run.step(&snap, &[]);
    assert_eq!(run.screen(), "home");
}

#[test]
fn a_change_in_the_quick_menu_is_written_for_the_game() {
    let (mut run, snap) = quick_menu();
    run.step(&snap, &TO_FORCE_60);
    assert_eq!(run.intents(), [force_60(Level::Game)]);
    assert_eq!(run.screen(), "quick-menu");
}

#[test]
fn leaving_after_a_change_asks_where_to_keep_it() {
    for (presses, level) in [
        (&[Accept][..], Level::Game),
        (&[Right, Accept][..], Level::System),
        (&[Right, Right, Accept][..], Level::Global),
    ] {
        let (mut run, snap) = quick_menu();
        run.step(&snap, &TO_FORCE_60);
        run.step(&snap, &[Back]);
        assert_eq!(run.screen(), "quick-menu/keep");
        assert_eq!(run.focused(), Some("This game"));
        assert!(run.intents().is_empty());
        run.step(&snap, presses);
        let mut expected = Vec::new();
        if level != Level::Game {
            expected.push(force_60(level));
            expected.push(Intent::ResetValue {
                key: REFRESH,
                level: Level::Game,
            });
        }
        expected.push(Intent::Resume);
        assert_eq!(run.intents(), expected, "{level:?}");
        assert!(!run.screens.shows_ui());
        run.step(&snap, &[Menu, Back]);
        assert_eq!(run.intents(), [Intent::Resume], "nothing is left to keep");
    }
}

#[test]
fn back_in_the_prompt_returns_to_the_quick_menu() {
    let (mut run, snap) = quick_menu();
    run.step(&snap, &TO_FORCE_60);
    run.step(&snap, &[Back, Back]);
    assert!(run.intents().is_empty());
    assert_eq!(run.screen(), "quick-menu");
    run.step(&snap, &[Up, Down, Down, Down, Down, Accept]);
    assert_eq!(run.screen(), "quick-menu/keep");
    run.step(&snap, &[Accept]);
    assert_eq!(run.intents(), [Intent::QuitGame]);
}

#[test]
fn a_reset_leaves_nothing_to_keep() {
    let (mut run, snap) = quick_menu();
    run.step(&snap, &TO_FORCE_60);
    let mut reset = snap.clone();
    if let Some(row) = reset.settings.rows.iter_mut().find(|r| r.key == REFRESH) {
        row.source = Level::Game;
    }
    run.step(&reset, &[Menu]);
    assert_eq!(
        run.intents(),
        [Intent::ResetValue {
            key: REFRESH,
            level: Level::Game
        }]
    );
    run.step(&snap, &[Back]);
    assert_eq!(run.intents(), [Intent::Resume]);
}

#[test]
fn the_display_panel_opens_from_the_quick_menu_and_returns_to_it() {
    let (mut run, mut snap) = quick_menu();
    run.step(&snap, &[Down, Down, Down, Accept]);
    assert_eq!(run.screen(), "display");
    assert_eq!(run.intents(), [Intent::LoadSettings { page: DISPLAY_PAGE }]);
    snap.settings = with_display().settings;
    run.step(&snap, &[Back]);
    assert_eq!(run.screen(), "quick-menu");
    assert_eq!(
        run.intents(),
        [Intent::LoadSettings {
            page: QUICK_MENU_PAGE
        }]
    );
    assert!(run.screens.shows_ui());
}

#[test]
fn the_grid_pattern_opens_from_the_display_panel_and_the_quick_menu() {
    let mut run = Run::new();
    let snap = with_display();
    run.step(&snap, &TO_DISPLAY);
    run.step(&snap, &[Up, Accept]);
    assert_eq!(run.screen(), "options", "the list stops at its first row");
    run.step(&snap, &[Back, Down, Down, Down, Down, Down, Accept]);
    assert_eq!(run.screen(), "pattern/grid");
    assert!(run.intents().is_empty());
    run.step(&snap, &[Up, Left, Menu]);
    assert_eq!(run.screen(), "pattern/grid");
    run.step(&snap, &[Back]);
    assert_eq!(run.screen(), "display");

    let (mut run, mut snap) = quick_menu();
    if let Some(display) = snap.settings.rows.iter_mut().find(|r| r.label == "Display") {
        display.value = crate::snapshot::SettingValue::Action(super::GRID_PATTERN);
    }
    run.step(&snap, &[Down, Down, Down, Accept]);
    assert_eq!(run.screen(), "pattern/grid");
    assert!(run.screens.shows_ui());
    run.step(&snap, &[Accept]);
    assert_eq!(run.screen(), "quick-menu");
}

/// The memory that one frame holds: scene, tree and intents.
fn capacities(frame: &UiFrame) -> ((usize, usize), (usize, usize), usize) {
    (
        frame.scene.capacity(),
        frame.tree.capacity(),
        frame.intents.capacity(),
    )
}

#[test]
fn a_static_screen_allocates_nothing_after_its_first_frame() {
    let quick = in_game();
    let mut confirm = with_display();
    confirm.confirm = Some(PendingConfirm::new(
        ConfirmId(1),
        "640x480 at 59.94 Hz",
        5 * SECOND_NS,
    ));
    let mut noticed = with_systems();
    noticed.notices = vec![notice(1, "The scan found 12 new games.")];
    let (_, games) = snes_list();
    let video = in_game_with(long_status());
    let cases: [(&Snapshot, &[UiAction], &str); 12] = [
        (&with_systems(), &[], "home"),
        (&Snapshot::default(), &[], "home"),
        (&games, &[Down, Accept], "games"),
        (&with_display(), &TO_DISPLAY, "display"),
        (&confirm, &TO_DISPLAY, "confirm"),
        (&noticed, &[], "notice"),
        (&quick, &[Menu], "quick-menu"),
        (&quick, &[Menu, Down, Accept], "options"),
        (&video, &[Menu], "quick-menu"),
        (&video, &[Menu, Down, Accept], "options"),
        (&video, &[Menu, Down, Down, Down, Accept], "display"),
        (
            &with_display(),
            &[
                Down, Down, Down, Down, Down, Accept, Down, Down, Down, Down, Accept,
            ],
            "pattern/grid",
        ),
    ];
    for (snap, path, screen) in cases {
        let mut run = Run::new();
        run.step(snap, &[]);
        run.step(snap, path);
        run.step(snap, &[]);
        assert_eq!(run.screen(), screen);
        let before = capacities(&run.frame);
        let stack = run.screens.stack_capacity();
        for _ in 0..10 {
            run.step(snap, &[]);
        }
        assert_eq!(capacities(&run.frame), before, "{screen}");
        assert_eq!(run.screens.stack_capacity(), stack, "{screen}");
    }
}

mod fuzz {
    use super::*;
    use crate::screens::{GRID_PATTERN, OPEN_DISPLAY, QUIT_GAME, RESUME};
    use crate::snapshot::{
        ActionId, ChoiceOption, DisplayInfo, GameEntry, MenuItem, PageId, ProfileEntry, ProfileId,
        Scopes, SettingKey, SettingValue, SystemEntry, SystemId, VideoStatus,
    };
    use crate::theme::SAFE_AREA;
    use crate::{CpuCanvas, WidgetKind};

    const SEED: u64 = 0x5EED_0020_5C2E_E115;
    const RUNS: usize = 40;
    const FRAMES: usize = 240;
    const ACTIONS: [UiAction; 7] = [Up, Down, Left, Right, Accept, Back, Menu];
    const LONG: &str =
        "A name that is far too long for any row of the canvas, with more words after it";

    struct Rng(u64);

    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }
        fn below(&mut self, n: u64) -> u64 {
            self.next() % n.max(1)
        }
        fn chance(&mut self, percent: u64) -> bool {
            self.below(100) < percent
        }
        fn count(&mut self) -> usize {
            let edges = [
                0,
                1,
                2,
                11,
                12,
                13,
                127,
                128,
                129,
                10_000,
                usize::MAX - 1,
                usize::MAX,
            ];
            match self.below(3) {
                0 => edges[self.below(edges.len() as u64) as usize],
                _ => self.below(300) as usize,
            }
        }
        fn text(&mut self) -> &'static str {
            [
                "",
                "x",
                "Mega Drive",
                LONG,
                "\u{e9}\u{e8}\u{3a9}\u{2603} \n line",
            ][self.below(5) as usize]
        }
        fn source(&mut self) -> GameSource {
            match self.below(4) {
                0 => GameSource::Favourites,
                1 => GameSource::Recents,
                _ => GameSource::System(SystemId(self.below(4) as u32)),
            }
        }
        fn page(&mut self) -> PageId {
            [DISPLAY_PAGE, QUICK_MENU_PAGE, PageId(3)][self.below(3) as usize]
        }
        fn action_id(&mut self) -> ActionId {
            [RESUME, QUIT_GAME, OPEN_DISPLAY, GRID_PATTERN, ActionId(7)][self.below(5) as usize]
        }
        fn value(&mut self) -> SettingValue {
            match self.below(4) {
                0 => SettingValue::Toggle(self.chance(50)),
                1 => SettingValue::Number {
                    value: self.next() as i64,
                    min: -(self.below(100) as i64),
                    max: self.next() as i64,
                    step: self.next() as i64,
                    unit: self.text().into(),
                },
                2 => SettingValue::Choice {
                    index: self.count(),
                    options: (0..self.below(7))
                        .map(|_| {
                            let option = ChoiceOption::new(self.text()).with_detail(self.text());
                            if self.chance(40) {
                                option.unavailable(self.text())
                            } else {
                                option
                            }
                        })
                        .collect(),
                },
                _ => SettingValue::Action(self.action_id()),
            }
        }
        fn levels(&mut self) -> Scopes {
            Scopes {
                global: self.chance(70),
                system: self.chance(50),
                game: self.chance(50),
            }
        }
        fn snapshot(&mut self) -> Snapshot {
            let systems = (0..self.below(5))
                .map(|i| SystemEntry::new(SystemId(i as u32), self.text(), self.next() as u32))
                .collect();
            let mut snap = Snapshot {
                systems,
                ..Snapshot::default()
            };
            if self.chance(80) {
                let source = self.source();
                let first = self.count();
                let total = self.count();
                let rows = (0..self.below(150))
                    .map(|i| {
                        let id = GameId(first.wrapping_add(i as usize) as u64);
                        GameEntry::new(id, SystemId(0), self.text())
                    })
                    .collect();
                snap.games = GameWindow::new(source, first, total, rows);
            }
            if self.chance(80) {
                let rows = (0..self.below(20))
                    .map(|k| {
                        let mut item =
                            MenuItem::new(SettingKey(k as u32 % 5), self.text(), self.value());
                        item.scopes = self.levels();
                        item.source = [Level::Default, Level::Global, Level::System, Level::Game]
                            [self.below(4) as usize];
                        item
                    })
                    .collect();
                snap.settings = SettingsPage::new(self.page(), self.text(), rows);
            }
            snap.profiles = (0..self.below(3))
                .map(|i| ProfileEntry::new(ProfileId(i as u32), self.text()))
                .collect();
            snap.active_profile = self.chance(50).then(|| ProfileId(self.below(4) as u32));
            if self.chance(70) {
                snap.display = Some(DisplayInfo::new(self.text(), self.text()));
            }
            if self.chance(25) {
                let remaining = [0, 1, SECOND_NS, 10 * SECOND_NS, u64::MAX][self.below(5) as usize];
                let id = ConfirmId(self.below(3) as u32);
                snap.confirm = Some(PendingConfirm::new(id, self.text(), remaining));
            }
            if self.chance(30) {
                snap.session = Some(SessionInfo::new(GameId(1), self.text()));
            }
            if self.chance(40) {
                let status = VideoStatus::new(self.text(), self.text(), self.text());
                snap.video = Some(status.with_alert(self.chance(50)));
            }
            snap.notices = (0..self.below(3))
                .map(|i| {
                    Notice::new(
                        NoticeId(self.below(3) as u32 + i as u32),
                        self.text(),
                        NoticeKind::Info,
                    )
                })
                .collect();
            snap
        }
    }

    fn requests(intents: &[Intent]) -> usize {
        intents
            .iter()
            .filter(|i| matches!(i, Intent::LoadGames { .. } | Intent::LoadSettings { .. }))
            .count()
    }

    #[test]
    fn random_snapshots_and_actions_never_break_a_rule() {
        let mut rng = Rng(SEED);
        let mut canvas = CpuCanvas::new();
        let mut keeps = 0;
        for _ in 0..RUNS {
            let mut run = Run::new();
            let mut snap = rng.snapshot();
            let mut keep = KeepModel::default();
            let mut idle_frames = 0;
            let mut next_id = 100;
            for frame in 0..FRAMES {
                if idle_frames == 0 && rng.chance(15) {
                    snap = rng.snapshot();
                }
                if idle_frames == 0 && rng.chance(4) {
                    idle_frames = QUIET_FRAMES_BEFORE_MOVE + 10;
                    if rng.chance(50) {
                        next_id += 1;
                        let pending = PendingConfirm::new(ConfirmId(next_id), "2x", SECOND_NS);
                        snap.confirm = Some(pending);
                    }
                }
                let actions: Vec<_> = if idle_frames > 1 {
                    idle_frames -= 1;
                    Vec::new()
                } else if idle_frames == 1 {
                    idle_frames = 0;
                    if rng.chance(50) {
                        vec![Right, Accept]
                    } else {
                        Vec::new()
                    }
                } else {
                    (0..rng.below(5))
                        .map(|_| ACTIONS[rng.below(ACTIONS.len() as u64) as usize])
                        .collect()
                };
                let may_keep = keep.before_frame(&snap, &actions);
                run.step(&snap, &actions);
                let out = &run.frame;
                assert!(
                    requests(&out.intents) <= 1,
                    "frame {frame}: {:?}",
                    out.intents
                );
                if out.intents.contains(&Intent::KeepMode) {
                    assert!(
                        may_keep,
                        "frame {frame}: keep without the dialog, a quiet time, right, then accept: {actions:?}"
                    );
                    keeps += 1;
                }
                keep.after_frame(&actions);
                for row in out.tree.rows() {
                    assert!(SAFE_AREA.contains(&row.rect), "frame {frame}: {row:?}");
                }
                if snap.confirm.is_some() {
                    assert_eq!(run.screen(), "confirm");
                    assert!(run.screens.shows_ui());
                }
                assert!(run.screens.depth() <= crate::screens::MAX_DEPTH);
                if frame % 8 == 0 {
                    out.render(&mut canvas);
                }
                let buttons = out
                    .tree
                    .rows()
                    .filter(|r| r.widget == WidgetKind::Button)
                    .count();
                assert!(buttons <= 3);
            }
        }
        assert!(keeps > 0, "the fuzz never reached a deliberate keep");
    }

    /// What the player did during the confirmation on screen, as the rule
    /// of the dialog needs it: the quiet frames after it appeared, and a
    /// right after them.
    #[derive(Default)]
    struct KeepModel {
        id: Option<ConfirmId>,
        quiet_frames: u32,
        moved: bool,
    }

    impl KeepModel {
        /// True when the frame may keep: the confirmation was on screen
        /// for the quiet frames, and a right came after them, before an
        /// accept of this frame.
        fn before_frame(&mut self, snap: &Snapshot, actions: &[UiAction]) -> bool {
            let id = snap.confirm.as_ref().map(|c| c.id);
            if id != self.id {
                *self = Self {
                    id,
                    ..Self::default()
                };
            }
            if id.is_none() || self.quiet_frames < QUIET_FRAMES_BEFORE_MOVE {
                return false;
            }
            let moved_before = self.moved;
            self.moved |= actions.contains(&Right);
            let Some(accept) = actions.iter().rposition(|a| *a == Accept) else {
                return false;
            };
            moved_before || actions[..accept].contains(&Right)
        }

        fn after_frame(&mut self, actions: &[UiAction]) {
            if self.quiet_frames >= QUIET_FRAMES_BEFORE_MOVE {
                return;
            }
            let direction = actions
                .iter()
                .any(|a| matches!(a, Up | Down | Left | Right));
            self.quiet_frames = if direction { 0 } else { self.quiet_frames + 1 };
        }
    }
}

fn hints(run: &Run) -> Vec<&str> {
    run.labels()
        .into_iter()
        .filter(|l| l.contains(": "))
        .collect()
}

#[test]
fn a_dialog_shows_its_own_hints() {
    let (mut run, snap) = confirming(10 * SECOND_NS);
    assert_eq!(hints(&run), ["Accept: revert", "Right, then Accept: keep"]);
    run.step(&snap, &[Accept]);
    assert!(hints(&run).is_empty());

    let (mut run, mut snap) = snes_list();
    snap.notices = vec![notice(1, "The scan found 12 new games.")];
    run.step(&snap, &[]);
    assert_eq!(hints(&run), ["Accept: OK"]);

    let (mut run, snap) = quick_menu();
    run.step(&snap, &TO_FORCE_60);
    run.step(&snap, &[Back]);
    assert_eq!(hints(&run), ["Accept: choose", "Back: cancel"]);
}

#[test]
fn a_change_whose_row_is_gone_stays_for_the_game() {
    let (mut run, mut snap) = quick_menu();
    run.step(&snap, &TO_FORCE_60);
    snap.settings.rows.retain(|r| r.key != REFRESH);
    run.step(&snap, &[Back]);
    assert_eq!(run.screen(), "quick-menu/keep");
    run.step(&snap, &[Right, Accept]);
    assert_eq!(run.intents(), [Intent::Resume]);
}

#[test]
fn back_after_a_quit_request_gives_the_quick_menu_back() {
    let (mut run, snap) = quick_menu();
    run.step(&snap, &[Down, Down, Down, Down, Accept]);
    assert_eq!(run.intents(), [Intent::QuitGame]);
    run.step(&snap, &[Up, Accept]);
    assert!(
        run.intents().is_empty(),
        "the menu waits for the session to end"
    );
    run.step(&snap, &[Back]);
    assert!(run.intents().is_empty());
    assert_eq!(run.screen(), "quick-menu");
    run.step(&snap, &[Up, Up, Up, Up, Accept]);
    assert_eq!(run.intents(), [Intent::Resume]);
}

#[test]
fn many_dismissed_notices_never_come_back() {
    let (mut run, mut snap) = snes_list();
    let texts: Vec<String> = (0..20).map(|i| format!("Notice {i}")).collect();
    snap.notices = texts
        .iter()
        .enumerate()
        .map(|(i, t)| notice(i as u32, t))
        .collect();
    run.step(&snap, &[]);
    for (i, text) in texts.iter().enumerate() {
        assert_eq!(message(&run), text);
        run.step(&snap, &[Accept]);
        assert_eq!(run.intents(), [Intent::DismissNotice(NoticeId(i as u32))]);
    }
    assert_eq!(run.screen(), "games");
    snap.notices.drain(..5);
    snap.notices.push(notice(99, "New"));
    run.step(&snap, &[]);
    assert_eq!(message(&run), "New");
}

/// Scrolls the long list to its end and back to its top, loading each
/// window that the screen asks for.
fn scroll_round_trip(run: &mut Run, snap: &mut Snapshot) {
    for direction in [Right, Left] {
        for _ in 0..LONG_LIST / 8 {
            run.step(snap, &[direction]);
            if let Some(&(first, count)) = run.loads().first() {
                snap.games = window(SNES_LIST, first, LONG_LIST, count);
            }
        }
    }
}

#[test]
fn scrolling_a_long_list_allocates_nothing_after_a_warm_up() {
    let (mut run, mut snap) = snes_list();
    scroll_round_trip(&mut run, &mut snap);
    let before = (capacities(&run.frame), run.screens.stack_capacity());
    scroll_round_trip(&mut run, &mut snap);
    assert_eq!(run.focused(), Some(super::scenes::title(0).as_str()));
    assert_eq!(
        (capacities(&run.frame), run.screens.stack_capacity()),
        before
    );
}

/// Opens and closes a confirmation, a notice and the quick menu over a
/// running game.
fn overlay_round_trip(run: &mut Run, base: &Snapshot, round: u32) {
    let mut snap = base.clone();
    snap.confirm = Some(PendingConfirm::new(ConfirmId(round), "2x", 3 * SECOND_NS));
    run.step(&snap, &[]);
    run.step(&snap, &[Back]);
    snap.confirm = None;
    snap.notices = vec![notice(round, "Forced to 60 Hz")];
    run.step(&snap, &[Menu]);
    run.step(&snap, &[Accept]);
    snap.notices.clear();
    run.step(&snap, &[Down, Back]);
    run.step(&snap, &[]);
}

#[test]
fn overlays_that_open_and_close_allocate_nothing_after_a_warm_up() {
    for snap in [in_game(), in_game_with(long_status())] {
        overlays_allocate_nothing_after_a_warm_up(&snap);
    }
}

fn overlays_allocate_nothing_after_a_warm_up(snap: &Snapshot) {
    let mut run = Run::new();
    let snap = snap.clone();
    run.step(&snap, &[]);
    overlay_round_trip(&mut run, &snap, 1);
    let before = (capacities(&run.frame), run.screens.stack_capacity());
    for round in 2..6 {
        overlay_round_trip(&mut run, &snap, round);
        assert_eq!(run.screen(), "game");
        assert_eq!(
            (capacities(&run.frame), run.screens.stack_capacity()),
            before,
            "round {round}"
        );
    }
}

fn video_rows(run: &Run) -> Vec<(&str, Option<&str>)> {
    run.frame
        .tree
        .rows()
        .filter(|r| r.label.starts_with("video/") || r.label == "Video")
        .map(|r| (r.label, r.value))
        .collect()
}

#[test]
fn the_quick_menu_shows_the_video_block_only_with_a_status() {
    let (run, _) = quick_menu();
    assert!(video_rows(&run).is_empty());

    let mut run = Run::new();
    let snap = in_game_with(exact_status());
    run.step(&snap, &[]);
    run.step(&snap, &[Menu]);
    assert_eq!(run.screen(), "quick-menu");
    assert_eq!(
        video_rows(&run),
        [
            ("Video", None),
            (VIDEO_GAME_ROW, Some("256x224             60.099 Hz")),
            (VIDEO_MODE_ROW, Some("768x448p  30.05 kHz 60.099 Hz  2x")),
            (VIDEO_RESULT_ROW, Some("exact")),
        ]
    );
    assert!(run.frame.tree.rows().all(|r| !r.clipped));
}

#[test]
fn the_fixture_columns_fit_the_value_column() {
    for status in [exact_status(), filtered_status(), long_status()] {
        let mut run = Run::new();
        let snap = in_game_with(status.clone());
        run.step(&snap, &[]);
        run.step(&snap, &[Menu]);
        let clipped: Vec<_> = run
            .frame
            .tree
            .rows()
            .filter(|r| r.label == VIDEO_GAME_ROW || r.label == VIDEO_MODE_ROW)
            .map(|r| r.clipped)
            .collect();
        assert_eq!(clipped, [false, false], "{status:?}");
    }
}

#[test]
fn the_video_block_takes_no_focus_and_changes_no_navigation() {
    let paths: [&[UiAction]; 7] = [
        &[],
        &[Up, Up],
        &[Down, Down, Down, Down, Down, Down],
        &[Down, Up, Up, Accept],
        &TO_FORCE_60,
        &[Down, Down, Down, Down, Accept, Up, Accept, Back],
        &[Left, Right, Menu, Back],
    ];
    for path in paths {
        let (mut plain, snap) = quick_menu();
        let mut run = Run::new();
        let with = in_game_with(long_status());
        run.step(&with, &[]);
        run.step(&with, &[Menu]);
        assert_eq!(run.focused(), Some("Resume"));
        for &action in path.iter().chain(&[Back, Accept, Back]) {
            plain.step(&snap, &[action]);
            run.step(&with, &[action]);
            assert_eq!(run.screen(), plain.screen(), "{path:?} {action:?}");
            assert_eq!(run.focused(), plain.focused(), "{path:?} {action:?}");
            assert_eq!(run.intents(), plain.intents(), "{path:?} {action:?}");
            assert_eq!(run.screens.shows_ui(), plain.screens.shows_ui());
        }
    }
}

#[test]
fn a_status_that_comes_or_goes_keeps_the_cursor() {
    let (mut run, mut snap) = quick_menu();
    run.step(&snap, &[Down, Down]);
    assert_eq!(run.focused(), Some("Refresh"));
    snap.video = Some(filtered_status());
    run.step(&snap, &[]);
    assert_eq!(run.focused(), Some("Refresh"));
    assert_eq!(video_rows(&run).len(), 4);
    run.step(&snap, &[Down]);
    snap.video = None;
    run.step(&snap, &[]);
    assert_eq!(run.focused(), Some("Display"));
    assert!(video_rows(&run).is_empty());
}

/// The text runs of the frame with their left edge, top edge and width.
fn text_runs(run: &Run) -> Vec<(i32, i32, u32, &str)> {
    let scene = &run.frame.scene;
    scene
        .commands()
        .iter()
        .filter_map(|c| match *c {
            crate::DrawCommand::Text { x, y, span, .. } => {
                let text = scene.text(span);
                Some((x, y, crate::font::text_width(text), text))
            }
            _ => None,
        })
        .collect()
}

#[test]
fn long_video_text_is_cut_and_stays_in_its_line() {
    const LONG: &str = "1920x240p 15.73 kHz 59.940 Hz  native  and much more text after it";
    let mut status = long_status();
    status.game = LONG.into();
    status.mode = LONG.into();
    status.result = format!("{LONG} {LONG}");
    let mut run = Run::new();
    let snap = in_game_with(status.clone());
    run.step(&snap, &[]);
    run.step(&snap, &[Menu]);
    let rows: Vec<_> = run
        .frame
        .tree
        .rows()
        .filter(|r| r.label.starts_with("video/"))
        .collect();
    assert_eq!(rows.len(), 3);
    for row in &rows {
        assert!(row.clipped, "{row:?}");
        let inside: Vec<_> = text_runs(&run)
            .into_iter()
            .filter(|&(x, y, _, _)| {
                x >= row.rect.x
                    && i64::from(y) >= i64::from(row.rect.y)
                    && i64::from(y) < row.rect.bottom()
            })
            .collect();
        assert!(!inside.is_empty(), "{row:?}");
        for (x, _, w, text) in inside {
            assert!(
                i64::from(x) + i64::from(w) <= row.rect.right(),
                "{text:?} leaves {row:?}"
            );
        }
    }
    assert_eq!(rows[2].value, Some(status.result.as_str()));
    let ellipses = text_runs(&run)
        .iter()
        .filter(|&&(_, _, _, t)| t == crate::text::ELLIPSIS)
        .count();
    assert_eq!(ellipses, 3, "one cut for each field");
    let menu_top = run
        .frame
        .tree
        .rows()
        .find(|r| r.label == "Resume")
        .map(|r| r.rect.y);
    assert!(menu_top.is_some_and(|top| i64::from(top) > rows[2].rect.bottom()));
}

#[test]
fn an_alert_result_has_the_warning_colour() {
    let theme = Theme::default();
    assert_ne!(theme.colors.warning, theme.colors.focus);
    for (status, colour) in [
        (exact_status(), theme.colors.text),
        (filtered_status(), theme.colors.warning),
    ] {
        let mut run = Run::new();
        let snap = in_game_with(status.clone());
        run.step(&snap, &[]);
        run.step(&snap, &[Menu]);
        let scene = &run.frame.scene;
        let drawn = scene.commands().iter().find_map(|c| match *c {
            crate::DrawCommand::Text { color, span, .. } if scene.text(span) == status.result => {
                Some(color)
            }
            _ => None,
        });
        assert_eq!(drawn, Some(colour), "{}", status.result);
    }
}

/// A run on the display panel, opened from the quick menu of a game with
/// `status`.
fn display_in_game(status: Option<VideoStatus>) -> (Run, Snapshot) {
    let mut run = Run::new();
    let mut snap = in_game();
    snap.video = status;
    run.step(&snap, &[]);
    run.step(&snap, &[Menu, Down, Down, Down, Accept]);
    snap.settings = with_display().settings;
    run.step(&snap, &[]);
    assert_eq!(run.screen(), "display");
    (run, snap)
}

#[test]
fn the_display_panel_shows_the_video_block_while_a_game_runs() {
    let (run, _) = display_in_game(Some(exact_status()));
    assert_eq!(field(&run, "Output"), Some("VGA-1"));
    assert_eq!(field(&run, "Monitor"), Some("pc_31_120"));
    assert_eq!(field(&run, "Mode"), None, "the block shows the mode");
    assert_eq!(
        field(&run, VIDEO_MODE_ROW),
        Some("768x448p  30.05 kHz 60.099 Hz  2x")
    );
    assert_eq!(run.focused(), Some("Output"));
    let block = run
        .frame
        .tree
        .rows()
        .find(|r| r.label == VIDEO_RESULT_ROW)
        .map(|r| r.rect.bottom());
    let first_row = run
        .frame
        .tree
        .rows()
        .find(|r| r.label == "Output" && r.widget == crate::WidgetKind::SettingsList)
        .map(|r| i64::from(r.rect.y));
    assert!(block < first_row, "{block:?} {first_row:?}");

    let (run, _) = display_in_game(None);
    assert!(video_rows(&run).is_empty());
    assert_eq!(field(&run, "Mode"), Some("640x480 59.94 Hz"));

    let mut run = Run::new();
    let mut snap = with_display();
    snap.video = Some(exact_status());
    run.step(&snap, &TO_DISPLAY);
    assert!(video_rows(&run).is_empty(), "no game runs");
}

#[test]
fn the_video_block_changes_no_navigation_of_the_display_panel() {
    let paths: [&[UiAction]; 4] = [
        &[Up, Down, Down, Down, Down, Down, Down],
        &[Down, Down, Accept, Down, Accept],
        &[Down, Down, Down, Down, Accept, Back],
        &[Right, Left, Back, Back],
    ];
    for path in paths {
        let (mut plain, snap) = display_in_game(None);
        let (mut run, with) = display_in_game(Some(long_status()));
        for &action in path {
            plain.step(&snap, &[action]);
            run.step(&with, &[action]);
            assert_eq!(run.screen(), plain.screen(), "{path:?} {action:?}");
            assert_eq!(run.focused(), plain.focused(), "{path:?} {action:?}");
            assert_eq!(run.intents(), plain.intents(), "{path:?} {action:?}");
        }
    }
}

/// Checks that the rows of the settings list sit inside the content area
/// under every box above them, and that the focused row is one of them.
fn assert_rows_in_their_box(run: &Run, theme: &Theme, what: &str) {
    let content = super::chrome::Layout::new(theme).content;
    let above = run
        .frame
        .tree
        .rows()
        .filter(|r| r.widget == crate::WidgetKind::Label && content.contains(&r.rect))
        .map(|r| r.rect.bottom())
        .max()
        .unwrap_or(i64::from(content.y));
    let rows: Vec<_> = run
        .frame
        .tree
        .rows()
        .filter(|r| r.widget == crate::WidgetKind::SettingsList)
        .collect();
    assert!(!rows.is_empty(), "{what}");
    for row in &rows {
        assert!(content.contains(&row.rect), "{what}: {row:?} {content:?}");
        assert!(
            i64::from(row.rect.y) > above,
            "{what}: {row:?} under {above}"
        );
    }
    assert!(rows.iter().any(|r| r.focused), "{what}: no focused row");
}

#[test]
fn the_largest_skin_keeps_the_rows_in_their_box() {
    let theme = Theme::new(
        Theme::default().colors,
        crate::theme::ROW_HEIGHT_MAX_PX,
        crate::theme::PADDING_MAX_PX,
    )
    .expect("the limits are valid");
    for status in [None, Some(long_status())] {
        let what = if status.is_some() { "status" } else { "none" };
        let mut snap = in_game();
        snap.video = status;
        let mut run = Run {
            screens: Screens::new(theme.clone()),
            frame: UiFrame::new(),
        };
        run.step(&snap, &[]);
        run.step(&snap, &[Menu]);
        let rows = |run: &Run| {
            run.frame
                .tree
                .rows()
                .filter(|r| r.widget == crate::WidgetKind::SettingsList)
                .count()
        };
        let (block, visible) = if snap.video.is_some() { (4, 3) } else { (0, 5) };
        assert_eq!(video_rows(&run).len(), block, "{what}");
        assert_eq!(rows(&run), visible, "{what}");
        for step in 0..6 {
            assert_rows_in_their_box(&run, &theme, &format!("quick menu {what} {step}"));
            run.step(&snap, &[Down]);
        }
        run.step(&snap, &[Up, Accept]);
        snap.settings = with_display().settings;
        run.step(&snap, &[]);
        assert_eq!(run.screen(), "display");
        assert!(video_rows(&run).is_empty(), "no room for the block: {what}");
        assert_eq!(field(&run, "Mode"), Some("640x480 59.94 Hz"));
        for step in 0..6 {
            assert_rows_in_their_box(&run, &theme, &format!("display {what} {step}"));
            run.step(&snap, &[Down]);
        }
    }
}
