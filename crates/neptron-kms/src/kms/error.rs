use std::io;

use neptron_gfx::gles::GlesError;

use crate::ChoiceError;

/// An error while the display opens.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum KmsError {
    /// `NEPTRON_DRM_FD` is not set or not a descriptor number.
    #[error("NEPTRON_DRM_FD {value:?} is not a descriptor number; the launcher sets it")]
    Env {
        /// The value, or an empty text when the variable is not set.
        value: String,
    },
    /// The descriptor is not open.
    #[error("descriptor {fd} is not open")]
    NotOpen {
        /// The descriptor number.
        fd: i32,
    },
    /// The number names standard input, output or error, which other code
    /// owns.
    #[error("descriptor {fd} is a standard stream, not a DRM device from the launcher")]
    StandardFd {
        /// The descriptor number.
        fd: i32,
    },
    /// The launcher's descriptor was taken before in this process.
    #[error("descriptor {fd} from NEPTRON_DRM_FD was taken before; it has one owner")]
    LauncherFdTaken {
        /// The descriptor number.
        fd: i32,
    },
    /// The descriptor is not a DRM device.
    #[error("the descriptor is not a DRM device: {reason}")]
    NotDrm {
        /// What is wrong with it.
        reason: String,
    },
    /// A DRM call failed.
    #[error("cannot {operation}: {source}")]
    Io {
        /// What the backend was doing.
        operation: &'static str,
        /// The kernel error.
        #[source]
        source: io::Error,
    },
    /// No connector can be chosen.
    #[error(transparent)]
    Choice(#[from] ChoiceError),
    /// No CRTC can drive the connector.
    #[error("no free CRTC can drive {connector}")]
    NoCrtc {
        /// The connector name.
        connector: String,
    },
    /// The CRTC of the connector drives another connector too, which would
    /// get the same modes.
    #[error("the CRTC of {connector} also drives {other}, which would get the same modes")]
    SharedCrtc {
        /// The chosen connector.
        connector: String,
        /// The other connector on the same CRTC.
        other: String,
    },
    /// No free primary plane can feed the CRTC.
    #[error("no free primary plane can feed CRTC {crtc}")]
    NoPrimaryPlane {
        /// The CRTC object id.
        crtc: u32,
    },
    /// A DRM object lacks an atomic property the backend needs.
    #[error("the {object} has no {name} property; atomic modesetting needs it")]
    MissingProperty {
        /// The kind of object.
        object: &'static str,
        /// The property name.
        name: &'static str,
    },
    /// Writeback is asked for and the device has no writeback connector.
    #[error("the device has no writeback connector")]
    NoWriteback,
    /// The buffer size is zero or above the GPU limits.
    #[error(
        "buffers of {width}x{height} are not possible: each side must be 1 to the GPU limit \
         of {max_width}x{max_height}"
    )]
    BufferSize {
        /// The asked width, in pixels.
        width: u16,
        /// The asked height, in lines.
        height: u16,
        /// `GpuCaps::max_width`.
        max_width: u16,
        /// `GpuCaps::max_height`.
        max_height: u16,
    },
    /// A GBM call failed.
    #[error("{operation} failed: {reason}")]
    Gbm {
        /// The GBM call.
        operation: &'static str,
        /// The error.
        reason: String,
    },
    /// The renderer cannot start on the GBM surface.
    #[error("the renderer cannot start on the GBM surface: {0}")]
    Renderer(#[from] GlesError),
}

impl KmsError {
    pub(super) fn io(operation: &'static str) -> impl FnOnce(io::Error) -> KmsError {
        move |source| KmsError::Io { operation, source }
    }
}
