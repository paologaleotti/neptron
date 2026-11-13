use crate::font;
use crate::frame::UiFrame;
use crate::geom::Rect;
use crate::input::UiAction;
use crate::intent::{Intent, Value};
use crate::snapshot::{ChoiceOption, Level, MenuItem, SettingValue};
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
    /// The player accepted the choice row at this index, in a list made
    /// with [`SettingsList::with_pickers`]: the screen opens the options.
    Pick(usize),
}

/// A list of settings rows: label, value and the level the value comes from.
///
/// Up and down move. Left and right step the value. Accept toggles a switch,
/// moves a choice to the next option, or runs an action. Menu resets the
/// value at the edit level, so the row inherits again.
///
/// A list made with [`SettingsList::with_pickers`] never changes a choice
/// in place: accept on a choice row gives [`SettingsEvent::Pick`], so the
/// screen can show every option with its detail before one applies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SettingsList {
    list: List,
    pickers: bool,
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
            pickers: false,
        }
    }

    /// The same list, in which accept on a choice row gives
    /// [`SettingsEvent::Pick`] and left and right do not change it.
    pub fn with_pickers(mut self) -> Self {
        self.pickers = true;
        self
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
        if !item.is_enabled() {
            return SettingsEvent::None;
        }
        if self.picks(item) {
            match action {
                UiAction::Accept => return SettingsEvent::Pick(self.list.cursor()),
                UiAction::Left | UiAction::Right => return SettingsEvent::None,
                _ => {}
            }
        }
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
            let focused = index == cursor;
            draw_row(
                frame,
                theme,
                row,
                item,
                focused,
                focused && item.is_enabled() && !self.picks(item),
            );
        }
        if scrolls {
            self.list.draw_scrollbar(frame, theme, len, top);
        }
    }

    fn picks(&self, item: &MenuItem) -> bool {
        self.pickers && matches!(item.value, SettingValue::Choice { .. })
    }
}

fn draw_row(
    frame: &mut UiFrame,
    theme: &Theme,
    row: Rect,
    item: &MenuItem,
    focused: bool,
    arrows: bool,
) {
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
    if let Some(reason) = &item.unavailable {
        let label_color = if focused { color } else { dim };
        let reason_box = Rect::new(
            add(inner.x, inner.w.saturating_sub(columns_w - advance)),
            row.y,
            columns_w - advance,
            row.h,
        );
        let reason_clipped = text::draw_in(&mut frame.scene, reason_box, Align::Right, dim, reason);
        let label_box = Rect::new(inner.x, row.y, inner.w.saturating_sub(columns_w), row.h);
        let label_clipped = text::draw_in(
            &mut frame.scene,
            label_box,
            Align::Left,
            label_color,
            &item.label,
        );
        push(
            frame,
            item,
            row,
            Some(reason),
            focused,
            label_clipped || reason_clipped,
        );
        return;
    }
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
    if arrows {
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
            next_choice(*index, options, direction).map(Value::Choice)
        }
        SettingValue::Action(_) => None,
    }
}

/// The next option that the player can choose: the one before or after
/// `index` for a `direction` of `-1` or `1`, or the one after it with a wrap
/// to the start for `0`. Options that are not available are skipped.
fn next_choice(index: usize, options: &[ChoiceOption], direction: i64) -> Option<usize> {
    let len = options.len();
    let enabled = |i: &usize| options.get(*i).is_some_and(ChoiceOption::is_enabled);
    let next = match direction {
        -1 if index < len => (0..index).rev().find(enabled),
        1 if index < len => (index + 1..len).find(enabled),
        0 => {
            let start = index.saturating_add(1).min(len);
            (start..len).chain(0..start).find(enabled)
        }
        _ => None,
    }?;
    (next != index).then_some(next)
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
            unavailable: None,
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
    fn a_choice_skips_the_options_that_are_not_available() {
        let mut l = SettingsList::new(Rect::new(0, 0, 288, 48), 12);
        at(&mut l, 2);
        let mut items = items();
        let style = |index| SettingValue::Choice {
            index,
            options: vec![
                ChoiceOption::new("Auto"),
                ChoiceOption::new("Native").unavailable("Needs a 15 kHz monitor"),
                ChoiceOption::new("2x"),
                ChoiceOption::new("3x").unavailable("Needs a 45 kHz monitor"),
            ],
        };
        items[2].value = style(0);
        assert_eq!(
            l.handle(UiAction::Right, &items, Level::Game),
            set(3, Value::Choice(2))
        );
        items[2].value = style(2);
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
        items[2].value = SettingValue::Choice {
            index: 0,
            options: vec![
                ChoiceOption::new("Auto"),
                ChoiceOption::new("Native").unavailable("Needs a 15 kHz monitor"),
            ],
        };
        assert_eq!(
            l.handle(UiAction::Accept, &items, Level::Game),
            SettingsEvent::None
        );
    }

    #[test]
    fn with_pickers_a_choice_opens_its_options_and_never_steps() {
        let mut l = SettingsList::new(Rect::new(16, 12, 288, 48), 12).with_pickers();
        let items = items();
        at(&mut l, 2);
        assert_eq!(
            l.handle(UiAction::Accept, &items, Level::Game),
            SettingsEvent::Pick(2)
        );
        for action in [UiAction::Left, UiAction::Right] {
            assert_eq!(l.handle(action, &items, Level::Game), SettingsEvent::None);
        }
        assert_eq!(
            l.handle(UiAction::Menu, &items, Level::System),
            SettingsEvent::Intent(Intent::ResetValue {
                key: SettingKey(3),
                level: Level::System
            })
        );
        at(&mut l, 0);
        assert_eq!(
            l.handle(UiAction::Accept, &items, Level::Game),
            set(1, Value::Toggle(true))
        );
        let texts = |list: &SettingsList| {
            let mut frame = UiFrame::new();
            list.draw(&mut frame, &Theme::default(), &items);
            frame
                .scene
                .commands()
                .iter()
                .filter_map(|c| match *c {
                    crate::DrawCommand::Text { span, .. } => {
                        Some(frame.scene.text(span).to_owned())
                    }
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        at(&mut l, 2);
        assert!(!texts(&l).iter().any(|t| t == "<" || t == ">"));
        at(&mut l, 1);
        assert!(texts(&l).iter().any(|t| t == "<"));
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
    fn an_unavailable_row_changes_nothing_and_shows_its_reason() {
        let reason = "Not used on a flat panel";
        let mut items = items();
        for item in &mut items[..3] {
            *item = item.clone().unavailable(reason);
        }
        for pickers in [false, true] {
            let mut l = SettingsList::new(Rect::new(16, 12, 288, 48), 12);
            if pickers {
                l = l.with_pickers();
            }
            for index in 0..3 {
                at(&mut l, index);
                for action in [
                    UiAction::Accept,
                    UiAction::Left,
                    UiAction::Right,
                    UiAction::Menu,
                ] {
                    assert_eq!(
                        l.handle(action, &items, Level::Game),
                        SettingsEvent::None,
                        "{index} {action:?}"
                    );
                }
            }
            assert_eq!(
                l.handle(UiAction::Down, &items, Level::Game),
                SettingsEvent::Moved
            );
            let mut frame = UiFrame::new();
            at(&mut l, 2);
            l.draw(&mut frame, &Theme::default(), &items);
            let rows: Vec<_> = frame
                .tree
                .rows()
                .map(|r| (r.label, r.value, r.clipped))
                .collect();
            assert_eq!(rows[0], ("Setting 1", Some(reason), false));
            assert_eq!(rows[2], ("Setting 3", Some(reason), false));
            let texts: Vec<_> = frame
                .scene
                .commands()
                .iter()
                .filter_map(|c| match *c {
                    crate::DrawCommand::Text { span, .. } => Some(frame.scene.text(span)),
                    _ => None,
                })
                .collect();
            assert!(!texts.contains(&"<"), "no arrows on a disabled row");
            for source in ["default", "game", "system"] {
                assert!(!texts.contains(&source), "no source on a disabled row");
            }
        }
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
