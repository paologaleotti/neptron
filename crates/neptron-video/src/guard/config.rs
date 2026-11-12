use neptron_backend::Display;

use crate::{Monitor, PolicySettings, VideoEngine};

/// The inputs of the engine that the guard owns: what the user chooses and
/// what a revert restores.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisplayConfig {
    /// The monitor profile. Only the monitor is last known good.
    pub monitor: Monitor,
    /// The policy settings: picture style and refresh policy.
    pub settings: PolicySettings,
}

/// What the engine shows now, as the guard judges it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Health {
    /// The engine has a mode on the display ([`VideoEngine::current`]).
    pub mode_set: bool,
    /// A mode the inputs accept is on the display, and a flip in it has
    /// completed.
    pub picture: bool,
    /// The mode on the display is one the inputs refuse, or one the engine
    /// does not know ([`VideoEngine::current_refused`]).
    pub refused: bool,
    /// The output is off ([`VideoEngine::blanked`]).
    pub blanked: bool,
    /// The engine has decided on its current inputs
    /// ([`crate::Odren::decision_due`] is false), so the mode on the
    /// display comes from them.
    pub decided: bool,
}

impl Health {
    /// The health of `engine` now.
    pub fn of<D: Display>(engine: &VideoEngine<D>) -> Self {
        let mode_set = engine.current().is_some();
        let refused = engine.current_refused();
        let blanked = engine.blanked();
        Self {
            mode_set,
            picture: mode_set && !refused && !blanked && !engine.muted(),
            refused,
            blanked,
            decided: !engine.odren().decision_due(),
        }
    }

    /// The configuration gives no valid picture: the output is off, a
    /// refused mode is on the display, or the engine has set no mode. A
    /// mode that waits for its first flip is none of these.
    pub fn no_picture(self) -> bool {
        self.refused || self.blanked || !self.mode_set
    }
}

impl DisplayConfig {
    /// Gives the configuration to `engine`. The next frame decides again
    /// with no debounce. `retry` comes first: it lifts a source blocked
    /// after device errors and the counts of failed modeset and blank
    /// attempts, so the new inputs get a full set of attempts. The monitor
    /// is set only when it differs, because `set_monitor` also forgets the
    /// modes the driver refused.
    pub(crate) fn apply_to<D: Display>(&self, engine: &mut VideoEngine<D>) {
        engine.retry();
        if engine.odren().monitor() != &self.monitor {
            engine.set_monitor(self.monitor.clone());
        }
        engine.set_settings(self.settings);
    }
}
