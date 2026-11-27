//! The per-game files of a game: its save file and its settings file. Both
//! follow the folders of the game below its system's ROM folder, so two
//! games of one name in two folders keep two saves and two settings files.
//! A game at the top of the system folder keeps the flat RetroArch names,
//! `saves/<system>/<stem>.srm` and `games/<system>/<stem>.toml`.
//!
//! Two files of one name in one folder (`Game.sfc` beside `Game.zip`) share
//! one save and one settings file, as in RetroArch.
//!
//! Before the files followed the folders, every game of one name in a
//! system used the flat files. At its first launch, a game in a folder
//! whose own file is missing gets a copy of the flat file, when no game of
//! that name is at the top of the system folder: then the flat file is the
//! one that game used. The flat file is never moved, changed or removed.
//! The copy is made once per game: an empty `<file>.adopted` beside the
//! game's own file records it, so a file that the player removes later
//! does not come back. Two games of one name in two folders each take one
//! copy of the same flat file, as both used it before. A ROM or a folder
//! that is moved or renamed later leaves its save and settings behind.

use std::io;
use std::path::Path;

use neptron_library::disc::save_name;
use neptron_library::{Catalog, SystemDef};

use crate::relaunch::GameRef;

/// Where the per-game files of one game are, and whether its flat files
/// are its own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameFiles {
    /// The folders of the game below its system's ROM folder, in order.
    /// Empty for a game at the top.
    pub folders: Vec<String>,
    /// The file name of the game without its extension.
    pub stem: String,
    /// The flat files may be copied to the game's own files: the game is
    /// in a folder, and no game of its name is at the top of the system.
    pub adopt_flat: bool,
    /// A log line when a folder name of the game is longer than
    /// [`NAME_MAX_BYTES`]: the game then keeps the flat files.
    pub long_folder: Option<String>,
}

/// The longest file name that ext4, the file system of the state
/// partition, takes, in bytes. exFAT takes 255 UTF-16 units, which can be
/// up to 765 bytes of UTF-8.
pub const NAME_MAX_BYTES: usize = 255;

impl GameFiles {
    /// The folders and the stem of `game`, with no claim on the flat files.
    /// The first part of the path is the system folder (or an alias), and
    /// the last one the file. A path of one part is a game at the top.
    pub fn of(game: &GameRef) -> Self {
        let parts: Vec<&str> = game.path.split('/').collect();
        let mut folders: Vec<String> = match parts.len() {
            0..=2 => Vec::new(),
            n => parts[1..n - 1].iter().map(|p| (*p).to_owned()).collect(),
        };
        // A folder that the state partition cannot name would stop the
        // launch, so such a game keeps the flat files and starts.
        let long_folder = folders
            .iter()
            .find(|f| f.len() > NAME_MAX_BYTES)
            .map(|f| {
                format!(
                    "the folder name {f:?} of {} is longer than {NAME_MAX_BYTES} bytes, so the game keeps the save and settings files of the top of its system",
                    game.path
                )
            });
        if long_folder.is_some() {
            folders.clear();
        }
        let file = parts.last().copied().unwrap_or_default();
        let stem = Path::new(file)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(file)
            .to_owned();
        Self {
            folders,
            stem,
            adopt_flat: false,
            long_folder,
        }
    }

    /// The game key of the settings file: the folders and the stem, with
    /// `/` between them. The stem alone for a game at the top.
    pub fn key(&self) -> String {
        let mut key = String::new();
        for folder in &self.folders {
            key.push_str(folder);
            key.push('/');
        }
        key.push_str(&self.stem);
        key
    }

    /// Sets [`GameFiles::adopt_flat`]: true for a game in a folder when no
    /// game of its name is at the top of a ROM folder of its system.
    pub fn with_top_level(mut self, top_level_game: bool) -> Self {
        self.adopt_flat = !self.folders.is_empty() && !top_level_game;
        self
    }
}

/// True when a game file named `stem` is at the top of a ROM folder of
/// `system` under `roms`: directly in a folder whose name is the folder
/// name or an alias of the system in any ASCII case, as the scan finds
/// them, with an extension that the system accepts (in any ASCII case).
/// Only directory entries are read; no file is opened.
///
/// # Errors
///
/// A folder that cannot be read. The caller then copies nothing: a flat
/// file that may belong to another game stays with it.
pub fn top_level_game(roms: &Path, system: &SystemDef, stem: &str) -> io::Result<bool> {
    for entry in std::fs::read_dir(roms)? {
        let entry = entry?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if !system.folder_names().any(|f| f.eq_ignore_ascii_case(name))
            || !entry.file_type()?.is_dir()
        {
            continue;
        }
        for game in std::fs::read_dir(entry.path())? {
            let game = game?;
            let path = game.path();
            let same_stem = path.file_stem().and_then(|s| s.to_str()) == Some(stem);
            let accepted = path
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| system.extensions.iter().any(|x| x.eq_ignore_ascii_case(e)));
            if same_stem && accepted && game.file_type()?.is_file() {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

/// The files of `game`: [`GameFiles::of`], with the flat files adopted
/// when [`top_level_game`] finds no game of its name at the top. An
/// unknown system or a folder that cannot be read adopts nothing, and the
/// reason goes to `issues`.
pub fn find(roms: &Path, catalog: &Catalog, game: &GameRef, issues: &mut Vec<String>) -> GameFiles {
    let mut files = GameFiles::of(game);
    if catalog
        .system(&game.system)
        .is_some_and(SystemDef::has_discs)
    {
        // Every disc of a game shares one save and one settings file
        // (`docs/spec/disc-games.md`, section 8).
        let name = save_name(&files.stem);
        if !name.is_empty() {
            files.stem = name;
        }
    }
    if files.folders.is_empty() {
        return files;
    }
    let Some(system) = catalog.system(&game.system) else {
        return files;
    };
    match top_level_game(roms, system, &files.stem) {
        Ok(top) => files.with_top_level(top),
        Err(e) => {
            issues.push(format!(
                "cannot read the ROM folders of {} ({e}); the earlier save and settings of {} are not copied",
                game.system, files.stem
            ));
            files
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game(path: &str) -> GameRef {
        GameRef {
            system: "snes".to_owned(),
            path: path.to_owned(),
        }
    }

    #[test]
    fn a_game_at_the_top_keeps_the_flat_names() {
        let files = GameFiles::of(&game("snes/Chrono Trigger.sfc"));
        assert_eq!(files.folders, Vec::<String>::new());
        assert_eq!(files.key(), "Chrono Trigger");
        assert!(!files.with_top_level(false).adopt_flat, "nothing to adopt");
        assert_eq!(GameFiles::of(&game("Lone.sfc")).key(), "Lone");
    }

    #[test]
    fn a_game_in_folders_gets_them_in_its_key() {
        let files = GameFiles::of(&game("snes/hacks/fr/Chrono Trigger (fr) [!].zip"));
        assert_eq!(files.folders, ["hacks", "fr"]);
        assert_eq!(files.stem, "Chrono Trigger (fr) [!]");
        assert_eq!(files.key(), "hacks/fr/Chrono Trigger (fr) [!]");
    }

    #[test]
    fn odd_names_stay_as_they_are() {
        for (path, key) in [
            ("snes/é ü/漢字.sfc", "é ü/漢字"),
            ("snes/a;b&c$(d)'e\"/x.y.sfc", "a;b&c$(d)'e\"/x.y"),
            ("snes/  /#~%.sfc", "  /#~%"),
            ("snes/no extension/Game", "no extension/Game"),
        ] {
            assert_eq!(GameFiles::of(&game(path)).key(), key, "{path}");
        }
    }

    /// ext4 takes names of 255 bytes at most; exFAT takes 255 UTF-16 units,
    /// up to 765 bytes of UTF-8. A game in a folder with a longer name
    /// keeps the flat files and starts.
    #[test]
    fn a_folder_name_too_long_for_the_state_partition_keeps_the_flat_files() {
        let longest = "a".repeat(NAME_MAX_BYTES);
        let files = GameFiles::of(&game(&format!("snes/{longest}/Game.sfc")));
        assert_eq!(files.folders, std::slice::from_ref(&longest));
        assert_eq!(files.long_folder, None);
        for long in ["a".repeat(NAME_MAX_BYTES + 1), "é".repeat(128)] {
            let files = GameFiles::of(&game(&format!("snes/ok/{long}/Game.sfc")));
            assert_eq!(files.folders, Vec::<String>::new(), "{long}");
            assert_eq!(files.key(), "Game");
            assert!(!files.clone().with_top_level(false).adopt_flat);
            let note = files.long_folder.expect("a log line");
            assert!(note.contains("longer than 255 bytes"), "{note}");
        }
    }

    #[test]
    fn only_a_game_in_a_folder_with_none_of_its_name_on_top_adopts() {
        let files = GameFiles::of(&game("snes/hacks/Chrono Trigger.sfc"));
        assert!(files.clone().with_top_level(false).adopt_flat);
        assert!(!files.with_top_level(true).adopt_flat);
    }

    fn snes() -> SystemDef {
        Catalog::builtin()
            .expect("the catalog")
            .system("snes")
            .expect("snes")
            .clone()
    }

    fn roms(name: &str, files: &[&str]) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("neptron-game-files-{}-{name}", std::process::id()));
        // A leftover of an earlier run with the same process id; absence is
        // fine.
        let _ = std::fs::remove_dir_all(&dir);
        for file in files {
            let path = dir.join(file);
            std::fs::create_dir_all(path.parent().expect("a parent")).expect("the folder");
            if !file.ends_with('/') {
                std::fs::write(&path, b"").expect("the file");
            }
        }
        std::fs::create_dir_all(&dir).expect("the roms folder");
        dir
    }

    #[test]
    fn a_game_on_top_is_found_in_any_folder_of_the_system() {
        let system = snes();
        let stem = "Chrono Trigger";
        for (name, files, found) in [
            ("none", &["snes/hacks/Chrono Trigger.sfc"][..], false),
            ("top", &["snes/Chrono Trigger.sfc"][..], true),
            ("case", &["SNES/Chrono Trigger.SFC"][..], true),
            ("zip", &["snes/Chrono Trigger.zip"][..], true),
            ("other-ext", &["snes/Chrono Trigger.txt"][..], false),
            ("other-stem", &["snes/chrono trigger.sfc"][..], false),
            ("other-system", &["nes/Chrono Trigger.sfc"][..], false),
            ("folder", &["snes/Chrono Trigger.sfc/"][..], false),
        ] {
            let dir = roms(name, files);
            assert_eq!(
                top_level_game(&dir, &system, stem).expect("the read"),
                found,
                "{name}"
            );
            // The folder is the test's own; a failed removal leaves a temp
            // folder.
            let _ = std::fs::remove_dir_all(&dir);
        }
    }

    #[test]
    fn a_folder_that_cannot_be_read_adopts_nothing() {
        let catalog = Catalog::builtin().expect("the catalog");
        let mut issues = Vec::new();
        let files = find(
            Path::new("/nonexistent/roms"),
            &catalog,
            &game("snes/hacks/Chrono Trigger.sfc"),
            &mut issues,
        );
        assert!(!files.adopt_flat);
        assert_eq!(issues.len(), 1, "{issues:?}");
    }

    #[test]
    fn every_disc_of_a_game_shares_one_save_and_settings_name() {
        let catalog = crate::test_disc::catalog();
        let dir = crate::test_disc::Dir::new("game-files-discs");
        let disc = |path: &str| GameRef {
            system: "testdisc".to_owned(),
            path: path.to_owned(),
        };
        let mut issues = Vec::new();
        let files = find(
            &dir.0,
            &catalog,
            &disc("testdisc/rpg/Saga (USA) (Disc 2).cue"),
            &mut issues,
        );
        assert_eq!(files.key(), "rpg/Saga (USA)");
        let files = find(
            &dir.0,
            &catalog,
            &disc("testdisc/Saga (USA).m3u"),
            &mut issues,
        );
        assert_eq!(files.key(), "Saga (USA)");
        let files = find(
            &dir.0,
            &catalog,
            &disc("testdisc/(Disc 1).chd"),
            &mut issues,
        );
        assert_eq!(files.key(), "(Disc 1)", "an empty name keeps the stem");
        let snes = Catalog::builtin().unwrap();
        let cart = GameRef {
            system: "snes".to_owned(),
            path: "snes/Game (Disc 1).sfc".to_owned(),
        };
        assert_eq!(
            find(&dir.0, &snes, &cart, &mut issues).key(),
            "Game (Disc 1)"
        );
    }
}
