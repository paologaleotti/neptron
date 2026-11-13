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
use crate::snapshot::{GameId, GameSource, Snapshot};
use crate::text::StackText;
use crate::theme::Theme;
use crate::widgets::{List, ListEvent, ListItem};

use super::chrome::{self, Layout};
use super::router::Nav;

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
            ListEvent::Activated(i) => {
                if let Some(game) = focused(i) {
                    out.push(Intent::Launch { game });
                }
            }
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
        layout: &Layout,
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
            _ => "Back: home",
        };
        let accept = match total {
            Some(1..) => "Accept: play",
            _ => "",
        };
        chrome::frame(
            frame,
            theme,
            self.title(snap),
            position.as_str(),
            accept,
            back,
        );
        let Some(total) = total else {
            return;
        };
        if total == 0 {
            let text = chrome::padded(layout.content, theme);
            chrome::paragraph(frame, theme, text, theme.colors.text, self.empty_text());
            return;
        }
        chrome::boxed(frame, theme, self.list_box);
        self.list
            .draw(frame, theme, total, |i| match snap.games.get(i) {
                Some(game) => ListItem {
                    label: &game.title,
                    detail: game.favourite.then_some("*"),
                    enabled: true,
                },
                None => ListItem {
                    label: PENDING_ROW,
                    detail: None,
                    enabled: false,
                },
            });
        self.draw_title_panel(snap, theme, frame);
    }

    fn draw_title_panel(&self, snap: &Snapshot, theme: &Theme, frame: &mut UiFrame) {
        chrome::boxed(frame, theme, self.panel);
        let Some(game) = snap.games.get(self.list.cursor()) else {
            return;
        };
        chrome::paragraph(
            frame,
            theme,
            chrome::padded(self.title, theme),
            theme.colors.accent,
            &game.title,
        );
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
        }
    }
}
