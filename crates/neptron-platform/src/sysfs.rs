//! The thin reader of the DRM class in sysfs. It reads files and leaves
//! every decision to the parsers and the capability table.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::gpu::{
    Connector, ConnectorName, Gpu, ParseError, Platform, parse_card_name, parse_connector_name,
    parse_status, parse_uevent,
};

/// Why sysfs could not be read.
#[derive(Debug, Error)]
pub enum SysfsError {
    /// A file or a directory could not be read.
    #[error("cannot read {path}: {source}")]
    Read {
        /// The path.
        path: PathBuf,
        /// The error of the read.
        #[source]
        source: io::Error,
    },
    /// A file that a connector always has is missing.
    #[error("{path} is missing")]
    Missing {
        /// The path.
        path: PathBuf,
    },
    /// A file did not parse.
    #[error("{path}: {source}")]
    Parse {
        /// The path.
        path: PathBuf,
        /// The parse error.
        #[source]
        source: ParseError,
    },
}

/// Lists the GPUs and connectors under `sysfs_root`, which is `/sys` on a
/// running system and a fixture tree in tests.
///
/// It reads `class/drm`: for each `card<n>` the uevent of its device, and
/// for each `card<n>-<connector>` its `status` and `edid` files. Other
/// entries are skipped. A system with no `class/drm` has no GPU.
///
/// # Errors
///
/// Returns a [`SysfsError`] with the path when a read fails for a reason
/// other than a missing file, when a connector has no `status` file, or
/// when a file does not parse.
pub fn detect(sysfs_root: &Path) -> Result<Platform, SysfsError> {
    let drm = sysfs_root.join("class").join("drm");
    let Some(names) = entry_names(&drm)? else {
        return Ok(Platform::default());
    };
    let mut platform = Platform::default();
    for name in names {
        if parse_card_name(&name).is_some() {
            platform.gpus.push(read_gpu(&drm, &name)?);
        } else if let Some(parts) = parse_connector_name(&name) {
            platform
                .connectors
                .push(read_connector(&drm, &name, parts)?);
        }
    }
    platform.gpus.sort_by_key(|gpu| parse_card_name(&gpu.card));
    platform.connectors.sort_by(|a, b| {
        (parse_card_name(&a.card), &a.name).cmp(&(parse_card_name(&b.card), &b.name))
    });
    Ok(platform)
}

/// The UTF-8 entry names of `dir`, or `None` when it does not exist.
fn entry_names(dir: &Path) -> Result<Option<Vec<String>>, SysfsError> {
    let read_error = |source| SysfsError::Read {
        path: dir.to_path_buf(),
        source,
    };
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(read_error(error)),
    };
    let mut names = Vec::new();
    for entry in entries {
        let entry = entry.map_err(read_error)?;
        // The kernel names DRM entries in ASCII; any other name is not one
        // of them.
        if let Ok(name) = entry.file_name().into_string() {
            names.push(name);
        }
    }
    Ok(Some(names))
}

fn read_gpu(drm: &Path, card: &str) -> Result<Gpu, SysfsError> {
    let path = drm.join(card).join("device").join("uevent");
    let uevent = match read_optional(&path)? {
        Some(bytes) => parse_uevent(&String::from_utf8_lossy(&bytes))
            .map_err(|source| SysfsError::Parse { path, source })?,
        None => Default::default(),
    };
    Ok(Gpu {
        card: card.into(),
        driver: uevent.driver,
        pci_id: uevent.pci_id,
        of_compatible: uevent.of_compatible,
    })
}

fn read_connector(
    drm: &Path,
    dir_name: &str,
    parts: ConnectorName,
) -> Result<Connector, SysfsError> {
    let dir = drm.join(dir_name);
    let status_path = dir.join("status");
    let status_bytes = read_optional(&status_path)?.ok_or_else(|| SysfsError::Missing {
        path: status_path.clone(),
    })?;
    let status = parse_status(&String::from_utf8_lossy(&status_bytes)).map_err(|source| {
        SysfsError::Parse {
            path: status_path,
            source,
        }
    })?;
    let edid = read_optional(&dir.join("edid"))?.filter(|bytes| !bytes.is_empty());
    Ok(Connector {
        card: parts.card,
        name: parts.name,
        kind: parts.kind,
        status,
        edid,
    })
}

/// The bytes of `path`, or `None` when it does not exist.
fn read_optional(path: &Path) -> Result<Option<Vec<u8>>, SysfsError> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(SysfsError::Read {
            path: path.to_path_buf(),
            source,
        }),
    }
}
