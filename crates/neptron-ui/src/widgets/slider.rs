use core::fmt::Write;

use crate::frame::UiFrame;
use crate::geom::Rect;
use crate::input::UiAction;
use crate::text::{self, Align, StackText};
use crate::theme::Theme;
use crate::tree::{TreeRow, WidgetKind};

use super::{add, framed, step_value};

const TRACK_HEIGHT_PX: u32 = 8;
const TRACK_GAP_PX: u32 = 2;

/// The value of a [`Slider`] and its range.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SliderValue {
    /// The value.
    pub value: i64,
    /// The smallest value.
    pub min: i64,
    /// The largest value.
    pub max: i64,
    /// The change for one press. Zero or less means 1.
    pub step: i64,
}

/// What one action did to a [`Slider`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SliderEvent {
    /// Nothing changed.
    None,
    /// The player moved the value to this one.
    Changed(i64),
    /// The player pressed accept.
    Accepted,
    /// The player pressed back.
    Back,
}

/// A label, the value as text, and a horizontal track filled up to the
/// value. Left and right step the value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Slider {
    rect: Rect,
}

impl SliderValue {
    /// The filled part of a track `width_px` wide, rounded down. A value
    /// outside the range fills nothing or everything.
    pub fn fill_px(&self, width_px: u32) -> u32 {
        if self.max <= self.min {
            return 0;
        }
        let span = i128::from(self.max) - i128::from(self.min);
        let part = (i128::from(self.value) - i128::from(self.min)).clamp(0, span);
        let fill = part * i128::from(width_px) / span;
        u32::try_from(fill).unwrap_or(width_px)
    }
}

impl Slider {
    /// A slider in `rect`. [`Slider::height_px`] is the height it needs.
    pub fn new(rect: Rect) -> Self {
        Self { rect }
    }

    /// The height a slider needs with `theme`: a label row, the track and a
    /// gap above and below the track.
    pub fn height_px(theme: &Theme) -> u32 {
        theme.row_height_px() + TRACK_GAP_PX + TRACK_HEIGHT_PX + TRACK_GAP_PX
    }

    /// Applies one action.
    pub fn handle(&self, action: UiAction, value: SliderValue) -> SliderEvent {
        let direction = match action {
            UiAction::Left => -1,
            UiAction::Right => 1,
            UiAction::Accept => return SliderEvent::Accepted,
            UiAction::Back => return SliderEvent::Back,
            _ => return SliderEvent::None,
        };
        let next = step_value(value.value, value.min, value.max, value.step, direction);
        if next == value.value {
            SliderEvent::None
        } else {
            SliderEvent::Changed(next)
        }
    }

    /// Draws the slider. `unit` follows the value, for example `%`.
    pub fn draw(
        &self,
        frame: &mut UiFrame,
        theme: &Theme,
        label: &str,
        value: SliderValue,
        unit: &str,
        focused: bool,
    ) {
        let row = Rect::new(self.rect.x, self.rect.y, self.rect.w, theme.row_height_px());
        let color = if focused {
            theme.colors.focus
        } else {
            theme.colors.text
        };
        let mut shown = StackText::<32>::new();
        // `StackText` never returns an error; it drops what does not fit.
        let _ = if unit.is_empty() {
            write!(shown, "{}", value.value)
        } else {
            write!(shown, "{} {unit}", value.value)
        };
        let value_w = crate::font::text_width(shown.as_str()).min(row.w / 2);
        let value_box = Rect::new(add(row.x, row.w - value_w), row.y, value_w, row.h);
        let value_clipped = text::draw_in(
            &mut frame.scene,
            value_box,
            Align::Right,
            color,
            shown.as_str(),
        );
        let label_box = Rect::new(
            row.x,
            row.y,
            row.w
                .saturating_sub(value_w.saturating_add(crate::font::ADVANCE)),
            row.h,
        );
        let label_clipped = text::draw_in(&mut frame.scene, label_box, Align::Left, color, label);

        let track_y = add(self.rect.y, theme.row_height_px() + TRACK_GAP_PX);
        let track = Rect::new(self.rect.x, track_y, self.rect.w, TRACK_HEIGHT_PX);
        let track_frame = if focused {
            theme.colors.focus
        } else {
            theme.colors.border
        };
        framed(&mut frame.scene, track, track_frame, theme.colors.panel, 1);
        let inner = track.inset(1);
        frame.scene.rect(
            Rect::new(inner.x, inner.y, value.fill_px(inner.w), inner.h),
            theme.colors.accent,
        );

        frame.tree.push(TreeRow {
            widget: WidgetKind::Slider,
            kind: None,
            label,
            value: Some(shown.as_str()),
            source: None,
            rect: Rect::new(
                self.rect.x,
                self.rect.y,
                self.rect.w,
                Self::height_px(theme),
            ),
            focused,
            clipped: label_clipped || value_clipped,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_height_follows_the_theme() {
        assert_eq!(Slider::height_px(&Theme::default()), 24);
        let tall = Theme::new(Default::default(), 24, 0).expect("valid");
        assert_eq!(Slider::height_px(&tall), 36);
    }

    const VOLUME: SliderValue = SliderValue {
        value: 30,
        min: 0,
        max: 100,
        step: 10,
    };

    #[test]
    fn left_and_right_step_inside_the_range() {
        let s = Slider::new(Rect::new(0, 0, 100, 24));
        assert_eq!(s.handle(UiAction::Right, VOLUME), SliderEvent::Changed(40));
        assert_eq!(s.handle(UiAction::Left, VOLUME), SliderEvent::Changed(20));
        let full = SliderValue {
            value: 100,
            ..VOLUME
        };
        assert_eq!(s.handle(UiAction::Right, full), SliderEvent::None);
        assert_eq!(s.handle(UiAction::Accept, VOLUME), SliderEvent::Accepted);
        assert_eq!(s.handle(UiAction::Back, VOLUME), SliderEvent::Back);
        assert_eq!(s.handle(UiAction::Up, VOLUME), SliderEvent::None);
    }

    #[test]
    fn fill_uses_integer_maths() {
        assert_eq!(VOLUME.fill_px(98), 29);
        assert_eq!(
            SliderValue {
                value: 100,
                ..VOLUME
            }
            .fill_px(98),
            98
        );
        assert_eq!(
            SliderValue {
                value: -5,
                ..VOLUME
            }
            .fill_px(98),
            0
        );
        assert_eq!(
            SliderValue {
                value: 500,
                ..VOLUME
            }
            .fill_px(98),
            98
        );
        assert_eq!(
            SliderValue {
                min: 5,
                max: 5,
                ..VOLUME
            }
            .fill_px(98),
            0
        );
        let wide = SliderValue {
            value: 0,
            min: i64::MIN,
            max: i64::MAX,
            step: 1,
        };
        assert_eq!(wide.fill_px(u32::MAX), u32::MAX / 2);
    }

    #[test]
    fn the_tree_shows_the_value_with_its_unit() {
        let mut frame = UiFrame::new();
        Slider::new(Rect::new(16, 100, 200, 24)).draw(
            &mut frame,
            &Theme::default(),
            "Volume",
            VOLUME,
            "%",
            true,
        );
        let row = frame.tree.focused().expect("a focused slider");
        assert_eq!(
            (row.label, row.value, row.clipped),
            ("Volume", Some("30 %"), false)
        );
        assert_eq!(row.rect, Rect::new(16, 100, 200, 24));
    }
}
