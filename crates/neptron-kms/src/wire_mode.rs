use neptron_types::Modeline;

/// The mode on the wire as the backend knows it, and what `set_mode` must
/// do for a requested mode.
///
/// A successful modeset commit puts the mode on the wire before the
/// backend reads it back. So [`committed`](Self::committed) records it at
/// once, and [`read_back`](Self::read_back) adds the readback when it
/// comes. A failed readback leaves the mode on the wire with no readback:
/// a request for another mode is then a real commit, and a request for the
/// same mode reads it back again.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WireMode {
    on_wire: Option<Modeline>,
    readback: Option<Modeline>,
}

/// What `set_mode` does for a requested mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NextStep {
    /// The mode is on the wire and was read back: no commit, return this.
    Reuse(Modeline),
    /// The mode is on the wire but its readback failed: no commit, read it
    /// back again.
    ReadAgain,
    /// Another mode, or none, is on the wire: commit.
    Commit,
}

impl WireMode {
    /// No mode on the wire.
    pub fn new() -> WireMode {
        WireMode {
            on_wire: None,
            readback: None,
        }
    }

    /// The step for `requested`.
    pub fn next_step(&self, requested: &Modeline) -> NextStep {
        match (self.on_wire, self.readback) {
            (Some(m), Some(r)) if m == *requested => NextStep::Reuse(r),
            (Some(m), None) if m == *requested => NextStep::ReadAgain,
            _ => NextStep::Commit,
        }
    }

    /// A modeset commit of `mode` succeeded: it is on the wire, with no
    /// readback yet.
    pub fn committed(&mut self, mode: Modeline) {
        self.on_wire = Some(mode);
        self.readback = None;
    }

    /// The kernel reported `readback` for the mode on the wire.
    pub fn read_back(&mut self, readback: Modeline) {
        self.readback = Some(readback);
    }

    /// The mode the backend committed last, if any.
    pub fn on_wire(&self) -> Option<Modeline> {
        self.on_wire
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mode(hactive: u16) -> Modeline {
        Modeline {
            pclock_hz: 25_175_000,
            hactive,
            hsync_start: 656,
            hsync_end: 752,
            htotal: 800,
            vactive: 480,
            vsync_start: 490,
            vsync_end: 492,
            vtotal: 525,
            interlace: false,
            doublescan: false,
            hsync_positive: false,
            vsync_positive: false,
        }
    }

    #[test]
    fn nothing_on_the_wire_commits() {
        let w = WireMode::new();
        assert_eq!(w.next_step(&mode(640)), NextStep::Commit);
        assert_eq!(w.on_wire(), None);
    }

    #[test]
    fn a_read_back_mode_is_reused() {
        let mut w = WireMode::new();
        w.committed(mode(640));
        let read = Modeline {
            pclock_hz: 25_175_000,
            ..mode(640)
        };
        w.read_back(read);
        assert_eq!(w.next_step(&mode(640)), NextStep::Reuse(read));
        assert_eq!(w.next_step(&mode(320)), NextStep::Commit);
    }

    #[test]
    fn a_failed_readback_keeps_the_new_mode_on_the_wire() {
        let mut w = WireMode::new();
        w.committed(mode(640));
        w.read_back(mode(640));
        // Mode B commits, and its readback fails: no read_back call.
        w.committed(mode(320));
        assert_eq!(w.on_wire(), Some(mode(320)));
        assert_eq!(w.next_step(&mode(640)), NextStep::Commit);
        assert_eq!(w.next_step(&mode(320)), NextStep::ReadAgain);
        w.read_back(mode(320));
        assert_eq!(w.next_step(&mode(320)), NextStep::Reuse(mode(320)));
    }

    #[test]
    fn a_failed_first_readback_still_counts_as_a_mode() {
        let mut w = WireMode::new();
        w.committed(mode(640));
        assert_eq!(w.on_wire(), Some(mode(640)));
    }
}
