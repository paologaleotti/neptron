//! BIOS files: where they are in `share/bios`, whether each one is a good
//! copy, and whether a game can start.
//!
//! A file is checked by its size and its CRC-32, never by its name. The
//! survey reads directory entries and the files whose size is a BIOS size.
//! It never changes a file.

use std::collections::VecDeque;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::catalog::{Catalog, SystemDef};
use crate::disc::Region;

/// The largest BIOS file that a system file can name, in bytes.
pub const BIOS_MAX_BYTES: u64 = 16 * 1024 * 1024;

/// The deepest folder level below `share/bios` that the survey reads.
/// `share/bios` is level 0.
pub const BIOS_MAX_DEPTH: usize = 3;

/// The most directory entries that the survey reads.
pub const BIOS_MAX_ENTRIES: usize = 4096;

/// One BIOS file that the core of a system needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BiosFile {
    /// The file name that the core opens.
    pub file: String,
    /// The region of the BIOS. `None`: every game needs the file.
    pub region: Option<Region>,
    /// The size of a good file, in bytes.
    pub size: u64,
    /// The CRC-32 of each good dump.
    pub crc32: Vec<u32>,
}

/// What the survey found for one BIOS file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BiosStatus {
    /// A good copy, at this path.
    Found(PathBuf),
    /// No good copy; a file of the wanted name is at this path.
    Wrong(PathBuf),
    /// No good copy and no file of the wanted name.
    Missing,
}

/// One BIOS file of a system and its status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BiosEntry {
    /// The file the system needs.
    pub file: BiosFile,
    /// What the survey found.
    pub status: BiosStatus,
}

/// The BIOS files of one system and their status, in file order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemBios {
    /// The system id.
    pub system: String,
    /// The display name of the system.
    pub name: String,
    /// The files, in the order of the system file.
    pub entries: Vec<BiosEntry>,
}

/// The status of the BIOS files of every system that has some, in catalog
/// order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BiosSurvey {
    systems: Vec<SystemBios>,
    /// Folders or files that could not be read, as text, for the log.
    pub unreadable: Vec<String>,
}

/// Why a game cannot start: the BIOS file that the notice names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BiosProblem {
    /// No good copy and no file of the name.
    Missing {
        /// The file name that the core wants.
        file: String,
    },
    /// No good copy; a file of the name is a wrong one.
    Wrong {
        /// The file name that the core wants.
        file: String,
    },
}

/// The CRC-32 (IEEE) of `bytes`, as the system files give it.
pub fn crc32(bytes: &[u8]) -> u32 {
    crate::cache::crc32::crc32(bytes)
}

/// Surveys `bios_root` (`share/bios`) for the BIOS files of every system
/// of `catalog` that has some. A missing folder finds nothing. The survey
/// stops early, with what it has, when `cancel` is set.
pub fn survey(bios_root: &Path, catalog: &Catalog, cancel: &AtomicBool) -> BiosSurvey {
    let systems: Vec<&SystemDef> = catalog.systems().iter().collect();
    survey_systems(bios_root, &systems, cancel)
}

/// Surveys `bios_root` as [`survey`] does, for `systems` only: the launch
/// of a game checks the files of its own system and reads no file of a
/// size that only another system wants.
pub fn survey_systems(bios_root: &Path, systems: &[&SystemDef], cancel: &AtomicBool) -> BiosSurvey {
    let wanted: Vec<&BiosFile> = systems.iter().flat_map(|s| &s.bios).collect();
    let mut out = BiosSurvey::default();
    let files = if wanted.is_empty() {
        Vec::new()
    } else {
        walk(bios_root, cancel, &mut out.unreadable)
    };
    let mut sums: Vec<(usize, u32)> = Vec::new();
    for (i, file) in files.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        if !wanted.iter().any(|w| w.size == file.size) {
            continue;
        }
        match read_bounded(&file.path, file.size) {
            Ok(Some(bytes)) => sums.push((i, crc32(&bytes))),
            Ok(None) => {}
            Err(e) => out.unreadable.push(format!("{}: {e}", file.path.display())),
        }
    }
    out.systems = systems
        .iter()
        .filter(|s| !s.bios.is_empty())
        .map(|s| SystemBios {
            system: s.folder.clone(),
            name: s.name.clone(),
            entries: s
                .bios
                .iter()
                .map(|bios| BiosEntry {
                    file: bios.clone(),
                    status: status_of(bios, &files, &sums),
                })
                .collect(),
        })
        .collect();
    out
}

impl BiosSurvey {
    /// Every system with BIOS files, in catalog order.
    pub fn systems(&self) -> &[SystemBios] {
        &self.systems
    }

    /// The BIOS files of one system. Empty for a system with none.
    pub fn entries(&self, system: &str) -> &[BiosEntry] {
        self.systems
            .iter()
            .find(|s| s.system == system)
            .map_or(&[], |s| s.entries.as_slice())
    }
}

/// Whether a game of `region` can start with these BIOS files
/// (`docs/spec/disc-games.md`, section 6.4): every file with no region is
/// found, and, when some files have a region, one of them is found. The
/// error names the file of the notice.
pub fn check(entries: &[BiosEntry], region: Option<Region>) -> Result<(), BiosProblem> {
    if let Some(entry) = entries
        .iter()
        .find(|e| e.file.region.is_none() && !e.is_found())
    {
        return Err(entry.problem());
    }
    let regional: Vec<&BiosEntry> = entries.iter().filter(|e| e.file.region.is_some()).collect();
    if regional.is_empty() || regional.iter().any(|e| e.is_found()) {
        return Ok(());
    }
    let named = regional
        .iter()
        .find(|e| region.is_some() && e.file.region == region)
        .or_else(|| regional.first());
    named.map_or(Ok(()), |entry| Err(entry.problem()))
}

impl BiosEntry {
    /// True when a good copy is found.
    pub fn is_found(&self) -> bool {
        matches!(self.status, BiosStatus::Found(_))
    }

    fn problem(&self) -> BiosProblem {
        let file = self.file.file.clone();
        match self.status {
            BiosStatus::Wrong(_) => BiosProblem::Wrong { file },
            BiosStatus::Found(_) | BiosStatus::Missing => BiosProblem::Missing { file },
        }
    }
}

/// A regular file of the survey.
struct Seen {
    path: PathBuf,
    name: String,
    size: u64,
}

fn status_of(bios: &BiosFile, files: &[Seen], sums: &[(usize, u32)]) -> BiosStatus {
    let good = sums.iter().find(|(i, sum)| {
        files.get(*i).is_some_and(|f| f.size == bios.size) && bios.crc32.contains(sum)
    });
    if let Some(file) = good.and_then(|(i, _)| files.get(*i)) {
        return BiosStatus::Found(file.path.clone());
    }
    match files
        .iter()
        .find(|f| f.name.eq_ignore_ascii_case(&bios.file))
    {
        Some(file) => BiosStatus::Wrong(file.path.clone()),
        None => BiosStatus::Missing,
    }
}

/// The regular files below `root`, level by level, each folder's names in
/// byte order. Links and hidden entries are skipped.
fn walk(root: &Path, cancel: &AtomicBool, unreadable: &mut Vec<String>) -> Vec<Seen> {
    let mut files = Vec::new();
    let mut folders = VecDeque::from([(root.to_path_buf(), 0usize)]);
    let mut entries_read = 0usize;
    while let Some((folder, depth)) = folders.pop_front() {
        let read = match fs::read_dir(&folder) {
            Ok(read) => read,
            Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
            Err(e) => {
                unreadable.push(format!("{}: {e}", folder.display()));
                continue;
            }
        };
        let mut names = Vec::new();
        for entry in read {
            if cancel.load(Ordering::Relaxed) || entries_read >= BIOS_MAX_ENTRIES {
                return files;
            }
            entries_read += 1;
            let Ok(entry) = entry else { continue };
            let Ok(name) = entry.file_name().into_string() else {
                continue;
            };
            if name.starts_with('.') {
                continue;
            }
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            names.push((name, kind, entry.path()));
        }
        names.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
        for (name, kind, path) in names {
            if kind.is_dir() && depth < BIOS_MAX_DEPTH {
                folders.push_back((path, depth + 1));
            } else if kind.is_file() {
                // `symlink_metadata` does not follow a link that replaced
                // the file after the read of the folder.
                if let Ok(meta) = fs::symlink_metadata(&path)
                    && meta.is_file()
                {
                    files.push(Seen {
                        path,
                        name,
                        size: meta.len(),
                    });
                }
            }
        }
    }
    files
}

/// The bytes of a file that should hold `size` bytes. `None` when it holds
/// another number now: it changed since the survey read its size.
fn read_bounded(path: &Path, size: u64) -> io::Result<Option<Vec<u8>>> {
    let file = fs::File::open(path)?;
    let mut bytes = Vec::new();
    file.take(size.saturating_add(1)).read_to_end(&mut bytes)?;
    Ok((bytes.len() as u64 == size).then_some(bytes))
}

#[cfg(test)]
mod tests;
