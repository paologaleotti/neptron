//! Integration tests of neptron-modeline: the solver on the presets in
//! data/monitors, its errors, its behaviour over a grid of sources, and the
//! ODREN policy.

#![forbid(unsafe_code)]

mod golden;
mod policy_golden;
mod policy_table;
mod presets;
mod safety;
mod solver;
