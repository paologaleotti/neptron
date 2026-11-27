//! What the game run checks and prepares before it opens the core of a
//! disc game or of a system with BIOS files (`docs/spec/disc-games.md`,
//! sections 5 and 6): the disc files, the BIOS files, and the system folder
//! of the core. The texts of the notices are here.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use neptron_library::SystemDef;
use neptron_library::bios::{self, BiosEntry, BiosProblem, BiosStatus, BiosSurvey};
use neptron_library::disc::{self, DiscProblem};

/// What the launch of a disc game or a game with BIOS files needs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DiscLaunch {
    /// The discs that the launch adds after the load.
    pub more_discs: Vec<PathBuf>,
    /// The number of each disc for the Disc page, the first disc first.
    /// Empty for a game that is not a disc game.
    pub numbers: Vec<u8>,
    /// The system folder of the core: `share/bios`, or the folder that
    /// holds NEPTRON's copies of the BIOS files.
    pub system_dir: PathBuf,
    /// The BIOS files that the launch found, for the BIOS files page.
    /// `None` when the system has none.
    pub survey: Option<BiosSurvey>,
}

/// Why the game does not start: the notice, and the log lines.
pub type Stop = (String, Vec<String>);

/// The notice when the BIOS copies for the core cannot be written.
pub fn prepare_notice(game: &str) -> String {
    format!("{game} did not start: the BIOS files could not be prepared.")
}

/// The notice of a missing or wrong BIOS file.
pub fn bios_notice(game: &str, system: &str, problem: &BiosProblem) -> String {
    match problem {
        BiosProblem::Missing { file } => format!(
            "{game} did not start: the {system} BIOS is missing. Put {file} in the bios folder."
        ),
        BiosProblem::Wrong { file } => format!(
            "{game} did not start: {file} is not a good {system} BIOS. Put a good copy of {file} in the bios folder."
        ),
    }
}

/// The notice of a missing disc file.
pub fn disc_notice(game: &str, problem: &DiscProblem) -> String {
    match problem {
        DiscProblem::Missing { file, list } => {
            format!("{game} did not start: the disc file {file} is missing. Put it beside {list}.")
        }
        DiscProblem::NoDisc { list } => {
            format!("{game} did not start: {list} names no disc file.")
        }
    }
}

/// Checks and prepares the launch of `content`, a game of `system`, named
/// `game` in the list. `bios_root` is `share/bios`, `core_system` the
/// folder for the BIOS copies of the system. A system with no disc formats
/// and no BIOS files gets `bios_root` and nothing else.
///
/// # Errors
///
/// The notice and the log lines when a disc file or a BIOS file is
/// missing, or the copies cannot be written.
pub fn prepare(
    system: &SystemDef,
    content: &Path,
    game: &str,
    (bios_root, core_system): (&Path, &Path),
) -> Result<DiscLaunch, Stop> {
    let mut launch = DiscLaunch {
        system_dir: bios_root.to_path_buf(),
        ..DiscLaunch::default()
    };
    let format = content
        .extension()
        .and_then(|e| e.to_str())
        .and_then(|e| system.disc_format(e));
    if let Some(format) = format {
        let plan =
            disc::plan_discs(content, format).map_err(|p| (disc_notice(game, &p), Vec::new()))?;
        launch.more_discs = plan.more;
        launch.numbers = plan.numbers;
    }
    if system.bios.is_empty() {
        return Ok(launch);
    }
    let survey = bios::survey_systems(bios_root, &[system], &AtomicBool::new(false));
    let entries = survey.entries(&system.folder);
    let log: Vec<String> = entries.iter().map(entry_line).collect();
    if let Err(problem) = bios::check(entries, disc::region_of(game)) {
        return Err((bios_notice(game, &system.name, &problem), log));
    }
    if let Err(e) = fill_system_folder(core_system, entries) {
        let mut log = log;
        log.push(format!(
            "cannot write the BIOS copies in {}: {e}",
            core_system.display()
        ));
        return Err((prepare_notice(game), log));
    }
    launch.system_dir = core_system.to_path_buf();
    launch.survey = Some(survey);
    Ok(launch)
}

/// Puts a copy of each found BIOS file in `folder` under the name that the
/// core opens (temp file, fsync, rename), unless the copy there is equal
/// (size and CRC-32), and removes a file that carries the name of a BIOS
/// file that is not found now: an old copy of NEPTRON. A file of any other
/// name stays: a core can keep its own files there. A copy, so a core
/// that writes into its system folder never changes a user's file.
pub fn fill_system_folder(folder: &Path, entries: &[BiosEntry]) -> io::Result<()> {
    fs::create_dir_all(folder)?;
    for entry in entries {
        let target = folder.join(&entry.file.file);
        let BiosStatus::Found(source) = &entry.status else {
            match fs::symlink_metadata(&target) {
                Ok(meta) if !meta.is_dir() => fs::remove_file(&target)?,
                _ => {}
            }
            continue;
        };
        let bytes = fs::read(source)?;
        let sum = bios::crc32(&bytes);
        if bytes.len() as u64 != entry.file.size || !entry.file.crc32.contains(&sum) {
            return Err(io::Error::other(format!(
                "{} changed after the check",
                source.display()
            )));
        }
        if same_copy(&target, entry.file.size, sum) {
            continue;
        }
        crate::atomic::write(&target, &bytes)?;
    }
    Ok(())
}

/// True when `path` is a regular file of `size` bytes with the CRC-32
/// `sum`. A file of another size is not read.
fn same_copy(path: &Path, size: u64, sum: u32) -> bool {
    let regular = fs::symlink_metadata(path).is_ok_and(|m| m.is_file() && m.len() == size);
    regular && fs::read(path).is_ok_and(|bytes| bios::crc32(&bytes) == sum)
}

/// The log line of one BIOS file: its status and its path.
fn entry_line(entry: &BiosEntry) -> String {
    let file = &entry.file.file;
    match &entry.status {
        BiosStatus::Found(path) => format!("BIOS {file}: found at {}", path.display()),
        BiosStatus::Wrong(path) => format!("BIOS {file}: wrong file at {}", path.display()),
        BiosStatus::Missing => format!("BIOS {file}: missing"),
    }
}

#[cfg(test)]
mod tests;
