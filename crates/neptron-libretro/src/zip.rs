//! Reads one entry of a zip archive into memory: stored and deflate entries
//! only, no ZIP64, no encryption, no multi-disk archives.
//!
//! The reader only reads. Content archives are user data, and the caller
//! opens them read-only.

use std::io::{self, Read, Seek, SeekFrom};

use crate::crc32::crc32;

/// The entry that [`read_zip_entry`] found, read into memory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZipEntry {
    /// The name of the entry in the archive, with its folders.
    pub name: String,
    /// The uncompressed bytes.
    pub data: Vec<u8>,
}

/// Why a zip entry could not be read.
#[derive(Debug, thiserror::Error)]
pub enum ZipError {
    /// Reading the archive failed.
    #[error("cannot read the archive: {0}")]
    Io(#[source] io::Error),
    /// The archive has no end of central directory record.
    #[error("no end of central directory record: not a zip file, or a damaged one")]
    NoEndOfCentralDirectory,
    /// The archive spans more than one disk.
    #[error("multi-disk archives are not supported")]
    MultiDisk,
    /// The archive or the entry needs ZIP64.
    #[error("ZIP64 archives are not supported")]
    Zip64,
    /// The central directory is outside the file, too large, or damaged.
    #[error("damaged central directory: {0}")]
    CentralDirectory(&'static str),
    /// No entry has an extension of the list.
    #[error("no entry with one of the extensions {extensions:?}")]
    NoMatchingEntry {
        /// The extensions that were accepted.
        extensions: Vec<String>,
    },
    /// The entry is encrypted.
    #[error("entry {name} is encrypted")]
    Encrypted {
        /// The entry name.
        name: String,
    },
    /// The entry uses a compression method other than stored or deflate.
    #[error(
        "entry {name} uses compression method {method}; only stored (0) and deflate (8) are supported"
    )]
    Method {
        /// The entry name.
        name: String,
        /// The method number of the archive.
        method: u16,
    },
    /// The entry is larger than the limit.
    #[error("entry {name} has {size} bytes, more than the limit of {limit} bytes")]
    TooLarge {
        /// The entry name.
        name: String,
        /// The uncompressed size that the archive gives.
        size: u64,
        /// The limit.
        limit: u64,
    },
    /// The local header of the entry is missing or does not fit the file.
    #[error("damaged local header of entry {name}")]
    LocalHeader {
        /// The entry name.
        name: String,
    },
    /// The deflate data is damaged or inflates to more than the given size.
    #[error("damaged deflate data in entry {name}")]
    Inflate {
        /// The entry name.
        name: String,
    },
    /// The entry data does not have the size that the archive gives.
    #[error("entry {name} has {actual} bytes, the archive says {expected}")]
    SizeMismatch {
        /// The entry name.
        name: String,
        /// The size that the archive gives.
        expected: u64,
        /// The size found.
        actual: u64,
    },
    /// The CRC-32 of the data does not match the archive.
    #[error(
        "entry {name} fails its CRC-32 check: {actual:#010x}, the archive says {expected:#010x}"
    )]
    Crc {
        /// The entry name.
        name: String,
        /// The CRC-32 that the archive gives.
        expected: u32,
        /// The CRC-32 of the data.
        actual: u32,
    },
}

/// The largest central directory that the reader accepts, in bytes. One
/// entry takes 46 bytes plus its name, so this holds far more ROMs than one
/// archive holds.
pub const MAX_CENTRAL_DIRECTORY_BYTES: u64 = 16 * 1024 * 1024;

/// Reads the first entry, in central-directory order, that is not a folder
/// and whose extension is in `extensions` (without dots, compared without
/// case). The entry must be stored or deflated and have at most
/// `limit_bytes` bytes. Its CRC-32 is checked.
pub fn read_zip_entry<R: Read + Seek>(
    archive: &mut R,
    extensions: &[String],
    limit_bytes: u64,
) -> Result<ZipEntry, ZipError> {
    let eocd = find_end_of_central_directory(archive)?;
    let directory = read_at(archive, eocd.directory_offset, eocd.directory_size)?;
    let entry = find_entry(&directory, eocd.entries, extensions)?;
    read_entry(archive, &entry, eocd.directory_offset, limit_bytes)
}

const EOCD_SIGNATURE: u32 = 0x0605_4b50;
const EOCD_LEN: u64 = 22;
const MAX_COMMENT_LEN: u64 = 0xFFFF;
const CENTRAL_HEADER_SIGNATURE: u32 = 0x0201_4b50;
const CENTRAL_HEADER_LEN: usize = 46;
const LOCAL_HEADER_SIGNATURE: u32 = 0x0403_4b50;
const LOCAL_HEADER_LEN: u64 = 30;
const FLAG_ENCRYPTED: u16 = 1;
const FLAG_UTF8: u16 = 1 << 11;
const METHOD_STORED: u16 = 0;
const METHOD_DEFLATE: u16 = 8;
const ZIP64_MARK_16: u16 = 0xFFFF;
const ZIP64_MARK_32: u32 = 0xFFFF_FFFF;
/// Deflate can make incompressible data larger: 5 bytes per stored block of
/// at most 65,535 bytes. This bound is far above that and still keeps a
/// damaged size from making the reader allocate a large buffer.
const fn max_compressed_len(uncompressed: u64) -> u64 {
    uncompressed + uncompressed / 16 + 1024
}

struct EndOfCentralDirectory {
    entries: u16,
    directory_size: u64,
    directory_offset: u64,
}

struct CentralEntry {
    name: String,
    flags: u16,
    method: u16,
    crc: u32,
    compressed_size: u32,
    uncompressed_size: u32,
    local_header_offset: u32,
}

fn find_end_of_central_directory<R: Read + Seek>(
    archive: &mut R,
) -> Result<EndOfCentralDirectory, ZipError> {
    let len = archive.seek(SeekFrom::End(0)).map_err(ZipError::Io)?;
    if len < EOCD_LEN {
        return Err(ZipError::NoEndOfCentralDirectory);
    }
    let tail_len = len.min(EOCD_LEN + MAX_COMMENT_LEN);
    let tail_start = len - tail_len;
    let tail = read_at(archive, tail_start, tail_len)?;
    let record_at = (0..=tail.len() - EOCD_LEN as usize)
        .rev()
        .find(|&i| {
            u32_at(&tail, i) == Some(EOCD_SIGNATURE)
                && u16_at(&tail, i + 20).is_some_and(|comment| {
                    i + EOCD_LEN as usize + usize::from(comment) <= tail.len()
                })
        })
        .ok_or(ZipError::NoEndOfCentralDirectory)?;
    let record = &tail[record_at..];
    let field16 = |at| u16_at(record, at).ok_or(ZipError::NoEndOfCentralDirectory);
    let field32 = |at| u32_at(record, at).ok_or(ZipError::NoEndOfCentralDirectory);
    let disk = field16(4)?;
    let directory_disk = field16(6)?;
    let entries_on_disk = field16(8)?;
    let entries = field16(10)?;
    let directory_size = field32(12)?;
    let directory_offset = field32(16)?;
    if entries == ZIP64_MARK_16
        || directory_size == ZIP64_MARK_32
        || directory_offset == ZIP64_MARK_32
    {
        return Err(ZipError::Zip64);
    }
    if disk != 0 || directory_disk != 0 || entries_on_disk != entries {
        return Err(ZipError::MultiDisk);
    }
    let record_offset = tail_start + record_at as u64;
    let directory_size = u64::from(directory_size);
    let directory_offset = u64::from(directory_offset);
    if directory_offset + directory_size > record_offset {
        return Err(ZipError::CentralDirectory("it overlaps its end record"));
    }
    if directory_size > MAX_CENTRAL_DIRECTORY_BYTES {
        return Err(ZipError::CentralDirectory("it is too large"));
    }
    Ok(EndOfCentralDirectory {
        entries,
        directory_size,
        directory_offset,
    })
}

fn find_entry(
    directory: &[u8],
    entries: u16,
    extensions: &[String],
) -> Result<CentralEntry, ZipError> {
    let mut at = 0usize;
    for _ in 0..entries {
        let (entry, next) = parse_central_entry(directory, at)?;
        at = next;
        if !entry.name.ends_with('/') && has_extension(&entry.name, extensions) {
            return Ok(entry);
        }
    }
    Err(ZipError::NoMatchingEntry {
        extensions: extensions.to_vec(),
    })
}

fn parse_central_entry(directory: &[u8], at: usize) -> Result<(CentralEntry, usize), ZipError> {
    let damaged = || ZipError::CentralDirectory("an entry header does not fit");
    let header = directory
        .get(at..at.checked_add(CENTRAL_HEADER_LEN).ok_or_else(damaged)?)
        .ok_or_else(damaged)?;
    if u32_at(header, 0) != Some(CENTRAL_HEADER_SIGNATURE) {
        return Err(ZipError::CentralDirectory("an entry has no signature"));
    }
    let field16 = |offset| u16_at(header, offset).ok_or_else(damaged);
    let field32 = |offset| u32_at(header, offset).ok_or_else(damaged);
    let flags = field16(8)?;
    let name_len = usize::from(field16(28)?);
    let extra_len = usize::from(field16(30)?);
    let comment_len = usize::from(field16(32)?);
    let name_start = at + CENTRAL_HEADER_LEN;
    let name_bytes = directory
        .get(name_start..name_start + name_len)
        .ok_or_else(damaged)?;
    let next = name_start + name_len + extra_len + comment_len;
    if next > directory.len() {
        return Err(damaged());
    }
    let entry = CentralEntry {
        name: decode_name(name_bytes, flags),
        flags,
        method: field16(10)?,
        crc: field32(16)?,
        compressed_size: field32(20)?,
        uncompressed_size: field32(24)?,
        local_header_offset: field32(42)?,
    };
    Ok((entry, next))
}

/// Names are UTF-8 when flag bit 11 is set, and code page 437 otherwise.
/// ASCII is the same in both. Other bytes of a code page 437 name become
/// U+FFFD: the name only serves the extension check and the path given to
/// the core.
fn decode_name(bytes: &[u8], flags: u16) -> String {
    if flags & FLAG_UTF8 != 0 || bytes.is_ascii() {
        String::from_utf8_lossy(bytes).into_owned()
    } else {
        bytes
            .iter()
            .map(|&b| {
                if b.is_ascii() {
                    char::from(b)
                } else {
                    '\u{FFFD}'
                }
            })
            .collect()
    }
}

fn has_extension(name: &str, extensions: &[String]) -> bool {
    let file = name.rsplit('/').next().unwrap_or(name);
    file.rsplit_once('.').is_some_and(|(stem, ext)| {
        !stem.is_empty() && extensions.iter().any(|e| e.eq_ignore_ascii_case(ext))
    })
}

fn read_entry<R: Read + Seek>(
    archive: &mut R,
    entry: &CentralEntry,
    directory_offset: u64,
    limit_bytes: u64,
) -> Result<ZipEntry, ZipError> {
    let name = || entry.name.clone();
    if entry.flags & FLAG_ENCRYPTED != 0 {
        return Err(ZipError::Encrypted { name: name() });
    }
    if entry.method != METHOD_STORED && entry.method != METHOD_DEFLATE {
        return Err(ZipError::Method {
            name: name(),
            method: entry.method,
        });
    }
    if entry.compressed_size == ZIP64_MARK_32
        || entry.uncompressed_size == ZIP64_MARK_32
        || entry.local_header_offset == ZIP64_MARK_32
    {
        return Err(ZipError::Zip64);
    }
    let size = u64::from(entry.uncompressed_size);
    let compressed = u64::from(entry.compressed_size);
    if size > limit_bytes {
        return Err(ZipError::TooLarge {
            name: name(),
            size,
            limit: limit_bytes,
        });
    }
    let mismatch = |actual| ZipError::SizeMismatch {
        name: name(),
        expected: size,
        actual,
    };
    if entry.method == METHOD_STORED && compressed != size {
        return Err(mismatch(compressed));
    }
    if compressed > max_compressed_len(size) {
        return Err(ZipError::Inflate { name: name() });
    }
    let data_start = local_data_start(archive, entry)?;
    if data_start + compressed > directory_offset {
        return Err(ZipError::LocalHeader { name: name() });
    }
    let raw = read_at(archive, data_start, compressed)?;
    let data = if entry.method == METHOD_STORED {
        raw
    } else {
        let limit = usize::try_from(size).map_err(|_| mismatch(size))?;
        miniz_oxide::inflate::decompress_to_vec_with_limit(&raw, limit)
            .map_err(|_| ZipError::Inflate { name: name() })?
    };
    if data.len() as u64 != size {
        return Err(mismatch(data.len() as u64));
    }
    let actual = crc32(&data);
    if actual != entry.crc {
        return Err(ZipError::Crc {
            name: name(),
            expected: entry.crc,
            actual,
        });
    }
    Ok(ZipEntry { name: name(), data })
}

fn local_data_start<R: Read + Seek>(
    archive: &mut R,
    entry: &CentralEntry,
) -> Result<u64, ZipError> {
    let offset = u64::from(entry.local_header_offset);
    let damaged = || ZipError::LocalHeader {
        name: entry.name.clone(),
    };
    let header = read_at(archive, offset, LOCAL_HEADER_LEN).map_err(|e| match e {
        ZipError::Io(io) if io.kind() == io::ErrorKind::UnexpectedEof => damaged(),
        other => other,
    })?;
    if u32_at(&header, 0) != Some(LOCAL_HEADER_SIGNATURE) {
        return Err(damaged());
    }
    let name_len = u16_at(&header, 26).ok_or_else(damaged)?;
    let extra_len = u16_at(&header, 28).ok_or_else(damaged)?;
    Ok(offset + LOCAL_HEADER_LEN + u64::from(name_len) + u64::from(extra_len))
}

/// Reads `len` bytes at `offset`. `len` is bounded by the caller.
fn read_at<R: Read + Seek>(archive: &mut R, offset: u64, len: u64) -> Result<Vec<u8>, ZipError> {
    let len = usize::try_from(len)
        .map_err(|_| ZipError::Io(io::Error::from(io::ErrorKind::OutOfMemory)))?;
    archive
        .seek(SeekFrom::Start(offset))
        .map_err(ZipError::Io)?;
    let mut bytes = vec![0; len];
    archive.read_exact(&mut bytes).map_err(ZipError::Io)?;
    Ok(bytes)
}

fn u16_at(bytes: &[u8], at: usize) -> Option<u16> {
    let b = bytes.get(at..at.checked_add(2)?)?;
    Some(u16::from_le_bytes([b[0], b[1]]))
}

fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    let b = bytes.get(at..at.checked_add(4)?)?;
    Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::io::Cursor;

    /// One entry for [`build`].
    pub(crate) struct TestEntry<'a> {
        pub name: &'a str,
        pub data: &'a [u8],
        pub deflate: bool,
    }

    /// Builds a zip archive in memory, as an archiver writes one.
    pub(crate) fn build(entries: &[TestEntry<'_>]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut central = Vec::new();
        for e in entries {
            let offset = out.len() as u32;
            let (method, payload) = if e.deflate {
                (
                    METHOD_DEFLATE,
                    miniz_oxide::deflate::compress_to_vec(e.data, 6),
                )
            } else {
                (METHOD_STORED, e.data.to_vec())
            };
            let crc = crc32(e.data);
            let name = e.name.as_bytes();
            out.extend_from_slice(&LOCAL_HEADER_SIGNATURE.to_le_bytes());
            out.extend_from_slice(&[20, 0, 0, 0]);
            out.extend_from_slice(&method.to_le_bytes());
            out.extend_from_slice(&[0; 4]);
            out.extend_from_slice(&crc.to_le_bytes());
            out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
            out.extend_from_slice(&(e.data.len() as u32).to_le_bytes());
            out.extend_from_slice(&(name.len() as u16).to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes());
            out.extend_from_slice(name);
            out.extend_from_slice(&payload);

            central.extend_from_slice(&CENTRAL_HEADER_SIGNATURE.to_le_bytes());
            central.extend_from_slice(&[20, 0, 20, 0, 0, 0]);
            central.extend_from_slice(&method.to_le_bytes());
            central.extend_from_slice(&[0; 4]);
            central.extend_from_slice(&crc.to_le_bytes());
            central.extend_from_slice(&(payload.len() as u32).to_le_bytes());
            central.extend_from_slice(&(e.data.len() as u32).to_le_bytes());
            central.extend_from_slice(&(name.len() as u16).to_le_bytes());
            central.extend_from_slice(&[0; 12]);
            central.extend_from_slice(&offset.to_le_bytes());
            central.extend_from_slice(name);
        }
        let directory_offset = out.len() as u32;
        out.extend_from_slice(&central);
        out.extend_from_slice(&EOCD_SIGNATURE.to_le_bytes());
        out.extend_from_slice(&[0; 4]);
        out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
        out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
        out.extend_from_slice(&(central.len() as u32).to_le_bytes());
        out.extend_from_slice(&directory_offset.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out
    }

    fn exts(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_owned()).collect()
    }

    fn read(bytes: &[u8], list: &[&str], limit: u64) -> Result<ZipEntry, ZipError> {
        read_zip_entry(&mut Cursor::new(bytes), &exts(list), limit)
    }

    const ROM: &[u8] =
        b"NES\x1a not a real rom, but a rom all the same, repeated repeated repeated";

    fn one(name: &str, deflate: bool) -> Vec<u8> {
        build(&[TestEntry {
            name,
            data: ROM,
            deflate,
        }])
    }

    /// Offset of the central directory in an archive from [`build`].
    fn directory_offset(zip: &[u8]) -> usize {
        u32_at(zip, zip.len() - 6).unwrap() as usize
    }

    #[test]
    fn a_stored_entry_is_read() {
        let entry = read(&one("game.nes", false), &["nes"], 1024).unwrap();
        assert_eq!(entry.name, "game.nes");
        assert_eq!(entry.data, ROM);
    }

    #[test]
    fn a_deflated_entry_is_read() {
        let entry = read(&one("dir/Game.NES", true), &["nes"], 1024).unwrap();
        assert_eq!(entry.name, "dir/Game.NES");
        assert_eq!(entry.data, ROM);
    }

    #[test]
    fn the_first_matching_entry_wins() {
        let zip = build(&[
            TestEntry {
                name: "readme.txt",
                data: b"text",
                deflate: false,
            },
            TestEntry {
                name: "roms.nes/",
                data: b"",
                deflate: false,
            },
            TestEntry {
                name: "b.sfc",
                data: b"second",
                deflate: true,
            },
            TestEntry {
                name: "a.smc",
                data: b"third",
                deflate: false,
            },
        ]);
        let entry = read(&zip, &["smc", "sfc"], 1024).unwrap();
        assert_eq!(entry.name, "b.sfc");
        assert_eq!(entry.data, b"second");
    }

    #[test]
    fn no_matching_entry_is_an_error() {
        let zip = one("game.nes", false);
        assert!(matches!(
            read(&zip, &["sfc"], 1024),
            Err(ZipError::NoMatchingEntry { extensions }) if extensions == ["sfc"]
        ));
        assert!(matches!(
            read(&one(".nes", false), &["nes"], 1024),
            Err(ZipError::NoMatchingEntry { .. })
        ));
    }

    #[test]
    fn an_entry_above_the_limit_is_an_error() {
        let limit = ROM.len() as u64 - 1;
        assert!(matches!(
            read(&one("game.nes", true), &["nes"], limit),
            Err(ZipError::TooLarge { size, limit: l, .. }) if size == ROM.len() as u64 && l == limit
        ));
        assert!(read(&one("game.nes", true), &["nes"], ROM.len() as u64).is_ok());
    }

    #[test]
    fn a_file_that_is_not_a_zip_is_an_error() {
        assert!(matches!(
            read(b"short", &["nes"], 1024),
            Err(ZipError::NoEndOfCentralDirectory)
        ));
        assert!(matches!(
            read(&[0u8; 4096], &["nes"], 1024),
            Err(ZipError::NoEndOfCentralDirectory)
        ));
    }

    #[test]
    fn a_wrong_crc_is_an_error() {
        let mut zip = one("game.nes", false);
        let data_at = 30 + "game.nes".len();
        zip[data_at] ^= 0xFF;
        assert!(matches!(
            read(&zip, &["nes"], 1024),
            Err(ZipError::Crc { .. })
        ));
    }

    #[test]
    fn damaged_deflate_data_is_an_error() {
        let mut zip = one("game.nes", true);
        let data_at = 30 + "game.nes".len();
        for b in &mut zip[data_at..data_at + 8] {
            *b = 0xFF;
        }
        assert!(matches!(
            read(&zip, &["nes"], 1024),
            Err(ZipError::Inflate { .. } | ZipError::SizeMismatch { .. } | ZipError::Crc { .. })
        ));
    }

    #[test]
    fn a_wrong_uncompressed_size_is_an_error() {
        let mut zip = one("game.nes", true);
        let cd = directory_offset(&zip);
        let smaller = (ROM.len() as u32 - 1).to_le_bytes();
        zip[cd + 24..cd + 28].copy_from_slice(&smaller);
        assert!(matches!(
            read(&zip, &["nes"], 1024),
            Err(ZipError::Inflate { .. })
        ));
        let mut zip = one("game.nes", false);
        let cd = directory_offset(&zip);
        zip[cd + 24..cd + 28].copy_from_slice(&smaller);
        assert!(matches!(
            read(&zip, &["nes"], 1024),
            Err(ZipError::SizeMismatch { .. })
        ));
    }

    #[test]
    fn encrypted_entries_and_other_methods_are_errors() {
        let mut zip = one("game.nes", false);
        let cd = directory_offset(&zip);
        zip[cd + 8] |= 1;
        assert!(matches!(
            read(&zip, &["nes"], 1024),
            Err(ZipError::Encrypted { .. })
        ));
        let mut zip = one("game.nes", false);
        zip[cd + 10] = 14;
        assert!(matches!(
            read(&zip, &["nes"], 1024),
            Err(ZipError::Method { method: 14, .. })
        ));
    }

    #[test]
    fn zip64_and_multi_disk_archives_are_errors() {
        let base = one("game.nes", false);
        let eocd = base.len() - 22;
        let mut zip = base.clone();
        zip[eocd + 16..eocd + 20].copy_from_slice(&ZIP64_MARK_32.to_le_bytes());
        assert!(matches!(read(&zip, &["nes"], 1024), Err(ZipError::Zip64)));
        let mut zip = base.clone();
        zip[eocd + 4] = 1;
        assert!(matches!(
            read(&zip, &["nes"], 1024),
            Err(ZipError::MultiDisk)
        ));
        let mut zip = base.clone();
        let cd = directory_offset(&zip);
        zip[cd + 20..cd + 24].copy_from_slice(&ZIP64_MARK_32.to_le_bytes());
        assert!(matches!(read(&zip, &["nes"], 1024), Err(ZipError::Zip64)));
    }

    #[test]
    fn a_damaged_directory_or_header_is_an_error() {
        let base = one("game.nes", false);
        let eocd = base.len() - 22;
        let cd = directory_offset(&base);

        let mut zip = base.clone();
        zip[eocd + 16..eocd + 20].copy_from_slice(&(cd as u32 + 1000).to_le_bytes());
        assert!(matches!(
            read(&zip, &["nes"], 1024),
            Err(ZipError::CentralDirectory(_))
        ));

        let mut zip = base.clone();
        zip[cd] = 0;
        assert!(matches!(
            read(&zip, &["nes"], 1024),
            Err(ZipError::CentralDirectory(_))
        ));

        let mut zip = base.clone();
        zip[cd + 28] = 0xFF;
        assert!(matches!(
            read(&zip, &["nes"], 1024),
            Err(ZipError::CentralDirectory(_))
        ));

        let mut zip = base.clone();
        zip[0] = 0;
        assert!(matches!(
            read(&zip, &["nes"], 1024),
            Err(ZipError::LocalHeader { .. })
        ));

        let mut zip = base.clone();
        zip[26] = 0xFF;
        assert!(matches!(
            read(&zip, &["nes"], 1024),
            Err(ZipError::LocalHeader { .. })
        ));
    }

    #[test]
    fn an_archive_comment_is_skipped() {
        let mut zip = one("game.nes", false);
        let len = zip.len();
        zip[len - 2..].copy_from_slice(&5u16.to_le_bytes());
        zip.extend_from_slice(b"hello");
        assert_eq!(read(&zip, &["nes"], 1024).unwrap().data, ROM);
    }

    #[test]
    fn a_code_page_437_name_keeps_its_ascii() {
        assert_eq!(decode_name(b"caf\x82.nes", 0), "caf\u{FFFD}.nes");
        assert_eq!(decode_name("café.nes".as_bytes(), FLAG_UTF8), "café.nes");
    }
}
