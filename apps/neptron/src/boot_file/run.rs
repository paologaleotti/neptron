//! The file side of the import: it reads `neptron.toml` and the stored
//! hash, and writes the settings and the new hash. It decides nothing.

use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

use neptron_settings::{Change, Schema, Storage, Target};

use super::{Decision, FILE_NAME, FileRead, MAX_FILE_BYTES, decide};
use crate::atomic;

/// What the import did, for the notices and the log of the start.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Imported {
    /// The problems for the player, each also a log line.
    pub problems: Vec<String>,
    /// Lines for the log only.
    pub log: Vec<String>,
}

/// Imports `file` into the global settings of `storage` when its hash
/// differs from the one in `hash_file`, then stores the new hash. Every
/// failure is a problem in the result; none stops the start.
pub fn import(schema: &Schema, file: &Path, hash_file: &Path, storage: &Storage) -> Imported {
    let mut log = Vec::new();
    let stored = match std::fs::read_to_string(hash_file) {
        Ok(text) => Some(text.trim().to_owned()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => None,
        Err(e) => {
            log.push(format!(
                "the hash of the last {FILE_NAME} in {} cannot be read ({e}); the file is imported again",
                hash_file.display()
            ));
            None
        }
    };
    let Decision {
        changes,
        hash,
        mut problems,
        log: decided,
    } = decide(schema, &read(file), stored.as_deref());
    log.extend(decided);
    let Some(hash) = hash else {
        return Imported { problems, log };
    };
    if !changes.is_empty() {
        let changes: Vec<Change<'_>> = changes
            .iter()
            .map(|(key, value)| Change::Set {
                key,
                value: value.clone(),
            })
            .collect();
        if let Err(e) = storage.apply(schema, Target::Global, &changes) {
            problems.push(format!(
                "The settings of {FILE_NAME} are not copied ({e}). The next start tries again."
            ));
            return Imported { problems, log };
        }
    }
    if let Err(e) = atomic::write(hash_file, format!("{hash}\n").as_bytes()) {
        problems.push(format!(
            "The hash of {FILE_NAME} cannot be stored in {} ({e}). Each start copies the file again, so a menu change of its keys is lost.",
            hash_file.display()
        ));
    }
    Imported { problems, log }
}

/// Reads the file, up to [`MAX_FILE_BYTES`].
fn read(path: &Path) -> FileRead {
    let unreadable = |e: io::Error| FileRead::Unreadable {
        message: e.to_string(),
    };
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return FileRead::Missing,
        Err(e) => return unreadable(e),
    };
    let mut bytes = Vec::new();
    // One byte past the limit tells a file at the limit from a larger one.
    match file
        .by_ref()
        .take(MAX_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
    {
        Ok(_) if bytes.len() as u64 > MAX_FILE_BYTES => FileRead::TooLarge {
            bytes: file.metadata().map_or(bytes.len() as u64, |m| m.len()),
        },
        Ok(_) => FileRead::Read(bytes),
        Err(e) => unreadable(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(name: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("neptron-boot-file-{name}-{}", std::process::id()));
        // A leftover of an earlier run with the same process id; absence is fine.
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("the folder");
        dir
    }

    #[test]
    fn the_read_tells_a_missing_a_large_and_an_unreadable_file() {
        let dir = folder("read");
        assert_eq!(read(&dir.join("none.toml")), FileRead::Missing);
        let at_limit = dir.join("limit.toml");
        let limit = usize::try_from(MAX_FILE_BYTES).expect("a small limit");
        std::fs::write(&at_limit, vec![b'#'; limit]).expect("write");
        assert!(matches!(read(&at_limit), FileRead::Read(b) if b.len() == limit));
        let large = dir.join("large.toml");
        std::fs::write(&large, vec![b'#'; limit + 10]).expect("write");
        assert_eq!(
            read(&large),
            FileRead::TooLarge {
                bytes: MAX_FILE_BYTES + 10
            }
        );
        assert!(matches!(read(&dir), FileRead::Unreadable { .. }));
        // The folder is the test's own; a failed removal leaves a temp folder.
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_settings_file_that_does_not_parse_keeps_the_hash_for_the_next_start() {
        let dir = folder("apply");
        let config = dir.join("config");
        std::fs::create_dir_all(&config).expect("config");
        std::fs::write(config.join("global.toml"), "[display\n").expect("a bad global file");
        let file = dir.join("neptron.toml");
        std::fs::write(&file, "[display]\ntv = \"pal\"\n").expect("the file");
        let hash = dir.join("hash");
        let imported = import(&Schema::builtin(), &file, &hash, &Storage::new(&config));
        assert_eq!(imported.problems.len(), 1, "{imported:?}");
        assert!(imported.problems[0].starts_with("The settings of neptron.toml are not copied ("));
        assert!(!hash.exists());
        assert_eq!(
            std::fs::read_to_string(config.join("global.toml")).expect("read"),
            "[display\n"
        );
        // The folder is the test's own; a failed removal leaves a temp folder.
        let _ = std::fs::remove_dir_all(&dir);
    }
}
