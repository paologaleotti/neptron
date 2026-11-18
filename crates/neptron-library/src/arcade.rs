//! The arcade index: the sets of the DAT of one arcade core, as a text file
//! that the image holds (`docs/spec/arcade.md`, section 6).
//!
//! `cargo xtask arcade-index` makes the index from the DAT with
//! [`encode`]; the frontend reads it with [`read`] on the scan thread.
//! Every function here is pure except [`read`].

use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

mod check;
mod format;

pub use check::{
    DISK_EXTENSIONS, DiskPath, FoundZip, FoundZips, NAMED_ROM_CHARS_MAX, NAMED_ROMS_MAX, Problem,
    RomProblem, Verdict, WRONG_SET_PERCENT, Wanted, ZipEntry, check_set, disk_paths, wanted,
    wanted_version,
};
pub use format::{decode, encode};

/// The first field of the first line of an index file.
pub const MAGIC: &str = "neptron-arcade-index";

/// The format version that this crate writes, and the highest it reads.
pub const FORMAT_VERSION: u32 = 1;

/// The largest index file that [`read`] accepts, in bytes.
///
/// The index of the FinalBurn Neo DAT (about 8,500 sets and 170,000 ROMs)
/// takes about 7 MiB. The limit leaves room for a larger DAT and stops a
/// wrong file from filling the memory of the device.
pub const MAX_INDEX_BYTES: u64 = 32 * 1024 * 1024;

/// The most `romof` steps that [`ArcadeIndex::bios_of`] follows. A real
/// chain has two (clone, parent, BIOS); the bound stops a loop.
pub const MAX_ROMOF_STEPS: usize = 8;

/// The sets of one arcade core, sorted by name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArcadeIndex {
    /// Where the index comes from.
    pub source: IndexSource,
    sets: Vec<SetRecord>,
}

/// Where an index comes from: the core, its commit and its DAT.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IndexSource {
    /// The core id, as `data/cores` gives it, for example `fbneo`.
    pub core: String,
    /// The 40-hex commit of the core that the DAT comes from.
    pub commit: String,
    /// The file name of the DAT.
    pub dat_name: String,
    /// The version that the DAT header gives.
    pub dat_version: String,
    /// The licence line of the index.
    pub licence: String,
}

/// One set of the DAT.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetRecord {
    /// The set name: the zip file stem, ASCII lower case.
    pub name: String,
    /// The parent set (`cloneof`), for a clone.
    pub parent: Option<String>,
    /// The set whose ROMs this set uses (`romof`): its parent or its BIOS.
    pub romof: Option<String>,
    /// The flags.
    pub flags: SetFlags,
    /// The year, as the DAT gives it (it can hold `?`).
    pub year: String,
    /// The maker.
    pub maker: String,
    /// The driver source file of the core, which names the board.
    pub board: String,
    /// The orientation of the screen of the board.
    pub orientation: Orientation,
    /// The width of the picture, as the DAT gives it.
    pub width: u32,
    /// The height of the picture, as the DAT gives it.
    pub height: u32,
    /// The title: the description less its last bracket group.
    pub title: String,
    /// The last bracket group of the description, with no brackets. Empty
    /// when the description has none.
    pub details: String,
    /// The ROMs, in DAT order.
    pub roms: Vec<RomRecord>,
    /// The hard disk images (CHD), in DAT order. The core opens them
    /// itself; the set check asks only that a file is present.
    pub disks: Vec<DiskRecord>,
}

/// The flags of a set.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SetFlags {
    /// A BIOS set (`isbios="yes"`): not a game.
    pub bios: bool,
    /// A vertical screen.
    pub vertical: bool,
    /// The driver does not work.
    pub not_working: bool,
    /// The driver status is `preliminary`.
    pub preliminary: bool,
}

/// The orientation of the screen of a board.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Orientation {
    /// A horizontal screen.
    #[default]
    Horizontal,
    /// A vertical screen.
    Vertical,
}

/// One ROM of a set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RomRecord {
    /// The file name in the zip.
    pub name: String,
    /// The size in bytes.
    pub size: u64,
    /// The CRC-32. `None` for a ROM with no dump.
    pub crc: Option<u32>,
    /// The name of the same ROM in the parent or the BIOS set, if any.
    pub merge: Option<String>,
}

/// One hard disk image of a set (a `disk` element of the DAT).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiskRecord {
    /// The name of the image, with no extension: the core looks for
    /// `<name>.chd`.
    pub name: String,
    /// The SHA-1 of the image, 40 lower-case hex digits, when the DAT
    /// gives it.
    pub sha1: Option<String>,
}

/// An index that cannot be read, made or written.
#[derive(Debug, thiserror::Error)]
pub enum IndexError {
    /// The file could not be read.
    #[error("arcade index {path}: {source}")]
    Io {
        /// The file.
        path: PathBuf,
        /// The error from the file system.
        source: io::Error,
    },
    /// The file is larger than [`MAX_INDEX_BYTES`].
    #[error("the arcade index {path} has more than {limit} bytes")]
    TooLarge {
        /// The file.
        path: PathBuf,
        /// The limit in bytes.
        limit: u64,
    },
    /// The file is not UTF-8.
    #[error("the arcade index is not UTF-8 text (byte {offset})")]
    NotText {
        /// The offset of the first bad byte.
        offset: usize,
    },
    /// The first line does not start with [`MAGIC`].
    #[error("the arcade index starts with `{found}`, expected `{expected}`")]
    Magic {
        /// The first field of the file.
        found: String,
        /// The magic that this crate reads.
        expected: &'static str,
    },
    /// The format version is higher than [`FORMAT_VERSION`], or not a number.
    #[error("arcade index format version `{found}`, this build reads up to {supported}")]
    Version {
        /// The version field of the file.
        found: String,
        /// The highest version that this crate reads.
        supported: u32,
    },
    /// A line does not follow the format.
    #[error("line {line} of the arcade index: bad {what}")]
    Line {
        /// The line number, from 1.
        line: usize,
        /// The field or the rule that is broken.
        what: &'static str,
    },
    /// The index has no `core` or no `dat` line before its first set.
    #[error("the arcade index has no `{what}` line")]
    MissingHeader {
        /// The kind of line.
        what: &'static str,
    },
    /// A field holds a tab or a line break, which the format cannot hold.
    #[error("the field `{field}` of the set `{set}` holds a tab or a line break")]
    Separator {
        /// The set.
        set: String,
        /// The field.
        field: &'static str,
    },
    /// A name is empty or `-`, which the reader takes for "none".
    #[error("the field `{field}` of the set `{set}` is empty or `-`")]
    Reserved {
        /// The set.
        set: String,
        /// The field.
        field: &'static str,
    },
    /// The SHA-1 of a disk is not 40 lower-case hex digits.
    #[error(
        "the disk `{disk}` of the set `{set}` has the SHA-1 `{value}`, not 40 lower-case hex digits"
    )]
    DiskSha1 {
        /// The set.
        set: String,
        /// The disk.
        disk: String,
        /// The SHA-1 as given.
        value: String,
    },
    /// A set name is empty or is not ASCII lower case.
    #[error("the set name `{set}` is empty or not ASCII lower case")]
    SetName {
        /// The set name.
        set: String,
    },
    /// Two sets have the same name.
    #[error("the set `{set}` is in the index twice")]
    Duplicate {
        /// The set name.
        set: String,
    },
}

impl ArcadeIndex {
    /// An index of `sets`, sorted here by name in byte order.
    ///
    /// # Errors
    ///
    /// [`IndexError::SetName`] for an empty set name or one that is not
    /// ASCII lower case, and [`IndexError::Duplicate`] for a name that comes
    /// twice.
    pub fn from_sets(source: IndexSource, mut sets: Vec<SetRecord>) -> Result<Self, IndexError> {
        if let Some(bad) = sets.iter().find(|s| !valid_set_name(&s.name)) {
            return Err(IndexError::SetName {
                set: bad.name.clone(),
            });
        }
        sets.sort_by(|a, b| a.name.cmp(&b.name));
        if let Some(pair) = sets.windows(2).find(|w| w[0].name == w[1].name) {
            return Err(IndexError::Duplicate {
                set: pair[0].name.clone(),
            });
        }
        Ok(Self { source, sets })
    }

    /// Every set, sorted by name.
    pub fn sets(&self) -> &[SetRecord] {
        &self.sets
    }

    /// The set named `stem`, a zip file stem. The match ignores ASCII case,
    /// as exFAT does. It allocates nothing.
    pub fn find(&self, stem: &str) -> Option<&SetRecord> {
        self.sets
            .binary_search_by(|s| {
                let ours = s.name.bytes();
                let theirs = stem.bytes().map(|b| b.to_ascii_lowercase());
                ours.cmp(theirs)
            })
            .ok()
            .and_then(|i| self.sets.get(i))
    }

    /// The BIOS set of `set`: the first set with the BIOS flag along the
    /// `romof` chain, the set itself excluded. `None` when the chain has
    /// none, names a set that the index lacks, or is longer than
    /// [`MAX_ROMOF_STEPS`].
    pub fn bios_of(&self, set: &SetRecord) -> Option<&SetRecord> {
        let mut next = set.romof.as_deref();
        for _ in 0..MAX_ROMOF_STEPS {
            let current = self.find(next?)?;
            if current.flags.bios {
                return Some(current);
            }
            next = current.romof.as_deref();
        }
        None
    }

    /// The number of BIOS sets.
    pub fn bios_count(&self) -> usize {
        self.sets.iter().filter(|s| s.flags.bios).count()
    }
}

impl SetRecord {
    /// A set with this name and title and no other data: no parent, no
    /// flags, no ROMs, no disks, a horizontal screen of 0 by 0.
    pub fn new(name: String, title: String) -> Self {
        Self {
            name,
            parent: None,
            romof: None,
            flags: SetFlags::default(),
            year: String::new(),
            maker: String::new(),
            board: String::new(),
            orientation: Orientation::Horizontal,
            width: 0,
            height: 0,
            title,
            details: String::new(),
            roms: Vec::new(),
            disks: Vec::new(),
        }
    }
}

/// Reads and parses the index file at `path` (see [`decode`]).
///
/// # Errors
///
/// [`IndexError::Io`] when the file cannot be read, [`IndexError::TooLarge`]
/// above [`MAX_INDEX_BYTES`], and the errors of [`decode`].
pub fn read(path: &Path) -> Result<ArcadeIndex, IndexError> {
    let io_error = |source| IndexError::Io {
        path: path.to_path_buf(),
        source,
    };
    let mut bytes = Vec::new();
    File::open(path)
        .map_err(io_error)?
        .take(MAX_INDEX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(io_error)?;
    if bytes.len() as u64 > MAX_INDEX_BYTES {
        return Err(IndexError::TooLarge {
            path: path.to_path_buf(),
            limit: MAX_INDEX_BYTES,
        });
    }
    let text = std::str::from_utf8(&bytes).map_err(|e| IndexError::NotText {
        offset: e.valid_up_to(),
    })?;
    decode(text)
}

/// The title and the details of a DAT description: the title is the
/// description less its last bracket group, the details are the text in
/// that group. A description that does not end with a bracket group, or
/// that is one group alone, is all title.
///
/// `"Star Patrol - Mission One (XYZ-0001 ~ XYZ-0002)"` gives
/// `("Star Patrol - Mission One", "XYZ-0001 ~ XYZ-0002")`. The group can hold
/// brackets of its own: `"Game (World (rev 1))"` gives `("Game", "World (rev 1)")`.
pub fn split_description(description: &str) -> (&str, &str) {
    let text = description.trim();
    let Some(inner_end) = text.strip_suffix(')').map(str::len) else {
        return (text, "");
    };
    let mut depth = 0usize;
    for (at, c) in text.char_indices().rev() {
        match c {
            ')' => depth += 1,
            '(' => {
                depth -= 1;
                if depth == 0 {
                    let title = text[..at].trim_end();
                    if title.is_empty() {
                        return (text, "");
                    }
                    return (title, text[at + 1..inner_end].trim());
                }
            }
            _ => {}
        }
    }
    (text, "")
}

/// A set name is not empty and holds no ASCII upper-case letter, so the
/// lookup that lowers a file stem finds it.
fn valid_set_name(name: &str) -> bool {
    !name.is_empty() && !name.bytes().any(|b| b.is_ascii_uppercase())
}

#[cfg(test)]
mod tests;
