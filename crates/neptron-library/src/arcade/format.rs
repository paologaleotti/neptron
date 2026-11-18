//! The text layout of the index: one record per line, fields split by one
//! tab, shown here as ` | `.
//!
//! ```text
//! neptron-arcade-index | 1
//! core | <core id> | <40-hex commit>
//! dat | <DAT file name> | <DAT version>
//! licence | <text>
//! set | <name> | <parent or -> | <romof or -> | <flags> | <year> | <maker> | <board> | <orientation> | <width> | <height> | <title> | <details>
//! rom | <name> | <size> | <crc or -> | <merge or -> | <flags>
//! disk | <name> | <sha1 or ->
//! ```
//!
//! Set flags, in this order, `-` for none: `b` BIOS, `v` vertical, `n` not
//! working, `p` preliminary. ROM flags: `-`, or `d` for a ROM with no dump.
//! The `disk` lines of a set follow its `rom` lines.
//! The format is append-only: a reader ignores fields after the ones it
//! knows, and skips a line whose first field it does not know.

use std::fmt::Write as _;

use super::{
    ArcadeIndex, DiskRecord, FORMAT_VERSION, IndexError, IndexSource, MAGIC, Orientation,
    RomRecord, SetFlags, SetRecord,
};

/// The field that stands for "none".
const NONE: &str = "-";

/// The text of the index file.
///
/// # Errors
///
/// [`IndexError::Separator`] when a field holds a tab or a line break, with
/// the set and the field, and [`IndexError::Reserved`] for a name that is
/// empty or that the reader would take for "none" (`-`).
pub fn encode(index: &ArcadeIndex) -> Result<String, IndexError> {
    let mut out = String::new();
    let source = &index.source;
    let header = [
        ("core", source.core.as_str()),
        ("commit", source.commit.as_str()),
        ("dat name", source.dat_name.as_str()),
        ("dat version", source.dat_version.as_str()),
        ("licence", source.licence.as_str()),
    ];
    for (field, value) in header {
        check_field("header", field, value)?;
    }
    put_line(&mut out, format_args!("{MAGIC}\t{FORMAT_VERSION}"));
    put_line(
        &mut out,
        format_args!("core\t{}\t{}", source.core, source.commit),
    );
    put_line(
        &mut out,
        format_args!("dat\t{}\t{}", source.dat_name, source.dat_version),
    );
    put_line(&mut out, format_args!("licence\t{}", source.licence));
    for set in index.sets() {
        put_set(&mut out, set)?;
    }
    Ok(out)
}

/// The index in the text of an index file.
///
/// The reader checks the magic and the version first. It needs a `core`
/// and a `dat` line before the first set, sets sorted by name with no name
/// twice, and each `rom` line after a `set` line. It skips a line whose
/// first field it does not know, and fields after the ones it knows.
///
/// # Errors
///
/// [`IndexError::Magic`] and [`IndexError::Version`] for a file of another
/// kind or a newer format, [`IndexError::MissingHeader`], and
/// [`IndexError::Line`] with the line number for any other fault.
pub fn decode(text: &str) -> Result<ArcadeIndex, IndexError> {
    let mut lines = text.lines().enumerate().map(|(i, l)| (i + 1, l));
    let first = lines.next().map_or("", |(_, l)| l);
    check_magic(first)?;
    let mut source = IndexSource::default();
    let mut have_core = false;
    let mut have_dat = false;
    let mut sets: Vec<SetRecord> = Vec::new();
    for (n, line) in lines {
        let mut fields = line.split('\t');
        let kind = fields.next().unwrap_or("");
        let mut f = Fields { fields, line: n };
        match kind {
            "core" => {
                source.core = f.text("core id")?.to_owned();
                source.commit = f.text("core commit")?.to_owned();
                have_core = true;
            }
            "dat" => {
                source.dat_name = f.text("DAT name")?.to_owned();
                source.dat_version = f.text("DAT version")?.to_owned();
                have_dat = true;
            }
            "licence" => source.licence = f.text("licence")?.to_owned(),
            "set" => {
                if !have_core {
                    return Err(IndexError::MissingHeader { what: "core" });
                }
                if !have_dat {
                    return Err(IndexError::MissingHeader { what: "dat" });
                }
                let set = read_set(&mut f)?;
                if sets.last().is_some_and(|last| last.name >= set.name) {
                    return Err(f.bad("set order"));
                }
                sets.push(set);
            }
            "rom" => {
                let rom = read_rom(&mut f)?;
                sets.last_mut()
                    .ok_or_else(|| f.bad("rom before any set"))?
                    .roms
                    .push(rom);
            }
            "disk" => {
                let disk = read_disk(&mut f)?;
                sets.last_mut()
                    .ok_or_else(|| f.bad("disk before any set"))?
                    .disks
                    .push(disk);
            }
            _ => {}
        }
    }
    if !have_core {
        return Err(IndexError::MissingHeader { what: "core" });
    }
    if !have_dat {
        return Err(IndexError::MissingHeader { what: "dat" });
    }
    Ok(ArcadeIndex { source, sets })
}

fn check_magic(first: &str) -> Result<(), IndexError> {
    let mut fields = first.split('\t');
    let magic = fields.next().unwrap_or("");
    if magic != MAGIC {
        return Err(IndexError::Magic {
            found: magic.chars().take(64).collect(),
            expected: MAGIC,
        });
    }
    let version = fields.next().unwrap_or("");
    match version.parse::<u32>() {
        Ok(v) if (1..=FORMAT_VERSION).contains(&v) => Ok(()),
        _ => Err(IndexError::Version {
            found: version.chars().take(64).collect(),
            supported: FORMAT_VERSION,
        }),
    }
}

fn put_set(out: &mut String, set: &SetRecord) -> Result<(), IndexError> {
    let parent = set.parent.as_deref().unwrap_or(NONE);
    let romof = set.romof.as_deref().unwrap_or(NONE);
    let texts = [
        ("name", set.name.as_str()),
        ("cloneof", parent),
        ("romof", romof),
        ("year", set.year.as_str()),
        ("manufacturer", set.maker.as_str()),
        ("sourcefile", set.board.as_str()),
        ("title", set.title.as_str()),
        ("details", set.details.as_str()),
    ];
    for (field, value) in texts {
        check_field(&set.name, field, value)?;
    }
    check_name(&set.name, "cloneof", set.parent.as_deref())?;
    check_name(&set.name, "romof", set.romof.as_deref())?;
    let orientation = match set.orientation {
        Orientation::Horizontal => 'h',
        Orientation::Vertical => 'v',
    };
    put_line(
        out,
        format_args!(
            "set\t{}\t{parent}\t{romof}\t{}\t{}\t{}\t{}\t{orientation}\t{}\t{}\t{}\t{}",
            set.name,
            set_flags(set.flags),
            set.year,
            set.maker,
            set.board,
            set.width,
            set.height,
            set.title,
            set.details,
        ),
    );
    for rom in &set.roms {
        check_field(&set.name, "rom name", &rom.name)?;
        if let Some(merge) = &rom.merge {
            check_field(&set.name, "rom merge", merge)?;
        }
        check_name(&set.name, "rom name", Some(&rom.name))?;
        check_name(&set.name, "rom merge", rom.merge.as_deref())?;
        let crc = rom
            .crc
            .map_or_else(|| NONE.to_owned(), |c| format!("{c:08x}"));
        let merge = rom.merge.as_deref().unwrap_or(NONE);
        let flags = if rom.crc.is_none() { "d" } else { NONE };
        put_line(
            out,
            format_args!("rom\t{}\t{}\t{crc}\t{merge}\t{flags}", rom.name, rom.size),
        );
    }
    for disk in &set.disks {
        check_field(&set.name, "disk name", &disk.name)?;
        check_name(&set.name, "disk name", Some(&disk.name))?;
        let sha1 = match &disk.sha1 {
            Some(sha1) if is_sha1(sha1) => sha1.as_str(),
            Some(other) => {
                return Err(IndexError::DiskSha1 {
                    set: set.name.clone(),
                    disk: disk.name.clone(),
                    value: other.clone(),
                });
            }
            None => NONE,
        };
        put_line(out, format_args!("disk\t{}\t{sha1}", disk.name));
    }
    Ok(())
}

fn check_field(set: &str, field: &'static str, value: &str) -> Result<(), IndexError> {
    if value.contains(['\t', '\n', '\r']) {
        return Err(IndexError::Separator {
            set: set.to_owned(),
            field,
        });
    }
    Ok(())
}

/// Appends one line of text and its line break.
fn put_line(out: &mut String, args: std::fmt::Arguments<'_>) {
    // The `fmt::Write` of a String never returns an error: it only grows
    // the String, and an allocation failure aborts instead.
    let _ = out.write_fmt(args);
    out.push('\n');
}

/// A name field that is present must not be empty or `-`: the reader
/// takes `-` for "none" and refuses an empty name.
fn check_name(set: &str, field: &'static str, value: Option<&str>) -> Result<(), IndexError> {
    match value {
        Some(v) if v.is_empty() || v == NONE => Err(IndexError::Reserved {
            set: set.to_owned(),
            field,
        }),
        _ => Ok(()),
    }
}

fn set_flags(flags: SetFlags) -> String {
    let mut out = String::new();
    for (on, c) in [
        (flags.bios, 'b'),
        (flags.vertical, 'v'),
        (flags.not_working, 'n'),
        (flags.preliminary, 'p'),
    ] {
        if on {
            out.push(c);
        }
    }
    if out.is_empty() {
        out.push_str(NONE);
    }
    out
}

fn read_set(f: &mut Fields<'_>) -> Result<SetRecord, IndexError> {
    let name = f.text("set name")?;
    if !super::valid_set_name(name) {
        return Err(f.bad("set name"));
    }
    let parent = f.optional("parent")?;
    let romof = f.optional("romof")?;
    let flags = read_set_flags(f.text("set flags")?).ok_or_else(|| f.bad("set flags"))?;
    let year = f.text("year")?.to_owned();
    let maker = f.text("maker")?.to_owned();
    let board = f.text("board")?.to_owned();
    let orientation = match f.text("orientation")? {
        "h" => Orientation::Horizontal,
        "v" => Orientation::Vertical,
        _ => return Err(f.bad("orientation")),
    };
    let width = f.number("width")?;
    let height = f.number("height")?;
    let title = f.text("title")?.to_owned();
    let details = f.text("details")?.to_owned();
    Ok(SetRecord {
        name: name.to_owned(),
        parent,
        romof,
        flags,
        year,
        maker,
        board,
        orientation,
        width,
        height,
        title,
        details,
        roms: Vec::new(),
        disks: Vec::new(),
    })
}

/// The flags of a `set` line: `-`, or the letters `b`, `v`, `n`, `p` in
/// that order, each at most once. A letter that this reader does not know
/// is skipped, so a later format can add one.
fn read_set_flags(text: &str) -> Option<SetFlags> {
    let mut flags = SetFlags::default();
    if text == NONE {
        return Some(flags);
    }
    if text.is_empty() {
        return None;
    }
    let mut last = 0usize;
    for c in text.chars() {
        let (slot, rank) = match c {
            'b' => (&mut flags.bios, 1),
            'v' => (&mut flags.vertical, 2),
            'n' => (&mut flags.not_working, 3),
            'p' => (&mut flags.preliminary, 4),
            c if c.is_ascii_lowercase() => continue,
            _ => return None,
        };
        if rank <= last {
            return None;
        }
        last = rank;
        *slot = true;
    }
    Some(flags)
}

fn read_rom(f: &mut Fields<'_>) -> Result<RomRecord, IndexError> {
    let name = f.text("rom name")?.to_owned();
    if name.is_empty() || name == NONE {
        return Err(f.bad("rom name"));
    }
    let size = f
        .text("rom size")?
        .parse::<u64>()
        .map_err(|_| f.bad("rom size"))?;
    let crc = match f.text("rom crc")? {
        NONE => None,
        hex if is_crc(hex) => Some(u32::from_str_radix(hex, 16).map_err(|_| f.bad("rom crc"))?),
        _ => return Err(f.bad("rom crc")),
    };
    let merge = f.optional("rom merge")?;
    let nodump = match f.text("rom flags")? {
        NONE => false,
        "d" => true,
        _ => return Err(f.bad("rom flags")),
    };
    if nodump != crc.is_none() {
        return Err(f.bad("rom crc"));
    }
    Ok(RomRecord {
        name,
        size,
        crc,
        merge,
    })
}

fn read_disk(f: &mut Fields<'_>) -> Result<DiskRecord, IndexError> {
    let name = f.text("disk name")?.to_owned();
    if name.is_empty() || name == NONE {
        return Err(f.bad("disk name"));
    }
    let sha1 = match f.text("disk sha1")? {
        NONE => None,
        hex if is_sha1(hex) => Some(hex.to_owned()),
        _ => return Err(f.bad("disk sha1")),
    };
    Ok(DiskRecord { name, sha1 })
}

/// Forty lower-case hex digits.
fn is_sha1(text: &str) -> bool {
    text.len() == 40 && is_lower_hex(text)
}

/// Lower-case hex digits only.
fn is_lower_hex(text: &str) -> bool {
    text.bytes()
        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Eight lower-case hex digits.
fn is_crc(text: &str) -> bool {
    text.len() == 8 && is_lower_hex(text)
}

/// The fields of one line after its first.
struct Fields<'a> {
    fields: std::str::Split<'a, char>,
    line: usize,
}

impl<'a> Fields<'a> {
    fn text(&mut self, what: &'static str) -> Result<&'a str, IndexError> {
        let line = self.line;
        self.fields.next().ok_or(IndexError::Line { line, what })
    }

    fn optional(&mut self, what: &'static str) -> Result<Option<String>, IndexError> {
        match self.text(what)? {
            NONE => Ok(None),
            "" => Err(self.bad(what)),
            text => Ok(Some(text.to_owned())),
        }
    }

    fn number(&mut self, what: &'static str) -> Result<u32, IndexError> {
        self.text(what)?.parse().map_err(|_| self.bad(what))
    }

    fn bad(&self, what: &'static str) -> IndexError {
        IndexError::Line {
            line: self.line,
            what,
        }
    }
}
