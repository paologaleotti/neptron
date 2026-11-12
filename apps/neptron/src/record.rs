//! The file of the guard record: last known good, failed boots and safe
//! mode, kept apart from the user settings on the state partition.

use std::io;
use std::path::Path;

use neptron_video::{GuardRecord, Monitor};
use serde::{Deserialize, Serialize};

use crate::atomic;

/// The format version of the record file. The format is append-only: a
/// new field is optional, and a reader ignores fields it does not know.
pub const RECORD_VERSION: u32 = 1;

/// A monitor as the record stores it: the kind and the profile id. The
/// profile itself is built again at boot, from the preset files or from
/// the EDID of the connected monitor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MonitorRef {
    /// A preset of the monitor folder.
    Profile {
        /// The profile id, the file name of the preset.
        id: String,
    },
    /// A profile built from the EDID of a monitor.
    Edid {
        /// The profile id: `edid_<manufacturer>_<product>` for a CRT,
        /// `panel_<manufacturer>_<product>` for a flat panel.
        id: String,
    },
}

/// Why the record file was not used. The guard then gets
/// [`GuardRecord::default`].
#[derive(Debug, thiserror::Error)]
pub enum RecordError {
    /// The file cannot be read.
    #[error("cannot read the guard record: {0}")]
    Read(#[source] io::Error),
    /// The file is not a record.
    #[error("the guard record does not parse: {0}")]
    Parse(String),
    /// The file has a version this build does not read.
    #[error("the guard record has version {found}; this build reads version {RECORD_VERSION}")]
    Version {
        /// The version in the file.
        found: u32,
    },
}

/// A record as the file holds it: the guard's record and the output the
/// last known good was confirmed on.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Stored {
    /// The guard's record.
    pub record: GuardRecord,
    /// The connector of the confirmation, `None` in a file that has none.
    pub output: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct RecordFile {
    version: u32,
    failed_boots: u32,
    safe_mode: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    last_known_good: Option<MonitorRef>,
    /// Added after version 1 was written; optional, so older files read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    output: Option<String>,
}

impl MonitorRef {
    /// The reference of `monitor`.
    pub fn of(monitor: &Monitor) -> Self {
        match monitor {
            Monitor::Profile(profile) => Self::Profile {
                id: profile.id.clone(),
            },
            Monitor::Edid(edid) => Self::Edid {
                id: edid.profile.id.clone(),
            },
        }
    }
}

/// The text of the record file for `record`, confirmed on `output`.
pub fn encode(record: &GuardRecord, output: Option<&str>) -> String {
    let file = RecordFile {
        version: RECORD_VERSION,
        failed_boots: record.failed_boots,
        safe_mode: record.safe_mode,
        last_known_good: record.last_known_good.as_ref().map(MonitorRef::of),
        output: output.map(str::to_owned),
    };
    // A struct of integers, booleans and strings always serialises.
    toml::to_string(&file).unwrap_or_default()
}

/// Reads the record from `text`. `rebuild` makes the monitor of a
/// reference again; `None` from it gives a record with no last known good,
/// as the guard asks.
///
/// # Errors
///
/// [`RecordError::Parse`] and [`RecordError::Version`].
pub fn decode(
    text: &str,
    rebuild: impl Fn(&MonitorRef) -> Option<Monitor>,
) -> Result<Stored, RecordError> {
    let file: RecordFile = toml::from_str(text).map_err(|e| RecordError::Parse(e.to_string()))?;
    if file.version != RECORD_VERSION {
        return Err(RecordError::Version {
            found: file.version,
        });
    }
    Ok(Stored {
        record: GuardRecord {
            last_known_good: file.last_known_good.as_ref().and_then(rebuild),
            failed_boots: file.failed_boots,
            safe_mode: file.safe_mode,
        },
        output: file.output,
    })
}

/// Reads the record file at `path`. A missing file is the default record
/// with no error.
///
/// # Errors
///
/// [`RecordError`]. The caller uses the default record then.
pub fn load(
    path: &Path,
    rebuild: impl Fn(&MonitorRef) -> Option<Monitor>,
) -> Result<Stored, RecordError> {
    match std::fs::read_to_string(path) {
        Ok(text) => decode(&text, rebuild),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Stored::default()),
        Err(e) => Err(RecordError::Read(e)),
    }
}

/// Writes the record to `path` with temp file, fsync and rename.
///
/// # Errors
///
/// The error of the write.
pub fn save(path: &Path, record: &GuardRecord, output: Option<&str>) -> io::Result<()> {
    atomic::write(path, encode(record, output).as_bytes())
}

/// The record for a start on `output`. A last known good confirmed on
/// another connector, or on an unknown one, is dropped: the guard then
/// opens the boot prompt and reverts to the safe default for the new
/// output, because the guard compares monitors only and would otherwise
/// drive the new display with the old profile and no prompt.
pub fn for_output(stored: Stored, output: &str) -> GuardRecord {
    let mut record = stored.record;
    if stored.output.as_deref() != Some(output) {
        record.last_known_good = None;
    }
    record
}

/// The record for a run that continues the boot of the run before it: a
/// planned relaunch into a game or back to the game list. `Guard::boot`
/// counts one failed boot for each start, and the run before has already
/// counted this boot, so this gives that count back. A crash, a power
/// cut or a restart of the launcher gives no relaunch file, and then the
/// start counts.
pub fn for_continued_boot(mut record: GuardRecord) -> GuardRecord {
    record.failed_boots = record.failed_boots.saturating_sub(1);
    record
}

#[cfg(test)]
mod tests {
    use neptron_types::{MonitorKind, MonitorProfile};

    use super::*;

    fn profile(id: &str) -> Monitor {
        Monitor::Profile(MonitorProfile {
            id: id.to_owned(),
            kind: MonitorKind::Crt { ranges: Vec::new() },
        })
    }

    fn rebuild(r: &MonitorRef) -> Option<Monitor> {
        match r {
            MonitorRef::Profile { id } if id != "gone" => Some(profile(id)),
            _ => None,
        }
    }

    #[test]
    fn a_record_survives_a_round_trip() {
        let record = GuardRecord {
            last_known_good: Some(profile("pc_31_120")),
            failed_boots: 1,
            safe_mode: true,
        };
        let text = encode(&record, Some("VGA-1"));
        assert!(text.contains("kind = \"profile\""), "{text}");
        let stored = decode(&text, rebuild).expect("decode");
        assert_eq!(stored.record, record);
        assert_eq!(stored.output.as_deref(), Some("VGA-1"));
    }

    #[test]
    fn a_monitor_that_cannot_be_built_again_is_no_last_known_good() {
        let record = GuardRecord {
            last_known_good: Some(profile("gone")),
            failed_boots: 0,
            safe_mode: false,
        };
        let back = decode(&encode(&record, None), rebuild).expect("decode");
        assert_eq!(back.record.last_known_good, None);
    }

    #[test]
    fn a_bad_record_is_an_error() {
        assert!(matches!(
            decode("not toml [", rebuild),
            Err(RecordError::Parse(_))
        ));
        assert!(matches!(
            decode(
                "version = 2\nfailed_boots = 0\nsafe_mode = false\n",
                rebuild
            ),
            Err(RecordError::Version { found: 2 })
        ));
        assert!(matches!(
            decode("version = 1\n", rebuild),
            Err(RecordError::Parse(_))
        ));
    }

    #[test]
    fn a_later_field_is_ignored() {
        let text = "version = 1\nfailed_boots = 2\nsafe_mode = false\nnew_field = 3\n";
        assert_eq!(
            decode(text, rebuild).expect("decode").record.failed_boots,
            2
        );
    }

    #[test]
    fn a_missing_file_is_the_default_record() {
        let path = Path::new("/nonexistent/neptron/guard.toml");
        assert_eq!(load(path, rebuild).expect("load"), Stored::default());
    }

    #[test]
    fn a_last_known_good_of_another_output_is_dropped() {
        let stored = |output: Option<&str>| Stored {
            record: GuardRecord {
                last_known_good: Some(profile("pc_31_120")),
                failed_boots: 1,
                safe_mode: false,
            },
            output: output.map(str::to_owned),
        };
        assert!(
            for_output(stored(Some("VGA-1")), "VGA-1")
                .last_known_good
                .is_some()
        );
        let other = for_output(stored(Some("VGA-1")), "DP-1");
        assert_eq!(other.last_known_good, None);
        assert_eq!(other.failed_boots, 1, "the counts stay");
        assert_eq!(for_output(stored(None), "VGA-1").last_known_good, None);
    }

    #[test]
    fn a_continued_boot_gives_back_its_count() {
        let record = GuardRecord {
            last_known_good: None,
            failed_boots: 1,
            safe_mode: true,
        };
        let continued = for_continued_boot(record);
        assert_eq!(continued.failed_boots, 0);
        assert!(continued.safe_mode);
        assert_eq!(for_continued_boot(GuardRecord::default()).failed_boots, 0);
    }
}
