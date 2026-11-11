//! A scan on its own thread, owned by a [`ScanWorker`].

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::thread::{self, JoinHandle};

use crate::catalog::Catalog;
use crate::scan::{ScanError, ScanOutcome, ScanProgress, scan};

/// Runs [`scan`] on a thread that it owns.
///
/// No method blocks except [`ScanWorker::join`], and `join` does not block
/// once [`ScanWorker::is_finished`] is true. Drop cancels the scan and joins
/// the thread.
#[derive(Debug)]
pub struct ScanWorker {
    shared: Arc<Shared>,
    thread: Option<JoinHandle<Result<ScanOutcome, ScanError>>>,
}

/// State that the scan thread writes and the owner reads. Atomics keep both
/// sides free of locks and waits, and the owner always reads the newest counts.
#[derive(Debug, Default)]
struct Shared {
    cancel: AtomicBool,
    folders: AtomicU32,
    games: AtomicU32,
}

impl ScanWorker {
    /// Starts a scan of `roms_root` on a new thread.
    pub fn spawn(roms_root: PathBuf, catalog: Catalog) -> Result<Self, ScanError> {
        let shared = Arc::new(Shared::default());
        let theirs = Arc::clone(&shared);
        let thread = thread::Builder::new()
            .name("neptron-scan".into())
            .spawn(move || {
                let mut report = |p: ScanProgress| theirs.store(p);
                scan(&roms_root, &catalog, &theirs.cancel, &mut report)
            })
            .map_err(ScanError::Spawn)?;
        Ok(Self {
            shared,
            thread: Some(thread),
        })
    }

    /// The newest progress of the scan. The two counts can come from two
    /// neighbouring folders, which is close enough for a progress display.
    pub fn progress(&self) -> ScanProgress {
        ScanProgress {
            folders: self.shared.folders.load(Ordering::Relaxed),
            games: self.shared.games.load(Ordering::Relaxed),
        }
    }

    /// Asks the scan to stop before its next directory entry.
    pub fn cancel(&self) {
        self.shared.cancel.store(true, Ordering::Relaxed);
    }

    /// True when the scan thread has ended.
    pub fn is_finished(&self) -> bool {
        self.thread.as_ref().is_none_or(JoinHandle::is_finished)
    }

    /// Waits for the scan thread and returns the result of the scan.
    pub fn join(mut self) -> Result<ScanOutcome, ScanError> {
        // Only this method and drop take the handle, and this method takes
        // `self`, so the handle is always here.
        match self.thread.take() {
            Some(thread) => thread.join().map_err(|_| ScanError::Panicked)?,
            None => Err(ScanError::Panicked),
        }
    }
}

impl Drop for ScanWorker {
    fn drop(&mut self) {
        self.cancel();
        if let Some(thread) = self.thread.take() {
            // The owner gave up the result, so a cancelled or failed scan
            // needs no report here.
            let _ = thread.join();
        }
    }
}

impl Shared {
    fn store(&self, progress: ScanProgress) {
        self.folders.store(progress.folders, Ordering::Relaxed);
        self.games.store(progress.games, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests;
