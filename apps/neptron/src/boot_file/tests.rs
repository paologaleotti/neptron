use neptron_settings::{DISPLAY_OUTPUT, DISPLAY_TV, Schema, Value};

use super::*;

const PI_FILE: &str = include_str!("../../../../os/board/rpi/neptron.toml");
const PC_FILE: &str = include_str!("../../../../os/board/pc/neptron.toml");

fn read(text: &str) -> FileRead {
    FileRead::Read(text.as_bytes().to_vec())
}

fn decide_text(text: &str, stored: Option<&str>) -> Decision {
    decide(&Schema::builtin(), &read(text), stored)
}

fn text(value: &str) -> Value {
    Value::Text(value.to_owned())
}

#[test]
fn a_new_file_copies_its_values_and_gives_its_hash() {
    let file = "[display]\ntv = \"pal\"\noutput = \"Composite-1\"\n";
    let decision = decide_text(file, None);
    assert_eq!(
        decision.changes,
        vec![
            (DISPLAY_OUTPUT, text("Composite-1")),
            (DISPLAY_TV, text("pal"))
        ]
    );
    assert_eq!(
        decision.hash.as_deref(),
        Some(sha256::hex(file.as_bytes()).as_str())
    );
    assert!(decision.problems.is_empty(), "{:?}", decision.problems);
    assert_eq!(
        decision.log,
        ["neptron.toml has changed: copied display.output, display.tv into config/global.toml"]
    );
}

#[test]
fn a_file_with_the_stored_hash_changes_nothing() {
    let file = "[display]\ntv = \"pal\"\n";
    let hash = sha256::hex(file.as_bytes());
    let decision = decide_text(file, Some(&hash));
    assert!(decision.changes.is_empty());
    assert_eq!(decision.hash, None);
    assert!(decision.problems.is_empty());
}

#[test]
fn a_changed_file_is_imported_again() {
    let old = sha256::hex(b"[display]\ntv = \"pal\"\n");
    let decision = decide_text("[display]\ntv = \"ntsc\"\n", Some(&old));
    assert_eq!(decision.changes, vec![(DISPLAY_TV, text("ntsc"))]);
    assert!(decision.hash.is_some());
}

#[test]
fn a_file_saved_on_windows_with_a_bom_and_crlf_is_imported() {
    let file = "\u{feff}[display]\r\ntv = \"pal\"\r\npi_dac = \"none\"\r\n";
    let decision = decide_text(file, None);
    assert_eq!(decision.changes, vec![(DISPLAY_TV, text("pal"))]);
    assert!(decision.problems.is_empty(), "{:?}", decision.problems);
    assert!(decision.hash.is_some());
}

#[test]
fn pi_dac_with_a_bom_and_crlf_gives_the_notice_with_its_line() {
    let file = "\u{feff}[display]\r\ntv = \"pal\"\r\npi_dac = \"vga666\"\r\n";
    let decision = decide_text(file, None);
    assert_eq!(decision.changes, vec![(DISPLAY_TV, text("pal"))]);
    assert_eq!(
        decision.problems,
        [format!(
            "neptron.toml, line 3: display.pi_dac: {PI_DAC_NOTICE}."
        )]
    );
}

#[test]
fn the_raw_read_of_pi_dac_takes_a_bom_and_crlf() {
    assert_eq!(
        pi_dac("\u{feff}[display]\r\npi_dac = \"none\"\r\n").as_deref(),
        Some(PI_DAC_NONE)
    );
    assert_eq!(pi_dac("display.pi_dac = 1\r\n"), None);
}

#[test]
fn an_unknown_key_is_ignored_and_listed_with_its_line() {
    let decision = decide_text("[display]\ntv = \"pal\"\nwifi_ssid = \"home\"\n", None);
    assert_eq!(decision.changes, vec![(DISPLAY_TV, text("pal"))]);
    assert_eq!(
        decision.problems,
        ["neptron.toml, line 3: display.wifi_ssid is not a setting. It is ignored."]
    );
    assert!(decision.hash.is_some(), "the rest of the file is imported");
}

#[test]
fn a_refused_value_is_ignored_and_listed() {
    let decision = decide_text("[display]\ntv = \"pal\"\nmonitor = 15\n", None);
    assert_eq!(decision.changes, vec![(DISPLAY_TV, text("pal"))]);
    assert_eq!(decision.problems.len(), 1, "{:?}", decision.problems);
    let problem = &decision.problems[0];
    assert!(
        problem.starts_with("neptron.toml, line 3: display.monitor: ")
            && problem.ends_with(". The value is ignored."),
        "{problem}"
    );
}

#[test]
fn a_value_out_of_the_choices_is_refused() {
    let decision = decide_text("[display]\ntv = \"pal_x\"\n", None);
    assert!(decision.changes.is_empty());
    assert_eq!(decision.problems.len(), 1, "{:?}", decision.problems);
    assert!(decision.problems[0].contains("display.tv"));
}

#[test]
fn pi_dac_none_needs_no_notice() {
    let decision = decide_text("[display]\npi_dac = \"none\"\n", None);
    assert!(decision.changes.is_empty());
    assert!(decision.problems.is_empty(), "{:?}", decision.problems);
}

#[test]
fn another_pi_dac_gives_the_notice_and_is_ignored() {
    for file in [
        "[display]\npi_dac = \"vga666\"\n",
        "display.pi_dac = \"vga666\"\n",
        "[display]\npi_dac = 1\n",
    ] {
        let decision = decide_text(file, None);
        assert!(decision.changes.is_empty(), "{file}");
        assert_eq!(decision.problems.len(), 1, "{file}");
        assert!(
            decision.problems[0].ends_with(&format!("display.pi_dac: {PI_DAC_NOTICE}.")),
            "{:?}",
            decision.problems
        );
    }
}

#[test]
fn a_file_that_does_not_parse_changes_nothing_and_keeps_the_hash() {
    let decision = decide_text("[display\ntv = \"pal\"\n", None);
    assert!(decision.changes.is_empty());
    assert_eq!(decision.hash, None, "the next start reports it again");
    assert_eq!(decision.problems.len(), 1);
    assert!(
        decision.problems[0].starts_with("neptron.toml does not parse (")
            && decision.problems[0].ends_with("). No setting changes."),
        "{:?}",
        decision.problems
    );
}

#[test]
fn a_file_that_is_not_utf8_changes_nothing() {
    let decision = decide(&Schema::builtin(), &FileRead::Read(vec![0xff, 0xfe]), None);
    assert!(decision.changes.is_empty());
    assert_eq!(decision.hash, None);
    assert_eq!(
        decision.problems,
        ["neptron.toml is not UTF-8 text. No setting changes."]
    );
}

#[test]
fn a_file_that_cannot_be_read_or_is_too_large_changes_nothing() {
    let schema = Schema::builtin();
    let unreadable = decide(
        &schema,
        &FileRead::Unreadable {
            message: "I/O error".to_owned(),
        },
        None,
    );
    assert_eq!(
        unreadable.problems,
        ["neptron.toml cannot be read (I/O error). No setting changes."]
    );
    let large = decide(&schema, &FileRead::TooLarge { bytes: 70_000 }, None);
    assert_eq!(
        large.problems,
        ["neptron.toml has 70000 bytes, more than 65536. No setting changes."]
    );
    for decision in [unreadable, large] {
        assert!(decision.changes.is_empty());
        assert_eq!(decision.hash, None);
    }
}

#[test]
fn a_missing_file_is_a_log_line_only() {
    let decision = decide(&Schema::builtin(), &FileRead::Missing, Some("abc"));
    assert!(decision.changes.is_empty());
    assert_eq!(decision.hash, None);
    assert!(decision.problems.is_empty());
    assert_eq!(
        decision.log,
        ["no neptron.toml on the boot partition; the settings stand"]
    );
}

#[test]
fn a_file_with_comments_only_stores_its_hash() {
    let decision = decide_text("# nothing set\n", None);
    assert!(decision.changes.is_empty());
    assert!(decision.hash.is_some());
    assert_eq!(
        decision.log,
        ["neptron.toml has changed: no setting to copy"]
    );
}

/// The commented key lines of a shipped file: the line with its `# `
/// removed, the key, and the choices that its comment names.
fn commented_keys(file: &str) -> Vec<(String, String, Vec<String>)> {
    file.lines()
        .filter_map(|line| {
            let line = line.strip_prefix("# ")?;
            let (key, _) = line.split_once('=')?;
            let (setting, comment) = line.split_once('#')?;
            Some((
                setting.trim().to_owned(),
                key.trim().to_owned(),
                choices(comment),
            ))
        })
        .collect()
}

/// The values a comment names: the first word of each `|` part, or the
/// names after "such as".
fn choices(comment: &str) -> Vec<String> {
    let mut out = Vec::new();
    for part in comment.split('|') {
        match part.split_once("such as") {
            Some((_, names)) => out.extend(
                names
                    .split(',')
                    .flat_map(|n| n.split(" or "))
                    .map(|n| n.trim().to_owned()),
            ),
            None => {
                let word = part.split_whitespace().next().unwrap_or_default();
                out.push(word.trim_end_matches('.').to_owned());
            }
        }
    }
    out
}

#[test]
fn the_shipped_files_set_nothing_until_a_line_is_uncommented() {
    for (name, file, keys) in [
        (
            "rpi",
            PI_FILE,
            &["output", "tv", "tv_width", "pi_dac", "orientation"][..],
        ),
        ("pc", PC_FILE, &["output", "monitor", "orientation"][..]),
    ] {
        let decision = decide_text(file, None);
        assert!(
            decision.changes.is_empty(),
            "{name}: {:?}",
            decision.changes
        );
        assert!(
            decision.problems.is_empty(),
            "{name}: {:?}",
            decision.problems
        );
        let found = commented_keys(file);
        let names: Vec<&str> = found.iter().map(|(_, key, _)| key.as_str()).collect();
        assert_eq!(names, keys, "{name}");
    }
}

#[test]
fn each_commented_key_and_its_choices_are_accepted_by_the_schema() {
    let schema = Schema::builtin();
    let monitors = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/monitors");
    for file in [PI_FILE, PC_FILE] {
        for (setting, key, choices) in commented_keys(file) {
            let uncommented = decide(&schema, &read(&format!("[display]\n{setting}\n")), None);
            assert!(
                uncommented.problems.is_empty(),
                "{setting}: {:?}",
                uncommented.problems
            );
            let dotted = format!("display.{key}");
            if dotted != PI_DAC_KEY {
                assert_eq!(uncommented.changes.len(), 1, "{setting}");
                assert_eq!(uncommented.changes[0].0, dotted);
            }
            assert!(!choices.is_empty(), "{setting}");
            for choice in choices {
                let one = format!("[display]\n{key} = \"{choice}\"\n");
                let decision = decide(&schema, &read(&one), None);
                assert!(
                    decision.problems.is_empty(),
                    "{key} = {choice}: {:?}",
                    decision.problems
                );
                if key == "monitor" && choice != "auto" {
                    assert!(
                        monitors.join(format!("{choice}.toml")).is_file(),
                        "no profile {choice}"
                    );
                }
            }
        }
    }
}
