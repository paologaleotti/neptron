//! The relaunch file: what the next run of the frontend does after a
//! planned relaunch.
//!
//! One core runs per process, and the display descriptor closes with the
//! display, so the frontend cannot re-execute itself with the display
//! open. It writes this file and exits with [`RELAUNCH_EXIT_CODE`]; the
//! launcher, which holds the descriptor, starts it again at once, and the
//! new run takes the file. The file lives on a RAM file system (`/run`),
//! so a power cut leaves none, and the next start counts as a boot.

use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::atomic;

/// The exit code that asks the launcher for a relaunch. It equals
/// `RELAUNCH_EXIT_CODE` of `neptron-launch`: 75, `EX_TEMPFAIL`.
pub const RELAUNCH_EXIT_CODE: u8 = 75;

/// The format version of the relaunch file.
pub const RELAUNCH_VERSION: u32 = 1;

/// What the next run does.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NextRun {
    /// The game to launch, or `None` for the game list.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub game: Option<GameRef>,
    /// A message for the player in the next run: why a game did not start.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notice: Option<String>,
}

/// A game of the library.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameRef {
    /// The system id, the folder name of `data/systems`.
    pub system: String,
    /// The path of the game under the ROM folder, as the library index
    /// holds it.
    pub path: String,
}

impl GameRef {
    /// Checks that the game stays inside the ROM folder: the system is a
    /// plain id, and the path is relative, with no empty, `.` or `..`
    /// part. The run joins the path to the ROM folder, so anything else
    /// could name a file anywhere.
    ///
    /// # Errors
    ///
    /// The reason as text.
    pub fn check(&self) -> Result<(), String> {
        let id_ok = !self.system.is_empty()
            && self
                .system
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-');
        if !id_ok {
            return Err(format!("the system {:?} is not an id", self.system));
        }
        let parts_ok = !self.path.is_empty()
            && !self.path.contains(['\\', '\0'])
            && self
                .path
                .split('/')
                .all(|part| !part.is_empty() && part != "." && part != "..");
        if !parts_ok {
            return Err(format!(
                "the game path {:?} is not a relative path inside the ROM folder",
                self.path
            ));
        }
        Ok(())
    }
}

/// Why the relaunch file was not used. The run then starts as a new boot
/// with the game list.
#[derive(Debug, thiserror::Error)]
pub enum RelaunchError {
    /// The file cannot be read or removed.
    #[error("cannot take the relaunch file: {0}")]
    Io(#[source] io::Error),
    /// The file is not a relaunch file of this version.
    #[error("the relaunch file is not valid: {0}")]
    Parse(String),
}

#[derive(Serialize, Deserialize)]
struct RelaunchFile {
    version: u32,
    #[serde(flatten)]
    next: NextRun,
}

/// Writes the relaunch file.
///
/// # Errors
///
/// The error of the write.
pub fn write(path: &Path, next: &NextRun) -> io::Result<()> {
    let file = RelaunchFile {
        version: RELAUNCH_VERSION,
        next: next.clone(),
    };
    // A struct of an integer and strings always serialises.
    let text = toml::to_string(&file).unwrap_or_default();
    atomic::write(path, text.as_bytes())
}

/// Reads and removes the relaunch file. `None` when there is none: a new
/// boot.
///
/// # Errors
///
/// [`RelaunchError`]. The file is removed also when it does not parse, so
/// a bad file cannot start every run in a game.
pub fn take(path: &Path) -> Result<Option<NextRun>, RelaunchError> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(RelaunchError::Io(e)),
    };
    std::fs::remove_file(path).map_err(RelaunchError::Io)?;
    let file: RelaunchFile =
        toml::from_str(&text).map_err(|e| RelaunchError::Parse(e.to_string()))?;
    if file.version != RELAUNCH_VERSION {
        return Err(RelaunchError::Parse(format!(
            "version {} is not {RELAUNCH_VERSION}",
            file.version
        )));
    }
    if let Some(game) = &file.next.game {
        game.check().map_err(RelaunchError::Parse)?;
    }
    Ok(Some(file.next))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> std::path::PathBuf {
        std::env::temp_dir()
            .join(format!("neptron-relaunch-{}", std::process::id()))
            .join(name)
    }

    #[test]
    fn a_relaunch_file_is_taken_once() {
        let path = temp("game.toml");
        let next = NextRun {
            game: Some(GameRef {
                system: "nes".to_owned(),
                path: "nes/Game.nes".to_owned(),
            }),
            notice: None,
        };
        write(&path, &next).expect("write");
        assert_eq!(take(&path).expect("take"), Some(next));
        assert_eq!(take(&path).expect("take again"), None);
    }

    #[test]
    fn a_relaunch_to_the_list_has_no_game() {
        let path = temp("list.toml");
        let next = NextRun {
            game: None,
            notice: Some("Game did not start.".to_owned()),
        };
        write(&path, &next).expect("write");
        assert_eq!(take(&path).expect("take"), Some(next));
    }

    #[test]
    fn a_game_path_outside_the_rom_folder_is_refused() {
        for (system, path) in [
            ("nes", "/etc/passwd"),
            ("nes", "nes/../../state/config/global.toml"),
            ("nes", "../nes/Game.nes"),
            ("nes", ""),
            ("nes", "nes/./Game.nes"),
            ("nes", "nes\\..\\..\\Game.nes"),
            ("nes", "nes/Game\0.nes"),
            ("../x", "nes/Game.nes"),
            ("", "nes/Game.nes"),
        ] {
            let path_file = temp("outside.toml");
            let next = NextRun {
                game: Some(GameRef {
                    system: system.to_owned(),
                    path: path.to_owned(),
                }),
                notice: None,
            };
            write(&path_file, &next).expect("write");
            assert!(
                matches!(take(&path_file), Err(RelaunchError::Parse(_))),
                "{system} {path}"
            );
            assert!(!path_file.exists());
        }
    }

    #[test]
    fn a_bad_file_is_removed_and_reported() {
        let path = temp("bad.toml");
        atomic::write(&path, b"version = 9\n").expect("write");
        assert!(matches!(take(&path), Err(RelaunchError::Parse(_))));
        assert!(!path.exists());
    }
}
