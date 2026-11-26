//! The heartbeat of a game for the watchdog of the launcher
//! (`docs/spec/hw-render.md`, section 13a). A core that does not return
//! holds the frame thread, so a thread of its own sends the beats: the
//! frame thread only counts its game frames in an atomic.

use std::ffi::OsStr;
use std::io;
use std::os::unix::net::UnixDatagram;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{RecvTimeoutError, SyncSender, sync_channel};
use std::thread::JoinHandle;
use std::time::Duration;

/// The variable with the path of the heartbeat socket of the launcher. It
/// equals `HEARTBEAT_ENV` of `neptron-launch`.
pub const HEARTBEAT_ENV: &str = "NEPTRON_HEARTBEAT";

/// The variable with which the launcher tells this start that it killed
/// the run before for a hung game. It equals `HUNG_ENV` of
/// `neptron-launch`.
pub const HUNG_ENV: &str = "NEPTRON_HUNG";

/// The notice of the game list after the launcher killed a hung game.
pub const HUNG_NOTICE: &str = "The game stopped: it did not answer for 15 seconds.";

/// The time between two looks of the heartbeat thread, in nanoseconds. The
/// launcher waits 15 s, so a few lost beats do not count.
pub const HEARTBEAT_PERIOD_NS: u64 = 1_000_000_000;

/// The datagram of a game whose frames advance. It equals `BEAT` of
/// `neptron-launch`.
pub const BEAT: u8 = b'B';

/// The datagram of a run with no game. It equals `DISARM` of
/// `neptron-launch`.
pub const DISARM: u8 = b'D';

/// What the frame thread tells the heartbeat thread: whether a game runs,
/// and how many of its frames ended. Clones share one state. Each call is
/// one atomic store or add, so the frame thread does no I/O and takes no
/// lock for it.
#[derive(Debug, Clone, Default)]
pub struct Pulse {
    state: Arc<PulseState>,
}

#[derive(Debug, Default)]
struct PulseState {
    in_game: AtomicBool,
    frames: AtomicU64,
}

/// The heartbeat thread and its stop. Dropping it stops and joins the
/// thread.
#[derive(Debug)]
pub struct Heartbeat {
    stop: Option<SyncSender<()>>,
    thread: Option<JoinHandle<()>>,
}

impl Pulse {
    /// A game starts: the watchdog of the launcher watches it from the
    /// next look.
    pub fn enter_game(&self) {
        self.state.in_game.store(true, Ordering::Release);
    }

    /// The game ends: the watchdog stops watching. Call it before the last
    /// save RAM flush, which can take longer than a frame.
    pub fn leave_game(&self) {
        self.state.in_game.store(false, Ordering::Release);
    }

    /// A frame of the game ended.
    pub fn frame(&self) {
        self.state.frames.fetch_add(1, Ordering::Relaxed);
    }

    fn read(&self) -> (bool, u64) {
        (
            self.state.in_game.load(Ordering::Acquire),
            self.state.frames.load(Ordering::Relaxed),
        )
    }
}

/// The datagram of one look, from what the frame thread says now and
/// `seen`, the frame count of the last beat in this game: a beat at the
/// first look in a game and when the count moved, nothing when it did
/// not, and a disarm out of a game.
fn look((in_game, frames): (bool, u64), seen: &mut Option<u64>) -> Option<u8> {
    if !in_game {
        *seen = None;
        return Some(DISARM);
    }
    if *seen == Some(frames) {
        return None;
    }
    *seen = Some(frames);
    Some(BEAT)
}

impl Heartbeat {
    /// Starts the thread that sends the beats of `pulse` to the launcher's
    /// socket in [`HEARTBEAT_ENV`], one look every [`HEARTBEAT_PERIOD_NS`].
    /// `None` with no variable: a run with no launcher has no watchdog.
    ///
    /// # Errors
    ///
    /// The socket or the thread could not be made.
    pub fn from_env(pulse: Pulse) -> Option<io::Result<Self>> {
        let path = std::env::var_os(HEARTBEAT_ENV)?;
        Some(Self::start(
            PathBuf::from(path),
            pulse,
            Duration::from_nanos(HEARTBEAT_PERIOD_NS),
        ))
    }

    /// Starts the thread that sends the beats of `pulse` to `path`, one
    /// look every `period`.
    ///
    /// # Errors
    ///
    /// The socket or the thread could not be made.
    pub fn start(path: PathBuf, pulse: Pulse, period: Duration) -> io::Result<Self> {
        let socket = UnixDatagram::unbound()?;
        socket.set_nonblocking(true)?;
        let (stop, stopped) = sync_channel::<()>(1);
        let thread = std::thread::Builder::new()
            .name("heartbeat".to_owned())
            .spawn(move || {
                let mut seen = None;
                while let Err(RecvTimeoutError::Timeout) = stopped.recv_timeout(period) {
                    if let Some(byte) = look(pulse.read(), &mut seen) {
                        // A beat that does not go (a full socket, no
                        // launcher) is one of many: the launcher waits 15
                        // looks.
                        let _ = socket.send_to(&[byte], &path);
                    }
                }
            })?;
        Ok(Self {
            stop: Some(stop),
            thread: Some(thread),
        })
    }
}

impl Drop for Heartbeat {
    fn drop(&mut self) {
        // The thread ends at the closed channel within one period.
        drop(self.stop.take());
        if let Some(thread) = self.thread.take() {
            // The thread only sends datagrams; a panic there has nothing
            // left to report at the end of the run.
            let _ = thread.join();
        }
    }
}

/// The Warning notice for `value`, the value of [`HUNG_ENV`]. `None` with
/// no value or another one.
pub fn hung_notice(value: Option<&OsStr>) -> Option<&'static str> {
    value.filter(|v| *v == "1").map(|_| HUNG_NOTICE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_game_beats_when_its_frames_advance_and_only_then() {
        let mut seen = None;
        assert_eq!(
            look((true, 0), &mut seen),
            Some(BEAT),
            "the first look arms"
        );
        assert_eq!(
            look((true, 0), &mut seen),
            None,
            "a hung game sends nothing"
        );
        assert_eq!(look((true, 5), &mut seen), Some(BEAT));
        assert_eq!(look((true, 5), &mut seen), None);
        assert_eq!(look((false, 5), &mut seen), Some(DISARM));
        assert_eq!(
            look((false, 5), &mut seen),
            Some(DISARM),
            "again at each look"
        );
        assert_eq!(
            look((true, 5), &mut seen),
            Some(BEAT),
            "a new game arms again"
        );
    }

    #[test]
    fn the_notice_comes_only_with_the_value_of_the_launcher() {
        assert_eq!(hung_notice(Some(OsStr::new("1"))), Some(HUNG_NOTICE));
        assert_eq!(hung_notice(Some(OsStr::new("0"))), None);
        assert_eq!(hung_notice(None), None);
    }

    #[test]
    fn the_thread_sends_the_beats_of_the_frames_and_stops_on_drop() {
        let path = std::env::temp_dir().join(format!("neptron-heartbeat-{}", std::process::id()));
        // A leftover of an earlier run with the same process id; absence is fine.
        let _ = std::fs::remove_file(&path);
        let launcher = UnixDatagram::bind(&path).expect("bind");
        launcher
            .set_read_timeout(Some(Duration::from_secs(10)))
            .expect("a timeout");
        let pulse = Pulse::default();
        let heartbeat =
            Heartbeat::start(path.clone(), pulse.clone(), Duration::from_millis(5)).expect("start");
        let mut byte = [0_u8; 1];
        launcher.recv(&mut byte).expect("a datagram");
        assert_eq!(byte[0], DISARM, "no game yet");
        pulse.enter_game();
        loop {
            launcher.recv(&mut byte).expect("a datagram");
            if byte[0] == BEAT {
                break;
            }
        }
        pulse.frame();
        loop {
            launcher.recv(&mut byte).expect("a datagram");
            if byte[0] == BEAT {
                break;
            }
        }
        drop(heartbeat);
        // The test's own socket; a failed removal leaves a temp file.
        let _ = std::fs::remove_file(&path);
    }
}
