//! The probe of a machine for the GL cores (`docs/spec/hw-render.md`,
//! work package W1): it prints the EGL and GL strings of the Mesa
//! surfaceless platform, the EGL extensions that the core path needs, and
//! the offer of context types and versions that NEPTRON makes on this
//! machine. Copy it to the machine and run it there; it changes nothing.
//! The exit code is 0 when the renderer starts, else 1.

#![forbid(unsafe_code)]

use std::process::ExitCode;

use neptron_gfx::gles::GlesRenderer;

/// The EGL extensions of the core path: the context with no surface, the
/// image of a texture, the fences with a wait in the GPU, the reset notice
/// of a lost context, and the display reference count of the renderer.
const EGL_EXTENSIONS: [&str; 8] = [
    "EGL_KHR_surfaceless_context",
    "EGL_KHR_image_base",
    "EGL_KHR_gl_texture_2D_image",
    "EGL_KHR_fence_sync",
    "EGL_KHR_wait_sync",
    "EGL_EXT_create_context_robustness",
    "EGL_KHR_no_config_context",
    "EGL_KHR_display_reference",
];

fn main() -> ExitCode {
    let mut gles = match GlesRenderer::headless() {
        Ok(r) => r,
        Err(e) => {
            println!("the GLES renderer does not start: {e}");
            return ExitCode::FAILURE;
        }
    };
    let egl = gles.egl_strings();
    println!("EGL vendor: {}", egl.vendor);
    println!("EGL version: {}", egl.version);
    println!("EGL client APIs: {}", egl.client_apis);
    match gles.gl_strings() {
        Ok((renderer, version)) => {
            println!("GL renderer: {renderer}");
            println!("GL version: {version}");
        }
        Err(e) => println!("GL strings: {e}"),
    }
    let list: Vec<&str> = egl.extensions.split_ascii_whitespace().collect();
    for name in EGL_EXTENSIONS {
        let found = if list.contains(&name) { "yes" } else { "no" };
        println!("{name}: {found}");
    }
    let offer = gles.core_offer();
    let show = |v: Option<(u8, u8)>| v.map_or("no".to_owned(), |(a, b)| format!("{a}.{b}"));
    println!("offer OpenGL ES: {}", show(offer.gles));
    println!("offer OpenGL core profile: {}", show(offer.gl_core));
    println!(
        "offer OpenGL compatibility profile: {}",
        show(offer.gl_compatibility)
    );
    println!("EGL extensions: {}", egl.extensions);
    ExitCode::SUCCESS
}
