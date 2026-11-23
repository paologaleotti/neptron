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
//! system used the flat files. A game in a folder whose own file is missing
//! gets a copy of the flat file at its first launch, when no game of that
//! name is at the top of the system folder: then the flat file can only be
//! its own. The flat file is never moved, changed or removed.

use std::io;
use std::path::Path;

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
}

impl GameFiles {
    /// The folders and the stem of `game`, with no claim on the flat files.
    /// The first part of the path is the system folder (or an alias), and
    /// the last one the file. A path of one part is a game at the top.
    pub fn of(game: &GameRef) -> Self {
        let parts: Vec<&str> = game.path.split('/').collect();
        let folders = match parts.len() {
            0..=2 => Vec::new(),
            n => parts[1..n - 1].iter().map(|p| (*p).to_owned()).collect(),
        };
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
    let files = GameFiles::of(game);
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
}
