//! The home screen: the systems that contain games, the favourites, the
//! recent games, the display panel and the audio panel.

use core::fmt::Write;

use crate::frame::UiFrame;
use crate::geom::Rect;
use crate::input::UiAction;
use crate::snapshot::{GameSource, Snapshot};
use crate::text::StackText;
use crate::theme::Theme;
use crate::widgets::{List, ListEvent, ListItem};

use super::chrome::{self, Layout};
use super::router::{Nav, Target};

/// The text of the home screen when no system has games: what a new user
/// sees first. The folder names are those of the system definitions.
pub(super) const NO_GAMES: &str = "No games yet.\n\nCopy your games into the roms folder of the share partition, one folder per system:\n  roms/nes   roms/snes   roms/megadrive\n\nPlain files and .zip files both work. The games show here when the scan finds them.";

/// The rows whose game count is formatted in one frame. The content area
/// shows at most 15 rows of the smallest row height.
const COUNTED_ROWS: usize = 16;

/// One row of the home screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Row {
    System(usize),
    Favourites,
    Recents,
    Display,
    Audio,
}

/// The home screen. With no systems it shows the empty-state text and one
/// row, the display panel, at the bottom: a new user can fix the picture
/// before any game plays a sound.
pub(super) struct Home {
    list: List,
    list_box: Rect,
    alone: List,
    alone_box: Rect,
}

impl Home {
    pub(super) fn new(layout: &Layout, theme: &Theme) -> Self {
        let row = theme.row_height_px();
        let content = layout.content;
        let (list_box, list) = chrome::rows_box(content, theme, chrome::box_rows(content, theme));
        let (alone_box, alone) = chrome::rows_box_bottom(content, theme, 1);
        Self {
            list: List::new(list, row),
            list_box,
            alone: List::new(alone, row).without_arrows(),
            alone_box,
        }
    }

    pub(super) fn handle(&mut self, action: UiAction, snap: &Snapshot) -> Nav {
        let list = if snap.systems.is_empty() {
            &mut self.alone
        } else {
            &mut self.list
        };
        let ListEvent::Activated(i) = list.handle(action, row_count(snap)) else {
            return Nav::Stay;
        };
        match row(snap, i) {
            Some(Row::System(s)) => snap.systems.get(s).map_or(Nav::Stay, |system| {
                Nav::Open(Target::Games(GameSource::System(system.id)))
            }),
            Some(Row::Favourites) => Nav::Open(Target::Games(GameSource::Favourites)),
            Some(Row::Recents) => Nav::Open(Target::Games(GameSource::Recents)),
            Some(Row::Display) => Nav::Open(Target::Display),
            Some(Row::Audio) => Nav::Open(Target::Audio),
            None => Nav::Stay,
        }
    }

    pub(super) fn draw(
        &self,
        snap: &Snapshot,
        theme: &Theme,
        layout: &Layout,
        frame: &mut UiFrame,
    ) {
        chrome::frame(frame, theme, "NEPTRON", "", "Accept: open", "");
        if snap.systems.is_empty() {
            let text = Rect::new(
                layout.content.x,
                layout.content.y,
                layout.content.w,
                (self.alone_box.y - layout.content.y).max(0) as u32,
            );
            chrome::paragraph(
                frame,
                theme,
                chrome::padded(text, theme),
                theme.colors.text,
                NO_GAMES,
            );
            chrome::boxed(frame, theme, self.alone_box);
            self.alone
                .draw(frame, theme, 1, |_| ListItem::page("Display"));
            return;
        }
        chrome::boxed(frame, theme, self.list_box);
        let top = self.list.top();
        let mut counts = [StackText::<16>::new(); COUNTED_ROWS];
        for (slot, buf) in counts.iter_mut().enumerate() {
            if let Some(Row::System(s)) = row(snap, top + slot) {
                let n = snap.systems.get(s).map_or(0, |s| s.game_count);
                let unit = if n == 1 { "game" } else { "games" };
                // `StackText` never fails; it drops what does not fit.
                let _ = write!(buf, "{n} {unit}");
            }
        }
        self.list.draw(frame, theme, row_count(snap), |i| {
            let detail = i
                .checked_sub(top)
                .and_then(|slot| counts.get(slot))
                .map(StackText::as_str)
                .filter(|s| !s.is_empty());
            let label = match row(snap, i) {
                Some(Row::System(s)) => snap.systems.get(s).map_or("", |s| s.name.as_str()),
                Some(Row::Favourites) => "Favourites",
                Some(Row::Recents) => "Recent games",
                Some(Row::Display) => "Display",
                Some(Row::Audio) => "Audio",
                None => "",
            };
            ListItem {
                detail,
                ..ListItem::page(label)
            }
        });
    }
}

fn row_count(snap: &Snapshot) -> usize {
    match snap.systems.len() {
        0 => 1,
        systems => systems.saturating_add(4),
    }
}

fn row(snap: &Snapshot, index: usize) -> Option<Row> {
    let systems = snap.systems.len();
    if systems == 0 {
        return (index == 0).then_some(Row::Display);
    }
    match index.checked_sub(systems) {
        None => Some(Row::System(index)),
        Some(0) => Some(Row::Favourites),
        Some(1) => Some(Row::Recents),
        Some(2) => Some(Row::Display),
        Some(3) => Some(Row::Audio),
        Some(_) => None,
    }
}
