//! The fake display as the host of a GL core context, on the reference
//! core GL.

use neptron_backend::{
    ContextOffer, ContextRequest, CoreFrameStatus, CoreRender, CoreRenderError, Display,
    DisplayError, GlApi, GlVersion,
};
use neptron_safety::ValidatedMode;
use neptron_testkit::gl::{bind_framebuffer, clear, clear_color, enable, scissor};
use neptron_testkit::preset::{CAPS_LOW_CLOCK, MODE_240P, validated_mode};
use neptron_testkit::{
    CORE_MAX_SIDE_PX, CoreCall, DisplayOptions, FakeDisplay, GL_INVALID_ENUM, Render, SimClock,
    gl_lookup,
};
use neptron_types::{
    Filter, Frame, FrameOrigin, GamePicture, GpuFrame, GpuImageId, Modeline, PixelFormat, Rect,
    Rotation, Scanlines, Scene,
};

const GL_FRAMEBUFFER: u32 = 0x8D40;
const GL_SCISSOR_TEST: u32 = 0x0C11;
const GL_COLOR_BUFFER_BIT: u32 = 0x4000;

const GLES2: ContextRequest = ContextRequest {
    api: GlApi::Gles,
    version: GlVersion::new(2, 0),
    depth: false,
    stencil: false,
    origin: FrameOrigin::TopLeft,
};

const PI3: ContextOffer = ContextOffer {
    gles: Some(GlVersion::new(2, 0)),
    gl_core: None,
    gl_compatibility: None,
};

fn mode(m: Modeline) -> ValidatedMode {
    validated_mode(&m, "generic_15", &CAPS_LOW_CLOCK).unwrap()
}

fn display(offer: Option<ContextOffer>) -> FakeDisplay {
    FakeDisplay::with_options(
        SimClock::new(),
        CAPS_LOW_CLOCK,
        DisplayOptions {
            render: Render::Scene,
            core_offer: offer,
            ..DisplayOptions::default()
        },
    )
}

/// A scene that shows a 4x2 picture over the whole 320x240 mode.
fn scene(game: GamePicture<'_>) -> Scene<'_> {
    Scene {
        game: Some(game),
        crop: Rect {
            x: 0,
            y: 0,
            w: 4,
            h: 2,
        },
        dest: Rect {
            x: 0,
            y: 0,
            w: 320,
            h: 240,
        },
        filter: Filter::Nearest,
        scanlines: Scanlines::NONE,
        ui: None,
        ui_dest: Rect {
            x: 0,
            y: 0,
            w: 0,
            h: 0,
        },
        rotation: Rotation::R0,
        ui_rotation: Rotation::R0,
        ui_filter: Filter::Nearest,
    }
}

fn gpu(image: GpuImageId, origin: FrameOrigin) -> GamePicture<'static> {
    GamePicture::Gpu(GpuFrame {
        image,
        width: 4,
        height: 2,
        storage_width: 8,
        storage_height: 4,
        origin,
    })
}

/// Draws, as a core does: red over the whole image, then green over GL
/// row 0, columns 0 and 1.
fn draw(framebuffer: u32) {
    bind_framebuffer(GL_FRAMEBUFFER, framebuffer);
    clear_color(1.0, 0.0, 0.0, 1.0);
    clear(GL_COLOR_BUFFER_BIT);
    enable(GL_SCISSOR_TEST);
    scissor(0, 0, 2, 1);
    clear_color(0.0, 1.0, 0.0, 1.0);
    clear(GL_COLOR_BUFFER_BIT);
}

#[test]
fn a_display_without_an_offer_hosts_no_core() {
    let mut d = display(None);
    assert!(d.core_render().is_none());
    let mut d = display(Some(PI3));
    let core = d.core_render().expect("an offer hosts a core");
    assert_eq!(core.offer(), PI3);
}

#[test]
fn create_checks_the_offer_the_size_and_the_faults() {
    let mut d = display(Some(PI3));
    let gles3 = ContextRequest {
        version: GlVersion::new(3, 0),
        ..GLES2
    };
    assert_eq!(
        d.create(&gles3, 8, 4).unwrap_err(),
        CoreRenderError::NotOffered { request: gles3 }
    );
    let side = CORE_MAX_SIDE_PX + 1;
    assert!(matches!(
        d.create(&GLES2, side, 4),
        Err(CoreRenderError::Size { width, .. }) if width == side
    ));
    assert!(matches!(
        d.create(&GLES2, 8, 0),
        Err(CoreRenderError::Size { .. })
    ));
    d.fault_core_create("EGL_BAD_ALLOC".to_owned());
    assert_eq!(
        d.create(&GLES2, 8, 4).unwrap_err(),
        CoreRenderError::Driver {
            operation: "create",
            reason: "EGL_BAD_ALLOC".to_owned()
        }
    );
    let target = d.create(&GLES2, 8, 4).unwrap();
    assert_eq!((target.lookup)(c"glClear"), gl_lookup(c"glClear"));
    let calls = d.core_calls();
    assert_eq!(calls.len(), 5);
    assert!(matches!(
        calls[4].call,
        CoreCall::Create { result: Ok(id), .. } if id == target.image
    ));
}

#[test]
fn frames_need_a_context_and_report_errors() {
    let mut d = display(Some(PI3));
    assert_eq!(
        d.begin(),
        Err(CoreRenderError::NoContext { operation: "begin" })
    );
    assert_eq!(
        d.end(),
        Err(CoreRenderError::NoContext { operation: "end" })
    );
    assert!(matches!(
        d.resize(8, 8),
        Err(CoreRenderError::NoContext { .. })
    ));
    d.create(&GLES2, 8, 4).unwrap();
    d.begin().unwrap();
    enable(0xFFFF);
    assert_eq!(
        d.end(),
        Ok(CoreFrameStatus {
            gl_errors: 1,
            context_lost: false
        })
    );
    assert_eq!(neptron_testkit::gl::get_error(), 0, "no context: no answer");
    d.begin().unwrap();
    assert_eq!(
        neptron_testkit::gl::get_error(),
        GL_INVALID_ENUM,
        "the core keeps its flag"
    );
    d.end().unwrap();
}

#[test]
fn a_gpu_picture_shows_as_its_memory_frame_by_its_origin() {
    let mut d = display(Some(PI3));
    d.set_mode(&mode(MODE_240P)).unwrap();
    let target = d.create(&GLES2, 8, 4).unwrap();
    d.begin().unwrap();
    draw(target.framebuffer);
    d.end().unwrap();

    let (red, green) = (0x00FF_0000_u32, 0x0000_FF00_u32);
    // GL row 0 is picture line 0 with a top-left origin.
    let top_left = [green, green, red, red, red, red, red, red];
    let bottom_left = [red, red, red, red, green, green, red, red];
    for (origin, pixels) in [
        (FrameOrigin::TopLeft, top_left),
        (FrameOrigin::BottomLeft, bottom_left),
    ] {
        d.present(&scene(gpu(target.image, origin))).unwrap();
        d.wait_flip().unwrap();
        let shown = d.screen().unwrap();
        let bytes: Vec<u8> = pixels.iter().flat_map(|p| p.to_ne_bytes()).collect();
        let memory = Frame {
            data: &bytes,
            width: 4,
            height: 2,
            pitch: 16,
            format: PixelFormat::Xrgb8888,
        };
        d.present(&scene(GamePicture::Memory(memory))).unwrap();
        d.wait_flip().unwrap();
        assert_eq!(d.screen().unwrap(), shown, "{origin:?}");

        let frames = d.frames();
        let (gpu_frame, memory_frame) = (&frames[frames.len() - 2], &frames[frames.len() - 1]);
        assert_eq!(gpu_frame.scene.gpu_origin, Some(origin));
        assert_eq!(gpu_frame.scene.game, Some((4, 2, PixelFormat::Xrgb8888)));
        assert_eq!(memory_frame.scene.gpu_origin, None);
        assert_ne!(gpu_frame.hash, memory_frame.hash, "the kind is in the hash");
    }
    let frames = d.frames();
    assert_ne!(frames[0].hash, frames[2].hash, "so is the origin");

    let mut out = vec![0_u8; 32];
    d.read_back(&mut out, 4, 2, FrameOrigin::BottomLeft)
        .unwrap();
    let read: Vec<u32> = out
        .chunks_exact(4)
        .map(|b| u32::from_ne_bytes([b[0], b[1], b[2], b[3]]))
        .collect();
    assert_eq!(read, bottom_left);
    assert_eq!(
        d.read_back(&mut out[..31], 4, 2, FrameOrigin::TopLeft),
        Err(CoreRenderError::BufferTooSmall { len: 31, min: 32 })
    );
    assert!(matches!(
        d.read_back(&mut out, 9, 1, FrameOrigin::TopLeft),
        Err(CoreRenderError::Size { .. })
    ));
}

#[test]
fn an_image_that_is_not_there_is_refused() {
    let mut d = display(Some(PI3));
    d.set_mode(&mode(MODE_240P)).unwrap();
    let refused = |d: &mut FakeDisplay, picture| {
        matches!(
            d.present(&scene(picture)),
            Err(DisplayError::SceneRefused { .. })
        )
    };
    assert!(
        refused(&mut d, gpu(GpuImageId(1), FrameOrigin::TopLeft)),
        "none made"
    );
    let target = d.create(&GLES2, 8, 4).unwrap();
    assert!(refused(&mut d, gpu(GpuImageId(99), FrameOrigin::TopLeft)));
    let too_large = GamePicture::Gpu(GpuFrame {
        image: target.image,
        width: 9,
        height: 2,
        storage_width: 8,
        storage_height: 4,
        origin: FrameOrigin::TopLeft,
    });
    assert!(refused(&mut d, too_large));

    d.fault_core_loss(1);
    d.begin().unwrap();
    assert!(!d.end().unwrap().context_lost);
    d.begin().unwrap();
    assert!(d.end().unwrap().context_lost, "the scripted loss");
    assert!(
        refused(&mut d, gpu(target.image, FrameOrigin::TopLeft)),
        "lost"
    );
    assert!(matches!(d.begin(), Err(CoreRenderError::NoContext { .. })));

    let again = d.create(&GLES2, 8, 4).unwrap();
    assert_ne!(again.image, target.image);
    d.present(&scene(gpu(again.image, FrameOrigin::TopLeft)))
        .unwrap();
    d.wait_flip().unwrap();
    d.destroy();
    assert!(
        refused(&mut d, gpu(again.image, FrameOrigin::TopLeft)),
        "destroyed"
    );
}

#[test]
fn a_resize_keeps_the_framebuffer_and_the_image() {
    let mut d = display(Some(PI3));
    d.set_mode(&mode(MODE_240P)).unwrap();
    let target = d.create(&GLES2, 4, 2).unwrap();
    d.resize(8, 4).unwrap();
    d.begin().unwrap();
    draw(target.framebuffer);
    d.end().unwrap();
    d.present(&scene(gpu(target.image, FrameOrigin::TopLeft)))
        .unwrap();
    assert!(matches!(
        d.resize(CORE_MAX_SIDE_PX + 1, 4),
        Err(CoreRenderError::Size { .. })
    ));
    let calls: Vec<u64> = d.core_calls().iter().map(|c| c.presents_before).collect();
    assert_eq!(
        calls,
        [0, 0, 0, 0, 1],
        "the last resize came after a present"
    );
}
