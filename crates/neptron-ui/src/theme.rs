//! Colours and template parameters. A skin fills these values through
//! [`Theme::new`], which refuses sizes outside the limits; the built-in
//! values are [`Theme::default`].

use crate::canvas::{UI_HEIGHT, UI_WIDTH};
use crate::font;
use crate::geom::{Color, Rect};

/// Horizontal overscan margin: 5% of the canvas width, in pixels.
pub const SAFE_MARGIN_X_PX: u32 = UI_WIDTH / 20;
/// Vertical overscan margin: 5% of the canvas height, in pixels.
pub const SAFE_MARGIN_Y_PX: u32 = UI_HEIGHT / 20;

/// The area that every screen keeps its content in, so that a CRT that
/// overscans shows all of it.
pub const SAFE_AREA: Rect = Rect::new(
    SAFE_MARGIN_X_PX as i32,
    SAFE_MARGIN_Y_PX as i32,
    UI_WIDTH - 2 * SAFE_MARGIN_X_PX,
    UI_HEIGHT - 2 * SAFE_MARGIN_Y_PX,
);

/// The smallest row height: one glyph box, so text is never cut.
pub const ROW_HEIGHT_MIN_PX: u32 = font::GLYPH_HEIGHT;
/// The largest row height: two glyph boxes. With it and the largest
/// padding, a dialog still shows three lines of message.
pub const ROW_HEIGHT_MAX_PX: u32 = 2 * font::GLYPH_HEIGHT;
/// The largest padding: with it, a dialog still has room for 30 characters
/// on a line.
pub const PADDING_MAX_PX: u32 = 16;

/// The colours of the templates. Every colour is valid.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Palette {
    /// Screen background.
    pub background: Color,
    /// Background of panels and dialogs.
    pub panel: Color,
    /// Frame lines of panels, dialogs and tracks.
    pub border: Color,
    /// Normal text.
    pub text: Color,
    /// Secondary text: details, sources, disabled rows.
    pub text_dim: Color,
    /// Background of the focused row, cell or button.
    pub focus: Color,
    /// Text on the focus background.
    pub focus_text: Color,
    /// Values, slider fill and title bars.
    pub accent: Color,
    /// Text that the player must notice, for example a video result that
    /// changes the picture or the speed, and the title bar of a warning.
    /// It differs from `focus`, so a warning never reads as a focused row.
    pub warning: Color,
    /// Text and title bars of an error: something failed.
    pub error: Color,
}

/// The colours and sizes that the widget templates use. The sizes are
/// inside the limits of this module, so layout maths on them cannot
/// overflow.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Theme {
    /// The colours.
    pub colors: Palette,
    row_height_px: u32,
    padding_px: u32,
}

/// A theme value outside its limits.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ThemeError {
    /// The row height is outside its limits.
    #[error("row height {value} px is outside {min}..={max} px")]
    RowHeight {
        /// The value given.
        value: u32,
        /// The smallest value.
        min: u32,
        /// The largest value.
        max: u32,
    },
    /// The padding is too large.
    #[error("padding {value} px is above the limit of {max} px")]
    Padding {
        /// The value given.
        value: u32,
        /// The largest value.
        max: u32,
    },
}

impl Theme {
    /// A theme from a palette and sizes, refused with the value and its
    /// limit when a size is outside its limits.
    pub fn new(colors: Palette, row_height_px: u32, padding_px: u32) -> Result<Self, ThemeError> {
        if !(ROW_HEIGHT_MIN_PX..=ROW_HEIGHT_MAX_PX).contains(&row_height_px) {
            return Err(ThemeError::RowHeight {
                value: row_height_px,
                min: ROW_HEIGHT_MIN_PX,
                max: ROW_HEIGHT_MAX_PX,
            });
        }
        if padding_px > PADDING_MAX_PX {
            return Err(ThemeError::Padding {
                value: padding_px,
                max: PADDING_MAX_PX,
            });
        }
        Ok(Self {
            colors,
            row_height_px,
            padding_px,
        })
    }

    /// Height of a list row in pixels.
    pub fn row_height_px(&self) -> u32 {
        self.row_height_px
    }

    /// Space between a frame and its content, in pixels.
    pub fn padding_px(&self) -> u32 {
        self.padding_px
    }
}

impl Default for Palette {
    fn default() -> Self {
        Self {
            background: Color::rgb(0x10, 0x14, 0x28),
            panel: Color::rgb(0x20, 0x28, 0x48),
            border: Color::rgb(0x60, 0x70, 0xA0),
            text: Color::rgb(0xE8, 0xE8, 0xE8),
            text_dim: Color::rgb(0x90, 0x98, 0xB0),
            focus: Color::rgb(0xF0, 0xC0, 0x30),
            focus_text: Color::rgb(0x10, 0x14, 0x28),
            accent: Color::rgb(0x50, 0xC8, 0xF0),
            warning: Color::rgb(0xFF, 0x80, 0x20),
            error: Color::rgb(0xFF, 0x60, 0x60),
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            colors: Palette::default(),
            row_height_px: 12,
            padding_px: 4,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skin_values_that_would_overflow_the_layout_are_refused() {
        let p = Palette::default();
        assert!(Theme::new(p.clone(), 12, 111).is_err());
        assert!(Theme::new(p.clone(), u32::MAX, 0).is_err());
        assert!(Theme::new(p, 0, u32::MAX).is_err());
    }

    #[test]
    fn safe_area_is_five_percent_in() {
        assert_eq!(SAFE_AREA, Rect::new(16, 12, 288, 216));
    }

    #[test]
    fn the_default_theme_is_inside_the_limits() {
        let d = Theme::default();
        let checked = Theme::new(d.colors.clone(), d.row_height_px(), d.padding_px());
        assert_eq!(checked, Ok(d));
    }

    #[test]
    fn sizes_outside_the_limits_are_refused() {
        let p = Palette::default();
        assert_eq!(
            Theme::new(p.clone(), 11, 4),
            Err(ThemeError::RowHeight {
                value: 11,
                min: 12,
                max: 24
            })
        );
        assert!(matches!(
            Theme::new(p.clone(), 25, 4),
            Err(ThemeError::RowHeight { .. })
        ));
        assert!(Theme::new(p.clone(), 24, 16).is_ok());
        assert_eq!(
            Theme::new(p, 12, 17),
            Err(ThemeError::Padding { value: 17, max: 16 })
        );
    }
}
