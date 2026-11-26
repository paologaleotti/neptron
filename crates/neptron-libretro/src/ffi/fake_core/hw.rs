//! Hardware rendering through the fake core: the answers to
//! `SET_HW_RENDER` and its sibling calls, hardware frames, the two
//! callbacks of the frontend and the context callbacks of the core.

use std::ffi::{CStr, c_void};
use std::sync::Mutex;

use neptron_backend::{ContextOffer, ContextRequest, GlApi, GlVersion};
use neptron_types::FrameOrigin;

use super::*;
use crate::{HwApi, HwRefusal, HwRefusalReason};

/// What the last `SET_HW_RENDER` of a hook returned, and the struct after
/// the call.
static ANSWER: Mutex<Option<(bool, retro_hw_render_callback)>> = Mutex::new(None);

fn answer() -> Option<(bool, retro_hw_render_callback)> {
    *ANSWER.lock().unwrap_or_else(|e| e.into_inner())
}

extern "C" fn context_reset() {
    record("context_reset");
}

extern "C" fn context_destroy() {
    record("context_destroy");
}

fn callback(context_type: u32, major: u32, minor: u32) -> retro_hw_render_callback {
    retro_hw_render_callback {
        context_type,
        context_reset: Some(context_reset),
        get_current_framebuffer: None,
        get_proc_address: None,
        depth: true,
        stencil: true,
        bottom_left_origin: true,
        version_major: major,
        version_minor: minor,
        cache_context: false,
        context_destroy: Some(context_destroy),
        debug_context: false,
    }
}

fn ask(cb: &Callbacks, mut request: retro_hw_render_callback) {
    let ok = cb.env(RETRO_ENVIRONMENT_SET_HW_RENDER, &raw mut request);
    *ANSWER.lock().unwrap_or_else(|e| e.into_inner()) = Some((ok, request));
}

const PC: ContextOffer = ContextOffer {
    gles: Some(GlVersion::new(3, 2)),
    gl_core: Some(GlVersion::new(4, 6)),
    gl_compatibility: Some(GlVersion::new(4, 6)),
};

const PI3: ContextOffer = ContextOffer {
    gles: Some(GlVersion::new(2, 0)),
    gl_core: None,
    gl_compatibility: None,
};

/// A host with the game loaded after `offer` was set; `on_load` runs in
/// `retro_load_game`.
fn loaded_with(offer: ContextOffer, on_load: Hook) -> Host {
    script(|s| {
        s.on_set_environment = Some(|cb| {
            let mut yes = true;
            assert!(cb.env(RETRO_ENVIRONMENT_SET_SUPPORT_NO_GAME, &raw mut yes));
        });
        s.on_load = Some(on_load);
    });
    let mut host = open().unwrap();
    host.set_hw_offer(offer);
    host.load_game(None).unwrap();
    host
}

fn gl_core_33() -> ContextRequest {
    ContextRequest {
        api: GlApi::GlCore,
        version: GlVersion::new(3, 3),
        depth: true,
        stencil: true,
        origin: FrameOrigin::BottomLeft,
    }
}

#[test]
fn an_offered_context_is_accepted_and_gets_the_callbacks() {
    let host = loaded_with(PC, |cb| {
        ask(cb, callback(RETRO_HW_CONTEXT_OPENGL_CORE, 3, 3))
    });
    let (ok, filled) = answer().unwrap();
    assert!(ok);
    assert!(filled.get_current_framebuffer.is_some());
    assert!(filled.get_proc_address.is_some());
    assert_eq!(
        filled.version_major, 3,
        "the rest stays as the core wrote it"
    );
    assert!(filled.context_reset.is_some());
    assert_eq!(host.hw_request(), Some(gl_core_33()));
    assert_eq!(host.hw_refusal(), None);
}

#[test]
fn a_context_that_the_offer_lacks_is_refused_and_kept() {
    let host = loaded_with(PI3, |cb| {
        ask(cb, callback(RETRO_HW_CONTEXT_OPENGLES3, 0, 0))
    });
    let (ok, untouched) = answer().unwrap();
    assert!(!ok);
    assert!(untouched.get_current_framebuffer.is_none());
    assert_eq!(host.hw_request(), None);
    assert_eq!(
        host.hw_refusal(),
        Some(HwRefusal {
            context_type: RETRO_HW_CONTEXT_OPENGLES3,
            api: HwApi::OpenGles,
            major: 3,
            minor: 0,
            reason: HwRefusalReason::NotOffered,
        })
    );
}

#[test]
fn vulkan_and_an_empty_offer_are_refused() {
    let host = loaded_with(PC, |cb| ask(cb, callback(RETRO_HW_CONTEXT_VULKAN, 1, 1)));
    assert!(!answer().unwrap().0);
    let refusal = host.hw_refusal().unwrap();
    assert_eq!(
        (refusal.api, refusal.reason),
        (HwApi::Vulkan, HwRefusalReason::NotOffered)
    );
    drop(host);

    let host = loaded_with(ContextOffer::NONE, |cb| {
        ask(cb, callback(RETRO_HW_CONTEXT_OPENGLES2, 0, 0));
    });
    assert!(!answer().unwrap().0, "no offer: today's answer");
    assert_eq!(host.hw_request(), None);
}

#[test]
fn a_call_outside_the_load_is_refused() {
    script(|s| s.on_run = Some(|cb| ask(cb, callback(RETRO_HW_CONTEXT_OPENGLES2, 0, 0))));
    let mut host = loaded_with(PC, |_| {});
    host.run_frame().unwrap();
    assert!(!answer().unwrap().0);
    assert_eq!(host.hw_request(), None);
    assert_eq!(
        host.hw_refusal().map(|r| r.reason),
        Some(HwRefusalReason::OutsideLoad)
    );
}

#[test]
fn a_call_outside_the_load_keeps_an_accepted_context() {
    script(|s| s.on_run = Some(|cb| ask(cb, callback(RETRO_HW_CONTEXT_VULKAN, 1, 0))));
    let mut host = loaded_with(PC, |cb| {
        ask(cb, callback(RETRO_HW_CONTEXT_OPENGL_CORE, 3, 3))
    });
    host.run_frame().unwrap();
    assert!(!answer().unwrap().0);
    assert_eq!(host.hw_request(), Some(gl_core_33()));
    assert_eq!(host.hw_refusal(), None);
}

#[test]
fn the_last_call_of_the_load_wins() {
    let host = loaded_with(PC, |cb| {
        ask(cb, callback(RETRO_HW_CONTEXT_OPENGL_CORE, 5, 0));
        ask(cb, callback(RETRO_HW_CONTEXT_OPENGL_CORE, 3, 3));
    });
    assert_eq!(host.hw_request(), Some(gl_core_33()));
    assert_eq!(
        host.hw_refusal(),
        None,
        "an accepted call clears the refusal"
    );
    drop(host);

    let host = loaded_with(PC, |cb| {
        ask(cb, callback(RETRO_HW_CONTEXT_OPENGL_CORE, 3, 3));
        ask(cb, callback(RETRO_HW_CONTEXT_OPENGL_CORE, 5, 0));
    });
    assert_eq!(host.hw_request(), None, "a refused call drops the request");
    assert_eq!(host.hw_refusal().map(|r| (r.major, r.minor)), Some((5, 0)));
}

#[test]
fn a_second_load_reports_nothing_of_a_refused_load() {
    script(|s| {
        s.on_set_environment = Some(|cb| {
            let mut yes = true;
            assert!(cb.env(RETRO_ENVIRONMENT_SET_SUPPORT_NO_GAME, &raw mut yes));
        });
        s.load_result = false;
        s.on_load = Some(|cb| ask(cb, callback(RETRO_HW_CONTEXT_OPENGLES3, 0, 0)));
    });
    let mut host = open().unwrap();
    host.set_hw_offer(PI3);
    assert!(matches!(host.load_game(None), Err(LoadError::Refused)));
    assert!(
        host.hw_refusal().is_some(),
        "the refused load keeps its refusal"
    );

    script(|s| {
        s.load_result = true;
        s.on_load = None;
    });
    host.load_game(None).unwrap();
    assert_eq!(host.hw_refusal(), None);
    assert_eq!(host.hw_request(), None);
    drop(host);

    script(|s| {
        s.load_result = false;
        s.on_load = Some(|cb| ask(cb, callback(RETRO_HW_CONTEXT_OPENGLES2, 0, 0)));
    });
    let mut host = open().unwrap();
    host.set_hw_offer(PI3);
    assert!(matches!(host.load_game(None), Err(LoadError::Refused)));
    assert!(host.hw_request().is_some());
    script(|s| {
        s.load_result = true;
        s.on_load = None;
    });
    host.load_game(None).unwrap();
    assert_eq!(
        host.hw_request(),
        None,
        "a software load after a refused hardware load"
    );
}

#[test]
fn a_hardware_frame_is_checked_as_a_memory_frame() {
    script(|s| {
        s.on_run = Some(|cb| {
            let frame = super::super::RETRO_HW_FRAME_BUFFER_VALID;
            // SAFETY: the host must not read the marker.
            unsafe { (cb.video_refresh.unwrap())(frame, 320, 240, 0) };
        });
    });
    let mut host = loaded_with(PC, |cb| ask(cb, callback(RETRO_HW_CONTEXT_OPENGLES2, 0, 0)));
    assert_eq!(
        host.run_frame().unwrap().video,
        Video::Hardware {
            width: 320,
            height: 240
        }
    );
    script(|s| {
        s.on_run = Some(|cb| {
            let frame = super::super::RETRO_HW_FRAME_BUFFER_VALID;
            // SAFETY: the host must not read the marker.
            unsafe { (cb.video_refresh.unwrap())(frame, 321, 240, 0) };
        });
    });
    assert_eq!(
        host.run_frame().unwrap().video,
        Video::Rejected(FrameFault::OverMaximum {
            width: 321,
            height: 240,
            max_width: 320,
            max_height: 240
        })
    );
    script(|s| s.on_run = Some(|cb| cb.video(None, 320, 240, 0)));
    assert_eq!(host.run_frame().unwrap().video, Video::Duplicate(None));
}

fn ask_preferred(cb: &Callbacks) {
    let mut preferred = u32::MAX;
    let ok = cb.env(
        RETRO_ENVIRONMENT_GET_PREFERRED_HW_RENDER,
        &raw mut preferred,
    );
    record(&format!("preferred {ok} {preferred}"));
}

#[test]
fn the_preferred_type_follows_the_offer() {
    drop(loaded_with(PI3, ask_preferred));
    drop(loaded_with(PC, ask_preferred));
    drop(loaded_with(ContextOffer::NONE, ask_preferred));
    let calls: Vec<String> = seen()
        .calls
        .into_iter()
        .filter(|c| c.starts_with("preferred"))
        .collect();
    assert_eq!(
        calls,
        [
            format!("preferred true {RETRO_HW_CONTEXT_OPENGLES2}"),
            format!("preferred true {RETRO_HW_CONTEXT_OPENGL_CORE}"),
            format!("preferred false {}", u32::MAX),
        ]
    );
}

#[test]
fn the_other_hardware_calls_are_refused() {
    let host = loaded_with(PC, |cb| {
        let mut data = [0_u64; 8];
        for cmd in [
            RETRO_ENVIRONMENT_GET_HW_RENDER_INTERFACE,
            RETRO_ENVIRONMENT_SET_HW_RENDER_CONTEXT_NEGOTIATION_INTERFACE,
            RETRO_ENVIRONMENT_GET_HW_RENDER_CONTEXT_NEGOTIATION_INTERFACE_SUPPORT,
            RETRO_ENVIRONMENT_SET_HW_SHARED_CONTEXT,
        ] {
            record(&format!("{cmd} {}", cb.env(cmd, data.as_mut_ptr())));
        }
        assert!(!cb.env(
            RETRO_ENVIRONMENT_SET_HW_RENDER,
            std::ptr::null_mut::<c_void>()
        ));
    });
    drop(host);
    let calls = seen().calls;
    for cmd in [
        RETRO_ENVIRONMENT_GET_HW_RENDER_INTERFACE,
        RETRO_ENVIRONMENT_SET_HW_RENDER_CONTEXT_NEGOTIATION_INTERFACE,
        RETRO_ENVIRONMENT_GET_HW_RENDER_CONTEXT_NEGOTIATION_INTERFACE_SUPPORT,
        RETRO_ENVIRONMENT_SET_HW_SHARED_CONTEXT,
    ] {
        assert!(calls.contains(&format!("{cmd} false")), "{cmd}");
    }
}

#[test]
fn the_context_callbacks_run_in_order_once() {
    let mut host = loaded_with(PC, |cb| ask(cb, callback(RETRO_HW_CONTEXT_OPENGLES2, 0, 0)));
    host.context_destroy().unwrap();
    host.context_reset().unwrap();
    host.context_reset().unwrap();
    host.context_destroy().unwrap();
    host.context_destroy().unwrap();
    let calls: Vec<String> = seen()
        .calls
        .into_iter()
        .filter(|c| c.starts_with("context_"))
        .collect();
    assert_eq!(
        calls,
        ["context_reset", "context_reset", "context_destroy"],
        "no destroy before a reset, a second reset after a loss, one destroy"
    );
}

#[test]
fn the_context_callbacks_need_a_game_and_a_context() {
    let mut host = open().unwrap();
    assert_eq!(host.context_reset(), Err(RunError::NoGame));
    drop(host);
    let mut host = loaded_with(PC, |_| {});
    assert_eq!(host.context_reset(), Err(RunError::NoHwRender));
    assert_eq!(host.context_destroy(), Err(RunError::NoHwRender));
}

extern "C" fn gl_clear() {}

fn lookup(name: &CStr) -> usize {
    if name == c"glClear" {
        gl_clear as *const () as usize
    } else {
        0
    }
}

#[test]
fn the_frontend_callbacks_read_the_target_on_any_thread() {
    let mut host = loaded_with(PC, |cb| ask(cb, callback(RETRO_HW_CONTEXT_OPENGLES2, 0, 0)));
    let (_, filled) = answer().unwrap();
    let framebuffer = filled.get_current_framebuffer.unwrap();
    let proc_address = filled.get_proc_address.unwrap();
    // SAFETY: the callbacks the host wrote, with NUL-terminated names or
    // null.
    let before = unsafe { (framebuffer(), proc_address(c"glClear".as_ptr())) };
    assert_eq!(before, (0, None), "no target yet");
    host.set_hw_target(7, lookup);
    // SAFETY: as above.
    unsafe {
        assert_eq!(framebuffer(), 7);
        let found = proc_address(c"glClear".as_ptr()).map(|f| f as usize);
        assert_eq!(found, Some(gl_clear as *const () as usize));
        assert!(proc_address(c"glNothing".as_ptr()).is_none());
        assert!(proc_address(std::ptr::null()).is_none());
    }
    let other = std::thread::spawn(move || {
        // SAFETY: as above, on a thread of the core.
        unsafe { (framebuffer(), proc_address(c"glClear".as_ptr()).is_some()) }
    })
    .join()
    .unwrap();
    assert_eq!(other, (7, true));
}
