//! The silent pad: a pad that is connected but has sent no input.
//!
//! A pad can enumerate and send only neutral reports: a PS3 arcade stick
//! on a PC does, and works behind a USB hub. The rule finds such a pad
//! and never nags: one notice per pad per plug, only after a clear time
//! while other devices press buttons, and only where the caller allows it
//! (the game list).

use crate::device::{DeviceIdentity, MAX_PORTS};

/// The time a pad stays connected and not heard before its notice, in
/// nanoseconds.
pub const SILENT_PAD_AFTER_NS: u64 = 20_000_000_000;

/// The button presses of other devices, after the connect of a pad that is
/// not heard, before its notice. Somebody uses the machine, and this pad
/// does not answer.
pub const SILENT_PAD_OTHER_PRESSES: u32 = 8;

/// The pad on one port, as the silent pad rule reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PadSignal {
    /// A number that is new for each connect of a pad in the run.
    pub plug: u32,
    /// The identity of the pad.
    pub identity: DeviceIdentity,
    /// The hash of the path of the pad.
    pub path: DeviceIdentity,
    /// True once the pad has sent a value that is not neutral.
    pub heard: bool,
}

/// The connected pads by port, with no allocation, so the frontend can
/// read it in every frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PadActivity {
    /// The pad of each port that has a connected pad, from port 0.
    pub ports: [Option<PadSignal>; MAX_PORTS as usize],
}

/// A pad that the rule found: the frontend shows its notice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SilentPad {
    /// Its port now.
    pub port: u8,
    /// Its identity.
    pub identity: DeviceIdentity,
    /// The hash of its path.
    pub path: DeviceIdentity,
}

/// The silent pad rule over the frames of a run.
#[derive(Debug, Clone, Default)]
pub struct SilentPads {
    watched: [Option<Watched>; MAX_PORTS as usize],
    /// The pads of the runs before that were heard or told: the first plug
    /// of each in this run needs no check.
    checked: Vec<(DeviceIdentity, DeviceIdentity)>,
}

#[derive(Debug, Clone, Copy)]
struct Watched {
    signal: PadSignal,
    since_ns: u64,
    presses: u32,
    /// Heard or told: no notice in this plug.
    done: bool,
}

impl SilentPads {
    /// The rule with no pad, where the pads of `checked` (identity, path)
    /// need no check at their first plug: an earlier run of the boot heard
    /// them or told the player.
    pub fn new(checked: &[(DeviceIdentity, DeviceIdentity)]) -> Self {
        Self {
            watched: [None; MAX_PORTS as usize],
            checked: checked.to_vec(),
        }
    }

    /// Follows the pads of one frame. `presses` is the number of button
    /// presses of the frame, of every device; a pad that is not heard
    /// pressed none of them. `check` allows a notice. Returns at most one
    /// pad for a notice; the next frame gives the next one. It allocates
    /// nothing.
    pub fn update(
        &mut self,
        activity: &PadActivity,
        presses: u32,
        now_ns: u64,
        check: bool,
    ) -> Option<SilentPad> {
        for slot in &mut self.watched {
            if slot.is_some_and(|w| !plugged(activity, w.signal.plug)) {
                *slot = None;
            }
        }
        for signal in activity.ports.iter().flatten() {
            if self.find(signal.plug).is_some() {
                continue;
            }
            let earlier = self
                .checked
                .iter()
                .position(|key| *key == (signal.identity, signal.path));
            if let Some(i) = earlier {
                self.checked.swap_remove(i);
            }
            if let Some(slot) = self.watched.iter_mut().find(|s| s.is_none()) {
                *slot = Some(Watched {
                    signal: *signal,
                    since_ns: now_ns,
                    presses: 0,
                    done: earlier.is_some(),
                });
            }
        }
        let mut found = None;
        for (port, signal) in (0u8..).zip(&activity.ports) {
            let Some(signal) = signal else {
                continue;
            };
            let Some(watched) = self.find(signal.plug) else {
                continue;
            };
            if watched.done {
                continue;
            }
            if signal.heard {
                watched.done = true;
                continue;
            }
            watched.presses = watched.presses.saturating_add(presses);
            let due = now_ns.saturating_sub(watched.since_ns) >= SILENT_PAD_AFTER_NS
                && watched.presses >= SILENT_PAD_OTHER_PRESSES;
            if check && due && found.is_none() {
                watched.done = true;
                found = Some(SilentPad {
                    port,
                    identity: signal.identity,
                    path: signal.path,
                });
            }
        }
        found
    }

    /// The pads that need no check in the next run of the boot: the pads
    /// heard or told in this run, and the checked pads of the runs before
    /// that did not come back.
    pub fn checked(&self) -> Vec<(DeviceIdentity, DeviceIdentity)> {
        let mut keys = self.checked.clone();
        for watched in self.watched.iter().flatten().filter(|w| w.done) {
            let key = (watched.signal.identity, watched.signal.path);
            if !keys.contains(&key) {
                keys.push(key);
            }
        }
        keys
    }

    /// The number of pads of this run that were heard or told. It grows
    /// when a pad is heard or told, so the frontend can compare it in every
    /// frame and keep [`SilentPads::checked`] only on a change. It
    /// allocates nothing.
    pub fn checked_count(&self) -> usize {
        self.watched.iter().flatten().filter(|w| w.done).count()
    }

    fn find(&mut self, plug: u32) -> Option<&mut Watched> {
        self.watched
            .iter_mut()
            .flatten()
            .find(|w| w.signal.plug == plug)
    }
}

fn plugged(activity: &PadActivity, plug: u32) -> bool {
    activity.ports.iter().flatten().any(|s| s.plug == plug)
}

#[cfg(test)]
mod tests {
    use neptron_testkit::SimClock;

    use super::*;
    use neptron_backend::Clock;

    const STEP_NS: u64 = 1_000_000_000;

    fn signal(plug: u32, heard: bool) -> PadSignal {
        PadSignal {
            plug,
            identity: DeviceIdentity(10 + u64::from(plug)),
            path: DeviceIdentity(20 + u64::from(plug)),
            heard,
        }
    }

    fn on_port(port: usize, signal: PadSignal) -> PadActivity {
        let mut activity = PadActivity::default();
        activity.ports[port] = Some(signal);
        activity
    }

    /// Runs `seconds` frames of one second, with `presses` each, and gives
    /// every pad found.
    fn run(
        rule: &mut SilentPads,
        clock: &SimClock,
        activity: &PadActivity,
        seconds: u32,
        presses: u32,
        check: bool,
    ) -> Vec<SilentPad> {
        let mut found = Vec::new();
        for _ in 0..seconds {
            found.extend(rule.update(activity, presses, clock.now_ns(), check));
            clock.advance(STEP_NS);
        }
        found
    }

    #[test]
    fn a_pad_that_is_never_heard_while_others_press_gets_one_notice() {
        let clock = SimClock::new();
        let mut rule = SilentPads::new(&[]);
        let activity = on_port(1, signal(1, false));
        assert_eq!(run(&mut rule, &clock, &activity, 20, 1, true), []);
        let found = run(&mut rule, &clock, &activity, 100, 1, true);
        assert_eq!(
            found,
            [SilentPad {
                port: 1,
                identity: DeviceIdentity(11),
                path: DeviceIdentity(21)
            }]
        );
        assert_eq!(rule.checked(), [(DeviceIdentity(11), DeviceIdentity(21))]);
        assert_eq!(rule.checked_count(), 1);
    }

    #[test]
    fn no_notice_before_the_time_even_with_many_presses() {
        let clock = SimClock::new();
        let mut rule = SilentPads::new(&[]);
        let activity = on_port(0, signal(1, false));
        let seconds = u32::try_from(SILENT_PAD_AFTER_NS / STEP_NS).unwrap();
        assert_eq!(run(&mut rule, &clock, &activity, seconds, 50, true), []);
        assert_eq!(run(&mut rule, &clock, &activity, 1, 0, true).len(), 1);
    }

    #[test]
    fn no_notice_while_nobody_presses_anything() {
        let clock = SimClock::new();
        let mut rule = SilentPads::new(&[]);
        let activity = on_port(0, signal(1, false));
        assert_eq!(run(&mut rule, &clock, &activity, 600, 0, true), []);
        let short = SILENT_PAD_OTHER_PRESSES - 1;
        assert_eq!(run(&mut rule, &clock, &activity, 1, short, true), []);
        assert_eq!(run(&mut rule, &clock, &activity, 1, 1, true).len(), 1);
    }

    #[test]
    fn a_heard_pad_never_gets_the_notice_in_its_plug() {
        let clock = SimClock::new();
        let mut rule = SilentPads::new(&[]);
        run(&mut rule, &clock, &on_port(0, signal(1, false)), 5, 1, true);
        run(&mut rule, &clock, &on_port(0, signal(1, true)), 1, 1, true);
        // The mapper never clears `heard` in a plug; the rule keeps it
        // done also if it read a stale value.
        let later = run(
            &mut rule,
            &clock,
            &on_port(0, signal(1, false)),
            100,
            5,
            true,
        );
        assert_eq!(later, []);
        assert_eq!(rule.checked(), [(DeviceIdentity(11), DeviceIdentity(21))]);
    }

    #[test]
    fn a_game_run_records_but_never_tells() {
        let clock = SimClock::new();
        let mut rule = SilentPads::new(&[]);
        let activity = on_port(0, signal(1, false));
        assert_eq!(run(&mut rule, &clock, &activity, 100, 5, false), []);
        assert_eq!(rule.checked(), []);
        assert_eq!(rule.checked_count(), 0);
        run(&mut rule, &clock, &on_port(0, signal(1, true)), 1, 0, false);
        assert_eq!(rule.checked_count(), 1, "a pad heard in a game is checked");
    }

    #[test]
    fn a_new_plug_is_checked_again_and_a_checked_pad_of_an_earlier_run_is_not() {
        let clock = SimClock::new();
        let earlier = [(DeviceIdentity(11), DeviceIdentity(21))];
        let mut rule = SilentPads::new(&earlier);
        let first = on_port(0, signal(1, false));
        assert_eq!(run(&mut rule, &clock, &first, 100, 5, true), []);
        // Unplugged, then plugged again: a new plug, checked again.
        run(&mut rule, &clock, &PadActivity::default(), 1, 0, true);
        let again = on_port(2, signal(2, false));
        let found = run(&mut rule, &clock, &again, 100, 5, true);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].port, 2);
    }

    #[test]
    fn two_silent_pads_come_one_frame_after_the_other() {
        let clock = SimClock::new();
        let mut rule = SilentPads::new(&[]);
        let mut activity = on_port(0, signal(1, false));
        activity.ports[3] = Some(signal(2, false));
        assert_eq!(rule.update(&activity, 0, clock.now_ns(), true), None);
        clock.advance(SILENT_PAD_AFTER_NS);
        let presses = SILENT_PAD_OTHER_PRESSES;
        let first = rule.update(&activity, presses, clock.now_ns(), true);
        let second = rule.update(&activity, 0, clock.now_ns(), true);
        let third = rule.update(&activity, 0, clock.now_ns(), true);
        assert_eq!(first.map(|p| p.port), Some(0));
        assert_eq!(second.map(|p| p.port), Some(3));
        assert_eq!(third, None);
    }
}
