//! Command-line client of the control API. It talks to the frontend on the
//! control socket, prints each reply as one JSON line, and exits with a
//! stable code.

#![forbid(unsafe_code)]

mod args;
mod run;
mod selftest;

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Result<Vec<String>, _> = std::env::args_os()
        .skip(1)
        .map(|a| a.into_string())
        .collect();
    let mut out = std::io::stdout().lock();
    let mut err = std::io::stderr().lock();
    let code = match args {
        Ok(args) => run::main(
            &args,
            std::env::var_os(neptron_ipc::SOCKET_PATH_ENV),
            &mut out,
            &mut err,
        ),
        Err(bad) => run::usage_error(&format!("the argument {bad:?} is not UTF-8"), &mut err),
    };
    ExitCode::from(code)
}
