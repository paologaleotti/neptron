use crate::font;
use crate::frame::UiFrame;
use crate::geom::Rect;
use crate::input::UiAction;
use crate::metrics::{DIALOG_BORDER_PX as BORDER_PX, Metrics};
use crate::text::{self, Align};
use crate::theme::Theme;
use crate::tree::{TreeRow, WidgetKind};

use super::{add, framed, to_u32};

const BUTTON_GAP_PX: u32 = 8;
const BUTTON_MIN_WIDTH_PX: u32 = 48;

/// The value of a waiting button in the tree.
const WAITING: &str = "...";

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

/// A box in the middle of the dialog column of the metrics with a title, a message and a row
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

    /// The area the dialog covers for this content, in the dialog column
    /// of `metrics`.
    pub fn layout(content: &DialogContent<'_>, theme: &Theme, metrics: &Metrics) -> Rect {
        let column = metrics.dialog;
        let row = theme.row_height_px();
        let lines = to_u32(text::wrap(content.message, message_width(column, theme)).count())
            .min(max_lines(column, theme));
        // A dialog with no buttons, such as a box that says what happens
        // now, ends below its text.
        let buttons = if content.buttons.is_empty() {
            0
        } else {
            theme.padding_px() + button_row(theme)
        };
        let height = (2 * BORDER_PX + row + 2 * theme.padding_px() + buttons)
            .saturating_add(lines.saturating_mul(row));
        let height = height.min(column.h);
        let y = add(column.y, (column.h - height) / 2);
        Rect::new(column.x, y, column.w, height)
    }

    /// Draws the dialog.
    pub fn draw(
        &self,
        frame: &mut UiFrame,
        theme: &Theme,
        metrics: &Metrics,
        content: &DialogContent<'_>,
    ) {
        self.draw_waiting(frame, theme, metrics, content, &[]);
    }

    /// Draws the dialog with the buttons that wait marked: `waiting[i]` is
    /// true for a button that takes no choice yet. Its label is dim, and
    /// its row in the tree has the value `...`.
    pub fn draw_waiting(
        &self,
        frame: &mut UiFrame,
        theme: &Theme,
        metrics: &Metrics,
        content: &DialogContent<'_>,
        waiting: &[bool],
    ) {
        let column = metrics.dialog;
        let area = Self::layout(content, theme, metrics);
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
            message_width(column, theme),
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
            kind: None,
            label: content.title,
            value: None,
            source: None,
            rect: title,
            focused: false,
            clipped: title_clipped,
        });

        let message_clipped = self.draw_message(frame, theme, column, content.message, inner);
        frame.tree.push(TreeRow {
            widget: WidgetKind::Dialog,
            kind: None,
            label: content.message,
            value: None,
            source: None,
            rect: inner,
            focused: false,
            clipped: message_clipped,
        });
        self.draw_buttons(frame, theme, content.buttons, waiting, area);
    }

    fn draw_message(
        &self,
        frame: &mut UiFrame,
        theme: &Theme,
        column: Rect,
        message: &str,
        inner: Rect,
    ) -> bool {
        let row = theme.row_height_px();
        let width = message_width(column, theme);
        let limit = max_lines(column, theme) as usize;
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

    fn draw_buttons(
        &self,
        frame: &mut UiFrame,
        theme: &Theme,
        buttons: &[&str],
        waiting: &[bool],
        area: Rect,
    ) {
        let count = to_u32(buttons.len());
        if count == 0 {
            return;
        }
        let row = theme.row_height_px();
        let widths = ButtonWidths::of(buttons, area.w, theme);
        let gaps = (count - 1).saturating_mul(BUTTON_GAP_PX);
        let total = buttons
            .iter()
            .fold(gaps, |sum, b| sum.saturating_add(widths.width(b, theme)));
        let x0 = add(area.x, (area.w.saturating_sub(total)) / 2);
        let y = i32::try_from(area.bottom() - i64::from(BORDER_PX + theme.padding_px() + row + 2))
            .unwrap_or(area.y);
        let focus = self.focus.min(buttons.len() - 1);
        let mut x = x0;
        for (i, label) in buttons.iter().enumerate() {
            let button_w = widths.width(label, theme);
            let left = x;
            x = add(x, button_w.saturating_add(BUTTON_GAP_PX));
            let rect = Rect::new(left, y, button_w, row + 2);
            let focused = i == focus;
            let waits = waiting.get(i).copied().unwrap_or(false);
            let (fill, color) = match (focused, waits) {
                (true, false) => (theme.colors.focus, theme.colors.focus_text),
                (true, true) => (theme.colors.border, theme.colors.text_dim),
                (false, false) => (theme.colors.panel, theme.colors.text),
                (false, true) => (theme.colors.panel, theme.colors.text_dim),
            };
            framed(&mut frame.scene, rect, theme.colors.border, fill, BORDER_PX);
            let clipped = text::draw_in(&mut frame.scene, rect, Align::Center, color, label);
            frame.tree.push(TreeRow {
                widget: WidgetKind::Button,
                kind: None,
                label,
                value: waits.then_some(WAITING),
                source: None,
                rect,
                focused,
                clipped,
            });
        }
    }
}

/// How wide the buttons of a row are.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ButtonWidths {
    /// Every button has this width.
    Equal(u32),
    /// Each button has the width of its label ([`own_width`]).
    Own,
}

impl ButtonWidths {
    /// The buttons share one width, that of the widest label, while it
    /// fits in an equal share of the row. A longer label, such as `Set up
    /// this controller`, gets its own width when every button then fits;
    /// else every button gets the equal share and the long label is cut.
    fn of(buttons: &[&str], dialog_w: u32, theme: &Theme) -> Self {
        let count = to_u32(buttons.len()).max(1);
        let gaps = (count - 1).saturating_mul(BUTTON_GAP_PX);
        let room = dialog_w
            .saturating_sub(2 * theme.padding_px())
            .saturating_sub(gaps);
        let share = room / count;
        let widest = buttons
            .iter()
            .map(|b| own_width(b, theme))
            .max()
            .unwrap_or(0);
        if widest <= share {
            return Self::Equal(widest);
        }
        let sum = buttons
            .iter()
            .fold(0u32, |sum, b| sum.saturating_add(own_width(b, theme)));
        if sum <= room {
            Self::Own
        } else {
            Self::Equal(share)
        }
    }

    fn width(self, label: &str, theme: &Theme) -> u32 {
        match self {
            Self::Equal(width) => width,
            Self::Own => own_width(label, theme),
        }
    }
}

/// The width of a button that holds `label` whole.
fn own_width(label: &str, theme: &Theme) -> u32 {
    font::text_width(label)
        .saturating_add(2 * theme.padding_px())
        .max(BUTTON_MIN_WIDTH_PX)
}

fn message_width(column: Rect, theme: &Theme) -> u32 {
    column
        .w
        .saturating_sub(2 * BORDER_PX + 2 * theme.padding_px())
}

fn button_row(theme: &Theme) -> u32 {
    theme.row_height_px() + 2 + 2 * theme.padding_px()
}

fn max_lines(column: Rect, theme: &Theme) -> u32 {
    let fixed = 2 * BORDER_PX + theme.row_height_px() + 3 * theme.padding_px() + button_row(theme);
    column.h.saturating_sub(fixed) / theme.row_height_px().max(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::Form;

    fn landscape(theme: &Theme) -> Metrics {
        Metrics::of(Form::Landscape, theme)
    }

    #[test]
    fn a_long_second_label_gets_its_own_width_when_both_fit() {
        let theme = Theme::default();
        let content = DialogContent {
            buttons: &["Cancel", "Set up this controller"],
            ..CONTENT
        };
        let mut frame = UiFrame::new();
        Dialog::new(0).draw(&mut frame, &theme, &landscape(&theme), &content);
        let buttons: Vec<_> = frame
            .tree
            .rows()
            .filter(|r| r.widget == WidgetKind::Button)
            .map(|r| (r.label, r.clipped, r.rect))
            .collect();
        assert_eq!(buttons.len(), 2);
        assert!(
            buttons.iter().all(|(_, clipped, _)| !clipped),
            "{buttons:?}"
        );
        assert!(buttons[1].2.w > buttons[0].2.w);
        let area = Dialog::layout(&content, &theme, &landscape(&theme));
        assert!(buttons[0].2.x >= area.x);
        assert!(i64::from(buttons[1].2.x) + i64::from(buttons[1].2.w) <= area.right());
        assert_eq!(
            ButtonWidths::of(&["Cancel", "Start"], 224, &theme),
            ButtonWidths::Equal(BUTTON_MIN_WIDTH_PX),
            "short labels keep one width"
        );
        let long = "A label far too long for any button of a dialog";
        assert!(matches!(
            ButtonWidths::of(&["Cancel", long], 224, &theme),
            ButtonWidths::Equal(_)
        ));
    }

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
            dialog.draw(&mut frame, &Theme::default(), &Metrics::default(), &content);
            let drawn = frame
                .tree
                .rows()
                .filter(|r| r.widget == WidgetKind::Button)
                .count();
            assert_eq!(drawn, count);
        }
    }

    #[test]
    fn a_dialog_with_no_buttons_is_as_tall_as_its_text() {
        let theme = Theme::default();
        let with = Dialog::layout(&CONTENT, &theme, &landscape(&theme));
        let none = DialogContent {
            buttons: &[],
            ..CONTENT
        };
        let without = Dialog::layout(&none, &theme, &landscape(&theme));
        assert_eq!(
            without.h,
            with.h - button_row(&theme) - theme.padding_px(),
            "no room for buttons"
        );
        assert_eq!(without.w, with.w);
        let lines = to_u32(
            text::wrap(
                CONTENT.message,
                message_width(landscape(&theme).dialog, &theme),
            )
            .count(),
        );
        assert_eq!(
            without.h,
            2 * BORDER_PX + theme.row_height_px() * (1 + lines) + 2 * theme.padding_px()
        );
    }

    #[test]
    fn the_largest_sizes_keep_three_message_lines() {
        let theme = Theme::new(
            Default::default(),
            crate::theme::ROW_HEIGHT_MAX_PX,
            crate::theme::PADDING_MAX_PX,
        )
        .expect("valid");
        assert!(message_width(landscape(&theme).dialog, &theme) >= 30 * font::ADVANCE);
        assert!(max_lines(landscape(&theme).dialog, &theme) >= 3);
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
        let area = Dialog::layout(&CONTENT, &Theme::default(), &Metrics::default());
        assert!(Metrics::default().safe.contains(&area));
        assert_eq!(area.x, 48);
        assert_eq!(area.w, 224);
        assert_eq!(i64::from(area.y) - 12, 228 - area.bottom());
    }

    #[test]
    fn the_message_lines_and_width_follow_the_form() {
        let theme = Theme::default();
        let land = landscape(&theme);
        assert_eq!(max_lines(land.dialog, &theme), 14);
        assert_eq!(font::chars_in(message_width(land.dialog, &theme)), 35);
        let portrait = Metrics::of(Form::Portrait, &theme);
        assert_eq!(max_lines(portrait.dialog, &theme), 20);
        assert_eq!(font::chars_in(message_width(portrait.dialog, &theme)), 34);
        let area = Dialog::layout(&CONTENT, &theme, &portrait);
        assert_eq!((area.x, area.w), (12, 216));
        assert!(portrait.safe.contains(&area));
    }

    #[test]
    fn the_tree_has_title_message_and_buttons() {
        let mut frame = UiFrame::new();
        Dialog::new(1).draw(&mut frame, &Theme::default(), &Metrics::default(), &CONTENT);
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
        let area = Dialog::layout(&CONTENT, &Theme::default(), &Metrics::default());
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
            Dialog::new(0).draw(
                &mut frame,
                &theme,
                &landscape(&theme),
                &DialogContent { tone, ..CONTENT },
            );
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
        Dialog::new(0).draw(&mut frame, &Theme::default(), &Metrics::default(), &content);
        let message = frame.tree.rows().nth(1).expect("message row");
        assert!(message.clipped);
        assert!(Metrics::default().safe.contains(&Dialog::layout(
            &content,
            &Theme::default(),
            &Metrics::default()
        )));
    }
}
