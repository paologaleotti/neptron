//! The byte layout of the cache file, all integers little-endian:
//!
//! | Offset | Size | Field |
//! | --- | --- | --- |
//! | 0 | 8 | magic `NEPTRIDX` |
//! | 8 | 4 | format version |
//! | 12 | 8 | payload length in bytes |
//! | 20 | 4 | CRC-32 of the payload |
//! | 24 | n | payload |
//!
//! Payload of version 2: a `u32` system count, then for each system its id
//! (a string) and a `u32` game count, then for each game its name and its
//! path (two strings) and its size (`u64`). Then a `u32` folder count, and
//! for each folder its path (a string), a trust byte (1: the time is
//! trusted, 0: it is not and the time fields are 0), the seconds (`i64`)
//! and the nanoseconds (`u32`) of its time. The folders are sorted by path,
//! each path once. A string is a `u32` byte length and UTF-8 bytes.
//!
//! Version 1 had no sizes and no folders. A file of version 1 is discarded
//! and the scan writes version 2.

use super::CacheError;
use super::crc32::crc32;
use crate::catalog::{Catalog, SystemDef};
use crate::folders::{FolderStamp, Mtime};
use crate::index::{Game, Index, MAX_TEXT_BYTES, SystemGames};

/// The first 8 bytes of a cache file.
pub const MAGIC: [u8; 8] = *b"NEPTRIDX";

/// The format version that this crate writes and reads.
pub const CACHE_VERSION: u32 = 2;

/// The size of the header in bytes.
pub const HEADER_BYTES: usize = 24;

/// The bytes of the smallest valid game: two length fields, a 1-byte name,
/// a 3-byte path such as `s/n` and the size.
const MIN_GAME_BYTES: usize = 4 + 1 + 4 + 3 + 8;

/// The bytes of the smallest valid folder: a length field, a 1-byte path,
/// the trust byte and the time.
const MIN_FOLDER_BYTES: usize = 4 + 1 + 1 + 8 + 4;

/// Nanoseconds in a second: the nanoseconds of a time are below it.
const NANOS_PER_SECOND: u32 = 1_000_000_000;

pub(super) fn encode(index: &Index) -> Vec<u8> {
    let mut payload = Vec::new();
    put_count(&mut payload, index.systems().len());
    for system in index.systems() {
        put_str(&mut payload, system.system());
        put_count(&mut payload, system.len());
        for game in system.games() {
            put_str(&mut payload, game.name());
            put_str(&mut payload, game.path());
            payload.extend_from_slice(&game.size().to_le_bytes());
        }
    }
    put_count(&mut payload, index.folders().len());
    for folder in index.folders() {
        put_str(&mut payload, &folder.path);
        let (trust, time) = match folder.mtime {
            Some(t) => (1u8, t),
            None => (0u8, Mtime { secs: 0, nanos: 0 }),
        };
        payload.push(trust);
        payload.extend_from_slice(&time.secs.to_le_bytes());
        payload.extend_from_slice(&time.nanos.to_le_bytes());
    }
    let mut out = Vec::with_capacity(HEADER_BYTES + payload.len());
    out.extend_from_slice(&MAGIC);
    out.extend_from_slice(&CACHE_VERSION.to_le_bytes());
    out.extend_from_slice(&(payload.len() as u64).to_le_bytes());
    out.extend_from_slice(&crc32(&payload).to_le_bytes());
    out.extend_from_slice(&payload);
    out
}

/// Every loop and allocation here is bounded by the file: the system count
/// by the catalog, each game count by the bytes left, each string by
/// [`MAX_TEXT_BYTES`] and the bytes left. Each system is looked up once in
/// the catalog, so the work is linear in the size of the file.
pub(super) fn decode(bytes: &[u8], catalog: &Catalog) -> Result<Index, CacheError> {
    let payload = check_header(bytes)?;
    let mut r = Reader {
        bytes: payload,
        at: 0,
    };
    let known = catalog.systems();
    let system_count = r.u32("system count")? as usize;
    if system_count > known.len() {
        return Err(r.malformed("system count"));
    }
    let mut seen = vec![false; known.len()];
    let mut systems: Vec<SystemGames> = Vec::with_capacity(system_count);
    for _ in 0..system_count {
        let id = r.string("system id")?;
        let Some((system, flag)) = known
            .iter()
            .zip(seen.iter_mut())
            .find(|(s, _)| s.folder == id)
        else {
            return Err(CacheError::UnknownSystem { system: id });
        };
        if *flag {
            return Err(r.malformed("system id"));
        }
        *flag = true;
        let game_count = r.count(MIN_GAME_BYTES, "game count")?;
        let mut games = Vec::with_capacity(game_count);
        for _ in 0..game_count {
            let name = r.string("game name")?;
            if name.is_empty() {
                return Err(r.malformed("game name"));
            }
            let path = r.string("game path")?;
            if !path_is_inside(&path, system) {
                return Err(r.malformed("game path"));
            }
            let size = r.u64("game size")?;
            games.push(Game::new(name, path).with_size(size));
        }
        systems.push(SystemGames::new(id, games));
    }
    let folders = read_folders(&mut r, catalog)?;
    if r.at != payload.len() {
        return Err(r.malformed("trailing bytes"));
    }
    Ok(Index::from_systems(systems).with_folders(folders))
}

/// The folder stamps. Each path must be a system folder of `catalog`, or a
/// folder inside one, and the paths must rise strictly.
fn read_folders(r: &mut Reader<'_>, catalog: &Catalog) -> Result<Vec<FolderStamp>, CacheError> {
    let count = r.count(MIN_FOLDER_BYTES, "folder count")?;
    let mut folders: Vec<FolderStamp> = Vec::with_capacity(count);
    for _ in 0..count {
        let path = r.string("folder path")?;
        let inside = catalog.systems().iter().any(|s| folder_is_inside(&path, s));
        let rises = folders.last().is_none_or(|f| f.path < path);
        if !inside || !rises {
            return Err(r.malformed("folder path"));
        }
        let trust = r.u8("folder trust")?;
        let secs = r.i64("folder time")?;
        let nanos = r.u32("folder time")?;
        let mtime = match trust {
            1 if nanos < NANOS_PER_SECOND => Some(Mtime { secs, nanos }),
            0 if secs == 0 && nanos == 0 => None,
            _ => return Err(r.malformed("folder time")),
        };
        folders.push(FolderStamp { path, mtime });
    }
    Ok(folders)
}

/// True when `path` is a folder name or alias of `system`, alone or
/// followed by subfolders, with no empty, `.` or `..` part and no NUL byte.
fn folder_is_inside(path: &str, system: &SystemDef) -> bool {
    let mut parts = path.split('/');
    let folder_ok = parts
        .next()
        .is_some_and(|f| system.folder_names().any(|n| n.eq_ignore_ascii_case(f)));
    folder_ok && parts.all(|p| !p.is_empty() && p != "." && p != ".." && !p.contains('\0'))
}

/// True when `path` is relative, starts with a folder name or alias of
/// `system`, and has a file part, with no empty, `.` or `..` part and no
/// NUL byte. Joined to the ROM folder, such a path stays inside it.
fn path_is_inside(path: &str, system: &SystemDef) -> bool {
    let mut parts = path.split('/');
    let folder_ok = parts
        .next()
        .is_some_and(|f| system.folder_names().any(|n| n.eq_ignore_ascii_case(f)));
    let mut rest = parts.peekable();
    folder_ok
        && rest.peek().is_some()
        && rest.all(|p| !p.is_empty() && p != "." && p != ".." && !p.contains('\0'))
}

fn check_header(bytes: &[u8]) -> Result<&[u8], CacheError> {
    let (header, payload) =
        bytes
            .split_first_chunk::<HEADER_BYTES>()
            .ok_or(CacheError::Truncated {
                len: bytes.len(),
                header: HEADER_BYTES,
            })?;
    if field::<8>(header, 0) != MAGIC {
        return Err(CacheError::BadMagic);
    }
    let version = u32::from_le_bytes(field(header, 8));
    if version != CACHE_VERSION {
        return Err(CacheError::Version {
            found: version,
            expected: CACHE_VERSION,
        });
    }
    let declared = u64::from_le_bytes(field(header, 12));
    if declared != payload.len() as u64 {
        return Err(CacheError::Length {
            declared,
            actual: payload.len() as u64,
        });
    }
    let stored = u32::from_le_bytes(field(header, 20));
    let computed = crc32(payload);
    if stored != computed {
        return Err(CacheError::Checksum { stored, computed });
    }
    Ok(payload)
}

/// The `N` bytes of the header at `at`. Every caller passes a constant
/// range inside the header, so the range is always there.
fn field<const N: usize>(header: &[u8; HEADER_BYTES], at: usize) -> [u8; N] {
    let mut out = [0u8; N];
    if let Some(src) = header.get(at..at + N) {
        out.copy_from_slice(src);
    }
    out
}

/// No index holds more than `u32::MAX` items. If one did, the saturated
/// count would fail the decode and the cache would be rebuilt.
fn put_count(out: &mut Vec<u8>, n: usize) {
    out.extend_from_slice(&u32::try_from(n).unwrap_or(u32::MAX).to_le_bytes());
}

fn put_str(out: &mut Vec<u8>, s: &str) {
    put_count(out, s.len());
    out.extend_from_slice(s.as_bytes());
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn bytes<const N: usize>(&mut self, what: &'static str) -> Result<[u8; N], CacheError> {
        let raw = self.take(N, what)?;
        let mut out = [0u8; N];
        out.copy_from_slice(raw);
        Ok(out)
    }

    fn u8(&mut self, what: &'static str) -> Result<u8, CacheError> {
        let [byte] = self.bytes(what)?;
        Ok(byte)
    }

    fn u32(&mut self, what: &'static str) -> Result<u32, CacheError> {
        self.bytes(what).map(u32::from_le_bytes)
    }

    fn u64(&mut self, what: &'static str) -> Result<u64, CacheError> {
        self.bytes(what).map(u64::from_le_bytes)
    }

    fn i64(&mut self, what: &'static str) -> Result<i64, CacheError> {
        self.bytes(what).map(i64::from_le_bytes)
    }

    /// A count of items that each take at least `min_bytes`. A count that
    /// the rest of the payload cannot hold is rejected before any
    /// allocation, so a buffer made for the count is at most the bytes left
    /// divided by `min_bytes` items.
    fn count(&mut self, min_bytes: usize, what: &'static str) -> Result<usize, CacheError> {
        let n = self.u32(what)? as usize;
        let left = self.bytes.len() - self.at;
        if n.saturating_mul(min_bytes) > left {
            return Err(self.malformed(what));
        }
        Ok(n)
    }

    fn string(&mut self, what: &'static str) -> Result<String, CacheError> {
        let len = self.u32(what)? as usize;
        if len > MAX_TEXT_BYTES {
            return Err(self.malformed(what));
        }
        let raw = self.take(len, what)?;
        let text = std::str::from_utf8(raw).map_err(|_| self.malformed(what))?;
        Ok(text.to_owned())
    }

    fn take(&mut self, n: usize, what: &'static str) -> Result<&'a [u8], CacheError> {
        let bytes: &'a [u8] = self.bytes;
        let out = self
            .at
            .checked_add(n)
            .and_then(|end| bytes.get(self.at..end))
            .ok_or_else(|| self.malformed(what))?;
        self.at += n;
        Ok(out)
    }

    fn malformed(&self, what: &'static str) -> CacheError {
        CacheError::Malformed {
            offset: HEADER_BYTES + self.at,
            what,
        }
    }
}
