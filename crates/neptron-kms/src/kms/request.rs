//! Sending an atomic [`Request`]: once, or test-only first.

use std::io;
use std::os::fd::AsFd as _;

pub(super) use crate::request::{ALLOW_MODESET, NONBLOCK, PAGE_FLIP_EVENT, Request, TEST_ONLY};

use super::device::Card;

/// Sends `req` with `flags`. Copies of the arrays go to the ioctl, which
/// takes mutable pointers, so `req` stays reusable.
pub(super) fn commit(req: &Request, card: &Card, flags: u32) -> io::Result<()> {
    let Some(mut p) = req.parts() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "the atomic request has more objects or properties than its arrays",
        ));
    };
    drm_ffi::mode::atomic_commit(
        card.as_fd(),
        flags,
        &mut p.objects[..p.n_objects],
        &mut p.counts[..p.n_objects],
        &mut p.props[..p.n_props],
        &mut p.values[..p.n_props],
    )
}

/// A test-only commit, then the real one, both with `flags`.
pub(super) fn test_then_commit(req: &Request, card: &Card, flags: u32) -> Result<(), Refusal> {
    commit(req, card, flags | TEST_ONLY).map_err(Refusal::Test)?;
    commit(req, card, flags).map_err(Refusal::Commit)
}

/// Which of the two commits failed.
#[derive(Debug)]
pub(super) enum Refusal {
    /// The test-only commit: nothing changed.
    Test(io::Error),
    /// The real commit, after the test passed: nothing changed either,
    /// because a commit is all or nothing.
    Commit(io::Error),
}
