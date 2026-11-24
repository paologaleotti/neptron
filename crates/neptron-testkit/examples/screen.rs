//! The screen simulator: `cargo xtask screen ...` runs this example with
//! its arguments. See `docs/spec/screen-simulator.md`.

#![forbid(unsafe_code)]

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    match neptron_testkit::screen::cli::run(&args) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("screen: {e}");
            ExitCode::FAILURE
        }
    }
}
