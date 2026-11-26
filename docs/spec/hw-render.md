# Hardware rendering of libretro cores: GL and GLES

This spec defines how NEPTRON runs a libretro core that draws with OpenGL or OpenGL ES (`RETRO_ENVIRONMENT_SET_HW_RENDER`), so that milestone 0.2 can run PlayStation and Dreamcast with Naomi. Task S01 designs it. Task S02 builds it in the work packages of section 10. ADR 0022 (proposed) records the contract change.

Status: design, not accepted. Section 13 lists the questions for the owner. No implementer starts the work packages that section 13 shapes before the owner answers (run rule 20).

## 0. The owner's decisions for 0.2

- libretro cores only. No standalone emulator.
- GL and GLES cores in 0.2. Vulkan in 0.3.
- PlayStation first, then Dreamcast with Naomi.
- Machines: the PC (x86_64, Intel graphics, Mesa), the Raspberry Pi 3, and new images for the Pi 4 and the Pi 5.

## 0.1 The facts of today (checked in the code)

- The host refuses `SET_HW_RENDER`, `GET_HW_RENDER_INTERFACE`, `SET_HW_RENDER_CONTEXT_NEGOTIATION_INTERFACE` and `GET_PREFERRED_HW_RENDER` (`crates/neptron-libretro/src/ffi/env.rs`). `SET_HW_SHARED_CONTEXT` reaches the unknown arm and gets `false`. A frame of `RETRO_HW_FRAME_BUFFER_VALID` is the fault `HardwareFrame` (`docs/spec/libretro-host.md`, "Frames").
- `retro_hw_render_callback` in `crates/neptron-libretro-sys/src/bindings.rs` has the fields `context_type`, `context_reset`, `get_current_framebuffer`, `get_proc_address`, `depth`, `stencil`, `bottom_left_origin`, `version_major`, `version_minor`, `cache_context`, `context_destroy`, `debug_context` (64 bytes). `RETRO_HW_FRAME_BUFFER_VALID` is `((void*)-1)` in `vendor/libretro.h` (a macro, so not in the bindings).
- One thread runs the core, the engine, the renderer and the KMS commit: the main thread of the frontend. `Host` is `!Send` and `!Sync`. `retro_run` runs inside `Session::run_frame`, on the same thread as `Display::present`. There is no separate core thread. The other threads are I/O threads (save writer, library, log, network, control).
- There is no watchdog that kills a hung core. `neptron-launch` sends SIGTERM and then SIGKILL after 5 s (`STOP_WAIT_NS`) only when the launcher itself gets a stop signal. The 10 s values in the code are `START_WINDOW_NS` (the failed-start rule, `docs/spec/arcade.md` 10.1) and `RELAUNCH_WINDOW_NS` (the relaunch loop). Section 13, Q10, asks the owner about a watchdog.
- `GlesRenderer` (`crates/neptron-gfx`) makes one EGL 1.5 display on the GBM device of `neptron-kms`, one OpenGL ES 2 context and one window surface. It makes its context current at the start of each call.
- The GBM surface has the size of the largest mode. `KmsDisplay::open` makes it once and never again. A modeset changes only the timings and the visible part of the buffers (`docs/spec/kms.md`). So a modeset never loses a GL context.
- `Scene::game` is `Option<Frame<'a>>`: CPU bytes. Every renderer and the fake display read bytes.
- The images: the PC Mesa has `iris`, `llvmpipe`, `softpipe`, EGL and GLES. The Pi image has `vc4`, `softpipe`, EGL and GLES. Neither sets a desktop GL library option (`os/configs/*_defconfig`). The Pi image has no `v3d` driver, which the Pi 4 and 5 need (`os/REFERENCE.md`). `iris` covers Intel Gen 8 (Broadwell) and later; older Intel GPUs need `crocus`, which the image does not have.

## 1. Context types per machine

### 1.1 What Mesa gives

| Machine | Driver | GLES | Desktop GL | Source |
| --- | --- | --- | --- | --- |
| PC, Intel Gen 8 and later | `iris` | 3.2 | 4.6 core and compatibility | Mesa features list. Not verified on the image |
| Pi 3 | `vc4` (VideoCore IV) | 2.0 | 2.1 | Mesa `vc4` driver. Not verified on the image |
| Pi 4 | `v3d` (VideoCore VI, V3D 4.2) | 3.1 (conformant since 2020) | 2.1 in Mesa 20.x. The hardware has 4 draw buffers, and GL 3.0 needs 8 | Igalia blog, January 2020; Mesa docs `drivers/v3d` |
| Pi 5 | `v3d` (V3D 7.1) | 3.1 | 2.1 or 3.1: not verified | Mesa docs `drivers/v3d` |

S02 records the real strings with `eglinfo` (or a probe of the frontend) on each image (work package W1).

On the Pi 4 and 5 the display controller (`vc4`) and the GPU (`v3d`) are two DRM devices. EGL on the GBM device of the `vc4` card renders with `v3d` through Mesa's `kmsro` path. Not verified on the NEPTRON image.

### 1.2 What NEPTRON offers

NEPTRON probes the machine once per process, before `retro_load_game`, and offers what the probe finds. No table in the code fixes the versions. The probe makes a context of each API at the highest version it can get and drops it.

| Context type of the core | PC (`iris`) | Pi 4, Pi 5 (`v3d`) | Pi 3 (`vc4`) |
| --- | --- | --- | --- |
| `OPENGLES2` | yes | yes | yes |
| `OPENGLES3` | yes | yes | no |
| `OPENGLES_VERSION` (3.1 and later) | up to the probe | up to the probe (3.1) | no |
| `OPENGL_CORE` (with version) | up to the probe (4.6) | no in 0.2 | no |
| `OPENGL` (2.x, compatibility) | yes | no in 0.2 | no |
| `VULKAN` | no in 0.2 (0.3) | no | no |
| `D3D*`, `RSX`, others | no | no | no |

Desktop GL on the Pi is out of 0.2: no 0.2 core needs GL 2.1 on a Pi, and the Pi image would need a desktop GL library. The PC image needs a desktop GL library for `OPENGL_CORE` (`libOpenGL.so` through glvnd, or `libGL.so`). Whether the Buildroot Mesa build of today gives one with EGL and no GLX is not verified (W1).

### 1.3 The answers of the host

| Call | Answer |
| --- | --- |
| `SET_HW_RENDER` | `true` when the offer holds the type and the version (section 1.2) and the host is in `retro_load_game`. Else `false`, and the host keeps the request for the notice (section 6). A second call in the same load replaces the first. A call outside `retro_load_game` gets `false` |
| `GET_PREFERRED_HW_RENDER` | `true`, with `OPENGL_CORE` on the PC, `OPENGLES3` on the Pi 4 and 5, `OPENGLES2` on the Pi 3. A core that picks its renderer by this call (SwanStation "auto", Beetle PSX) then picks GL |
| `GET_HW_RENDER_INTERFACE` | `false`: GL has no interface; Vulkan is 0.3 |
| `SET_HW_RENDER_CONTEXT_NEGOTIATION_INTERFACE`, `GET_HW_RENDER_CONTEXT_NEGOTIATION_INTERFACE_SUPPORT` | `false`: Vulkan only |
| `SET_HW_SHARED_CONTEXT` | `false`. The core never shares the frontend context (section 2) |

A core that gets `false` can fall back to software (Beetle PSX HW does this; not verified for each core) or refuse the game. A refused game gives notice H1 (section 6).

## 2. Threading and ownership

### 2.1 Two contexts on one thread

- The frontend context: the GLES 2 context of `GlesRenderer`, as today. Only the renderer uses it.
- The core context: a second EGL context on the same EGL display, of the type and version of the request, made after `retro_load_game` returns. It has no surface (`EGL_KHR_surfaceless_context`). It does not share objects with the frontend context: EGL shares objects only between contexts of the same client API, and a desktop GL core context and a GLES context are not of the same API.
- Both contexts are current only on the main thread. The frame loop switches between them with `eglMakeCurrent`: the core context before each call into the core that can draw, the frontend context at the start of each renderer call (as today).

Why two contexts and not one, as RetroArch does: a core that leaves GL state dirty (bound objects, blend, scissor, viewport, pixel store) cannot change the frontend context, so the renderer needs no state reset and stays as it is. A desktop GL core needs a desktop GL context, and the renderer stays GLES 2. The cost is two context switches per frame (section 8).

### 2.2 The picture crosses with an EGL image

The core context owns a colour texture of the maximum size, and a framebuffer object with that texture attached. The core draws into that framebuffer object. An EGL image of the texture (`EGL_KHR_gl_texture_2D_image`) goes to the frontend context, which binds it to a texture with `glEGLImageTargetTexture2DOES` (`GL_OES_EGL_image`) and samples it as it samples an uploaded frame today. An EGL image crosses client APIs. Both extensions on `vc4` and `v3d`: not verified (W1).

One framebuffer, not a ring. Two server-side fences order the work, with no wait on the CPU:

1. After the renderer draws a frame that reads the texture, it adds a fence in the frontend context.
2. Before the core draws, the core context waits on that fence (`eglWaitSyncKHR`).
3. After `retro_run`, the core context adds a fence. The renderer waits on it before it reads the texture.

With no `EGL_KHR_wait_sync`, the step that waits calls `glFinish` in the other context. S02 measures both (section 8).

Why one framebuffer: `get_current_framebuffer` may change each frame by the header, but some cores keep the name from `context_reset`. A ring would show such a core's old frames in turn. RetroArch also uses one framebuffer when no shader needs old frames (not verified in its current code).

### 2.3 Who owns what

| Object | Owner | Crate |
| --- | --- | --- |
| EGL display, frontend context, window surface | `GlesRenderer` | `neptron-gfx` |
| Core context, colour texture, depth and stencil buffer, framebuffer object, EGL image, fences | `CoreGl`, made by the renderer on its EGL display | `neptron-gfx` (feature `gles`) |
| The `CoreRender` trait object the session talks to | `KmsDisplay`, which holds the renderer | `neptron-kms` |
| The request, the answers, the framebuffer name and the lookup of GL functions for the core | `Host` state | `neptron-libretro` |
| When to make the context, call `context_reset`, begin and end a core frame | `Session` | `neptron-session` |

GL and EGL types stay in `neptron-gfx`. `neptron-backend` holds the trait and plain types (ADR 0022). `neptron-libretro` sees a framebuffer name (`u32`) and a lookup function `fn(&CStr) -> usize`. The desktop window backend (`neptron-window`) offers no core context in 0.2: a GL core there gets `false` and notice H1.

### 2.4 The callbacks with no state

`get_proc_address` and `get_current_framebuffer` read process globals, not the thread-local state: the lookup function in a `OnceLock`, the framebuffer name in an `AtomicU32`. So a core that resolves GL functions on its own thread still gets them. One core runs per process, so a process global is correct. A GL call of the core on another thread is not supported: the core context is current only on the main thread. `SET_HW_SHARED_CONTEXT` stays refused.

### 2.5 The rule "no lock across a call into a core"

No lock exists on this path. The two contexts, the fences and the host state are on one thread. The globals of 2.4 are an atomic and a value set once.

### 2.6 A hung core

A core that does not return from `retro_run` holds the main thread, with the core context current. Today nothing kills it, unless the launcher gets a stop signal (SIGTERM, then SIGKILL after 5 s). When the process ends, the kernel frees the contexts and the GPU buffers of the process. The launcher keeps the DRM descriptor, so the last flip stays on the wire, as for any crash. A context cannot outlive its process, so a kill leaks nothing. Q10 asks whether 0.2 gets a watchdog. A watchdog must allow for a core that compiles shaders for several seconds in its first frames (section 8).

## 3. The frame path

### 3.1 Launch order

```text
probe the offer (once per process)            CoreRender::offer
Host::set_hw_offer(offer)
retro_load_game                               the core calls SET_HW_RENDER; the host answers from the offer
Host::hw_request()                            Some(request) for a GL core
CoreRender::create(request, max geometry)     core context, texture, framebuffer, depth and stencil
Host::set_hw_target(framebuffer, lookup)
CoreRender::begin; Host::context_reset; CoreRender::end
settle frames, then the loop                  as today (LAUNCH_STABLE_FRAMES)
```

The probe makes a context of each API, so it costs a GL setup. Every launch is a new process, so the app keeps the offer of the display in the RAM folder (`run/gl-offer.toml`) for the boot: the first launch of a boot probes, and each later launch, of a software core too, reads the file and asks the display nothing (`VideoSetup::core_offer`).

`context_reset` comes after `retro_load_game`, as the header requires. A failed `create` ends the launch with notice H2, and the session unloads the core.

### 3.2 One frame

The frame order of `docs/spec/pacing-audio.md` gets two steps around the core:

```text
flip event -> input -> CoreRender::begin -> core (retro_run) -> CoreRender::end -> mute if asked -> present -> ...
```

- `begin`: make the core context current, wait on the fence of the last present.
- The core binds the framebuffer of `get_current_framebuffer`, draws, and calls `video_refresh(RETRO_HW_FRAME_BUFFER_VALID, width, height, 0)`.
- The host checks the size as for a memory frame (no geometry, zero size, over the maximum) and gives `Video::Hardware { width, height }`. `NULL` stays a duplicate. `RETRO_HW_FRAME_BUFFER_VALID` from a core with no accepted `SET_HW_RENDER` stays the fault `HardwareFrame`.
- `end`: add the fence. Count GL errors as in section 6.3.
- The session builds the source from the size, as today, and the engine builds the scene with `GamePicture::Gpu` (ADR 0022).
- `present`: the renderer makes the frontend context current, waits on the core fence, draws the game quad from the imported texture, and the UI quad as today.

The other calls into the core that can draw run between `begin` and `end` too: `retro_reset`, `retro_unserialize`, `context_reset`, `context_destroy`, `retro_unload_game`.

While the quick menu is open, the core does not run, and the texture keeps the last frame. The renderer draws it under the UI as it draws the last memory frame today. The game under an opaque UI is not drawn (the scissor rule of `GlesRenderer` holds).

### 3.3 The framebuffer

- Colour: an RGBA8 texture of `max_width` by `max_height` of the AV info. A `SET_SYSTEM_AV_INFO` with a larger maximum makes new attachments between frames and keeps the framebuffer name. That is the only allocation after the launch.
- `depth` true: a depth renderbuffer, 24 bits where the context has it (`GL_OES_depth24` on GLES 2), else 16 bits. `depth` and `stencil` true: a packed 24/8 renderbuffer (`GL_OES_packed_depth_stencil` on GLES 2). `stencil` alone is ignored, as the header says. A context that cannot give what the core asked for fails `create`: notice H2.
- The picture is the `width` by `height` part at texel `(0, 0)` of the texture, the storage corner that GL calls the origin.

### 3.4 `bottom_left_origin`

The texture stores rows from GL `y = 0` upward.

- `bottom_left_origin` false (the libretro default): picture line `y` is texel row `y`. Line 0 (the top of the picture) is at GL `y = 0`.
- `bottom_left_origin` true: picture line `y` is texel row `height - 1 - y`.

`GpuFrame::origin` carries it (ADR 0022). The renderer applies it as one more sign on the vertical axis of the turn it already does for `Scene::rotation`, before the turn: no extra pass. The test core draws in both conventions (section 7). The RetroArch convention is not verified in its source; W3 checks the result against a real core on the PC.

### 3.5 Scale, filter, scanlines, TATE, UI

Nothing changes in the rules. `crop`, `dest`, `rotation`, `filter`, `scanlines`, the UI canvas and its turn apply to a `Gpu` picture exactly as to a `Memory` picture. The renderer gives the shader the storage size in `u_frame`, as it does for a smaller frame today. The texel format is RGBA8, not `BGRA_EXT`: the sampler gives the same colour for both, so the shader does not change.

The reference renderer has no GPU images. The fake display resolves a `Gpu` picture to memory first (section 7). The reference renderer refuses a `Gpu` picture with a new `RenderError::NoPixels`.

### 3.6 Who reads the bytes of a GPU picture

`Session::last_frame()`, the control API picture and any screenshot read bytes. For a `Gpu` picture they call `CoreRender::read_back` into a buffer made at launch. It runs only on a request, never in a steady frame, because `glReadPixels` stalls the GPU.

## 4. Resolution

A GL core can draw above its native size (internal resolution: Beetle PSX HW, SwanStation and Flycast have a core option). The core then reports the larger size in `video_refresh` and in its geometry.

### 4.1 What the picture policy sees

The policy always sees the native size: the frame size over the scale factor `k` that NEPTRON set. The scene crop is in frame pixels, so the engine multiplies the native crop by `k`. `INTERLACE_MIN_LINES` applies to the native lines.

NEPTRON sets `k` itself, through the core option that the core metadata names (a new key in `data/cores`, append-only). It does not guess `k` from the frame. When the frame size is not `k` times a size the native geometry allows, the session uses `k = 1` for that frame and logs it once.

As built:

- "Fits" counts lines only: the largest whole `k` of the core data with `k * native_lines` at most the lines of the largest mode of the panel (`vactive`), at most 4. The width does not count: the policy scales the columns. `native_lines` comes from the core data, because the session sets the option before the load, when the core has reported no geometry yet.
- A frame counts as `k` times its source only when the core's largest frame has at least `k * native_lines` lines (a core that ignored the option reports its native maximum), both sides are multiples of `k`, and the native size fits the native maximum.
- The scene: every rule runs on the native size, and the crop is `k` times the native crop. The filter of the placement stays when the destination is a whole multiple of the scaled crop on both axes (a 4x picture in a 4x destination is nearest, 1:1); otherwise it is sharp bilinear (a 4x picture in a 4.5x fit, or 4x columns in a 5x destination).
- `k` is picked again on a live change of the scanlines and on a change of the monitor or of its class in a game.

### 4.2 Default per display class (proposal, Q3 to Q6)

| Display class | Internal resolution | Why |
| --- | --- | --- |
| CRT (15, 25, 31 kHz) and TV | 1x, always | The mode carries the native lines. A larger frame has no place on the tube and costs GPU time |
| Flat panel, PC | the largest whole `k` with `k * native lines <= panel lines`, at most 4 | 240 lines on 1080: 4x |
| Flat panel, Pi 4 and Pi 5 | 1x until S02 measures (W6) | Not measured |
| Flat panel, scanlines on | 1x | The scanline rule needs `dest.h / T.h >= 2`; at `k` above 1 the lines would not follow the native lines |

## 5. Context loss and mode changes

| Event | What happens |
| --- | --- |
| Modeset, a new mode of any size | Nothing in GL. The GBM surface and both contexts stay (section 0.1). No `context_reset` |
| Display guard, blank, output off | Nothing in GL. The core runs as today |
| The GBM surface is made again | Never in a session. A future change that does it keeps the core context: the core context has no surface |
| A larger maximum from the core (`SET_SYSTEM_AV_INFO`) | New attachments, same framebuffer name, no `context_reset` |
| The GPU loses the context (a GPU reset; `GL_CONTEXT_LOST`, robustness through `EGL_EXT_create_context_robustness` where the probe finds it) | `CoreRender::end` reports it. The session makes a new core context and calls `context_reset` with no `context_destroy`, as the header says. A second loss in the session ends the game with notice H4 (Q9) |
| End of the game | `context_destroy` (if not null), `retro_unload_game`, `retro_deinit`, all with the core context current; then the context goes |

`cache_context`: NEPTRON never resets a context on purpose, so it honours the flag for every core. `debug_context`: ignored in the image; the debug build makes a debug context and logs `KHR_debug` messages through the log thread.

The order "`context_destroy` before `retro_unload_game`" follows the header's text. The order in RetroArch is not verified. A core that frees GL objects in `retro_unload_game` still finds its context current.

## 6. Safety and failure

### 6.1 Cases

| Case | What happens | Notice |
| --- | --- | --- |
| The core asks for a type or a version the offer lacks, and refuses the game | `LaunchError::Refused`; the host has the request | H1 |
| The same, and the core falls back to software | The game runs. One log line names the request and the fallback | none |
| The offer holds the request, but `create` fails | The launch ends, the core unloads | H2 |
| The core leaves GL state dirty | No effect: the frontend has its own context (2.1) | none |
| The core binds framebuffer 0 and draws | It draws into no surface: the surfaceless context has no default framebuffer. The picture stays the last one | none; a debug log line |
| The core never sends a hardware frame | The screen stays black under the UI; the quick menu works | H3 after `START_WINDOW_NS`, if the owner takes Q8 (b) |
| GL errors | Counted, not fatal (6.3) | none |
| A core that draws too slowly | The late pacing of `docs/spec/pacing-audio.md`, and its slow-speed notice | existing |
| Context lost twice | The game ends | H4 |

### 6.2 Notice texts (proposal)

`<name>` and the core text follow the rules of `docs/spec/arcade.md` 10.1. `<api>` is `OpenGL`, `OpenGL ES` or `Vulkan`. `<version>` is `major.minor`.

| # | Text |
| --- | --- |
| H1 | `<name>` did not start: the core needs `<api>` `<version>`, and this machine does not have it. |
| H2 | `<name>` did not start: the graphics driver could not prepare `<api>` `<version>` for the core. |
| H3 | `<name>` did not start: the core showed no picture in 10 seconds. |
| H4 | The game stopped: the graphics driver lost the picture of the game two times. |

H1 and H2 come before S1 in `pick_notice`, because they give the real reason.

### 6.3 GL errors

`glGetError` can stop a threaded Mesa context until its queue is empty, so `end` reads it once every 60 core frames, not each frame. The count goes to the counters and one log line per second at most. A GL error never ends a game: many cores leave harmless errors.

## 7. Tests

### 7.1 A GL test core (`neptron-testcore`, script version 5)

New setup line `hw_render <type> <major> <minor> [depth] [stencil] [bottom_left]`, with `<type>` one of `gles2`, `gles3`, `gles`, `gl`, `glcore`. New actions: `hw_null` (send `NULL`), `hw_none` (send nothing), `hw_dirty` (leave a scissor, a blend, a viewport and framebuffer 0 bound), `hw_fbo0` (draw into framebuffer 0).

The test card is the `shape` card of version 4, drawn with scissored `glClear` calls only: a white border, a blue top-left corner, a green staircase from the top-left to the bottom-right corner, and a yellow arrow of stacked rectangles that points to the top. It uses only `glBindFramebuffer`, `glViewport`, `glEnable` and `glDisable` of `GL_SCISSOR_TEST`, `glScissor`, `glClearColor`, `glClear` and `glGetError`. These exist in every context type of section 1.2, need no shader, and show shape and direction (run rule 21). The core gets each function through `get_proc_address` and logs an error if one is missing.

### 7.2 A reference core GL (`neptron-testkit`)

The same eight functions in plain Rust, on a memory image, as `extern "C"` functions with thread-local state. The lookup returns their addresses as `usize` (a cast of a safe function item, no `unsafe`). The fake display implements `CoreRender` with it and resolves a `Gpu` picture to a memory frame by the origin rule of 3.4. So tier 1 and tier 2 run the GL test core on macOS and Linux with no GPU and check the picture with the reference renderer and goldens.

### 7.3 What each tier proves

| Tier | Proves |
| --- | --- |
| T1, pure | Host answers per offer (every type, version above and below, call outside load), the request kept for H1, `Video::Hardware` size checks, the launch order of 3.1, the frame order of 3.2, the `k` rule of 4.1, the notice choice |
| T1 and T2, fake display and reference core GL | The test core in each origin, each `rotation` and screen orientation, with scanlines and the UI over it; the quick menu over a GPU picture; `hw_null`, `hw_none` (H3), a refused type (H1), a failed `create` (H2), a context loss (H4); goldens of the test card |
| T3, `gles-compare` (QEMU, softpipe) | The real path: core context of type `gles2` and `gles3` (and `glcore` if the image has desktop GL), EGL image, fences, the imported texture through `GlesRenderer` against the reference renderer, both origins, all turns. `hw_dirty` and `hw_fbo0` change no pixel of the composition. Depth and stencil attachments are made for each type |
| T3, `kms-check` (`vkms`) | The GBM platform: a core context on the display of the KMS backend, a GPU picture through the writeback connector equal to the reference |
| Hardware only | `iris`, `vc4` and `v3d` extensions and versions (W1), the cost of section 8, a real core (Beetle PSX HW, PCSX ReARMed, Flycast), `kmsro` on the Pi 4 and 5, GPU reset on Intel. Each goes to `docs/hardware-checklist.md`, labelled `needs-hardware` |

## 8. Performance facts that shape the design

Known:

- The Pi 3 has GLES 2.0 only. No PlayStation GL renderer runs there (section 9). PCSX ReARMed draws in software with NEON, so the Pi 3 needs no hardware path for PlayStation.
- `vc4` and `v3d` share memory with the CPU, and `vc4` allocates GPU buffers from CMA. A framebuffer of 1024x512 RGBA8 with 24/8 depth is 4 MiB. The CMA check of `docs/hardware-checklist.md` applies.
- The design adds no copy of a frame on the CPU: the texture goes from the core context to the renderer as an EGL image.
- `glReadPixels` stalls the GPU, so it is never in a steady frame (3.6).
- Shader compilation in a core's first frames can take seconds on a Pi (Flycast, not measured).

To measure in S02 (W6), each on the PC and the Pi 4 (and the Pi 5 when there is a board), with the bench build:

1. The cost of the two `eglMakeCurrent` calls per frame.
2. The fence wait against `glFinish`, in GPU time per frame.
3. GPU time of the composition of a GPU picture against an uploaded frame of the same size.
4. Beetle PSX HW at 1x, 2x, 4x on the Intel PC, and PCSX ReARMed on the Pi 3 and the Pi 4: full speed or not, through the slow-speed counter.
5. Flycast on the Pi 4 and the Intel PC at 1x: full speed or not.
6. The first-frame stall of each core (time to the first hardware frame).

## 9. Candidate cores

The licence column is a fact to check in S10 (PlayStation) and S11 (Dreamcast). This spec decides no licence question. The frontend loads each core in its own process (ADR 0019).

| System | Core | Licence (to check) | Render paths | Pi 3 | Pi 4, Pi 5 | PC |
| --- | --- | --- | --- | --- | --- | --- |
| PlayStation | Beetle PSX HW | GPL-2.0 (libretro docs) | software, GL 3.3 core, Vulkan | software only, too slow (not measured) | software; GL 3.3 not on `v3d` | GL 3.3 core |
| PlayStation | SwanStation | GPL-3.0 (OpenEmbedded recipe; not checked upstream) | software, OpenGL, Vulkan, D3D11; GLES not verified | software renderer (forum report) | GLES path not verified | OpenGL |
| PlayStation | PCSX ReARMed | GPL-2.0 (libretro docs) | software (NEON GPU plugin, `unai`); no `SET_HW_RENDER` | yes, the usual choice | yes | yes, no upscale |
| Dreamcast, Naomi, Naomi 2, Atomiswave | Flycast | GPL-2.0 (GitHub) | OpenGL (3.0 and later), OpenGL ES 3, Vulkan; a GLES 2 path is not verified | not playable (not measured) | GLES 3 | GL core |

Sources: docs.libretro.com pages of Beetle PSX HW, PCSX ReARMed and Flycast; github.com/flyinghead/flycast; the OpenEmbedded layer index for `swanstation-libretro`; libretro forum threads on SwanStation on the Pi 3. The data checked here has no minimum GL version for Flycast or for SwanStation.

What it means:

- PlayStation on the Pi 3 is a software core. The hardware path is for the PC (Beetle PSX HW or SwanStation) and is optional on the Pi 4 and 5.
- Dreamcast and Naomi need GLES 3 at least (to check in S11), so they are for the Pi 4, the Pi 5 and the PC. Q2 asks what the Pi 3 shows.
- `data/cores/README.md` says "Each system has exactly one core". A core per machine is a new rule (Q1).

## 10. Work packages for S02

Each package is one branch, mergeable alone, with its gate. Order: W1, then W2 and W3 in parallel, then W4, W5, W6, W7.

| # | Package | Owns | Needs | Gate |
| --- | --- | --- | --- | --- |
| W0 | Contract: ADR 0022 accepted. `GamePicture`, `GpuFrame`, `GpuImageId`, `FrameOrigin` in `neptron-types`; `CoreRender` and its types in `neptron-backend` with `Display::core_render` defaulting to `None`; every `Scene` caller fixed with `GamePicture::Memory`; `RenderError::NoPixels`. No golden changes | `crates/neptron-types`, `crates/neptron-backend`, the `Scene` callers of ADR 0022 | owner answers | T1, every golden unchanged |
| W1 | Probe: `eglinfo`-like strings and the offer on each image; the PC desktop GL library in `os/configs`; `v3d` and `kmsro` in the Pi 4 and 5 images (or a note that the new images own it) | `crates/neptron-gfx/examples`, `os/configs` (orchestrator) | W0 | T0; hardware checklist lines |
| W2 | Host: offer, `SET_HW_RENDER` and the other answers of 1.3, `Video::Hardware`, `context_reset` and `context_destroy`, the globals of 2.4. Spec update of `docs/spec/libretro-host.md` | `crates/neptron-libretro`, `docs/spec/libretro-host.md` | W0 | T1 |
| W3 | GL core test tools: test core script version 5 (7.1); reference core GL and `CoreRender` in the fake display (7.2) | `crates/neptron-testcore`, `crates/neptron-testkit` | W0 | T1 |
| W4 | Session: launch order 3.1, frame order 3.2, the `k` rule 4.1, notices H1 to H4 in `pick_notice`, the core metadata key for internal resolution | `crates/neptron-session`, `crates/neptron-video`, `apps/neptron/src/shell`, `data/cores/README.md` | W2, W3 | T2 with the GL test core |
| W5 | GPU: `CoreGl` in `neptron-gfx` (context, texture, framebuffer, EGL image, fences, read-back, loss), `GlesRenderer` draws `GamePicture::Gpu` with the origin sign; `CoreRender` in `KmsDisplay`; `gles-compare` and `kms-check` scenes of 7.3 | `crates/neptron-gfx`, `crates/neptron-kms` | W0, W1 | T3 `gles-compare`, `kms-check` |
| W6 | Measurements of section 8 | `docs/hardware-checklist.md` | W4, W5 | needs-hardware |
| W7 | First real cores: the PlayStation core packages (S10) and Flycast (S11) | `os/package` | W4, W5, licence answers | `os app-check` |

Out of 0.2:

- Vulkan (`RETRO_HW_CONTEXT_VULKAN`, the negotiation interface): 0.3.
- Shader passes (CRT shaders, the core's own shader presets).
- Shared contexts (`SET_HW_SHARED_CONTEXT`) and GL calls of a core on its own thread.
- Desktop GL on the Pi.
- A user setting for internal resolution, if the owner takes Q5 (a).
- Frame history (a ring of framebuffers) for effects that need old frames.
- Hardware frames on the desktop window backend.
- `crocus` for Intel GPUs before Gen 8.

## 11. Not verified

- The versions in section 1.1 on the NEPTRON images, and the Pi 5 desktop GL version.
- `EGL_KHR_gl_texture_2D_image`, `GL_OES_EGL_image`, `EGL_KHR_surfaceless_context`, `EGL_KHR_wait_sync`, `GL_OES_packed_depth_stencil` on `vc4` and `v3d`.
- A desktop GL library in the PC image with EGL and no GLX.
- `kmsro` on the Pi 4 and 5 with the NEPTRON kernel.
- The `bottom_left_origin` convention of RetroArch, and its order of `context_destroy` and `retro_unload_game`.
- The licences, the GLES paths of SwanStation and Flycast, and the fallback to software of each core.
- Every cost of section 8.

## 12. Code choices made here (not for the owner)

- Two contexts on one thread, not one shared context (2.1).
- An EGL image of a texture of the core context, not a GBM buffer imported into both contexts (2.2). A GBM buffer stays the fallback if `EGL_KHR_gl_texture_2D_image` is missing on a driver.
- One framebuffer with two fences, not a ring (2.2).
- Process globals for the two callbacks that need no state (2.4).
- `glGetError` once every 60 frames (6.3).
- The reference core GL in `neptron-testkit` (7.2).

## 13a. The hang watchdog (Q10)

The owner took Q10 (b): a heartbeat, and a kill after 15 s with a notice. It is a package of its own, not tied to the GL path: any core can hang. This section is the design; S02 builds it.

### What it watches

A core that does not return from a call holds the main thread of the frontend: the frame loop, the display and the input stop with it. Only a process outside the frontend can end that. The launcher is that process: it already starts the frontend, holds the DRM descriptor and looks at its child every 20 ms (`CHILD_POLL`).

The watchdog watches a game only, from its launch to its end. The boot, the game list and the load of a game (`retro_load_game`, which can read a large file for many seconds) are not watched. A hang in `retro_load_game` stays a hang in 0.2.

### How the heartbeat crosses

- The launcher binds a Unix datagram socket at `<temp dir>/neptron-launch-<pid>.heartbeat` and gives its path to each frontend in `NEPTRON_HEARTBEAT`. Std has the socket: no new crate and no new dependency. The launcher removes the file when it exits.
- The frontend runs one heartbeat thread, which the KMS run starts and owns: it stops it and joins it before the run ends. The frame thread does no I/O for it: after each game frame it adds 1 to an atomic counter, and it sets an atomic flag when a game starts and clears it before the game ends (before the last save RAM flush, which can take long).
- The heartbeat thread looks once per second (`HEARTBEAT_PERIOD_NS`). In a game, it sends one byte `B` when the counter moved since its last look, and at its first look in the game. Out of a game it sends `D`. A send never blocks: the socket is non-blocking, and a full socket drops the byte.
- The launcher reads every datagram at each look at its child. `B` arms the watchdog and marks the time; `D` disarms it. A new run starts disarmed.

### What is killed

- When the watchdog is armed and no `B` came for 15 s (`HANG_WAIT_NS`), the launcher sends SIGKILL to the frontend and logs one line. No SIGTERM first: the frontend reads its stop flag on the main thread, which the core holds.
- 15 s leaves room for a 3D core that compiles its shaders in its first frames (section 8). The first beat of a game comes at its first look, so the first frame has 15 s too.
- The run counts as a crash for the backoff of the launcher (a short wait), and the launcher starts the frontend again.

### The save RAM on disk

- The writer thread writes every save file as temp file, fsync, rename, and keeps one `.bak`. A kill at any point leaves the last complete save file and its backup. A kill during a write leaves a temp file that the next write replaces. The settings and the guard record are written the same way.
- The save RAM that the core changed after its last flush is lost: at most `SAVE_RAM_FLUSH_INTERVAL_NS` of play, less when the quick menu opened since then. No code can copy it, because the core holds the thread that owns it. This is the same loss as a power cut.

### What the user sees

- The relaunch file of the game was taken at the start of the run, so the new run boots to the game list, not into the hung game: no loop.
- The launcher gives that start `NEPTRON_HUNG=1` (`HUNG_ENV`), once, as it does with `NEPTRON_POWER_FAILED`. The frontend then shows the Warning notice: "The game stopped: it did not answer for 15 seconds." The game list does not know the name of the game, so the text has none. The owner approves the text.

### What changes, plainly

- A new thread with an owner in the frontend: the heartbeat thread of the KMS run.
- The launcher protocol: two new environment variables, `NEPTRON_HEARTBEAT` (the socket path) and `NEPTRON_HUNG` (once, after a kill). A frontend with no `NEPTRON_HEARTBEAT` sends nothing; a launcher that gets no `B` never kills.
- User data: a kill loses the save RAM since the last flush, as section "The save RAM on disk" says. Nothing on disk is lost.

## 13. Questions for the owner

**Decision: Owner, 2026-11-26: the recommendation of every question, Q1 to Q11.** The mission for each choice in this work: as plug and play as possible, and an accurate, sharp picture on a CRT.

- Q1 (a): Beetle PSX HW on the PC, PCSX ReARMed on the Pi 3, 4 and 5.
- Q2 (a): Dreamcast and Naomi are hidden on a Pi 3.
- Q3 (b): on a panel, the PC takes the largest whole multiple that fits, at most 4x; a Pi 4 or 5 takes 1x until it is measured.
- Q4 (a): a CRT and a TV always get 1x.
- Q5 (a): no user setting for internal resolution in 0.2.
- Q6 (a): scanlines force 1x.
- Q7 (b): a system is hidden where no game of it can run; the notice H1 for the rest.
- Q8 (b): a core with no picture ends after 10 s with the notice H3.
- Q9 (a): one recovery of a lost context; the game ends at the second loss.
- Q10 (b): a heartbeat and a kill after 15 s with a notice, as a package of its own.
- Q11 (a): Naomi and Atomiswave are systems of their own.

The questions as they were asked:

**Q1. Which PlayStation core runs on each machine?**
- (a) Beetle PSX HW (GL 3.3 core) on the PC, PCSX ReARMed on the Pi 3, 4 and 5. A core per machine becomes a rule of `data/cores`.
- (b) SwanStation on every machine: GL on the PC, software on the Pis.
- (c) PCSX ReARMed on every machine: no hardware path for PlayStation in 0.2, no upscale on a panel.
- Recommendation: (a). The best picture on the PC, the proven core on the Pi. The licences go to S10.

**Q2. What does the Pi 3 show for Dreamcast and Naomi?**
- (a) The systems do not appear on a Pi 3.
- (b) They appear, and a game gives notice H1.
- (c) They appear with a warning in the game list that they run slowly.
- Recommendation: (a). The Pi 3 has no GLES 3.

**Q3. The internal resolution on a flat panel, by default.**
- (a) Native (1x) on every machine.
- (b) The largest whole multiple of the native lines that fits the panel, at most 4x, on the PC; 1x on the Pi 4 and 5 until S02 measures.
- (c) A fixed 2x everywhere.
- Recommendation: (b).

**Q4. The internal resolution on a CRT and a TV.**
- (a) Always native (1x), with no way to change it.
- (b) Native by default, with a setting.
- Recommendation: (a). The tube shows the native lines.

**Q5. Does the user get an internal resolution setting in 0.2?**
- (a) No. Defaults only.
- (b) One setting per system in the video settings (Auto, 1x, 2x, 3x, 4x), on flat panels only.
- (c) A setting per game.
- Recommendation: (a) for 0.2; (b) later.

**Q6. Scanlines on a flat panel with a larger internal resolution.**
- (a) Scanlines on forces 1x.
- (b) Above 1x, scanlines are off and the setting says so.
- (c) Scanlines on the native lines at any `k` (a second contract change).
- Recommendation: (a).

**Q7. What does the user see when a core needs a GL version that the machine lacks?**
- (a) Notice H1 when the game starts.
- (b) The library hides a system whose core this machine cannot run (a metadata key and the probe), and H1 covers what remains.
- (c) The game list shows such a game in grey with the reason.
- Recommendation: (b). The texts H1 and H2 of section 6.2 are part of the question.

**Q8. A core that shows no picture.**
- (a) The game stays black; the player leaves through the quick menu.
- (b) After 10 s with no hardware frame (`START_WINDOW_NS`), the game ends with notice H3.
- Recommendation: (b).

**Q9. The GPU loses the context of the game.**
- (a) Make a new context once and continue; a second loss ends the game with H4.
- (b) End the game at the first loss with a notice.
- Recommendation: (a).

**Q10. A core that hangs: a watchdog in 0.2?**
- (a) No watchdog, as today. A hung game needs a power cycle or a stop from SSH.
- (b) The frontend sends a heartbeat to the launcher each frame. The launcher kills a frontend with no heartbeat for 15 s and relaunches to the game list with a notice: "`<name>` stopped: the game did not respond for 15 seconds." The time allows for the shader compile of a 3D core.
- Recommendation: (b), as its own work package, not tied to the GL path.

**Q11. Where do Naomi and Atomiswave games go?**
- (a) Their own systems (`naomi`, `atomiswave`), each with Flycast.
- (b) Inside the `arcade` system next to FinalBurn Neo games, with the core chosen per set.
- Recommendation: (a). One system has one core today; (b) would change that rule. S11 owns the details.
