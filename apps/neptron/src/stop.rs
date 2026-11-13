//! The stop signals of a run.
//!
//! The launcher stops the frontend with SIGTERM, and a person at a terminal
//! with SIGINT. Both set one flag that the frame loop reads at each frame,
//! so a run ends as a quit: the game's save RAM and the pending writes
//! reach their files, the display drops by the rules of its backend, and
//! the exit code is 0. The flag works whether or not anything polls the
//! SDL event queue, which the KMS run does not do.
//!
//! SDL3 installs its own SIGTERM and SIGINT handlers only when a signal
//! still has its default action. The flag is registered before any SDL
//! code starts, so SDL leaves these signals to it.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

/// A new stop flag, set by SIGTERM and SIGINT from now on. Call it before
/// the backend opens SDL.
///
/// # Errors
///
/// The error of the operating system when a handler cannot be installed.
pub fn stop_flag() -> std::io::Result<Arc<AtomicBool>> {
    let flag = Arc::new(AtomicBool::new(false));
    for signal in [signal_hook::consts::SIGTERM, signal_hook::consts::SIGINT] {
        signal_hook::flag::register(signal, Arc::clone(&flag))?;
    }
    Ok(flag)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering;

    #[test]
    fn sigterm_sets_the_stop_flag() {
        let flag = stop_flag().expect("the handlers");
        assert!(!flag.load(Ordering::Relaxed));
        signal_hook::low_level::raise(signal_hook::consts::SIGTERM).expect("raise");
        assert!(flag.load(Ordering::Relaxed));
    }

    #[test]
    fn sigint_sets_the_stop_flag() {
        let flag = stop_flag().expect("the handlers");
        signal_hook::low_level::raise(signal_hook::consts::SIGINT).expect("raise");
        assert!(flag.load(Ordering::Relaxed));
    }
}
