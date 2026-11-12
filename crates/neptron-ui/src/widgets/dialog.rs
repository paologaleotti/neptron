use crate::font;
use crate::frame::UiFrame;
use crate::geom::Rect;
use crate::input::UiAction;
use crate::text::{self, Align};
use crate::theme::{SAFE_AREA, Theme};
use crate::tree::{TreeRow, WidgetKind};

use super::{add, framed, to_u32};

/// Width of a dialog in pixels.
pub const DIALOG_WIDTH_PX: u32 = 224;

const BORDER_PX: u32 = 1;
const BUTTON_GAP_PX: u32 = 8;
const BUTTON_MIN_WIDTH_PX: u32 = 48;

/// The text of a [`Dialog`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DialogContent<'a> {
    /// The title bar.
    pub title: &'a str,
    /// The message, wrapped at spaces. `\n` starts a new line.
    pub message: &'a str,
    /// The button labels, left to right.
    pub buttons: &'a [&'a str],
    /// The colour of the title bar.
    pub tone: DialogTone,
}

/// How serious a [`Dialog`] is. It sets the colour of the title bar, so a
/// warning or an error reads as one before the player reads the text.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DialogTone {
    /// A question or a note: the accent colour.
    #[default]
    Normal,
    /// Something the player should know: the warning colour.
    Warning,
    /// Something failed: the error colour.
    Error,
}

/// What one action did to a [`Dialog`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DialogEvent {
    /// Nothing changed.
    None,
    /// The focus moved to a different button.
    Moved,
    /// The player accepted the button at this index.
    Chosen(usize),
    /// The player pressed back.
    Cancelled,
}

/// A box in the middle of the safe area with a title, a message and a row
/// of buttons. Left and right move between the buttons.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Dialog {
    focus: usize,
}

impl Dialog {
    /// A dialog with the focus on the button at `focus`.
    pub fn new(focus: usize) -> Self {
        Self { focus }
    }

    /// The focused button.
    pub fn focus(&self) -> usize {
        self.focus
    }

    /// Applies one action to a dialog with `buttons` buttons.
    pub fn handle(&mut self, action: UiAction, buttons: usize) -> DialogEvent {
        self.focus = self.focus.min(buttons.saturating_sub(1));
        let target = match action {
            UiAction::Left => self.focus.saturating_sub(1),
            UiAction::Right => (self.focus + 1).min(buttons.saturating_sub(1)),
            UiAction::Accept if buttons > 0 => return DialogEvent::Chosen(self.focus),
            UiAction::Back => return DialogEvent::Cancelled,
            _ => return DialogEvent::None,
        };
        if target == self.focus {
            return DialogEvent::None;
        }
        self.focus = target;
        DialogEvent::Moved
    }

    /// The area the dialog covers for this content.
    pub fn layout(content: &DialogContent<'_>, theme: &Theme) -> Rect {
        let row = theme.row_height_px();
        let lines =
            to_u32(text::wrap(content.message, message_width(theme)).count()).min(max_lines(theme));
        let height = (2 * BORDER_PX + row + 3 * theme.padding_px() + button_row(theme))
            .saturating_add(lines.saturating_mul(row));
        let height = height.min(SAFE_AREA.h);
        let x = add(SAFE_AREA.x, (SAFE_AREA.w - DIALOG_WIDTH_PX) / 2);
        let y = add(SAFE_AREA.y, (SAFE_AREA.h - height) / 2);
        Rect::new(x, y, DIALOG_WIDTH_PX, height)
    }

    /// Draws the dialog.
    pub fn draw(&self, frame: &mut UiFrame, theme: &Theme, content: &DialogContent<'_>) {
        let area = Self::layout(content, theme);
        let row = theme.row_height_px();
        framed(
            &mut frame.scene,
            area,
            theme.colors.border,
            theme.colors.panel,
            BORDER_PX,
        );
        let inner = area.inset(BORDER_PX);
        let title = Rect::new(inner.x, inner.y, inner.w, row);
        let bar = match content.tone {
            DialogTone::Normal => theme.colors.accent,
            DialogTone::Warning => theme.colors.warning,
            DialogTone::Error => theme.colors.error,
        };
        frame.scene.rect(title, bar);
        let title_box = Rect::new(
            add(title.x, theme.padding_px()),
            title.y,
            message_width(theme),
            row,
        );
        let title_clipped = text::draw_in(
            &mut frame.scene,
            title_box,
            Align::Left,
            theme.colors.focus_text,
            content.title,
        );
        frame.tree.push(TreeRow {
            widget: WidgetKind::Label,
            label: content.title,
            value: None,
            source: None,
            rect: title,
            focused: false,
            clipped: title_clipped,
        });

        let message_clipped = self.draw_message(frame, theme, content.message, inner);
        frame.tree.push(TreeRow {
            widget: WidgetKind::Dialog,
            label: content.message,
            value: None,
            source: None,
            rect: inner,
            focused: false,
            clipped: message_clipped,
        });
        self.draw_buttons(frame, theme, content.buttons, area);
    }

    fn draw_message(&self, frame: &mut UiFrame, theme: &Theme, message: &str, inner: Rect) -> bool {
        let row = theme.row_height_px();
        let width = message_width(theme);
        let limit = max_lines(theme) as usize;
        let top = add(inner.y, row + theme.padding_px());
        let mut clipped = false;
        for (i, line) in text::wrap(message, width).enumerate() {
            if i >= limit {
                clipped = true;
                break;
            }
            let y = add(top, to_u32(i).saturating_mul(row));
            let line_box = Rect::new(add(inner.x, theme.padding_px()), y, width, row);
            clipped |= text::draw_in(
                &mut frame.scene,
                line_box,
                Align::Left,
                theme.colors.text,
                line,
            );
        }
        clipped
    }

    fn draw_buttons(&self, frame: &mut UiFrame, theme: &Theme, buttons: &[&str], area: Rect) {
        let count = to_u32(buttons.len());
        if count == 0 {
            return;
        }
        let row = theme.row_height_px();
        let widest = buttons
            .iter()
            .map(|b| font::text_width(b))
            .max()
            .unwrap_or(0);
        let gaps = (count - 1).saturating_mul(BUTTON_GAP_PX);
        let max_w = DIALOG_WIDTH_PX
            .saturating_sub(2 * theme.padding_px())
            .saturating_sub(gaps)
            / count;
        let button_w = widest
            .saturating_add(2 * theme.padding_px())
            .max(BUTTON_MIN_WIDTH_PX)
            .min(max_w);
        let total = count.saturating_mul(button_w).saturating_add(gaps);
        let x0 = add(area.x, (area.w.saturating_sub(total)) / 2);
        let y = i32::try_from(area.bottom() - i64::from(BORDER_PX + theme.padding_px() + row + 2))
            .unwrap_or(area.y);
        let focus = self.focus.min(buttons.len() - 1);
        for (i, label) in buttons.iter().enumerate() {
            let x = add(x0, to_u32(i).saturating_mul(button_w + BUTTON_GAP_PX));
            let rect = Rect::new(x, y, button_w, row + 2);
            let focused = i == focus;
            let (fill, color) = if focused {
                (theme.colors.focus, theme.colors.focus_text)
            } else {
                (theme.colors.panel, theme.colors.text)
            };
            framed(&mut frame.scene, rect, theme.colors.border, fill, BORDER_PX);
            let clipped = text::draw_in(&mut frame.scene, rect, Align::Center, color, label);
            frame.tree.push(TreeRow {
                widget: WidgetKind::Button,
                label,
                value: None,
                source: None,
                rect,
                focused,
                clipped,
            });
        }
    }
}

fn message_width(theme: &Theme) -> u32 {
    DIALOG_WIDTH_PX.saturating_sub(2 * BORDER_PX + 2 * theme.padding_px())
}

fn button_row(theme: &Theme) -> u32 {
    theme.row_height_px() + 2 + 2 * theme.padding_px()
}

fn max_lines(theme: &Theme) -> u32 {
    let fixed = 2 * BORDER_PX + theme.row_height_px() + 3 * theme.padding_px() + button_row(theme);
    SAFE_AREA.h.saturating_sub(fixed) / theme.row_height_px().max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn many_buttons_do_not_overflow() {
        for count in [27, 28, 29, 1000] {
            let buttons = vec!["Button"; count];
            let content = DialogContent {
                buttons: &buttons,
                ..CONTENT
            };
            let mut frame = UiFrame::new();
            let mut dialog = Dialog::new(count - 1);
            assert_eq!(dialog.handle(UiAction::Right, count), DialogEvent::None);
            dialog.draw(&mut frame, &Theme::default(), &content);
            let drawn = frame
                .tree
                .rows()
                .filter(|r| r.widget == WidgetKind::Button)
                .count();
            assert_eq!(drawn, count);
        }
    }

    #[test]
    fn the_largest_sizes_keep_three_message_lines() {
        let theme = Theme::new(
            Default::default(),
            crate::theme::ROW_HEIGHT_MAX_PX,
            crate::theme::PADDING_MAX_PX,
        )
        .expect("valid");
        assert!(message_width(&theme) >= 30 * font::ADVANCE);
        assert!(max_lines(&theme) >= 3);
    }

    const CONTENT: DialogContent<'static> = DialogContent {
        title: "Keep this mode?",
        message: "The picture reverts in 10 seconds unless you keep it.",
        buttons: &["Keep", "Revert"],
        tone: DialogTone::Normal,
    };

    #[test]
    fn left_and_right_move_between_buttons() {
        let mut d = Dialog::new(0);
        assert_eq!(d.handle(UiAction::Left, 2), DialogEvent::None);
        assert_eq!(d.handle(UiAction::Right, 2), DialogEvent::Moved);
        assert_eq!(d.handle(UiAction::Right, 2), DialogEvent::None);
        assert_eq!(d.handle(UiAction::Accept, 2), DialogEvent::Chosen(1));
        assert_eq!(d.handle(UiAction::Up, 2), DialogEvent::None);
        assert_eq!(d.handle(UiAction::Back, 2), DialogEvent::Cancelled);
    }

    #[test]
    fn a_dialog_without_buttons_cannot_be_accepted() {
        let mut d = Dialog::new(3);
        assert_eq!(d.handle(UiAction::Accept, 0), DialogEvent::None);
        assert_eq!(d.focus(), 0);
    }

    #[test]
    fn the_dialog_is_centred_in_the_safe_area() {
        let area = Dialog::layout(&CONTENT, &Theme::default());
        assert!(SAFE_AREA.contains(&area));
        assert_eq!(area.x, 48);
        assert_eq!(i64::from(area.y) - 12, 228 - area.bottom());
    }

    #[test]
    fn the_tree_has_title_message_and_buttons() {
        let mut frame = UiFrame::new();
        Dialog::new(1).draw(&mut frame, &Theme::default(), &CONTENT);
        let rows: Vec<_> = frame
            .tree
            .rows()
            .map(|r| (r.widget, r.label, r.focused, r.clipped))
            .collect();
        assert_eq!(
            rows,
            [
                (WidgetKind::Label, CONTENT.title, false, false),
                (WidgetKind::Dialog, CONTENT.message, false, false),
                (WidgetKind::Button, "Keep", false, false),
                (WidgetKind::Button, "Revert", true, false),
            ]
        );
        let area = Dialog::layout(&CONTENT, &Theme::default());
        assert!(frame.tree.rows().all(|r| area.contains(&r.rect)));
    }

    #[test]
    fn the_tone_sets_the_colour_of_the_title_bar() {
        let theme = Theme::default();
        let tones = [
            (DialogTone::Normal, theme.colors.accent),
            (DialogTone::Warning, theme.colors.warning),
            (DialogTone::Error, theme.colors.error),
        ];
        for (tone, colour) in tones {
            let mut frame = UiFrame::new();
            Dialog::new(0).draw(&mut frame, &theme, &DialogContent { tone, ..CONTENT });
            let bar = frame.tree.rows().next().expect("title row").rect;
            let fill = frame.scene.commands().iter().find_map(|c| match *c {
                crate::DrawCommand::Rect { rect, color } if rect == bar => Some(color),
                _ => None,
            });
            assert_eq!(fill, Some(colour), "{tone:?}");
        }
    }

    #[test]
    fn a_long_message_is_cut_and_reported() {
        let long = "word ".repeat(200);
        let content = DialogContent {
            message: &long,
            ..CONTENT
        };
        let mut frame = UiFrame::new();
        Dialog::new(0).draw(&mut frame, &Theme::default(), &content);
        let message = frame.tree.rows().nth(1).expect("message row");
        assert!(message.clipped);
        assert!(SAFE_AREA.contains(&Dialog::layout(&content, &Theme::default())));
    }
}
