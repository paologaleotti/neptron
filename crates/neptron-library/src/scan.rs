//! The ROM scan: reads directory entries under `share/roms/` and builds an [`Index`].
//!
//! The scan reads directory entries and their types only. It never opens,
//! changes or deletes a file.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::catalog::Catalog;
use crate::index::{Game, Index, MAX_TEXT_BYTES, SystemGames};

/// The deepest folder level under a system folder that the scan reads.
/// The system folder is level 0.
pub const MAX_FOLDER_DEPTH: usize = 8;

/// The most entries that each list of a [`ScanReport`] keeps.
pub const MAX_REPORTED_ENTRIES: usize = 64;

/// How far a scan is. The scan reports it after each folder.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ScanProgress {
    /// Folders read so far.
    pub folders: u32,
    /// Games found so far.
    pub games: u32,
}

/// What the scan saw but did not put in the index.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScanReport {
    /// Folders under `share/roms/` that are not a system folder or alias, sorted.
    pub unknown_folders: Vec<String>,
    /// Unknown folders that did not fit in `unknown_folders`.
    pub more_unknown_folders: u32,
    /// Files with an extension that the system does not accept, and files directly in `share/roms/`.
    pub ignored_files: u32,
    /// Entries whose name starts with `.`.
    pub hidden_entries: u32,
    /// Symbolic links. The scan does not follow them.
    pub symlinks: u32,
    /// Entries whose name is not UTF-8, or whose path is longer than [`MAX_TEXT_BYTES`].
    pub bad_names: u32,
    /// Folders deeper than [`MAX_FOLDER_DEPTH`].
    pub too_deep: u32,
    /// Folders or entries that could not be read, with the error.
    pub unreadable: Vec<Unreadable>,
    /// Errors that did not fit in `unreadable`.
    pub more_unreadable: u32,
}

/// A folder or entry that the scan could not read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unreadable {
    /// The path.
    pub path: PathBuf,
    /// The error, as text.
    pub error: String,
}

/// The result of a complete scan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanOutcome {
    /// Every game found.
    pub index: Index,
    /// What the scan did not put in the index.
    pub report: ScanReport,
}

/// A scan that did not complete.
#[derive(Debug, thiserror::Error)]
pub enum ScanError {
    /// The folder `share/roms/` could not be read.
    #[error("cannot read the ROM folder {path}: {source}")]
    Root {
        /// The ROM folder.
        path: PathBuf,
        /// The error from the file system.
        source: io::Error,
    },
    /// The scan stopped because it was cancelled.
    #[error("the scan was cancelled")]
    Cancelled,
    /// The scan thread could not start.
    #[error("cannot start the scan thread: {0}")]
    Spawn(#[source] io::Error),
    /// The scan thread panicked.
    #[error("the scan thread panicked")]
    Panicked,
}

/// Scans `roms_root` (the `share/roms/` folder) for the games of every
/// system in `catalog`.
///
/// The scan checks `cancel` before each directory entry and returns
/// [`ScanError::Cancelled`] when it is set. It calls `progress` after each
/// folder; `progress` must not block (see [`crate::ScanWorker`]).
///
/// Rules:
/// - A folder directly under `roms_root` is a system folder when its name
///   equals a folder name or alias of the catalog, with no regard to ASCII
///   case. Other folders go in [`ScanReport::unknown_folders`].
/// - The scan reads subfolders of a system folder down to
///   [`MAX_FOLDER_DEPTH`] and puts their games in the same list.
/// - A game is a regular file whose extension the system accepts. Its name
///   is the file name with no extension.
/// - Entries whose name starts with `.` are hidden and skipped. This also
///   skips the `._` files that macOS writes on exFAT.
/// - Symbolic links are skipped, so a link cannot make a loop or show a
///   game twice.
pub fn scan(
    roms_root: &Path,
    catalog: &Catalog,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(ScanProgress),
) -> Result<ScanOutcome, ScanError> {
    let mut scanner = Scanner {
        catalog,
        cancel,
        progress,
        report: ScanReport::default(),
        counts: ScanProgress::default(),
        games: vec![Vec::new(); catalog.systems().len()],
    };
    let mut folders = scanner.read_root(roms_root)?;
    while let Some(folder) = folders.pop() {
        scanner.read_folder(folder, &mut folders)?;
    }
    Ok(scanner.finish())
}

/// Makes `roms_root` and an empty folder for every system in `catalog`
/// that has no folder yet. Returns the folders that it made.
///
/// A system whose folder exists under an alias or in another letter case
/// gets no new folder on a file system that ignores case, such as exFAT.
pub fn ensure_system_folders(roms_root: &Path, catalog: &Catalog) -> io::Result<Vec<PathBuf>> {
    fs::create_dir_all(roms_root)?;
    let mut made = Vec::new();
    for system in catalog.systems() {
        let path = roms_root.join(&system.folder);
        match fs::create_dir(&path) {
            Ok(()) => made.push(path),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e),
        }
    }
    Ok(made)
}

struct Folder {
    path: PathBuf,
    relative: String,
    system: usize,
    depth: usize,
}

struct Scanner<'a> {
    catalog: &'a Catalog,
    cancel: &'a AtomicBool,
    progress: &'a mut dyn FnMut(ScanProgress),
    report: ScanReport,
    counts: ScanProgress,
    games: Vec<Vec<Game>>,
}

impl Scanner<'_> {
    fn read_root(&mut self, roms_root: &Path) -> Result<Vec<Folder>, ScanError> {
        let root_error = |source| ScanError::Root {
            path: roms_root.to_path_buf(),
            source,
        };
        let entries = fs::read_dir(roms_root).map_err(root_error)?;
        let mut folders = Vec::new();
        for entry in entries {
            self.check_cancel()?;
            let entry = entry.map_err(root_error)?;
            let Some((name, kind)) = self.classify(&entry) else {
                continue;
            };
            if !kind.is_dir() {
                bump(&mut self.report.ignored_files);
                continue;
            }
            match self.catalog.system_for_folder(&name) {
                Some(system) => folders.push(Folder {
                    path: entry.path(),
                    relative: name,
                    system,
                    depth: 0,
                }),
                None => push_bounded(
                    &mut self.report.unknown_folders,
                    name,
                    &mut self.report.more_unknown_folders,
                ),
            }
        }
        self.report.unknown_folders.sort();
        self.tick_folder();
        Ok(folders)
    }

    fn read_folder(&mut self, folder: Folder, folders: &mut Vec<Folder>) -> Result<(), ScanError> {
        let entries = match fs::read_dir(&folder.path) {
            Ok(entries) => entries,
            Err(e) => {
                self.unreadable(&folder.path, &e);
                return Ok(());
            }
        };
        for entry in entries {
            self.check_cancel()?;
            match entry {
                Ok(entry) => self.read_entry(&folder, &entry, folders),
                Err(e) => self.unreadable(&folder.path, &e),
            }
        }
        self.tick_folder();
        Ok(())
    }

    fn read_entry(&mut self, folder: &Folder, entry: &fs::DirEntry, folders: &mut Vec<Folder>) {
        let Some((name, kind)) = self.classify(entry) else {
            return;
        };
        let relative = format!("{}/{name}", folder.relative);
        if relative.len() > MAX_TEXT_BYTES {
            bump(&mut self.report.bad_names);
        } else if kind.is_dir() {
            if folder.depth < MAX_FOLDER_DEPTH {
                folders.push(Folder {
                    path: entry.path(),
                    relative,
                    system: folder.system,
                    depth: folder.depth + 1,
                });
            } else {
                bump(&mut self.report.too_deep);
            }
        } else if kind.is_file() {
            self.add_file(folder.system, &name, relative);
        }
    }

    fn add_file(&mut self, system: usize, name: &str, relative: String) {
        let accepted = name.rsplit_once('.').filter(|(stem, ext)| {
            !stem.is_empty()
                && self
                    .catalog
                    .systems()
                    .get(system)
                    .is_some_and(|s| s.accepts_extension(ext))
        });
        match (accepted, self.games.get_mut(system)) {
            (Some((stem, _)), Some(list)) => {
                list.push(Game::new(stem.to_owned(), relative));
                bump(&mut self.counts.games);
            }
            _ => bump(&mut self.report.ignored_files),
        }
    }

    /// The UTF-8 name and the type of an entry that the scan reads further,
    /// or `None` for a hidden entry, a link, a bad name or a type that cannot
    /// be read. The type comes from the directory entry, so links are not followed.
    fn classify(&mut self, entry: &fs::DirEntry) -> Option<(String, fs::FileType)> {
        let Ok(name) = entry.file_name().into_string() else {
            bump(&mut self.report.bad_names);
            return None;
        };
        if name.starts_with('.') {
            bump(&mut self.report.hidden_entries);
            return None;
        }
        let kind = match entry.file_type() {
            Ok(kind) => kind,
            Err(e) => {
                self.unreadable(&entry.path(), &e);
                return None;
            }
        };
        if kind.is_symlink() {
            bump(&mut self.report.symlinks);
            return None;
        }
        Some((name, kind))
    }

    fn check_cancel(&self) -> Result<(), ScanError> {
        if self.cancel.load(Ordering::Relaxed) {
            Err(ScanError::Cancelled)
        } else {
            Ok(())
        }
    }

    fn tick_folder(&mut self) {
        bump(&mut self.counts.folders);
        (self.progress)(self.counts);
    }

    fn unreadable(&mut self, path: &Path, error: &io::Error) {
        let item = Unreadable {
            path: path.to_path_buf(),
            error: error.to_string(),
        };
        push_bounded(
            &mut self.report.unreadable,
            item,
            &mut self.report.more_unreadable,
        );
    }

    fn finish(self) -> ScanOutcome {
        let lists = self
            .catalog
            .systems()
            .iter()
            .zip(self.games)
            .map(|(system, games)| SystemGames::new(system.folder.clone(), games))
            .collect();
        ScanOutcome {
            index: Index::from_systems(lists),
            report: self.report,
        }
    }
}

fn bump(counter: &mut u32) {
    *counter = counter.saturating_add(1);
}

fn push_bounded<T>(list: &mut Vec<T>, item: T, overflow: &mut u32) {
    if list.len() < MAX_REPORTED_ENTRIES {
        list.push(item);
    } else {
        bump(overflow);
    }
}

#[cfg(test)]
mod tests;
