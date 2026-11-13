use neptron_types::{Modeline, TvStandard};

/// The mode on the wire as the backend knows it, with its TV standard, and
/// what `set_mode` must do for a requested mode.
///
/// The standard is part of the state on the wire: a request with the same
/// mode and another standard, or with a standard where none was set, is a
/// modeset.
///
/// A successful modeset commit puts the mode on the wire before the
/// backend reads it back. So [`committed`](Self::committed) records it at
/// once, and [`read_back`](Self::read_back) adds the readback when it
/// comes. A failed readback leaves the mode on the wire with no readback:
/// a request for another mode is then a real commit, and a request for the
/// same mode reads it back again. After [`blanked`](Self::blanked) no mode
/// is on the wire: every request commits, and drop has nothing to restore.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WireMode {
    on_wire: Option<(Modeline, Option<TvStandard>)>,
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

    /// The step for `requested` with the TV standard `tv`.
    pub fn next_step(&self, requested: &Modeline, tv: Option<TvStandard>) -> NextStep {
        let same = self.on_wire == Some((*requested, tv));
        match self.readback {
            Some(r) if same => NextStep::Reuse(r),
            None if same => NextStep::ReadAgain,
            _ => NextStep::Commit,
        }
    }

    /// A modeset commit of `mode` with the TV standard `tv` succeeded: it
    /// is on the wire, with no readback yet.
    pub fn committed(&mut self, mode: Modeline, tv: Option<TvStandard>) {
        self.on_wire = Some((mode, tv));
        self.readback = None;
    }

    /// A blank commit succeeded: the output is off, with no mode.
    pub fn blanked(&mut self) {
        self.on_wire = None;
        self.readback = None;
    }

    /// The kernel reported `readback` for the mode on the wire.
    pub fn read_back(&mut self, readback: Modeline) {
        self.readback = Some(readback);
    }

    /// The mode the backend committed last, if any.
    pub fn on_wire(&self) -> Option<Modeline> {
        self.on_wire.map(|(mode, _)| mode)
    }

    /// The TV standard of the mode the backend committed last, if it has
    /// one.
    pub fn tv_standard(&self) -> Option<TvStandard> {
        self.on_wire.and_then(|(_, tv)| tv)
    }
}

#[cfg(test)]
mod tests {
    use neptron_types::{TV_525_240P, TV_625_288P};

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
        assert_eq!(w.next_step(&mode(640), None), NextStep::Commit);
        assert_eq!(w.on_wire(), None);
    }

    #[test]
    fn a_read_back_mode_is_reused() {
        let mut w = WireMode::new();
        w.committed(mode(640), None);
        let read = Modeline {
            pclock_hz: 25_175_000,
            ..mode(640)
        };
        w.read_back(read);
        assert_eq!(w.next_step(&mode(640), None), NextStep::Reuse(read));
        assert_eq!(w.next_step(&mode(320), None), NextStep::Commit);
    }

    #[test]
    fn a_failed_readback_keeps_the_new_mode_on_the_wire() {
        let mut w = WireMode::new();
        w.committed(mode(640), None);
        w.read_back(mode(640));
        // Mode B commits, and its readback fails: no read_back call.
        w.committed(mode(320), None);
        assert_eq!(w.on_wire(), Some(mode(320)));
        assert_eq!(w.next_step(&mode(640), None), NextStep::Commit);
        assert_eq!(w.next_step(&mode(320), None), NextStep::ReadAgain);
        w.read_back(mode(320));
        assert_eq!(w.next_step(&mode(320), None), NextStep::Reuse(mode(320)));
    }

    #[test]
    fn after_a_blank_the_same_mode_commits_again() {
        let mut w = WireMode::new();
        w.committed(mode(640), None);
        w.read_back(mode(640));
        w.blanked();
        assert_eq!(w.on_wire(), None);
        assert_eq!(w.next_step(&mode(640), None), NextStep::Commit);
        w.blanked();
        assert_eq!(w.next_step(&mode(640), None), NextStep::Commit);
        w.committed(mode(640), None);
        w.read_back(mode(640));
        assert_eq!(w.next_step(&mode(640), None), NextStep::Reuse(mode(640)));
    }

    #[test]
    fn a_change_of_the_standard_alone_commits() {
        let m = TV_525_240P;
        let mut w = WireMode::new();
        w.committed(m, Some(TvStandard::Ntsc));
        w.read_back(m);
        assert_eq!(w.tv_standard(), Some(TvStandard::Ntsc));
        assert_eq!(w.next_step(&m, Some(TvStandard::Ntsc)), NextStep::Reuse(m));
        assert_eq!(w.next_step(&m, Some(TvStandard::Pal60)), NextStep::Commit);
        assert_eq!(w.next_step(&m, Some(TvStandard::Mono60)), NextStep::Commit);
        assert_eq!(w.next_step(&m, None), NextStep::Commit);
        w.committed(m, Some(TvStandard::Pal60));
        assert_eq!(
            w.next_step(&m, Some(TvStandard::Pal60)),
            NextStep::ReadAgain
        );
        assert_eq!(w.next_step(&m, Some(TvStandard::Ntsc)), NextStep::Commit);
        w.read_back(m);
        assert_eq!(w.next_step(&m, Some(TvStandard::Pal60)), NextStep::Reuse(m));
    }

    #[test]
    fn a_standard_where_none_was_set_commits() {
        let mut w = WireMode::new();
        w.committed(TV_525_240P, None);
        w.read_back(TV_525_240P);
        assert_eq!(w.tv_standard(), None);
        assert_eq!(
            w.next_step(&TV_525_240P, Some(TvStandard::Ntsc)),
            NextStep::Commit
        );
    }

    #[test]
    fn a_blank_forgets_the_standard() {
        let mut w = WireMode::new();
        w.committed(TV_625_288P, Some(TvStandard::Pal));
        w.read_back(TV_625_288P);
        w.blanked();
        assert_eq!(w.tv_standard(), None);
        assert_eq!(
            w.next_step(&TV_625_288P, Some(TvStandard::Pal)),
            NextStep::Commit
        );
    }

    #[test]
    fn a_failed_first_readback_still_counts_as_a_mode() {
        let mut w = WireMode::new();
        w.committed(mode(640), None);
        assert_eq!(w.on_wire(), Some(mode(640)));
    }
}
