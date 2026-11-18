//! The check of an arcade set before launch, on the thread
//! `neptron-set-check` that the library owns. The thread lists the zips of
//! the set, its parent and its BIOS, and looks for the files of its disk
//! images; `neptron_library::arcade::check_set` decides.

use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};

use neptron_library::arcade::{self, FoundZip, FoundZips, Verdict, ZipEntry};
use neptron_ui::snapshot::GameId;

use super::ArcadeState;

/// The folder under `share/bios` that the core searches before
/// `share/bios` itself.
pub const CORE_BIOS_FOLDER: &str = "fbneo";

/// What the check thread needs.
#[derive(Debug, Clone)]
pub struct CheckJob {
    /// The zip stem of the game.
    pub stem: String,
    /// The folder of the game zip.
    pub game_folder: PathBuf,
    /// `share/bios`.
    pub bios: PathBuf,
    /// The arcade index as the library has it.
    pub arcade: ArcadeState,
    /// The index file, for an index that no scan has read yet.
    pub arcade_file: PathBuf,
}

/// A check on its own thread. Drop cancels it and joins the thread; the
/// check stops before its next zip.
#[derive(Debug)]
pub(super) struct Checking {
    pub(super) game: GameId,
    cancel: Arc<AtomicBool>,
    thread: Option<JoinHandle<Option<(Verdict, ArcadeState)>>>,
}

impl Checking {
    /// Starts the check of `job` for `game`.
    pub(super) fn start(game: GameId, job: CheckJob) -> io::Result<Self> {
        let cancel = Arc::new(AtomicBool::new(false));
        let theirs = Arc::clone(&cancel);
        let thread = thread::Builder::new()
            .name("neptron-set-check".into())
            .spawn(move || run(job, &theirs))?;
        Ok(Self {
            game,
            cancel,
            thread: Some(thread),
        })
    }

    /// True when the thread has ended, so a join does not wait.
    pub(super) fn is_finished(&self) -> bool {
        self.thread.as_ref().is_none_or(JoinHandle::is_finished)
    }

    /// The verdict and the index state of the finished thread. An error
    /// text when the thread panicked.
    pub(super) fn join(mut self) -> Result<(Verdict, ArcadeState), String> {
        let joined = self.thread.take().map(JoinHandle::join);
        match joined {
            Some(Ok(Some(result))) => Ok(result),
            Some(Ok(None)) => Err("the set check was cancelled".to_owned()),
            Some(Err(_)) | None => Err("the set check thread panicked".to_owned()),
        }
    }
}

impl Drop for Checking {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            // The owner gave up the result, so a cancelled check needs no
            // report here.
            let _ = thread.join();
        }
    }
}

/// The work of the thread. `None` when it was cancelled.
fn run(job: CheckJob, cancel: &AtomicBool) -> Option<(Verdict, ArcadeState)> {
    let state = job.arcade.or_read(&job.arcade_file);
    let ArcadeState::Ready(index) = &state else {
        return Some((Verdict::NoIndex, state));
    };
    let Some(wanted) = arcade::wanted(index, &job.stem) else {
        let verdict = arcade::check_set(index, &job.stem, &FoundZips::default());
        return Some((verdict, state));
    };
    let places = places(&job.game_folder, &job.bios);
    let mut found = FoundZips::default();
    for (name, list) in [
        (Some(wanted.set), &mut found.set),
        (wanted.parent, &mut found.parent),
        (wanted.bios, &mut found.bios),
    ] {
        let Some(name) = name else { continue };
        for place in &places {
            if cancel.load(Ordering::Relaxed) {
                return None;
            }
            if let Some(zip) = list_zip(&place.join(format!("{name}.zip"))) {
                list.push(zip);
            }
        }
    }
    found.disks = present_disks(&arcade::disk_paths(index, &job.stem), &job.game_folder);
    let verdict = arcade::check_set(index, &job.stem, &found);
    Some((verdict, state))
}

/// The names of the disk images with a file at one of `paths`, relative
/// to the folder of the game zip. It checks only that a file is there:
/// an image takes hundreds of megabytes, and the core reads it.
fn present_disks(paths: &[arcade::DiskPath], game_folder: &Path) -> Vec<String> {
    let mut present: Vec<String> = Vec::new();
    for path in paths {
        if !present.contains(&path.disk) && game_folder.join(&path.path).is_file() {
            present.push(path.disk.clone());
        }
    }
    present
}

/// The folders the core searches, in its order: the folder of the game,
/// then `share/bios/fbneo`, then `share/bios`.
pub fn places(game_folder: &Path, bios: &Path) -> [PathBuf; 3] {
    [
        game_folder.to_path_buf(),
        bios.join(CORE_BIOS_FOLDER),
        bios.to_path_buf(),
    ]
}

/// The entries of the zip at `path`, or why it cannot be read. `None`
/// when no file is there. It reads only the central directory.
fn list_zip(path: &Path) -> Option<FoundZip> {
    let file = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    match std::fs::metadata(path) {
        Ok(meta) if meta.is_file() => {}
        Ok(_) => return None,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return None,
        Err(e) => {
            return Some(FoundZip {
                file,
                entries: Err(e.to_string()),
            });
        }
    }
    let entries = File::open(path)
        .map_err(|e| e.to_string())
        .and_then(|mut f| neptron_session::zip_entries(&mut f).map_err(|e| e.to_string()))
        .map(|list| {
            list.into_iter()
                .map(|e| ZipEntry {
                    name: e.name,
                    size: u64::from(e.uncompressed_size),
                    crc: e.crc32,
                })
                .collect()
        });
    Some(FoundZip { file, entries })
}

#[cfg(test)]
mod tests;
