use std::ffi::CStr;

use neptron_types::GpuImageId;

use super::GlesError;

/// The GL API of the context of a core.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CoreApi {
    /// OpenGL ES.
    Gles,
    /// Desktop OpenGL with the core profile.
    GlCore,
    /// Desktop OpenGL with the compatibility profile.
    GlCompatibility,
}

/// The context that a core asks for, and its buffers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CoreSpec {
    /// The API.
    pub api: CoreApi,
    /// The lowest major version that the core can use.
    pub major: u8,
    /// The lowest minor version that the core can use.
    pub minor: u8,
    /// A depth buffer.
    pub depth: bool,
    /// A stencil buffer. It counts only with `depth`.
    pub stencil: bool,
}

/// The highest version, `(major, minor)`, of each API that the renderer can
/// make for a core. `None` for an API it cannot make.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct CoreOffer {
    /// OpenGL ES.
    pub gles: Option<(u8, u8)>,
    /// Desktop OpenGL, core profile.
    pub gl_core: Option<(u8, u8)>,
    /// Desktop OpenGL, compatibility profile.
    pub gl_compatibility: Option<(u8, u8)>,
}

/// What the renderer made for a core.
#[derive(Debug, Clone, Copy)]
pub struct CoreSurface {
    /// The framebuffer object that the core draws into, a name of its
    /// context.
    pub framebuffer: u32,
    /// The image that a `GamePicture::Gpu` picture names.
    pub image: GpuImageId,
    /// The address of the GL function of a name, or 0: a process global,
    /// so a core can call it on any thread.
    pub lookup: fn(&CStr) -> usize,
}

/// What [`GlesRenderer::core_end`](super::GlesRenderer::core_end) found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CoreStatus {
    /// The GL errors read now. They are read once every
    /// [`GL_ERROR_PERIOD_FRAMES`] calls, so 0 does not prove that the core
    /// raised none.
    pub gl_errors: u32,
    /// The GPU reset the context: every object of the core is gone.
    pub context_lost: bool,
}

/// The core calls between two reads of the GL error flags and the reset
/// status. `glGetError` can stall a threaded Mesa context until its queue
/// is empty, so the read is rare: about once a second at 60 Hz.
pub const GL_ERROR_PERIOD_FRAMES: u32 = 60;

/// The most GL error flags read in one check. GL keeps one flag per kind
/// of error, so more means a fault that the read does not clear.
pub const MAX_GL_ERRORS_PER_CHECK: u32 = 16;

impl CoreOffer {
    /// The highest version of `api` on offer.
    pub fn highest(&self, api: CoreApi) -> Option<(u8, u8)> {
        match api {
            CoreApi::Gles => self.gles,
            CoreApi::GlCore => self.gl_core,
            CoreApi::GlCompatibility => self.gl_compatibility,
        }
    }

    /// True when the offer holds the API of `spec` at its version or
    /// later.
    pub fn holds(&self, spec: &CoreSpec) -> bool {
        self.highest(spec.api)
            .is_some_and(|highest| (spec.major, spec.minor) <= highest)
    }
}

/// Counts the core calls and says when to read the GL error flags.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct ErrorCadence {
    calls: u32,
}

impl ErrorCadence {
    /// Counts one call. True on every [`GL_ERROR_PERIOD_FRAMES`]-th call.
    pub(super) fn tick(&mut self) -> bool {
        self.calls = (self.calls + 1) % GL_ERROR_PERIOD_FRAMES;
        self.calls == 0
    }
}

/// The renderbuffer format of the depth and stencil buffer of `spec`, or
/// `None` for no buffer. `es2` says that the context is OpenGL ES 2, where
/// 24-bit depth and packed depth and stencil are extensions:
/// `depth24` (`GL_OES_depth24`) and `packed` (`GL_OES_packed_depth_stencil`).
/// A stencil buffer alone is not made, as libretro.h says. The error names
/// the missing extension.
pub(super) fn depth_format(
    spec: &CoreSpec,
    es2: bool,
    depth24: bool,
    packed: bool,
) -> Result<Option<DepthFormat>, &'static str> {
    if !spec.depth {
        return Ok(None);
    }
    if spec.stencil {
        return if !es2 || packed {
            Ok(Some(DepthFormat::Depth24Stencil8))
        } else {
            Err("GL_OES_packed_depth_stencil")
        };
    }
    Ok(Some(if !es2 || depth24 {
        DepthFormat::Depth24
    } else {
        DepthFormat::Depth16
    }))
}

/// The storage of the depth and stencil renderbuffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DepthFormat {
    /// 16-bit depth, no stencil.
    Depth16,
    /// 24-bit depth, no stencil.
    Depth24,
    /// Packed 24-bit depth and 8-bit stencil.
    Depth24Stencil8,
}

impl DepthFormat {
    /// The GL internal format. The ES 2 extensions use the values of
    /// desktop GL and ES 3.
    pub(super) fn gl(self) -> u32 {
        match self {
            DepthFormat::Depth16 => 0x81A5,
            DepthFormat::Depth24 => 0x81A6,
            DepthFormat::Depth24Stencil8 => 0x88F0,
        }
    }

    /// True when the format has a stencil part.
    pub(super) fn has_stencil(self) -> bool {
        self == DepthFormat::Depth24Stencil8
    }
}

/// The result of `core_end` from what it found: the `status` of the core
/// and whether the fence and the release held. A context that the GPU
/// lost fails its fence or its release too; then the error says both, so
/// the loss is not hidden behind the failure of the call.
pub(super) fn end_outcome(
    status: CoreStatus,
    fenced: Result<(), GlesError>,
    released: Result<(), GlesError>,
) -> Result<CoreStatus, GlesError> {
    match fenced.and(released) {
        Ok(()) => Ok(status),
        Err(cause) if status.context_lost => Err(GlesError::CoreContextLost {
            cause: Box::new(cause),
        }),
        Err(e) => Err(e),
    }
}

/// `EGL_CONTEXT_LOST`: a power event or a GPU reset lost the context.
pub(super) const EGL_CONTEXT_LOST: i32 = 0x300E;

/// The error of a failed `core_begin`: a make-current that EGL refuses
/// because the GPU lost the context is a lost context, which the caller
/// makes again; any other error stays as it is.
pub(super) fn begin_error(error: GlesError) -> GlesError {
    match error {
        GlesError::Egl {
            code: EGL_CONTEXT_LOST,
            ..
        } => GlesError::CoreContextLost {
            cause: Box::new(error),
        },
        other => other,
    }
}

/// The end of a setup whose last step makes another context current:
/// `made` when the step `restored` holds too. A failed setup gives its own
/// error. A setup that held, with a failed last step, gives the error of
/// that step after `undo` frees what the setup made, so nothing leaks.
pub(super) fn keep_or_undo<T, E>(
    made: Result<T, E>,
    restored: Result<(), E>,
    undo: impl FnOnce(T),
) -> Result<T, E> {
    let made = made?;
    match restored {
        Ok(()) => Ok(made),
        Err(e) => {
            undo(made);
            Err(e)
        }
    }
}

/// Copies `rows` RGBA rows of `width` pixels, read from GL with row 0 at GL
/// `y = 0`, into `out` as XRGB8888 rows from the top of the picture, by the
/// rule of `bottom_left`: picture line `y` is GL row `y`, or GL row
/// `rows - 1 - y` with `bottom_left`. The X byte is 0, as the reference
/// core GL of the testkit gives it.
pub(super) fn picture_from_gl(
    rgba: &[u8],
    width: u16,
    rows: u16,
    bottom_left: bool,
    out: &mut [u8],
) {
    let row_bytes = usize::from(width) * 4;
    for (line, dst) in out
        .chunks_exact_mut(row_bytes)
        .take(usize::from(rows))
        .enumerate()
    {
        let gl_row = if bottom_left {
            usize::from(rows) - 1 - line
        } else {
            line
        };
        let Some(src) = rgba.get(gl_row * row_bytes..(gl_row + 1) * row_bytes) else {
            continue;
        };
        for (px, texel) in dst.chunks_exact_mut(4).zip(src.chunks_exact(4)) {
            let value = u32::from_be_bytes([0, texel[0], texel[1], texel[2]]);
            px.copy_from_slice(&value.to_ne_bytes());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(depth: bool, stencil: bool) -> CoreSpec {
        CoreSpec {
            api: CoreApi::Gles,
            major: 2,
            minor: 0,
            depth,
            stencil,
        }
    }

    #[test]
    fn an_offer_holds_versions_up_to_its_highest() {
        let offer = CoreOffer {
            gles: Some((3, 1)),
            gl_core: Some((4, 6)),
            gl_compatibility: None,
        };
        let ask = |api, major, minor| CoreSpec {
            api,
            major,
            minor,
            depth: false,
            stencil: false,
        };
        assert!(offer.holds(&ask(CoreApi::Gles, 3, 1)));
        assert!(!offer.holds(&ask(CoreApi::Gles, 3, 2)));
        assert!(offer.holds(&ask(CoreApi::GlCore, 3, 3)));
        assert!(!offer.holds(&ask(CoreApi::GlCompatibility, 2, 0)));
        assert!(!CoreOffer::default().holds(&ask(CoreApi::Gles, 2, 0)));
    }

    #[test]
    fn errors_are_read_once_every_period() {
        let mut cadence = ErrorCadence::default();
        let reads: Vec<u32> = (1..=3 * GL_ERROR_PERIOD_FRAMES)
            .filter(|_| cadence.tick())
            .collect();
        assert_eq!(reads.len(), 3);
    }

    #[test]
    fn the_depth_buffer_follows_the_request_and_the_context() {
        assert_eq!(
            depth_format(&spec(false, true), true, false, false),
            Ok(None)
        );
        assert_eq!(
            depth_format(&spec(true, false), true, false, false),
            Ok(Some(DepthFormat::Depth16)),
            "ES 2 without GL_OES_depth24"
        );
        assert_eq!(
            depth_format(&spec(true, false), true, true, false),
            Ok(Some(DepthFormat::Depth24))
        );
        assert_eq!(
            depth_format(&spec(true, false), false, false, false),
            Ok(Some(DepthFormat::Depth24))
        );
        assert_eq!(
            depth_format(&spec(true, true), true, true, false),
            Err("GL_OES_packed_depth_stencil")
        );
        assert_eq!(
            depth_format(&spec(true, true), true, false, true),
            Ok(Some(DepthFormat::Depth24Stencil8))
        );
        assert_eq!(
            depth_format(&spec(true, true), false, false, false),
            Ok(Some(DepthFormat::Depth24Stencil8))
        );
        assert!(DepthFormat::Depth24Stencil8.has_stencil());
        assert!(!DepthFormat::Depth24.has_stencil());
    }

    fn egl(call: &'static str) -> GlesError {
        GlesError::Egl {
            call,
            code: 0x300E,
            message: "EGL_CONTEXT_LOST".to_owned(),
        }
    }

    #[test]
    fn a_begin_that_egl_refuses_for_a_lost_context_is_a_lost_context() {
        assert_eq!(
            begin_error(egl("eglMakeCurrent of the core")),
            GlesError::CoreContextLost {
                cause: Box::new(egl("eglMakeCurrent of the core"))
            }
        );
        let bad_access = GlesError::Egl {
            call: "eglMakeCurrent of the core",
            code: 0x3002,
            message: "EGL_BAD_ACCESS".to_owned(),
        };
        assert_eq!(begin_error(bad_access.clone()), bad_access);
    }

    #[test]
    fn a_lost_context_is_reported_with_a_failed_fence_or_release() {
        let lost = CoreStatus {
            gl_errors: 1,
            context_lost: true,
        };
        assert_eq!(end_outcome(lost, Ok(()), Ok(())), Ok(lost));
        assert_eq!(
            end_outcome(lost, Err(egl("the core fence")), Ok(())),
            Err(GlesError::CoreContextLost {
                cause: Box::new(egl("the core fence"))
            })
        );
        assert_eq!(
            end_outcome(lost, Ok(()), Err(egl("release"))),
            Err(GlesError::CoreContextLost {
                cause: Box::new(egl("release"))
            })
        );
        let text = end_outcome(lost, Ok(()), Err(egl("release")))
            .expect_err("an error")
            .to_string();
        assert!(text.contains("lost") && text.contains("release"), "{text}");
    }

    #[test]
    fn a_failed_end_of_a_live_context_gives_its_first_error() {
        let live = CoreStatus::default();
        assert_eq!(
            end_outcome(live, Err(egl("the core fence")), Err(egl("release"))),
            Err(egl("the core fence"))
        );
        assert_eq!(
            end_outcome(live, Ok(()), Err(egl("release"))),
            Err(egl("release"))
        );
    }

    #[test]
    fn a_failed_last_make_current_frees_what_the_setup_made() {
        let mut freed = Vec::new();
        let kept = keep_or_undo(Ok::<_, &str>(7), Ok(()), |v| freed.push(v));
        assert_eq!(kept, Ok(7));
        assert!(freed.is_empty());

        let lost = keep_or_undo(Ok(7), Err("make current"), |v| freed.push(v));
        assert_eq!(lost, Err("make current"));
        assert_eq!(freed, [7], "the setup is freed once");

        freed.clear();
        let failed = keep_or_undo(Err::<u32, _>("setup"), Err("make current"), |v| {
            freed.push(v);
        });
        assert_eq!(failed, Err("setup"), "the error of the setup comes first");
        assert!(freed.is_empty(), "a failed setup frees its own objects");
    }

    #[test]
    fn a_read_back_puts_line_0_by_the_origin() {
        // Two GL rows of one pixel: row 0 red, row 1 blue.
        let rgba = [0xFF, 0, 0, 0x80, 0, 0, 0xFF, 0x80];
        let mut out = [0xAA_u8; 8];
        picture_from_gl(&rgba, 1, 2, false, &mut out);
        assert_eq!(
            u32::from_ne_bytes(out[..4].try_into().unwrap()),
            0x00FF_0000
        );
        assert_eq!(
            u32::from_ne_bytes(out[4..].try_into().unwrap()),
            0x0000_00FF
        );
        picture_from_gl(&rgba, 1, 2, true, &mut out);
        assert_eq!(
            u32::from_ne_bytes(out[..4].try_into().unwrap()),
            0x0000_00FF
        );
        assert_eq!(
            u32::from_ne_bytes(out[4..].try_into().unwrap()),
            0x00FF_0000
        );
    }
}
