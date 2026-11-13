use std::fmt;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// The permission bits of a new settings file, before the umask. The files are shared on
/// the LAN and hold no secrets, so others may read them.
pub const NEW_FILE_MODE: u32 = 0o644;

use crate::edit::{Change, EditError, edit, holds_nothing};
use crate::layer::Layer;
use crate::parse::{Issue, IssueKind, Parsed, parse_layer};
use crate::schema::{Level, Schema};

/// The settings folder: `global.toml`, `systems/<system>.toml` and
/// `games/<system>/<game>.toml` under one root directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Storage {
    root: PathBuf,
}

/// The file of one level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target<'a> {
    /// `global.toml`.
    Global,
    /// `systems/<system>.toml`.
    System {
        /// The system folder name, such as `nes`.
        system: &'a str,
    },
    /// `games/<system>/<game>.toml`.
    Game {
        /// The system folder name, such as `nes`.
        system: &'a str,
        /// The game name: the ROM file name without its extension.
        game: &'a str,
    },
}

/// One settings file after a load.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Loaded {
    /// The path of the file.
    pub path: PathBuf,
    /// The valid values and the report. A missing file gives an empty layer and no issue.
    pub parsed: Parsed,
}

/// Why a settings file cannot be loaded or written.
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    /// A system or game name cannot be one file name.
    #[error("{what} name \"{name}\" cannot be used as a file name")]
    BadName {
        /// `system` or `game`.
        what: &'static str,
        /// The name.
        name: String,
    },
    /// The file exists but cannot be read, so a write could lose it.
    #[error("cannot read {}: {source}", path.display())]
    Read {
        /// The path.
        path: PathBuf,
        /// The error from the system.
        source: io::Error,
    },
    /// The file is not UTF-8 text, so a write could lose it.
    #[error("{} is not UTF-8 text; fix it by hand first", path.display())]
    NotUtf8 {
        /// The path.
        path: PathBuf,
    },
    /// The edit was refused. The file is unchanged.
    #[error("cannot change {}: {source}", path.display())]
    Edit {
        /// The path.
        path: PathBuf,
        /// Why the edit was refused.
        source: EditError,
    },
    /// One step of the atomic write failed. The file holds its old content or, when only
    /// the directory sync failed, its new content.
    #[error("cannot write {}: {step} failed: {source}", path.display())]
    Write {
        /// The path of the settings file.
        path: PathBuf,
        /// The step that failed.
        step: WriteStep,
        /// The error from the system.
        source: io::Error,
    },
}

/// One step of the atomic write.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteStep {
    /// Making a missing parent directory.
    CreateDir,
    /// Writing the temporary file.
    WriteTemp,
    /// Flushing the temporary file to the disk.
    SyncTemp,
    /// Renaming the temporary file over the settings file.
    Rename,
    /// Flushing the directory entry to the disk.
    SyncDir,
    /// Removing a settings file that holds no value and no comment.
    Remove,
}

impl fmt::Display for WriteStep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            WriteStep::CreateDir => "creating the directory",
            WriteStep::WriteTemp => "writing the temporary file",
            WriteStep::SyncTemp => "syncing the temporary file",
            WriteStep::Rename => "renaming the temporary file",
            WriteStep::SyncDir => "syncing the directory",
            WriteStep::Remove => "removing the file",
        })
    }
}

impl Target<'_> {
    /// The level of this file.
    pub fn level(&self) -> Level {
        match self {
            Target::Global => Level::Global,
            Target::System { .. } => Level::System,
            Target::Game { .. } => Level::Game,
        }
    }
}

impl Storage {
    /// Storage under `root`, normally `config/` on the state partition. The root does not
    /// need to exist: a missing file reads as empty, and a write makes the directories.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Storage { root: root.into() }
    }

    /// The root directory.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The path of the file for `target`.
    pub fn path(&self, target: Target<'_>) -> Result<PathBuf, StorageError> {
        Ok(match target {
            Target::Global => self.root.join("global.toml"),
            Target::System { system } => self
                .root
                .join("systems")
                .join(toml_name(file_name("system", system)?)),
            Target::Game { system, game } => self
                .root
                .join("games")
                .join(file_name("system", system)?)
                .join(toml_name(file_name("game", game)?)),
        })
    }

    /// Loads the file for `target`. Only a bad system or game name is an error: a file that
    /// cannot be read or parsed gives an empty layer and an issue, so it never stops the boot.
    pub fn load(&self, schema: &Schema, target: Target<'_>) -> Result<Loaded, StorageError> {
        let path = self.path(target)?;
        let level = target.level();
        let parsed = match fs::read(&path) {
            Ok(bytes) => match String::from_utf8(bytes) {
                Ok(text) => parse_layer(schema, level, &text),
                Err(_) => whole_file_issue(level, IssueKind::NotUtf8),
            },
            Err(err) if err.kind() == io::ErrorKind::NotFound => parse_layer(schema, level, ""),
            Err(err) => whole_file_issue(
                level,
                IssueKind::Unreadable {
                    message: err.to_string(),
                },
            ),
        };
        Ok(Loaded { path, parsed })
    }

    /// Applies `changes` to the file for `target`. The file is read again first, so a hand
    /// edit made since the last load is kept. The write is atomic: after a crash the file
    /// holds its old content or its new content. A file that cannot be read or parsed is
    /// never written. When the edit leaves no value and no comment, the file is removed
    /// instead, so no file holds only the version or an empty table.
    pub fn apply(
        &self,
        schema: &Schema,
        target: Target<'_>,
        changes: &[Change<'_>],
    ) -> Result<(), StorageError> {
        let path = self.path(target)?;
        let text = match fs::read(&path) {
            Ok(bytes) => String::from_utf8(bytes)
                .map_err(|_| StorageError::NotUtf8 { path: path.clone() })?,
            Err(err) if err.kind() == io::ErrorKind::NotFound => String::new(),
            Err(source) => return Err(StorageError::Read { path, source }),
        };
        let new = match edit(schema, target.level(), &text, changes) {
            Ok(new) => new,
            Err(source) => return Err(StorageError::Edit { path, source }),
        };
        let done = if holds_nothing(&new) {
            remove_durably(&path)
        } else {
            write_atomic(&path, new.as_bytes())
        };
        done.map_err(|(step, source)| StorageError::Write { path, step, source })
    }
}

/// Checks that `name` is one plain file name: not empty, no path separator, no NUL, and no
/// leading dot, so it cannot climb out of its folder or hide.
fn file_name<'n>(what: &'static str, name: &'n str) -> Result<&'n str, StorageError> {
    let bad = name.is_empty() || name.starts_with('.') || name.contains(['/', '\\', '\0']);
    if bad {
        Err(StorageError::BadName {
            what,
            name: name.to_owned(),
        })
    } else {
        Ok(name)
    }
}

fn toml_name(name: &str) -> String {
    format!("{name}.toml")
}

fn whole_file_issue(level: Level, kind: IssueKind) -> Parsed {
    Parsed {
        layer: Layer::new(level),
        issues: vec![Issue {
            line: None,
            key: None,
            kind,
        }],
        version: None,
    }
}

/// Writes `bytes` to `path` so that a crash at any point leaves the old file or the new
/// file, never a mix.
///
/// The sequence: make missing parent directories and sync each new entry in its parent;
/// remove a stale `<name>.tmp`; create `<name>.tmp` new, so that an entry left there of any
/// kind, a symlink included, makes the write fail instead of being written through; write
/// it with the permission bits of the old file, or [`NEW_FILE_MODE`] for a new file; sync
/// it; rename it over the file; sync the directory.
///
/// The rename is atomic on one file system. It replaces the directory entry, so when the
/// settings file is a symlink, the link becomes a regular file and its target is not
/// written. The directory sync makes the rename itself durable. `File::sync_all` is `fsync`
/// on Linux and `fcntl(F_FULLFSYNC)` on macOS, where a plain `fsync` does not flush the
/// drive cache. Both systems accept it on a directory opened read-only.
fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), (WriteStep, io::Error)> {
    let dir = parent_dir(path);
    create_dirs(dir).map_err(|e| (WriteStep::CreateDir, e))?;
    let mut tmp_name = path.file_name().unwrap_or_default().to_os_string();
    tmp_name.push(".tmp");
    let tmp = dir.join(tmp_name);
    remove_stale(&tmp).map_err(|e| (WriteStep::WriteTemp, e))?;
    let old_permissions = fs::metadata(path).ok().map(|m| m.permissions());
    let moved = write_and_sync(&tmp, bytes, old_permissions)
        .and_then(|()| fs::rename(&tmp, path).map_err(|e| (WriteStep::Rename, e)));
    if let Err(err) = moved {
        // The temporary file is only a copy of the new content. Failing to remove it
        // loses nothing, and the next write removes it first.
        let _ = fs::remove_file(&tmp);
        return Err(err);
    }
    sync_dir(dir).map_err(|e| (WriteStep::SyncDir, e))
}

/// Removes the settings file and syncs its directory, so the removal survives a crash. A
/// file that is not there needs nothing, and no directory is made for it.
fn remove_durably(path: &Path) -> Result<(), (WriteStep, io::Error)> {
    match fs::remove_file(path) {
        Ok(()) => sync_dir(parent_dir(path)).map_err(|e| (WriteStep::SyncDir, e)),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err((WriteStep::Remove, err)),
    }
}

/// Removes a temporary file that a crash left. `remove_file` on a symlink removes the link,
/// not its target. A directory is not removed, so the create that follows fails.
fn remove_stale(tmp: &Path) -> io::Result<()> {
    match fs::symlink_metadata(tmp) {
        Ok(meta) if !meta.is_dir() => fs::remove_file(tmp),
        Ok(_) => Ok(()),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err),
    }
}

fn write_and_sync(
    tmp: &Path,
    bytes: &[u8],
    permissions: Option<fs::Permissions>,
) -> Result<(), (WriteStep, io::Error)> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, NEW_FILE_MODE);
    let mut file = options.open(tmp).map_err(|e| (WriteStep::WriteTemp, e))?;
    if let Some(permissions) = permissions {
        file.set_permissions(permissions)
            .map_err(|e| (WriteStep::WriteTemp, e))?;
    }
    file.write_all(bytes)
        .map_err(|e| (WriteStep::WriteTemp, e))?;
    file.sync_all().map_err(|e| (WriteStep::SyncTemp, e))
}

/// Makes `dir` and its missing ancestors. Each new directory is synced into its parent, so
/// a crash cannot leave a renamed file in a directory that does not exist on the disk.
fn create_dirs(dir: &Path) -> io::Result<()> {
    if dir.is_dir() {
        return Ok(());
    }
    let parent = parent_dir(dir);
    create_dirs(parent)?;
    match fs::create_dir(dir) {
        Ok(()) => sync_dir(parent),
        Err(err) if err.kind() == io::ErrorKind::AlreadyExists && dir.is_dir() => Ok(()),
        Err(err) => Err(err),
    }
}

/// The directory that holds `path`. A bare file name is in the current directory.
fn parent_dir(path: &Path) -> &Path {
    match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    }
}

fn sync_dir(dir: &Path) -> io::Result<()> {
    File::open(dir)?.sync_all()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TempDir(PathBuf);

    impl TempDir {
        fn create(test: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "neptron-settings-unit-{}-{test}",
                std::process::id()
            ));
            if path.exists() {
                fs::remove_dir_all(&path).unwrap();
            }
            fs::create_dir_all(&path).unwrap();
            TempDir(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            // A leftover directory in the system temp folder does no harm.
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn write_atomic_makes_every_missing_directory() {
        let dir = TempDir::create("deep");
        let path = dir.0.join("a/b/c/file.toml");
        write_atomic(&path, b"x = 1\n").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"x = 1\n");
        write_atomic(&path, b"x = 2\n").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"x = 2\n");
    }

    #[test]
    fn write_atomic_reports_a_directory_it_cannot_make() {
        let dir = TempDir::create("blocked");
        fs::write(dir.0.join("a"), "a file").unwrap();
        let (step, _) = write_atomic(&dir.0.join("a/b/file.toml"), b"").unwrap_err();
        assert_eq!(step, WriteStep::CreateDir);
        assert_eq!(fs::read_to_string(dir.0.join("a")).unwrap(), "a file");
    }

    #[test]
    fn parent_dir_of_a_bare_name_is_the_current_directory() {
        assert_eq!(parent_dir(Path::new("global.toml")), Path::new("."));
        assert_eq!(parent_dir(Path::new("/a/b.toml")), Path::new("/a"));
    }
}
