//! The file of the guard record: the last known good of each place, the
//! failed boots, safe mode and the run marker, kept apart from the user
//! settings on the state partition.

use std::io;
use std::path::Path;

use neptron_video::{GuardRecord, Monitor};
use serde::{Deserialize, Serialize};

use crate::atomic;

/// The format version of the record file. The format is append-only: a
/// new field is optional, and a reader ignores fields it does not know.
pub const RECORD_VERSION: u32 = 1;

/// The places the record keeps a last known good for, at most. A new place
/// beyond them drops the place used longest ago.
pub const PLACES_MAX: usize = 8;

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

/// A record as the file holds it: the counts of the guard, and the last
/// known good of each place as references, which only the boot at that
/// place builds again.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Stored {
    /// The guard's record, with no last known good.
    pub record: GuardRecord,
    /// The last known good of each place, the place used last at the end.
    pub places: Vec<PlaceEntry>,
}

/// Where a last known good was confirmed: the connector and the physical
/// monitor on it. A preset says nothing about the monitor, so only this
/// tells a swapped monitor on the same connector.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Place {
    /// The connector name, such as `VGA-1`.
    pub output: String,
    /// The identity of the monitor: a hash of its EDID, or `none`.
    pub monitor: String,
}

/// The last known good of one place.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaceEntry {
    /// The connector name.
    pub output: String,
    /// The identity of the monitor on it.
    pub monitor: String,
    /// The monitor the user kept there.
    pub last_known_good: MonitorRef,
}

#[derive(Debug, Serialize, Deserialize)]
struct RecordFile {
    version: u32,
    failed_boots: u32,
    safe_mode: bool,
    /// A file with no `run_open` has no run marker set; a build that does
    /// not know the field ignores it.
    #[serde(default)]
    run_open: bool,
    /// The last known good of the place used last, with `output` and
    /// `monitor`, so a build that does not know `places` finds it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    last_known_good: Option<MonitorRef>,
    /// The connector of the place used last; a file with none and no
    /// `places` has no place.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    output: Option<String>,
    /// The monitor identity of the place used last; a file with none and
    /// no `places` has no place.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    monitor: Option<String>,
    /// The last known good of each place. A file with none has one place
    /// from the three fields above, or none; a file with it ignores them,
    /// and a build that does not know the field ignores it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    places: Option<Vec<PlaceEntry>>,
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

impl PlaceEntry {
    fn is_at(&self, place: &Place) -> bool {
        self.output == place.output && self.monitor == place.monitor
    }
}

/// The text of the record file for the counts of `record` and `places`.
/// The last known good of `record` is not written: [`merge`] puts it in
/// `places` first.
pub fn encode(record: &GuardRecord, places: &[PlaceEntry]) -> String {
    let last = places.last();
    let file = RecordFile {
        version: RECORD_VERSION,
        failed_boots: record.failed_boots,
        safe_mode: record.safe_mode,
        run_open: record.run_open,
        last_known_good: last.map(|p| p.last_known_good.clone()),
        output: last.map(|p| p.output.clone()),
        monitor: last.map(|p| p.monitor.clone()),
        places: (!places.is_empty()).then(|| places.to_vec()),
    };
    // A struct of integers, booleans and strings always serialises.
    toml::to_string(&file).unwrap_or_default()
}

/// Reads the record from `text`. A file with no `places` gives one place
/// from its top fields when all three are there.
///
/// # Errors
///
/// [`RecordError::Parse`] and [`RecordError::Version`].
pub fn decode(text: &str) -> Result<Stored, RecordError> {
    let file: RecordFile = toml::from_str(text).map_err(|e| RecordError::Parse(e.to_string()))?;
    if file.version != RECORD_VERSION {
        return Err(RecordError::Version {
            found: file.version,
        });
    }
    let places = match file.places {
        Some(places) => places,
        None => match (file.output, file.monitor, file.last_known_good) {
            (Some(output), Some(monitor), Some(last_known_good)) => vec![PlaceEntry {
                output,
                monitor,
                last_known_good,
            }],
            _ => Vec::new(),
        },
    };
    Ok(Stored {
        record: GuardRecord {
            last_known_good: None,
            failed_boots: file.failed_boots,
            safe_mode: file.safe_mode,
            run_open: file.run_open,
        },
        places,
    })
}

/// Reads the record file at `path`. A missing file is the default record
/// with no error.
///
/// # Errors
///
/// [`RecordError`]. The caller uses the default record then.
pub fn load(path: &Path) -> Result<Stored, RecordError> {
    match std::fs::read_to_string(path) {
        Ok(text) => decode(&text),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Stored::default()),
        Err(e) => Err(RecordError::Read(e)),
    }
}

/// Writes the record to `path` with temp file, fsync and rename.
///
/// # Errors
///
/// The error of the write.
pub fn save(path: &Path, record: &GuardRecord, places: &[PlaceEntry]) -> io::Result<()> {
    atomic::write(path, encode(record, places).as_bytes())
}

/// The record for a start at `place`: the counts, and the last known good
/// of this place only, built again by `rebuild`. The guard compares
/// monitor profiles only, so a last known good of another connector or
/// another monitor would drive the new display with the old profile and no
/// prompt.
pub fn for_place(
    stored: &Stored,
    place: &Place,
    rebuild: impl Fn(&MonitorRef) -> Option<Monitor>,
) -> GuardRecord {
    GuardRecord {
        last_known_good: stored
            .places
            .iter()
            .find(|p| p.is_at(place))
            .and_then(|p| rebuild(&p.last_known_good)),
        ..stored.record.clone()
    }
}

/// `places` with `last_known_good` as the entry of `place`, moved to the
/// end, at most [`PLACES_MAX`] entries. With no last known good the places
/// stay as they are: a record never deletes the entry of a place, also
/// one whose monitor this start cannot build again.
pub fn merge(
    places: &[PlaceEntry],
    place: &Place,
    last_known_good: Option<&Monitor>,
) -> Vec<PlaceEntry> {
    let mut merged = places.to_vec();
    if let Some(monitor) = last_known_good {
        merged.retain(|p| !p.is_at(place));
        merged.push(PlaceEntry {
            output: place.output.clone(),
            monitor: place.monitor.clone(),
            last_known_good: MonitorRef::of(monitor),
        });
        let extra = merged.len().saturating_sub(PLACES_MAX);
        merged.drain(..extra);
    }
    merged
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

    fn place(output: &str, monitor: &str) -> Place {
        Place {
            output: output.to_owned(),
            monitor: monitor.to_owned(),
        }
    }

    fn rebuild(r: &MonitorRef) -> Option<Monitor> {
        match r {
            MonitorRef::Profile { id } if id != "gone" => Some(profile(id)),
            _ => None,
        }
    }

    fn counts() -> GuardRecord {
        GuardRecord {
            last_known_good: None,
            failed_boots: 1,
            safe_mode: true,
            run_open: true,
        }
    }

    #[test]
    fn a_record_survives_a_round_trip() {
        let vga = place("VGA-1", "none");
        let places = merge(&[], &vga, Some(&profile("pc_31_120")));
        let text = encode(&counts(), &places);
        assert!(text.contains("kind = \"profile\""), "{text}");
        let stored = decode(&text).expect("decode");
        assert_eq!(stored.record, counts());
        assert_eq!(stored.places, places);
        assert_eq!(
            for_place(&stored, &vga, rebuild).last_known_good,
            Some(profile("pc_31_120"))
        );
    }

    #[test]
    fn a_monitor_that_cannot_be_built_again_is_no_last_known_good_but_stays() {
        let here = place("VGA-1", "none");
        let places = merge(&[], &here, Some(&profile("gone")));
        let stored = decode(&encode(&counts(), &places)).expect("decode");
        assert_eq!(for_place(&stored, &here, rebuild).last_known_good, None);
        assert_eq!(merge(&stored.places, &here, None), places, "it stays");
    }

    #[test]
    fn a_bad_record_is_an_error() {
        assert!(matches!(decode("not toml ["), Err(RecordError::Parse(_))));
        assert!(matches!(
            decode("version = 2\nfailed_boots = 0\nsafe_mode = false\n"),
            Err(RecordError::Version { found: 2 })
        ));
        assert!(matches!(
            decode("version = 1\n"),
            Err(RecordError::Parse(_))
        ));
    }

    #[test]
    fn a_later_field_is_ignored() {
        let text = "version = 1\nfailed_boots = 2\nsafe_mode = false\nnew_field = 3\n";
        let stored = decode(text).expect("decode");
        assert_eq!(stored.record.failed_boots, 2);
        assert!(!stored.record.run_open, "a file before the run marker");
        assert_eq!(stored.places, []);
    }

    #[test]
    fn a_missing_file_is_the_default_record() {
        let path = Path::new("/nonexistent/neptron/guard.toml");
        assert_eq!(load(path).expect("load"), Stored::default());
    }

    #[test]
    fn a_file_of_one_place_reads_as_that_place() {
        let text = "version = 1\nfailed_boots = 0\nsafe_mode = false\noutput = \"Composite-1\"\nmonitor = \"none\"\n\n[last_known_good]\nkind = \"profile\"\nid = \"tv_ntsc\"\n";
        let stored = decode(text).expect("decode");
        assert_eq!(
            stored.places,
            [PlaceEntry {
                output: "Composite-1".to_owned(),
                monitor: "none".to_owned(),
                last_known_good: MonitorRef::Profile {
                    id: "tv_ntsc".to_owned()
                },
            }]
        );
        let old = "version = 1\nfailed_boots = 0\nsafe_mode = false\noutput = \"VGA-1\"\n\n[last_known_good]\nkind = \"profile\"\nid = \"pc_31_120\"\n";
        assert_eq!(
            decode(old).expect("decode").places,
            [],
            "a file from before the monitor field has no place"
        );
    }

    #[test]
    fn a_build_before_places_finds_the_place_used_last() {
        let first = merge(&[], &place("HDMI-A-1", "edid-1"), Some(&profile("a")));
        let both = merge(&first, &place("Composite-1", "none"), Some(&profile("b")));
        let text = encode(&counts(), &both);
        let top: toml::Table = toml::from_str(&text).expect("toml");
        assert_eq!(top["output"].as_str(), Some("Composite-1"));
        assert_eq!(top["monitor"].as_str(), Some("none"));
        assert_eq!(top["last_known_good"]["id"].as_str(), Some("b"));
        assert_eq!(top["version"].as_integer(), Some(1));
    }

    #[test]
    fn a_boot_at_another_place_keeps_the_confirmation_of_the_first() {
        let hdmi = place("HDMI-A-1", "edid-1");
        let composite = place("Composite-1", "none");
        let kept = merge(&[], &hdmi, Some(&profile("pc_31_120")));
        // A boot on composite with nothing kept there writes its counts.
        let stored = decode(&encode(&counts(), &kept)).expect("decode");
        assert_eq!(
            for_place(&stored, &composite, rebuild).last_known_good,
            None
        );
        let after = merge(&stored.places, &composite, None);
        let stored = decode(&encode(&counts(), &after)).expect("decode");
        assert_eq!(
            for_place(&stored, &hdmi, rebuild).last_known_good,
            Some(profile("pc_31_120")),
            "the HDMI confirmation stays"
        );
        // A confirmation on composite adds a place.
        let both = merge(&stored.places, &composite, Some(&profile("tv_ntsc")));
        let stored = decode(&encode(&counts(), &both)).expect("decode");
        assert_eq!(stored.places.len(), 2);
        assert!(for_place(&stored, &hdmi, rebuild).last_known_good.is_some());
        assert!(
            for_place(&stored, &composite, rebuild)
                .last_known_good
                .is_some()
        );
    }

    #[test]
    fn a_last_known_good_of_another_output_or_monitor_is_not_used() {
        let vga = place("VGA-1", "edid-1");
        let stored = Stored {
            record: counts(),
            places: merge(&[], &vga, Some(&profile("pc_31_120"))),
        };
        assert!(for_place(&stored, &vga, rebuild).last_known_good.is_some());
        let other = for_place(&stored, &place("DP-1", "edid-1"), rebuild);
        assert_eq!(other.last_known_good, None);
        assert_eq!(other.failed_boots, 1, "the counts stay");
        for (what, here) in [
            ("another monitor", place("VGA-1", "edid-2")),
            ("no EDID now", place("VGA-1", "none")),
        ] {
            assert_eq!(
                for_place(&stored, &here, rebuild).last_known_good,
                None,
                "{what}"
            );
        }
    }

    #[test]
    fn the_place_used_longest_ago_goes_beyond_the_limit() {
        let mut places = Vec::new();
        for i in 0..=PLACES_MAX {
            places = merge(
                &places,
                &place(&format!("out-{i}"), "none"),
                Some(&profile("p")),
            );
        }
        assert_eq!(places.len(), PLACES_MAX);
        assert_eq!(places[0].output, "out-1");
        assert_eq!(places[PLACES_MAX - 1].output, format!("out-{PLACES_MAX}"));
        // A place used again moves to the end.
        let again = merge(&places, &place("out-1", "none"), Some(&profile("q")));
        assert_eq!(again.len(), PLACES_MAX);
        assert_eq!(again.last().map(|p| p.output.as_str()), Some("out-1"));
    }
}
