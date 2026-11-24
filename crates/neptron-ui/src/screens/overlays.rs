//! The dialogs over any screen: the confirmation of a signal change, and
//! the notices.

use core::fmt::Write;

use crate::frame::UiFrame;
use crate::geom::Rect;
use crate::input::UiAction;
use crate::intent::Intent;
use crate::metrics::Metrics;
use crate::snapshot::{ConfirmId, Notice, NoticeId, NoticeKind, PendingConfirm, Snapshot};
use crate::text::StackText;
use crate::theme::Theme;
use crate::widgets::{Dialog, DialogContent, DialogEvent, DialogTone};

use super::chrome;

/// The buttons of the confirm dialog. Revert comes first and has the focus
/// when the dialog opens, so a press of accept by a player who cannot see
/// the picture reverts it.
const CONFIRM_BUTTONS: [&str; 2] = ["Revert", "Keep"];
const REVERT: usize = 0;
const KEEP: usize = 1;

const NANOS_PER_SECOND: u64 = 1_000_000_000;
/// The longest message of a confirm dialog, in bytes: more than the lines
/// of a dialog hold.
const MESSAGE_BYTES: usize = 256;
/// Width of the background ring that separates a dialog from the screen
/// under it, in pixels.
const RING_PX: u32 = 4;
/// The notices that the UI remembers as dismissed until the app removes
/// them from the snapshot.
const DISMISSED_SLOTS: usize = 8;

/// The confirm dialog of a signal change. It shows the time left from the
/// snapshot; the app reverts when the time is over, and the UI never does.
///
/// Keep must be a deliberate choice: the focus starts on Revert for each
/// new confirmation, accept on Revert or back reverts, and only right then
/// accept keeps. A confirmation is new when its id differs from the one of
/// the last frame, or when the last frame had none. After one answer the
/// dialog ignores the other actions of that frame. The app carries out the
/// answer in the same frame: when the next snapshot still holds the same
/// confirmation, the app refused the keep, and the dialog takes input
/// again, with the focus where it was.
///
/// A held direction must not move the focus to Keep. The UI cannot see
/// that a key is held, because `UiAction` has no press or repeat flag: the
/// app stops every key repeat and ignores every key held when a
/// confirmation opens, so each direction that reaches the dialog is a
/// fresh press, and it moves the focus at once.
///
/// When no time is left, the dialog shows Revert alone: the app refuses a
/// keep at the deadline, so a Keep button there would do nothing.
pub(super) struct Confirm {
    dialog: Dialog,
    answered: bool,
    id: Option<ConfirmId>,
}

/// The notices of the snapshot, oldest first, one at a time.
pub(super) struct Notices {
    dialog: Dialog,
    dismissed: [Option<NoticeId>; DISMISSED_SLOTS],
    /// The notice of the last frame, so a new one starts with the focus on
    /// its first button.
    shown: Option<NoticeId>,
}

/// The first button of a notice with a choice. It has the focus when the
/// notice opens, so a press of accept by a player who did not read the
/// text does nothing that cannot be undone.
const CANCEL: usize = 0;

impl Confirm {
    pub(super) fn new() -> Self {
        Self {
            dialog: Dialog::new(REVERT),
            answered: false,
            id: None,
        }
    }

    /// Starts again for a new confirmation, and forgets the last one when
    /// the snapshot has none.
    pub(super) fn follow(&mut self, pending: Option<&PendingConfirm>) {
        let id = pending.map(|p| p.id);
        if id != self.id {
            *self = Self::new();
            self.id = id;
        }
    }

    /// Takes an answer again: called once per frame, before its actions.
    pub(super) fn begin_frame(&mut self) {
        self.answered = false;
    }

    pub(super) fn handle(
        &mut self,
        action: UiAction,
        pending: &PendingConfirm,
        out: &mut Vec<Intent>,
    ) {
        if self.answered {
            return;
        }
        let intent = match self.dialog.handle(action, buttons(pending).len()) {
            DialogEvent::Chosen(KEEP) => Intent::KeepMode,
            DialogEvent::Chosen(_) | DialogEvent::Cancelled => Intent::RevertMode,
            DialogEvent::None | DialogEvent::Moved => return,
        };
        out.push(intent);
        self.answered = true;
    }

    pub(super) fn draw(
        &self,
        pending: &PendingConfirm,
        theme: &Theme,
        metrics: &Metrics,
        frame: &mut UiFrame,
    ) {
        let mut message = StackText::<MESSAGE_BYTES>::new();
        let seconds = pending.remaining_ns.div_ceil(NANOS_PER_SECOND);
        // `StackText` never fails; it drops what does not fit.
        let _ = match (self.answered, seconds) {
            (true, _) => write!(message, "{}\n\nOne moment.", pending.description),
            (false, 0) => write!(
                message,
                "{}\n\nThe old picture comes back now.",
                pending.description
            ),
            (false, s) => write!(
                message,
                "{}\n\nThe old picture comes back in {s} s.",
                pending.description
            ),
        };
        let content = DialogContent {
            title: "Keep this picture?",
            message: message.as_str(),
            buttons: buttons(pending),
            tone: DialogTone::Normal,
        };
        draw_dialog(&self.dialog, &content, &[], theme, frame);
        if self.answered {
            chrome::hints(frame, theme, metrics, "", "");
        } else if pending.remaining_ns == 0 {
            chrome::hints(frame, theme, metrics, "Accept: revert", "");
        } else {
            chrome::hints(
                frame,
                theme,
                metrics,
                "Accept: revert",
                "Right, then Accept: keep",
            );
        }
    }
}

impl Notices {
    pub(super) fn new() -> Self {
        Self {
            dialog: Dialog::new(CANCEL),
            dismissed: [None; DISMISSED_SLOTS],
            shown: None,
        }
    }

    /// Forgets the dismissed notices that the app has removed, and puts
    /// the focus on the first button for a notice that is new on screen.
    pub(super) fn follow(&mut self, snap: &Snapshot) {
        for slot in &mut self.dismissed {
            if slot.is_some_and(|id| !snap.notices.iter().any(|n| n.id == id)) {
                *slot = None;
            }
        }
        let current = self.current(snap).map(|n| n.id);
        if current != self.shown {
            self.dialog = Dialog::new(CANCEL);
            self.shown = current;
        }
    }

    /// The notice after the newest one the player dismissed. The notices
    /// are oldest first and the player dismisses them in that order, so
    /// every notice before a dismissed one was dismissed too, also when its
    /// id no longer fits in the remembered set.
    pub(super) fn current<'a>(&self, snap: &'a Snapshot) -> Option<&'a Notice> {
        let after = snap
            .notices
            .iter()
            .rposition(|n| self.dismissed.contains(&Some(n.id)))
            .map_or(0, |last| last + 1);
        snap.notices.get(after)
    }

    /// Accept or back dismisses the notice; the other actions do nothing,
    /// so a notice cannot be passed over by accident. In a notice with a
    /// choice, left and right move the focus, accept on the second button
    /// sends [`Intent::AcceptNotice`], and back or accept on `Cancel`
    /// dismisses it.
    pub(super) fn handle(&mut self, action: UiAction, snap: &Snapshot, out: &mut Vec<Intent>) {
        let Some(notice) = self.current(snap) else {
            return;
        };
        let intent = if notice.choice.is_some() {
            match self.dialog.handle(action, 2) {
                DialogEvent::Chosen(CANCEL) | DialogEvent::Cancelled => {
                    Intent::DismissNotice(notice.id)
                }
                DialogEvent::Chosen(_) => Intent::AcceptNotice(notice.id),
                DialogEvent::None | DialogEvent::Moved => return,
            }
        } else if matches!(action, UiAction::Accept | UiAction::Back) {
            Intent::DismissNotice(notice.id)
        } else {
            return;
        };
        out.push(intent);
        self.remember(notice.id, snap);
        // A later action of the same frame goes to the next notice, which
        // starts with the focus on its first button too.
        self.dialog = Dialog::new(CANCEL);
        self.shown = self.current(snap).map(|n| n.id);
    }

    pub(super) fn draw(
        &self,
        notice: &Notice,
        theme: &Theme,
        metrics: &Metrics,
        frame: &mut UiFrame,
    ) {
        let (title, tone) = match notice.kind {
            NoticeKind::Info => ("Note", DialogTone::Normal),
            NoticeKind::Warning => ("Warning", DialogTone::Warning),
            NoticeKind::Error => ("Error", DialogTone::Error),
        };
        let Some(choice) = notice.choice.as_deref() else {
            let content = DialogContent {
                title,
                message: &notice.text,
                buttons: &["OK"],
                tone,
            };
            draw_dialog(&self.dialog, &content, &[], theme, frame);
            chrome::hints(frame, theme, metrics, "Accept: OK", "");
            return;
        };
        let content = DialogContent {
            title,
            message: &notice.text,
            buttons: &["Cancel", choice],
            tone,
        };
        draw_dialog(&self.dialog, &content, &[], theme, frame);
        chrome::hints(frame, theme, metrics, "Accept: choose", "Back: cancel");
    }

    /// Remembers a dismissed notice. When the set is full, every id in it
    /// is still in the snapshot (`follow` drops the others), and the id of
    /// the oldest notice makes room: a newer dismissed notice still hides
    /// it, see [`Notices::current`].
    fn remember(&mut self, id: NoticeId, snap: &Snapshot) {
        let position = |slot: &Option<NoticeId>| {
            slot.and_then(|old| snap.notices.iter().position(|n| n.id == old))
        };
        let slot = self.dismissed.iter().position(Option::is_none).or_else(|| {
            (0..DISMISSED_SLOTS).min_by_key(|i| self.dismissed.get(*i).and_then(position))
        });
        if let Some(entry) = slot.and_then(|i| self.dismissed.get_mut(i)) {
            *entry = Some(id);
        }
    }
}

/// The buttons of the confirm dialog for `pending`: Revert alone when no
/// time is left.
fn buttons(pending: &PendingConfirm) -> &'static [&'static str] {
    if pending.remaining_ns == 0 {
        &CONFIRM_BUTTONS[..1]
    } else {
        &CONFIRM_BUTTONS
    }
}

/// Draws a dialog with a ring of background colour around it, so its frame
/// stands apart from the text of the screen under it.
pub(super) fn draw_dialog(
    dialog: &Dialog,
    content: &DialogContent<'_>,
    waiting: &[bool],
    theme: &Theme,
    frame: &mut UiFrame,
) {
    let area = Dialog::layout(content, theme);
    let ring = Rect::new(
        area.x.saturating_sub(RING_PX as i32),
        area.y.saturating_sub(RING_PX as i32),
        area.w.saturating_add(2 * RING_PX),
        area.h.saturating_add(2 * RING_PX),
    );
    frame.scene.rect(ring, theme.colors.background);
    dialog.draw_waiting(frame, theme, content, waiting);
}
