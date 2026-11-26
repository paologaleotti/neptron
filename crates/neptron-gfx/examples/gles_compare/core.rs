//! The checks of the context of a GL core (`docs/spec/hw-render.md`,
//! section 7.3): for each context type that the renderer offers, the core
//! stand-in draws the test card into the framebuffer of the core context,
//! and the renderer draws the EGL image of it as a GPU picture. The output
//! must equal the reference renderer's output for the card in memory, in
//! both origins, at 1x, with scanlines, turned, under the UI and with
//! sharp bilinear. A core that leaves GL state dirty or draws into
//! framebuffer 0 must change no pixel.

use neptron_gfx::gles::{CoreApi, CoreOffer, CoreSpec, GlesError, GlesRenderer};
use neptron_gfx::{OutputBuffer, ReferenceRenderer};
use neptron_types::{
    Filter, FrameOrigin, GamePicture, GpuFrame, GpuImageId, PixelFormat, Rotation, Scene,
};

use crate::core_card::{CoreCard, Leave, card_pixel};
use crate::scenes::{Case, Source, rect};
use crate::script::Size;
use crate::{Diff, render};

/// The picture that the stand-in draws.
const PICTURE: Size = Size {
    width: 320,
    height: 224,
};
/// The image of the core context: larger than the picture, so the shader
/// reads a part of the storage, as for a core with a larger maximum.
const IMAGE: (u16, u16) = (384, 256);
/// The image after the resize check.
const RESIZED: (u16, u16) = (512, 512);

/// One check: its name and whether it holds.
pub type Outcome = (String, bool);

/// Runs every check of the core context and returns one outcome each.
pub fn core_checks(gles: &mut GlesRenderer, reference: &mut ReferenceRenderer) -> Vec<Outcome> {
    let mut out = Vec::new();
    let offer = gles.core_offer();
    println!("core offer: {offer:?}");
    out.push((
        "the renderer offers an OpenGL ES 2 core context".to_owned(),
        offer.gles.is_some_and(|v| v >= (2, 0)),
    ));
    for (label, spec) in specs(&offer) {
        for (depth, stencil) in [(false, false), (true, false), (true, true)] {
            let spec = CoreSpec {
                depth,
                stencil,
                ..spec
            };
            let made = gles.core_create(&spec, IMAGE.0, IMAGE.1);
            out.push((
                format!("{label}: create with depth {depth} and stencil {stencil}: {made:?}"),
                made.is_ok(),
            ));
        }
        for origin in [FrameOrigin::TopLeft, FrameOrigin::BottomLeft] {
            out.extend(picture_checks(gles, reference, label, &spec, origin));
        }
    }
    out.extend(error_checks(gles));
    gles.core_destroy();
    out
}

/// The context types to check: each that the offer holds, at its lowest
/// version, with a depth and stencil buffer.
fn specs(offer: &CoreOffer) -> Vec<(&'static str, CoreSpec)> {
    let spec = |api, major, minor| CoreSpec {
        api,
        major,
        minor,
        depth: true,
        stencil: true,
    };
    [
        ("gles2", spec(CoreApi::Gles, 2, 0)),
        ("gles3", spec(CoreApi::Gles, 3, 0)),
        ("glcore", spec(CoreApi::GlCore, 3, 2)),
        ("gl", spec(CoreApi::GlCompatibility, 2, 0)),
    ]
    .into_iter()
    .filter(|(_, s)| offer.holds(s))
    .collect()
}

/// The card in memory: what a display must show for the core.
fn card_source() -> Source {
    Source::from_fn(
        PICTURE.width as u16,
        PICTURE.height as u16,
        PixelFormat::Xrgb8888,
        |x, y| card_pixel(PICTURE, u32::from(x), u32::from(y)),
    )
}

/// The scenes of the picture checks, on the card in memory.
fn cases() -> Vec<Case> {
    let (w, h) = (PICTURE.width, PICTURE.height);
    let full = rect(0, 0, w, h);
    let ui = Source::pattern(320, 240, PixelFormat::Xrgb8888);
    vec![
        Case::game("1x", card_source(), full, full, Filter::Nearest, (w, h)),
        Case::game(
            "scanlines_2x",
            card_source(),
            full,
            rect(0, 0, 2 * w, 2 * h),
            Filter::Nearest,
            (2 * w, 2 * h),
        )
        .with_black_lines(50),
        Case::game(
            "turned_r90",
            card_source(),
            full,
            rect(0, 0, 2 * h, 2 * w),
            Filter::Nearest,
            (2 * h, 2 * w),
        )
        .turned(Rotation::R90),
        Case::game(
            "ui_over",
            card_source(),
            full,
            full,
            Filter::Nearest,
            (w, h),
        )
        .with_ui(ui, rect(160, 112, 160, 120)),
        Case::game(
            "sharp_1_5x",
            card_source(),
            full,
            rect(0, 0, w * 3 / 2, h * 3 / 2),
            Filter::SharpBilinear,
            (w * 3 / 2, h * 3 / 2),
        ),
    ]
}

/// The scene of `case` with the core image of `storage` texels in place of
/// its game frame.
fn gpu_scene(
    case: &Case,
    image: GpuImageId,
    storage: (u16, u16),
    origin: FrameOrigin,
) -> Scene<'_> {
    Scene {
        game: Some(GamePicture::Gpu(GpuFrame {
            image,
            width: PICTURE.width as u16,
            height: PICTURE.height as u16,
            storage_width: storage.0,
            storage_height: storage.1,
            origin,
        })),
        ..case.scene()
    }
}

/// Draws the card with the stand-in between `core_begin` and `core_end`.
fn draw_card(
    gles: &mut GlesRenderer,
    card: &CoreCard,
    framebuffer: u32,
    origin: FrameOrigin,
    leave: Leave,
) -> Result<(), String> {
    gles.core_begin().map_err(|e| e.to_string())?;
    let error = card.draw(
        framebuffer,
        PICTURE,
        origin == FrameOrigin::BottomLeft,
        leave,
    );
    let status = gles.core_end().map_err(|e| e.to_string())?;
    if error != 0 {
        return Err(format!("the card raised GL error {error:#06x}"));
    }
    if status.context_lost {
        return Err("the context is lost".to_owned());
    }
    Ok(())
}

/// The GLES output of `case` with the core image, against the reference
/// output of the card in memory. Prints one line, and the picture when
/// `show` is true.
fn compare_gpu(
    gles: &mut GlesRenderer,
    reference: &mut ReferenceRenderer,
    case: &Case,
    (image, storage): (GpuImageId, (u16, u16)),
    origin: FrameOrigin,
    show: Option<&str>,
) -> Result<String, String> {
    let expected = render(case, |out| {
        reference
            .render(&case.scene(), out)
            .map_err(|e| e.to_string())
    })?;
    let scene = gpu_scene(case, image, storage, origin);
    let actual = render(case, |out| {
        gles.render(&scene, out).map_err(|e| e.to_string())
    })?;
    if let Some(name) = show {
        print_picture(name, case, &actual);
    }
    let diff = Diff::of(case, &expected, &actual);
    if diff.within_tolerance() {
        Ok(diff.to_string())
    } else {
        Err(diff.to_string())
    }
}

/// Every picture check of one context type and origin.
fn picture_checks(
    gles: &mut GlesRenderer,
    reference: &mut ReferenceRenderer,
    label: &str,
    spec: &CoreSpec,
    origin: FrameOrigin,
) -> Vec<Outcome> {
    let mut out = Vec::new();
    let mut push = |name: String, result: Result<String, String>| {
        let ok = result.is_ok();
        let text = match result {
            Ok(note) | Err(note) => note,
        };
        out.push((format!("{name}: {text}"), ok));
    };
    let at = format!("{label} {origin:?}");
    let surface = match gles.core_create(spec, IMAGE.0, IMAGE.1) {
        Ok(surface) => surface,
        Err(e) => {
            push(format!("{at}: create"), Err(e.to_string()));
            return out;
        }
    };
    let card = match CoreCard::resolve(surface.lookup) {
        Ok(card) => card,
        Err(e) => {
            push(format!("{at}: lookup"), Err(e));
            return out;
        }
    };
    let drawn = draw_card(gles, &card, surface.framebuffer, origin, Leave::Clean);
    push(
        format!("{at}: the stand-in draws the card"),
        drawn.map(|()| String::new()),
    );
    let cases = cases();
    for case in &cases {
        let show = (label == "gles2").then(|| format!("{label}_{origin:?}_{}", case.name));
        let result = compare_gpu(
            gles,
            reference,
            case,
            (surface.image, IMAGE),
            origin,
            show.as_deref(),
        );
        push(format!("{at}: {}", case.name), result);
    }
    push(format!("{at}: read back"), read_back(gles, origin));

    let first = &cases[0];
    let dirty = draw_card(gles, &card, surface.framebuffer, origin, Leave::Dirty)
        .and_then(|()| compare_gpu(gles, reference, first, (surface.image, IMAGE), origin, None));
    push(format!("{at}: hw_dirty changes no pixel"), dirty);
    let zero = gles
        .core_begin()
        .map_err(|e| e.to_string())
        .and_then(|()| {
            card.draw_into_zero();
            gles.core_end().map_err(|e| e.to_string())
        })
        .and_then(|_| compare_gpu(gles, reference, first, (surface.image, IMAGE), origin, None));
    push(format!("{at}: hw_fbo0 changes no pixel"), zero);
    let twice = compare_gpu(gles, reference, first, (surface.image, IMAGE), origin, None);
    push(format!("{at}: a second present of the same picture"), twice);

    let resized = gles
        .core_resize(RESIZED.0, RESIZED.1)
        .map_err(|e| e.to_string())
        .and_then(|()| draw_card(gles, &card, surface.framebuffer, origin, Leave::Clean))
        .and_then(|()| {
            compare_gpu(
                gles,
                reference,
                first,
                (surface.image, RESIZED),
                origin,
                None,
            )
        });
    push(format!("{at}: resize keeps the framebuffer name"), resized);
    out
}

/// The read back of the picture equals the card in memory.
fn read_back(gles: &mut GlesRenderer, origin: FrameOrigin) -> Result<String, String> {
    let (w, h) = (PICTURE.width as u16, PICTURE.height as u16);
    let mut pixels = vec![0xAA_u8; usize::from(w) * usize::from(h) * 4];
    gles.core_read_back(&mut pixels, w, h, origin)
        .map_err(|e| e.to_string())?;
    for (i, px) in pixels.chunks_exact(4).enumerate() {
        let (x, y) = (i as u32 % PICTURE.width, i as u32 / PICTURE.width);
        let got = u32::from_ne_bytes([px[0], px[1], px[2], px[3]]);
        let want = card_pixel(PICTURE, x, y);
        if got != want {
            return Err(format!("({x}, {y}): {got:#010x}, {want:#010x} expected"));
        }
    }
    Ok("equal".to_owned())
}

/// The error paths of the core context.
fn error_checks(gles: &mut GlesRenderer) -> Vec<Outcome> {
    let mut out = Vec::new();
    gles.core_destroy();
    out.push((
        "begin with no core context".to_owned(),
        matches!(gles.core_begin(), Err(GlesError::NoCoreContext { .. })),
    ));
    out.push((
        "end with no core context".to_owned(),
        matches!(gles.core_end(), Err(GlesError::NoCoreContext { .. })),
    ));
    let not_offered = CoreSpec {
        api: CoreApi::GlCore,
        major: 9,
        minor: 9,
        depth: false,
        stencil: false,
    };
    out.push((
        "a version above the offer".to_owned(),
        matches!(
            gles.core_create(&not_offered, 64, 64),
            Err(GlesError::CoreNotOffered { .. })
        ),
    ));
    let gles2 = CoreSpec {
        api: CoreApi::Gles,
        major: 2,
        minor: 0,
        depth: false,
        stencil: false,
    };
    out.push((
        "an image of side 0".to_owned(),
        matches!(
            gles.core_create(&gles2, 0, 64),
            Err(GlesError::CoreSize { .. })
        ),
    ));
    let Ok(surface) = gles.core_create(&gles2, 64, 64) else {
        out.push(("create a small image".to_owned(), false));
        return out;
    };
    out.push((
        "end without begin".to_owned(),
        matches!(gles.core_end(), Err(GlesError::NoCoreContext { .. })),
    ));
    let mut small = [0_u8; 4];
    out.push((
        "a read back into a short buffer".to_owned(),
        matches!(
            gles.core_read_back(&mut small, 2, 1, FrameOrigin::TopLeft),
            Err(GlesError::CoreBufferTooSmall { .. })
        ),
    ));
    let mut big = vec![0_u8; 65 * 64 * 4];
    out.push((
        "a read back above the image".to_owned(),
        matches!(
            gles.core_read_back(&mut big, 65, 64, FrameOrigin::TopLeft),
            Err(GlesError::CoreSize { .. })
        ),
    ));
    let case = &cases()[0];
    let picture_too_large = gpu_scene(case, surface.image, (64, 64), FrameOrigin::TopLeft);
    let mut data = vec![0_u8; case.width as usize * case.height as usize * 4];
    let mut target = OutputBuffer {
        data: &mut data,
        width: case.width,
        height: case.height,
        pitch: case.width as usize * 4,
    };
    out.push((
        "a GPU picture larger than its image".to_owned(),
        matches!(
            gles.render(&picture_too_large, &mut target),
            Err(GlesError::PictureTooLarge { .. })
        ),
    ));
    gles.core_destroy();
    out.push((
        "a scene with a destroyed image".to_owned(),
        matches!(
            gles.render(&picture_too_large, &mut target),
            Err(GlesError::UnknownImage { .. })
        ),
    ));
    out
}

/// Prints the picture of `case`, `bytes` as the renderers write it, on the
/// console in runs of one colour: `PICTURE <name> <width> <height>`, then
/// lines `PICTURE-RUNS <count>x<RRGGBB> ...`, then `PICTURE-END`. The guest
/// cannot write to the shared folder, so the host takes the picture from
/// the console log.
fn print_picture(name: &str, case: &Case, bytes: &[u8]) {
    let pitch = bytes.len() / case.height.max(1) as usize;
    println!("PICTURE {name} {} {}", case.width, case.height);
    let mut runs: Vec<(u32, u32)> = Vec::new();
    for y in 0..case.height as usize {
        for x in 0..case.width as usize {
            let at = y * pitch + x * 4;
            let rgb = u32::from_be_bytes([0, bytes[at + 2], bytes[at + 1], bytes[at]]);
            match runs.last_mut() {
                Some((count, last)) if *last == rgb => *count += 1,
                _ => runs.push((1, rgb)),
            }
        }
    }
    for line in runs.chunks(16) {
        let text: Vec<String> = line.iter().map(|(n, c)| format!("{n}x{c:06X}")).collect();
        println!("PICTURE-RUNS {}", text.join(" "));
    }
    println!("PICTURE-END");
}
