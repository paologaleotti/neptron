//! Bounded waits on a descriptor: the page-flip event of the CRTC, and the
//! writeback fence.

use std::io;
use std::os::fd::AsFd;
use std::time::{Duration, Instant};

use drm::control::{Device as _, Event};
use rustix::event::{PollFd, PollFlags, Timespec, poll};

use super::device::Card;

/// The kernel's timestamp and vblank count of a page flip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Flip {
    pub time_ns: u64,
    pub frame: u32,
}

/// Waits at most `limit` for the page-flip event of `crtc`. `None` when the
/// time is up. Reads only after poll says that data is there, so a read
/// never blocks, also on a blocking descriptor. Allocates nothing.
pub(super) fn wait_flip(card: &Card, crtc: u32, limit: Duration) -> io::Result<Option<Flip>> {
    let end = Instant::now() + limit;
    loop {
        if !readable(card, end)? {
            return Ok(None);
        }
        for event in card.receive_events()? {
            if let Event::PageFlip(f) = event
                && u32::from(f.crtc) == crtc
            {
                return Ok(Some(Flip {
                    time_ns: u64::try_from(f.duration.as_nanos()).unwrap_or(u64::MAX),
                    frame: f.frame,
                }));
            }
        }
    }
}

/// Waits at most `limit` until `fd` is readable (a fence: signalled).
pub(super) fn wait_readable(fd: impl AsFd, limit: Duration) -> io::Result<bool> {
    readable(fd, Instant::now() + limit)
}

/// True when `fd` became readable before `end`.
fn readable(fd: impl AsFd, end: Instant) -> io::Result<bool> {
    loop {
        let left = end.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Ok(false);
        }
        let timeout = Timespec {
            tv_sec: i64::try_from(left.as_secs()).unwrap_or(i64::MAX),
            tv_nsec: i64::from(left.subsec_nanos()),
        };
        let mut fds = [PollFd::new(&fd, PollFlags::IN)];
        match poll(&mut fds, Some(&timeout)) {
            Ok(0) => {}
            Ok(_) => {
                let revents = fds[0].revents();
                if revents.contains(PollFlags::IN) {
                    return Ok(true);
                }
                return Err(io::Error::other(format!(
                    "poll reports {revents:?} on the descriptor"
                )));
            }
            Err(rustix::io::Errno::INTR) => {}
            Err(e) => return Err(e.into()),
        }
    }
}
