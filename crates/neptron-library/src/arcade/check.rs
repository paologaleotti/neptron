//! The check of a set before launch: are the ROMs that the core needs in
//! the zips it opens? The check is pure: the caller lists the zips, and
//! this module decides.
//!
//! The rules follow the way FinalBurn Neo loads a set: it opens every zip
//! of the set, its parent and its BIOS that it finds, and takes each ROM
//! from the first zip that holds its CRC. A ROM of the wrong size is an
//! error. So the match is by CRC and size, not by name, and one rule
//! covers split, non-merged and full non-merged sets.

use std::collections::HashSet;

use super::{ArcadeIndex, RomRecord};

/// One file of a zip, as its central directory gives it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZipEntry {
    /// The name in the zip.
    pub name: String,
    /// The uncompressed size in bytes.
    pub size: u64,
    /// The CRC-32 of the uncompressed bytes.
    pub crc: u32,
}

/// A zip that the caller found for one of the names of [`Wanted`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoundZip {
    /// The file name, for the messages, for example `neogeo.zip`.
    pub file: String,
    /// The entries, or why the zip cannot be read.
    pub entries: Result<Vec<ZipEntry>, String>,
}

/// The zips found for the set, its parent and its BIOS, each list in the
/// order of the places the core searches.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FoundZips {
    /// The zips with the name of the set.
    pub set: Vec<FoundZip>,
    /// The zips with the name of the parent.
    pub parent: Vec<FoundZip>,
    /// The zips with the name of the BIOS set.
    pub bios: Vec<FoundZip>,
}

/// The zip names that the core opens for a set, with no extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Wanted<'a> {
    /// The set.
    pub set: &'a str,
    /// Its parent, for a clone.
    pub parent: Option<&'a str>,
    /// Its BIOS set, if it has one.
    pub bios: Option<&'a str>,
}

/// A ROM that no found zip gives right.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RomProblem {
    /// The ROM name in the DAT.
    pub rom: String,
    /// What is wrong.
    pub problem: Problem,
}

/// What is wrong with a ROM.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Problem {
    /// No found zip holds its CRC.
    Missing,
    /// The first zip that holds its CRC gives another size.
    Size {
        /// The size in the DAT.
        expected: u64,
        /// The size in the zip.
        found: u64,
    },
}

/// The result of the check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// The set can start. `bios_differences` lists the ROMs of the BIOS set
    /// that no BIOS zip holds; the core needs only some of them.
    Pass {
        /// The BIOS ROMs that differ, for the diagnostics.
        bios_differences: Vec<RomProblem>,
    },
    /// Message 1: the zip stem is not a set of the index.
    UnknownSet {
        /// The zip stem.
        set: String,
    },
    /// Message 2: no zip of the BIOS name is found.
    MissingBios {
        /// The BIOS set.
        bios: String,
    },
    /// Message 3: a clone needs a ROM of its parent, and no zip of the
    /// parent name is found.
    MissingParent {
        /// The parent set.
        parent: String,
    },
    /// Message 4: ROMs of the set are missing or differ.
    Roms {
        /// Every ROM that is missing or differs.
        problems: Vec<RomProblem>,
    },
    /// Message 5: no BIOS zip holds any ROM of the BIOS set.
    BiosMismatch {
        /// The BIOS set.
        bios: String,
    },
    /// Message 6: a found zip cannot be read.
    Unreadable {
        /// The file name.
        file: String,
        /// Why.
        reason: String,
    },
    /// Message 7: there is no index, so nothing can be checked. The player
    /// chooses to start the game or not.
    NoIndex,
}

/// The zip names that the core opens for the set named `stem`, or `None`
/// when the index has no such set. The match ignores ASCII case.
pub fn wanted<'a>(index: &'a ArcadeIndex, stem: &str) -> Option<Wanted<'a>> {
    let set = index.find(stem)?;
    Some(Wanted {
        set: &set.name,
        parent: set.parent.as_deref(),
        bios: index.bios_of(set).map(|b| b.name.as_str()),
    })
}

/// Checks the set named `stem` against the zips that the caller found for
/// the names of [`wanted`].
///
/// The order of the rules: an unknown set; a missing BIOS zip; a zip that
/// cannot be read; a clone with no parent zip that needs one; a BIOS zip
/// that holds no ROM of the BIOS set; then each ROM of the set, except
/// the ROMs with no dump and the ROMs of the BIOS set (by CRC).
pub fn check_set(index: &ArcadeIndex, stem: &str, found: &FoundZips) -> Verdict {
    let Some(set) = index.find(stem) else {
        return Verdict::UnknownSet {
            set: stem.to_owned(),
        };
    };
    let bios = index.bios_of(set);
    if let Some(bios) = bios
        && found.bios.is_empty()
    {
        return Verdict::MissingBios {
            bios: bios.name.clone(),
        };
    }
    let every_zip = found.set.iter().chain(&found.parent).chain(&found.bios);
    for zip in every_zip {
        if let Err(reason) = &zip.entries {
            return Verdict::Unreadable {
                file: zip.file.clone(),
                reason: reason.clone(),
            };
        }
    }
    let bios_crcs: HashSet<u32> = bios
        .into_iter()
        .flat_map(|b| &b.roms)
        .filter_map(|r| r.crc)
        .collect();
    let needed: Vec<&RomRecord> = set
        .roms
        .iter()
        .filter(|r| r.crc.is_some_and(|crc| !bios_crcs.contains(&crc)))
        .collect();
    if let Some(parent) = &set.parent
        && found.parent.is_empty()
        && needed
            .iter()
            .any(|r| first_with_crc(&found.set, r).is_none())
    {
        return Verdict::MissingParent {
            parent: parent.clone(),
        };
    }
    let bios_differences = match bios {
        Some(bios) => {
            let differences =
                problems(bios.roms.iter().filter(|r| r.crc.is_some()), &[&found.bios]);
            let checked = bios.roms.iter().filter(|r| r.crc.is_some()).count();
            if checked > 0 && differences.len() == checked {
                return Verdict::BiosMismatch {
                    bios: bios.name.clone(),
                };
            }
            differences
        }
        None => Vec::new(),
    };
    let missing = problems(
        needed.into_iter(),
        &[&found.set, &found.parent, &found.bios],
    );
    if !missing.is_empty() {
        return Verdict::Roms { problems: missing };
    }
    Verdict::Pass { bios_differences }
}

impl Verdict {
    /// True when the launch stops. [`Verdict::NoIndex`] does not stop it by
    /// itself: the player chooses.
    pub fn stops(&self) -> bool {
        !matches!(self, Self::Pass { .. } | Self::NoIndex)
    }

    /// The message for the player, with `core` the name of the core, for
    /// example `FinalBurn Neo`. `None` for a set that passes.
    pub fn message(&self, core: &str) -> Option<String> {
        Some(match self {
            Self::Pass { .. } => return None,
            Self::UnknownSet { set } => {
                format!("{set}.zip is not a set name of {core}. Check the file name.")
            }
            Self::MissingBios { bios } => format!(
                "This game needs the BIOS file {bios}.zip. Put it in bios, or in the folder of the game."
            ),
            Self::MissingParent { parent } => {
                format!("This game needs the file {parent}.zip in the same folder.")
            }
            Self::Roms { problems } => format!(
                "{} files of this game are missing or differ. The set may be for another version of {core}, or a file may be damaged. The diagnostics page lists them.",
                problems.len()
            ),
            Self::BiosMismatch { bios } => {
                format!("The BIOS file {bios}.zip does not match this version of {core}.")
            }
            Self::Unreadable { file, reason } => {
                format!("The file {file} cannot be read: {reason}.")
            }
            Self::NoIndex => {
                "NEPTRON cannot find its list of arcade games, so it cannot check this game."
                    .to_owned()
            }
        })
    }

    /// The lines of the diagnostics page: one for each ROM that is missing
    /// or differs, in the set or in its BIOS.
    pub fn details(&self) -> Vec<String> {
        let problems = match self {
            Self::Roms { problems } => problems,
            Self::Pass { bios_differences } => bios_differences,
            _ => return Vec::new(),
        };
        problems
            .iter()
            .map(|p| match p.problem {
                Problem::Missing => format!("{}: missing", p.rom),
                Problem::Size { expected, found } => {
                    format!("{}: {found} bytes, expected {expected}", p.rom)
                }
            })
            .collect()
    }
}

/// The ROMs of `roms` that the zips of `places`, in that order, do not
/// give right.
fn problems<'a>(
    roms: impl Iterator<Item = &'a RomRecord>,
    places: &[&[FoundZip]],
) -> Vec<RomProblem> {
    roms.filter_map(|rom| {
        let entry = places.iter().find_map(|zips| first_with_crc(zips, rom));
        let problem = match entry {
            None => Problem::Missing,
            Some(e) if e.size != rom.size => Problem::Size {
                expected: rom.size,
                found: e.size,
            },
            Some(_) => return None,
        };
        Some(RomProblem {
            rom: rom.name.clone(),
            problem,
        })
    })
    .collect()
}

/// The first entry of `zips` with the CRC of `rom`.
fn first_with_crc<'a>(zips: &'a [FoundZip], rom: &RomRecord) -> Option<&'a ZipEntry> {
    let crc = rom.crc?;
    zips.iter()
        .filter_map(|z| z.entries.as_ref().ok())
        .flatten()
        .find(|e| e.crc == crc)
}

#[cfg(test)]
mod tests;
