//! Integration tests of neptron-safety through its public API: modes built
//! for every shipped preset, and random modes that must validate or be
//! rejected with a reason, never panic.

#![forbid(unsafe_code)]

mod presets;
mod property;
mod switchres;
mod synth;
