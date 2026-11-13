//! The log of a run, off the frame thread.
//!
//! The frame thread hands each line to a bounded channel and never waits:
//! a full channel drops the line and counts it. A writer thread writes the
//! lines to standard error. A write that fails (a full disk, a closed pipe)
//! loses the line and never stops the run, and nothing here panics. On the
//! development image standard error goes to `svlogd`, which bounds the
//! size of the log files.

use std::io::{self, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
use std::thread::JoinHandle;

/// The lines the channel holds before the frame thread drops one.
pub const LOG_QUEUE_LINES: usize = 256;

/// The sending side, for the frame thread. Cloning it is cheap.
#[derive(Debug, Clone)]
pub struct LogLines {
    tx: SyncSender<String>,
    dropped: Arc<AtomicU64>,
}

impl LogLines {
    /// Queues a line. A full channel, or a writer that has ended, drops the
    /// line and counts it; the call never waits.
    pub fn send(&self, line: String) {
        match self.tx.try_send(line) {
            Ok(()) => {}
            Err(TrySendError::Full(_) | TrySendError::Disconnected(_)) => {
                self.dropped.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    /// The lines dropped so far.
    pub fn dropped(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }
}

/// A bounded channel of `capacity` lines: the sending side and the
/// receiving side, for a writer.
pub fn channel(capacity: usize) -> (LogLines, Receiver<String>) {
    let (tx, rx) = mpsc::sync_channel(capacity);
    (
        LogLines {
            tx,
            dropped: Arc::new(AtomicU64::new(0)),
        },
        rx,
    )
}

/// The writer thread. Its owner joins it with [`LogWriter::finish`] after
/// every [`LogLines`] is dropped.
#[derive(Debug)]
pub struct LogWriter {
    thread: Option<JoinHandle<()>>,
}

impl LogWriter {
    /// Starts a writer that writes the lines of `rx` to `out`, and notes
    /// the lines that `lines` dropped.
    pub fn start<W: Write + Send + 'static>(
        rx: Receiver<String>,
        lines: &LogLines,
        out: W,
    ) -> Self {
        let dropped = Arc::clone(&lines.dropped);
        let thread = std::thread::spawn(move || write_lines(&rx, &dropped, out));
        Self {
            thread: Some(thread),
        }
    }

    /// Waits for the writer to write every queued line. It ends when every
    /// sending side is dropped.
    pub fn finish(mut self) {
        if let Some(thread) = self.thread.take() {
            // A writer that panicked has lost its lines; the run is over.
            let _ = thread.join();
        }
    }
}

/// The log of a run: the sending side, and the writer on standard error.
pub fn to_stderr() -> (LogLines, LogWriter) {
    let (lines, rx) = channel(LOG_QUEUE_LINES);
    let writer = LogWriter::start(rx, &lines, io::stderr());
    (lines, writer)
}

fn write_lines<W: Write>(rx: &Receiver<String>, dropped: &AtomicU64, mut out: W) {
    let mut reported = 0;
    for line in rx {
        // A line that cannot be written is lost; the run must not stop or
        // panic for its log.
        let _ = writeln!(out, "{line}");
        let now = dropped.load(Ordering::Relaxed);
        if now > reported {
            // Same reason as above.
            let _ = writeln!(
                out,
                "neptron: {} log lines were dropped: the log queue was full",
                now - reported
            );
            reported = now;
        }
    }
    // Same reason as above.
    let _ = out.flush();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// A writer that keeps what it gets.
    #[derive(Clone, Default)]
    struct Kept(Arc<Mutex<Vec<u8>>>);

    impl Write for Kept {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0.lock().expect("the buffer").extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    /// A writer that fails every write, as a full disk does.
    struct Failing;

    impl Write for Failing {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::Error::other("no space left on device"))
        }
        fn flush(&mut self) -> io::Result<()> {
            Err(io::Error::other("no space left on device"))
        }
    }

    #[test]
    fn a_full_channel_drops_the_line_counts_it_and_never_waits() {
        let (lines, _rx) = channel(2);
        for n in 0..5 {
            lines.send(format!("line {n}"));
        }
        assert_eq!(lines.dropped(), 3);
    }

    #[test]
    fn the_writer_writes_every_line_and_notes_the_dropped_ones() {
        let (lines, rx) = channel(2);
        for n in 0..4 {
            lines.send(format!("line {n}"));
        }
        let kept = Kept::default();
        let writer = LogWriter::start(rx, &lines, kept.clone());
        drop(lines);
        writer.finish();
        let text = String::from_utf8(kept.0.lock().expect("the buffer").clone()).expect("text");
        assert!(text.starts_with("line 0\n"), "{text}");
        assert!(text.contains("line 1\n"), "{text}");
        assert!(
            text.contains("neptron: 2 log lines were dropped: the log queue was full"),
            "{text}"
        );
    }

    #[test]
    fn a_failing_writer_does_not_stop_or_block_the_sender() {
        let (lines, rx) = channel(4);
        let writer = LogWriter::start(rx, &lines, Failing);
        for n in 0..1000 {
            lines.send(format!("line {n}"));
        }
        drop(lines);
        writer.finish();
    }

    #[test]
    fn a_line_after_the_writer_ended_is_counted_not_lost_silently() {
        let (lines, rx) = channel(4);
        drop(rx);
        lines.send("late".to_owned());
        assert_eq!(lines.dropped(), 1);
    }
}
