//! System and core definitions, parsed from `data/systems` and `data/cores`.

use std::collections::BTreeMap;

use serde::Deserialize;

/// The format version of a system or core file that this crate reads.
pub const DEFINITION_FORMAT: u32 = 1;

/// One system: its folder under `share/roms/`, its aliases and its game files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemDef {
    /// Folder name under `share/roms/`. It is also the system id.
    pub folder: String,
    /// Other folder names that hold games of this system.
    pub aliases: Vec<String>,
    /// Display name.
    pub name: String,
    /// File extensions of games, lower case, with no dot.
    pub extensions: Vec<String>,
}

/// One libretro core and the systems that it runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreDef {
    /// Core id, equal to the file name of its definition.
    pub id: String,
    /// Display name.
    pub name: String,
    /// File name of the core's shared library.
    pub library: String,
    /// Ids of the systems that the core runs.
    pub systems: Vec<String>,
    /// SPDX licence expression of the core.
    pub licence: String,
    /// Default core options (key, value), sorted by key. The app passes
    /// them to the core at launch. Empty when the file has no `options`
    /// table.
    pub options: Vec<(String, String)>,
}

/// The checked set of system and core definitions.
///
/// Every folder name and alias is unique with no regard to ASCII case, and
/// every system has exactly one core.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Catalog {
    systems: Vec<SystemDef>,
    cores: Vec<CoreDef>,
}

/// One definition file: its name with no extension, and its text.
#[derive(Debug, Clone, Copy)]
pub struct Source<'a> {
    /// File name with no extension. It must equal the system folder or the core id.
    pub name: &'a str,
    /// TOML text of the file.
    pub text: &'a str,
}

/// A definition file that does not parse or does not pass the checks.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CatalogError {
    /// The TOML text does not parse into a definition.
    #[error("{source_name}: {message}")]
    Parse {
        /// File name of the definition.
        source_name: String,
        /// Parser message.
        message: String,
    },
    /// The `format` value is not one that this crate reads.
    #[error("{source_name}: format {found} is not supported, expected {expected}")]
    Format {
        /// File name of the definition.
        source_name: String,
        /// The `format` value in the file.
        found: u32,
        /// The format that this crate reads.
        expected: u32,
    },
    /// The folder or id does not equal the file name.
    #[error("{source_name}: the id `{id}` does not equal the file name")]
    NameMismatch {
        /// File name of the definition.
        source_name: String,
        /// The folder or id in the file.
        id: String,
    },
    /// A text field is empty.
    #[error("{source_name}: the field `{field}` is empty")]
    Empty {
        /// File name of the definition.
        source_name: String,
        /// Name of the empty field.
        field: &'static str,
    },
    /// A folder name, alias, extension or id has a character that is not allowed.
    #[error("{source_name}: `{value}` is not a valid {what}")]
    BadName {
        /// File name of the definition.
        source_name: String,
        /// The value that is not valid.
        value: String,
        /// What the value is: folder, extension or id.
        what: &'static str,
    },
    /// Two systems use the same folder name or alias.
    #[error("the folder name `{folder}` is used by more than one system")]
    DuplicateFolder {
        /// The folder name, as written in the later definition.
        folder: String,
    },
    /// Two cores use the same id.
    #[error("the core id `{id}` is used more than once")]
    DuplicateCore {
        /// The core id.
        id: String,
    },
    /// A core names a system that has no definition.
    #[error("the core `{core}` names the unknown system `{system}`")]
    UnknownSystem {
        /// The core id.
        core: String,
        /// The unknown system id.
        system: String,
    },
    /// A system has no core, or more than one.
    #[error("the system `{system}` has {cores} cores, expected exactly 1")]
    CoreCount {
        /// The system id.
        system: String,
        /// The number of cores that name the system.
        cores: usize,
    },
}

const BUILTIN_SYSTEMS: &[Source<'static>] = &[
    Source {
        name: "nes",
        text: include_str!("../../../data/systems/nes.toml"),
    },
    Source {
        name: "snes",
        text: include_str!("../../../data/systems/snes.toml"),
    },
    Source {
        name: "megadrive",
        text: include_str!("../../../data/systems/megadrive.toml"),
    },
];

const BUILTIN_CORES: &[Source<'static>] = &[
    Source {
        name: "fceumm",
        text: include_str!("../../../data/cores/fceumm.toml"),
    },
    Source {
        name: "snes9x",
        text: include_str!("../../../data/cores/snes9x.toml"),
    },
    Source {
        name: "genesis_plus_gx",
        text: include_str!("../../../data/cores/genesis_plus_gx.toml"),
    },
];

impl Catalog {
    /// The definitions that are built into the binary from `data/systems` and `data/cores`.
    pub fn builtin() -> Result<Self, CatalogError> {
        Self::from_sources(BUILTIN_SYSTEMS, BUILTIN_CORES)
    }

    /// Parses and checks system and core definitions. Systems keep the order of `systems`.
    pub fn from_sources(
        systems: &[Source<'_>],
        cores: &[Source<'_>],
    ) -> Result<Self, CatalogError> {
        let systems = systems
            .iter()
            .map(parse_system)
            .collect::<Result<Vec<_>, _>>()?;
        let cores = cores
            .iter()
            .map(parse_core)
            .collect::<Result<Vec<_>, _>>()?;
        check_folders(&systems)?;
        check_cores(&systems, &cores)?;
        Ok(Self { systems, cores })
    }

    /// Every system, in catalog order.
    pub fn systems(&self) -> &[SystemDef] {
        &self.systems
    }

    /// Every core.
    pub fn cores(&self) -> &[CoreDef] {
        &self.cores
    }

    /// The system with this id.
    pub fn system(&self, id: &str) -> Option<&SystemDef> {
        self.systems.iter().find(|s| s.folder == id)
    }

    /// The position in [`Catalog::systems`] of the system that owns this
    /// folder name or alias. The match ignores ASCII case, as exFAT does.
    pub fn system_for_folder(&self, folder: &str) -> Option<usize> {
        self.systems
            .iter()
            .position(|s| s.folder_names().any(|f| f.eq_ignore_ascii_case(folder)))
    }

    /// The core that runs this system.
    pub fn core_for(&self, system: &str) -> Option<&CoreDef> {
        self.cores
            .iter()
            .find(|c| c.systems.iter().any(|s| s == system))
    }
}

impl SystemDef {
    /// True when a file with this extension (no dot) is a game of the
    /// system. The match ignores ASCII case.
    pub fn accepts_extension(&self, extension: &str) -> bool {
        self.extensions
            .iter()
            .any(|e| e.eq_ignore_ascii_case(extension))
    }

    /// The folder name and every alias.
    pub fn folder_names(&self) -> impl Iterator<Item = &str> {
        std::iter::once(self.folder.as_str()).chain(self.aliases.iter().map(String::as_str))
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SystemFile {
    format: u32,
    folder: String,
    aliases: Vec<String>,
    name: String,
    extensions: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CoreFile {
    format: u32,
    id: String,
    name: String,
    library: String,
    systems: Vec<String>,
    licence: String,
    #[serde(default)]
    options: BTreeMap<String, String>,
}

fn parse_system(source: &Source<'_>) -> Result<SystemDef, CatalogError> {
    let file: SystemFile = parse_toml(source)?;
    check_format(source, file.format)?;
    check_id(source, &file.folder)?;
    check_not_empty(source, "name", &file.name)?;
    for alias in &file.aliases {
        check_token(source, alias, "folder")?;
    }
    if file.extensions.is_empty() {
        return Err(empty(source, "extensions"));
    }
    for ext in &file.extensions {
        if ext.is_empty()
            || !ext
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        {
            return Err(bad_name(source, ext, "extension"));
        }
    }
    Ok(SystemDef {
        folder: file.folder,
        aliases: file.aliases,
        name: file.name,
        extensions: file.extensions,
    })
}

fn parse_core(source: &Source<'_>) -> Result<CoreDef, CatalogError> {
    let file: CoreFile = parse_toml(source)?;
    check_format(source, file.format)?;
    check_id(source, &file.id)?;
    check_not_empty(source, "name", &file.name)?;
    check_not_empty(source, "library", &file.library)?;
    check_not_empty(source, "licence", &file.licence)?;
    if file.library.contains('/') {
        return Err(bad_name(source, &file.library, "library file name"));
    }
    if file.systems.is_empty() {
        return Err(empty(source, "systems"));
    }
    for (key, value) in &file.options {
        check_option_key(source, key)?;
        if value.is_empty() || value.contains(['|', ';', '\0']) {
            return Err(bad_name(source, value, "core option value"));
        }
    }
    Ok(CoreDef {
        id: file.id,
        name: file.name,
        library: file.library,
        systems: file.systems,
        licence: file.licence,
        options: file.options.into_iter().collect(),
    })
}

fn parse_toml<T: serde::de::DeserializeOwned>(source: &Source<'_>) -> Result<T, CatalogError> {
    toml::from_str(source.text).map_err(|e| CatalogError::Parse {
        source_name: source.name.to_owned(),
        message: e.to_string(),
    })
}

fn check_format(source: &Source<'_>, found: u32) -> Result<(), CatalogError> {
    if found == DEFINITION_FORMAT {
        Ok(())
    } else {
        Err(CatalogError::Format {
            source_name: source.name.to_owned(),
            found,
            expected: DEFINITION_FORMAT,
        })
    }
}

fn check_id(source: &Source<'_>, id: &str) -> Result<(), CatalogError> {
    check_token(source, id, "id")?;
    if id == source.name {
        Ok(())
    } else {
        Err(CatalogError::NameMismatch {
            source_name: source.name.to_owned(),
            id: id.to_owned(),
        })
    }
}

/// A folder name or id: lower-case ASCII letters, digits, `-` and `_`,
/// starting with a letter or digit. This keeps ids safe in paths and in
/// the cache, and keeps folder names apart from hidden folders.
fn check_token(source: &Source<'_>, value: &str, what: &'static str) -> Result<(), CatalogError> {
    let mut bytes = value.bytes();
    let first_ok = bytes
        .next()
        .is_some_and(|b| b.is_ascii_lowercase() || b.is_ascii_digit());
    let rest_ok =
        bytes.all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_');
    if first_ok && rest_ok {
        Ok(())
    } else {
        Err(bad_name(source, value, what))
    }
}

/// A core option key: ASCII letters, digits and `_`, as libretro cores
/// name them. libretro joins a key and its value with `=` and `;`, so
/// neither may appear.
fn check_option_key(source: &Source<'_>, key: &str) -> Result<(), CatalogError> {
    let ok = !key.is_empty() && key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_');
    if ok {
        Ok(())
    } else {
        Err(bad_name(source, key, "core option key"))
    }
}

fn check_not_empty(
    source: &Source<'_>,
    field: &'static str,
    value: &str,
) -> Result<(), CatalogError> {
    if value.trim().is_empty() {
        Err(empty(source, field))
    } else {
        Ok(())
    }
}

fn empty(source: &Source<'_>, field: &'static str) -> CatalogError {
    CatalogError::Empty {
        source_name: source.name.to_owned(),
        field,
    }
}

fn bad_name(source: &Source<'_>, value: &str, what: &'static str) -> CatalogError {
    CatalogError::BadName {
        source_name: source.name.to_owned(),
        value: value.to_owned(),
        what,
    }
}

fn check_folders(systems: &[SystemDef]) -> Result<(), CatalogError> {
    let mut seen: Vec<&str> = Vec::new();
    for folder in systems.iter().flat_map(SystemDef::folder_names) {
        if seen.iter().any(|s| s.eq_ignore_ascii_case(folder)) {
            return Err(CatalogError::DuplicateFolder {
                folder: folder.to_owned(),
            });
        }
        seen.push(folder);
    }
    Ok(())
}

fn check_cores(systems: &[SystemDef], cores: &[CoreDef]) -> Result<(), CatalogError> {
    for (i, core) in cores.iter().enumerate() {
        if cores[..i].iter().any(|c| c.id == core.id) {
            return Err(CatalogError::DuplicateCore {
                id: core.id.clone(),
            });
        }
        if let Some(system) = core
            .systems
            .iter()
            .find(|s| !systems.iter().any(|d| &d.folder == *s))
        {
            return Err(CatalogError::UnknownSystem {
                core: core.id.clone(),
                system: system.clone(),
            });
        }
    }
    for system in systems {
        let count = cores
            .iter()
            .filter(|c| c.systems.contains(&system.folder))
            .count();
        if count != 1 {
            return Err(CatalogError::CoreCount {
                system: system.folder.clone(),
                cores: count,
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
