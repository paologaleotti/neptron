//! The monitor presets, read from files at run time. The presets carry the
//! GPL licence of Switchres, so the binary embeds none of them (OP-044).

use std::io;
use std::path::{Path, PathBuf};

use neptron_types::MonitorProfile;

/// The monitor presets of one folder, sorted by id.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Presets {
    profiles: Vec<MonitorProfile>,
}

/// A preset file that was left out.
#[derive(Debug, thiserror::Error)]
pub enum PresetError {
    /// The folder cannot be read.
    #[error("cannot read the monitor folder {}: {source}", path.display())]
    Folder {
        /// The folder.
        path: PathBuf,
        /// The error of the system.
        #[source]
        source: io::Error,
    },
    /// A file cannot be read.
    #[error("cannot read {}: {source}", path.display())]
    Read {
        /// The file.
        path: PathBuf,
        /// The error of the system.
        #[source]
        source: io::Error,
    },
    /// A file is not a monitor profile.
    #[error("{} is not a monitor profile: {message}", path.display())]
    Parse {
        /// The file.
        path: PathBuf,
        /// The parser message.
        message: String,
    },
    /// The id in a file is not its file name.
    #[error("{} holds the profile {id:?}, not the one of its file name", path.display())]
    IdMismatch {
        /// The file.
        path: PathBuf,
        /// The id in the file.
        id: String,
    },
}

impl Presets {
    /// Reads every `*.toml` file of `dir`. A file that cannot be used is
    /// left out and reported; the others load.
    ///
    /// # Errors
    ///
    /// [`PresetError::Folder`] when the folder cannot be read. The caller
    /// then has no preset, and every monitor is the safe default.
    pub fn load(dir: &Path) -> Result<(Self, Vec<PresetError>), PresetError> {
        let entries = std::fs::read_dir(dir).map_err(|source| PresetError::Folder {
            path: dir.to_owned(),
            source,
        })?;
        let mut paths: Vec<PathBuf> = entries
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "toml"))
            .collect();
        paths.sort();
        let mut profiles = Vec::with_capacity(paths.len());
        let mut issues = Vec::new();
        for path in paths {
            match read_profile(&path) {
                Ok(profile) => profiles.push(profile),
                Err(e) => issues.push(e),
            }
        }
        profiles.sort_by(|a, b| a.id.cmp(&b.id));
        Ok((Self { profiles }, issues))
    }

    /// Presets from profiles, for tests and tools.
    pub fn from_profiles(mut profiles: Vec<MonitorProfile>) -> Self {
        profiles.sort_by(|a, b| a.id.cmp(&b.id));
        profiles.dedup_by(|a, b| a.id == b.id);
        Self { profiles }
    }

    /// Every preset, sorted by id.
    pub fn profiles(&self) -> &[MonitorProfile] {
        &self.profiles
    }

    /// The preset with this id.
    pub fn get(&self, id: &str) -> Option<&MonitorProfile> {
        self.profiles
            .binary_search_by(|p| p.id.as_str().cmp(id))
            .ok()
            .map(|i| &self.profiles[i])
    }
}

fn read_profile(path: &Path) -> Result<MonitorProfile, PresetError> {
    let text = std::fs::read_to_string(path).map_err(|source| PresetError::Read {
        path: path.to_owned(),
        source,
    })?;
    let profile: MonitorProfile = toml::from_str(&text).map_err(|e| PresetError::Parse {
        path: path.to_owned(),
        message: e.to_string(),
    })?;
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    if profile.id != stem {
        return Err(PresetError::IdMismatch {
            path: path.to_owned(),
            id: profile.id,
        });
    }
    Ok(profile)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn monitors_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/monitors")
    }

    #[test]
    fn the_shipped_presets_load_with_no_issue() {
        let (presets, issues) = Presets::load(&monitors_dir()).expect("the folder");
        assert!(issues.is_empty(), "{issues:?}");
        assert!(presets.get("generic_15").is_some());
        assert!(presets.get("pc_31_120").is_some());
        assert!(presets.get("nope").is_none());
        let ids: Vec<&str> = presets.profiles().iter().map(|p| p.id.as_str()).collect();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        assert_eq!(ids, sorted);
    }

    #[test]
    fn a_bad_file_is_left_out_and_reported() {
        let dir = std::env::temp_dir().join(format!("neptron-presets-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dir");
        let good = std::fs::read_to_string(monitors_dir().join("ntsc.toml")).expect("ntsc");
        std::fs::write(dir.join("ntsc.toml"), &good).expect("write");
        std::fs::write(dir.join("other.toml"), &good).expect("write");
        std::fs::write(dir.join("broken.toml"), "id = 3").expect("write");
        std::fs::write(dir.join("README.md"), "text").expect("write");
        let (presets, issues) = Presets::load(&dir).expect("the folder");
        assert_eq!(presets.profiles().len(), 1);
        assert_eq!(issues.len(), 2, "{issues:?}");
        assert!(
            issues
                .iter()
                .any(|e| matches!(e, PresetError::IdMismatch { .. }))
        );
        assert!(
            issues
                .iter()
                .any(|e| matches!(e, PresetError::Parse { .. }))
        );
        // The folder is the test's own; a failed removal leaves a temp folder.
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_missing_folder_is_an_error() {
        assert!(Presets::load(Path::new("/nonexistent/monitors")).is_err());
    }
}
