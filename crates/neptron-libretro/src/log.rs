//! Log records of the core and of the host, in a queue that is made once
//! and never allocates after that. The error-level records also go to a
//! small store of their own, so a noisy call that fills the queue does not
//! lose the lines that say why it failed.

use std::fmt::{self, Write as _};

/// The most log records kept between two calls.
pub const LOG_CAPACITY: usize = 64;
/// The most error-level records kept between two calls, in their own
/// store. The store keeps the first ones: the first error of a failed load
/// names its cause, and the later ones follow from it.
pub const ERROR_LOG_CAPACITY: usize = 16;
/// The longest text of one record, in bytes. The C shim cuts at the same
/// length.
pub const LOG_TEXT_MAX_BYTES: usize = 1024;

/// The level of a log record, as libretro defines them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum LogLevel {
    /// Detail for debugging.
    Debug,
    /// Normal information.
    Info,
    /// Something is wrong but the core goes on.
    Warn,
    /// Something failed.
    Error,
}

/// Who wrote a log record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LogSource {
    /// The core, through the log interface.
    Core,
    /// The host, about the core: an unknown or refused environment call.
    Host,
}

/// One log line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogRecord {
    /// The level.
    pub level: LogLevel,
    /// Who wrote it.
    pub source: LogSource,
    /// The text, without a trailing newline, at most
    /// [`LOG_TEXT_MAX_BYTES`] bytes.
    pub text: String,
}

impl LogLevel {
    /// The level for a `retro_log_level` value. An unknown value is info.
    pub(crate) fn from_core(level: u32) -> Self {
        match level {
            0 => LogLevel::Debug,
            2 => LogLevel::Warn,
            3 => LogLevel::Error,
            _ => LogLevel::Info,
        }
    }
}

/// The queue of every record, and the store of the error-level ones.
#[derive(Debug)]
pub(crate) struct LogQueue {
    all: Slots,
    errors: Slots,
}

/// Records made once, each with its text buffer for the life of the queue.
/// A record past the capacity is counted and dropped.
#[derive(Debug)]
struct Slots {
    records: Vec<LogRecord>,
    len: usize,
    dropped: usize,
}

impl LogQueue {
    pub(crate) fn new() -> Self {
        Self {
            all: Slots::new(LOG_CAPACITY),
            errors: Slots::new(ERROR_LOG_CAPACITY),
        }
    }

    pub(crate) fn clear(&mut self) {
        self.all.clear();
        self.errors.clear();
    }

    pub(crate) fn records(&self) -> &[LogRecord] {
        self.all.records()
    }

    pub(crate) fn dropped(&self) -> usize {
        self.all.dropped
    }

    /// The error-level records, also those the full queue dropped.
    pub(crate) fn errors(&self) -> &[LogRecord] {
        self.errors.records()
    }

    /// Adds a record with text from a core: bytes that are not UTF-8
    /// become U+FFFD, and a trailing newline is removed.
    pub(crate) fn push_bytes(&mut self, level: LogLevel, source: LogSource, text: &[u8]) {
        if let Some(record) = self.all.next(level, source) {
            fill_bytes(&mut record.text, text);
        }
        if level == LogLevel::Error
            && let Some(record) = self.errors.next(level, source)
        {
            fill_bytes(&mut record.text, text);
        }
    }

    /// Adds a record with formatted text, with no allocation.
    pub(crate) fn push_fmt(&mut self, level: LogLevel, source: LogSource, args: fmt::Arguments) {
        if let Some(record) = self.all.next(level, source) {
            // A full buffer cuts the text; that is the intended limit.
            let _ = Bounded(&mut record.text).write_fmt(args);
        }
        if level == LogLevel::Error
            && let Some(record) = self.errors.next(level, source)
        {
            // As above.
            let _ = Bounded(&mut record.text).write_fmt(args);
        }
    }
}

impl Slots {
    fn new(capacity: usize) -> Self {
        let records = (0..capacity)
            .map(|_| LogRecord {
                level: LogLevel::Info,
                source: LogSource::Host,
                text: String::with_capacity(LOG_TEXT_MAX_BYTES),
            })
            .collect();
        Self {
            records,
            len: 0,
            dropped: 0,
        }
    }

    fn clear(&mut self) {
        self.len = 0;
        self.dropped = 0;
    }

    fn records(&self) -> &[LogRecord] {
        self.records.get(..self.len).unwrap_or_default()
    }

    fn next(&mut self, level: LogLevel, source: LogSource) -> Option<&mut LogRecord> {
        let Some(record) = self.records.get_mut(self.len) else {
            self.dropped = self.dropped.saturating_add(1);
            return None;
        };
        self.len += 1;
        record.level = level;
        record.source = source;
        record.text.clear();
        Some(record)
    }
}

/// Writes the text of a core into `out`: bytes that are not UTF-8 become
/// U+FFFD, and a trailing newline is removed.
fn fill_bytes(out: &mut String, text: &[u8]) {
    let mut bounded = Bounded(out);
    for chunk in text.utf8_chunks() {
        if bounded.write_str(chunk.valid()).is_err() {
            break;
        }
        if !chunk.invalid().is_empty() && bounded.write_char('\u{FFFD}').is_err() {
            break;
        }
    }
    let trimmed = out.trim_end_matches(['\n', '\r']).len();
    out.truncate(trimmed);
}

/// Writes into a string up to [`LOG_TEXT_MAX_BYTES`] and fails after that,
/// so the string never grows past the capacity it was made with.
struct Bounded<'a>(&'a mut String);

impl fmt::Write for Bounded<'_> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let room = LOG_TEXT_MAX_BYTES.saturating_sub(self.0.len());
        if s.len() <= room {
            self.0.push_str(s);
            return Ok(());
        }
        let mut cut = room;
        while !s.is_char_boundary(cut) {
            cut -= 1;
        }
        self.0.push_str(s.get(..cut).unwrap_or_default());
        Err(fmt::Error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_keep_their_order_and_lose_the_newline() {
        let mut q = LogQueue::new();
        q.push_bytes(LogLevel::Error, LogSource::Core, b"failed\n");
        q.push_fmt(
            LogLevel::Warn,
            LogSource::Host,
            format_args!("command {:#x}", 0x24E54),
        );
        assert_eq!(
            q.records(),
            [
                LogRecord {
                    level: LogLevel::Error,
                    source: LogSource::Core,
                    text: "failed".to_owned()
                },
                LogRecord {
                    level: LogLevel::Warn,
                    source: LogSource::Host,
                    text: "command 0x24e54".to_owned()
                },
            ]
        );
        q.clear();
        assert!(q.records().is_empty());
    }

    #[test]
    fn invalid_utf8_becomes_a_replacement_character() {
        let mut q = LogQueue::new();
        q.push_bytes(LogLevel::Info, LogSource::Core, b"a\xFFb");
        assert_eq!(q.records()[0].text, "a\u{FFFD}b");
    }

    #[test]
    fn long_text_is_cut_at_a_character_boundary_without_growing() {
        let mut q = LogQueue::new();
        let long = "é".repeat(LOG_TEXT_MAX_BYTES);
        q.push_bytes(LogLevel::Info, LogSource::Core, long.as_bytes());
        q.push_fmt(LogLevel::Info, LogSource::Host, format_args!("{long}"));
        for r in q.records() {
            assert_eq!(r.text.len(), LOG_TEXT_MAX_BYTES);
            assert_eq!(r.text.capacity(), LOG_TEXT_MAX_BYTES);
        }
    }

    #[test]
    fn a_full_queue_counts_what_it_drops() {
        let mut q = LogQueue::new();
        for _ in 0..LOG_CAPACITY + 3 {
            q.push_bytes(LogLevel::Debug, LogSource::Core, b"x");
        }
        assert_eq!(q.records().len(), LOG_CAPACITY);
        assert_eq!(q.dropped(), 3);
        q.clear();
        assert_eq!(q.dropped(), 0);
    }

    #[test]
    fn an_error_after_a_full_queue_survives_in_its_store() {
        let mut q = LogQueue::new();
        for _ in 0..LOG_CAPACITY {
            q.push_bytes(LogLevel::Debug, LogSource::Core, b"noise");
        }
        q.push_bytes(
            LogLevel::Error,
            LogSource::Core,
            b"HDD: game.chd not found\n",
        );
        q.push_fmt(LogLevel::Error, LogSource::Host, format_args!("host {}", 7));
        q.push_bytes(LogLevel::Warn, LogSource::Core, b"not an error");
        assert_eq!(q.records().len(), LOG_CAPACITY);
        assert_eq!(q.dropped(), 3);
        let errors: Vec<(LogSource, &str)> = q
            .errors()
            .iter()
            .map(|r| (r.source, r.text.as_str()))
            .collect();
        assert_eq!(
            errors,
            [
                (LogSource::Core, "HDD: game.chd not found"),
                (LogSource::Host, "host 7")
            ]
        );
        q.clear();
        assert!(q.errors().is_empty());
    }

    #[test]
    fn the_error_store_keeps_the_first_errors_and_never_grows() {
        let mut q = LogQueue::new();
        for i in 0..ERROR_LOG_CAPACITY + 5 {
            q.push_fmt(LogLevel::Error, LogSource::Core, format_args!("error {i}"));
        }
        assert_eq!(q.errors().len(), ERROR_LOG_CAPACITY);
        assert_eq!(q.errors()[0].text, "error 0");
        let long = "é".repeat(LOG_TEXT_MAX_BYTES);
        q.clear();
        q.push_bytes(LogLevel::Error, LogSource::Core, long.as_bytes());
        assert_eq!(q.errors()[0].text.len(), LOG_TEXT_MAX_BYTES);
        assert_eq!(q.errors()[0].text.capacity(), LOG_TEXT_MAX_BYTES);
    }

    #[test]
    fn core_levels_map_to_log_levels() {
        assert_eq!(LogLevel::from_core(0), LogLevel::Debug);
        assert_eq!(LogLevel::from_core(1), LogLevel::Info);
        assert_eq!(LogLevel::from_core(2), LogLevel::Warn);
        assert_eq!(LogLevel::from_core(3), LogLevel::Error);
        assert_eq!(LogLevel::from_core(77), LogLevel::Info);
    }
}
