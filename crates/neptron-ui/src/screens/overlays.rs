//! The dialogs over any screen: the confirmation of a signal change, and
//! the notices.

use core::fmt::Write;

use crate::frame::UiFrame;
use crate::geom::Rect;
use crate::input::UiAction;
use crate::intent::Intent;
use crate::snapshot::{ConfirmId, Notice, NoticeId, NoticeKind, PendingConfirm, Snapshot};
use crate::text::StackText;
use crate::theme::Theme;
use crate::widgets::{Dialog, DialogContent, DialogEvent};

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
/// The frames in a row with no direction action that the confirm dialog
/// waits for before left and right move its focus.
///
/// `UiAction` has no press or repeat flag, so the dialog cannot see that a
/// key is still held: a direction held from the screen below reaches it as
/// key repeat. One quiet frame proves nothing, because key repeat leaves
/// gaps (a first delay of about 400 ms, then a press every few frames).
/// 60 frames is longer than such a delay at every refresh up to 120 Hz, so
/// a held key keeps the dialog waiting, and the player who releases the
/// key can keep the mode about one second later.
pub(super) const QUIET_FRAMES_BEFORE_MOVE: u32 = 60;
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
/// dialog ignores input until the snapshot drops the confirmation or holds
/// a new one.
///
/// A held direction must not move the focus to Keep: after a confirmation
/// appears, the dialog ignores left and right until it has seen
/// [`QUIET_FRAMES_BEFORE_MOVE`] frames in a row with no direction action.
/// Accept and back work at once, and revert while the focus is on Revert.
pub(super) struct Confirm {
    dialog: Dialog,
    answered: bool,
    id: Option<ConfirmId>,
    quiet_frames: u32,
}

/// The notices of the snapshot, oldest first, one at a time.
pub(super) struct Notices {
    dialog: Dialog,
    dismissed: [Option<NoticeId>; DISMISSED_SLOTS],
    next_slot: usize,
}

impl Confirm {
    pub(super) fn new() -> Self {
        Self {
            dialog: Dialog::new(REVERT),
            answered: false,
            id: None,
            quiet_frames: 0,
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

    /// Counts the frames in a row with no direction action, once per frame
    /// after its actions. A direction starts the count again until the
    /// dialog takes left and right.
    pub(super) fn end_frame(&mut self, actions: &[UiAction]) {
        if self.takes_directions() {
            return;
        }
        let direction = actions.iter().any(|a| {
            matches!(
                a,
                UiAction::Up | UiAction::Down | UiAction::Left | UiAction::Right
            )
        });
        self.quiet_frames = if direction {
            0
        } else {
            self.quiet_frames.saturating_add(1)
        };
    }

    pub(super) fn handle(&mut self, action: UiAction, out: &mut Vec<Intent>) {
        if self.answered {
            return;
        }
        if matches!(action, UiAction::Left | UiAction::Right) && !self.takes_directions() {
            return;
        }
        let intent = match self.dialog.handle(action, CONFIRM_BUTTONS.len()) {
            DialogEvent::Chosen(KEEP) => Intent::KeepMode,
            DialogEvent::Chosen(_) | DialogEvent::Cancelled => Intent::RevertMode,
            DialogEvent::None | DialogEvent::Moved => return,
        };
        out.push(intent);
        self.answered = true;
    }

    fn takes_directions(&self) -> bool {
        self.quiet_frames >= QUIET_FRAMES_BEFORE_MOVE
    }

    pub(super) fn draw(&self, pending: &PendingConfirm, theme: &Theme, frame: &mut UiFrame) {
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
            buttons: &CONFIRM_BUTTONS,
        };
        draw_dialog(&self.dialog, &content, theme, frame);
    }
}

impl Notices {
    pub(super) fn new() -> Self {
        Self {
            dialog: Dialog::new(0),
            dismissed: [None; DISMISSED_SLOTS],
            next_slot: 0,
        }
    }

    /// Forgets the dismissed notices that the app has removed.
    pub(super) fn follow(&mut self, snap: &Snapshot) {
        for slot in &mut self.dismissed {
            if slot.is_some_and(|id| !snap.notices.iter().any(|n| n.id == id)) {
                *slot = None;
            }
        }
    }

    /// The oldest notice that the player has not dismissed.
    pub(super) fn current<'a>(&self, snap: &'a Snapshot) -> Option<&'a Notice> {
        snap.notices
            .iter()
            .find(|n| !self.dismissed.contains(&Some(n.id)))
    }

    /// Accept or back dismisses the notice; the other actions do nothing,
    /// so a notice cannot be passed over by accident.
    pub(super) fn handle(&mut self, action: UiAction, snap: &Snapshot, out: &mut Vec<Intent>) {
        let Some(notice) = self.current(snap) else {
            return;
        };
        if matches!(action, UiAction::Accept | UiAction::Back) {
            out.push(Intent::DismissNotice(notice.id));
            self.remember(notice.id);
        }
    }

    pub(super) fn draw(&self, notice: &Notice, theme: &Theme, frame: &mut UiFrame) {
        let title = match notice.kind {
            NoticeKind::Info => "Note",
            NoticeKind::Warning => "Warning",
            NoticeKind::Error => "Error",
        };
        let content = DialogContent {
            title,
            message: &notice.text,
            buttons: &["OK"],
        };
        draw_dialog(&self.dialog, &content, theme, frame);
    }

    fn remember(&mut self, id: NoticeId) {
        let free = self.dismissed.iter().position(Option::is_none);
        let slot = free.unwrap_or(self.next_slot % DISMISSED_SLOTS);
        if let Some(entry) = self.dismissed.get_mut(slot) {
            *entry = Some(id);
        }
        self.next_slot = slot.wrapping_add(1);
    }
}

/// Draws a dialog with a ring of background colour around it, so its frame
/// stands apart from the text of the screen under it.
pub(super) fn draw_dialog(
    dialog: &Dialog,
    content: &DialogContent<'_>,
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
    dialog.draw(frame, theme, content);
}
