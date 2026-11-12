//! The save RAM flush: the frame thread copies the core's save RAM into a
//! buffer, and a writer thread writes it to the save file.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError};
use std::thread::{self, JoinHandle};

use neptron_libretro::{SaveFileError, SaveRamError, write_save_file};

/// How often the session flushes the save RAM while the game runs, in
/// nanoseconds. A crash loses at most this much of the game's saves. The
/// file is written only when the save RAM changed, so a game that saves
/// once costs one write; a game that changes its save RAM all the time
/// costs one write of three files in this time, which an SD card takes.
pub const SAVE_RAM_FLUSH_INTERVAL_NS: u64 = 30_000_000_000;

/// The buffers that travel between the frame thread and the writer: one
/// in the writer, one for the next copy. Both channels hold this many, so
/// neither side ever finds a channel full.
const BUFFERS: usize = 2;

/// What a flush did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flush {
    /// The save RAM went to the writer thread.
    Sent,
    /// The save RAM equals the last copy sent or loaded. Nothing to write.
    Unchanged,
    /// Both buffers are with the writer. The next flush tries again.
    Busy,
    /// The copy failed. [`SaveStats::last_error`] has the reason.
    Failed,
    /// The core has no save RAM, or the content has no save file.
    NoSaveRam,
}

/// The counters and the last error of the save RAM writer.
#[derive(Debug, Default)]
pub struct SaveStats {
    /// Flushes that sent the save RAM to the writer.
    pub sent: u64,
    /// Files written.
    pub written: u64,
    /// Flushes with nothing to write.
    pub unchanged: u64,
    /// Flushes that found both buffers with the writer.
    pub busy: u64,
    /// Failed copies or writes.
    pub failures: u64,
    /// The last failure, if any.
    pub last_error: Option<SaveError>,
}

/// A failure of the save RAM flush.
#[derive(Debug, thiserror::Error)]
pub enum SaveError {
    /// The host could not copy the save RAM.
    #[error("cannot copy the save RAM: {0}")]
    Copy(#[from] SaveRamError),
    /// The writer could not write the save file.
    #[error("cannot write the save file: {0}")]
    Write(#[from] SaveFileError),
    /// The writer thread stopped: it panicked.
    #[error("the save RAM writer for {path} stopped")]
    WriterStopped {
        /// The save file.
        path: PathBuf,
    },
}

/// The save RAM writer: a thread that writes save files, and the buffers
/// of the frame thread. The owner calls [`SaveRamWriter::finish`], which
/// stops and joins the thread.
#[derive(Debug)]
pub(crate) struct SaveRamWriter {
    path: PathBuf,
    jobs: Option<SyncSender<Vec<u8>>>,
    done: Receiver<Done>,
    worker: Option<JoinHandle<()>>,
    /// Buffers back from the writer, ready for the next copy.
    spare: Vec<Vec<u8>>,
    /// The save RAM as the file holds it, or will once the writer is done.
    last: Vec<u8>,
    stats: SaveStats,
}

#[derive(Debug)]
struct Done {
    buffer: Vec<u8>,
    result: Result<(), SaveFileError>,
}

impl SaveRamWriter {
    /// Starts the writer for `path`. `loaded` is the save RAM as it was
    /// loaded from the file (empty when there was none), and `ram_bytes`
    /// the size of the core's save RAM, so that no copy allocates.
    ///
    /// # Errors
    ///
    /// The error of [`thread::Builder::spawn`].
    pub(crate) fn start(path: PathBuf, loaded: &[u8], ram_bytes: usize) -> std::io::Result<Self> {
        let (jobs, job_queue) = mpsc::sync_channel::<Vec<u8>>(BUFFERS);
        let (done_tx, done) = mpsc::sync_channel::<Done>(BUFFERS);
        let file = path.clone();
        let worker = thread::Builder::new()
            .name("save-ram".to_owned())
            .spawn(move || write_loop(&file, &job_queue, &done_tx))?;
        let mut last = Vec::with_capacity(ram_bytes);
        last.extend_from_slice(loaded);
        Ok(Self {
            path,
            jobs: Some(jobs),
            done,
            worker: Some(worker),
            spare: (0..BUFFERS)
                .map(|_| Vec::with_capacity(ram_bytes))
                .collect(),
            last,
            stats: SaveStats::default(),
        })
    }

    /// Copies the save RAM with `copy` and sends it to the writer when it
    /// changed. Never blocks.
    pub(crate) fn flush(
        &mut self,
        copy: impl FnOnce(&mut Vec<u8>) -> Result<usize, SaveRamError>,
    ) -> Flush {
        self.collect();
        let Some(buffer) = self.spare.pop() else {
            self.stats.busy += 1;
            return Flush::Busy;
        };
        self.copy_and_send(buffer, copy)
    }

    /// The last flush, at exit: waits for a buffer, sends the save RAM when
    /// it changed, stops the writer and joins it. The frame loop has
    /// ended, so the waits here stop no frame.
    pub(crate) fn finish(
        &mut self,
        copy: impl FnOnce(&mut Vec<u8>) -> Result<usize, SaveRamError>,
    ) -> Flush {
        self.collect();
        let flush = match self.spare.pop().or_else(|| self.wait_for_buffer()) {
            Some(buffer) => self.copy_and_send(buffer, copy),
            None => Flush::Busy,
        };
        self.stop();
        flush
    }

    /// Stops the writer after the jobs it has, and joins it.
    pub(crate) fn stop(&mut self) {
        self.jobs = None;
        if let Some(worker) = self.worker.take()
            && worker.join().is_err()
        {
            self.fail(SaveError::WriterStopped {
                path: self.path.clone(),
            });
        }
        self.collect();
    }

    /// The counters.
    pub(crate) fn stats(&self) -> &SaveStats {
        &self.stats
    }

    /// Takes the counters, for the exit report.
    pub(crate) fn take_stats(&mut self) -> SaveStats {
        std::mem::take(&mut self.stats)
    }

    fn copy_and_send(
        &mut self,
        mut buffer: Vec<u8>,
        copy: impl FnOnce(&mut Vec<u8>) -> Result<usize, SaveRamError>,
    ) -> Flush {
        if let Err(error) = copy(&mut buffer) {
            self.spare.push(buffer);
            self.fail(error.into());
            return Flush::Failed;
        }
        if buffer == self.last {
            self.spare.push(buffer);
            self.stats.unchanged += 1;
            return Flush::Unchanged;
        }
        let Some(jobs) = &self.jobs else {
            self.spare.push(buffer);
            return Flush::Busy;
        };
        self.last.clear();
        self.last.extend_from_slice(&buffer);
        match jobs.try_send(buffer) {
            Ok(()) => {
                self.stats.sent += 1;
                Flush::Sent
            }
            // The channel holds every buffer, so it is never full. A
            // disconnected channel means the writer stopped.
            Err(TrySendError::Full(buffer) | TrySendError::Disconnected(buffer)) => {
                self.spare.push(buffer);
                self.last.clear();
                self.fail(SaveError::WriterStopped {
                    path: self.path.clone(),
                });
                Flush::Busy
            }
        }
    }

    /// Takes the buffers and the results that the writer sent back.
    fn collect(&mut self) {
        loop {
            match self.done.try_recv() {
                Ok(done) => self.take(done),
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => return,
            }
        }
    }

    fn wait_for_buffer(&mut self) -> Option<Vec<u8>> {
        let done = self.done.recv().ok()?;
        self.take(done);
        self.spare.pop()
    }

    fn take(&mut self, done: Done) {
        match done.result {
            Ok(()) => self.stats.written += 1,
            Err(error) => {
                // The file does not hold `last`, so the next flush must
                // write again even when the save RAM did not change.
                self.last.clear();
                self.fail(error.into());
            }
        }
        self.spare.push(done.buffer);
    }

    fn fail(&mut self, error: SaveError) {
        self.stats.failures += 1;
        self.stats.last_error = Some(error);
    }
}

impl Drop for SaveRamWriter {
    fn drop(&mut self) {
        self.stop();
    }
}

/// The writer thread: writes each buffer it gets, and sends it back with
/// the result. It ends when the frame thread drops its sender.
fn write_loop(path: &std::path::Path, jobs: &Receiver<Vec<u8>>, done: &SyncSender<Done>) {
    for buffer in jobs {
        let result = write_save_file(path, &buffer);
        if done.send(Done { buffer, result }).is_err() {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use neptron_libretro::read_save_file;

    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "neptron-session-save-{}-{name}",
            std::process::id()
        ));
        // A leftover of an earlier run with the same process id; absence is
        // fine.
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn ram(bytes: &[u8]) -> impl FnOnce(&mut Vec<u8>) -> Result<usize, SaveRamError> + '_ {
        move |out: &mut Vec<u8>| {
            out.clear();
            out.extend_from_slice(bytes);
            Ok(bytes.len())
        }
    }

    fn read(path: &Path) -> Option<Vec<u8>> {
        read_save_file(path).unwrap()
    }

    #[test]
    fn a_changed_save_ram_is_written_with_a_backup() {
        let dir = temp_dir("changed");
        let path = dir.join("game.srm");
        std::fs::write(&path, [1, 2, 3, 4]).unwrap();
        let mut writer = SaveRamWriter::start(path.clone(), &[1, 2, 3, 4], 4).unwrap();
        assert_eq!(writer.flush(ram(&[1, 2, 3, 4])), Flush::Unchanged);
        assert_eq!(writer.finish(ram(&[9, 2, 3, 4])), Flush::Sent);
        assert_eq!(read(&path), Some(vec![9, 2, 3, 4]));
        assert_eq!(read(&dir.join("game.srm.bak")), Some(vec![1, 2, 3, 4]));
        let stats = writer.stats();
        assert_eq!((stats.sent, stats.written, stats.unchanged), (1, 1, 1));
        assert_eq!(stats.failures, 0);
    }

    #[test]
    fn an_unchanged_save_ram_writes_no_file() {
        let dir = temp_dir("unchanged");
        let path = dir.join("game.srm");
        let mut writer = SaveRamWriter::start(path.clone(), &[0; 8], 8).unwrap();
        assert_eq!(writer.finish(ram(&[0; 8])), Flush::Unchanged);
        assert_eq!(read(&path), None);
    }

    #[test]
    fn the_last_write_wins_and_the_writer_never_blocks() {
        let dir = temp_dir("many");
        let path = dir.join("game.srm");
        let mut writer = SaveRamWriter::start(path.clone(), &[], 1).unwrap();
        for value in 0..50_u8 {
            let flush = writer.flush(ram(&[value]));
            assert!(matches!(flush, Flush::Sent | Flush::Busy), "{flush:?}");
        }
        writer.finish(ram(&[200]));
        assert_eq!(read(&path), Some(vec![200]));
        assert_eq!(writer.stats().failures, 0);
        assert_eq!(writer.stats().written, writer.stats().sent);
    }

    #[test]
    fn a_failed_write_is_counted_and_kept() {
        let dir = temp_dir("fails");
        // A folder at the file's path makes the final rename fail.
        let path = dir.join("game.srm");
        std::fs::create_dir(&path).unwrap();
        std::fs::write(path.join("inside"), [0]).unwrap();
        let mut writer = SaveRamWriter::start(path.clone(), &[], 2).unwrap();
        assert_eq!(writer.flush(ram(&[1, 2])), Flush::Sent);
        while writer.stats().failures == 0 {
            let buffer = writer.wait_for_buffer().unwrap();
            writer.spare.push(buffer);
        }
        writer.finish(ram(&[1, 2]));
        let stats = writer.stats();
        assert_eq!(
            stats.sent, 2,
            "the same bytes are sent again after a failure"
        );
        assert_eq!(stats.failures, 2);
        assert!(matches!(stats.last_error, Some(SaveError::Write(_))));
        assert!(path.join("inside").exists(), "nothing is removed");
    }

    #[test]
    fn a_failed_copy_is_counted() {
        let dir = temp_dir("copy");
        let mut writer = SaveRamWriter::start(dir.join("game.srm"), &[], 2).unwrap();
        let flush = writer.flush(|_| Err(SaveRamError::NoGame));
        assert_eq!(flush, Flush::Failed);
        assert!(matches!(
            writer.stats().last_error,
            Some(SaveError::Copy(SaveRamError::NoGame))
        ));
    }
}
