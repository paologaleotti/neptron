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

use super::{ArcadeIndex, IndexSource, RomRecord};

/// The most ROM names that message 4 gives; it counts the others. With
/// each name cut to [`NAMED_ROM_CHARS_MAX`], the longest message keeps to
/// the lines of a dialog on the 320x240 canvas of the UI.
pub const NAMED_ROMS_MAX: usize = 2;

/// The most characters of one ROM name in message 4.
pub const NAMED_ROM_CHARS_MAX: usize = 24;

/// A set with more than this share of its ROMs missing or different, in
/// percent, is a set for another emulator or another version, and gets
/// its own message.
pub const WRONG_SET_PERCENT: usize = 50;

/// The extensions of a disk image that the core tries, in its order. It
/// knows a CHD by its signature, so a raw dump can carry either name.
pub const DISK_EXTENSIONS: [&str; 2] = ["chd", "img"];

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
    /// The names of the disk images of the set with a file at one of the
    /// paths of [`disk_paths`].
    pub disks: Vec<String>,
}

/// One path where the core looks for a disk image of a set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiskPath {
    /// The name of the disk image, with no extension.
    pub disk: String,
    /// The path relative to the folder of the game zip: a folder with the
    /// name of the set or of its parent, and the image in it, for example
    /// `kinst/kinst.chd`.
    pub path: String,
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
    /// Message 8: no file of a disk image of the set is at a path where
    /// the core looks.
    MissingDisk {
        /// The name of the disk image, with no extension.
        disk: String,
        /// The folder that the message names, next to the game zip: the
        /// folder of the parent for a clone, else of the set.
        folder: String,
        /// The zip stem of the set.
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
    /// Message 4: ROMs of the set are missing or differ. When more than
    /// [`WRONG_SET_PERCENT`] of the ROMs that the check asks for are, the
    /// message says that the set is for another emulator or version.
    Roms {
        /// Every ROM that is missing or differs.
        problems: Vec<RomProblem>,
        /// The number of ROMs that the check asked for.
        checked: usize,
        /// The version of the sets that the index lists, see
        /// [`wanted_version`].
        wanted: String,
    },
    /// Message 5: no BIOS zip holds any ROM of the BIOS set.
    BiosMismatch {
        /// The BIOS set.
        bios: String,
        /// The version of the sets that the index lists, see
        /// [`wanted_version`].
        wanted: String,
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
/// the ROMs with no dump and the ROMs of the BIOS set (by CRC); last, a
/// disk image with no file at a path of [`disk_paths`].
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
                    wanted: wanted_version(&index.source),
                };
            }
            differences
        }
        None => Vec::new(),
    };
    let checked = needed.len();
    let missing = problems(
        needed.into_iter(),
        &[&found.set, &found.parent, &found.bios],
    );
    if !missing.is_empty() {
        return Verdict::Roms {
            problems: missing,
            checked,
            wanted: wanted_version(&index.source),
        };
    }
    if let Some(disk) = set.disks.iter().find(|d| !found.disks.contains(&d.name)) {
        return Verdict::MissingDisk {
            disk: disk.name.clone(),
            folder: set.parent.clone().unwrap_or_else(|| set.name.clone()),
            set: set.name.clone(),
        };
    }
    Verdict::Pass { bios_differences }
}

/// The paths where the core looks for the disk images of the set named
/// `stem`, in its order: for each extension of [`DISK_EXTENSIONS`], the
/// folder of the set, then the folder of its parent, each next to the
/// game zip. Empty for a set with no disk or not in the index.
///
/// FinalBurn Neo builds the path from its HDD folder, which the libretro
/// port sets to the folder of the game zip, the driver name (then the
/// parent name) and the disk name (`BurnHDDOpen` in `burn_hdd.cpp`).
pub fn disk_paths(index: &ArcadeIndex, stem: &str) -> Vec<DiskPath> {
    let Some(set) = index.find(stem) else {
        return Vec::new();
    };
    let folders: Vec<&str> = std::iter::once(set.name.as_str())
        .chain(set.parent.as_deref())
        .collect();
    let mut paths = Vec::new();
    for disk in &set.disks {
        for ext in DISK_EXTENSIONS {
            for folder in &folders {
                paths.push(DiskPath {
                    disk: disk.name.clone(),
                    path: format!("{folder}/{}.{ext}", disk.name),
                });
            }
        }
    }
    paths
}

/// The version of the sets that an index lists, for the messages: the
/// version of its DAT and the short commit of the core, for example
/// `1.0.0.03 (commit 0123abc)`. Either part is left out when the index
/// has none.
pub fn wanted_version(source: &IndexSource) -> String {
    let commit: String = source.commit.chars().take(7).collect();
    match (source.dat_version.is_empty(), commit.is_empty()) {
        (false, false) => format!("{} (commit {commit})", source.dat_version),
        (false, true) => source.dat_version.clone(),
        (true, false) => format!("commit {commit}"),
        (true, true) => String::new(),
    }
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
            Self::MissingDisk { disk, folder, set } => format!(
                "This game needs the hard disk image {disk}.chd. Put it in the folder {folder}, next to {set}.zip."
            ),
            Self::MissingBios { bios } => format!(
                "This game needs the BIOS file {bios}.zip. Put it in bios, or in the folder of the game."
            ),
            Self::MissingParent { parent } => {
                format!("This game needs the file {parent}.zip in the same folder.")
            }
            Self::Roms {
                problems, wanted, ..
            } => {
                let needs = needs(core, wanted);
                if self.wrong_set() {
                    format!(
                        "Most files of this game do not match. The set is for another emulator or another version.{needs}"
                    )
                } else {
                    let (count, verb) = match problems.len() {
                        1 => ("1 file".to_owned(), "is missing or differs"),
                        n => (format!("{n} files"), "are missing or differ"),
                    };
                    format!(
                        "{count} of this game {verb}: {}.{needs} The log lists every file.",
                        named(problems)
                    )
                }
            }
            Self::BiosMismatch { bios, wanted } => format!(
                "The BIOS file {bios}.zip does not match this version of {core}.{}",
                needs(core, wanted)
            ),
            Self::Unreadable { file, reason } => {
                format!("The file {file} cannot be read: {reason}.")
            }
            Self::NoIndex => {
                "NEPTRON cannot find its list of arcade games, so it cannot check this game."
                    .to_owned()
            }
        })
    }

    /// True for message 4 when more than [`WRONG_SET_PERCENT`] of the ROMs
    /// that the check asked for are missing or differ.
    pub fn wrong_set(&self) -> bool {
        match self {
            Self::Roms {
                problems, checked, ..
            } => problems.len() * 100 > checked * WRONG_SET_PERCENT,
            _ => false,
        }
    }

    /// The lines of the log: one for each ROM that is missing or differs,
    /// in the set or in its BIOS.
    pub fn details(&self) -> Vec<String> {
        let problems = match self {
            Self::Roms { problems, .. } => problems,
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

/// The sentence that names the sets the image needs, with a leading
/// space, or nothing when the index gives no version.
fn needs(core: &str, wanted: &str) -> String {
    if wanted.is_empty() {
        String::new()
    } else {
        format!(" This image needs sets for {core} {wanted}.")
    }
}

/// The names of the first [`NAMED_ROMS_MAX`] ROMs of `problems`, each cut
/// to [`NAMED_ROM_CHARS_MAX`], and a count of the others:
/// `a.bin`, `a.bin and b.bin`, `a.bin, b.bin and 3 more`. The names come
/// from the index, never from the zip.
fn named(problems: &[RomProblem]) -> String {
    let names: Vec<String> = problems
        .iter()
        .take(NAMED_ROMS_MAX)
        .map(|p| p.rom.chars().take(NAMED_ROM_CHARS_MAX).collect())
        .collect();
    let more = problems.len().saturating_sub(names.len());
    match (names.as_slice(), more) {
        ([one], 0) => one.clone(),
        ([first, second], 0) => format!("{first} and {second}"),
        (listed, more) => format!("{} and {more} more", listed.join(", ")),
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
