//! The watchdog of a hung game (`docs/spec/hw-render.md`, section 13a): a
//! core that does not return holds the main thread of the frontend, and
//! only the launcher can end it. The frontend sends a heartbeat datagram
//! while the frames of a game advance; the launcher kills a frontend
//! whose game sent none for [`HANG_WAIT_NS`].

use std::io;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixDatagram;
use std::path::{Path, PathBuf};

/// The variable that gives the frontend the path of the heartbeat socket.
/// The frontend has the same name in `neptron::heartbeat::HEARTBEAT_ENV`.
pub const HEARTBEAT_ENV: &str = "NEPTRON_HEARTBEAT";

/// The variable that tells the start after a kill of the watchdog that the
/// game of the run before hung. The frontend has the same name in
/// `neptron::heartbeat::HUNG_ENV`.
pub const HUNG_ENV: &str = "NEPTRON_HUNG";

/// The value of [`HUNG_ENV`].
pub const HUNG_ENV_ON: &str = "1";

/// How long a game may send no heartbeat before the launcher kills the
/// frontend, in nanoseconds: 15 s, room for a 3D core that compiles its
/// shaders in its first frames.
pub const HANG_WAIT_NS: u64 = 15_000_000_000;

/// The mode of the socket file: read and write for its owner only.
pub const SOCKET_MODE: u32 = 0o600;

/// The datagram of a game whose frames advance. It arms the watchdog.
pub const BEAT: u8 = b'B';

/// The datagram of a frontend with no game. It disarms the watchdog.
pub const DISARM: u8 = b'D';

/// The watchdog of one run of the frontend: disarmed until the first
/// [`BEAT`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Watchdog {
    wait_ns: u64,
    last_beat_ns: Option<u64>,
    fired: bool,
}

impl Watchdog {
    /// A disarmed watchdog that fires `wait_ns` after the last beat.
    pub fn new(wait_ns: u64) -> Self {
        Self {
            wait_ns,
            last_beat_ns: None,
            fired: false,
        }
    }

    /// Takes one datagram byte that came at `now_ns`. Another byte changes
    /// nothing.
    pub fn take(&mut self, byte: u8, now_ns: u64) {
        match byte {
            BEAT => self.last_beat_ns = Some(now_ns),
            DISARM => self.last_beat_ns = None,
            _ => {}
        }
    }

    /// True once, at the first look at `now_ns` when the watchdog is armed
    /// and no beat came for the wait.
    pub fn expired(&mut self, now_ns: u64) -> bool {
        let due = self
            .last_beat_ns
            .is_some_and(|beat| now_ns.saturating_sub(beat) >= self.wait_ns);
        if due && !self.fired {
            self.fired = true;
            return true;
        }
        false
    }
}

/// The socket where the heartbeats of every run arrive. The launcher binds
/// it once; its file goes when the launcher drops it.
#[derive(Debug)]
pub struct HeartbeatSocket {
    socket: UnixDatagram,
    path: PathBuf,
}

/// The path of the heartbeat socket of the launcher with process id `pid`
/// in `dir`.
pub fn socket_path(dir: &Path, pid: u32) -> PathBuf {
    dir.join(format!("neptron-launch-{pid}.heartbeat"))
}

impl HeartbeatSocket {
    /// Binds a non-blocking datagram socket at `path`, with mode 0600, so
    /// only the user of the launcher (and of the frontend it starts) can
    /// send to it. A file left at the path by an earlier launcher with the
    /// same process id goes first.
    ///
    /// # Errors
    ///
    /// The error of the bind or of the mode change.
    pub fn bind(path: PathBuf) -> io::Result<Self> {
        match std::fs::remove_file(&path) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e),
            _ => {}
        }
        let socket = UnixDatagram::bind(&path)?;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(SOCKET_MODE))?;
        socket.set_nonblocking(true)?;
        Ok(Self { socket, path })
    }

    /// The path that the frontend sends to.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Gives each byte that arrived to `take`, until none is left. A
    /// datagram of more than one byte gives its first byte.
    pub fn drain(&self, mut take: impl FnMut(u8)) {
        let mut buffer = [0_u8; 16];
        while let Ok(len) = self.socket.recv(&mut buffer) {
            if len > 0 {
                take(buffer[0]);
            }
        }
    }
}

impl Drop for HeartbeatSocket {
    fn drop(&mut self) {
        // A file that cannot be removed is replaced by the next bind of a
        // launcher with the same process id.
        let _ = std::fs::remove_file(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const S: u64 = 1_000_000_000;

    #[test]
    fn a_watchdog_with_no_beat_never_fires() {
        let mut dog = Watchdog::new(HANG_WAIT_NS);
        assert!(!dog.expired(0));
        assert!(!dog.expired(3_600 * S));
    }

    #[test]
    fn a_beat_arms_it_and_fires_it_once_after_the_wait() {
        let mut dog = Watchdog::new(HANG_WAIT_NS);
        dog.take(BEAT, 10 * S);
        assert!(!dog.expired(24 * S));
        dog.take(BEAT, 20 * S);
        assert!(!dog.expired(34 * S), "the wait counts from the last beat");
        assert!(dog.expired(35 * S));
        assert!(!dog.expired(36 * S), "once");
    }

    #[test]
    fn the_end_of_a_game_disarms_it() {
        let mut dog = Watchdog::new(HANG_WAIT_NS);
        dog.take(BEAT, 0);
        dog.take(DISARM, S);
        assert!(!dog.expired(60 * S));
        dog.take(b'x', 61 * S);
        assert!(!dog.expired(120 * S), "an unknown byte does not arm it");
    }

    #[test]
    fn the_limit_is_fifteen_seconds() {
        assert_eq!(HANG_WAIT_NS, 15 * S);
    }

    #[test]
    fn a_socket_takes_the_bytes_that_a_frontend_sends_and_removes_its_file() {
        let dir = std::env::temp_dir();
        let path = socket_path(&dir, std::process::id());
        let socket = HeartbeatSocket::bind(path.clone()).expect("bind");
        let mode = std::fs::metadata(&path)
            .expect("the socket file")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, SOCKET_MODE, "only the owner sends: {mode:o}");
        let sender = UnixDatagram::unbound().expect("a sender");
        sender.send_to(&[BEAT], &path).expect("send");
        sender.send_to(&[DISARM, BEAT], &path).expect("send");
        let mut got = Vec::new();
        socket.drain(|b| got.push(b));
        assert_eq!(got, [BEAT, DISARM]);
        socket.drain(|b| got.push(b));
        assert_eq!(got.len(), 2, "nothing more");
        let again = HeartbeatSocket::bind(path.clone());
        drop(again.expect("a second bind replaces a file left behind"));
        drop(socket);
        assert!(!path.exists());
    }
}
