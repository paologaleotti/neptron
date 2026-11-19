//! Integration tests of neptron-modeline: the solver on the presets in
//! data/monitors, its errors, its behaviour over a grid of sources, and the
//! ODREN policy.

#![forbid(unsafe_code)]

mod arcade_table;
mod golden;
mod policy_golden;
mod policy_modes;
mod policy_property;
mod policy_sweep;
mod policy_table;
mod policy_tv;
mod presets;
mod safety;
mod solver;
