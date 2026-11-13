//! The frontend: the wiring of the crates, and no logic of its own.
//!
//! A run is one process. It shows the game list, or it runs one game,
//! because a process holds one core. [`Shell::boot`] starts a run from the
//! relaunch file of the run before; [`Shell::step`] runs one frame. The
//! binary picks the backend (the KMS display or the desktop window), and
//! the tier 2 tests run the same code on the fakes of `neptron-testkit`.

#![forbid(unsafe_code)]

pub mod atomic;
pub mod config;
pub mod control;
pub mod devices;
pub mod display;
pub mod display_wait;
pub mod library;
pub mod logbook;
pub mod nav;
pub mod options;
pub mod outbox;
pub mod pages;
pub mod paths;
pub mod power;
pub mod presets;
pub mod record;
pub mod relaunch;
pub mod shell;
pub mod stop;
pub mod storage;

pub use paths::Paths;
pub use relaunch::{NextRun, RELAUNCH_EXIT_CODE};
pub use shell::{BootError, Booted, ControlInput, Devices, MENU_SOURCE, Outcome, Runtime, Shell};

use neptron_backend::{AudioSink, Clock, Display, InputSource};

/// Runs frames until the run ends, and serves the control protocol
/// between them.
pub fn run<D: Display, S: AudioSink, I: InputSource, C: Clock + Clone>(
    shell: &mut Shell<D, S, I, C>,
    mut control: Option<&mut control::ControlPort>,
) -> Outcome {
    loop {
        if let Some(port) = control.as_deref_mut() {
            port.serve(shell, shell.now_ns());
        }
        if let Some(outcome) = shell.step() {
            return outcome;
        }
    }
}
