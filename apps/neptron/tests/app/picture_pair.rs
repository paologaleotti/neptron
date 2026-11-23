//! The picture pair in the running app: the picture mode and the refresh
//! rate are written together at one level, a reset removes both, and the
//! keep prompt of the quick menu moves the pair of the game whole. Each
//! test counts the writes of the pair in the log of the shell.

use neptron::shell::PAIR_WRITE_LOG;
use neptron_types::Control;
use neptron_ui::snapshot::NoticeId;

use crate::picture_walk::{game_file, global_file, options, the_game, to_video_page};
use crate::rig::{Rig, in_child, in_children, test_name};

/// The file of a settings level, as it is on disk now.
fn level_file(rig: &Rig, file: &str) -> String {
    std::fs::read_to_string(rig.paths.config().join(file)).unwrap_or_default()
}

/// Writes the settings file `file` of the rig with the display lines
/// `lines`.
fn write_level(rig: &Rig, file: &str, lines: &str) {
    let path = rig.paths.config().join(file);
    std::fs::create_dir_all(path.parent().expect("a parent")).expect("the folder");
    std::fs::write(&path, format!("schema_version = 1\n\n[display]\n{lines}\n"))
        .expect("the settings");
}

/// The writes of the pair at `level` that the log holds.
fn pair_writes(rig: &mut Rig, level: &str) -> usize {
    let start = format!("{PAIR_WRITE_LOG} {level}:");
    rig.shell()
        .log_lines(usize::MAX)
        .iter()
        .filter(|l| l.starts_with(&start))
        .count()
}

/// The id of the newest notice in the snapshot.
fn last_notice(rig: &mut Rig) -> Option<NoticeId> {
    rig.shell().snapshot().notices.iter().map(|n| n.id).max()
}

/// The texts of the notices newer than `after`.
fn notices_after(rig: &mut Rig, after: Option<NoticeId>) -> Vec<String> {
    rig.shell()
        .snapshot()
        .notices
        .iter()
        .filter(|n| Some(n.id) > after)
        .map(|n| n.text.clone())
        .collect()
}

/// Picks `option` on the row `label` of the page on show. The option must
/// be selectable, and its tag must say whether the pick also sets the
/// other row to Auto (`resets`).
fn pick(rig: &mut Rig, label: &str, option: &str, resets: bool) {
    let shown = options(rig, label);
    let picked = shown
        .iter()
        .find(|o| o.label == option)
        .unwrap_or_else(|| panic!("{option} in {shown:?}"));
    assert!(picked.enabled, "{picked:?}");
    let tag = picked.detail.ends_with("Auto too") || picked.detail.starts_with("Sets ");
    assert_eq!(tag, resets, "{picked:?}");
    rig.focus(label);
    rig.tap(&[Control::A]);
    rig.focus(option);
    rig.tap(&[Control::A]);
    rig.frames(10);
    rig.wait_for_picture_answers();
}

/// Closes the notices and the option list, back to the quick menu.
fn back_to_quick_menu(rig: &mut Rig) {
    for _ in 0..6 {
        if rig.screen() == "quick-menu" {
            return;
        }
        rig.tap(&[Control::B]);
    }
    assert_eq!(rig.screen(), "quick-menu");
}

/// Opens the keep prompt with Resume, waits until the button `right`
/// steps to the right of "This game" takes a choice, and takes it.
fn keep(rig: &mut Rig, right: u32) {
    back_to_quick_menu(rig);
    rig.focus("Resume");
    rig.tap(&[Control::A]);
    assert_eq!(rig.screen(), "quick-menu/keep");
    rig.until(600, "the answers of the keep prompt", |r| {
        r.shell()
            .snapshot()
            .session
            .as_ref()
            .is_some_and(|s| s.keep_waits == neptron_ui::snapshot::Scopes::NONE)
    });
    for _ in 0..right {
        rig.tap(&[Control::Right]);
    }
    rig.tap(&[Control::A]);
    rig.frames(30);
}

/// One case of the keep prompt: the global file holds `global`, the quick
/// menu picks `option` on the row `label` (whose tag says that the pick
/// also sets the other row to Auto), and the keep moves the pair to the
/// level of the button `right` steps to the right of "This game".
struct KeepCase {
    global: &'static str,
    label: &'static str,
    option: &'static str,
    right: u32,
    /// The file the keep writes, its level, and the lines it must hold.
    file: &'static str,
    level: &'static str,
    lines: [&'static str; 2],
}

/// The keep prompt moves the pair of the game: one write of both keys at
/// the chosen level, judged with the values up to that level, and one
/// write that removes both at the game level. Another game of the system
/// then resolves the pair of that level, which its answers give.
fn keep_moves_the_pair(dir: &std::path::Path, case: &KeepCase) {
    let mut rig = Rig::new(dir);
    write_level(&rig, "global.toml", case.global);
    rig.boot(the_game());
    rig.frames(30);
    to_video_page(&mut rig);
    pick(&mut rig, case.label, case.option, true);
    rig.until(600, "the game file", |r| {
        let game = game_file(r);
        game.contains("style") && game.contains("refresh_policy")
    });
    let (level_before, game_before) = (
        pair_writes(&mut rig, case.level),
        pair_writes(&mut rig, "game"),
    );
    keep(&mut rig, case.right);
    rig.until(600, "the files of the keep", |r| {
        let file = level_file(r, case.file);
        let game = game_file(r);
        case.lines.iter().all(|l| file.contains(l))
            && !game.contains("style")
            && !game.contains("refresh_policy")
    });
    assert_eq!(pair_writes(&mut rig, case.level), level_before + 1);
    assert_eq!(pair_writes(&mut rig, "game"), game_before + 1);
    let level = match case.level {
        "system" => neptron_settings::Level::System,
        _ => neptron_settings::Level::Global,
    };
    let style = rig.shell().config().policy_up_to(level);
    let style = style.expect("the policy of the level");
    let profile = rig.shell().guard().active().monitor.profile().clone();
    let engine = rig.shell().engine().expect("an engine");
    let caps = *engine.odren().caps();
    let source = engine.current().expect("a mode").source;
    let choices = neptron_video::picture_choices_on(
        &source,
        &style,
        neptron_video::ScanlineSettings {
            mode: neptron_types::ScanlineMode::Auto,
            strength_percent: 75,
        },
        &profile,
        &caps,
    )
    .expect("a plan");
    let mode = choices
        .modes
        .iter()
        .find(|(s, _)| *s == style.style)
        .map(|(_, c)| &c.answer);
    assert!(
        matches!(mode, Some(neptron_video::RowAnswer::Plain(_))),
        "{}: {mode:?}",
        case.file
    );
    rig.finish();
}

#[test]
#[ignore = "tier 2"]
fn the_keep_prompt_moves_the_pair_of_the_game() {
    let name = test_name(module_path!(), "the_keep_prompt_moves_the_pair_of_the_game");
    let cases = [
        KeepCase {
            global: "refresh_policy = \"force_50\"",
            label: "Picture mode",
            option: "Game speed",
            right: 1,
            file: "systems/nes.toml",
            level: "system",
            lines: ["style = \"speed\"", "refresh_policy = \"exact\""],
        },
        KeepCase {
            global: "refresh_policy = \"force_50\"",
            label: "Picture mode",
            option: "Game speed",
            right: 2,
            file: "global.toml",
            level: "global",
            lines: ["style = \"speed\"", "refresh_policy = \"exact\""],
        },
        KeepCase {
            global: "style = \"speed\"",
            label: "Refresh rate",
            option: "50 Hz",
            right: 1,
            file: "systems/nes.toml",
            level: "system",
            lines: ["style = \"auto\"", "refresh_policy = \"force_50\""],
        },
        KeepCase {
            global: "style = \"speed\"",
            label: "Refresh rate",
            option: "50 Hz",
            right: 2,
            file: "global.toml",
            level: "global",
            lines: ["style = \"auto\"", "refresh_policy = \"force_50\""],
        },
    ];
    // One process hosts one core, so each case runs in a child of its own.
    in_children(&name, cases.len() as u32, |dir, stage| {
        let root = dir.join(format!("case-{stage}"));
        std::fs::create_dir_all(&root).expect("a folder");
        keep_moves_the_pair(&root, &cases[stage as usize]);
    });
}

/// The system level holds its own refresh rate. A keep "All games" of the
/// pair of the game is refused, with a true text: the game asked to keep
/// its picture, and the system pair would win over the global one. The
/// pair stays at the game level, and no level is written.
#[test]
#[ignore = "tier 2"]
fn a_keep_for_all_games_is_refused_while_the_system_holds_a_pair() {
    let name = test_name(
        module_path!(),
        "a_keep_for_all_games_is_refused_while_the_system_holds_a_pair",
    );
    in_child(&name, |dir| {
        let mut rig = Rig::new(dir);
        write_level(&rig, "systems/nes.toml", "refresh_policy = \"force_50\"");
        rig.boot(the_game());
        rig.frames(30);
        to_video_page(&mut rig);
        pick(&mut rig, "Picture mode", "Game speed", true);
        rig.until(600, "the game file", |r| {
            game_file(r).contains("style = \"speed\"")
        });
        let before = last_notice(&mut rig);
        let writes = pair_writes(&mut rig, "game");
        keep(&mut rig, 2);
        let new = notices_after(&mut rig, before);
        assert!(
            new.contains(
                &"Not kept for all games: this system has its own picture settings. The choice stays for this game."
                    .to_owned()
            ),
            "{new:?}"
        );
        assert_eq!(pair_writes(&mut rig, "global"), 0);
        assert_eq!(pair_writes(&mut rig, "game"), writes, "the game keeps it");
        let config = rig.shell().config();
        assert_eq!(config.text("display.style"), "speed");
        assert_eq!(config.text("display.refresh_policy"), "exact");
        assert!(!global_file(&rig).contains("style"));
        assert!(level_file(&rig, "systems/nes.toml").contains("refresh_policy = \"force_50\""));
        rig.finish();
    });
}

/// A pick that sets the other row to Auto, then a plain pick on the other
/// row, then a keep for the system: the pair of the game moves whole, with
/// one write per level, at most one modeset, and no notice of a reset that
/// the keep did not make.
#[test]
#[ignore = "tier 2"]
fn a_reset_pick_then_a_plain_pick_then_a_keep_moves_one_pair() {
    let name = test_name(
        module_path!(),
        "a_reset_pick_then_a_plain_pick_then_a_keep_moves_one_pair",
    );
    in_child(&name, |dir| {
        let mut rig = Rig::new(dir);
        write_level(&rig, "global.toml", "refresh_policy = \"force_50\"");
        rig.boot(the_game());
        rig.frames(30);
        to_video_page(&mut rig);
        pick(&mut rig, "Picture mode", "Game speed", true);
        back_to_quick_menu(&mut rig);
        to_video_page(&mut rig);
        pick(&mut rig, "Refresh rate", "60 Hz", false);
        rig.until(600, "the game file", |r| {
            let game = game_file(r);
            game.contains("style = \"speed\"") && game.contains("refresh_policy = \"force_60\"")
        });
        let before = last_notice(&mut rig);
        let modes = rig.display.next_mode_call();
        keep(&mut rig, 1);
        rig.until(600, "the files of the keep", |r| {
            let system = level_file(r, "systems/nes.toml");
            system.contains("style = \"speed\"")
                && system.contains("refresh_policy = \"force_60\"")
                && !game_file(r).contains("style")
        });
        assert_eq!(pair_writes(&mut rig, "system"), 1);
        assert!(!game_file(&rig).contains("refresh_policy"));
        assert!(
            rig.display.next_mode_call() <= modes + 1,
            "at most one modeset"
        );
        let new = notices_after(&mut rig, before);
        assert!(new.iter().all(|n| !n.contains("Auto too")), "{new:?}");
        let config = rig.shell().config();
        assert_eq!(config.text("display.style"), "speed");
        assert_eq!(config.text("display.refresh_policy"), "force_60");
        rig.finish();
    });
}

/// A global refresh rate of 50 Hz, a quick menu pick of Game speed (which
/// sets the refresh rate to Auto at the game level), then the Menu button
/// on the Refresh rate row: both keys leave the game level in one write,
/// and the game takes the global pair again.
#[test]
#[ignore = "tier 2"]
fn the_menu_reset_removes_both_keys_of_the_level() {
    let name = test_name(
        module_path!(),
        "the_menu_reset_removes_both_keys_of_the_level",
    );
    in_child(&name, |dir| {
        let mut rig = Rig::new(dir);
        write_level(&rig, "global.toml", "refresh_policy = \"force_50\"");
        rig.boot(the_game());
        rig.frames(30);
        to_video_page(&mut rig);
        pick(&mut rig, "Picture mode", "Game speed", true);
        back_to_quick_menu(&mut rig);
        to_video_page(&mut rig);
        let writes = pair_writes(&mut rig, "game");
        rig.focus("Refresh rate");
        rig.tap(&[Control::X]);
        rig.frames(10);
        assert_eq!(pair_writes(&mut rig, "game"), writes + 1);
        rig.until(600, "the game file", |r| {
            let game = game_file(r);
            !game.contains("style") && !game.contains("refresh_policy")
        });
        let config = rig.shell().config();
        assert_eq!(config.text("display.style"), "auto");
        assert_eq!(config.text("display.refresh_policy"), "force_50");
        rig.finish();
    });
}

/// An old game file holds the refresh rate alone. A pick under the prompt
/// of a monitor change waits with the prompt; the revert takes both keys
/// back; the Menu button on the Refresh rate row then removes the one key
/// of the level, in one write.
#[test]
#[ignore = "tier 2"]
fn a_revert_then_a_reset_leaves_no_key_behind() {
    let name = test_name(module_path!(), "a_revert_then_a_reset_leaves_no_key_behind");
    in_child(&name, |dir| {
        let mut rig = Rig::new(dir);
        write_level(
            &rig,
            "games/nes/Test Game.toml",
            "refresh_policy = \"force_50\"",
        );
        rig.boot(the_game());
        rig.frames(30);
        to_video_page(&mut rig);
        rig.shell()
            .set_from_protocol(
                "display.monitor",
                neptron_settings::Level::Global,
                &serde_json::Value::String("arcade_31".to_owned()),
            )
            .expect("the monitor change");
        rig.frames(2);
        assert_eq!(rig.screen(), "confirm");
        rig.shell()
            .set_from_protocol(
                "display.style",
                neptron_settings::Level::Game,
                &serde_json::Value::String("auto".to_owned()),
            )
            .expect("the pick");
        assert_eq!(
            rig.shell()
                .config()
                .get(neptron_settings::Level::Game, "display.style"),
            Some(&neptron_settings::Value::Text("auto".to_owned()))
        );
        rig.tap(&[Control::B]);
        rig.frames(10);
        assert!(rig.shell().guard().pending().is_none());
        let config = rig.shell().config();
        assert_eq!(
            config.get(neptron_settings::Level::Game, "display.style"),
            None
        );
        assert_eq!(
            config.get(neptron_settings::Level::Game, "display.refresh_policy"),
            Some(&neptron_settings::Value::Text("force_50".to_owned()))
        );
        to_video_page(&mut rig);
        let writes = pair_writes(&mut rig, "game");
        rig.focus("Refresh rate");
        rig.tap(&[Control::X]);
        rig.frames(10);
        assert_eq!(pair_writes(&mut rig, "game"), writes + 1);
        assert!(
            !rig.shell()
                .config()
                .holds_picture(neptron_settings::Level::Game)
        );
        rig.until(600, "the game file", |r| {
            !game_file(r).contains("refresh_policy")
        });
        rig.finish();
    });
}
