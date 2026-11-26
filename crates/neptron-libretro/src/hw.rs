//! Hardware rendering: the context that a core asks for with
//! `SET_HW_RENDER`, and the context type that the host prefers.

use neptron_backend::{ContextOffer, ContextRequest, GlApi, GlVersion};
use neptron_libretro_sys::{
    RETRO_HW_CONTEXT_OPENGL, RETRO_HW_CONTEXT_OPENGL_CORE, RETRO_HW_CONTEXT_OPENGLES_VERSION,
    RETRO_HW_CONTEXT_OPENGLES2, RETRO_HW_CONTEXT_OPENGLES3, RETRO_HW_CONTEXT_VULKAN,
};
use neptron_types::FrameOrigin;

/// The API family of a context type, as a notice names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HwApi {
    /// Desktop OpenGL, any profile.
    OpenGl,
    /// OpenGL ES.
    OpenGles,
    /// Vulkan.
    Vulkan,
    /// Another API: Direct3D, RSX or a value that libretro.h does not
    /// define.
    Other,
}

/// Why the host refused a `SET_HW_RENDER` call.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HwRefusalReason {
    /// The offer of the display does not hold the type or the version.
    NotOffered,
    /// The call came outside `retro_load_game`.
    OutsideLoad,
}

/// A `SET_HW_RENDER` call that the host refused, kept for the notice of a
/// game that does not start.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HwRefusal {
    /// The `retro_hw_context_type` value of the call.
    pub context_type: u32,
    /// Its API family.
    pub api: HwApi,
    /// The major version that the type and the version fields ask for.
    pub major: u32,
    /// The minor version.
    pub minor: u32,
    /// Why the host refused it.
    pub reason: HwRefusalReason,
}

/// The fields of a `retro_hw_render_callback` that the host decides on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RawHwRender {
    pub context_type: u32,
    pub version_major: u32,
    pub version_minor: u32,
    pub depth: bool,
    pub stencil: bool,
    pub bottom_left_origin: bool,
}

/// The request of `raw`, or the API and version of a request that no
/// display can make: Vulkan, another API, or a version above 255.
///
/// libretro.h uses the version fields only for `OPENGL_CORE` and
/// `OPENGLES_VERSION`. `OPENGL` is version 2.0 with the compatibility
/// profile, `OPENGLES2` is 2.0 and `OPENGLES3` is 3.0. A stencil buffer
/// counts only with a depth buffer, as the header says.
pub(crate) fn request_of(raw: &RawHwRender) -> Result<ContextRequest, (HwApi, u32, u32)> {
    let given = (raw.version_major, raw.version_minor);
    let (api, (major, minor)) = match raw.context_type {
        RETRO_HW_CONTEXT_OPENGL => (GlApi::GlCompatibility, (2, 0)),
        RETRO_HW_CONTEXT_OPENGLES2 => (GlApi::Gles, (2, 0)),
        RETRO_HW_CONTEXT_OPENGLES3 => (GlApi::Gles, (3, 0)),
        RETRO_HW_CONTEXT_OPENGLES_VERSION => (GlApi::Gles, given),
        RETRO_HW_CONTEXT_OPENGL_CORE => (GlApi::GlCore, given),
        RETRO_HW_CONTEXT_VULKAN => return Err((HwApi::Vulkan, given.0, given.1)),
        _ => return Err((HwApi::Other, given.0, given.1)),
    };
    let family = match api {
        GlApi::Gles => HwApi::OpenGles,
        GlApi::GlCore | GlApi::GlCompatibility => HwApi::OpenGl,
    };
    let version = u8::try_from(major)
        .ok()
        .zip(u8::try_from(minor).ok())
        .map(|(major, minor)| GlVersion::new(major, minor))
        .ok_or((family, major, minor))?;
    Ok(ContextRequest {
        api,
        version,
        depth: raw.depth,
        stencil: raw.depth && raw.stencil,
        origin: if raw.bottom_left_origin {
            FrameOrigin::BottomLeft
        } else {
            FrameOrigin::TopLeft
        },
    })
}

/// The API family and the version of a request, for a notice.
pub(crate) fn describe(request: &ContextRequest) -> (HwApi, u32, u32) {
    let api = match request.api {
        GlApi::Gles => HwApi::OpenGles,
        GlApi::GlCore | GlApi::GlCompatibility => HwApi::OpenGl,
    };
    (
        api,
        u32::from(request.version.major),
        u32::from(request.version.minor),
    )
}

/// The context type for `GET_PREFERRED_HW_RENDER`: desktop GL with the
/// core profile where the display has it, else GLES 3, else GLES 2, else
/// desktop GL with the compatibility profile. `None` for an empty offer.
pub(crate) fn preferred(offer: &ContextOffer) -> Option<u32> {
    if offer.gl_core.is_some() {
        return Some(RETRO_HW_CONTEXT_OPENGL_CORE);
    }
    match offer.gles {
        Some(v) if v >= GlVersion::new(3, 0) => return Some(RETRO_HW_CONTEXT_OPENGLES3),
        Some(v) if v >= GlVersion::new(2, 0) => return Some(RETRO_HW_CONTEXT_OPENGLES2),
        _ => {}
    }
    offer
        .gl_compatibility
        .is_some()
        .then_some(RETRO_HW_CONTEXT_OPENGL)
}

#[cfg(test)]
mod tests {
    use neptron_libretro_sys::{RETRO_HW_CONTEXT_D3D11, RETRO_HW_CONTEXT_NONE};

    use super::*;

    fn raw(context_type: u32, major: u32, minor: u32) -> RawHwRender {
        RawHwRender {
            context_type,
            version_major: major,
            version_minor: minor,
            depth: false,
            stencil: false,
            bottom_left_origin: false,
        }
    }

    fn api_version(raw: &RawHwRender) -> (GlApi, GlVersion) {
        let r = request_of(raw).unwrap();
        (r.api, r.version)
    }

    #[test]
    fn each_gl_type_maps_to_an_api_and_a_version() {
        let v = GlVersion::new;
        assert_eq!(
            api_version(&raw(RETRO_HW_CONTEXT_OPENGL, 9, 9)),
            (GlApi::GlCompatibility, v(2, 0))
        );
        assert_eq!(
            api_version(&raw(RETRO_HW_CONTEXT_OPENGLES2, 9, 9)),
            (GlApi::Gles, v(2, 0))
        );
        assert_eq!(
            api_version(&raw(RETRO_HW_CONTEXT_OPENGLES3, 0, 0)),
            (GlApi::Gles, v(3, 0))
        );
        assert_eq!(
            api_version(&raw(RETRO_HW_CONTEXT_OPENGLES_VERSION, 3, 1)),
            (GlApi::Gles, v(3, 1))
        );
        assert_eq!(
            api_version(&raw(RETRO_HW_CONTEXT_OPENGL_CORE, 3, 3)),
            (GlApi::GlCore, v(3, 3))
        );
    }

    #[test]
    fn other_apis_and_huge_versions_have_no_request() {
        assert_eq!(
            request_of(&raw(RETRO_HW_CONTEXT_VULKAN, 1, 2)),
            Err((HwApi::Vulkan, 1, 2))
        );
        assert_eq!(
            request_of(&raw(RETRO_HW_CONTEXT_D3D11, 0, 0)),
            Err((HwApi::Other, 0, 0))
        );
        assert_eq!(
            request_of(&raw(RETRO_HW_CONTEXT_NONE, 0, 0)),
            Err((HwApi::Other, 0, 0))
        );
        assert_eq!(request_of(&raw(u32::MAX, 0, 0)), Err((HwApi::Other, 0, 0)));
        assert_eq!(
            request_of(&raw(RETRO_HW_CONTEXT_OPENGL_CORE, 256, 0)),
            Err((HwApi::OpenGl, 256, 0))
        );
        assert_eq!(
            request_of(&raw(RETRO_HW_CONTEXT_OPENGLES_VERSION, 3, 300)),
            Err((HwApi::OpenGles, 3, 300))
        );
    }

    #[test]
    fn stencil_counts_only_with_depth_and_the_origin_follows_the_flag() {
        let with = |depth, stencil, bottom_left_origin| {
            request_of(&RawHwRender {
                depth,
                stencil,
                bottom_left_origin,
                ..raw(RETRO_HW_CONTEXT_OPENGLES2, 0, 0)
            })
            .unwrap()
        };
        let r = with(false, true, false);
        assert_eq!(
            (r.depth, r.stencil, r.origin),
            (false, false, FrameOrigin::TopLeft)
        );
        let r = with(true, true, true);
        assert_eq!(
            (r.depth, r.stencil, r.origin),
            (true, true, FrameOrigin::BottomLeft)
        );
        let r = with(true, false, false);
        assert_eq!((r.depth, r.stencil), (true, false));
    }

    #[test]
    fn the_preferred_type_follows_the_offer() {
        let v = |major, minor| Some(GlVersion::new(major, minor));
        let pc = ContextOffer {
            gles: v(3, 2),
            gl_core: v(4, 6),
            gl_compatibility: v(4, 6),
        };
        assert_eq!(preferred(&pc), Some(RETRO_HW_CONTEXT_OPENGL_CORE));
        let pi4 = ContextOffer {
            gles: v(3, 1),
            ..ContextOffer::NONE
        };
        assert_eq!(preferred(&pi4), Some(RETRO_HW_CONTEXT_OPENGLES3));
        let pi3 = ContextOffer {
            gles: v(2, 0),
            ..ContextOffer::NONE
        };
        assert_eq!(preferred(&pi3), Some(RETRO_HW_CONTEXT_OPENGLES2));
        let old = ContextOffer {
            gles: v(1, 1),
            gl_compatibility: v(2, 1),
            ..ContextOffer::NONE
        };
        assert_eq!(preferred(&old), Some(RETRO_HW_CONTEXT_OPENGL));
        assert_eq!(preferred(&ContextOffer::NONE), None);
    }

    #[test]
    fn a_request_describes_its_family_and_version() {
        let r = request_of(&raw(RETRO_HW_CONTEXT_OPENGL_CORE, 3, 3)).unwrap();
        assert_eq!(describe(&r), (HwApi::OpenGl, 3, 3));
        let r = request_of(&raw(RETRO_HW_CONTEXT_OPENGLES3, 0, 0)).unwrap();
        assert_eq!(describe(&r), (HwApi::OpenGles, 3, 0));
    }
}
