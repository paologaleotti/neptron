//! The shape of the canvas and the numbers of a layout. A screen reads its
//! rectangles and its counts from a [`Metrics`] value, computed once per
//! form, and never from a fixed canvas size.

use crate::font;
use crate::geom::Rect;
use crate::theme::Theme;

/// The shape of the canvas. A closed set: each form has its goldens, and
/// the layout rules are written from sizes, so a later form needs a new
/// value here and no new layout code.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Form {
    /// 320x240: the menu of a screen at 0 or 180 degrees.
    #[default]
    Landscape,
    /// 240x320: the menu of a screen turned on its side.
    Portrait,
}

/// The size of a canvas of one [`Form`]. Only the sizes of a form can be
/// built, so a canvas of any other size does not exist.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Size {
    width: u32,
    height: u32,
}

/// A size that is not the size of a [`Form`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum SizeError {
    /// No form has this size.
    #[error("no canvas form is {width}x{height}: the forms are 320x240 and 240x320")]
    Unknown {
        /// The width given, in pixels.
        width: u32,
        /// The height given, in pixels.
        height: u32,
    },
}

/// The numbers of a layout, computed once from the form and the theme.
/// Every rectangle is in canvas pixels and lies inside the canvas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Metrics {
    /// The form these numbers are for.
    pub form: Form,
    /// The size of the canvas.
    pub canvas: Size,
    /// The overscan-safe area: 5 % of each axis in from each side.
    pub safe: Rect,
    /// The title band at the top of the safe area, one row high.
    pub title_band: Rect,
    /// The hint band at the bottom of the safe area, one row high.
    pub hint_band: Rect,
    /// The area between the title band and the hint band.
    pub content: Rect,
    /// The height of a row, from the theme.
    pub row_px: u32,
    /// The space between a frame and its content, from the theme.
    pub padding_px: u32,
    /// The characters on one line of a framed box across the safe area.
    pub text_chars: u32,
    /// The column of a dialog: its width and horizontal place, and the
    /// vertical room it is centred in.
    pub dialog: Rect,
    /// The characters on one line of a dialog message.
    pub dialog_chars: u32,
    /// The lines of the title box under the game list.
    pub game_title_lines: u32,
    /// The lines of the help box under an option list.
    pub help_lines: u32,
    /// The lines of the video block.
    pub video_lines: u32,
}

/// Height of the line under the title band. Two lines, so that an
/// interlaced mode shows it in both fields and it does not flicker.
pub(crate) const RULE_PX: u32 = 2;
/// Space between the bands and the content, in pixels.
pub(crate) const GAP_PX: u32 = 4;
/// Width of the frame line at the sides of a framed box, in pixels.
pub(crate) const BOX_SIDE_PX: u32 = 1;
/// The widest dialog, in pixels. A narrower safe area gives its whole
/// width to the dialog.
const DIALOG_WIDTH_MAX_PX: u32 = 224;
/// Width of the frame line of a dialog, in pixels.
pub(crate) const DIALOG_BORDER_PX: u32 = 1;

impl Form {
    /// The size of the canvas of this form.
    pub const fn size(self) -> Size {
        match self {
            Self::Landscape => Size {
                width: 320,
                height: 240,
            },
            Self::Portrait => Size {
                width: 240,
                height: 320,
            },
        }
    }

    /// The overscan-safe area of this form: 5 % of each axis in from each
    /// side, as a tube overscans along the axis the player sees.
    pub const fn safe_area(self) -> Rect {
        let size = self.size();
        let margin_x = size.width / 20;
        let margin_y = size.height / 20;
        Rect::new(
            margin_x as i32,
            margin_y as i32,
            size.width - 2 * margin_x,
            size.height - 2 * margin_y,
        )
    }
}

impl Size {
    /// The size `width` x `height`, refused when no form has it.
    pub fn new(width: u32, height: u32) -> Result<Self, SizeError> {
        [Form::Landscape, Form::Portrait]
            .into_iter()
            .map(Form::size)
            .find(|s| s.width == width && s.height == height)
            .ok_or(SizeError::Unknown { width, height })
    }

    /// Width in pixels.
    pub const fn width(self) -> u32 {
        self.width
    }

    /// Height in pixels.
    pub const fn height(self) -> u32 {
        self.height
    }

    /// The form of a canvas of this size.
    pub const fn form(self) -> Form {
        if self.height > self.width {
            Form::Portrait
        } else {
            Form::Landscape
        }
    }

    /// The whole canvas as a rectangle.
    pub const fn rect(self) -> Rect {
        Rect::new(0, 0, self.width, self.height)
    }
}

impl Default for Size {
    fn default() -> Self {
        Form::Landscape.size()
    }
}

impl Metrics {
    /// The metrics of `form` with `theme`. Pure: no I/O, no clock.
    pub fn of(form: Form, theme: &Theme) -> Self {
        let canvas = form.size();
        let safe = form.safe_area();
        let row_px = theme.row_height_px();
        let padding_px = theme.padding_px();
        let title_band = Rect::new(safe.x, safe.y, safe.w, row_px + 4);
        let hint_h = row_px + 2;
        let hint_band = Rect::new(safe.x, at(safe.y, safe.h - hint_h), safe.w, hint_h);
        let top = title_band.h + RULE_PX + GAP_PX;
        let content = Rect::new(
            safe.x,
            at(safe.y, top),
            safe.w,
            safe.h - top - hint_h - GAP_PX,
        );
        let box_text_px = safe
            .w
            .saturating_sub(2 * (BOX_SIDE_PX + 1) + 2 * padding_px);
        let dialog_w = safe.w.min(DIALOG_WIDTH_MAX_PX);
        let dialog = Rect::new(
            at(safe.x, (safe.w - dialog_w) / 2),
            safe.y,
            dialog_w,
            safe.h,
        );
        let dialog_text_px = dialog_w.saturating_sub(2 * DIALOG_BORDER_PX + 2 * padding_px);
        let (game_title_lines, help_lines, video_lines) = match form {
            Form::Landscape => (2, 2, 5),
            Form::Portrait => (3, 4, 7),
        };
        Self {
            form,
            canvas,
            safe,
            title_band,
            hint_band,
            content,
            row_px,
            padding_px,
            text_chars: chars(box_text_px),
            dialog,
            dialog_chars: chars(dialog_text_px),
            game_title_lines,
            help_lines,
            video_lines,
        }
    }
}

impl Default for Metrics {
    fn default() -> Self {
        Self::of(Form::Landscape, &Theme::default())
    }
}

/// `y` moved by `by` pixels. The safe area of a form is a few hundred
/// pixels, so the sum fits an `i32`.
const fn at(y: i32, by: u32) -> i32 {
    y + by as i32
}

fn chars(width_px: u32) -> u32 {
    u32::try_from(font::chars_in(width_px)).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::{PADDING_MAX_PX, Palette, ROW_HEIGHT_MAX_PX};

    #[test]
    fn the_forms_have_their_sizes_and_no_other_size_exists() {
        assert_eq!(Size::new(320, 240), Ok(Form::Landscape.size()));
        assert_eq!(Size::new(240, 320), Ok(Form::Portrait.size()));
        assert_eq!(Form::Landscape.size().form(), Form::Landscape);
        assert_eq!(Form::Portrait.size().form(), Form::Portrait);
        for (width, height) in [(0, 0), (320, 320), (640, 480), (241, 320), (320, 239)] {
            assert_eq!(
                Size::new(width, height),
                Err(SizeError::Unknown { width, height })
            );
        }
        assert_eq!(Size::default(), Form::Landscape.size());
        assert_eq!(Form::default(), Form::Landscape);
    }

    #[test]
    fn the_landscape_metrics_are_the_layout_of_the_fixed_canvas() {
        let m = Metrics::of(Form::Landscape, &Theme::default());
        assert_eq!((m.canvas.width(), m.canvas.height()), (320, 240));
        assert_eq!(m.canvas.rect(), Rect::new(0, 0, 320, 240));
        assert_eq!(m.safe, Rect::new(16, 12, 288, 216));
        assert_eq!(m.title_band, Rect::new(16, 12, 288, 16));
        assert_eq!(m.hint_band, Rect::new(16, 214, 288, 14));
        assert_eq!(m.content, Rect::new(16, 34, 288, 176));
        assert_eq!((m.row_px, m.padding_px), (12, 4));
        assert_eq!(m.text_chars, 46);
        assert_eq!(m.dialog, Rect::new(48, 12, 224, 216));
        assert_eq!(m.dialog_chars, 35);
        assert_eq!(m.game_title_lines, 2);
        assert_eq!(m.help_lines, 2);
        assert_eq!(m.video_lines, 5);
        assert_eq!((m.content.h - 6) / m.row_px, 14);
        assert_eq!(Metrics::default(), m);
    }

    #[test]
    fn the_landscape_metrics_equal_the_fixed_constants() {
        use crate::canvas::{UI_HEIGHT, UI_WIDTH};
        use crate::theme::SAFE_AREA;
        const DIALOG_WIDTH_PX: u32 = 224;
        let m = Metrics::of(Form::Landscape, &Theme::default());
        assert_eq!(m.canvas.width(), UI_WIDTH);
        assert_eq!(m.canvas.height(), UI_HEIGHT);
        assert_eq!(m.safe, SAFE_AREA);
        assert_eq!(m.safe.x, (UI_WIDTH / 20) as i32);
        assert_eq!(m.safe.y, (UI_HEIGHT / 20) as i32);
        assert_eq!(m.dialog.w, DIALOG_WIDTH_PX);
        assert_eq!(
            m.dialog.x,
            SAFE_AREA.x + ((SAFE_AREA.w - DIALOG_WIDTH_PX) / 2) as i32
        );
    }

    #[test]
    fn the_portrait_metrics_follow_the_same_rules() {
        let m = Metrics::of(Form::Portrait, &Theme::default());
        assert_eq!((m.canvas.width(), m.canvas.height()), (240, 320));
        assert_eq!(m.safe, Rect::new(12, 16, 216, 288));
        assert_eq!(m.title_band, Rect::new(12, 16, 216, 16));
        assert_eq!(m.hint_band, Rect::new(12, 290, 216, 14));
        assert_eq!(m.content, Rect::new(12, 38, 216, 248));
        assert_eq!(m.text_chars, 34);
        assert_eq!(m.dialog, Rect::new(12, 16, 216, 288));
        assert_eq!(m.dialog_chars, 34);
        assert_eq!(m.game_title_lines, 3);
        assert_eq!(m.help_lines, 4);
        assert_eq!(m.video_lines, 7);
        assert_eq!((m.content.h - 6) / m.row_px, 20);
    }

    #[test]
    fn every_rectangle_is_inside_the_safe_area_in_both_forms() {
        let largest = Theme::new(Palette::default(), ROW_HEIGHT_MAX_PX, PADDING_MAX_PX);
        for theme in [
            Theme::default(),
            largest.expect("the largest sizes are inside the limits"),
        ] {
            for form in [Form::Landscape, Form::Portrait] {
                let m = Metrics::of(form, &theme);
                assert_eq!(m.form, form);
                assert!(m.canvas.rect().contains(&m.safe), "{form:?}");
                for r in [m.title_band, m.hint_band, m.content, m.dialog] {
                    assert!(m.safe.contains(&r), "{form:?}: {r:?}");
                    assert!(!r.is_empty(), "{form:?}: {r:?}");
                }
                assert!(m.title_band.bottom() < i64::from(m.content.y));
                assert!(m.content.bottom() < i64::from(m.hint_band.y));
                assert!(m.dialog_chars >= 30, "{form:?}: {}", m.dialog_chars);
            }
        }
    }
}
