//! The screen of the guided mapping of a controller: the prompt of each
//! step, then the review that keeps or cancels the new map.

use core::fmt::Write;

use crate::frame::UiFrame;
use crate::geom::Rect;
use crate::input::UiAction;
use crate::intent::Intent;
use crate::snapshot::{MappingId, MappingStage, PadMapping};
use crate::text::{Align, StackText};
use crate::theme::Theme;
use crate::widgets::{Dialog, DialogContent, DialogEvent, DialogTone};

use super::chrome::{self, Layout, offset};
use super::overlays::draw_dialog;

/// The buttons of the review. `Cancel` comes first and has the focus, so
/// a blind accept with a map that does not work gives the old map back.
const REVIEW_BUTTONS: [&str; 2] = ["Cancel", "Keep"];
const CANCEL: usize = 0;
const KEEP: usize = 1;

const NANOS_PER_SECOND: u64 = 1_000_000_000;
/// The longest message of the review, in bytes.
const MESSAGE_BYTES: usize = 384;
/// The longest line of the prompt screen that the screen composes.
const LINE_BYTES: usize = 64;
/// Height of the progress bar, in pixels: two lines, so an interlaced mode
/// shows it in both fields.
const PROGRESS_H_PX: u32 = 6;

/// The guided mapping over every screen. Back from any device cancels it:
/// the pad of the mapping sends no action while it runs, so the player
/// with a pad that half works can always leave from the keyboard or from
/// another pad. The app also ends it when its time runs out.
///
/// The review is a dialog with `Cancel` and `Keep`, as the confirmation of
/// a picture: the new map drives the dialog, so only a pad whose D-pad and
/// accept work in the new map can choose `Keep`. A review whose map lacks
/// what the menus need shows `Cancel` alone.
pub(super) struct PadMappingScreen {
    dialog: Dialog,
    answered: bool,
    /// The mapping of the last frame and whether it was in its review.
    shown: Option<(MappingId, bool)>,
}

impl PadMappingScreen {
    pub(super) fn new() -> Self {
        Self {
            dialog: Dialog::new(CANCEL),
            answered: false,
            shown: None,
        }
    }

    /// Starts again for a new mapping, or for the review of the mapping.
    pub(super) fn follow(&mut self, mapping: Option<&PadMapping>) {
        let shown = mapping.map(|m| (m.id, matches!(m.stage, MappingStage::Review { .. })));
        if shown != self.shown {
            *self = Self::new();
            self.shown = shown;
        }
    }

    /// Takes an answer again: called once per frame, before its actions.
    pub(super) fn begin_frame(&mut self) {
        self.answered = false;
    }

    /// One action. After an answer the screen ignores the other actions of
    /// the frame.
    pub(super) fn handle(&mut self, action: UiAction, mapping: &PadMapping, out: &mut Vec<Intent>) {
        if self.answered {
            return;
        }
        let intent = match &mapping.stage {
            MappingStage::Prompt { .. } => match action {
                UiAction::Back => Intent::CancelPadMapping,
                _ => return,
            },
            MappingStage::Review { can_keep, .. } => {
                match self.dialog.handle(action, buttons(*can_keep).len()) {
                    DialogEvent::Chosen(KEEP) => Intent::KeepPadMapping,
                    DialogEvent::Chosen(_) | DialogEvent::Cancelled => Intent::CancelPadMapping,
                    DialogEvent::None | DialogEvent::Moved => return,
                }
            }
        };
        out.push(intent);
        self.answered = true;
    }

    /// The screen id of the tree.
    pub(super) fn screen_id(mapping: &PadMapping) -> &'static str {
        match mapping.stage {
            MappingStage::Prompt { .. } => "pad-mapping",
            MappingStage::Review { .. } => "pad-mapping/review",
        }
    }

    pub(super) fn draw(&self, mapping: &PadMapping, theme: &Theme, frame: &mut UiFrame) {
        let seconds = mapping.remaining_ns.div_ceil(NANOS_PER_SECOND);
        match &mapping.stage {
            MappingStage::Prompt {
                prompt,
                step,
                steps,
                note,
            } => draw_prompt(
                frame,
                theme,
                mapping,
                prompt,
                (*step, *steps),
                note,
                seconds,
            ),
            MappingStage::Review { message, can_keep } => {
                chrome::background(frame, theme);
                let mut text = StackText::<MESSAGE_BYTES>::new();
                // `StackText` never fails; it drops what does not fit.
                let _ = match (*can_keep, self.answered) {
                    (_, true) => write!(text, "{message}\n\nOne moment."),
                    (true, false) => {
                        write!(
                            text,
                            "{message}\n\nThe old layout comes back in {seconds} s."
                        )
                    }
                    (false, false) => write!(text, "{message}\n\nThis closes in {seconds} s."),
                };
                let (title, tone) = if *can_keep {
                    ("Keep this layout?", DialogTone::Normal)
                } else {
                    ("This layout is not complete", DialogTone::Warning)
                };
                let content = DialogContent {
                    title,
                    message: text.as_str(),
                    buttons: buttons(*can_keep),
                    tone,
                };
                draw_dialog(&self.dialog, &content, theme, frame);
                if *can_keep {
                    chrome::hints(frame, theme, "Accept: cancel", "Right, then Accept: keep");
                } else {
                    chrome::hints(frame, theme, "Accept: cancel", "");
                }
            }
        }
    }
}

/// The buttons of the review: `Cancel` alone when the map cannot be kept.
fn buttons(can_keep: bool) -> &'static [&'static str] {
    if can_keep {
        &REVIEW_BUTTONS
    } else {
        &REVIEW_BUTTONS[..1]
    }
}

/// The prompt of a step: the name of the pad, what to press in a bar, the
/// progress, the note, how to skip, how to cancel and the countdown.
fn draw_prompt(
    frame: &mut UiFrame,
    theme: &Theme,
    mapping: &PadMapping,
    prompt: &str,
    (step, steps): (u32, u32),
    note: &str,
    seconds: u64,
) {
    let mut progress = StackText::<LINE_BYTES>::new();
    // `StackText` never fails; it drops what does not fit.
    let _ = write!(progress, "Step {step} of {steps}");
    chrome::frame(
        frame,
        theme,
        "Map buttons",
        progress.as_str(),
        "Back: cancel",
        "Hold: skip",
    );
    let content = Layout::new(theme).content;
    let row = theme.row_height_px();
    let rows = 11;
    let (outer, inner) = chrome::rows_box(content, theme, rows);
    chrome::boxed(frame, theme, outer);
    let inner = chrome::padded(inner, theme);
    let line = |n: u32| Rect::new(inner.x, offset(inner.y, n * row), inner.w, row);
    let colors = &theme.colors;
    chrome::label(frame, line(0), Align::Left, colors.text_dim, &mapping.pad);

    let bar = Rect::new(inner.x, offset(inner.y, 2 * row), inner.w, 2 * row);
    frame.scene.rect(bar, colors.focus);
    chrome::label(frame, bar, Align::Center, colors.focus_text, prompt);

    let track = Rect::new(
        inner.x,
        offset(inner.y, 5 * row + row.saturating_sub(PROGRESS_H_PX) / 2),
        inner.w,
        PROGRESS_H_PX,
    );
    frame.scene.rect(track, colors.border);
    let done = u64::from(step.saturating_sub(1)) * u64::from(track.w) / u64::from(steps.max(1));
    let filled = Rect::new(
        track.x,
        track.y,
        u32::try_from(done).unwrap_or(track.w),
        track.h,
    );
    frame.scene.rect(filled, colors.accent);

    if !note.is_empty() {
        chrome::label(frame, line(6), Align::Left, colors.warning, note);
    }
    chrome::label(
        frame,
        line(7),
        Align::Left,
        colors.text,
        "Hold a button of this controller to skip it.",
    );
    chrome::label(
        frame,
        line(8),
        Align::Left,
        colors.text,
        "Back on a keyboard or another pad: cancel.",
    );
    let mut countdown = StackText::<LINE_BYTES>::new();
    let _ = write!(countdown, "No input: cancel in {seconds} s.");
    chrome::label(
        frame,
        line(9),
        Align::Left,
        colors.text_dim,
        countdown.as_str(),
    );
}
