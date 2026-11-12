use std::os::fd::OwnedFd;
use std::rc::Rc;
use std::time::Duration;

use neptron_backend::{Display, DisplayError};
use neptron_gfx::gles::{GbmTarget, GlesError, GlesRenderer};
use neptron_safety::ValidatedMode;
use neptron_types::{FlipEvent, GpuCaps, Modeline, Scene};

use super::KmsError;
use super::device::{self, Card, Objects, PLANE_FB_ID};
use super::events;
use super::ffi::{self, Bo, Gbm, LinearBy};
use super::request::{self, NONBLOCK, PAGE_FLIP_EVENT, Request};
use crate::{FlipState, VblankCounter, WireMode};

/// What the caller decides for the display. The backend decides nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KmsConfig {
    /// The GPU capabilities from the capability table.
    pub caps: GpuCaps,
    /// The output named in the settings, such as `VGA-1`.
    pub output: Option<String>,
    /// The monitor profile is a CRT; it decides the connector when several
    /// are connected.
    pub crt: bool,
    /// Width of the scanout buffers: the widest mode the caller will set.
    pub buffer_width: u16,
    /// Height of the scanout buffers: the tallest mode the caller will set.
    pub buffer_height: u16,
    /// Route a writeback connector with each mode, for
    /// [`KmsDisplay::capture`]. Off in the product.
    pub writeback: bool,
}

/// The [`Display`] on DRM/KMS. `docs/spec/kms.md` defines it.
///
/// Only a [`ValidatedMode`] reaches the kernel. The display is not `Send`:
/// its GL context stays on the thread that made it.
pub struct KmsDisplay {
    // Drop order: the renderer (EGL surface and context), then the GBM
    // surface and device, then the descriptor. Drop restores first.
    pub(super) renderer: GlesRenderer,
    pub(super) gbm: Rc<Gbm>,
    pub(super) card: Card,
    pub(super) objects: Objects,
    pub(super) config: KmsConfig,
    pub(super) state: FlipState,
    pub(super) wire: WireMode,
    pub(super) framebuffers: [Option<(usize, u32)>; FB_SLOTS],
    pub(super) front: Option<Bo>,
    pub(super) queued: Option<Bo>,
    pub(super) counter: VblankCounter,
    pub(super) driver: String,
}

/// Framebuffers kept, one per GBM buffer. Mesa's GBM surface has at most
/// four buffers.
pub(super) const FB_SLOTS: usize = 4;
/// The longest wait for a page flip. The slowest field of a shipped profile
/// is about 20 ms and an interlaced frame about 40 ms; this also covers
/// `vkms` under a loaded QEMU.
pub const FLIP_WAIT_LIMIT_MS: u64 = 500;
/// The name of the variable that holds the descriptor from the launcher.
pub const DRM_FD_ENV: &str = "NEPTRON_DRM_FD";

impl KmsDisplay {
    /// The display on the descriptor that the launcher passes in
    /// `NEPTRON_DRM_FD`. A process takes it once.
    ///
    /// # Errors
    ///
    /// [`KmsError`] when the variable is missing or wrong, the descriptor
    /// is not a DRM device, or [`open`](Self::open) fails.
    pub fn from_env(config: KmsConfig) -> Result<KmsDisplay, KmsError> {
        let value = std::env::var(DRM_FD_ENV).unwrap_or_default();
        let fd = value.trim().parse::<i32>().map_err(|_| KmsError::Env {
            value: value.clone(),
        })?;
        KmsDisplay::open(ffi::take_launcher_fd(fd)?, config)
    }

    /// The display on an open DRM device. It chooses the connector, the
    /// CRTC and the plane, makes the buffers and the renderer, and sets no
    /// mode.
    ///
    /// # Errors
    ///
    /// [`KmsError`] for a descriptor that is not a DRM device with atomic
    /// modesetting, no connector by the rule, no CRTC or plane, a buffer
    /// size above the GPU limits, or a GBM or EGL failure.
    pub fn open(fd: OwnedFd, config: KmsConfig) -> Result<KmsDisplay, KmsError> {
        let (w, h) = (config.buffer_width, config.buffer_height);
        if w == 0 || h == 0 || w > config.caps.max_width || h > config.caps.max_height {
            return Err(KmsError::BufferSize {
                width: w,
                height: h,
                max_width: config.caps.max_width,
                max_height: config.caps.max_height,
            });
        }
        let card = Card::new(fd, config.writeback)?;
        let driver = card.driver_name();
        let objects = device::select(
            &card,
            config.output.as_deref(),
            config.crt,
            config.writeback,
        )?;
        let gbm = Rc::new(Gbm::new(card.duplicate()?, u32::from(w), u32::from(h))?);
        let target: Rc<dyn GbmTarget> = gbm.clone();
        let renderer = GlesRenderer::on_gbm(target)?;
        Ok(KmsDisplay {
            renderer,
            gbm,
            card,
            objects,
            config,
            state: FlipState::new(),
            wire: WireMode::new(),
            framebuffers: [None; FB_SLOTS],
            front: None,
            queued: None,
            counter: VblankCounter::new(),
            driver,
        })
    }

    /// The connector in use, such as `VGA-1`.
    pub fn connector_name(&self) -> &str {
        &self.objects.connector_name
    }

    /// The kernel id of the CRTC in use.
    pub fn crtc_id(&self) -> u32 {
        self.objects.crtc
    }

    /// The driver name, such as `i915` or `vkms`.
    pub fn driver_name(&self) -> &str {
        &self.driver
    }

    /// True when the buffers got a linear layout through the modifier
    /// list, false when through the linear usage flag.
    pub fn linear_by_modifier(&self) -> bool {
        self.gbm.linear_by() == LinearBy::Modifier
    }

    /// The framebuffer of a locked buffer: from the table, or added once.
    pub(super) fn framebuffer_for(&mut self, bo: &Bo) -> Result<u32, DisplayError> {
        let key = bo.key();
        if let Some((_, fb)) = self.framebuffers.iter().flatten().find(|(k, _)| *k == key) {
            return Ok(*fb);
        }
        let Some(slot) = self.framebuffers.iter().position(Option::is_none) else {
            return Err(DisplayError::Device {
                operation: "add a framebuffer",
                reason: format!("the GBM surface gave more than {FB_SLOTS} buffers"),
            });
        };
        let modifier = self.gbm.modifier(bo);
        if modifier != MOD_LINEAR && !(modifier == MOD_INVALID && !self.linear_by_modifier()) {
            return Err(DisplayError::Device {
                operation: "add a framebuffer",
                reason: format!("the GBM buffer has the modifier {modifier:#x}, not linear"),
            });
        }
        let info = self.gbm.info(bo);
        let fb = drm_ffi::mode::add_fb(
            std::os::fd::AsFd::as_fd(&self.card),
            info.width,
            info.height,
            info.stride,
            BITS_PER_PIXEL,
            COLOUR_DEPTH,
            info.handle,
        )
        .map_err(|e| DisplayError::Device {
            operation: "add a framebuffer",
            reason: e.to_string(),
        })?
        .fb_id;
        self.framebuffers[slot] = Some((key, fb));
        Ok(fb)
    }

    /// Locks the buffer the renderer just swapped, with its framebuffer.
    pub(super) fn lock_swapped(&mut self) -> Result<(Bo, u32), DisplayError> {
        let bo = self.gbm.lock_front().ok_or(DisplayError::Device {
            operation: "lock the GBM front buffer",
            reason: "the surface has no swapped buffer".to_owned(),
        })?;
        match self.framebuffer_for(&bo) {
            Ok(fb) => Ok((bo, fb)),
            Err(e) => {
                self.gbm.release(bo);
                Err(e)
            }
        }
    }
}

const BITS_PER_PIXEL: u32 = 32;
/// `DRM_FORMAT_MOD_LINEAR`
const MOD_LINEAR: u64 = 0;
/// `DRM_FORMAT_MOD_INVALID`: an implicit layout. A surface made with the
/// linear usage flag reports it, and its layout is linear.
const MOD_INVALID: u64 = 0x00ff_ffff_ffff_ffff;
const COLOUR_DEPTH: u32 = 24;

impl Display for KmsDisplay {
    fn caps(&self) -> &GpuCaps {
        &self.config.caps
    }

    fn set_mode(&mut self, mode: &ValidatedMode) -> Result<Modeline, DisplayError> {
        self.commit_mode(mode)
    }

    fn present(&mut self, scene: &Scene) -> Result<(), DisplayError> {
        self.state.check_present()?;
        let Some(m) = self.wire.on_wire() else {
            return Err(DisplayError::NoMode);
        };
        self.renderer
            .draw_to_window(scene, u32::from(m.hactive), u32::from(m.vactive))
            .map_err(render_error)?;
        let (bo, fb) = self.lock_swapped()?;
        let mut req = Request::new();
        req.add(
            self.objects.plane,
            self.objects.props.plane[PLANE_FB_ID],
            u64::from(fb),
        );
        if let Err(refusal) = request::commit(&req, &self.card, NONBLOCK | PAGE_FLIP_EVENT) {
            self.gbm.release(bo);
            return Err(refusal.device("page flip commit"));
        }
        self.queued = Some(bo);
        self.state.flip_queued();
        Ok(())
    }

    fn wait_flip(&mut self) -> Result<FlipEvent, DisplayError> {
        self.state.check_wait()?;
        let limit = Duration::from_millis(FLIP_WAIT_LIMIT_MS);
        match events::wait_flip(&self.card, self.objects.crtc, limit) {
            Ok(Some(flip)) => {
                self.flip_done();
                Ok(FlipEvent {
                    time_ns: flip.time_ns,
                    sequence: self.counter.extend(flip.frame),
                })
            }
            Ok(None) => Err(DisplayError::Device {
                operation: "wait for the page flip",
                reason: format!(
                    "no page-flip event within {FLIP_WAIT_LIMIT_MS} ms; the flip stays pending"
                ),
            }),
            Err(e) => Err(DisplayError::Device {
                operation: "read the DRM events",
                reason: e.to_string(),
            }),
        }
    }
}

impl KmsDisplay {
    /// The queued buffer is on the screen now; the one before goes back.
    pub(super) fn flip_done(&mut self) {
        self.state.flip_done();
        if let Some(old) = self.front.take() {
            self.gbm.release(old);
        }
        self.front = self.queued.take();
    }
}

/// A scene the renderer refuses is the caller's error; anything else is the
/// device's.
pub(super) fn render_error(e: GlesError) -> DisplayError {
    match e {
        GlesError::Scene(_)
        | GlesError::ScaleTooLarge { .. }
        | GlesError::TextureTooLarge { .. }
        | GlesError::OutputTooLarge { .. } => DisplayError::SceneRefused {
            reason: e.to_string(),
        },
        other => DisplayError::Device {
            operation: "draw the scene",
            reason: other.to_string(),
        },
    }
}
