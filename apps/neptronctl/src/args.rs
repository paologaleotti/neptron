//! The command line, parsed by hand: the grammar is a few fixed words, and
//! a parser crate would cost more compile time than it saves.

use std::path::PathBuf;

use neptron_ipc::{
    ClockStepArgs, Command, InputSendArgs, LogTailArgs, SessionLaunchArgs, SettingsGetArgs,
    SettingsSetArgs, Stage, WaitArgs, WaitCondition,
};
use serde::de::DeserializeOwned;

/// The usage text.
pub const USAGE: &str = "\
usage: neptronctl [--socket PATH] [--timeout-ms N] COMMAND

Commands:
  hello
  screenshot ui|output|scanout --out FILE
  ui tree
  input send PORT CONTROL VALUE
  input script FILE
  clock step FRAMES
  wait screen ID|state STATE|frame N [--within-ms N]
  session launch CORE ROM
  session state
  video state
  stats
  settings get KEY [--level LEVEL]
  settings set KEY LEVEL VALUE
  log tail [LINES]
  help

The socket is --socket, else $NEPTRON_CONTROL_SOCKET, else
/run/neptron/control.sock. A reply prints as one JSON line on stdout. An
error prints as one JSON line on stderr.

Exit codes: 0 ok, 1 the server answered an error, 2 usage error,
3 cannot connect, 4 no response or a broken response, 5 local file error.
";

/// The wait timeout when the command line gives none, in milliseconds.
pub const DEFAULT_WAIT_MS: u32 = 10_000;

/// The number of log lines when the command line gives none.
pub const DEFAULT_LOG_LINES: u32 = 100;

/// One parsed command line.
#[derive(Debug, Clone, PartialEq)]
pub struct Invocation {
    /// The socket path from `--socket`.
    pub socket: Option<PathBuf>,
    /// The client timeout from `--timeout-ms`.
    pub timeout_ms: Option<u32>,
    /// What to do.
    pub action: Action,
}

/// What one invocation does.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    /// Print the usage text.
    Help,
    /// Send one command and print the reply data.
    Call(Command),
    /// Take a screenshot and write the PNG to a local file.
    Screenshot {
        /// The stage.
        stage: Stage,
        /// The local file.
        out: PathBuf,
    },
    /// Read a JSON array of scripted inputs from a local file and send it.
    InputScript {
        /// The local file.
        file: PathBuf,
    },
}

/// A command line that does not follow the usage.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct UsageError(pub String);

/// Parses the arguments after the program name.
pub fn parse(args: &[String]) -> Result<Invocation, UsageError> {
    let mut rest = args;
    let mut socket = None;
    let mut timeout_ms = None;
    while let Some((flag, tail)) = rest.split_first() {
        match flag.as_str() {
            "--socket" => {
                let (path, tail) = value_of(flag, tail)?;
                socket = Some(PathBuf::from(path));
                rest = tail;
            }
            "--timeout-ms" => {
                let (ms, tail) = value_of(flag, tail)?;
                let ms: u32 = number(flag, ms)?;
                // A socket refuses a zero timeout, and no reply comes in no
                // time.
                if ms == 0 {
                    return Err(usage("--timeout-ms must be above 0"));
                }
                timeout_ms = Some(ms);
                rest = tail;
            }
            _ => break,
        }
    }
    let words: Vec<&str> = rest.iter().map(String::as_str).collect();
    Ok(Invocation {
        socket,
        timeout_ms,
        action: action(&words)?,
    })
}

fn action(words: &[&str]) -> Result<Action, UsageError> {
    let call = |command| Ok(Action::Call(command));
    match words {
        [] => Err(usage("no command given")),
        ["help" | "--help" | "-h"] => Ok(Action::Help),
        ["hello"] => call(Command::Hello),
        ["screenshot", stage, "--out", out] => Ok(Action::Screenshot {
            stage: wire_name("stage", stage)?,
            out: PathBuf::from(out),
        }),
        ["ui", "tree"] => call(Command::UiTree),
        ["input", "send", port, control, value] => call(Command::InputSend(InputSendArgs {
            port: number("PORT", port)?,
            control: wire_name("control", control)?,
            value: number("VALUE", value)?,
        })),
        ["input", "script", file] => Ok(Action::InputScript {
            file: PathBuf::from(file),
        }),
        ["clock", "step", frames] => call(Command::ClockStep(ClockStepArgs {
            frames: number("FRAMES", frames)?,
        })),
        ["wait", kind, target, options @ ..] => {
            let until = match *kind {
                "screen" => WaitCondition::Screen((*target).to_owned()),
                "state" => WaitCondition::SessionState(wire_name("state", target)?),
                "frame" => WaitCondition::Frame(number("N", target)?),
                other => return Err(usage(&format!("unknown wait condition {other:?}"))),
            };
            let timeout_ms = match options {
                [] => DEFAULT_WAIT_MS,
                ["--within-ms", ms] => number("--within-ms", ms)?,
                _ => return Err(usage("wait takes only --within-ms N after the condition")),
            };
            call(Command::Wait(WaitArgs { until, timeout_ms }))
        }
        ["session", "launch", core, rom] => call(Command::SessionLaunch(SessionLaunchArgs {
            core: (*core).to_owned(),
            rom: (*rom).to_owned(),
        })),
        ["session", "state"] => call(Command::SessionState),
        ["video", "state"] => call(Command::VideoState),
        ["stats"] => call(Command::Stats),
        ["settings", "get", key, options @ ..] => {
            let level = match options {
                [] => None,
                ["--level", level] => Some(wire_name("level", level)?),
                _ => return Err(usage("settings get takes only --level LEVEL after the key")),
            };
            call(Command::SettingsGet(SettingsGetArgs {
                key: (*key).to_owned(),
                level,
            }))
        }
        ["settings", "set", key, level, value] => call(Command::SettingsSet(SettingsSetArgs {
            key: (*key).to_owned(),
            level: wire_name("level", level)?,
            value: setting_value(value),
        })),
        ["log", "tail"] => call(Command::LogTail(LogTailArgs {
            lines: DEFAULT_LOG_LINES,
        })),
        ["log", "tail", lines] => call(Command::LogTail(LogTailArgs {
            lines: number("LINES", lines)?,
        })),
        _ => Err(usage(&format!("unknown command {:?}", words.join(" ")))),
    }
}

/// A setting value: JSON when the text is JSON, else the text as a string,
/// so that `native` and `"native"` give the same value.
fn setting_value(text: &str) -> serde_json::Value {
    serde_json::from_str(text).unwrap_or_else(|_| serde_json::Value::String(text.to_owned()))
}

/// Reads an enum value by its wire name, so that the command line and the
/// protocol use the same names.
fn wire_name<T: DeserializeOwned>(what: &str, text: &str) -> Result<T, UsageError> {
    serde_json::from_value(serde_json::Value::String(text.to_owned()))
        .map_err(|_| usage(&format!("{text:?} is not a {what}")))
}

fn number<T: std::str::FromStr>(what: &str, text: &str) -> Result<T, UsageError> {
    text.parse()
        .map_err(|_| usage(&format!("{what} {text:?} is not a number in range")))
}

fn value_of<'a>(flag: &str, tail: &'a [String]) -> Result<(&'a str, &'a [String]), UsageError> {
    tail.split_first()
        .map(|(v, t)| (v.as_str(), t))
        .ok_or_else(|| usage(&format!("{flag} needs a value")))
}

fn usage(message: &str) -> UsageError {
    UsageError(format!("{message}; run `neptronctl help` for the usage"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use neptron_ipc::{InputSendArgs, SessionState, SettingLevel};
    use neptron_types::Control;

    fn parse_words(line: &str) -> Result<Invocation, UsageError> {
        let args: Vec<String> = line.split_whitespace().map(str::to_owned).collect();
        parse(&args)
    }

    fn call(line: &str) -> Command {
        match parse_words(line).unwrap().action {
            Action::Call(command) => command,
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn global_options_come_before_the_command() {
        let inv = parse_words("--socket /tmp/s --timeout-ms 250 hello").unwrap();
        assert_eq!(inv.socket, Some(PathBuf::from("/tmp/s")));
        assert_eq!(inv.timeout_ms, Some(250));
        assert_eq!(inv.action, Action::Call(Command::Hello));
    }

    #[test]
    fn every_command_parses() {
        assert_eq!(call("ui tree"), Command::UiTree);
        assert_eq!(
            call("input send 1 left_x -32768"),
            Command::InputSend(InputSendArgs {
                port: 1,
                control: Control::LeftX,
                value: -32768,
            })
        );
        assert_eq!(
            call("clock step 60"),
            Command::ClockStep(ClockStepArgs { frames: 60 })
        );
        assert_eq!(
            call("wait state quick_menu --within-ms 500"),
            Command::Wait(WaitArgs {
                until: WaitCondition::SessionState(SessionState::QuickMenu),
                timeout_ms: 500,
            })
        );
        assert_eq!(
            call("wait screen game_list"),
            Command::Wait(WaitArgs {
                until: WaitCondition::Screen("game_list".into()),
                timeout_ms: DEFAULT_WAIT_MS,
            })
        );
        assert_eq!(
            call("wait frame 7"),
            Command::Wait(WaitArgs {
                until: WaitCondition::Frame(7),
                timeout_ms: DEFAULT_WAIT_MS,
            })
        );
        assert_eq!(
            call("session launch fceumm /roms/a.nes"),
            Command::SessionLaunch(SessionLaunchArgs {
                core: "fceumm".into(),
                rom: "/roms/a.nes".into(),
            })
        );
        assert_eq!(call("session state"), Command::SessionState);
        assert_eq!(call("video state"), Command::VideoState);
        assert_eq!(call("stats"), Command::Stats);
        assert_eq!(
            call("settings get video.style --level game"),
            Command::SettingsGet(SettingsGetArgs {
                key: "video.style".into(),
                level: Some(SettingLevel::Game),
            })
        );
        assert_eq!(
            call("log tail"),
            Command::LogTail(LogTailArgs {
                lines: DEFAULT_LOG_LINES
            })
        );
        assert_eq!(
            call("log tail 5"),
            Command::LogTail(LogTailArgs { lines: 5 })
        );
        assert_eq!(
            parse_words("screenshot scanout --out a.png")
                .unwrap()
                .action,
            Action::Screenshot {
                stage: Stage::Scanout,
                out: PathBuf::from("a.png"),
            }
        );
        assert_eq!(
            parse_words("input script s.json").unwrap().action,
            Action::InputScript {
                file: PathBuf::from("s.json"),
            }
        );
        assert_eq!(parse_words("help").unwrap().action, Action::Help);
    }

    #[test]
    fn a_setting_value_is_json_or_else_a_string() {
        let value = |text: &str| match call(&format!("settings set k global {text}")) {
            Command::SettingsSet(args) => args.value,
            other => panic!("{other:?}"),
        };
        assert_eq!(value("3"), serde_json::json!(3));
        assert_eq!(value("true"), serde_json::json!(true));
        assert_eq!(value("native"), serde_json::json!("native"));
        assert_eq!(value("\"native\""), serde_json::json!("native"));
    }

    #[test]
    fn bad_command_lines_are_usage_errors() {
        for line in [
            "",
            "--socket",
            "--timeout-ms soon hello",
            "--timeout-ms 0 hello",
            "warp",
            "ui",
            "screenshot crt --out a.png",
            "screenshot ui",
            "input send 0 turbo 1",
            "input send 256 a 1",
            "input send 0 a 40000",
            "clock step -1",
            "wait moon 1",
            "wait frame x",
            "wait frame 1 --forever",
            "settings get k --level planet",
            "settings set k planet 1",
            "log tail all",
            "hello extra",
        ] {
            let err = parse_words(line).unwrap_err();
            assert!(err.0.contains("neptronctl help"), "{line}: {err}");
        }
    }
}
