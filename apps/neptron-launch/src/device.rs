//! The DRM device that the launcher holds for every run of the frontend.

use std::fs::{File, OpenOptions};
use std::io;
use std::os::fd::AsRawFd;
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::path::{Path, PathBuf};

use neptron_kms::{STANDARD_FD_MAX, is_drm_node};
use rustix::io::{FdFlags, fcntl_setfd};

/// The variable that gives the frontend the descriptor number. It is the
/// name that `neptron_kms::DRM_FD_ENV` reads.
pub const DRM_FD_ENV: &str = "NEPTRON_DRM_FD";

/// The device the launcher opens when the command line names none.
pub const DEFAULT_DEVICE: &str = "/dev/dri/card0";

/// The open DRM device. The launcher keeps it open for its whole life, so
/// the kernel keeps the framebuffers and the mode on the wire while no
/// frontend runs: across a crash, a relaunch or a restart.
#[derive(Debug)]
pub struct Device {
    file: File,
}

/// Why the device cannot be used.
#[derive(Debug, thiserror::Error)]
pub enum DeviceError {
    /// The device cannot be opened or read.
    #[error("cannot open {}: {source}", path.display())]
    Open {
        /// The device.
        path: PathBuf,
        /// The error of the system.
        #[source]
        source: io::Error,
    },
    /// The path is not a DRM character device.
    #[error("{} is not a DRM device (a character device of major 226)", path.display())]
    NotDrm {
        /// The path.
        path: PathBuf,
    },
    /// The descriptor is a standard stream: the launcher started with one
    /// of them closed. The frontend refuses such a number.
    #[error("{} opened on descriptor {fd}, a standard stream; start the launcher with 0, 1 and 2 open", path.display())]
    StandardFd {
        /// The device.
        path: PathBuf,
        /// The descriptor.
        fd: i32,
    },
    /// The descriptor cannot be made inheritable.
    #[error("cannot pass {} to the frontend: {source}", path.display())]
    Inherit {
        /// The device.
        path: PathBuf,
        /// The error of the system.
        #[source]
        source: io::Error,
    },
}

impl Device {
    /// Opens the DRM device at `path` for reading and writing, checks it,
    /// and makes its descriptor inheritable, so each frontend process gets
    /// it under the same number. The frontend and the launcher then share
    /// one open file description: DRM master, framebuffers and the mode.
    ///
    /// # Errors
    ///
    /// [`DeviceError`].
    pub fn open(path: &Path) -> Result<Self, DeviceError> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .map_err(|source| DeviceError::Open {
                path: path.to_owned(),
                source,
            })?;
        let meta = file.metadata().map_err(|source| DeviceError::Open {
            path: path.to_owned(),
            source,
        })?;
        if !is_drm_node(meta.file_type().is_char_device(), meta.rdev()) {
            return Err(DeviceError::NotDrm {
                path: path.to_owned(),
            });
        }
        let fd = file.as_raw_fd();
        if fd <= STANDARD_FD_MAX {
            return Err(DeviceError::StandardFd {
                path: path.to_owned(),
                fd,
            });
        }
        inheritable(&file).map_err(|source| DeviceError::Inherit {
            path: path.to_owned(),
            source,
        })?;
        Ok(Self { file })
    }

    /// The descriptor number that the frontend finds in [`DRM_FD_ENV`].
    pub fn fd(&self) -> i32 {
        self.file.as_raw_fd()
    }
}

/// Clears close-on-exec, which the standard library sets on every file it
/// opens, so a child process keeps the descriptor.
pub(crate) fn inheritable(file: &File) -> io::Result<()> {
    fcntl_setfd(file, FdFlags::empty()).map_err(io::Error::from)
}

#[cfg(test)]
mod tests {
    use rustix::io::fcntl_getfd;

    use super::*;

    fn temp_file(name: &str) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("neptron-launch-{}-{name}", std::process::id()));
        std::fs::write(&path, b"x").expect("write a temp file");
        path
    }

    #[test]
    fn a_regular_file_is_not_a_drm_device() {
        let path = temp_file("regular");
        let result = Device::open(&path);
        assert!(
            matches!(result, Err(DeviceError::NotDrm { .. })),
            "{result:?}"
        );
        // The file is the test's own; a failed removal leaves a temp file.
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_character_device_of_another_major_is_not_a_drm_device() {
        let result = Device::open(Path::new("/dev/null"));
        assert!(
            matches!(result, Err(DeviceError::NotDrm { .. })),
            "{result:?}"
        );
    }

    #[test]
    fn a_missing_device_cannot_be_opened() {
        let result = Device::open(Path::new("/dev/neptron-no-such-device"));
        assert!(
            matches!(result, Err(DeviceError::Open { .. })),
            "{result:?}"
        );
    }

    #[test]
    fn the_descriptor_becomes_inheritable() {
        let path = temp_file("inherit");
        let file = File::open(&path).expect("open");
        assert!(
            fcntl_getfd(&file)
                .expect("flags")
                .contains(FdFlags::CLOEXEC),
            "the standard library opens with close-on-exec"
        );
        inheritable(&file).expect("clear the flag");
        assert!(
            !fcntl_getfd(&file)
                .expect("flags")
                .contains(FdFlags::CLOEXEC)
        );
        // The file is the test's own; a failed removal leaves a temp file.
        let _ = std::fs::remove_file(&path);
    }
}
