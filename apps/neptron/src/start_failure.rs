//! The notice and the log lines of a game that the core does not start or
//! stops: the core refuses the game file, asks to stop before the game
//! shows or soon after an error at the launch (S1, S2), or ends a game
//! that ran (S3). The game list shows the notice after the relaunch, so a
//! core stop never returns to the list in silence.

use neptron_session::LaunchError;

/// The most characters of the words of the core in a notice, so the notice
/// keeps to the lines of a dialog on the 320x240 canvas of the UI.
pub const CORE_TEXT_CHARS_MAX: usize = 60;

/// The notice of a launch of `name` that failed with `error`.
pub fn launch_notice(name: &str, error: &LaunchError) -> String {
    match error {
        LaunchError::Refused { core_errors, .. } => format!(
            "{name} did not start: the core refused the game file.{} Check that the file is complete and is for this system.",
            says(first(core_errors, None))
        ),
        LaunchError::CoreStopped { core_errors, .. } => {
            stopped_notice(name, first(core_errors, None))
        }
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

/// The notice of a game `name` that ran and that the core ended (S3),
/// with `core_text` the words of the core, if any.
pub fn ended_notice(name: &str, core_text: Option<&str>) -> String {
    format!(
        "{name} stopped: the core ended the game.{}",
        says(core_text)
    )
}

/// The words of the core for a notice: its first error line of the
/// launch that is not blank, or else its message.
pub fn first<'a>(core_errors: &'a [String], message: Option<&'a str>) -> Option<&'a str> {
    core_errors
        .iter()
        .map(String::as_str)
        .chain(message)
        .find(|t| !clean(t).is_empty())
}

/// The log lines of a launch that failed with `error`: one for each error
/// line of the core, and one for the lines the log of the host lost.
pub fn launch_log(error: &LaunchError) -> Vec<String> {
    match error {
        LaunchError::Refused {
            core_errors,
            dropped_logs,
            ..
        }
        | LaunchError::CoreStopped {
            core_errors,
            dropped_logs,
        } => {
            let mut lines = core_log(core_errors);
            lines.extend(lost_lines(*dropped_logs));
            lines
        }
        _ => Vec::new(),
    }
}

/// The log line for `dropped` records of the launch that the log of the
/// host lost, or none. The host keeps the first error lines of each call
/// apart, and the session keeps the first
/// [`START_ERRORS_MAX`](neptron_session::START_ERRORS_MAX) of the launch,
/// so the line says that much and no more.
pub fn lost_lines(dropped: usize) -> Option<String> {
    (dropped > 0).then(|| {
        format!(
            "the core log lost {dropped} lines at the launch; the first {} error lines of the launch are kept",
            neptron_session::START_ERRORS_MAX
        )
    })
}

/// One log line for each error line of the core during the launch.
pub fn core_log(core_errors: &[String]) -> Vec<String> {
    core_errors
        .iter()
        .map(|e| format!("the core logged at the launch: {e}"))
        .collect()
}

/// The text of a core for a notice: each control character, line breaks
/// and tabs included, and the Unicode line and paragraph separators
/// become a space, and the ends are trimmed. A core text is not trusted to
/// keep to one line of the dialog.
fn clean(text: &str) -> String {
    let spaced: String = text
        .chars()
        .map(|c| {
            if c.is_control() || matches!(c, '\u{2028}' | '\u{2029}') {
                ' '
            } else {
                c
            }
        })
        .collect();
    spaced.trim().to_owned()
}

/// The sentence with the words of the core, with a leading space, cleaned
/// and cut to [`CORE_TEXT_CHARS_MAX`] characters. Nothing when the core
/// said nothing.
fn says(core_text: Option<&str>) -> String {
    let Some(text) = core_text.map(clean).filter(|t| !t.is_empty()) else {
        return String::new();
    };
    let cut = if text.chars().count() > CORE_TEXT_CHARS_MAX {
        let head: String = text.chars().take(CORE_TEXT_CHARS_MAX - 3).collect();
        format!("{}...", head.trim_end())
    } else {
        text
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
            dropped_logs: 0,
            hw_refusal: None,
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
            dropped_logs: 0,
        };
        assert_eq!(
            launch_notice("Test Game", &error),
            "Test Game did not start: the core stopped it at the start. Check that the game has all its files."
        );
        assert!(launch_log(&error).is_empty());
    }

    #[test]
    fn the_log_says_how_many_lines_the_launch_lost() {
        let error = LaunchError::CoreStopped {
            core_errors: vec![KINST.to_owned()],
            dropped_logs: 12,
        };
        assert_eq!(
            launch_log(&error),
            [
                "the core logged at the launch: HDD: kinst.chd not found",
                "the core log lost 12 lines at the launch; the first 8 error lines of the launch are kept"
            ]
        );
        assert_eq!(lost_lines(0), None);
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
        assert_eq!(first(&[], Some(" a message ")), Some(" a message "));
        assert!(stopped_notice("g", Some(" a message ")).contains("\"a message\""));
        assert_eq!(first(&[], Some("  ")), None);
        assert_eq!(first(&[], None), None);
    }

    #[test]
    fn a_core_that_ends_a_game_gets_its_own_notice() {
        assert_eq!(
            ended_notice("Test Game", Some("Game over")),
            "Test Game stopped: the core ended the game. The core says: \"Game over\"."
        );
        assert_eq!(
            ended_notice("Test Game", None),
            "Test Game stopped: the core ended the game."
        );
        assert_eq!(
            ended_notice("Test Game", Some(" \n ")),
            "Test Game stopped: the core ended the game."
        );
    }

    #[test]
    fn a_blank_error_line_falls_back_to_the_next_line_or_the_message() {
        let errors = ["  ".to_owned(), "\n".to_owned(), KINST.to_owned()];
        assert_eq!(first(&errors, Some("message")), Some(KINST));
        let blank = ["  ".to_owned()];
        assert_eq!(first(&blank, Some("message")), Some("message"));
        assert_eq!(first(&blank, Some("\t")), None);
    }

    #[test]
    fn control_characters_of_the_core_become_spaces() {
        let text = stopped_notice("g", Some("line one\nline two\r\tend\u{7}"));
        assert!(text.contains("\"line one line two  end\""), "{text}");
        assert!(!text.contains(['\n', '\r', '\t', '\u{7}']), "{text}");
        let text = ended_notice("g", Some("one\u{2028}two\u{2029}three"));
        assert!(text.contains("\"one two three\""), "{text}");
    }

    #[test]
    fn a_multi_byte_text_is_cut_on_a_character() {
        let long = "é".repeat(CORE_TEXT_CHARS_MAX + 5);
        let text = stopped_notice("g", Some(&long));
        let quoted = format!("\"{}...\"", "é".repeat(CORE_TEXT_CHARS_MAX - 3));
        assert!(text.contains(&quoted), "{text}");
        let wide = "漢".repeat(CORE_TEXT_CHARS_MAX + 1);
        assert!(
            ended_notice("g", Some(&wide))
                .contains(&format!("{}...", "漢".repeat(CORE_TEXT_CHARS_MAX - 3)))
        );
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
