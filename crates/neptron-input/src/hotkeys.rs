//! The hotkeys of the frontend, recognised from the RetroPad state of each
//! port. They need no picture and no UI.

use neptron_types::Control;

use crate::device::MAX_PORTS;
use crate::ports::PortStates;

/// The buttons that open the quick menu when they are all down on one
/// port.
pub const QUICK_MENU_COMBO: [Control; 2] = [Control::Select, Control::Start];

/// The buttons that, held together on one port for
/// [`BLIND_RECOVERY_HOLD_NS`], restore the safe display mode.
pub const BLIND_RECOVERY_COMBO: [Control; 2] = [Control::Select, Control::Start];

/// How long the blind recovery combination must stay held, in
/// nanoseconds.
pub const BLIND_RECOVERY_HOLD_NS: u64 = 5_000_000_000;

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
/// A combination fires once each time it goes down: the quick menu at
/// once, the blind recovery when it has stayed down for
/// [`BLIND_RECOVERY_HOLD_NS`]. A release starts the hold time again. With
/// the default combinations, a recovery hold opens the quick menu first.
#[derive(Debug, Clone, Default)]
pub struct Hotkeys {
    quick_menu: ComboTimer,
    blind_recovery: ComboTimer,
}

impl Hotkeys {
    /// A recogniser with no combination held.
    pub fn new() -> Self {
        Self::default()
    }

    /// Checks every port at time `now_ns` and appends to `out` the hotkeys
    /// that fire, port by port, the quick menu before the recovery.
    pub fn update(&mut self, ports: &PortStates, now_ns: u64, out: &mut Vec<Hotkey>) {
        for port in 0..MAX_PORTS {
            if self
                .quick_menu
                .fires(ports, port, &QUICK_MENU_COMBO, 0, now_ns)
            {
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
}
