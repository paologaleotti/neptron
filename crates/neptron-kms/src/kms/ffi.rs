//! Every `unsafe` call of the backend: taking a descriptor by number, libgbm,
//! and the pointers that EGL gets.
//!
//! A [`Bo`] exists only between [`Gbm::lock_front`] and [`Gbm::release`],
//! and only for the surface of the [`Gbm`] that made it. Each call below
//! relies on that.

use std::ffi::c_void;
use std::os::fd::{AsRawFd as _, FromRawFd as _, OwnedFd, RawFd};
use std::path::PathBuf;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicBool, Ordering};

use gbm_sys as gbm;
use neptron_gfx::gles::GbmTarget;

use super::KmsError;

/// How the buffers were made linear.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinearBy {
    /// The surface was made with the modifier list `[DRM_FORMAT_MOD_LINEAR]`.
    Modifier,
    /// The driver takes no modifiers; the surface was made with the usage
    /// flag `GBM_BO_USE_LINEAR`.
    UsageFlag,
}

/// Takes the descriptor `fd` that the launcher passed, once per process.
///
/// The launcher contract: the number in `NEPTRON_DRM_FD` names a descriptor
/// that the launcher opened for this process and that no other code of the
/// process uses.
pub(super) fn take_launcher_fd(fd: RawFd) -> Result<OwnedFd, KmsError> {
    if fd < 0 {
        return Err(KmsError::NotOpen { fd });
    }
    if !PathBuf::from(format!("/proc/self/fd/{fd}")).exists() {
        return Err(KmsError::NotOpen { fd });
    }
    if LAUNCHER_FD_TAKEN.swap(true, Ordering::SeqCst) {
        return Err(KmsError::LauncherFdTaken { fd });
    }
    // SAFETY: /proc/self/fd shows that fd is open. The launcher contract
    // says that no other code of the process owns it, and the flag above
    // lets the process take it only once, so this OwnedFd is its one owner.
    Ok(unsafe { OwnedFd::from_raw_fd(fd) })
}

static LAUNCHER_FD_TAKEN: AtomicBool = AtomicBool::new(false);

/// Takes the out-fence descriptor that the kernel wrote during a commit.
pub(super) fn take_fence(fd: i32) -> Option<OwnedFd> {
    if fd < 0 {
        return None;
    }
    // SAFETY: the kernel created this descriptor in the atomic commit that
    // just returned and wrote its number only into the caller's slot, so
    // no other code of the process knows it.
    Some(unsafe { OwnedFd::from_raw_fd(fd) })
}

/// A GBM device and one XRGB8888 scanout surface on it.
pub(super) struct Gbm {
    surface: NonNull<gbm::gbm_surface>,
    device: NonNull<gbm::gbm_device>,
    linear_by: LinearBy,
    // Dropped after the device, which uses it.
    _fd: OwnedFd,
}

/// One buffer of the surface, locked for scanout. Not `Clone`: it goes back
/// to the surface exactly once, through [`Gbm::release`].
pub(super) struct Bo(NonNull<gbm::gbm_bo>);

/// What the kernel needs to know of a buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct BoInfo {
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub handle: u32,
}

/// `GBM_FORMAT_XRGB8888`, the fourcc `XR24`.
pub(super) const FORMAT_XRGB8888: u32 = 0x3432_5258;
const USE_SCANOUT: u32 = 1 << 0;
const USE_RENDERING: u32 = 1 << 2;
const USE_LINEAR: u32 = 1 << 4;
const MOD_LINEAR: u64 = 0;

impl Gbm {
    /// A device on `fd` (a duplicate of the display's descriptor) and a
    /// linear surface of `width` by `height`.
    pub(super) fn new(fd: OwnedFd, width: u32, height: u32) -> Result<Gbm, KmsError> {
        // SAFETY: fd is an open DRM descriptor that this value owns and
        // keeps open for as long as the device lives.
        let device = NonNull::new(unsafe { gbm::gbm_create_device(fd.as_raw_fd()) }).ok_or(
            KmsError::Gbm {
                operation: "gbm_create_device",
                reason: std::io::Error::last_os_error().to_string(),
            },
        )?;
        let modifiers = [MOD_LINEAR];
        // SAFETY: device is a live GBM device; the modifier array holds
        // the count of entries given.
        let with_modifier = unsafe {
            gbm::gbm_surface_create_with_modifiers(
                device.as_ptr(),
                width,
                height,
                FORMAT_XRGB8888,
                modifiers.as_ptr(),
                1,
            )
        };
        let (surface, linear_by) = match NonNull::new(with_modifier) {
            Some(s) => (s, LinearBy::Modifier),
            None => {
                // SAFETY: device is a live GBM device; the flags are valid
                // usage bits.
                let plain = unsafe {
                    gbm::gbm_surface_create(
                        device.as_ptr(),
                        width,
                        height,
                        FORMAT_XRGB8888,
                        USE_SCANOUT | USE_RENDERING | USE_LINEAR,
                    )
                };
                match NonNull::new(plain) {
                    Some(s) => (s, LinearBy::UsageFlag),
                    None => {
                        let reason = std::io::Error::last_os_error().to_string();
                        // SAFETY: the device is live and has no surface.
                        unsafe { gbm::gbm_device_destroy(device.as_ptr()) };
                        return Err(KmsError::Gbm {
                            operation: "gbm_surface_create",
                            reason,
                        });
                    }
                }
            }
        };
        Ok(Gbm {
            surface,
            device,
            linear_by,
            _fd: fd,
        })
    }

    /// How the buffers were made linear.
    pub(super) fn linear_by(&self) -> LinearBy {
        self.linear_by
    }

    /// Locks the buffer that the last swap of the EGL surface finished.
    /// The caller calls it once after each swap.
    pub(super) fn lock_front(&self) -> Option<Bo> {
        // SAFETY: the surface is live. A null result (no swapped buffer)
        // is returned as None.
        NonNull::new(unsafe { gbm::gbm_surface_lock_front_buffer(self.surface.as_ptr()) }).map(Bo)
    }

    /// Gives a locked buffer back to the surface.
    pub(super) fn release(&self, bo: Bo) {
        // SAFETY: bo was locked on this surface (see the module comment)
        // and is consumed here, so it goes back once.
        unsafe { gbm::gbm_surface_release_buffer(self.surface.as_ptr(), bo.0.as_ptr()) };
    }

    /// The size, stride and GEM handle of a locked buffer.
    pub(super) fn info(&self, bo: &Bo) -> BoInfo {
        let p = bo.0.as_ptr();
        // SAFETY: bo is a locked buffer of this surface, so it is live. The
        // handle union of a DRM buffer holds a u32 GEM handle.
        unsafe {
            BoInfo {
                width: gbm::gbm_bo_get_width(p),
                height: gbm::gbm_bo_get_height(p),
                stride: gbm::gbm_bo_get_stride(p),
                handle: gbm::gbm_bo_get_handle(p).u32_,
            }
        }
    }

    /// The modifier of a locked buffer; `DRM_FORMAT_MOD_INVALID` when the
    /// layout is implicit.
    pub(super) fn modifier(&self, bo: &Bo) -> u64 {
        // SAFETY: bo is a locked buffer of this surface, so it is live.
        unsafe { gbm::gbm_bo_get_modifier(bo.0.as_ptr()) }
    }
}

impl Bo {
    /// A key that is the same for each lock of the same buffer.
    pub(super) fn key(&self) -> usize {
        self.0.as_ptr() as usize
    }
}

impl Drop for Gbm {
    fn drop(&mut self) {
        // SAFETY: both are live and owned by this value. Every Bo went back
        // to the surface first (the display releases them in its Drop), and
        // the EGL surface on it is gone (the renderer drops first).
        unsafe {
            gbm::gbm_surface_destroy(self.surface.as_ptr());
            gbm::gbm_device_destroy(self.device.as_ptr());
        }
    }
}

// SAFETY: both pointers are live for as long as the Gbm value, and the
// renderer holds an Rc to it, so they outlive its EGL display and surface.
// The surface was made on this device with format XRGB8888.
unsafe impl GbmTarget for Gbm {
    fn gbm_device(&self) -> *mut c_void {
        self.device.as_ptr().cast()
    }

    fn gbm_surface(&self) -> *mut c_void {
        self.surface.as_ptr().cast()
    }
}
