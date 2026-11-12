use neptron_backend::DisplayError;

/// The order rules of the `Display` calls, as the fake display of
/// `neptron-testkit` has them: no present, mode change or blank while a
/// flip is pending (a nonblocking atomic commit would get `EBUSY`), no wait
/// without a queued flip, and nothing before the first mode or after a
/// blank.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FlipState {
    mode_set: bool,
    pending: bool,
}

impl FlipState {
    /// No mode, no flip.
    pub fn new() -> FlipState {
        FlipState {
            mode_set: false,
            pending: false,
        }
    }

    /// A mode is set.
    pub fn has_mode(&self) -> bool {
        self.mode_set
    }

    /// A flip is queued and its event has not come.
    pub fn is_pending(&self) -> bool {
        self.pending
    }

    /// Checks that `set_mode` may commit.
    ///
    /// # Errors
    ///
    /// [`DisplayError::Device`] while a flip is pending.
    pub fn check_set_mode(&self) -> Result<(), DisplayError> {
        if self.pending {
            return Err(busy("set_mode"));
        }
        Ok(())
    }

    /// Checks that `blank` may commit.
    ///
    /// # Errors
    ///
    /// [`DisplayError::Device`] while a flip is pending.
    pub fn check_blank(&self) -> Result<(), DisplayError> {
        if self.pending {
            return Err(busy("blank"));
        }
        Ok(())
    }

    /// Checks that `present` may queue a flip.
    ///
    /// # Errors
    ///
    /// [`DisplayError::NoMode`] before the first mode,
    /// [`DisplayError::Device`] while a flip is pending.
    pub fn check_present(&self) -> Result<(), DisplayError> {
        if !self.mode_set {
            return Err(DisplayError::NoMode);
        }
        if self.pending {
            return Err(busy("present"));
        }
        Ok(())
    }

    /// Checks that `wait_flip` has a flip to wait for.
    ///
    /// # Errors
    ///
    /// [`DisplayError::NoMode`] before the first mode,
    /// [`DisplayError::Device`] with no flip queued.
    pub fn check_wait(&self) -> Result<(), DisplayError> {
        if !self.mode_set {
            return Err(DisplayError::NoMode);
        }
        if !self.pending {
            return Err(DisplayError::Device {
                operation: "wait_flip",
                reason: "no flip is queued; call present first".to_owned(),
            });
        }
        Ok(())
    }

    /// A mode change completed.
    pub fn mode_committed(&mut self) {
        self.mode_set = true;
    }

    /// The output is off: no mode until the next mode change.
    pub fn blanked(&mut self) {
        self.mode_set = false;
    }

    /// A flip was queued.
    pub fn flip_queued(&mut self) {
        self.pending = true;
    }

    /// The event of the queued flip came.
    pub fn flip_done(&mut self) {
        self.pending = false;
    }
}

fn busy(operation: &'static str) -> DisplayError {
    DisplayError::Device {
        operation,
        reason: "a flip is pending (the kernel would answer EBUSY); call wait_flip first"
            .to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_works_before_the_first_mode_except_set_mode() {
        let s = FlipState::new();
        assert_eq!(s.check_present(), Err(DisplayError::NoMode));
        assert_eq!(s.check_wait(), Err(DisplayError::NoMode));
        assert_eq!(s.check_set_mode(), Ok(()));
    }

    #[test]
    fn a_pending_flip_blocks_present_and_set_mode() {
        let mut s = FlipState::new();
        s.mode_committed();
        assert_eq!(s.check_present(), Ok(()));
        s.flip_queued();
        assert!(matches!(
            s.check_present(),
            Err(DisplayError::Device {
                operation: "present",
                ..
            })
        ));
        assert!(matches!(
            s.check_set_mode(),
            Err(DisplayError::Device {
                operation: "set_mode",
                ..
            })
        ));
        assert_eq!(s.check_wait(), Ok(()));
        s.flip_done();
        assert_eq!(s.check_present(), Ok(()));
        assert_eq!(s.check_set_mode(), Ok(()));
    }

    #[test]
    fn a_pending_flip_blocks_blank() {
        let mut s = FlipState::new();
        assert_eq!(s.check_blank(), Ok(()), "a blank needs no mode");
        s.mode_committed();
        s.flip_queued();
        assert!(matches!(
            s.check_blank(),
            Err(DisplayError::Device {
                operation: "blank",
                ..
            })
        ));
        s.flip_done();
        assert_eq!(s.check_blank(), Ok(()));
    }

    #[test]
    fn after_a_blank_only_set_mode_and_blank_work() {
        let mut s = FlipState::new();
        s.mode_committed();
        s.blanked();
        assert!(!s.has_mode());
        assert_eq!(s.check_present(), Err(DisplayError::NoMode));
        assert_eq!(s.check_wait(), Err(DisplayError::NoMode));
        assert_eq!(s.check_set_mode(), Ok(()));
        assert_eq!(s.check_blank(), Ok(()));
        s.mode_committed();
        assert_eq!(s.check_present(), Ok(()));
    }

    #[test]
    fn a_wait_needs_a_queued_flip() {
        let mut s = FlipState::new();
        s.mode_committed();
        assert!(matches!(
            s.check_wait(),
            Err(DisplayError::Device {
                operation: "wait_flip",
                ..
            })
        ));
    }
}
