//! Four players: the port of each pad in the menus and in a game, across
//! the relaunch of a launch and of a quit.

use neptron::relaunch::PadRecord;
use neptron::{NextRun, Outcome};
use neptron_input::{DeviceIdentity, PadButton, PadInfo, Roster};
use neptron_types::Control;

use crate::rig::{Rig, in_child, test_name};

/// Pad `n`: its own model at its own event node.
pub fn pad(n: u8) -> PadInfo {
    PadInfo {
        identity: DeviceIdentity::from_parts(&[b"model", &[n]]),
        path: DeviceIdentity::from_parts(&[b"/dev/input/event", &[n]]),
        name: format!("Pad {n}"),
    }
}

/// The name and presence of the pad on each of the first `ports` ports.
pub fn seats(roster: &Roster, ports: usize) -> Vec<Option<(String, bool)>> {
    roster.ports()[..ports]
        .iter()
        .map(|e| e.as_ref().map(|e| (e.pad.name.clone(), e.present)))
        .collect()
}

fn here(name: &str) -> Option<(String, bool)> {
    Some((name.to_owned(), true))
}

fn away(name: &str) -> Option<(String, bool)> {
    Some((name.to_owned(), false))
}

/// Starts the game list, waits for the scan, and launches the test game.
fn launch_from_the_list(rig: &mut Rig) -> NextRun {
    rig.until(600, "the scan", |r| {
        !r.shell().snapshot().systems.is_empty()
    });
    rig.tap(&[Control::A]);
    rig.until(10, "the game rows", |r| {
        r.focused().as_deref() == Some("Test Game")
    });
    rig.hold(&[Control::A]);
    let outcome = rig.until_end(2);
    rig.release(&[Control::A]);
    let Outcome::Relaunch(next) = outcome else {
        panic!("a launch relaunches: {outcome:?}");
    };
    next
}

#[test]
#[ignore = "tier 2"]
fn the_order_of_the_pads_goes_into_the_game_and_a_gone_pad_keeps_its_port() {
    let name = test_name(
        module_path!(),
        "the_order_of_the_pads_goes_into_the_game_and_a_gone_pad_keeps_its_port",
    );
    in_child(&name, |dir| {
        let mut rig = Rig::new(dir);
        rig.boot(None);
        rig.plug(pad(1));
        rig.plug(pad(2));
        rig.frames(2);
        assert_eq!(
            seats(&rig.shell().roster(), 3),
            [here("Pad 1"), here("Pad 2"), None]
        );
        let next = launch_from_the_list(&mut rig);
        assert_eq!(next.pads, PadRecord::list(&rig.shell().roster()));
        assert_eq!(next.pads.len(), 2);
        rig.finish();

        // The new run finds the pads in the other order, as SDL3 can list
        // them; each pad still gets its port.
        rig.plugged.reverse();
        rig.boot(Some(next));
        assert!(rig.shell().in_game());
        rig.frames(30);
        assert_eq!(
            seats(&rig.shell().roster(), 3),
            [here("Pad 1"), here("Pad 2"), None]
        );
        let two = rig.plugged[0].1;
        rig.pads.set(two, PadButton::East, true);
        rig.frames(1);
        let ports = rig.shell().ports().expect("ports");
        assert!(ports.is_held(1, Control::A), "player 2 presses A");
        assert!(!ports.is_held(0, Control::A));
        rig.pads.set(two, PadButton::East, false);

        // Player 1 goes away: its port waits, and a new pad of another
        // model takes port 2, not player 1.
        let one = rig.plugged[1].1;
        rig.unplug(one);
        rig.frames(1);
        assert_eq!(seats(&rig.shell().roster(), 1), [away("Pad 1")]);
        rig.plug(pad(3));
        rig.frames(1);
        assert_eq!(
            seats(&rig.shell().roster(), 3),
            [away("Pad 1"), here("Pad 2"), here("Pad 3")]
        );
        let one = rig.plug(pad(1));
        rig.frames(1);
        assert_eq!(seats(&rig.shell().roster(), 1), [here("Pad 1")]);
        rig.pads.set(one, PadButton::East, true);
        rig.frames(1);
        let ports = rig.shell().ports().expect("ports");
        assert!(ports.is_held(0, Control::A), "player 1 is back on port 0");
        rig.pads.set(one, PadButton::East, false);
        rig.frames(1);

        // Quit: the list run gets the order of the game.
        rig.tap(&[Control::Select, Control::Start]);
        rig.frames(1);
        assert_eq!(rig.screen(), "quick-menu");
        rig.focus("Quit game");
        rig.hold(&[Control::A]);
        let outcome = rig.until_end(4);
        rig.release(&[Control::A]);
        let Outcome::Relaunch(next) = outcome else {
            panic!("a quit relaunches: {outcome:?}");
        };
        let names: Vec<_> = next
            .pads
            .iter()
            .map(|p| (p.port, p.name.as_str()))
            .collect();
        assert_eq!(names, [(0, "Pad 1"), (1, "Pad 2"), (2, "Pad 3")]);
        rig.finish();
    });
}

#[test]
#[ignore = "tier 2"]
fn in_the_menus_player_2_becomes_player_1_when_player_1_goes() {
    let name = test_name(
        module_path!(),
        "in_the_menus_player_2_becomes_player_1_when_player_1_goes",
    );
    in_child(&name, |dir| {
        let mut rig = Rig::new(dir);
        rig.boot(None);
        let one = rig.plug(pad(1));
        rig.plug(pad(2));
        rig.frames(2);
        rig.unplug(one);
        rig.frames(1);
        assert_eq!(seats(&rig.shell().roster(), 2), [here("Pad 2"), None]);
        // Plugged in again, it is player 2.
        rig.plug(pad(1));
        rig.frames(1);
        assert_eq!(
            seats(&rig.shell().roster(), 2),
            [here("Pad 2"), here("Pad 1")]
        );
        rig.finish();
    });
}

/// Opens `Settings > Input` from home or the quick menu.
fn open_input(rig: &mut Rig) {
    rig.focus("Settings");
    rig.tap(&[Control::A]);
    assert_eq!(rig.screen(), "settings");
    rig.focus("Input");
    rig.tap(&[Control::A]);
    assert_eq!(rig.screen(), "input");
}

/// The label and the value of every row of the panel.
fn rows(rig: &mut Rig) -> Vec<(String, String)> {
    rig.tree()
        .rows
        .into_iter()
        .filter_map(|r| r.value.map(|v| (r.label, v)))
        .collect()
}

fn pairs(rows: &[(&str, &str)]) -> Vec<(String, String)> {
    rows.iter()
        .map(|(a, b)| ((*a).to_owned(), (*b).to_owned()))
        .collect()
}

#[test]
#[ignore = "tier 2"]
fn the_input_panel_shows_five_pads_and_follows_a_hotplug() {
    let name = test_name(
        module_path!(),
        "the_input_panel_shows_five_pads_and_follows_a_hotplug",
    );
    in_child(&name, |dir| {
        let mut rig = Rig::new(dir);
        let first = rig.plug(pad(1));
        for n in 2..=5 {
            rig.plug(pad(n));
        }
        rig.boot(None);
        let first = rig.plugged.iter().position(|(_, p)| *p == first);
        rig.frames(2);
        rig.scanned();
        open_input(&mut rig);
        assert_eq!(
            rows(&mut rig),
            pairs(&[
                ("Player 1", "Pad 1"),
                ("Player 2", "Pad 2"),
                ("Player 3", "Pad 3"),
                ("Player 4", "Pad 4"),
                ("Extra pad", "Pad 5"),
                ("Keyboard", "Player 1"),
                ("Hold Start for menu", "0 s"),
            ])
        );
        // Player 1 goes while the panel is open: every pad moves down, and
        // the fifth pad becomes player 4.
        let one = rig.plugged[first.expect("pad 1")].1;
        rig.unplug(one);
        rig.frames(2);
        assert_eq!(
            rows(&mut rig)[..5],
            pairs(&[
                ("Player 1", "Pad 2"),
                ("Player 2", "Pad 3"),
                ("Player 3", "Pad 4"),
                ("Player 4", "Pad 5"),
                ("Keyboard", "Player 1"),
            ])
        );
        rig.finish();
    });
}

#[test]
#[ignore = "tier 2"]
fn in_a_game_the_input_panel_shows_a_gone_player_as_away() {
    let name = test_name(
        module_path!(),
        "in_a_game_the_input_panel_shows_a_gone_player_as_away",
    );
    in_child(&name, |dir| {
        let mut rig = Rig::new(dir);
        let mut earlier = Roster::new();
        for (port, n) in [(0, 1), (1, 2)] {
            let entry = neptron_input::RosterEntry {
                pad: pad(n),
                present: true,
            };
            earlier.set(port, Some(entry)).expect("a port");
        }
        rig.plug(pad(2));
        rig.boot(Some(NextRun {
            game: Some(neptron::relaunch::GameRef {
                system: "nes".to_owned(),
                path: "nes/Test Game.nes".to_owned(),
            }),
            notice: None,
            pads: PadRecord::list(&earlier),
        }));
        rig.frames(30);
        rig.tap(&[Control::Select, Control::Start]);
        rig.frames(1);
        assert_eq!(rig.screen(), "quick-menu");
        open_input(&mut rig);
        assert_eq!(
            rows(&mut rig)[..2],
            pairs(&[("Player 1", "Pad 1 (away)"), ("Player 2", "Pad 2")])
        );
        // Player 1 comes back while the panel is open.
        rig.plug(pad(1));
        rig.frames(2);
        assert_eq!(
            rows(&mut rig)[..2],
            pairs(&[("Player 1", "Pad 1"), ("Player 2", "Pad 2")])
        );
        rig.finish();
    });
}
