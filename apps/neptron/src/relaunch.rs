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

use neptron_input::{DeviceIdentity, PadInfo, Roster, RosterEntry};
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
    /// The pad on each port when the run ended, so the next run keeps the
    /// order of the players. A file with no list gives an empty one.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pads: Vec<PadRecord>,
}

/// The pad of one port, as the relaunch file holds it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PadRecord {
    /// The port, from 0.
    pub port: u8,
    /// The identity of the pad (`DeviceIdentity`), 16 hex digits. TOML
    /// has no unsigned 64-bit integer, so it is a text.
    pub identity: String,
    /// The hash of the path of the pad, 16 hex digits.
    pub path: String,
    /// The name of the pad.
    pub name: String,
    /// False when the pad was away at the end of the run.
    pub present: bool,
}

impl PadRecord {
    /// The record of each port of `roster` that has a pad.
    pub fn list(roster: &Roster) -> Vec<Self> {
        (0u8..)
            .zip(roster.ports())
            .filter_map(|(port, entry)| {
                let entry = entry.as_ref()?;
                Some(Self {
                    port,
                    identity: format!("{:016x}", entry.pad.identity.0),
                    path: format!("{:016x}", entry.pad.path.0),
                    name: entry.pad.name.clone(),
                    present: entry.present,
                })
            })
            .collect()
    }

    /// The roster of `records`. A record with a port out of range or an
    /// identity that is not hex is left out: the list only keeps an order,
    /// and the pads of a lost record take a port by plug order.
    pub fn roster(records: &[Self]) -> Roster {
        let mut roster = Roster::new();
        for record in records {
            let hex = |text: &str| u64::from_str_radix(text, 16).ok().map(DeviceIdentity);
            let (Some(identity), Some(path)) = (hex(&record.identity), hex(&record.path)) else {
                continue;
            };
            let entry = RosterEntry {
                pad: PadInfo {
                    identity,
                    path,
                    name: record.name.clone(),
                },
                present: record.present,
                map: neptron_input::MapKind::Known,
            };
            // A port out of range leaves the record out, as the doc says.
            let _ = roster.set(record.port, Some(entry));
        }
        roster
    }
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
            pads: Vec::new(),
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
            pads: Vec::new(),
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
                pads: Vec::new(),
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

    fn roster_of_two() -> Roster {
        let mut roster = Roster::new();
        for (port, present) in [(0, true), (2, false)] {
            let entry = RosterEntry {
                pad: PadInfo {
                    identity: DeviceIdentity(u64::MAX - u64::from(port)),
                    path: DeviceIdentity(u64::from(port)),
                    name: format!("Pad on {port}"),
                },
                present,
                map: neptron_input::MapKind::Known,
            };
            roster.set(port, Some(entry)).expect("port in range");
        }
        roster
    }

    #[test]
    fn the_pads_cross_the_file_with_their_ports() {
        let path = temp("pads.toml");
        let roster = roster_of_two();
        let next = NextRun {
            game: None,
            notice: None,
            pads: PadRecord::list(&roster),
        };
        assert_eq!(next.pads.len(), 2);
        assert_eq!(next.pads[0].identity, "ffffffffffffffff");
        write(&path, &next).expect("write");
        let taken = take(&path).expect("take").expect("a file");
        assert_eq!(taken, next);
        assert_eq!(PadRecord::roster(&taken.pads), roster);
    }

    #[test]
    fn a_file_with_no_pads_reads_as_an_empty_list() {
        let path = temp("no-pads.toml");
        atomic::write(&path, b"version = 1\n").expect("write");
        let taken = take(&path).expect("take").expect("a file");
        assert!(taken.pads.is_empty());
        assert!(PadRecord::roster(&taken.pads).is_empty());
    }

    #[test]
    fn a_bad_pad_record_is_left_out() {
        let good = PadRecord::list(&roster_of_two()).remove(0);
        let records = [
            PadRecord {
                port: 9,
                ..good.clone()
            },
            PadRecord {
                port: 1,
                identity: "not hex".to_owned(),
                ..good.clone()
            },
            good.clone(),
        ];
        let roster = PadRecord::roster(&records);
        assert_eq!(PadRecord::list(&roster), [good]);
    }

    #[test]
    fn a_bad_file_is_removed_and_reported() {
        let path = temp("bad.toml");
        atomic::write(&path, b"version = 9\n").expect("write");
        assert!(matches!(take(&path), Err(RelaunchError::Parse(_))));
        assert!(!path.exists());
    }
}
