//! Sending an atomic [`Request`]: once, or test-only first.

use std::io;
use std::os::fd::AsFd as _;

use neptron_backend::DisplayError;

use super::device::Card;
use crate::request::RequestFull;
pub(super) use crate::request::{ALLOW_MODESET, NONBLOCK, PAGE_FLIP_EVENT, Request, TEST_ONLY};

/// Sends `req` with `flags`. Copies of the arrays go to the ioctl, which
/// takes mutable pointers, so `req` stays reusable.
pub(super) fn commit(req: &Request, card: &Card, flags: u32) -> Result<(), Refusal> {
    let mut p = req.parts().map_err(Refusal::Full)?;
    drm_ffi::mode::atomic_commit(
        card.as_fd(),
        flags,
        &mut p.objects[..p.n_objects],
        &mut p.counts[..p.n_objects],
        &mut p.props[..p.n_props],
        &mut p.values[..p.n_props],
    )
    .map_err(Refusal::Commit)
}

/// A test-only commit, then the real one, both with `flags`.
pub(super) fn test_then_commit(req: &Request, card: &Card, flags: u32) -> Result<(), Refusal> {
    match commit(req, card, flags | TEST_ONLY) {
        Err(Refusal::Commit(e)) => return Err(Refusal::Test(e)),
        other => other?,
    }
    commit(req, card, flags)
}

/// Why a request did not take effect. Nothing changed in each case.
#[derive(Debug)]
pub(super) enum Refusal {
    /// The request did not fit its arrays and was not sent.
    Full(RequestFull),
    /// The kernel refused the test-only commit.
    Test(io::Error),
    /// The kernel refused the real commit: a commit is all or nothing.
    Commit(io::Error),
}

impl Refusal {
    /// The error as a device error of `operation`.
    pub(super) fn device(self, operation: &'static str) -> DisplayError {
        DisplayError::Device {
            operation,
            reason: match self {
                Refusal::Full(e) => e.to_string(),
                Refusal::Test(e) => format!("the test-only commit failed: {e}"),
                Refusal::Commit(e) => e.to_string(),
            },
        }
    }
}
