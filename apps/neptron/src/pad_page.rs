//! The page of a controller and the texts of the guided mapping: what
//! the player reads of a map, from the map the input source reads.

use neptron_input::{
    Binding, GUIDED_STEPS, GenericLayout, GuidedNote, HatDirection, MapKind, PadMap, PlayerMap,
    RosterEntry,
};
use neptron_types::Control;
use neptron_ui::snapshot::{ActionId, MenuItem, PageId, SettingKey, SettingValue, SettingsPage};

use crate::pages::{ACTION_KEY_BASE, fit_name};

/// The page of the controller on port 0. The ports that follow have the
/// next ids.
pub const PAD_PAGE_FIRST: u32 = 0x0100;

/// The action of `Map buttons` on the page of port 0. The ports that
/// follow have the next ids.
pub const MAP_BUTTONS_FIRST: u32 = 0x0100;

/// The action of `Reset to default` on the page of port 0. The ports that
/// follow have the next ids.
pub const RESET_LAYOUT_FIRST: u32 = 0x0110;

/// The label of the row that starts the guided mapping.
pub const MAP_BUTTONS_LABEL: &str = "Map buttons";

/// The label of the row that removes the map of the player.
pub const RESET_LAYOUT_LABEL: &str = "Reset to default";

/// Why the actions of the page are disabled in a game: the mapping runs
/// only in the game list.
pub const NOT_IN_GAME: &str = "Not during a game";

/// Why the actions of the page are disabled when the port has no pad.
pub const NO_CONTROLLER: &str = "No controller";

/// Why `Reset to default` is disabled for a pad with no map of the player.
pub const ALREADY_DEFAULT: &str = "Already default";

/// The most characters of the name of a pad on the guided mapping.
pub const MAPPING_NAME_MAX_CHARS: usize = 44;

/// The most skipped controls that the review names one by one.
const SKIPPED_NAMED_MAX: usize = 6;

/// What a row of the page of a controller asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PadAction {
    /// Start the guided mapping of the pad on this port.
    MapButtons(u8),
    /// Remove the map of the player of the pad on this port.
    Reset(u8),
}

/// The page of the controller on `port`.
pub fn pad_page(port: u8) -> PageId {
    PageId(PAD_PAGE_FIRST + u32::from(port))
}

/// The port of the page of a controller, or `None` for another page.
pub fn port_of_page(page: PageId) -> Option<u8> {
    let port = page.0.checked_sub(PAD_PAGE_FIRST)?;
    u8::try_from(port)
        .ok()
        .filter(|p| *p < neptron_input::MAX_PORTS)
}

/// The action of a row of the page of a controller, or `None` for another
/// action.
pub fn pad_action(id: ActionId) -> Option<PadAction> {
    let port = |first: u32| {
        id.0.checked_sub(first)
            .and_then(|p| u8::try_from(p).ok())
            .filter(|p| *p < neptron_input::MAX_PORTS)
    };
    port(MAP_BUTTONS_FIRST)
        .map(PadAction::MapButtons)
        .or_else(|| port(RESET_LAYOUT_FIRST).map(PadAction::Reset))
}

/// The page of the controller on a port, titled `label` (`Player 1`):
/// the name of the pad and its kind of map, `Map buttons`, `Reset to
/// default`, and the binding of each control of `map`. `entry` is the pad
/// of the port, `None` for no pad. In a game the actions are disabled.
pub fn pad_page_rows(
    page: PageId,
    label: &str,
    entry: Option<&RosterEntry>,
    map: Option<PadMap>,
    in_game: bool,
) -> SettingsPage {
    let port = port_of_page(page).unwrap_or_default();
    let present = entry.filter(|e| e.present);
    let name = present.map_or_else(|| "None".to_owned(), |e| fit_name(&e.pad.name, 33));
    let kind = present.map_or("", |e| match e.map {
        MapKind::Known => "Known",
        MapKind::Generic => "Generic",
        MapKind::Player => "Yours",
    });
    let mut rows = vec![text(60, "Controller", &name), text(61, "Layout", kind)];
    let reason = match (present, in_game) {
        (None, _) => Some(NO_CONTROLLER),
        (Some(_), true) => Some(NOT_IN_GAME),
        (Some(_), false) => None,
    };
    let map_buttons = action(
        62,
        MAP_BUTTONS_LABEL,
        ActionId(MAP_BUTTONS_FIRST + u32::from(port)),
    );
    let reset = action(
        63,
        RESET_LAYOUT_LABEL,
        ActionId(RESET_LAYOUT_FIRST + u32::from(port)),
    )
    .with_confirm(
        "Reset this controller?",
        "Your layout of this model is removed. The default layout comes back.",
        "Resetting...",
    );
    let mine = present.is_some_and(|e| e.map == MapKind::Player);
    let reset_reason = reason.or((!mine).then_some(ALREADY_DEFAULT));
    rows.push(match reason {
        Some(reason) => map_buttons.unavailable(reason),
        None => map_buttons,
    });
    rows.push(match reset_reason {
        Some(reason) => reset.unavailable(reason),
        None => reset,
    });
    if let (Some(_), Some(map)) = (present, map) {
        for (control, n) in GUIDED_STEPS.into_iter().zip(64..) {
            rows.push(text(n, control_name(control), &binding_text(control, &map)));
        }
    }
    SettingsPage::new(page, label, rows)
}

/// The name of a control, as the page and the mapping show it.
pub fn control_name(control: Control) -> &'static str {
    match control {
        Control::B => "B",
        Control::Y => "Y",
        Control::Select => "Select",
        Control::Start => "Start",
        Control::Up => "Up",
        Control::Down => "Down",
        Control::Left => "Left",
        Control::Right => "Right",
        Control::A => "A",
        Control::X => "X",
        Control::L => "L",
        Control::R => "R",
        Control::L2 => "L2",
        Control::R2 => "R2",
        Control::L3 => "L3",
        Control::R3 => "R3",
        Control::LeftX => "Left stick X",
        Control::LeftY => "Left stick Y",
        Control::RightX => "Right stick X",
        Control::RightY => "Right stick Y",
    }
}

/// What the guided mapping asks for `control`.
pub fn prompt_text(control: Control) -> &'static str {
    match control {
        Control::Up => "Press Up",
        Control::Down => "Press Down",
        Control::Left => "Press Left",
        Control::Right => "Press Right",
        Control::A => "Press A (right face button)",
        Control::B => "Press B (bottom face button)",
        Control::Y => "Press Y (left face button)",
        Control::X => "Press X (top face button)",
        Control::Start => "Press Start",
        Control::Select => "Press Select",
        Control::L => "Press L (left shoulder)",
        Control::R => "Press R (right shoulder)",
        Control::L2 => "Press L2 (left trigger)",
        Control::R2 => "Press R2 (right trigger)",
        Control::L3 => "Press L3 (left stick press)",
        Control::R3 => "Press R3 (right stick press)",
        Control::LeftX => "Move the left stick right",
        Control::LeftY => "Move the left stick down",
        Control::RightX => "Move the right stick right",
        Control::RightY => "Move the right stick down",
    }
}

/// What the guided mapping says about the last input.
pub fn note_text(note: Option<GuidedNote>) -> String {
    match note {
        None => String::new(),
        Some(GuidedNote::Taken(control)) => {
            format!("{} uses it already. Try another.", control_name(control))
        }
        Some(GuidedNote::NeedsStick) => "Move a stick, or hold a button to skip.".to_owned(),
        Some(GuidedNote::Skipped(control)) => format!("{} is skipped.", control_name(control)),
    }
}

/// The message of the review of `map`, and whether it can be kept: a map
/// with no direction, no A or no B cannot drive the menus.
pub fn review_text(map: &PlayerMap) -> (String, bool) {
    let gaps: Vec<&str> = map.menu_gaps().map(control_name).collect();
    if !gaps.is_empty() {
        return (
            format!(
                "This layout has no {}. A controller needs the four directions, A and B.",
                join_no(&gaps)
            ),
            false,
        );
    }
    let set = map.bindings().count();
    let mut text = format!("{set} of {} controls are set.", GUIDED_STEPS.len());
    let skipped: Vec<&str> = GUIDED_STEPS
        .iter()
        .filter(|c| map.get(**c).is_none())
        .map(|c| control_name(*c))
        .collect();
    if skipped.len() > SKIPPED_NAMED_MAX {
        text.push_str(&format!(" {} are skipped.", skipped.len()));
    } else if !skipped.is_empty() {
        text.push_str(&format!(" Skipped: {}.", skipped.join(", ")));
    }
    if !map.has_menu_combo() {
        text.push_str(" Without Start and Select it cannot open the quick menu.");
    }
    (text, true)
}

/// The binding of `control` in `map`, as the page shows it.
pub fn binding_text(control: Control, map: &PadMap) -> String {
    match map {
        PadMap::Player(player) => player
            .get(control)
            .map_or_else(|| "None".to_owned(), |b| player_binding_text(&b)),
        PadMap::Generic(layout) => generic_binding_text(control, layout),
        PadMap::Known => known_binding_text(control).to_owned(),
        _ => "None".to_owned(),
    }
}

fn player_binding_text(binding: &Binding) -> String {
    match *binding {
        Binding::Button(index) => format!("Button {index}"),
        Binding::Hat { index, direction } => format!("Hat {index} {}", hat_word(direction)),
        Binding::HalfAxis {
            index,
            positive,
            full,
        } => {
            let side = if positive { '+' } else { '-' };
            let end = if full { " (from its end)" } else { "" };
            format!("Axis {index} {side}{end}")
        }
        Binding::Stick {
            index,
            inverted: false,
        } => format!("Axis {index}"),
        Binding::Stick {
            index,
            inverted: true,
        } => format!("Axis {index} inverted"),
    }
}

fn generic_binding_text(control: Control, layout: &GenericLayout) -> String {
    if let Some(index) = (0..=u8::MAX).find(|i| layout.button(*i) == Some(control)) {
        return format!("Button {index}");
    }
    let direction = match control {
        Control::Up => Some((HatDirection::Up, false)),
        Control::Down => Some((HatDirection::Down, true)),
        Control::Left => Some((HatDirection::Left, false)),
        Control::Right => Some((HatDirection::Right, true)),
        _ => None,
    };
    if let Some((direction, positive)) = direction {
        if layout.hat_is_dpad(0) {
            return format!("Hat 0 {}", hat_word(direction));
        }
        let axis = (0..layout.axes_read()).find(|i| {
            layout
                .axis(*i)
                .and_then(|role| role.dpad)
                .is_some_and(|(negative, plus)| (if positive { plus } else { negative }) == control)
        });
        if let Some(index) = axis {
            let side = if positive { '+' } else { '-' };
            return format!("Axis {index} {side}");
        }
    }
    let stick =
        (0..layout.axes_read()).find(|i| layout.axis(*i).is_some_and(|r| r.stick == control));
    stick.map_or_else(|| "None".to_owned(), |index| format!("Axis {index}"))
}

/// The control of the standard layout of SDL3 that drives `control`.
fn known_binding_text(control: Control) -> &'static str {
    match control {
        Control::B => "South button",
        Control::A => "East button",
        Control::Y => "West button",
        Control::X => "North button",
        Control::Select => "Back",
        Control::Start => "Start",
        Control::L => "Left shoulder",
        Control::R => "Right shoulder",
        Control::L2 => "Left trigger",
        Control::R2 => "Right trigger",
        Control::L3 => "Left stick press",
        Control::R3 => "Right stick press",
        Control::Up => "D-pad up",
        Control::Down => "D-pad down",
        Control::Left => "D-pad left",
        Control::Right => "D-pad right",
        Control::LeftX => "Left stick X",
        Control::LeftY => "Left stick Y",
        Control::RightX => "Right stick X",
        Control::RightY => "Right stick Y",
    }
}

fn hat_word(direction: HatDirection) -> &'static str {
    match direction {
        HatDirection::Up => "up",
        HatDirection::Right => "right",
        HatDirection::Down => "down",
        HatDirection::Left => "left",
    }
}

/// `a`, `a and no b`, `a, no b and no c`.
fn join_no(names: &[&str]) -> String {
    match names {
        [] => String::new(),
        [one] => (*one).to_owned(),
        [rest @ .., last] => format!("{} and no {last}", rest.join(", no ")),
    }
}

fn text(n: u32, label: &str, text: &str) -> MenuItem {
    MenuItem::new(
        SettingKey(ACTION_KEY_BASE + n),
        label,
        SettingValue::Text(text.to_owned()),
    )
}

fn action(n: u32, label: &str, id: ActionId) -> MenuItem {
    MenuItem::new(
        SettingKey(ACTION_KEY_BASE + n),
        label,
        SettingValue::Action(id),
    )
}

#[cfg(test)]
mod tests {
    use neptron_input::{DeviceIdentity, JoystickCounts, PadInfo};

    use super::*;

    fn entry(map: MapKind, present: bool) -> RosterEntry {
        RosterEntry {
            pad: PadInfo {
                identity: DeviceIdentity(1),
                path: DeviceIdentity(2),
                name: "USB Joystick".to_owned(),
            },
            present,
            map,
        }
    }

    fn rows(page: &SettingsPage) -> Vec<(String, String, Option<String>)> {
        page.rows
            .iter()
            .map(|r| {
                let value = match &r.value {
                    SettingValue::Text(text) => text.clone(),
                    _ => String::new(),
                };
                (r.label.clone(), value, r.unavailable.clone())
            })
            .collect()
    }

    fn mine() -> PlayerMap {
        let mut map = PlayerMap::new();
        for (control, direction) in [
            (Control::Up, HatDirection::Up),
            (Control::Down, HatDirection::Down),
            (Control::Left, HatDirection::Left),
            (Control::Right, HatDirection::Right),
        ] {
            let hat = Binding::Hat {
                index: 0,
                direction,
            };
            map.set(control, Some(hat)).unwrap();
        }
        map.set(Control::A, Some(Binding::Button(2))).unwrap();
        map.set(Control::B, Some(Binding::Button(1))).unwrap();
        map
    }

    #[test]
    fn page_ids_and_action_ids_hold_the_port() {
        for port in 0..neptron_input::MAX_PORTS {
            assert_eq!(port_of_page(pad_page(port)), Some(port));
            assert_eq!(
                pad_action(ActionId(MAP_BUTTONS_FIRST + u32::from(port))),
                Some(PadAction::MapButtons(port))
            );
            assert_eq!(
                pad_action(ActionId(RESET_LAYOUT_FIRST + u32::from(port))),
                Some(PadAction::Reset(port))
            );
        }
        assert_eq!(port_of_page(PageId(PAD_PAGE_FIRST + 8)), None);
        assert_eq!(port_of_page(PageId(3)), None);
        assert_eq!(pad_action(ActionId(1)), None);
        assert_eq!(pad_action(ActionId(MAP_BUTTONS_FIRST + 8)), None);
    }

    #[test]
    fn the_page_of_a_pad_with_a_map_of_the_player_shows_each_binding() {
        let page = pad_page_rows(
            pad_page(0),
            "Player 1",
            Some(&entry(MapKind::Player, true)),
            Some(PadMap::Player(mine())),
            false,
        );
        assert_eq!(page.title, "Player 1");
        let rows = rows(&page);
        assert_eq!(rows.len(), 4 + GUIDED_STEPS.len());
        let head: Vec<_> = rows[..6]
            .iter()
            .map(|(l, v, u)| (l.as_str(), v.as_str(), u.clone()))
            .collect();
        assert_eq!(
            head,
            [
                ("Controller", "USB Joystick", None),
                ("Layout", "Yours", None),
                ("Map buttons", "", None),
                ("Reset to default", "", None),
                ("Up", "Hat 0 up", None),
                ("Down", "Hat 0 down", None),
            ]
        );
        assert!(rows.contains(&("A".to_owned(), "Button 2".to_owned(), None)));
        assert!(rows.contains(&("L2".to_owned(), "None".to_owned(), None)));
        assert!(page.rows[3].confirm.is_some(), "reset asks first");
        assert_eq!(
            page.rows[2].value,
            SettingValue::Action(ActionId(MAP_BUTTONS_FIRST))
        );
    }

    #[test]
    fn the_actions_are_disabled_in_a_game_with_no_pad_and_reset_for_a_default_map() {
        let in_game = pad_page_rows(
            pad_page(1),
            "Player 2",
            Some(&entry(MapKind::Player, true)),
            Some(PadMap::Player(mine())),
            true,
        );
        let reasons: Vec<_> = in_game.rows[2..4]
            .iter()
            .map(|r| r.unavailable.as_deref())
            .collect();
        assert_eq!(reasons, [Some(NOT_IN_GAME), Some(NOT_IN_GAME)]);

        let known = pad_page_rows(
            pad_page(1),
            "Player 2",
            Some(&entry(MapKind::Known, true)),
            Some(PadMap::Known),
            false,
        );
        let reasons: Vec<_> = known.rows[2..4]
            .iter()
            .map(|r| r.unavailable.as_deref())
            .collect();
        assert_eq!(reasons, [None, Some(ALREADY_DEFAULT)]);
        assert!(rows(&known).contains(&("A".to_owned(), "East button".to_owned(), None)));

        for entry in [None, Some(entry(MapKind::Known, false))] {
            let none = pad_page_rows(pad_page(2), "Player 3", entry.as_ref(), None, false);
            let rows = rows(&none);
            assert_eq!(rows.len(), 4, "no binding rows with no pad");
            assert_eq!(rows[0].1, "None");
            assert_eq!(rows[2].2.as_deref(), Some(NO_CONTROLLER));
            assert_eq!(rows[3].2.as_deref(), Some(NO_CONTROLLER));
        }
    }

    #[test]
    fn a_generic_layout_names_the_indices_of_its_pad() {
        let full = GenericLayout::for_counts(JoystickCounts {
            buttons: 13,
            hats: 1,
            axes: 4,
        })
        .unwrap();
        let map = PadMap::Generic(full);
        assert_eq!(binding_text(Control::A, &map), "Button 2");
        assert_eq!(binding_text(Control::Up, &map), "Hat 0 up");
        assert_eq!(binding_text(Control::RightY, &map), "Axis 3");
        let few = GenericLayout::for_counts(JoystickCounts {
            buttons: 4,
            hats: 0,
            axes: 2,
        })
        .unwrap();
        let map = PadMap::Generic(few);
        assert_eq!(binding_text(Control::Start, &map), "None");
        assert_eq!(binding_text(Control::Left, &map), "Axis 0 -");
        assert_eq!(binding_text(Control::Down, &map), "Axis 1 +");
        assert_eq!(binding_text(Control::LeftX, &map), "Axis 0");
    }

    #[test]
    fn each_binding_of_the_player_reads_as_words() {
        let texts: Vec<_> = [
            Binding::Button(3),
            Binding::Hat {
                index: 1,
                direction: HatDirection::Left,
            },
            Binding::HalfAxis {
                index: 2,
                positive: false,
                full: false,
            },
            Binding::HalfAxis {
                index: 4,
                positive: true,
                full: true,
            },
            Binding::Stick {
                index: 0,
                inverted: false,
            },
            Binding::Stick {
                index: 1,
                inverted: true,
            },
        ]
        .iter()
        .map(player_binding_text)
        .collect();
        assert_eq!(
            texts,
            [
                "Button 3",
                "Hat 1 left",
                "Axis 2 -",
                "Axis 4 + (from its end)",
                "Axis 0",
                "Axis 1 inverted"
            ]
        );
    }

    #[test]
    fn the_review_names_what_is_missing_or_skipped() {
        let (text, keep) = review_text(&PlayerMap::new());
        assert!(!keep);
        assert_eq!(
            text,
            "This layout has no Up, no Down, no Left, no Right, no A and no B. A controller needs the four directions, A and B."
        );
        let mut map = mine();
        map.set(Control::Left, None).unwrap();
        assert_eq!(
            review_text(&map).0,
            "This layout has no Left. A controller needs the four directions, A and B."
        );
        let map = mine();
        let (text, keep) = review_text(&map);
        assert!(keep);
        assert_eq!(
            text,
            "6 of 20 controls are set. 14 are skipped. Without Start and Select it cannot open the quick menu."
        );
        let mut full = mine();
        for (control, index) in [
            (Control::Start, 9),
            (Control::Select, 8),
            (Control::Y, 0),
            (Control::X, 3),
            (Control::L, 4),
            (Control::R, 5),
            (Control::L2, 6),
            (Control::R2, 7),
            (Control::L3, 10),
            (Control::R3, 11),
        ] {
            full.set(control, Some(Binding::Button(index))).unwrap();
        }
        assert_eq!(
            review_text(&full).0,
            "16 of 20 controls are set. Skipped: Left stick X, Left stick Y, Right stick X, Right stick Y."
        );
    }

    #[test]
    fn every_prompt_and_note_fits_the_mapping_screen() {
        for control in GUIDED_STEPS {
            assert!(prompt_text(control).len() <= 44, "{control:?}");
            let note = note_text(Some(GuidedNote::Taken(control)));
            assert!(note.chars().count() <= 46, "{note}");
        }
        assert_eq!(note_text(None), "");
        assert_eq!(
            note_text(Some(GuidedNote::Skipped(Control::L3))),
            "L3 is skipped."
        );
    }
}
