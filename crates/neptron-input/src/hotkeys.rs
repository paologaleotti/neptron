//! The hotkeys of the frontend, recognised from the RetroPad state of each
//! port. They need no picture and no UI.

use neptron_types::Control;

use crate::device::MAX_PORTS;
use crate::ports::PortStates;

/// The buttons that, held together on one port for
/// [`QUICK_MENU_HOLD_NS`], open the quick menu.
pub const QUICK_MENU_COMBO: [Control; 2] = [Control::Select, Control::Start];

/// How long the quick menu combination must stay held, in nanoseconds.
///
/// On an arcade stick Select is the coin and Start is start: a coin and a
/// Start pressed in a quick sequence overlap for a moment, and that
/// overlap must not open the menu.
pub const QUICK_MENU_HOLD_NS: u64 = 500_000_000;

/// The buttons that, held together on one port for
/// [`BLIND_RECOVERY_HOLD_NS`], restore the safe display mode.
pub const BLIND_RECOVERY_COMBO: [Control; 2] = [Control::Select, Control::Start];

/// How long the blind recovery combination must stay held, in
/// nanoseconds.
pub const BLIND_RECOVERY_HOLD_NS: u64 = 5_000_000_000;

/// The button that, held alone on one port for the hold time of
/// [`Hotkeys::set_start_hold_ns`], opens the quick menu.
pub const START_HOLD_BUTTON: Control = Control::Start;

/// The button that, down on the same port, stops a [`START_HOLD_BUTTON`]
/// hold until the hold button is released.
pub const START_HOLD_BLOCKER: Control = Control::Select;

/// A hotkey that the frontend acts on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Hotkey {
    /// Open the quick menu.
    QuickMenu {
        /// The port where the combination was pressed.
        port: u8,
    },
    /// Restore the safe display mode.
    BlindRecovery {
        /// The port where the combination was held.
        port: u8,
    },
}

/// Recognises the hotkeys. Call [`Hotkeys::update`] once per frame, after
/// the port state took the events of the frame, also when no event came,
/// so that a hold can complete.
///
/// A combination fires once each time it goes down, when it has stayed
/// down for its hold time: the quick menu at [`QUICK_MENU_HOLD_NS`], the
/// blind recovery at [`BLIND_RECOVERY_HOLD_NS`]. The two share their
/// buttons, so a recovery hold opens the quick menu first. A release of
/// one of the buttons starts the hold time again.
///
/// The Start hold is off until [`Hotkeys::set_start_hold_ns`] gives it a
/// time. Then Start held alone on a port for that time also opens the quick
/// menu, once per hold. Alone means: Start went down while Select was up,
/// and Select has not gone down since. Other buttons do not matter.
#[derive(Debug, Clone, Default)]
pub struct Hotkeys {
    quick_menu: ComboTimer,
    blind_recovery: ComboTimer,
    start_hold: StartHold,
}

impl Hotkeys {
    /// A recogniser with no combination held and the Start hold off.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets how long Start must stay held alone to open the quick menu, in
    /// nanoseconds. 0 turns the Start hold off.
    ///
    /// A new value stops every Start hold in progress, also one that started
    /// while the hold was off: the next hold starts when Start is released
    /// and pressed again. The same value changes nothing, so the caller can
    /// pass the setting on every frame. A value set before the first
    /// [`Hotkeys::update`] applies to a Start held from power-on.
    pub fn set_start_hold_ns(&mut self, hold_ns: u64) {
        if hold_ns != self.start_hold.hold_ns {
            self.start_hold.hold_ns = hold_ns;
            for state in &mut self.start_hold.state {
                if matches!(state, HoldState::Timing(_)) {
                    *state = HoldState::Done;
                }
            }
        }
    }

    /// Ends every quick menu hold in progress: it opens no menu, and the
    /// next one starts when the combination is down again after a release.
    /// The app calls it when the menu closes, so a combination held across
    /// the close does not open the menu again. The blind recovery hold goes
    /// on.
    pub fn cancel_quick_menu(&mut self) {
        self.quick_menu.cancel();
    }

    /// How long Start must stay held alone to open the quick menu, in
    /// nanoseconds. 0 when the Start hold is off.
    pub fn start_hold_ns(&self) -> u64 {
        self.start_hold.hold_ns
    }

    /// Checks every port at time `now_ns` and appends to `out` the hotkeys
    /// that fire, port by port, the quick menu before the recovery. On one
    /// port, the Select and Start combination and the Start hold never both
    /// give a quick menu in the same hold of Start.
    pub fn update(&mut self, ports: &PortStates, now_ns: u64, out: &mut Vec<Hotkey>) {
        for port in 0..MAX_PORTS {
            let start_hold_fired = self.start_hold.fired(port);
            let combo =
                self.quick_menu
                    .fires(ports, port, &QUICK_MENU_COMBO, QUICK_MENU_HOLD_NS, now_ns);
            if combo && !start_hold_fired {
                out.push(Hotkey::QuickMenu { port });
            }
            if self.start_hold.fires(ports, port, now_ns) {
                out.push(Hotkey::QuickMenu { port });
            }
            if self.blind_recovery.fires(
                ports,
                port,
                &BLIND_RECOVERY_COMBO,
                BLIND_RECOVERY_HOLD_NS,
                now_ns,
            ) {
                out.push(Hotkey::BlindRecovery { port });
            }
        }
    }
}

/// The hold state of one combination on each port.
#[derive(Debug, Clone, Copy, Default)]
struct ComboTimer {
    /// When the combination went down on the port, while it stays down.
    since_ns: [Option<u64>; MAX_PORTS as usize],
    /// True once the combination fired in the current hold.
    fired: [bool; MAX_PORTS as usize],
}

impl ComboTimer {
    /// True when `combo` on `port` completes a hold of `hold_ns` at
    /// `now_ns` and has not fired in this hold.
    fn fires(
        &mut self,
        ports: &PortStates,
        port: u8,
        combo: &[Control],
        hold_ns: u64,
        now_ns: u64,
    ) -> bool {
        let i = usize::from(port);
        if !combo.iter().all(|control| ports.is_held(port, *control)) {
            self.since_ns[i] = None;
            self.fired[i] = false;
            return false;
        }
        let since = *self.since_ns[i].get_or_insert(now_ns);
        if self.fired[i] || now_ns.saturating_sub(since) < hold_ns {
            return false;
        }
        self.fired[i] = true;
        true
    }

    /// The holds in progress fire no more. A port where the combination
    /// was not down at the last check keeps its next hold.
    fn cancel(&mut self) {
        for (fired, since) in self.fired.iter_mut().zip(&self.since_ns) {
            *fired |= since.is_some();
        }
    }
}

/// The Start hold on each port.
#[derive(Debug, Clone, Copy, Default)]
struct StartHold {
    /// The hold time, in nanoseconds. 0 is off.
    hold_ns: u64,
    state: [HoldState; MAX_PORTS as usize],
}

/// Where a port is in a Start hold.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum HoldState {
    /// Start is up.
    #[default]
    Idle,
    /// Start is down alone since the given time. The time runs also while
    /// the hold is off.
    Timing(u64),
    /// This press of Start can give no quick menu any more: Select went
    /// down, or the hold time changed.
    Done,
    /// The hold fired on this press of Start.
    Fired,
}

impl StartHold {
    /// True when the Start hold fired on the press of Start in progress on
    /// `port`.
    fn fired(&self, port: u8) -> bool {
        self.state[usize::from(port)] == HoldState::Fired
    }

    /// True when Start on `port` completes a hold alone at `now_ns`.
    fn fires(&mut self, ports: &PortStates, port: u8, now_ns: u64) -> bool {
        let state = &mut self.state[usize::from(port)];
        if !ports.is_held(port, START_HOLD_BUTTON) {
            *state = HoldState::Idle;
            return false;
        }
        if ports.is_held(port, START_HOLD_BLOCKER) {
            if *state != HoldState::Fired {
                *state = HoldState::Done;
            }
            return false;
        }
        let since = match *state {
            HoldState::Done | HoldState::Fired => return false,
            HoldState::Idle => {
                *state = HoldState::Timing(now_ns);
                now_ns
            }
            HoldState::Timing(since) => since,
        };
        if self.hold_ns == 0 || now_ns.saturating_sub(since) < self.hold_ns {
            return false;
        }
        *state = HoldState::Fired;
        true
    }
}
