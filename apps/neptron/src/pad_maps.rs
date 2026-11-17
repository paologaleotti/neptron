//! The maps of the player in `config/input/pads.toml`: read at the start
//! of a run, and written whole by the writer thread when the player keeps
//! or resets a map.
//!
//! The format is append-only. A write starts from the file as the run
//! read it and changes only the one `[[pad]]` table of the identity, so a
//! table or a key of a newer build survives. An entry with a key `system`
//! or `game` is a map for one system or one game: this build skips it and
//! keeps it.

use neptron_input::{Binding, CONTROL_KEYS, DeviceIdentity, PlayerMap, PlayerMaps};
use toml::{Table, Value};

/// The version of the layout of the file that this build writes.
pub const PAD_MAPS_VERSION: i64 = 1;

/// The key of the version.
const VERSION_KEY: &str = "version";

/// The key of the array of the maps.
const PAD_KEY: &str = "pad";

/// The keys that make an entry a map for one system or one game.
const SCOPE_KEYS: [&str; 2] = ["system", "game"];

/// The maps of the player and the file they came from.
#[derive(Debug, Clone)]
pub struct PadMapsFile {
    maps: PlayerMaps,
    /// The file as read; `None` when it cannot be written: it did not
    /// parse, or a newer build wrote it.
    table: Option<Table>,
    /// Why the file cannot be written, for the player.
    locked: Option<String>,
    /// Makes the text of the file. A test gives one that fails.
    render: Render,
}

/// What makes the text of the file from its table.
type Render = fn(&Table) -> Result<String, String>;

impl Default for PadMapsFile {
    fn default() -> Self {
        Self {
            maps: PlayerMaps::new(),
            table: None,
            locked: None,
            render: render_toml,
        }
    }
}

/// Why a map cannot be stored.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PadMapsError {
    /// The file of the maps is damaged or newer, and a write could lose
    /// it.
    #[error("{0}")]
    Locked(String),
    /// The text of the file cannot be made. The file is not written.
    #[error("the file of the controller layouts cannot be made: {0}")]
    Render(String),
}

impl PadMapsFile {
    /// Reads `text`, the content of the file, or `None` for no file.
    /// Returns the maps and the problems, one line each. A file that does
    /// not parse gives no map and cannot be written.
    pub fn parse(text: Option<&str>) -> (Self, Vec<String>) {
        let mut issues = Vec::new();
        let Some(text) = text else {
            return (
                Self {
                    table: Some(Table::new()),
                    ..Self::default()
                },
                issues,
            );
        };
        let table: Table = match toml::from_str(text) {
            Ok(table) => table,
            Err(e) => {
                let reason = format!("the file of the controller layouts cannot be read: {e}");
                issues.push(reason.clone());
                return (
                    Self {
                        locked: Some(reason),
                        ..Self::default()
                    },
                    issues,
                );
            }
        };
        let version = match table.get(VERSION_KEY) {
            None => Some(1),
            Some(Value::Integer(v)) => Some(*v),
            Some(_) => None,
        };
        let locked = match version {
            Some(v) if v <= PAD_MAPS_VERSION => None,
            Some(v) => Some(format!(
                "the file of the controller layouts has version {v}, newer than {PAD_MAPS_VERSION}"
            )),
            None => {
                Some("the version of the file of the controller layouts is not a number".to_owned())
            }
        };
        issues.extend(locked.clone());
        let mut maps = PlayerMaps::new();
        let entries = table.get(PAD_KEY).and_then(Value::as_array);
        for (n, entry) in entries.into_iter().flatten().enumerate() {
            let Some(entry) = entry.as_table() else {
                issues.push(format!("controller layout {}: not a table", n + 1));
                continue;
            };
            if SCOPE_KEYS.iter().any(|k| entry.contains_key(*k)) {
                continue;
            }
            match read_entry(entry, n + 1, &mut issues) {
                Some((identity, map)) => maps.insert(identity, map),
                None => continue,
            }
        }
        let table = locked.is_none().then_some(table);
        (
            Self {
                maps,
                table,
                locked,
                ..Self::default()
            },
            issues,
        )
    }

    /// The maps of the player.
    pub fn maps(&self) -> &PlayerMaps {
        &self.maps
    }

    /// Why the file cannot be written, or `None` when it can.
    pub fn locked(&self) -> Option<&str> {
        self.locked.as_deref()
    }

    /// Reads the file at `path`. A missing file is no map. A file that
    /// cannot be read gives no map and cannot be written.
    pub fn load(path: &std::path::Path) -> (Self, Vec<String>) {
        match std::fs::read_to_string(path) {
            Ok(text) => Self::parse(Some(&text)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Self::parse(None),
            Err(e) => {
                let reason = format!("cannot read {}: {e}", path.display());
                (
                    Self {
                        locked: Some(reason.clone()),
                        ..Self::default()
                    },
                    vec![reason],
                )
            }
        }
    }

    /// Sets the map of `identity` and returns the text of the new file.
    ///
    /// # Errors
    ///
    /// [`PadMapsError::Locked`] when the file cannot be written, and
    /// [`PadMapsError::Render`] when its text cannot be made. Nothing
    /// changes then.
    pub fn store(
        &mut self,
        identity: DeviceIdentity,
        name: &str,
        map: &PlayerMap,
    ) -> Result<String, PadMapsError> {
        let mut table = self.writable()?.clone();
        let mut entries = take_entries(&mut table);
        let at = entries.iter().position(|e| is_entry_of(e, identity));
        let mut entry = at
            .and_then(|i| entries.get(i))
            .and_then(Value::as_table)
            .cloned()
            .unwrap_or_default();
        for (_, key) in CONTROL_KEYS {
            entry.remove(key);
        }
        entry.insert("identity".to_owned(), Value::String(hex(identity)));
        entry.insert("name".to_owned(), Value::String(name.to_owned()));
        for (control, binding) in map.bindings() {
            entry.insert(
                neptron_input::control_key(control).to_owned(),
                Value::String(binding.to_string()),
            );
        }
        match at.and_then(|i| entries.get_mut(i)) {
            Some(slot) => *slot = Value::Table(entry),
            None => entries.push(Value::Table(entry)),
        }
        put_entries(&mut table, entries);
        let text = self.commit(table)?;
        self.maps.insert(identity, *map);
        Ok(text)
    }

    /// Removes the map of `identity` and returns the text of the new
    /// file.
    ///
    /// # Errors
    ///
    /// [`PadMapsError::Locked`] when the file cannot be written, and
    /// [`PadMapsError::Render`] when its text cannot be made. Nothing
    /// changes then.
    pub fn remove(&mut self, identity: DeviceIdentity) -> Result<String, PadMapsError> {
        let mut table = self.writable()?.clone();
        let mut entries = take_entries(&mut table);
        entries.retain(|e| !is_entry_of(e, identity));
        put_entries(&mut table, entries);
        let text = self.commit(table)?;
        self.maps.remove(identity);
        Ok(text)
    }

    fn writable(&self) -> Result<&Table, PadMapsError> {
        let locked = self.locked.clone();
        self.table.as_ref().ok_or_else(|| {
            PadMapsError::Locked(locked.unwrap_or_else(|| {
                "the file of the controller layouts cannot be written".to_owned()
            }))
        })
    }

    /// Makes the text of `table` and, when it can, keeps `table` as the
    /// file. A text that cannot be made changes nothing, so no write ever
    /// replaces the file with an empty one.
    fn commit(&mut self, mut table: Table) -> Result<String, PadMapsError> {
        table.insert(VERSION_KEY.to_owned(), Value::Integer(PAD_MAPS_VERSION));
        let text = (self.render)(&table).map_err(PadMapsError::Render)?;
        self.table = Some(table);
        Ok(text)
    }
}

/// The TOML text of `table`.
fn render_toml(table: &Table) -> Result<String, String> {
    toml::to_string(table).map_err(|e| e.to_string())
}

/// Takes the array of the maps out of `table`: empty when it is missing
/// or is no array. [`put_entries`] puts it back.
fn take_entries(table: &mut Table) -> Vec<Value> {
    match table.remove(PAD_KEY) {
        Some(Value::Array(entries)) => entries,
        _ => Vec::new(),
    }
}

fn put_entries(table: &mut Table, entries: Vec<Value>) {
    table.insert(PAD_KEY.to_owned(), Value::Array(entries));
}

/// True for the entry of all games of `identity`.
fn is_entry_of(entry: &Value, identity: DeviceIdentity) -> bool {
    let Some(entry) = entry.as_table() else {
        return false;
    };
    !SCOPE_KEYS.iter().any(|k| entry.contains_key(*k))
        && entry.get("identity").and_then(Value::as_str) == Some(hex(identity).as_str())
}

/// The identity and the map of one entry. A damaged binding loses only
/// itself; an entry with no valid identity is skipped.
fn read_entry(
    entry: &Table,
    n: usize,
    issues: &mut Vec<String>,
) -> Option<(DeviceIdentity, PlayerMap)> {
    let identity = entry
        .get("identity")
        .and_then(Value::as_str)
        .filter(|text| text.len() == 16)
        .and_then(|text| u64::from_str_radix(text, 16).ok());
    let Some(identity) = identity else {
        issues.push(format!(
            "controller layout {n}: no identity of 16 hex digits"
        ));
        return None;
    };
    let mut map = PlayerMap::new();
    for (control, key) in CONTROL_KEYS {
        let Some(value) = entry.get(key) else {
            continue;
        };
        let result = value
            .as_str()
            .ok_or_else(|| "not a text".to_owned())
            .and_then(|text| Binding::parse(text).map_err(|e| e.to_string()))
            .and_then(|binding| map.set(control, Some(binding)).map_err(|e| e.to_string()));
        if let Err(reason) = result {
            issues.push(format!("controller layout {n}, {key}: {reason}"));
        }
    }
    Some((DeviceIdentity(identity), map))
}

/// An identity as 16 hex digits: TOML has no unsigned 64-bit integer.
fn hex(identity: DeviceIdentity) -> String {
    format!("{:016x}", identity.0)
}

#[cfg(test)]
mod tests {
    use neptron_input::HatDirection;
    use neptron_types::Control;

    use super::*;

    fn sample() -> PlayerMap {
        let mut map = PlayerMap::new();
        let hat = Binding::Hat {
            index: 0,
            direction: HatDirection::Up,
        };
        map.set(Control::Up, Some(hat)).unwrap();
        map.set(Control::A, Some(Binding::Button(2))).unwrap();
        let trigger = Binding::HalfAxis {
            index: 4,
            positive: true,
            full: true,
        };
        map.set(Control::L2, Some(trigger)).unwrap();
        let stick = Binding::Stick {
            index: 1,
            inverted: true,
        };
        map.set(Control::LeftY, Some(stick)).unwrap();
        map
    }

    const ID: DeviceIdentity = DeviceIdentity(0x8f3a_2b1c_0d4e_5f60);

    #[test]
    fn a_stored_map_reads_back_from_its_file() {
        let (mut file, issues) = PadMapsFile::parse(None);
        assert!(issues.is_empty());
        let text = file.store(ID, "USB Joystick", &sample()).unwrap();
        assert!(text.contains("version = 1"), "{text}");
        assert!(text.contains("identity = \"8f3a2b1c0d4e5f60\""), "{text}");
        assert!(text.contains("l2 = \"a4+full\""), "{text}");
        assert!(text.contains("left_y = \"-a1\""), "{text}");
        let (again, issues) = PadMapsFile::parse(Some(&text));
        assert!(issues.is_empty(), "{issues:?}");
        assert_eq!(again.maps().get(ID), Some(&sample()));
        assert_eq!(again.maps(), file.maps());
    }

    #[test]
    fn a_store_replaces_only_its_entry_and_keeps_what_it_does_not_know() {
        let text = r#"
version = 1
future = "kept"

[[pad]]
identity = "8f3a2b1c0d4e5f60"
name = "Old"
a = "b9"
turbo = "b5"

[[pad]]
identity = "8f3a2b1c0d4e5f60"
system = "snes"
a = "b1"

[[pad]]
identity = "0000000000000001"
b = "b0"
"#;
        let (mut file, issues) = PadMapsFile::parse(Some(text));
        assert!(issues.is_empty(), "{issues:?}");
        assert_eq!(
            file.maps().get(ID).and_then(|m| m.get(Control::A)),
            Some(Binding::Button(9)),
            "the entry of a system is skipped"
        );
        let written = file.store(ID, "USB Joystick", &sample()).unwrap();
        let (again, _) = PadMapsFile::parse(Some(&written));
        assert_eq!(again.maps().get(ID), Some(&sample()));
        assert!(again.maps().get(DeviceIdentity(1)).is_some());
        let table: Table = toml::from_str(&written).unwrap();
        assert_eq!(table.get("future").and_then(Value::as_str), Some("kept"));
        let entries = table.get("pad").and_then(Value::as_array).unwrap();
        assert_eq!(entries.len(), 3);
        let first = entries[0].as_table().unwrap();
        assert_eq!(first.get("turbo").and_then(Value::as_str), Some("b5"));
        assert_eq!(first.get("a").and_then(Value::as_str), Some("b2"));
        assert_eq!(
            first.get("name").and_then(Value::as_str),
            Some("USB Joystick")
        );
        let system = entries[1].as_table().unwrap();
        assert_eq!(system.get("a").and_then(Value::as_str), Some("b1"));

        let removed = file.remove(ID).unwrap();
        let (gone, _) = PadMapsFile::parse(Some(&removed));
        assert_eq!(gone.maps().get(ID), None);
        let table: Table = toml::from_str(&removed).unwrap();
        let entries = table.get("pad").and_then(Value::as_array).unwrap();
        assert_eq!(entries.len(), 2, "the entry of the system stays");
    }

    #[test]
    fn a_damaged_binding_loses_only_itself_and_a_bad_identity_its_entry() {
        let text = r#"
[[pad]]
identity = "8f3a2b1c0d4e5f60"
a = "b2"
b = "b99"
x = 3
left_x = "b1"

[[pad]]
identity = "nothex"
a = "b1"
"#;
        let (file, issues) = PadMapsFile::parse(Some(text));
        let map = file.maps().get(ID).unwrap();
        assert_eq!(map.get(Control::A), Some(Binding::Button(2)));
        assert_eq!(map.get(Control::B), None);
        assert_eq!(map.get(Control::LeftX), None);
        assert_eq!(file.maps().iter().count(), 1);
        assert_eq!(issues.len(), 4, "{issues:?}");
        assert!(issues.iter().any(|i| i.contains("button 99")), "{issues:?}");
    }

    fn fails(_: &Table) -> Result<String, String> {
        Err("no text".to_owned())
    }

    #[test]
    fn a_file_that_cannot_be_rendered_is_not_written_and_nothing_changes() {
        let (mut file, _) = PadMapsFile::parse(Some(
            "[[pad]]\nidentity = \"8f3a2b1c0d4e5f60\"\na = \"b9\"\n",
        ));
        file.render = fails;
        let before = file.maps().clone();
        assert!(matches!(
            file.store(ID, "x", &sample()),
            Err(PadMapsError::Render(_))
        ));
        assert!(matches!(file.remove(ID), Err(PadMapsError::Render(_))));
        assert_eq!(file.maps(), &before, "the maps did not change");
        file.render = render_toml;
        let text = file.remove(DeviceIdentity(1)).unwrap();
        assert!(
            text.contains("a = \"b9\""),
            "the table did not change: {text}"
        );
    }

    #[test]
    fn a_damaged_or_newer_file_is_never_written() {
        let (mut file, issues) = PadMapsFile::parse(Some("[[pad]\nbroken"));
        assert_eq!(issues.len(), 1);
        assert!(file.maps().iter().next().is_none());
        assert!(matches!(
            file.store(ID, "x", &sample()),
            Err(PadMapsError::Locked(_))
        ));
        assert!(file.remove(ID).is_err());

        let newer = "version = 2\n[[pad]]\nidentity = \"8f3a2b1c0d4e5f60\"\na = \"b2\"\n";
        let (mut file, issues) = PadMapsFile::parse(Some(newer));
        assert_eq!(issues.len(), 1, "{issues:?}");
        assert!(file.maps().get(ID).is_some(), "read as far as known");
        assert!(file.store(ID, "x", &sample()).is_err());
        assert_eq!(
            file.maps().get(ID).and_then(|m| m.get(Control::Up)),
            None,
            "a refused store changes nothing"
        );
    }
}
