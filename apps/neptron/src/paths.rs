//! Where the frontend finds its files, from the folders of the command
//! line.

use std::path::{Path, PathBuf};

/// The folders of the frontend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paths {
    /// The share partition: `roms/`, `bios/`.
    pub share: PathBuf,
    /// The state partition: `config/`, `saves/`, `video/`, `cache/`.
    pub state: PathBuf,
    /// The read-only data of the image: `monitors/`, `skins/`.
    pub data: PathBuf,
    /// The libretro cores, by the library file name of `data/cores`.
    pub cores: PathBuf,
    /// The RAM folder for the relaunch file.
    pub run: PathBuf,
    /// The sysfs root for the GPU and EDID detection.
    pub sysfs: PathBuf,
}

impl Paths {
    /// The folders of the image.
    pub fn image() -> Self {
        Self {
            share: PathBuf::from("/share"),
            state: PathBuf::from("/state"),
            data: PathBuf::from("/usr/share/neptron"),
            cores: PathBuf::from("/usr/lib/libretro"),
            run: PathBuf::from("/run/neptron"),
            sysfs: PathBuf::from("/sys"),
        }
    }

    /// `share/roms`.
    pub fn roms(&self) -> PathBuf {
        self.share.join("roms")
    }

    /// `share/bios`, the system folder of the cores.
    pub fn bios(&self) -> PathBuf {
        self.share.join("bios")
    }

    /// `state/config`, the settings folder.
    pub fn config(&self) -> PathBuf {
        self.state.join("config")
    }

    /// `state/saves/<system>`.
    pub fn saves(&self, system: &str) -> PathBuf {
        self.state.join("saves").join(system)
    }

    /// `state/video/guard.toml`, the guard record.
    pub fn guard_record(&self) -> PathBuf {
        self.state.join("video").join("guard.toml")
    }

    /// `state/cache/library.cache`, the library index cache.
    pub fn library_cache(&self) -> PathBuf {
        self.state.join("cache").join("library.cache")
    }

    /// `data/monitors`, the monitor presets.
    pub fn monitors(&self) -> PathBuf {
        self.data.join("monitors")
    }

    /// `data/skins/default/skin.toml`.
    pub fn skin(&self) -> PathBuf {
        self.data.join("skins").join("default").join("skin.toml")
    }

    /// `run/library-scanned`: present when a scan of every folder reached
    /// the library cache in this power cycle. The run folder is a RAM
    /// folder, so a new power cycle starts with no marker.
    pub fn library_scanned(&self) -> PathBuf {
        self.run.join("library-scanned")
    }

    /// `run/output`, the connector of this boot: the output that the first
    /// run of the boot chose under `auto`, which every relaunch keeps. The
    /// run folder is a RAM folder, so a new power cycle has none.
    pub fn boot_output(&self) -> PathBuf {
        self.run.join("output")
    }

    /// `run/next.toml`, the relaunch file.
    pub fn relaunch(&self) -> PathBuf {
        self.run.join("next.toml")
    }

    /// The library of a core.
    pub fn core(&self, library: &str) -> PathBuf {
        self.cores.join(library)
    }

    /// A game of the index, by its path under the ROM folder.
    pub fn game(&self, path: &str) -> PathBuf {
        self.roms().join(Path::new(path))
    }
}
