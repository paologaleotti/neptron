use crate::font;
use crate::frame::UiFrame;
use crate::geom::Rect;
use crate::input::UiAction;
use crate::intent::{Intent, Value};
use crate::snapshot::{Level, MenuItem, SettingValue};
use crate::text::{self, Align, StackText};
use crate::theme::Theme;
use crate::tree::{TreeRow, WidgetKind};

use super::list::{List, ListEvent};
use super::{add, step_value};

/// What one action did to a [`SettingsList`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingsEvent {
    /// Nothing changed.
    None,
    /// The cursor moved.
    Moved,
    /// The player pressed back.
    Back,
    /// The player asked for a change; the app carries it out.
    Intent(Intent),
}

/// A list of settings rows: label, value and the level the value comes from.
///
/// Up and down move. Left and right step the value. Accept toggles a switch,
/// moves a choice to the next option, or runs an action. Menu resets the
/// value at the edit level, so the row inherits again.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SettingsList {
    list: List,
}

/// The short name of a level, as the source column shows it.
pub fn source_label(level: Level) -> &'static str {
    match level {
        Level::Default => "default",
        Level::Global => "global",
        Level::System => "system",
        Level::Game => "game",
    }
}

const VALUE_CHARS: usize = 14;
const SOURCE_CHARS: usize = 7;

impl SettingsList {
    /// A settings list in `rect` with rows of `row_height_px`.
    pub fn new(rect: Rect, row_height_px: u32) -> Self {
        Self {
            list: List::new(rect, row_height_px),
        }
    }

    /// The focused row.
    pub fn cursor(&self) -> usize {
        self.list.cursor()
    }

    /// Moves the cursor to `index`.
    pub fn select(&mut self, index: usize, len: usize) {
        self.list.select(index, len);
    }

    /// Applies one action. A change writes at `level`; a row whose scopes do
    /// not allow `level` does not change.
    pub fn handle(&mut self, action: UiAction, items: &[MenuItem], level: Level) -> SettingsEvent {
        match action {
            UiAction::Up | UiAction::Down => {
                return match self.list.handle(action, items.len()) {
                    ListEvent::Moved => SettingsEvent::Moved,
                    _ => SettingsEvent::None,
                };
            }
            UiAction::Back => return SettingsEvent::Back,
            _ => {}
        }
        self.list.select(self.list.cursor(), items.len());
        let Some(item) = items.get(self.list.cursor()) else {
            return SettingsEvent::None;
        };
        intent_for(action, item, level).map_or(SettingsEvent::None, SettingsEvent::Intent)
    }

    /// Draws the visible rows.
    pub fn draw(&self, frame: &mut UiFrame, theme: &Theme, items: &[MenuItem]) {
        let len = items.len();
        let (cursor, top) = self.list.window(len);
        let scrolls = self.list.scrolls(len);
        let rows = self.list.visible_rows();
        for (slot, (index, item)) in items.iter().enumerate().skip(top).take(rows).enumerate() {
            let row = self.list.row_rect(slot, scrolls);
            draw_row(frame, theme, row, item, index == cursor);
        }
        if scrolls {
            self.list.draw_scrollbar(frame, theme, len, top);
        }
    }
}

fn draw_row(frame: &mut UiFrame, theme: &Theme, row: Rect, item: &MenuItem, focused: bool) {
    let (color, dim) = if focused {
        frame.scene.rect(row, theme.colors.focus);
        (theme.colors.focus_text, theme.colors.focus_text)
    } else {
        (theme.colors.text, theme.colors.text_dim)
    };
    let inner = Rect::new(
        add(row.x, theme.padding_px()),
        row.y,
        row.w.saturating_sub(2 * theme.padding_px()),
        row.h,
    );
    if let SettingValue::Action(_) = item.value {
        let clipped = text::draw_in(&mut frame.scene, inner, Align::Left, color, &item.label);
        push(frame, item, row, None, focused, clipped);
        return;
    }
    let advance = font::ADVANCE;
    let source_w = font::char_count_width(SOURCE_CHARS);
    let value_w = font::char_count_width(VALUE_CHARS);
    let arrows_w = 2 * advance;
    let columns_w = source_w + advance + value_w + arrows_w + advance;
    let source_x = add(inner.x, inner.w.saturating_sub(source_w));
    let source_box = Rect::new(source_x, row.y, source_w, row.h);
    let source_color = if item.source == Level::Default {
        dim
    } else if focused {
        color
    } else {
        theme.colors.accent
    };
    text::draw_in(
        &mut frame.scene,
        source_box,
        Align::Right,
        source_color,
        source_label(item.source),
    );

    let value_x = add(inner.x, inner.w.saturating_sub(columns_w - advance));
    let value_box = Rect::new(add(value_x, advance), row.y, value_w, row.h);
    let mut buf = StackText::<32>::new();
    let value = text::value_text(&item.value, &mut buf);
    let value_clipped = text::draw_in(&mut frame.scene, value_box, Align::Center, color, value);
    if focused {
        let left = Rect::new(value_x, row.y, advance, row.h);
        let right = Rect::new(add(value_x, advance + value_w), row.y, advance, row.h);
        text::draw_in(&mut frame.scene, left, Align::Left, color, "<");
        text::draw_in(&mut frame.scene, right, Align::Left, color, ">");
    }

    let label_box = Rect::new(inner.x, row.y, inner.w.saturating_sub(columns_w), row.h);
    let label_clipped = text::draw_in(&mut frame.scene, label_box, Align::Left, color, &item.label);
    push(
        frame,
        item,
        row,
        Some(value),
        focused,
        label_clipped || value_clipped,
    );
}

fn push(
    frame: &mut UiFrame,
    item: &MenuItem,
    rect: Rect,
    value: Option<&str>,
    focused: bool,
    clipped: bool,
) {
    let is_action = matches!(item.value, SettingValue::Action(_));
    frame.tree.push(TreeRow {
        widget: WidgetKind::SettingsList,
        label: &item.label,
        value,
        source: (!is_action).then_some(item.source),
        rect,
        focused,
        clipped,
    });
}

fn intent_for(action: UiAction, item: &MenuItem, level: Level) -> Option<Intent> {
    if let SettingValue::Action(id) = item.value {
        return (action == UiAction::Accept).then_some(Intent::Action(id));
    }
    if !item.scopes.allows(level) {
        return None;
    }
    if action == UiAction::Menu {
        return (item.source == level).then_some(Intent::ResetValue {
            key: item.key,
            level,
        });
    }
    let value = next_value(action, &item.value)?;
    Some(Intent::SetValue {
        key: item.key,
        level,
        value,
    })
}

fn next_value(action: UiAction, value: &SettingValue) -> Option<Value> {
    let direction = match action {
        UiAction::Left => -1,
        UiAction::Right => 1,
        UiAction::Accept => 0,
        _ => return None,
    };
    match value {
        SettingValue::Toggle(on) => Some(Value::Toggle(!on)),
        SettingValue::Number {
            value,
            min,
            max,
            step,
            ..
        } => {
            if direction == 0 {
                return None;
            }
            let next = step_value(*value, *min, *max, *step, direction);
            (next != *value).then_some(Value::Number(next))
        }
        SettingValue::Choice { index, options } => {
            let len = options.len();
            let next = match direction {
                0 if len > 0 => index.checked_add(1).filter(|n| *n < len).unwrap_or(0),
                -1 => index.checked_sub(1)?,
                1 => index.checked_add(1)?,
                _ => return None,
            };
            (next < len && next != *index).then_some(Value::Choice(next))
        }
        SettingValue::Action(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_choice_index_at_the_limit_does_not_overflow() {
        let mut l = SettingsList::new(Rect::new(0, 0, 288, 48), 12);
        let mut items = items();
        items[2].value = SettingValue::Choice {
            index: usize::MAX,
            options: vec!["A".into(), "B".into()],
        };
        at(&mut l, 2);
        assert_eq!(
            l.handle(UiAction::Right, &items, Level::Game),
            SettingsEvent::None
        );
        assert_eq!(
            l.handle(UiAction::Left, &items, Level::Game),
            SettingsEvent::None
        );
        assert_eq!(
            l.handle(UiAction::Accept, &items, Level::Game),
            set(3, Value::Choice(0))
        );
    }
    use crate::snapshot::{ActionId, Scopes, SettingKey};

    const ALL: Scopes = Scopes {
        global: true,
        system: true,
        game: true,
    };

    fn item(key: u32, value: SettingValue, source: Level) -> MenuItem {
        MenuItem {
            key: SettingKey(key),
            label: format!("Setting {key}"),
            value,
            source,
            scopes: ALL,
            live: true,
        }
    }

    fn items() -> Vec<MenuItem> {
        vec![
            item(1, SettingValue::Toggle(false), Level::Default),
            item(
                2,
                SettingValue::Number {
                    value: 4,
                    min: 0,
                    max: 5,
                    step: 2,
                    unit: "px".into(),
                },
                Level::Game,
            ),
            item(
                3,
                SettingValue::Choice {
                    index: 1,
                    options: vec!["A".into(), "B".into()],
                },
                Level::System,
            ),
            item(4, SettingValue::Action(ActionId(9)), Level::Default),
        ]
    }

    fn at(list: &mut SettingsList, index: usize) {
        list.select(index, 4);
    }

    fn set(key: u32, value: Value) -> SettingsEvent {
        SettingsEvent::Intent(Intent::SetValue {
            key: SettingKey(key),
            level: Level::Game,
            value,
        })
    }

    #[test]
    fn a_toggle_flips_on_every_press() {
        let mut l = SettingsList::new(Rect::new(0, 0, 288, 48), 12);
        let items = items();
        for action in [UiAction::Left, UiAction::Right, UiAction::Accept] {
            assert_eq!(
                l.handle(action, &items, Level::Game),
                set(1, Value::Toggle(true))
            );
        }
    }

    #[test]
    fn a_number_steps_and_stops_at_the_limits() {
        let mut l = SettingsList::new(Rect::new(0, 0, 288, 48), 12);
        at(&mut l, 1);
        let items = items();
        assert_eq!(
            l.handle(UiAction::Right, &items, Level::Game),
            set(2, Value::Number(5))
        );
        assert_eq!(
            l.handle(UiAction::Left, &items, Level::Game),
            set(2, Value::Number(2))
        );
        assert_eq!(
            l.handle(UiAction::Accept, &items, Level::Game),
            SettingsEvent::None
        );
        let mut at_max = items.clone();
        at_max[1].value = SettingValue::Number {
            value: 5,
            min: 0,
            max: 5,
            step: 2,
            unit: String::new(),
        };
        assert_eq!(
            l.handle(UiAction::Right, &at_max, Level::Game),
            SettingsEvent::None
        );
    }

    #[test]
    fn a_choice_steps_and_accept_wraps() {
        let mut l = SettingsList::new(Rect::new(0, 0, 288, 48), 12);
        at(&mut l, 2);
        let items = items();
        assert_eq!(
            l.handle(UiAction::Right, &items, Level::Game),
            SettingsEvent::None
        );
        assert_eq!(
            l.handle(UiAction::Left, &items, Level::Game),
            set(3, Value::Choice(0))
        );
        assert_eq!(
            l.handle(UiAction::Accept, &items, Level::Game),
            set(3, Value::Choice(0))
        );
    }

    #[test]
    fn an_action_runs_on_accept_only() {
        let mut l = SettingsList::new(Rect::new(0, 0, 288, 48), 12);
        at(&mut l, 3);
        let items = items();
        assert_eq!(
            l.handle(UiAction::Accept, &items, Level::Game),
            SettingsEvent::Intent(Intent::Action(ActionId(9)))
        );
        assert_eq!(
            l.handle(UiAction::Right, &items, Level::Game),
            SettingsEvent::None
        );
    }

    #[test]
    fn menu_resets_a_value_set_at_the_edit_level() {
        let mut l = SettingsList::new(Rect::new(0, 0, 288, 48), 12);
        let items = items();
        at(&mut l, 1);
        assert_eq!(
            l.handle(UiAction::Menu, &items, Level::Game),
            SettingsEvent::Intent(Intent::ResetValue {
                key: SettingKey(2),
                level: Level::Game
            })
        );
        assert_eq!(
            l.handle(UiAction::Menu, &items, Level::System),
            SettingsEvent::None
        );
    }

    #[test]
    fn a_level_outside_the_scopes_changes_nothing() {
        let mut l = SettingsList::new(Rect::new(0, 0, 288, 48), 12);
        let mut items = items();
        items[0].scopes = Scopes {
            global: true,
            ..Scopes::default()
        };
        assert_eq!(
            l.handle(UiAction::Accept, &items, Level::Game),
            SettingsEvent::None
        );
        assert_eq!(
            l.handle(UiAction::Accept, &items, Level::Global),
            SettingsEvent::Intent(Intent::SetValue {
                key: SettingKey(1),
                level: Level::Global,
                value: Value::Toggle(true)
            })
        );
    }

    #[test]
    fn navigation_and_back() {
        let mut l = SettingsList::new(Rect::new(0, 0, 288, 48), 12);
        let items = items();
        assert_eq!(
            l.handle(UiAction::Down, &items, Level::Game),
            SettingsEvent::Moved
        );
        assert_eq!(
            l.handle(UiAction::Up, &items, Level::Game),
            SettingsEvent::Moved
        );
        assert_eq!(
            l.handle(UiAction::Up, &items, Level::Game),
            SettingsEvent::None
        );
        assert_eq!(
            l.handle(UiAction::Back, &items, Level::Game),
            SettingsEvent::Back
        );
        assert_eq!(
            l.handle(UiAction::Accept, &[], Level::Game),
            SettingsEvent::None
        );
    }

    #[test]
    fn the_tree_shows_value_and_source() {
        let mut l = SettingsList::new(Rect::new(16, 12, 288, 48), 12);
        at(&mut l, 1);
        let mut frame = UiFrame::new();
        l.draw(&mut frame, &Theme::default(), &items());
        let rows: Vec<_> = frame
            .tree
            .rows()
            .map(|r| (r.label, r.value, r.source, r.focused))
            .collect();
        assert_eq!(
            rows,
            [
                ("Setting 1", Some("Off"), Some(Level::Default), false),
                ("Setting 2", Some("4 px"), Some(Level::Game), true),
                ("Setting 3", Some("B"), Some(Level::System), false),
                ("Setting 4", None, None, false),
            ]
        );
    }
}
