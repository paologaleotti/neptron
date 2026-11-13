//! The games folder seen again without a restart: the rescan row, the
//! check of the folders on a return to the list, and their error paths.

use std::path::{Path, PathBuf};

use neptron::NextRun;
use neptron::pages::{RESCAN_LABEL, SCANNING_REASON};
use neptron_types::Control;

use crate::rig::Rig;

/// A new, empty folder for one test.
fn fresh(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("neptron-app-{label}-{}", std::process::id()));
    // A leftover of an earlier run with the same process id; absence is fine.
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("the folder");
    dir
}

/// Writes a game file under the games folder of `rig`.
fn add_game(rig: &Rig, path: &str) {
    std::fs::write(rig.paths.roms().join(path), b"a new game").expect("the game");
}

/// A game list run with `Beta Game` next to the rig's `Test Game`, the
/// boot prompt kept and the first scan ended.
fn list_with_two_games(dir: &Path) -> Rig {
    let mut rig = Rig::new(dir);
    add_game(&rig, "nes/Beta Game.nes");
    rig.boot(None);
    rig.frames(2);
    rig.keep();
    rig.until(600, "the first scan", |r| nes_games(r) == 2);
    rig
}

/// The number of NES games on the home screen.
fn nes_games(rig: &mut Rig) -> u32 {
    rig.shell()
        .snapshot()
        .systems
        .iter()
        .find(|s| s.name == "Nintendo Entertainment System")
        .map_or(0, |s| s.game_count)
}

fn notices(rig: &mut Rig) -> Vec<String> {
    rig.shell()
        .snapshot()
        .notices
        .iter()
        .map(|n| n.text.clone())
        .collect()
}

/// Runs frames until a notice with `text` is in the snapshot.
fn until_notice(rig: &mut Rig, text: &str) {
    rig.until(600, text, |r| notices(r).iter().any(|n| n.contains(text)));
}

#[test]
#[ignore = "tier 2"]
fn a_rescan_shows_a_new_game_and_keeps_the_focus() {
    let mut rig = list_with_two_games(&fresh("rescan-focus"));
    assert!(notices(&mut rig).is_empty(), "the first scan says nothing");
    rig.focus("Nintendo Entertainment System");
    rig.tap(&[Control::A]);
    assert_eq!(rig.screen(), "games");
    rig.focus("Test Game");

    // Alpha sorts first, so Test Game moves from row 2 to row 3.
    add_game(&rig, "nes/Alpha Game.nes");
    rig.shell().rescan_games();
    until_notice(&mut rig, "1 game added.");
    assert_eq!(rig.screen(), "notice");
    rig.tap(&[Control::A]);

    assert_eq!(rig.screen(), "games");
    assert_eq!(rig.focused().as_deref(), Some("Test Game"));
    let games: Vec<String> = rig
        .tree()
        .rows
        .into_iter()
        .map(|r| r.label)
        .filter(|l| l.ends_with(" Game"))
        .take(3)
        .collect();
    assert_eq!(games, ["Alpha Game", "Beta Game", "Test Game"]);
    rig.finish();
}

#[test]
#[ignore = "tier 2"]
fn the_rescan_row_of_the_library_panel_finds_a_new_game() {
    let mut rig = list_with_two_games(&fresh("rescan-row"));
    rig.focus("Settings");
    rig.tap(&[Control::A]);
    rig.focus("Library");
    rig.tap(&[Control::A]);
    assert_eq!(rig.screen(), "library");
    rig.focus(RESCAN_LABEL);
    add_game(&rig, "nes/Alpha Game.nes");
    rig.hold(&[Control::A]);
    rig.frames(1);
    rig.release(&[Control::A]);
    assert!(rig.shell().library().scanning());
    // The row says that the scan runs until the frame that takes its end.
    let row = rescan_row(&mut rig);
    assert_eq!(
        row.unavailable.as_deref(),
        Some(SCANNING_REASON),
        "a disabled row"
    );

    until_notice(&mut rig, "1 game added.");
    rig.tap(&[Control::A]);
    assert_eq!(rig.screen(), "library");
    assert_eq!(rig.focused().as_deref(), Some(RESCAN_LABEL));
    assert!(rescan_row(&mut rig).is_enabled(), "enabled after the scan");
    assert_eq!(nes_games(&mut rig), 3);
    rig.finish();
}

/// The rescan row of the page in the snapshot.
fn rescan_row(rig: &mut Rig) -> neptron_ui::snapshot::MenuItem {
    rig.shell()
        .snapshot()
        .settings
        .rows
        .iter()
        .find(|r| r.label == RESCAN_LABEL)
        .cloned()
        .expect("the rescan row")
}

#[test]
#[ignore = "tier 2"]
fn a_return_to_the_list_checks_the_folders() {
    let mut rig = list_with_two_games(&fresh("rescan-check"));
    rig.finish();
    assert!(rig.paths.library_cache().exists(), "the cache of the scan");

    // A copy over the network during the game.
    add_game(&rig, "nes/Alpha Game.nes");
    std::fs::remove_file(rig.paths.roms().join("nes/Beta Game.nes")).expect("remove");
    rig.boot(Some(NextRun {
        game: None,
        notice: None,
    }));
    rig.until(600, "the check", |r| !r.shell().library().scanning());
    rig.frames(1);
    assert_eq!(nes_games(&mut rig), 2);
    // Same size, same system: still two different games.
    assert_eq!(notices(&mut rig), ["1 game added, 1 removed."]);
    rig.finish();

    // Nothing changed: no notice.
    rig.boot(Some(NextRun {
        game: None,
        notice: None,
    }));
    rig.until(600, "the check", |r| !r.shell().library().scanning());
    rig.frames(1);
    assert!(notices(&mut rig).is_empty(), "{:?}", notices(&mut rig));
    rig.finish();
}

#[test]
#[ignore = "tier 2"]
fn a_missing_games_folder_keeps_the_list_and_says_why() {
    let mut rig = list_with_two_games(&fresh("rescan-missing"));
    let roms = rig.paths.roms();
    let away = rig.paths.share.join("roms-away");
    std::fs::rename(&roms, &away).expect("take the folder away");
    rig.shell().rescan_games();
    until_notice(&mut rig, "The game scan failed: cannot read the ROM folder");
    assert_eq!(nes_games(&mut rig), 2, "the list keeps its games");

    std::fs::rename(&away, &roms).expect("put the folder back");
    rig.tap(&[Control::A]);
    rig.shell().rescan_games();
    until_notice(&mut rig, "No games added or removed.");
    rig.finish();
}

#[test]
#[ignore = "tier 2"]
fn a_rescan_asked_while_one_runs_gives_one_result() {
    let mut rig = list_with_two_games(&fresh("rescan-twice"));
    add_game(&rig, "nes/Alpha Game.nes");
    rig.shell().rescan_games();
    rig.shell().rescan_games();
    until_notice(&mut rig, "1 game added.");
    rig.frames(30);
    assert!(!rig.shell().library().scanning());
    assert_eq!(notices(&mut rig), ["1 game added."]);
    rig.finish();
}

#[test]
#[ignore = "tier 2"]
fn a_quit_during_a_rescan_stops_it() {
    let mut rig = list_with_two_games(&fresh("rescan-quit"));
    rig.shell().rescan_games();
    rig.stop.store(true, std::sync::atomic::Ordering::Relaxed);
    assert_eq!(rig.until_end(1), neptron::Outcome::Quit);
    // The end of the run cancels the scan and joins its thread.
    rig.finish();
}

/// Sets the time of the NES folder to an old, fixed time, as a PC that
/// adds a file on exFAT may leave it.
fn age_nes_folder(rig: &Rig) {
    let old = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_600_000_000);
    std::fs::File::open(rig.paths.roms().join("nes"))
        .and_then(|f| f.set_modified(old))
        .expect("set the folder time");
}

#[test]
#[ignore = "tier 2"]
fn a_return_with_no_scan_in_this_power_cycle_reads_every_folder() {
    let dir = fresh("rescan-cycle");
    let mut rig = Rig::new(&dir);
    age_nes_folder(&rig);
    rig.boot(None);
    rig.frames(2);
    rig.keep();
    rig.until(600, "the first scan", |r| nes_games(r) == 1);
    rig.finish();

    // Power off: the run folder is a RAM folder. A PC adds a game and
    // leaves the folder time as it was.
    // The run folder may not exist yet; absence is a cleared folder too.
    let _ = std::fs::remove_dir_all(&rig.paths.run);
    std::fs::create_dir_all(&rig.paths.run).expect("the run folder");
    add_game(&rig, "nes/Alpha Game.nes");
    age_nes_folder(&rig);

    // Power on: a game starts before the first scan ends, so no cache is
    // written in this power cycle.
    rig.boot(None);
    rig.finish();

    // The return from the game.
    rig.boot(Some(NextRun {
        game: None,
        notice: None,
    }));
    rig.until(600, "the scan", |r| !r.shell().library().scanning());
    rig.frames(1);
    assert_eq!(nes_games(&mut rig), 2, "the new game is listed");
    rig.finish();
}
