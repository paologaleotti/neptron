//! The ODREN engine: core geometry to policy to safety to display.
//! Debounce, commit, mute and the switch trace.

#![forbid(unsafe_code)]

mod monitor;
mod scene;
mod settings;

pub use monitor::Monitor;
pub use scene::build_scene;
pub use settings::{SettingsError, policy_settings};

/// The policy types that the engine's callers name. Only this crate may
/// depend on `neptron-modeline`.
pub use neptron_modeline::policy::{
    Dropped, HorizontalStrategy, Notice, PictureStyle, Placement, PolicyError, PolicySettings,
    RefreshKind, RefreshPolicy, RefreshTarget, Refused, Rung, StyleRow, Timing,
};
