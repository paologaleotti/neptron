//! The disc of a disc game: what the quick menu shows, and the change of
//! disc (`docs/spec/disc-games.md`, section 9).

use neptron_backend::{AudioSink, Clock, Display, InputSource};
use neptron_libretro::{DiscError, DiscState};

use super::Session;

/// The core frames that the tray stays open after a change of disc, so a
/// game that reads the lid sees it open. A real core may need another
/// count (S10).
pub const DISC_TRAY_OPEN_FRAMES: u32 = 60;

impl<D: Display, S: AudioSink, I: InputSource, C: Clock> Session<D, S, I, C> {
    /// The discs of the game: the count, the disc in the drive and the
    /// tray. `None` when the core has no disc control interface.
    pub fn discs(&mut self) -> Option<DiscState> {
        self.host.disc_state()
    }

    /// Puts disc `index` in the drive: opens the tray, sets the disc, and
    /// closes the tray after [`DISC_TRAY_OPEN_FRAMES`] core frames. A
    /// change while the tray is open sets the disc and counts again.
    pub fn change_disc(&mut self, index: u32) -> Result<(), DiscError> {
        let open = self.host.disc_state().is_some_and(|d| d.tray_open);
        if !open {
            self.host.set_disc_tray(true)?;
        }
        self.disc_close_frames = DISC_TRAY_OPEN_FRAMES;
        self.host.set_disc_index(index)
    }

    /// What went wrong with the discs since the launch, for the log.
    pub fn disc_issues(&self) -> &[String] {
        &self.disc_issues
    }

    /// Counts a core frame with the tray open for a change, and closes the
    /// tray after the last one. A tray that does not close is noted once.
    pub(super) fn tick_disc_tray(&mut self) {
        if self.disc_close_frames == 0 {
            return;
        }
        self.disc_close_frames -= 1;
        if self.disc_close_frames == 0
            && let Err(e) = self.host.set_disc_tray(false)
        {
            self.disc_issues
                .push(format!("the disc tray did not close: {e}"));
        }
    }
}
