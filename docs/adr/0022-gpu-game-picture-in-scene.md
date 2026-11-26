# 0022: The scene carries a game picture that lives on the GPU, and a display can host a core context

- Status: proposed
- Date: 2026-11-26
- Task: S01

## Context

Milestone 0.2 runs libretro cores that draw with OpenGL or OpenGL ES (`docs/spec/hw-render.md`). Such a core draws into a framebuffer object of the frontend and sends `RETRO_HW_FRAME_BUFFER_VALID` in place of pixels.

The contracts of today:

- `Scene::game` is `Option<Frame<'a>>`. A `Frame` borrows CPU bytes. The reference renderer, the GLES renderer, the fake display, the goldens and the scene hash read those bytes.
- `Display` (`neptron-backend`) has `caps`, `set_mode`, `present`, `wait_flip` and `blank`. Nothing in it makes a GL context for a core.
- The GL context and the EGL display live in `GlesRenderer` (`neptron-gfx`), which `KmsDisplay` holds. GL and EGL types must not leave `neptron-gfx`. `neptron-session` and `neptron-video` depend on `neptron-backend`, never on `neptron-kms` or `neptron-gfx`.
- One thread runs the core, the engine and the renderer. A modeset never makes the GBM surface or the GL context again.

So the session needs a way to ask the display for a core context, and the scene needs a way to name a picture that has no CPU bytes.

## Decision

1. `neptron-types` gets:
   - `enum GamePicture<'a> { Memory(Frame<'a>), Gpu(GpuFrame) }`.
   - `struct GpuFrame { image: GpuImageId, width: u16, height: u16, storage_width: u16, storage_height: u16, origin: FrameOrigin }`.
   - `struct GpuImageId(u32)`: a handle that only the display that made it can resolve.
   - `enum FrameOrigin { TopLeft, BottomLeft }`. With `TopLeft`, picture line `y` is texel row `y` of the storage, counted from GL `y = 0`. With `BottomLeft`, it is texel row `height - 1 - y`.
2. `Scene::game` becomes `Option<GamePicture<'a>>`. The meaning of `crop`, `dest`, `rotation`, `filter`, `scanlines` and every UI field does not change: they apply to the picture of `width` by `height`, upright as the core draws it, after the origin rule.
3. `neptron-backend` gets the trait `CoreRender` and `Display::core_render(&mut self) -> Option<&mut dyn CoreRender>`, with a default body that returns `None`. `CoreRender` has, in plain types:
   - `offer() -> ContextOffer`: the context types and highest versions this display can make.
   - `create(&ContextRequest, max_width, max_height) -> Result<CoreTarget, CoreRenderError>`: the core context, the colour texture, the depth and stencil buffer and the framebuffer. `CoreTarget` holds the framebuffer name (`u32`), the `GpuImageId` and the lookup of GL functions (`fn(&CStr) -> usize`).
   - `resize(max_width, max_height)`: new attachments, the same framebuffer name.
   - `begin()` and `end() -> Result<CoreFrameStatus, CoreRenderError>`: make the core context current and order the work with fences; report GL errors and a lost context.
   - `read_back(&mut [u8], width, height, origin)`: the bytes of the picture, upright, for a screenshot. Never in a steady frame.
   - `destroy()`.
4. A display resolves a `GpuImageId` that it made. An id it does not know gives `DisplayError::SceneRefused`. The reference renderer refuses a `Gpu` picture with `RenderError::NoPixels`; the fake display of `neptron-testkit` resolves it to a `Memory` frame first.
5. `neptron-libretro` reports a hardware frame as `Video::Hardware { width, height }` for a core with an accepted `SET_HW_RENDER`, after the size checks of a memory frame. `FrameFault::HardwareFrame` stays for a core with no accepted request.
6. The scene hash of the fake display includes the picture kind and `origin`.

## Consequences

- Every code that builds a `Scene` wraps its frame in `GamePicture::Memory`. `Scene` has no `Default`, so a missed caller is a build error. The callers are the list of ADR 0020 (the engine, the `neptron-gfx` tests and `gles_compare`, the `neptron-testkit` display, the black scene of `neptron-kms` and `kms_check`, the window smoke example, `neptronctl` selftest). The change in the KMS commit path is the wrap of the black scene and nothing else. Every golden stays unchanged.
- Every `Display` of today compiles with no change, because `core_render` has a default. Only `KmsDisplay` and the fake display implement `CoreRender` in 0.2. The window backend returns `None`, so a GL core there gets notice H1.
- The GLES renderer draws a `Gpu` picture from an imported texture with the code of the game quad. `FrameOrigin::BottomLeft` is one more sign on the vertical axis, applied before the turn. No extra pass.
- Consumers that need bytes (`Session::last_frame`, the control API picture) call `read_back` on request. A steady frame copies nothing on the CPU.
- A Vulkan core in 0.3 uses the same `GamePicture::Gpu` and a new context type in `ContextRequest`. Its image import is a matter of the backend.
- Nothing on disk or on the wire changes. `Scene` borrows its frames and has no serde (ADR 0001).
- To watch: the handle is valid only for the display that made it, and only until `destroy`. The session must not keep a `Scene` across a `destroy`; the borrow of `Scene<'a>` does not prove this for a `GpuImageId`, so the display checks it.

## Alternatives

- Read the framebuffer back to the CPU each frame and keep `Scene` as it is: no contract change, and the fake display, the reference renderer and the goldens work at once. Rejected: `glReadPixels` stalls the GPU each frame, and it adds a CPU copy and an upload of the whole picture. At 4x on a panel that is about 4 MiB down and up per frame. It stays the fallback of `read_back` for screenshots.
- One GL context shared by the core and the renderer, with a reset of the GL state after each core frame (the RetroArch way): no second context and no fences. Rejected: a desktop GL core needs a desktop GL context, and the renderer is GLES 2; a core that leaves state the reset does not know changes the composition; the state reset grows with each core found.
- A separate call `Display::present_gpu`, with `Scene::game` set to `None` and the texture held in the display: no change to `Scene`. Rejected: hidden state, two code paths for one picture (the code standards ask for one path), and the engine could not see that a picture exists.
- A trait object in `Scene` (`&dyn GpuPicture`): rejected. `Scene` is `Copy` and `Hash`, and the renderers in `neptron-gfx` would need to cast the object back to their own type.
- A GBM buffer of the frontend imported into both contexts (dma-buf): equal in cost, and it is the fallback if a driver lacks `EGL_KHR_gl_texture_2D_image`. Not the first choice: it brings GBM calls into `neptron-gfx` for the core path, where an EGL image of a texture needs none. The contract of this ADR does not change with that choice.
