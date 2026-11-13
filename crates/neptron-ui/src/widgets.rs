//! The widget templates that every screen is built from: list, settings
//! list, dialog, grid and slider.
//!
//! A template holds only its navigation state (cursor, scroll, focus). The
//! data comes from the snapshot at each frame. `handle` turns one action
//! into an event; `draw` adds commands to the scene and rows to the tree.
//! A template that sets a clip removes it again before it returns.

mod dialog;
mod grid;
mod list;
mod settings_list;
mod slider;

pub use dialog::{DIALOG_WIDTH_PX, Dialog, DialogContent, DialogEvent, DialogTone};
pub use grid::{Grid, GridCell, GridEvent, MAX_COLUMNS};
pub use list::{List, ListEvent, ListItem, MAX_VISIBLE_ROWS, SCROLLBAR_WIDTH_PX};
pub use settings_list::{
    HINT_CHANGE, HINT_CHOOSE, HINT_OPEN, HINT_STEP, SettingsEvent, SettingsList, source_label,
};
pub use slider::{Slider, SliderEvent, SliderValue};

use crate::geom::{Color, Rect};
use crate::scene::UiScene;

/// Width of the column at the right edge of a row that holds the arrow of
/// a row that opens a page, in pixels: the arrow and a gap before it.
pub const ARROW_COLUMN_PX: u32 = 8;
/// Width of the arrow, in pixels.
pub const ARROW_WIDTH_PX: u32 = 4;
/// Height of the arrow, in pixels: each column to the right is two pixels
/// shorter, so the tip is one pixel.
pub const ARROW_HEIGHT_PX: u32 = 2 * ARROW_WIDTH_PX - 1;

/// The next value of an integer setting after one press: `direction` is
/// `-1` or `1`. A step of zero or less counts as 1. The result stays in
/// `min..=max`.
pub fn step_value(value: i64, min: i64, max: i64, step: i64, direction: i64) -> i64 {
    let step = if step <= 0 { 1 } else { step };
    let next = value.saturating_add(step.saturating_mul(direction.signum()));
    if min > max {
        return value;
    }
    next.clamp(min, max)
}

/// Draws the arrow of a row that opens a page at the right edge of `inner`,
/// the row without its padding, centred on the row. The font has `>`, but a
/// `>` at the edge reads as the end of `< value >`, so the arrow is a
/// triangle of rectangles.
fn draw_arrow(scene: &mut UiScene, inner: Rect, color: Color) {
    let x = add(inner.x, inner.w.saturating_sub(ARROW_WIDTH_PX));
    let top = add(inner.y, inner.h.saturating_sub(ARROW_HEIGHT_PX) / 2);
    for column in 0..ARROW_WIDTH_PX {
        let rect = Rect::new(
            add(x, column),
            add(top, column),
            1,
            ARROW_HEIGHT_PX - 2 * column,
        );
        scene.rect(rect, color);
    }
}

fn framed(scene: &mut UiScene, rect: Rect, frame: Color, fill: Color, width_px: u32) {
    scene.rect(rect, frame);
    scene.rect(rect.inset(width_px), fill);
}

fn add(x: i32, by: u32) -> i32 {
    i32::try_from(i64::from(x) + i64::from(by)).unwrap_or(i32::MAX)
}

fn to_u32(v: usize) -> u32 {
    u32::try_from(v).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_stay_in_range() {
        assert_eq!(step_value(5, 0, 10, 2, 1), 7);
        assert_eq!(step_value(9, 0, 10, 2, 1), 10);
        assert_eq!(step_value(1, 0, 10, 2, -1), 0);
        assert_eq!(step_value(5, 0, 10, 0, 1), 6);
        assert_eq!(step_value(5, 0, 10, -3, -1), 4);
        assert_eq!(
            step_value(i64::MAX, i64::MIN, i64::MAX, i64::MAX, 1),
            i64::MAX
        );
        assert_eq!(step_value(20, 0, 10, 1, -1), 10);
    }

    #[test]
    fn an_empty_range_keeps_the_value() {
        assert_eq!(step_value(5, 10, 0, 1, 1), 5);
    }

    mod fuzz {
        use crate::canvas::{CpuCanvas, SpriteId};
        use crate::frame::UiFrame;
        use crate::geom::Rect;
        use crate::input::UiAction;
        use crate::snapshot::{ActionId, Level, MenuItem, Scopes, SettingKey, SettingValue};
        use crate::theme::{PADDING_MAX_PX, Palette, ROW_HEIGHT_MAX_PX, ROW_HEIGHT_MIN_PX, Theme};
        use crate::widgets::{
            Dialog, DialogContent, DialogTone, Grid, GridCell, List, ListItem, SettingsList,
            Slider, SliderValue,
        };

        const SEED: u64 = 0x5EED_0007_C0FF_EE00;
        const ROUNDS: usize = 400;
        const ACTIONS: [UiAction; 7] = [
            UiAction::Up,
            UiAction::Down,
            UiAction::Left,
            UiAction::Right,
            UiAction::Accept,
            UiAction::Back,
            UiAction::Menu,
        ];

        struct Rng(u64);

        impl Rng {
            fn next(&mut self) -> u64 {
                self.0 ^= self.0 << 13;
                self.0 ^= self.0 >> 7;
                self.0 ^= self.0 << 17;
                self.0
            }
            fn below(&mut self, n: u64) -> u64 {
                self.next() % n.max(1)
            }
            fn u32(&mut self) -> u32 {
                let edges = [
                    0,
                    1,
                    2,
                    11,
                    12,
                    240,
                    320,
                    u32::MAX / 2,
                    u32::MAX - 1,
                    u32::MAX,
                ];
                match self.below(3) {
                    0 => edges[self.below(edges.len() as u64) as usize],
                    1 => self.below(400) as u32,
                    _ => self.next() as u32,
                }
            }
            fn i32(&mut self) -> i32 {
                let edges = [
                    i32::MIN,
                    i32::MIN + 1,
                    -1000,
                    -1,
                    0,
                    1,
                    16,
                    300,
                    i32::MAX - 1,
                    i32::MAX,
                ];
                match self.below(3) {
                    0 => edges[self.below(edges.len() as u64) as usize],
                    1 => self.below(400) as i32 - 50,
                    _ => self.next() as i32,
                }
            }
            fn i64(&mut self) -> i64 {
                let edges = [
                    i64::MIN,
                    i64::MIN + 1,
                    -1,
                    0,
                    1,
                    100,
                    i64::MAX - 1,
                    i64::MAX,
                ];
                match self.below(2) {
                    0 => edges[self.below(edges.len() as u64) as usize],
                    _ => self.next() as i64,
                }
            }
            fn count(&mut self) -> usize {
                let edges = [0, 1, 2, 27, 28, 29, 1000, usize::MAX - 1, usize::MAX];
                match self.below(2) {
                    0 => edges[self.below(edges.len() as u64) as usize],
                    _ => self.below(60) as usize,
                }
            }
            fn rect(&mut self) -> Rect {
                Rect::new(self.i32(), self.i32(), self.u32(), self.u32())
            }
            fn theme(&mut self) -> Theme {
                let span = u64::from(ROW_HEIGHT_MAX_PX - ROW_HEIGHT_MIN_PX + 1);
                let row = ROW_HEIGHT_MIN_PX + self.below(span) as u32;
                let padding = self.below(u64::from(PADDING_MAX_PX) + 1) as u32;
                Theme::new(Palette::default(), row, padding).expect("inside the limits")
            }
            fn action(&mut self) -> UiAction {
                ACTIONS[self.below(ACTIONS.len() as u64) as usize]
            }
            fn setting(&mut self) -> SettingValue {
                match self.below(4) {
                    0 => SettingValue::Toggle(self.below(2) == 0),
                    1 => SettingValue::Number {
                        value: self.i64(),
                        min: self.i64(),
                        max: self.i64(),
                        step: self.i64(),
                        unit: "unit text that is long".into(),
                    },
                    2 => SettingValue::Choice {
                        index: self.count(),
                        options: vec!["A".into(); self.below(4) as usize],
                    },
                    _ => SettingValue::Action(ActionId(1)),
                }
            }
        }

        fn render(frame: &UiFrame) {
            let mut canvas = CpuCanvas::new();
            frame.render(&mut canvas);
        }

        #[test]
        fn random_layouts_never_panic() {
            let mut rng = Rng(SEED);
            for _ in 0..ROUNDS {
                let theme = rng.theme();
                let mut frame = UiFrame::new();

                let len = rng.count();
                let mut list = List::new(rng.rect(), rng.u32());
                for _ in 0..4 {
                    list.handle(rng.action(), len);
                }
                list.draw(&mut frame, &theme, len, |_| ListItem {
                    label: "a label that is long enough to be cut",
                    detail: Some("detail text"),
                    enabled: true,
                    kind: crate::tree::RowKind::Page,
                });

                let items: Vec<MenuItem> = (0..rng.below(6))
                    .map(|k| MenuItem {
                        key: SettingKey(k as u32),
                        label: "label".into(),
                        value: rng.setting(),
                        source: Level::Game,
                        scopes: Scopes::ALL,
                        live: true,
                        unavailable: None,
                    })
                    .collect();
                let mut settings = SettingsList::new(rng.rect(), rng.u32());
                settings.select(rng.count(), items.len());
                for _ in 0..4 {
                    settings.handle(rng.action(), &items, Level::Game);
                }
                settings.draw(&mut frame, &theme, &items);

                let buttons = vec!["Button"; rng.below(40) as usize];
                let message = "word ".repeat(rng.below(300) as usize);
                let mut dialog = Dialog::new(rng.count());
                dialog.handle(rng.action(), buttons.len());
                let content = DialogContent {
                    title: "A title",
                    message: &message,
                    buttons: &buttons,
                    tone: DialogTone::Normal,
                };
                dialog.draw(&mut frame, &theme, &content);

                let len = rng.count();
                let mut grid = Grid::new(rng.rect(), rng.count(), rng.u32());
                for _ in 0..4 {
                    grid.handle(rng.action(), len);
                }
                grid.draw(&mut frame, &theme, len, |_| GridCell {
                    label: "cell",
                    sprite: Some(SpriteId(0)),
                });

                let value = SliderValue {
                    value: rng.i64(),
                    min: rng.i64(),
                    max: rng.i64(),
                    step: rng.i64(),
                };
                let slider = Slider::new(rng.rect());
                slider.handle(rng.action(), value);
                slider.draw(&mut frame, &theme, "label", value, "unit", true);

                render(&frame);
            }
        }
    }
}
