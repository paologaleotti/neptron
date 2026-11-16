//! The Network panel: the address file of the run folder, read off the
//! frame thread, on screen while the panel is open, and its error paths.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use neptron::NextRun;
use neptron::network::{SHARE_OFF_FILE, STATUS_FILE};
use neptron::pages::{ADDRESS_LABEL, ADDRESS_NOT_KNOWN, NO_ADDRESS};
use neptron_types::Control;

use crate::rig::{Rig, in_child, test_name};

/// The longest real time a test waits for the reader thread: a guard
/// against a hang only. A read of the file takes far less, but the thread
/// runs in real time on a loaded machine.
const READ_WAIT_MAX: Duration = Duration::from_secs(60);

const ONE_PORT: &str = "version = 1\nhost_name = \"neptron\"\n\n[[address]]\nport = \"eth0\"\nipv4 = \"192.168.0.112\"\n";
const NO_LEASE: &str = "version = 1\nhost_name = \"neptron\"\n";

/// A new, empty folder for one test, removed when the test ends.
struct Fresh(PathBuf);

impl Fresh {
    fn new(label: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "neptron-app-network-{label}-{}",
            std::process::id()
        ));
        // A leftover of an earlier run with the same process id; absence is
        // fine.
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("the folder");
        Self(dir)
    }
}

impl Drop for Fresh {
    fn drop(&mut self) {
        // A folder that cannot be removed only leaves files in the temp
        // folder.
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Writes the address file as the script of the image does: a temp file,
/// then a rename, so the reader never sees a part of it.
fn write_status(run: &Path, text: &str) {
    std::fs::create_dir_all(run).expect("the run folder");
    let temp = run.join(".network.toml.test.new");
    std::fs::write(&temp, text).expect("the temp file");
    std::fs::rename(&temp, run.join(STATUS_FILE)).expect("the rename");
}

/// The value of the row `label` on the screen, from the tree of the
/// control protocol.
fn value(rig: &mut Rig, label: &str) -> Option<String> {
    rig.tree()
        .rows
        .into_iter()
        .find(|r| r.label == label)
        .and_then(|r| r.value)
}

/// Runs frames until the row `label` shows `text`. The reader runs in real
/// time, so the bound is real time, not a count of frames.
fn until_row(rig: &mut Rig, label: &str, text: &str) {
    let deadline = Instant::now() + READ_WAIT_MAX;
    while value(rig, label).as_deref() != Some(text) {
        assert!(
            Instant::now() < deadline,
            "the row {label:?} did not show {text:?} in {READ_WAIT_MAX:?}: {:?}",
            rig.tree()
        );
        rig.frames(1);
    }
}

/// Opens `Settings > Network` from home or the quick menu.
fn open_network(rig: &mut Rig) {
    rig.focus("Settings");
    rig.tap(&[Control::A]);
    assert_eq!(rig.screen(), "settings");
    rig.focus("Network");
    rig.tap(&[Control::A]);
    assert_eq!(rig.screen(), "network");
}

/// A game list run on the Network panel.
fn on_the_network_panel(rig: &mut Rig) {
    rig.boot(None);
    rig.frames(2);
    rig.scanned();
    open_network(rig);
}

/// The label and the value of every row on the screen that has a value:
/// the rows of the panel, not the title or the hints.
fn rows(rig: &mut Rig) -> Vec<(String, Option<String>)> {
    rig.tree()
        .rows
        .into_iter()
        .filter(|r| r.value.is_some())
        .map(|r| (r.label, r.value))
        .collect()
}

#[test]
#[ignore = "tier 2"]
fn the_panel_shows_the_address_the_names_and_the_share() {
    let dir = Fresh::new("one-port");
    let mut rig = Rig::new(&dir.0);
    write_status(&rig.paths.run, ONE_PORT);
    on_the_network_panel(&mut rig);
    until_row(&mut rig, "Address eth0", "192.168.0.112");
    let some = |text: &str| Some(text.to_owned());
    assert_eq!(
        rows(&mut rig),
        [
            ("Address eth0".to_owned(), some("192.168.0.112")),
            ("Host name".to_owned(), some("neptron")),
            ("Network name".to_owned(), some("neptron.local")),
            ("Share".to_owned(), some("On")),
            ("Windows".to_owned(), some("\\\\192.168.0.112\\share")),
            ("macOS, Linux".to_owned(), some("smb://192.168.0.112/share")),
            ("Share user".to_owned(), some("neptron")),
            ("Share password".to_owned(), some("neptron")),
        ]
    );
    assert!(rig.tree().rows.iter().all(|r| !r.clipped));
    rig.tap(&[Control::A]);
    assert_eq!(rig.screen(), "network", "accept does nothing");
    rig.tap(&[Control::B]);
    assert_eq!(rig.screen(), "settings");
    assert_eq!(rig.focused().as_deref(), Some("Network"));
    rig.finish();
}

#[test]
#[ignore = "tier 2"]
fn a_late_lease_shows_without_leaving_the_panel() {
    let dir = Fresh::new("late-lease");
    let mut rig = Rig::new(&dir.0);
    write_status(&rig.paths.run, NO_LEASE);
    on_the_network_panel(&mut rig);
    until_row(&mut rig, ADDRESS_LABEL, NO_ADDRESS);
    assert_eq!(
        value(&mut rig, "Windows").as_deref(),
        Some("\\\\neptron.local\\share")
    );

    write_status(&rig.paths.run, ONE_PORT);
    until_row(&mut rig, "Address eth0", "192.168.0.112");
    assert_eq!(rig.screen(), "network");
    assert_eq!(
        value(&mut rig, "Windows").as_deref(),
        Some("\\\\192.168.0.112\\share")
    );

    // The lease goes: the cable is pulled.
    write_status(&rig.paths.run, NO_LEASE);
    until_row(&mut rig, ADDRESS_LABEL, NO_ADDRESS);
    assert_eq!(value(&mut rig, "Address eth0"), None);
    assert_eq!(rig.screen(), "network");
    rig.finish();
}

#[test]
#[ignore = "tier 2"]
fn the_share_switch_hides_the_way_to_the_share_and_the_network_name() {
    let dir = Fresh::new("share-off");
    let mut rig = Rig::new(&dir.0);
    write_status(&rig.paths.run, ONE_PORT);
    std::fs::create_dir_all(&rig.paths.state).expect("the state folder");
    std::fs::write(rig.paths.state.join(SHARE_OFF_FILE), b"").expect("the switch");
    on_the_network_panel(&mut rig);
    until_row(&mut rig, "Share", "Off");
    assert_eq!(value(&mut rig, "Network name"), None, "mdnsd does not run");
    assert_eq!(value(&mut rig, "Windows"), None);
    assert_eq!(value(&mut rig, "Share password"), None);
    rig.finish();
}

#[test]
#[ignore = "tier 2"]
fn a_damaged_or_newer_file_says_not_known_and_stops_nothing() {
    let dir = Fresh::new("damaged");
    let mut rig = Rig::new(&dir.0);
    write_status(&rig.paths.run, "version = 1\nhost_name = \n");
    on_the_network_panel(&mut rig);
    until_row(&mut rig, ADDRESS_LABEL, ADDRESS_NOT_KNOWN);
    assert_eq!(rows(&mut rig).len(), 1, "the address row alone");

    write_status(&rig.paths.run, ONE_PORT);
    until_row(&mut rig, "Address eth0", "192.168.0.112");

    write_status(
        &rig.paths.run,
        "version = 2\nhost_name = \"neptron\"\n[[address]]\nport = \"eth0\"\nipv4 = \"192.168.0.112\"\n",
    );
    until_row(&mut rig, ADDRESS_LABEL, ADDRESS_NOT_KNOWN);
    let notices = &rig.shell().snapshot().notices;
    assert!(
        notices.iter().all(|n| !n.text.contains("network")),
        "no notice for the player: {notices:?}"
    );
    let log = rig.shell().log_lines(64);
    assert!(
        log.iter()
            .any(|l| l.starts_with("network: ") && l.contains("version 2")),
        "the log has the reason: {log:?}"
    );
    rig.finish();
}

#[test]
#[ignore = "tier 2"]
fn with_no_file_the_address_is_not_known() {
    let dir = Fresh::new("no-file");
    let mut rig = Rig::new(&dir.0);
    on_the_network_panel(&mut rig);
    until_row(&mut rig, ADDRESS_LABEL, ADDRESS_NOT_KNOWN);
    assert_eq!(rows(&mut rig).len(), 1);
    rig.finish();
}

#[test]
#[ignore = "tier 2"]
fn in_a_game_the_panel_shows_the_same_rows() {
    let name = test_name(module_path!(), "in_a_game_the_panel_shows_the_same_rows");
    in_child(&name, |dir| {
        let mut rig = Rig::new(dir);
        write_status(&rig.paths.run, ONE_PORT);
        let game = neptron::relaunch::GameRef {
            system: "nes".to_owned(),
            path: "nes/Test Game.nes".to_owned(),
        };
        rig.boot(Some(NextRun {
            game: Some(game),
            notice: None,
        }));
        rig.frames(30);
        rig.tap(&[Control::Select, Control::Start]);
        rig.frames(1);
        assert_eq!(rig.screen(), "quick-menu");
        open_network(&mut rig);
        until_row(&mut rig, "Address eth0", "192.168.0.112");
        assert_eq!(
            value(&mut rig, "macOS, Linux").as_deref(),
            Some("smb://192.168.0.112/share")
        );
        rig.finish();
    });
}

/// Two seconds of the clock of the frame loop, in nanoseconds: two
/// periods of the refresh.
const TWO_SECONDS_NS: u64 = 2_000_000_000;

/// The most frames that 2 s of the clock may take: far above the frames
/// of 2 s in any menu mode.
const FRAMES_MAX: u32 = 1_000;

#[test]
#[ignore = "tier 2"]
fn a_closed_panel_reads_nothing_and_a_new_open_reads_at_once() {
    let dir = Fresh::new("closed");
    let mut rig = Rig::new(&dir.0);
    write_status(&rig.paths.run, NO_LEASE);
    on_the_network_panel(&mut rig);
    until_row(&mut rig, ADDRESS_LABEL, NO_ADDRESS);
    rig.tap(&[Control::B]);
    rig.tap(&[Control::B]);
    assert_eq!(rig.screen(), "home");
    // A read asked for before the close may still come back.
    rig.frames(2);
    let before = rig.shell().network_counts();
    let start = rig.shell().now_ns();
    write_status(&rig.paths.run, ONE_PORT);
    rig.until(FRAMES_MAX, "2 s of the clock", |r| {
        r.shell().now_ns() - start >= TWO_SECONDS_NS
    });
    assert_eq!(
        rig.shell().network_counts(),
        before,
        "no read and no build while the panel is closed"
    );
    open_network(&mut rig);
    let (requests, _) = rig.shell().network_counts();
    assert!(requests > before.0, "a read at the open");
    until_row(&mut rig, "Address eth0", "192.168.0.112");
    rig.finish();
}

#[test]
#[ignore = "tier 2"]
fn with_no_valid_host_name_the_address_and_its_share_path_stay() {
    let dir = Fresh::new("no-host");
    let mut rig = Rig::new(&dir.0);
    write_status(
        &rig.paths.run,
        "version = 1\nhost_name = \"\"\n[[address]]\nport = \"eth0\"\nipv4 = \"192.168.0.112\"\n",
    );
    on_the_network_panel(&mut rig);
    until_row(&mut rig, "Address eth0", "192.168.0.112");
    assert_eq!(value(&mut rig, "Host name"), None);
    assert_eq!(value(&mut rig, "Network name"), None);
    assert_eq!(
        value(&mut rig, "macOS, Linux").as_deref(),
        Some("smb://192.168.0.112/share")
    );
    rig.finish();
}
