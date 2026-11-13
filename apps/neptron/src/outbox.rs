//! The writer thread: every file write of the frame loop goes here, so the
//! frame thread does no file I/O.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;
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
/// The results of format request writes the channel back holds. The run
/// sends one request and waits for its result before it sends another.
const REQUEST_RESULTS: usize = 1;

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
    /// Write the guard record, with the place of its last known good.
    Record {
        /// The record.
        record: GuardRecord,
        /// The connector and the monitor the last known good was confirmed
        /// on.
        place: Option<record::Place>,
    },
    /// Write the library index cache.
    Cache {
        /// The index, shared with the library, which never changes it.
        index: Arc<Index>,
        /// The index comes from a scan that read every folder. After the
        /// cache, the writer then marks the run folder (see
        /// [`Places::scanned`]).
        full_scan: bool,
    },
    /// Write the request for the format of the share partition at the next
    /// boot (see [`Places::share_request`]), with the write rule of user
    /// data: the storage script formats only what a whole request names.
    /// Its result comes back through [`Outbox::request_result`], not as a
    /// failure message.
    ShareFormatRequest {
        /// The text of the request file.
        text: String,
    },
}

/// The frame thread's side of the writer thread. Drop or
/// [`Outbox::finish`] stops and joins it.
#[derive(Debug)]
pub struct Outbox {
    jobs: Option<SyncSender<Job>>,
    failures: Receiver<String>,
    requests: Receiver<Result<(), String>>,
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
    /// The marker in the run folder (a RAM folder): a full scan of this
    /// power cycle reached the cache.
    pub scanned: PathBuf,
    /// The format request file on the state partition.
    pub share_request: PathBuf,
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
        let (tx_request, requests) = mpsc::sync_channel(REQUEST_RESULTS);
        let replies = Replies {
            failures: tx_fail,
            requests: tx_request,
        };
        let thread = std::thread::Builder::new()
            .name("neptron-writer".to_owned())
            .spawn(move || write_jobs(&places, &rx, &replies))?;
        Ok(Self {
            jobs: Some(jobs),
            failures,
            requests,
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

    /// The result of the last [`Job::ShareFormatRequest`], once the writer
    /// thread has carried it out. It never waits. A writer thread that has
    /// stopped gives an error: the request will never be written.
    pub fn request_result(&mut self) -> Option<Result<(), String>> {
        match self.requests.try_recv() {
            Ok(result) => Some(result),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(Err("the writer thread stopped".to_owned())),
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

/// The writer thread's channels back to the frame thread.
struct Replies {
    failures: SyncSender<String>,
    requests: SyncSender<Result<(), String>>,
}

fn write_jobs(places: &Places, jobs: &Receiver<Job>, replies: &Replies) {
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
            Job::Record { record: r, place } => record::save(&places.record, r, place.as_ref())
                .map_err(|e| format!("cannot write the guard record: {e}")),
            Job::ShareFormatRequest { text } => {
                let result =
                    crate::atomic::write(&places.share_request, text.as_bytes()).map_err(|e| {
                        format!(
                            "cannot write the format request {}: {e}",
                            places.share_request.display()
                        )
                    });
                // The run sends one request and waits for this result, so
                // the channel has room; a run that has ended reads nothing.
                let _ = replies.requests.try_send(result);
                continue;
            }
            Job::Cache { index, full_scan } => write_cache(&places.cache, index).and_then(|()| {
                if *full_scan {
                    mark_scanned(&places.scanned)
                } else {
                    Ok(())
                }
            }),
        };
        if let Err(message) = result {
            // A full channel drops the message: the frame thread shows the
            // first ones, and the next write tries again.
            let _ = replies.failures.try_send(message);
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

/// Makes the marker of a full scan. It holds nothing: its presence in the
/// RAM folder is the fact.
fn mark_scanned(path: &std::path::Path) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("cannot make the run folder {}: {e}", dir.display()))?;
    }
    std::fs::write(path, b"")
        .map_err(|e| format!("cannot mark the scan in {}: {e}", path.display()))
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
            scanned: dir.join("run").join("library-scanned"),
            share_request: dir.join("state").join("share-format-request"),
        }
    }

    #[test]
    fn the_format_request_is_written_whole() {
        let places = places("request");
        let mut outbox = Outbox::start(places.clone()).expect("start");
        outbox.send(Job::ShareFormatRequest {
            text: "device=/dev/sda4\n".to_owned(),
        });
        let result = loop {
            if let Some(result) = outbox.request_result() {
                break result;
            }
            std::thread::yield_now();
        };
        assert_eq!(result, Ok(()));
        assert!(outbox.finish().is_empty());
        assert_eq!(
            std::fs::read_to_string(&places.share_request).expect("the request"),
            "device=/dev/sda4\n"
        );
    }

    #[test]
    fn a_format_request_that_cannot_be_written_comes_back_as_its_result() {
        let places = places("request-fail");
        let dir = places.share_request.parent().expect("dir").to_owned();
        std::fs::create_dir_all(dir.parent().expect("dir")).expect("dir");
        std::fs::write(&dir, b"a file where the folder must be").expect("file");
        let mut outbox = Outbox::start(places).expect("start");
        outbox.send(Job::ShareFormatRequest {
            text: "device=/dev/sda4\n".to_owned(),
        });
        let result = loop {
            if let Some(result) = outbox.request_result() {
                break result;
            }
            std::thread::yield_now();
        };
        let message = result.expect_err("the write fails");
        assert!(
            message.contains("cannot write the format request"),
            "{message}"
        );
        assert!(outbox.finish().is_empty(), "no second message");
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
            place: None,
        });
        assert!(outbox.finish().is_empty());
        let text = std::fs::read_to_string(places.config.join("global.toml")).expect("global");
        assert!(text.contains("scanline_strength = 30"), "{text}");
        let record = std::fs::read_to_string(&places.record).expect("record");
        assert!(record.contains("failed_boots = 1"), "{record}");
    }

    #[test]
    fn only_the_cache_of_a_full_scan_marks_the_run_folder() {
        let places = places("scanned");
        let outbox = Outbox::start(places.clone()).expect("start");
        let mut outbox = outbox;
        outbox.send(Job::Cache {
            index: Arc::default(),
            full_scan: false,
        });
        assert!(outbox.finish().is_empty());
        assert!(places.cache.exists());
        assert!(!places.scanned.exists());
        let mut outbox = Outbox::start(places.clone()).expect("start");
        outbox.send(Job::Cache {
            index: Arc::default(),
            full_scan: true,
        });
        assert!(outbox.finish().is_empty());
        assert!(places.scanned.exists());
    }

    #[test]
    fn a_failed_cache_write_does_not_mark_the_run_folder() {
        let places = places("scanned-fail");
        let dir = places.cache.parent().expect("dir").to_owned();
        std::fs::create_dir_all(dir.parent().expect("dir")).expect("dir");
        std::fs::write(&dir, b"a file where the folder must be").expect("file");
        let mut outbox = Outbox::start(places.clone()).expect("start");
        outbox.send(Job::Cache {
            index: Arc::default(),
            full_scan: true,
        });
        assert_eq!(outbox.finish().len(), 1);
        assert!(!places.scanned.exists());
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
