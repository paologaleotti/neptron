//! The notices of a game that the machine cannot keep at full speed: the
//! speed of a slow game, and the sound that is off below half speed. The
//! session measures both (`Session::slow_speed_percent`,
//! `Session::sound_too_slow`); this module decides when a notice shows.

/// The shortest time between two notices of a slow game, in nanoseconds
/// of the clock of the run: 60 s.
pub const SLOW_NOTICE_GAP_NS: u64 = 60_000_000_000;

/// The text of the notice when the sound turns off.
pub const SOUND_OFF_TEXT: &str = "Sound off: the game runs at less than half speed.";

/// A notice about the speed of the game.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpeedNotice {
    /// The game runs at this speed, in percent of the rate of the mode.
    Slow(u32),
    /// The sound is off: the game runs at less than half speed.
    SoundOff,
}

impl SpeedNotice {
    /// The text for the player and the log.
    pub fn text(self) -> String {
        match self {
            Self::Slow(percent) => {
                format!("This game runs at {percent}% speed. This machine is too slow for it.")
            }
            Self::SoundOff => SOUND_OFF_TEXT.to_owned(),
        }
    }
}

/// When the speed notices of a game show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpeedNotices {
    /// The slow notice can show: at the start, and after the game was no
    /// longer slow.
    slow_armed: bool,
    /// The time of the last slow notice, in nanoseconds.
    last_slow_ns: Option<u64>,
    /// The sound was off at the last step.
    sound_off: bool,
}

impl Default for SpeedNotices {
    fn default() -> Self {
        Self::new()
    }
}

impl SpeedNotices {
    /// The state at the start of a game: nothing shown yet.
    pub fn new() -> Self {
        Self {
            slow_armed: true,
            last_slow_ns: None,
            sound_off: false,
        }
    }

    /// One frame with the speed `slow_percent` that the session reports,
    /// `sound_off` when it muted the sound, at `now_ns`. Returns the notice
    /// to show now, if any.
    ///
    /// The slow notice shows once while the game stays slow. It shows
    /// again after the game was no longer slow, and at least
    /// [`SLOW_NOTICE_GAP_NS`] after the one before. The sound notice shows
    /// at each turn of the sound to off. When both start in one frame, the
    /// sound notice shows first and the slow notice at the next frame.
    pub fn step(
        &mut self,
        slow_percent: Option<u32>,
        sound_off: bool,
        now_ns: u64,
    ) -> Option<SpeedNotice> {
        let turned_off = sound_off && !self.sound_off;
        self.sound_off = sound_off;
        if turned_off {
            return Some(SpeedNotice::SoundOff);
        }
        let Some(percent) = slow_percent else {
            self.slow_armed = true;
            return None;
        };
        let gap_passed = self
            .last_slow_ns
            .is_none_or(|last| now_ns.saturating_sub(last) >= SLOW_NOTICE_GAP_NS);
        if !self.slow_armed || !gap_passed {
            return None;
        }
        self.slow_armed = false;
        self.last_slow_ns = Some(now_ns);
        Some(SpeedNotice::Slow(percent))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECOND_NS: u64 = 1_000_000_000;

    #[test]
    fn the_first_slow_speed_shows_with_its_value() {
        let mut state = SpeedNotices::new();
        assert_eq!(state.step(None, false, 0), None);
        assert_eq!(
            state.step(Some(76), false, SECOND_NS),
            Some(SpeedNotice::Slow(76))
        );
        assert_eq!(
            SpeedNotice::Slow(76).text(),
            "This game runs at 76% speed. This machine is too slow for it."
        );
    }

    #[test]
    fn a_change_of_the_value_does_not_show_again() {
        let mut state = SpeedNotices::new();
        assert!(state.step(Some(76), false, 0).is_some());
        for (i, percent) in (70..80).cycle().take(10_000).enumerate() {
            let now = (i as u64 + 1) * SECOND_NS;
            assert_eq!(state.step(Some(percent), false, now), None);
        }
    }

    #[test]
    fn a_slow_game_shows_again_after_it_went_away_and_the_gap_passed() {
        let mut state = SpeedNotices::new();
        assert!(state.step(Some(76), false, 0).is_some());
        assert_eq!(state.step(None, false, SECOND_NS), None);
        assert_eq!(
            state.step(Some(80), false, 2 * SECOND_NS),
            None,
            "back before the gap"
        );
        assert_eq!(
            state.step(Some(81), false, SLOW_NOTICE_GAP_NS),
            Some(SpeedNotice::Slow(81)),
            "still slow when the gap passed"
        );
        assert_eq!(state.step(Some(81), false, 3 * SLOW_NOTICE_GAP_NS), None);
        assert_eq!(state.step(None, false, 3 * SLOW_NOTICE_GAP_NS), None);
        assert_eq!(
            state.step(Some(90), false, 3 * SLOW_NOTICE_GAP_NS + 1),
            Some(SpeedNotice::Slow(90))
        );
    }

    #[test]
    fn a_slow_game_that_stays_slow_never_shows_again() {
        let mut state = SpeedNotices::new();
        assert!(state.step(Some(76), false, 0).is_some());
        assert_eq!(state.step(Some(76), false, 10 * SLOW_NOTICE_GAP_NS), None);
    }

    #[test]
    fn the_sound_notice_shows_at_each_turn_to_off() {
        let mut state = SpeedNotices::new();
        assert_eq!(state.step(None, true, 0), Some(SpeedNotice::SoundOff));
        assert_eq!(state.step(None, true, SECOND_NS), None);
        assert_eq!(state.step(None, false, 2 * SECOND_NS), None);
        assert_eq!(
            state.step(None, true, 3 * SECOND_NS),
            Some(SpeedNotice::SoundOff)
        );
        assert_eq!(
            SpeedNotice::SoundOff.text(),
            "Sound off: the game runs at less than half speed."
        );
    }

    #[test]
    fn both_notices_in_one_frame_show_one_after_the_other() {
        let mut state = SpeedNotices::new();
        assert_eq!(state.step(Some(40), true, 0), Some(SpeedNotice::SoundOff));
        assert_eq!(state.step(Some(40), true, 1), Some(SpeedNotice::Slow(40)));
        assert_eq!(state.step(Some(40), true, 2), None);
    }
}
