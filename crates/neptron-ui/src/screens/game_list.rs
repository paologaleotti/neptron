//! The game list of one source, read through a window of rows that the app
//! loads on request.
//!
//! Request rule: the screen needs the visible rows and [`PREFETCH_ROWS`]
//! rows on each side of them. While the snapshot window covers that range,
//! the screen asks for nothing. When it does not, the screen asks for
//! [`WINDOW_ROWS`] rows centred on the visible rows, one request per frame,
//! until the snapshot covers the range. The margin is larger than a page,
//! so a step or a page jump shows rows that the old window still holds,
//! and the new window arrives while the player reads them.

use core::fmt::Write;

use crate::frame::UiFrame;
use crate::geom::Rect;
use crate::input::UiAction;
use crate::intent::Intent;
use crate::metrics::Metrics;
use crate::snapshot::{GameEntry, GameId, GameSource, Snapshot};
use crate::text::StackText;
use crate::theme::Theme;
use crate::tree::RowKind;
use crate::widgets::{List, ListEvent, ListItem};

use super::chrome::{self, Layout};
use super::router::{Nav, Target};

/// The rows that one request asks for.
pub const WINDOW_ROWS: usize = 128;
/// The rows before and after the visible rows that the snapshot must hold
/// before the screen stops asking.
pub const PREFETCH_ROWS: usize = 32;

/// Lines of the panel under the list that shows the full title of the
/// focused game.
const TITLE_LINES: u32 = 2;
/// Space between the list and the title panel, in pixels.
const GAP_PX: u32 = 4;
/// The label of a row that the snapshot does not hold yet.
const PENDING_ROW: &str = "...";

/// The detail column of a row: `*` favourite, `V` vertical screen, `!` not
/// working, `?` not a set of the arcade index. The index is the sum of
/// 1, 2, 4 and 8 for these marks, so the column needs no allocation.
const BADGES: [&str; 16] = [
    "", "*", "V", "*V", "!", "*!", "V!", "*V!", "?", "*?", "V?", "*V?", "!?", "*!?", "V!?", "*V!?",
];

/// The longest line of facts under the title, in bytes.
const FACTS_BYTES: usize = 96;

pub(super) struct GameList {
    source: GameSource,
    list: List,
    /// The game under the cursor, once the snapshot held its row. When the
    /// app gives a changed list (a scan ended), the cursor moves to this
    /// game while the window holds it.
    focused: Option<GameId>,
    list_box: Rect,
    panel: Rect,
    title: Rect,
}

impl GameList {
    pub(super) fn new(source: GameSource, layout: &Layout, theme: &Theme) -> Self {
        let content = layout.content;
        let (panel, title) = chrome::rows_box_bottom(content, theme, TITLE_LINES);
        let above = Rect::new(
            content.x,
            content.y,
            content.w,
            content.h.saturating_sub(panel.h + GAP_PX),
        );
        let (list_box, list) = chrome::rows_box(above, theme, chrome::box_rows(above, theme));
        Self {
            source,
            list: List::new(list, theme.row_height_px()),
            focused: None,
            list_box,
            panel,
            title,
        }
    }

    /// Moves the cursor to `index` of a list of `len` rows.
    pub(super) fn select(&mut self, index: usize, len: usize) {
        self.list.select(index, len);
        self.focused = None;
    }

    /// Clamps the cursor to the list of the snapshot, and follows the
    /// focused game. A window of a different source does not count, so the
    /// cursor survives while the app loads the list again.
    ///
    /// Game ids stay with their game when the list changes, so another id
    /// under the cursor means the list moved. The cursor then goes to the
    /// focused game when the window holds it; a game that is gone, or out
    /// of the window, leaves the cursor at its index.
    pub(super) fn sync(&mut self, snap: &Snapshot) {
        let Some(total) = self.total(snap) else {
            return;
        };
        self.list.select(self.list.cursor(), total);
        let Some(here) = snap.games.get(self.list.cursor()).map(|g| g.id) else {
            return;
        };
        let moved_to = self.focused.filter(|f| *f != here).and_then(|f| {
            let offset = snap.games.rows.iter().position(|g| g.id == f)?;
            Some(snap.games.first + offset)
        });
        match moved_to {
            Some(index) => self.list.select(index, total),
            None => self.focused = Some(here),
        }
    }

    pub(super) fn handle(
        &mut self,
        action: UiAction,
        snap: &Snapshot,
        out: &mut Vec<Intent>,
    ) -> Nav {
        if action == UiAction::Back {
            return Nav::Back;
        }
        let Some(total) = self.total(snap) else {
            return Nav::Stay;
        };
        let focused = |i| snap.games.get(i).map(|g| g.id);
        let event = self.list.handle(action, total);
        // The player moved the cursor: the game under it is the new focus,
        // or none until its row loads.
        self.focused = focused(self.list.cursor());
        match event {
            ListEvent::Activated(i) => match snap.games.get(i) {
                Some(entry) if self.opens_versions(entry) => {
                    return Nav::Open(Target::Games(GameSource::Versions(entry.id)));
                }
                Some(entry) => out.push(Intent::Launch { game: entry.id }),
                None => {}
            },
            ListEvent::Menu => {
                if let Some(game) = focused(self.list.cursor()) {
                    out.push(Intent::ToggleFavourite { game });
                }
            }
            _ => {}
        }
        Nav::Stay
    }

    /// The request for the rows the screen needs, or `None` when the
    /// snapshot holds them.
    pub(super) fn request(&self, snap: &Snapshot) -> Option<Intent> {
        let rows = self.list.visible_rows();
        let Some(total) = self.total(snap) else {
            let first = self.list.cursor().saturating_sub(WINDOW_ROWS / 2);
            return Some(self.load(first));
        };
        let top = self.list.top();
        let need_first = top.saturating_sub(PREFETCH_ROWS);
        let need_end = top
            .saturating_add(rows)
            .saturating_add(PREFETCH_ROWS)
            .min(total);
        if snap
            .games
            .covers(self.source, need_first, need_end.saturating_sub(need_first))
        {
            return None;
        }
        let centre = top.saturating_add(rows / 2);
        let first = centre
            .saturating_sub(WINDOW_ROWS / 2)
            .min(total.saturating_sub(WINDOW_ROWS));
        Some(self.load(first))
    }

    pub(super) fn draw(
        &self,
        snap: &Snapshot,
        theme: &Theme,
        metrics: &Metrics,
        frame: &mut UiFrame,
    ) {
        let total = self.total(snap);
        let mut position = StackText::<32>::new();
        if let Some(total) = total.filter(|t| *t > 0) {
            // `StackText` never fails; it drops what does not fit.
            let _ = write!(position, "{}/{}", self.list.cursor() + 1, total);
        }
        let back = match self.source {
            GameSource::System(_) => "Back: systems",
            GameSource::Versions(_) => "Back: games",
            GameSource::Favourites | GameSource::Recents => "Back: home",
        };
        let focused = snap.games.get(self.list.cursor());
        let accept = match (total, focused) {
            (Some(1..), Some(entry)) if self.opens_versions(entry) => "Accept: versions",
            (Some(1..), _) => "Accept: play",
            _ => "",
        };
        chrome::frame(
            frame,
            theme,
            metrics,
            self.title(snap),
            position.as_str(),
            accept,
            back,
        );
        let Some(total) = total else {
            return;
        };
        if total == 0 {
            let text = chrome::padded(metrics.content, theme);
            chrome::paragraph(frame, theme, text, theme.colors.text, self.empty_text());
            return;
        }
        chrome::boxed(frame, theme, self.list_box);
        self.list
            .draw(frame, theme, total, |i| match snap.games.get(i) {
                Some(game) => ListItem {
                    accent_detail: false,
                    label: &game.title,
                    detail: Some(badge(game)).filter(|b| !b.is_empty()),
                    enabled: true,
                    kind: RowKind::Action,
                },
                None => ListItem {
                    accent_detail: false,
                    label: PENDING_ROW,
                    detail: None,
                    enabled: false,
                    kind: RowKind::Action,
                },
            });
        self.draw_title_panel(snap, theme, frame);
    }

    fn draw_title_panel(&self, snap: &Snapshot, theme: &Theme, frame: &mut UiFrame) {
        chrome::boxed(frame, theme, self.panel);
        let Some(game) = snap.games.get(self.list.cursor()) else {
            return;
        };
        let facts = self.facts(game);
        let area = chrome::padded(self.title, theme);
        if facts.as_str().is_empty() {
            chrome::paragraph(frame, theme, area, theme.colors.accent, &game.title);
            return;
        }
        // The facts take the last line, so a long title keeps one line.
        let row = theme.row_height_px();
        let title_h = area.h.saturating_sub(row);
        let title = Rect::new(area.x, area.y, area.w, title_h);
        let line = Rect::new(area.x, chrome::offset(area.y, title_h), area.w, row);
        chrome::paragraph(frame, theme, title, theme.colors.accent, &game.title);
        chrome::paragraph(frame, theme, line, theme.colors.text_dim, facts.as_str());
    }

    /// The line of facts of a game under its title: its marks in words,
    /// and its other versions. Empty for a game with none.
    fn facts(&self, game: &GameEntry) -> StackText<FACTS_BYTES> {
        let mut out = StackText::<FACTS_BYTES>::new();
        let marks = game.marks;
        let words = [
            (marks.vertical, "Vertical"),
            (marks.not_working, "Not working"),
            (marks.unknown_set, "Unknown set"),
        ];
        for (_, word) in words.iter().filter(|(on, _)| *on) {
            let sep = if out.as_str().is_empty() { "" } else { ", " };
            // `StackText` never fails; it drops what does not fit.
            let _ = write!(out, "{sep}{word}");
        }
        if self.opens_versions(game) {
            let sep = if out.as_str().is_empty() { "" } else { ", " };
            // `StackText` never fails; it drops what does not fit.
            let _ = write!(out, "{sep}Other versions ({})", game.versions);
        }
        out
    }

    /// True when Accept on `game` opens its other versions: it has some,
    /// and this list is not already its versions.
    fn opens_versions(&self, game: &GameEntry) -> bool {
        game.versions > 0 && !matches!(self.source, GameSource::Versions(_))
    }

    fn total(&self, snap: &Snapshot) -> Option<usize> {
        snap.games.is_for(self.source).then_some(snap.games.total)
    }

    fn load(&self, first: usize) -> Intent {
        Intent::LoadGames {
            source: self.source,
            first,
            count: WINDOW_ROWS,
        }
    }

    fn title<'a>(&self, snap: &'a Snapshot) -> &'a str {
        match self.source {
            GameSource::System(id) => snap
                .systems
                .iter()
                .find(|s| s.id == id)
                .map_or("Games", |s| s.name.as_str()),
            GameSource::Favourites => "Favourites",
            GameSource::Recents => "Recent games",
            GameSource::Versions(_) => "Other versions",
        }
    }

    fn empty_text(&self) -> &'static str {
        match self.source {
            GameSource::System(_) => {
                "This system has no games now.\n\nPut its games into its folder under roms on the share partition."
            }
            GameSource::Favourites => {
                "No favourites yet.\n\nPress Menu on a game in a game list to add it here."
            }
            GameSource::Recents => "No games played yet.",
            GameSource::Versions(_) => "This game has no other versions now.",
        }
    }
}

/// The detail column of a row, from [`BADGES`].
fn badge(game: &GameEntry) -> &'static str {
    let index = usize::from(game.favourite)
        | usize::from(game.marks.vertical) << 1
        | usize::from(game.marks.not_working) << 2
        | usize::from(game.marks.unknown_set) << 3;
    BADGES.get(index).copied().unwrap_or_default()
}
