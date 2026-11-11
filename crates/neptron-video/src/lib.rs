//! The ODREN engine: core geometry to policy to safety to display.
//! Debounce, commit, mute and the switch trace.
//!
//! [`Odren`] decides and does no I/O: it takes the source of each core
//! frame and the results of the display calls. [`VideoEngine`] applies its
//! directives to a [`neptron_backend::Display`]. Only a `ValidatedMode`
//! reaches `Display::set_mode`, and the engine sets only the token that
//! `neptron_safety::validate` returned for the winner.

#![forbid(unsafe_code)]

mod cache;
mod decision;
mod engine;
mod monitor;
mod odren;
mod scene;
mod settings;
mod trace;

pub use cache::CACHE_ENTRIES;
pub use decision::{Chosen, Refusal, Unshowable, Veto};
pub use engine::{Presented, VideoEngine, VideoError};
pub use monitor::Monitor;
pub use odren::{Current, DEBOUNCE_FRAMES, Directive, Odren};
pub use scene::build_scene;
pub use settings::{SettingsError, policy_settings};
pub use trace::{Outcome, SwitchRecord, SwitchTrace, TRACE_CAPACITY, Trigger};

/// The policy types that the engine's callers name. Only this crate may
/// depend on `neptron-modeline`.
pub use neptron_modeline::policy::{
    Dropped, HorizontalStrategy, Notice, PictureStyle, Placement, PolicyError, PolicySettings,
    RefreshKind, RefreshPolicy, RefreshTarget, Refused, Rung, StyleRow, Timing,
};
