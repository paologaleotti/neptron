//! Reads the serial console of a guest that runs one command.
//!
//! The guest init script prints `NEPTRON-BEGIN <nonce> <uptime>` before the
//! command and `NEPTRON-END <nonce> <exit code>` after it. The host makes a
//! new nonce for each run, so a command that prints the marker text cannot
//! end the run or set its exit code. Everything before the begin marker is
//! the boot log; everything between the markers is the command output.

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Marker before the command output. Its arguments are the nonce and
/// `/proc/uptime`.
pub const BEGIN: &str = "NEPTRON-BEGIN";
/// Marker after the command output. Its arguments are the nonce and the exit
/// code.
pub const END: &str = "NEPTRON-END";

/// The phase of a guest run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// The kernel and init are starting.
    Boot,
    /// The command runs.
    Command,
    /// The command has ended.
    Done,
}

/// What the host has seen of a guest run so far.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transcript {
    /// The nonce that the markers of this run must carry.
    pub nonce: String,
    /// The current phase.
    pub phase: Phase,
    /// Every console line, for the log file.
    pub lines: Vec<String>,
    /// Host time from the start of QEMU to the begin marker.
    pub ready_after: Option<Duration>,
    /// Guest uptime at the begin marker, in milliseconds.
    pub guest_uptime_ms: Option<u64>,
    /// The exit code of the command.
    pub exit_code: Option<i32>,
}

impl Transcript {
    /// An empty transcript in the boot phase that accepts only markers with
    /// `nonce`.
    pub fn new(nonce: &str) -> Self {
        Self {
            nonce: nonce.to_owned(),
            phase: Phase::Boot,
            lines: Vec::new(),
            ready_after: None,
            guest_uptime_ms: None,
            exit_code: None,
        }
    }

    /// Takes one console line, seen `at` after QEMU started, and returns it
    /// when it is command output.
    pub fn feed(&mut self, raw: &str, at: Duration) -> Option<String> {
        let line = raw.trim_end_matches(['\r', '\n']).to_owned();
        self.lines.push(line.clone());
        match self.phase {
            Phase::Boot => {
                if let Some(rest) = self.marker(&line, BEGIN) {
                    self.guest_uptime_ms = parse_uptime_ms(rest);
                    self.phase = Phase::Command;
                    self.ready_after = Some(at);
                }
                None
            }
            Phase::Command => match self.marker(&line, END).map(|c| c.trim().parse::<i32>()) {
                Some(Ok(code)) => {
                    self.phase = Phase::Done;
                    self.exit_code = Some(code);
                    None
                }
                _ => Some(line),
            },
            Phase::Done => None,
        }
    }

    /// The last `n` lines, for an error message.
    pub fn tail(&self, n: usize) -> String {
        let start = self.lines.len().saturating_sub(n);
        self.lines[start..].join("\n")
    }

    /// The rest of `line` after `name` and the nonce of this run, or `None`
    /// when `line` is not that marker.
    fn marker<'a>(&self, line: &'a str, name: &str) -> Option<&'a str> {
        let rest = line.strip_prefix(name)?.strip_prefix(' ')?;
        let (nonce, rest) = rest.split_once(' ')?;
        (nonce == self.nonce).then_some(rest)
    }
}

/// A new nonce: 16 hex digits from the random keys of the standard hasher,
/// mixed with the time and the process id.
pub fn new_nonce() -> String {
    let mut hasher = RandomState::new().build_hasher();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    hasher.write_u128(nanos);
    hasher.write_u32(std::process::id());
    format!("{:016x}", hasher.finish())
}

/// Parses the first field of `/proc/uptime` (seconds with two decimals)
/// into milliseconds, with integer maths.
pub fn parse_uptime_ms(text: &str) -> Option<u64> {
    let field = text.split_whitespace().next()?;
    let (secs, frac) = field.split_once('.').unwrap_or((field, ""));
    let secs: u64 = secs.parse().ok()?;
    let mut frac_ms = 0;
    let mut scale = 100;
    for c in frac.chars().take(3) {
        frac_ms += u64::from(c.to_digit(10)?) * scale;
        scale /= 10;
    }
    secs.checked_mul(1000)?.checked_add(frac_ms)
}

#[cfg(test)]
mod tests {
    use super::*;

    const T: Duration = Duration::from_millis(1500);
    const NONCE: &str = "0123456789abcdef";

    fn running() -> Transcript {
        let mut t = Transcript::new(NONCE);
        t.feed(&format!("NEPTRON-BEGIN {NONCE} 0.50"), T);
        t
    }

    #[test]
    fn boot_lines_are_logged_but_not_output() {
        let mut t = Transcript::new(NONCE);
        assert_eq!(t.feed("[    0.000000] Booting Linux\r\n", T), None);
        assert_eq!(t.phase, Phase::Boot);
        assert_eq!(t.lines, ["[    0.000000] Booting Linux"]);
    }

    #[test]
    fn a_full_run_gives_output_code_and_times() {
        let mut t = Transcript::new(NONCE);
        t.feed("Starting mdev... OK", Duration::ZERO);
        t.feed(&format!("NEPTRON-BEGIN {NONCE} 1.07 1.80\r"), T);
        assert_eq!(t.feed("hello\r", T), Some("hello".to_owned()));
        assert_eq!(t.feed(&format!("NEPTRON-END {NONCE} 3"), T), None);
        assert_eq!(t.feed("reboot: Restarting system", T), None);
        assert_eq!(t.phase, Phase::Done);
        assert_eq!(t.exit_code, Some(3));
        assert_eq!(t.ready_after, Some(T));
        assert_eq!(t.guest_uptime_ms, Some(1070));
    }

    #[test]
    fn an_end_marker_without_the_nonce_is_output() {
        let mut t = running();
        assert_eq!(t.feed("NEPTRON-END 0", T), Some("NEPTRON-END 0".to_owned()));
        assert_eq!(t.feed("after", T), Some("after".to_owned()));
        assert_eq!(t.feed(&format!("NEPTRON-END {NONCE} 7"), T), None);
        assert_eq!(t.exit_code, Some(7));
    }

    #[test]
    fn an_end_marker_with_a_wrong_nonce_is_output() {
        let mut t = running();
        let forged = "NEPTRON-END fedcba9876543210 0";
        assert_eq!(t.feed(forged, T), Some(forged.to_owned()));
        assert_eq!(t.phase, Phase::Command);
        assert_eq!(t.exit_code, None);
    }

    #[test]
    fn a_begin_marker_with_a_wrong_nonce_does_not_start_the_command() {
        let mut t = Transcript::new(NONCE);
        t.feed("NEPTRON-BEGIN fedcba9876543210 0.50", T);
        assert_eq!(t.phase, Phase::Boot);
        assert_eq!(t.ready_after, None);
    }

    #[test]
    fn markers_count_only_at_the_start_of_a_line() {
        let mut t = running();
        let echoed = format!("echo NEPTRON-END {NONCE} 0");
        assert_eq!(t.feed(&echoed, T), Some(echoed.clone()));
        assert_eq!(t.phase, Phase::Command);
    }

    #[test]
    fn an_end_marker_without_a_number_is_output() {
        let mut t = running();
        let line = format!("NEPTRON-END {NONCE} later");
        assert_eq!(t.feed(&line, T), Some(line.clone()));
        assert_eq!(t.exit_code, None);
    }

    #[test]
    fn a_guest_that_dies_has_no_exit_code() {
        let mut t = Transcript::new(NONCE);
        t.feed("Kernel panic - not syncing: VFS", T);
        assert_eq!(t.exit_code, None);
        assert_eq!(t.tail(1), "Kernel panic - not syncing: VFS");
    }

    #[test]
    fn nonces_are_16_hex_digits_and_differ() {
        let a = new_nonce();
        let b = new_nonce();
        assert_eq!(a.len(), 16);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()), "{a}");
        assert_ne!(a, b);
    }

    #[test]
    fn uptime_parses_with_integer_maths() {
        assert_eq!(parse_uptime_ms("2.34 3.10"), Some(2340));
        assert_eq!(parse_uptime_ms("12"), Some(12_000));
        assert_eq!(parse_uptime_ms("0.5"), Some(500));
        assert_eq!(parse_uptime_ms("1.2345"), Some(1234));
        assert_eq!(parse_uptime_ms("x.10"), None);
        assert_eq!(parse_uptime_ms(""), None);
    }

    #[test]
    fn tail_takes_the_last_lines() {
        let mut t = Transcript::new(NONCE);
        for line in ["a", "b", "c"] {
            t.feed(line, T);
        }
        assert_eq!(t.tail(2), "b\nc");
        assert_eq!(t.tail(10), "a\nb\nc");
    }
}
