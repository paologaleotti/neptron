//! The status of the state and share partitions, as the storage init script
//! of the image found them at boot.
//!
//! The script mounts a RAM folder in place of a partition that fails its
//! check or its mount, so the frontend runs, and writes `storage.toml`
//! into the run folder. Everything written to a RAM folder is lost at
//! power-off, and the player must know it.

use std::io;
use std::path::Path;

use serde::Deserialize;

/// The status file in the run folder.
pub const STATUS_FILE: &str = "storage.toml";

/// What the script mounted at one mount point.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mounted {
    /// The partition: what is written is kept.
    Partition,
    /// A RAM folder: what is written is lost at power-off.
    Ram,
}

/// The status of both mount points.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct StorageStatus {
    /// What is at `/state`.
    pub state: Mounted,
    /// Why `/state` is a RAM folder.
    #[serde(default)]
    pub state_reason: Option<String>,
    /// What is at `/share`.
    pub share: Mounted,
    /// Why `/share` is a RAM folder.
    #[serde(default)]
    pub share_reason: Option<String>,
}

/// The status file cannot be used.
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    /// The file cannot be read.
    #[error("cannot read the storage status {path}: {source}")]
    Read {
        /// The file.
        path: String,
        /// The error.
        #[source]
        source: io::Error,
    },
    /// The file does not parse.
    #[error("the storage status {path} does not parse: {reason}")]
    Parse {
        /// The file.
        path: String,
        /// The parser's message.
        reason: String,
    },
}

impl StorageStatus {
    /// Reads `storage.toml` in the run folder `run`. `None` when the file
    /// is not there: a desktop run, or an image with no such script.
    ///
    /// # Errors
    ///
    /// [`StorageError`] for a file that cannot be read or parsed.
    pub fn read(run: &Path) -> Result<Option<Self>, StorageError> {
        let path = run.join(STATUS_FILE);
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(source) => {
                return Err(StorageError::Read {
                    path: path.display().to_string(),
                    source,
                });
            }
        };
        toml::from_str(&text)
            .map(Some)
            .map_err(|e| StorageError::Parse {
                path: path.display().to_string(),
                reason: e.to_string(),
            })
    }

    /// True when `/state` is a RAM folder: settings, saves and the guard
    /// record are lost at power-off.
    pub fn state_in_ram(&self) -> bool {
        self.state == Mounted::Ram
    }

    /// The notices for the player, one for each RAM folder.
    pub fn notices(&self) -> Vec<String> {
        let mut notices = Vec::new();
        if self.state == Mounted::Ram {
            notices.push(format!(
                "The state partition is not in use ({}). Settings, saves and display choices are lost at power-off.",
                reason(self.state_reason.as_deref())
            ));
        }
        if self.share == Mounted::Ram {
            notices.push(format!(
                "The share partition is not in use ({}). The game folder is empty, and files copied to it are lost at power-off.",
                reason(self.share_reason.as_deref())
            ));
        }
        notices
    }
}

fn reason(text: Option<&str>) -> &str {
    text.filter(|t| !t.is_empty()).unwrap_or("no reason given")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(name: &str, text: Option<&str>) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("neptron-storage-{}-{name}", std::process::id()));
        // A leftover of an earlier run with the same process id; absence is
        // fine.
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("the folder");
        if let Some(text) = text {
            std::fs::write(dir.join(STATUS_FILE), text).expect("the status");
        }
        dir
    }

    #[test]
    fn no_file_is_no_status() {
        let dir = folder("none", None);
        assert_eq!(StorageStatus::read(&dir).expect("read"), None);
    }

    #[test]
    fn both_partitions_give_no_notice() {
        let dir = folder("ok", Some("state = \"partition\"\nshare = \"partition\"\n"));
        let status = StorageStatus::read(&dir).expect("read").expect("a status");
        assert!(!status.state_in_ram());
        assert!(status.notices().is_empty());
    }

    #[test]
    fn a_ram_folder_gives_a_notice_with_its_reason() {
        let text = "state = \"ram\"\nstate_reason = \"e2fsck /dev/sda3 left errors (exit 4)\"\n\
                    share = \"ram\"\nshare_reason = \"no share partition\"\n";
        let dir = folder("ram", Some(text));
        let status = StorageStatus::read(&dir).expect("read").expect("a status");
        assert!(status.state_in_ram());
        let notices = status.notices();
        assert_eq!(notices.len(), 2);
        assert!(notices[0].contains("(e2fsck /dev/sda3 left errors (exit 4))"));
        assert!(notices[0].contains("lost at power-off"));
        assert!(notices[1].contains("(no share partition)"));
    }

    #[test]
    fn a_bad_file_is_an_error() {
        let dir = folder("bad", Some("state = \"floppy\"\n"));
        assert!(matches!(
            StorageStatus::read(&dir),
            Err(StorageError::Parse { .. })
        ));
    }
}
