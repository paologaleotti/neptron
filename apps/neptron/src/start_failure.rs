//! The notice and the log lines of a game that does not start: the core
//! refuses the game file, or it asks to stop before the game shows or
//! after an error at the launch. The game list shows the notice after the
//! relaunch, so the player always learns what failed.

use neptron_session::LaunchError;

/// The most characters of the words of the core in a notice, so the notice
/// keeps to the lines of a dialog on the 320x240 canvas of the UI.
pub const CORE_TEXT_CHARS_MAX: usize = 60;

/// The notice of a launch of `name` that failed with `error`.
pub fn launch_notice(name: &str, error: &LaunchError) -> String {
    match error {
        LaunchError::Refused { core_errors } => format!(
            "{name} did not start: the core refused the game file.{} Check that the file is complete and is for this system.",
            says(first(core_errors, None))
        ),
        LaunchError::CoreStopped { core_errors } => stopped_notice(name, first(core_errors, None)),
        other => format!("{name} did not start: {other}."),
    }
}

/// The notice of a game `name` that the core stopped at its start, with
/// `core_text` the words of the core, if any.
pub fn stopped_notice(name: &str, core_text: Option<&str>) -> String {
    format!(
        "{name} did not start: the core stopped it at the start.{} Check that the game has all its files.",
        says(core_text)
    )
}

/// The words of the core for a notice: its first error line of the
/// launch, or else its last message.
pub fn first<'a>(core_errors: &'a [String], message: Option<&'a str>) -> Option<&'a str> {
    core_errors
        .first()
        .map(String::as_str)
        .or(message)
        .map(str::trim)
        .filter(|t| !t.is_empty())
}

/// The log lines of a launch that failed with `error`: one for each error
/// line of the core.
pub fn launch_log(error: &LaunchError) -> Vec<String> {
    match error {
        LaunchError::Refused { core_errors } | LaunchError::CoreStopped { core_errors } => {
            core_log(core_errors)
        }
        _ => Vec::new(),
    }
}

/// One log line for each error line of the core during the launch.
pub fn core_log(core_errors: &[String]) -> Vec<String> {
    core_errors
        .iter()
        .map(|e| format!("the core logged at the launch: {e}"))
        .collect()
}

/// The sentence with the words of the core, with a leading space, cut to
/// [`CORE_TEXT_CHARS_MAX`] characters. Nothing when the core said nothing.
fn says(core_text: Option<&str>) -> String {
    let Some(text) = core_text else {
        return String::new();
    };
    let cut = if text.chars().count() > CORE_TEXT_CHARS_MAX {
        let head: String = text.chars().take(CORE_TEXT_CHARS_MAX - 3).collect();
        format!("{}...", head.trim_end())
    } else {
        text.to_owned()
    };
    format!(" The core says: \"{cut}\".")
}

#[cfg(test)]
mod tests {
    use super::*;

    const KINST: &str = "HDD: kinst.chd not found";

    #[test]
    fn a_refusal_quotes_the_core_and_says_what_to_check() {
        let error = LaunchError::Refused {
            core_errors: vec![KINST.to_owned(), "second".to_owned()],
        };
        assert_eq!(
            launch_notice("kinst", &error),
            "kinst did not start: the core refused the game file. The core says: \"HDD: kinst.chd not found\". Check that the file is complete and is for this system."
        );
        assert_eq!(
            launch_log(&error),
            [
                "the core logged at the launch: HDD: kinst.chd not found",
                "the core logged at the launch: second"
            ]
        );
    }

    #[test]
    fn a_stop_at_the_start_with_no_words_of_the_core() {
        let error = LaunchError::CoreStopped {
            core_errors: Vec::new(),
        };
        assert_eq!(
            launch_notice("Test Game", &error),
            "Test Game did not start: the core stopped it at the start. Check that the game has all its files."
        );
        assert!(launch_log(&error).is_empty());
    }

    #[test]
    fn another_launch_error_keeps_its_text() {
        let error = LaunchError::Writer(std::io::Error::other("no thread"));
        assert_eq!(
            launch_notice("Test Game", &error),
            "Test Game did not start: cannot start the save RAM writer: no thread."
        );
        assert!(launch_log(&error).is_empty());
    }

    #[test]
    fn the_error_line_comes_before_the_message() {
        let errors = [KINST.to_owned()];
        assert_eq!(first(&errors, Some("a message")), Some(KINST));
        assert_eq!(first(&[], Some(" a message ")), Some("a message"));
        assert_eq!(first(&[], Some("  ")), None);
        assert_eq!(first(&[], None), None);
    }

    #[test]
    fn long_words_of_the_core_are_cut() {
        let long = "x".repeat(CORE_TEXT_CHARS_MAX + 1);
        let text = stopped_notice("g", Some(&long));
        let quoted = format!("\"{}...\"", "x".repeat(CORE_TEXT_CHARS_MAX - 3));
        assert!(text.contains(&quoted), "{text}");
        let exact = "y".repeat(CORE_TEXT_CHARS_MAX);
        assert!(stopped_notice("g", Some(&exact)).contains(&format!("\"{exact}\"")));
    }
}
