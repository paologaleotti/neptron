//! The widget templates that every screen is built from: list, settings
//! list, dialog, grid and slider.
//!
//! A template holds only its navigation state (cursor, scroll, focus). The
//! data comes from the snapshot at each frame. `handle` turns one action
//! into an event; `draw` adds commands to the scene and rows to the tree.
//! A template that sets a clip removes it again before it returns.

mod list;
mod settings_list;

pub use list::{List, ListEvent, ListItem, MAX_VISIBLE_ROWS, SCROLLBAR_WIDTH_PX};
pub use settings_list::{SettingsEvent, SettingsList, source_label};

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
}
