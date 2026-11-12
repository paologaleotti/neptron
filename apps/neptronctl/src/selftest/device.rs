//! The KMS device of the run on a display: the DRM node opened by the
//! tool, the stop signals and the monotonic clock. Linux with the `kms`
//! feature only.

use std::fs::OpenOptions;
use std::os::fd::OwnedFd;
use std::os::unix::fs::{FileTypeExt as _, MetadataExt as _};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use neptron_backend::Clock;
use neptron_kms::{KmsConfig, KmsDisplay, is_drm_node};
use neptron_types::GpuCaps;

use super::error::SelftestError;
use super::report::DisplayReport;

/// Opens the DRM node at `path` and the KMS display on it, with buffers of
/// `buffer` (width, height). It sets no mode.
///
/// # Errors
///
/// [`SelftestError::Device`] when the node cannot be opened, is not a DRM
/// device, or the backend cannot start on it.
pub fn open(
    path: &Path,
    connector: Option<String>,
    crt: bool,
    caps: GpuCaps,
    buffer: (u16, u16),
) -> Result<KmsDisplay, SelftestError> {
    let failed = |reason: String| SelftestError::Device {
        path: path.to_owned(),
        reason,
    };
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .map_err(|e| failed(e.to_string()))?;
    let meta = file.metadata().map_err(|e| failed(e.to_string()))?;
    if !is_drm_node(meta.file_type().is_char_device(), meta.rdev()) {
        return Err(failed(
            "not a DRM device (a character device of major 226)".to_owned(),
        ));
    }
    KmsDisplay::open(
        OwnedFd::from(file),
        KmsConfig {
            caps,
            output: connector,
            crt,
            buffer_width: buffer.0,
            buffer_height: buffer.1,
            writeback: false,
        },
    )
    .map_err(|e| failed(e.to_string()))
}

/// What the backend says about the display, for the report.
pub fn facts(path: &Path, display: &KmsDisplay) -> DisplayReport {
    DisplayReport {
        device: Some(path.display().to_string()),
        driver: Some(display.driver_name().to_owned()),
        connector: Some(display.connector_name().to_owned()),
        linear_by_modifier: Some(display.linear_by_modifier()),
    }
}

/// A flag that SIGINT, SIGTERM and SIGHUP set. The run reads it before
/// each flip, so a signal stops the run and turns the output off.
///
/// # Errors
///
/// [`SelftestError::Signal`] when a handler cannot be installed.
pub fn stop_flag() -> Result<Arc<AtomicBool>, SelftestError> {
    let flag = Arc::new(AtomicBool::new(false));
    for signal in [
        signal_hook::consts::SIGINT,
        signal_hook::consts::SIGTERM,
        signal_hook::consts::SIGHUP,
    ] {
        signal_hook::flag::register(signal, Arc::clone(&flag)).map_err(|e| {
            SelftestError::Signal {
                reason: format!("signal {signal}: {e}"),
            }
        })?;
    }
    Ok(flag)
}

/// The monotonic clock of the display time.
#[derive(Debug, Clone, Copy)]
pub struct Monotonic {
    start: Instant,
}

impl Monotonic {
    /// A clock that starts now.
    pub fn new() -> Self {
        Self {
            start: Instant::now(),
        }
    }
}

impl Default for Monotonic {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for Monotonic {
    fn now_ns(&self) -> u64 {
        u64::try_from(self.start.elapsed().as_nanos()).unwrap_or(u64::MAX)
    }
}
