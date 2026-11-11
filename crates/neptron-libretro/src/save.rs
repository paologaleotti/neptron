//! Save RAM files: the `.srm` name, the read, and the atomic write with one
//! backup. These functions do file I/O, so the frame thread never calls
//! them.

use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

/// The largest save RAM that the host reads from a file or from a core, in
/// bytes.
pub const MAX_SAVE_RAM_BYTES: usize = 16 * 1024 * 1024;

/// Why a save file could not be read or written.
#[derive(Debug, thiserror::Error)]
pub enum SaveFileError {
    /// Reading a file failed.
    #[error("cannot read {path}: {source}")]
    Read {
        /// The file.
        path: PathBuf,
        /// The cause.
        #[source]
        source: io::Error,
    },
    /// Writing, syncing or renaming a file failed.
    #[error("cannot write {path}: {source}")]
    Write {
        /// The file.
        path: PathBuf,
        /// The cause.
        #[source]
        source: io::Error,
    },
    /// The file is larger than [`MAX_SAVE_RAM_BYTES`].
    #[error("{path} has {len} bytes, more than the limit of {limit} bytes")]
    TooLarge {
        /// The file.
        path: PathBuf,
        /// Its length.
        len: u64,
        /// The limit.
        limit: usize,
    },
    /// The path has no file name.
    #[error("{path} has no file name")]
    NoFileName {
        /// The path.
        path: PathBuf,
    },
}

/// The save file of `content`: `<save_dir>/<content file stem>.srm`, as
/// RetroArch names it. For a zip file it is the stem of the archive. `None`
/// when the content path has no file stem.
pub fn save_file_path(save_dir: &Path, content: &Path) -> Option<PathBuf> {
    let mut name = content.file_stem()?.to_os_string();
    name.push(".srm");
    Some(save_dir.join(name))
}

/// Reads a save file. A missing file is `Ok(None)`.
pub fn read_save_file(path: &Path) -> Result<Option<Vec<u8>>, SaveFileError> {
    match read_bounded(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(SaveFileError::Read { source, .. }) if source.kind() == io::ErrorKind::NotFound => {
            Ok(None)
        }
        Err(e) => Err(e),
    }
}

/// Writes a save file so that a crash leaves the old file or the new file,
/// never a broken one, and keeps the old file as `<name>.bak`.
///
/// 1. Write `<name>.tmp` and sync it.
/// 2. Copy the old file, if there is one, to `<name>.bak.tmp`, sync it,
///    and rename it to `<name>.bak`.
/// 3. Rename `<name>.tmp` to `<name>` and sync the folder.
///
/// Each temporary file is removed first and made with `create_new`, so a
/// symlink left at its path is never followed.
pub fn write_save_file(path: &Path, bytes: &[u8]) -> Result<(), SaveFileError> {
    let tmp = with_suffix(path, ".tmp")?;
    let bak = with_suffix(path, ".bak")?;
    let bak_tmp = with_suffix(path, ".bak.tmp")?;
    write_new(&tmp, bytes)?;
    match read_bounded(path) {
        Ok(old) => {
            write_new(&bak_tmp, &old)?;
            rename(&bak_tmp, &bak)?;
        }
        Err(SaveFileError::Read { source, .. }) if source.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }
    rename(&tmp, path)?;
    sync_folder(path)
}

fn read_bounded(path: &Path) -> Result<Vec<u8>, SaveFileError> {
    let read_error = |source| SaveFileError::Read {
        path: path.to_owned(),
        source,
    };
    let file = File::open(path).map_err(read_error)?;
    let len = file.metadata().map_err(read_error)?.len();
    let too_large = |len| SaveFileError::TooLarge {
        path: path.to_owned(),
        len,
        limit: MAX_SAVE_RAM_BYTES,
    };
    if len > MAX_SAVE_RAM_BYTES as u64 {
        return Err(too_large(len));
    }
    let mut bytes = Vec::new();
    // The file can grow after the length check; the bound still holds.
    file.take(MAX_SAVE_RAM_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(read_error)?;
    if bytes.len() > MAX_SAVE_RAM_BYTES {
        return Err(too_large(bytes.len() as u64));
    }
    Ok(bytes)
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<(), SaveFileError> {
    let write_error = |source| SaveFileError::Write {
        path: path.to_owned(),
        source,
    };
    match fs::remove_file(path) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(write_error(e)),
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(write_error)?;
    file.write_all(bytes).map_err(write_error)?;
    file.sync_all().map_err(write_error)
}

fn rename(from: &Path, to: &Path) -> Result<(), SaveFileError> {
    fs::rename(from, to).map_err(|source| SaveFileError::Write {
        path: to.to_owned(),
        source,
    })
}

fn sync_folder(path: &Path) -> Result<(), SaveFileError> {
    let folder = match path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p,
        _ => Path::new("."),
    };
    File::open(folder)
        .and_then(|f| f.sync_all())
        .map_err(|source| SaveFileError::Write {
            path: folder.to_owned(),
            source,
        })
}

fn with_suffix(path: &Path, suffix: &str) -> Result<PathBuf, SaveFileError> {
    let name = path.file_name().ok_or_else(|| SaveFileError::NoFileName {
        path: path.to_owned(),
    })?;
    let mut name = OsString::from(name);
    name.push(suffix);
    Ok(path.with_file_name(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "neptron-libretro-save-{}-{name}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir); // A leftover of an earlier run; absence is fine.
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn the_save_name_follows_the_content_stem() {
        let dir = Path::new("/state/saves/nes");
        assert_eq!(
            save_file_path(dir, Path::new("/share/roms/nes/Mario (USA).nes")),
            Some(dir.join("Mario (USA).srm"))
        );
        assert_eq!(
            save_file_path(dir, Path::new("/share/roms/nes/Zelda.zip")),
            Some(dir.join("Zelda.srm"))
        );
        assert_eq!(save_file_path(dir, Path::new("/")), None);
    }

    #[test]
    fn a_missing_file_reads_as_none() {
        let dir = temp_dir("missing");
        assert_eq!(read_save_file(&dir.join("none.srm")).unwrap(), None);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_write_round_trips_and_keeps_one_backup() {
        let dir = temp_dir("round-trip");
        let path = dir.join("game.srm");
        write_save_file(&path, b"first").unwrap();
        assert_eq!(
            read_save_file(&path).unwrap().as_deref(),
            Some(&b"first"[..])
        );
        assert!(!dir.join("game.srm.bak").exists(), "nothing to back up");

        write_save_file(&path, b"second").unwrap();
        write_save_file(&path, b"third").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"third");
        assert_eq!(fs::read(dir.join("game.srm.bak")).unwrap(), b"second");
        let mut names: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        assert_eq!(
            names,
            ["game.srm", "game.srm.bak"],
            "no temporary file stays"
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_stale_temp_file_is_replaced() {
        let dir = temp_dir("stale");
        let path = dir.join("game.srm");
        fs::write(dir.join("game.srm.tmp"), b"half a write from a crash").unwrap();
        write_save_file(&path, b"new").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"new");
        assert!(!dir.join("game.srm.tmp").exists());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_symlink_at_the_temp_path_is_not_followed() {
        let dir = temp_dir("symlink");
        let path = dir.join("game.srm");
        let victim = dir.join("victim");
        fs::write(&victim, b"keep me").unwrap();
        std::os::unix::fs::symlink(&victim, dir.join("game.srm.tmp")).unwrap();
        std::os::unix::fs::symlink(&victim, dir.join("game.srm.bak.tmp")).unwrap();
        write_save_file(&path, b"one").unwrap();
        write_save_file(&path, b"two").unwrap();
        assert_eq!(fs::read(&victim).unwrap(), b"keep me");
        assert_eq!(fs::read(&path).unwrap(), b"two");
        assert_eq!(fs::read(dir.join("game.srm.bak")).unwrap(), b"one");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_file_above_the_limit_is_an_error() {
        let dir = temp_dir("large");
        let path = dir.join("big.srm");
        let file = File::create(&path).unwrap();
        file.set_len(MAX_SAVE_RAM_BYTES as u64 + 1).unwrap();
        assert!(matches!(
            read_save_file(&path),
            Err(SaveFileError::TooLarge { len, .. }) if len == MAX_SAVE_RAM_BYTES as u64 + 1
        ));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_path_without_a_file_name_is_an_error() {
        assert!(matches!(
            write_save_file(Path::new("/"), b"x"),
            Err(SaveFileError::NoFileName { .. })
        ));
    }
}
