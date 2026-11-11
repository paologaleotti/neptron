//! Log records of the core and of the host, in a queue that is made once
//! and never allocates after that.

use std::fmt::{self, Write as _};

/// The most log records kept between two calls.
pub const LOG_CAPACITY: usize = 64;
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

/// The queue. Each record keeps its text buffer for the life of the queue.
#[derive(Debug)]
pub(crate) struct LogQueue {
    records: Vec<LogRecord>,
    len: usize,
    dropped: usize,
}

impl LogQueue {
    pub(crate) fn new() -> Self {
        let records = (0..LOG_CAPACITY)
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

    pub(crate) fn clear(&mut self) {
        self.len = 0;
        self.dropped = 0;
    }

    pub(crate) fn records(&self) -> &[LogRecord] {
        self.records.get(..self.len).unwrap_or_default()
    }

    pub(crate) fn dropped(&self) -> usize {
        self.dropped
    }

    /// Adds a record with text from a core: bytes that are not UTF-8
    /// become U+FFFD, and a trailing newline is removed.
    pub(crate) fn push_bytes(&mut self, level: LogLevel, source: LogSource, text: &[u8]) {
        let Some(record) = self.next(level, source) else {
            return;
        };
        let mut out = Bounded(&mut record.text);
        for chunk in text.utf8_chunks() {
            if out.write_str(chunk.valid()).is_err() {
                break;
            }
            if !chunk.invalid().is_empty() && out.write_char('\u{FFFD}').is_err() {
                break;
            }
        }
        let trimmed = record.text.trim_end_matches(['\n', '\r']).len();
        record.text.truncate(trimmed);
    }

    /// Adds a record with formatted text, with no allocation.
    pub(crate) fn push_fmt(&mut self, level: LogLevel, source: LogSource, args: fmt::Arguments) {
        if let Some(record) = self.next(level, source) {
            // A full buffer cuts the text; that is the intended limit.
            let _ = Bounded(&mut record.text).write_fmt(args);
        }
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
    fn core_levels_map_to_log_levels() {
        assert_eq!(LogLevel::from_core(0), LogLevel::Debug);
        assert_eq!(LogLevel::from_core(1), LogLevel::Info);
        assert_eq!(LogLevel::from_core(2), LogLevel::Warn);
        assert_eq!(LogLevel::from_core(3), LogLevel::Error);
        assert_eq!(LogLevel::from_core(77), LogLevel::Info);
    }
}
