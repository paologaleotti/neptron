//! `neptron.toml` on the boot partition: an inbox for `config/global.toml`.
//! When the file differs from the one imported last, its values are copied
//! into the global settings once. Otherwise the settings of the menus
//! stand.
//!
//! [`decide`] is pure: it hashes and parses what the start read, and says
//! what to copy and what to report. [`import`] reads and writes the files.
//! The import only writes settings: the display then follows them through
//! the same code as a hand edit of `global.toml`.

mod run;

pub use run::{Imported, import};

use neptron_settings::{Issue, IssueKind, Level, Schema, Value, parse_layer};

use crate::sha256;

/// The name of the file at the root of the boot partition.
pub const FILE_NAME: &str = "neptron.toml";
/// The largest file the start reads, in bytes. The shipped file is about
/// 1 KiB; a larger one is not a settings file.
pub const MAX_FILE_BYTES: u64 = 64 * 1024;
/// The key of the GPIO video DAC of a Raspberry Pi. It is not in the
/// schema: this image drives no DAC.
pub const PI_DAC_KEY: &str = "display.pi_dac";
/// The one value of [`PI_DAC_KEY`] that needs no notice.
pub const PI_DAC_NONE: &str = "none";
/// The notice for a [`PI_DAC_KEY`] other than [`PI_DAC_NONE`].
pub const PI_DAC_NOTICE: &str = "GPIO DACs are not supported by this image; the value is ignored";

/// What the start found at the place of the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileRead {
    /// No file: the boot partition is not mounted, or the user removed it.
    Missing,
    /// The file is there but cannot be read.
    Unreadable {
        /// The error from the system.
        message: String,
    },
    /// The file is larger than [`MAX_FILE_BYTES`], so it is not read.
    TooLarge {
        /// Its size in bytes.
        bytes: u64,
    },
    /// The content of the file.
    Read(Vec<u8>),
}

/// What the start does with the file.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Decision {
    /// The values to copy into `config/global.toml`, in key order.
    pub changes: Vec<(&'static str, Value)>,
    /// The hash to store once the values are written. `None` keeps the
    /// stored hash: a file that cannot be used is tried again at the next
    /// start.
    pub hash: Option<String>,
    /// The problems for the player and the log, each with its line.
    pub problems: Vec<String>,
    /// Lines for the log only.
    pub log: Vec<String>,
}

/// Decides the import of `file`. `stored` is the hash of the file imported
/// last, as 64 hex digits, or `None` when no file was imported.
///
/// A file with the stored hash changes nothing. Otherwise every value the
/// schema accepts at the global level is copied; an unknown key and a value
/// the schema refuses are ignored and listed with their line. A file that
/// cannot be read or parsed changes nothing and keeps the stored hash.
pub fn decide(schema: &Schema, file: &FileRead, stored: Option<&str>) -> Decision {
    let refuse = |problem: String| Decision {
        problems: vec![problem],
        ..Decision::default()
    };
    let bytes = match file {
        FileRead::Missing => {
            return Decision {
                log: vec![format!(
                    "no {FILE_NAME} on the boot partition; the settings stand"
                )],
                ..Decision::default()
            };
        }
        FileRead::Unreadable { message } => {
            return refuse(format!(
                "{FILE_NAME} cannot be read ({message}). No setting changes."
            ));
        }
        FileRead::TooLarge { bytes } => {
            return refuse(format!(
                "{FILE_NAME} has {bytes} bytes, more than {MAX_FILE_BYTES}. No setting changes."
            ));
        }
        FileRead::Read(bytes) => bytes,
    };
    let hash = sha256::hex(bytes);
    if stored == Some(hash.as_str()) {
        return Decision {
            log: vec![format!(
                "{FILE_NAME} has not changed since its import; the settings stand"
            )],
            ..Decision::default()
        };
    }
    let Ok(text) = std::str::from_utf8(bytes) else {
        return refuse(format!(
            "{FILE_NAME} is not UTF-8 text. No setting changes."
        ));
    };
    let parsed = parse_layer(schema, Level::Global, text);
    if let Some(message) = parsed.issues.iter().find_map(|i| match &i.kind {
        IssueKind::Unparseable { message } => Some(message),
        _ => None,
    }) {
        return refuse(format!(
            "{FILE_NAME} does not parse ({message}). No setting changes."
        ));
    }
    let problems = parsed
        .issues
        .iter()
        .filter_map(|issue| problem(issue, text))
        .collect();
    let changes: Vec<(&'static str, Value)> = parsed
        .layer
        .iter()
        .map(|(key, value)| (key, value.clone()))
        .collect();
    let copied = if changes.is_empty() {
        "no setting to copy".to_owned()
    } else {
        let keys: Vec<&str> = changes.iter().map(|(key, _)| *key).collect();
        format!("copied {} into config/global.toml", keys.join(", "))
    };
    Decision {
        changes,
        hash: Some(hash),
        problems,
        log: vec![format!("{FILE_NAME} has changed: {copied}")],
    }
}

/// The text of one problem of the file, or `None` when it needs none.
fn problem(issue: &Issue, text: &str) -> Option<String> {
    let at = match issue.line {
        Some(line) => format!("{FILE_NAME}, line {line}"),
        None => FILE_NAME.to_owned(),
    };
    let key = issue.key.as_deref().unwrap_or_default();
    match &issue.kind {
        IssueKind::UnknownKey if key == PI_DAC_KEY => (pi_dac(text).as_deref()
            != Some(PI_DAC_NONE))
        .then(|| format!("{at}: {key}: {PI_DAC_NOTICE}.")),
        IssueKind::UnknownKey => Some(format!("{at}: {key} is not a setting. It is ignored.")),
        IssueKind::Invalid { reason } => {
            Some(format!("{at}: {key}: {reason}. The value is ignored."))
        }
        IssueKind::LevelNotAllowed { .. } => Some(format!(
            "{at}: {key} is not a global setting. It is ignored."
        )),
        _ => Some(format!("{FILE_NAME}, {issue}")),
    }
}

/// The value of [`PI_DAC_KEY`] when it is a string. The settings parser
/// keeps no value of a key outside the schema, so the text is read again.
/// An editor on Windows can start the file with a byte order mark; it is
/// skipped here as the settings parser skips it.
fn pi_dac(text: &str) -> Option<String> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let table: toml::Table = toml::from_str(text).ok()?;
    let (section, name) = PI_DAC_KEY.split_once('.')?;
    table.get(section)?.get(name)?.as_str().map(str::to_owned)
}

#[cfg(test)]
mod tests;
