//! The writer thread: every file write of the frame loop goes here, so the
//! frame thread does no file I/O.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError};
use std::thread::JoinHandle;

use neptron_library::Index;
use neptron_settings::{Change, Schema, Storage, Target, Value};
use neptron_video::GuardRecord;

use crate::record;

/// The jobs the channel holds. A job that finds it full waits in the
/// backlog of the frame thread and goes at a later frame.
const CHANNEL_JOBS: usize = 16;
/// The failure messages the channel back holds. More are dropped and
/// counted.
const FAILURE_MESSAGES: usize = 16;

/// The settings file of one level.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileTarget {
    /// `global.toml`.
    Global,
    /// `systems/<system>.toml`.
    System(String),
    /// `games/<system>/<game>.toml`.
    Game {
        /// The system.
        system: String,
        /// The game name.
        game: String,
    },
}

/// One write.
#[derive(Debug, Clone, PartialEq)]
pub enum Job {
    /// Set or unset keys in one settings file. `None` unsets.
    Settings {
        /// The file.
        target: FileTarget,
        /// The keys and their new values.
        changes: Vec<(&'static str, Option<Value>)>,
    },
    /// Write the guard record, with the output of its last known good.
    Record {
        /// The record.
        record: GuardRecord,
        /// The connector the last known good was confirmed on.
        output: Option<String>,
    },
    /// Write the library index cache.
    Cache(Box<Index>),
}

/// The frame thread's side of the writer thread. Drop or
/// [`Outbox::finish`] stops and joins it.
#[derive(Debug)]
pub struct Outbox {
    jobs: Option<SyncSender<Job>>,
    failures: Receiver<String>,
    backlog: VecDeque<Job>,
    thread: Option<JoinHandle<()>>,
}

/// Where the writer thread writes.
#[derive(Debug, Clone)]
pub struct Places {
    /// The settings folder.
    pub config: PathBuf,
    /// The guard record file.
    pub record: PathBuf,
    /// The library cache file.
    pub cache: PathBuf,
}

impl Outbox {
    /// Starts the writer thread.
    ///
    /// # Errors
    ///
    /// The thread cannot start.
    pub fn start(places: Places) -> std::io::Result<Self> {
        let (jobs, rx) = mpsc::sync_channel::<Job>(CHANNEL_JOBS);
        let (tx_fail, failures) = mpsc::sync_channel::<String>(FAILURE_MESSAGES);
        let thread = std::thread::Builder::new()
            .name("neptron-writer".to_owned())
            .spawn(move || write_jobs(&places, &rx, &tx_fail))?;
        Ok(Self {
            jobs: Some(jobs),
            failures,
            backlog: VecDeque::new(),
            thread: Some(thread),
        })
    }

    /// Queues a write. It never waits: a full channel keeps the job in the
    /// backlog, in order.
    pub fn send(&mut self, job: Job) {
        self.backlog.push_back(job);
        self.flush();
    }

    /// Moves the backlog into the channel as far as it has room. Call it
    /// once per frame.
    pub fn flush(&mut self) {
        let Some(jobs) = &self.jobs else {
            return;
        };
        while let Some(job) = self.backlog.pop_front() {
            match jobs.try_send(job) {
                Ok(()) => {}
                Err(TrySendError::Full(job)) => {
                    self.backlog.push_front(job);
                    return;
                }
                Err(TrySendError::Disconnected(job)) => {
                    self.backlog.push_front(job);
                    self.jobs = None;
                    return;
                }
            }
        }
    }

    /// The next failure message of the writer thread, if any.
    pub fn failure(&mut self) -> Option<String> {
        match self.failures.try_recv() {
            Ok(message) => Some(message),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => self
                .thread
                .is_some()
                .then(|| "the writer thread stopped; changes are not saved".to_owned())
                .inspect(|_| self.thread = None),
        }
    }

    /// Writes the backlog, stops the thread and waits for every write.
    /// For the end of a run, off the frame loop.
    pub fn finish(mut self) -> Vec<String> {
        let mut messages = Vec::new();
        if let Some(jobs) = self.jobs.take() {
            for job in self.backlog.drain(..) {
                if jobs.send(job).is_err() {
                    break;
                }
            }
        }
        if let Some(thread) = self.thread.take()
            && thread.join().is_err()
        {
            messages.push("the writer thread panicked".to_owned());
        }
        messages.extend(self.failures.try_iter());
        messages
    }
}

impl Drop for Outbox {
    fn drop(&mut self) {
        self.jobs = None;
        if let Some(thread) = self.thread.take() {
            // Drop has no caller to report to; `finish` reports.
            let _ = thread.join();
        }
    }
}

fn write_jobs(places: &Places, jobs: &Receiver<Job>, failures: &SyncSender<String>) {
    let schema = Schema::builtin();
    let storage = Storage::new(&places.config);
    for job in jobs {
        let result = match &job {
            Job::Settings { target, changes } => {
                let changes: Vec<Change<'_>> = changes
                    .iter()
                    .map(|(key, value)| match value {
                        Some(value) => Change::Set {
                            key,
                            value: value.clone(),
                        },
                        None => Change::Unset { key },
                    })
                    .collect();
                storage
                    .apply(&schema, target.as_target(), &changes)
                    .map_err(|e| e.to_string())
            }
            Job::Record { record: r, output } => record::save(&places.record, r, output.as_deref())
                .map_err(|e| format!("cannot write the guard record: {e}")),
            Job::Cache(index) => write_cache(&places.cache, index),
        };
        if let Err(message) = result {
            // A full channel drops the message: the frame thread shows the
            // first ones, and the next write tries again.
            let _ = failures.try_send(message);
        }
    }
}

/// The cache folder must exist before `cache::write`, which makes only the
/// file.
fn write_cache(path: &std::path::Path, index: &Index) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("cannot make the cache folder {}: {e}", dir.display()))?;
    }
    neptron_library::cache::write(path, index)
        .map_err(|e| format!("cannot write the library cache: {e}"))
}

impl FileTarget {
    /// The storage target of the file.
    pub fn as_target(&self) -> Target<'_> {
        match self {
            Self::Global => Target::Global,
            Self::System(system) => Target::System { system },
            Self::Game { system, game } => Target::Game { system, game },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn places(name: &str) -> Places {
        let dir =
            std::env::temp_dir().join(format!("neptron-outbox-{}-{name}", std::process::id()));
        Places {
            config: dir.join("config"),
            record: dir.join("video").join("guard.toml"),
            cache: dir.join("cache").join("library.cache"),
        }
    }

    #[test]
    fn jobs_are_written_in_order_by_the_end() {
        let places = places("order");
        let mut outbox = Outbox::start(places.clone()).expect("start");
        for strength in [10, 20, 30] {
            outbox.send(Job::Settings {
                target: FileTarget::Global,
                changes: vec![("display.scanline_strength", Some(Value::Int(strength)))],
            });
        }
        outbox.send(Job::Record {
            record: GuardRecord {
                failed_boots: 1,
                ..GuardRecord::default()
            },
            output: None,
        });
        assert!(outbox.finish().is_empty());
        let text = std::fs::read_to_string(places.config.join("global.toml")).expect("global");
        assert!(text.contains("scanline_strength = 30"), "{text}");
        let record = std::fs::read_to_string(&places.record).expect("record");
        assert!(record.contains("failed_boots = 1"), "{record}");
    }

    #[test]
    fn a_failed_write_comes_back_as_a_message() {
        let places = places("fail");
        std::fs::create_dir_all(places.config.parent().expect("dir")).expect("dir");
        std::fs::write(&places.config, b"a file where the folder must be").expect("file");
        let mut outbox = Outbox::start(places).expect("start");
        outbox.send(Job::Settings {
            target: FileTarget::Global,
            changes: vec![("display.style", Some(Value::Text("2x".to_owned())))],
        });
        let messages = outbox.finish();
        assert_eq!(messages.len(), 1, "{messages:?}");
    }
}
