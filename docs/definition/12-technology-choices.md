## Technology choices

NEPTRON hand-writes only what makes it NEPTRON: the display layer, the libretro host, the ODREN policy, the safety validator and a small UI. Everything with a proven library behind it is reused, even where that library is C or C++. Every crate named here was checked on crates.io on 2026-11-10. Pin sdl3 (0.22), which still makes breaking releases; khronos-egl has had no release since 2023 but is stable.

| Concern | Choice | Build or reuse | Why | Rejected |
| --- | --- | --- | --- | --- |
| Base OS | Buildroot stable (2026.08 or later) at a pinned commit, br2-external tree. The LTS branch (2025.02) has no SDL3 package and its Rust is too old for edition 2024 | Reuse | Small, reproducible, you asked for it | Yocto (heavier), Debian base (slow boot, large) |
| Language | Rust stable, edition 2024, pinned toolchain |  | Project rule; memory safety around a lot of FFI |  |
| Display | DRM/KMS atomic API through the `drm` crate; buffers through `gbm` | Build on crates | Full control of modesets, timings and buffer modifiers; this is the product | SDL3 for video (hides the modeset), X11, Wayland |
| Game rendering | OpenGL ES 2/3 on EGL (`khronos-egl`, `glow`), Mesa drivers | Build on crates | Works on every target; GL cores need a shared GL context anyway | wgpu for now (heavier; revisit with Vulkan cores) |
| Shaders | Own GLES 2 shaders for the built-in set; librashader for slang presets | Mixed | Built-ins are a few dozen lines each; the slang runtime is a solved, large problem | Writing a slang runtime ourselves |
| Core hosting | Bindings generated from `libretro.h`, `libloading`; core runs in-process | Build | Per-frame control of video, audio and input; this is the product | RetroArch as a subprocess (two UIs, no control of switching) |
| Modelines | Switchres linked as a library behind a Rust wrapper; ODREN policy and the validator are our own Rust | Reuse the maths, build the policy | Proven on real monitors by RetroArch, GroovyMAME and RePlayOS; a port would have to re-earn that trust | A Rust port (the fallback if the library cannot be built on both hosts) |
| Audio | SDL3 audio output, `rubato` resampler, RetroArch-style dynamic rate control | Reuse | Device handling and resampling are solved; only the rate-control loop is ours | Hand-written ALSA code and resampler; PipeWire, PulseAudio |
| Input | SDL3 gamepad API with its controller database, hotplug and rumble. The evdev crate for keyboards, keyboard encoders, mice, spinners and HID guns, because SDL3 reads those only through its own video driver | Reuse | Controller quirks and mappings are a long tail SDL already covers | Hand-written gamepad layer and mapping parser |
| UI | Own code. It emits a scene that the GLES renderer draws on hardware and a CPU reference renderer draws in tests; a handful of fixed screen templates; TOML skins | Build, small | A list-and-dialog UI is simple; GLES leaves room for a sharper look on flat panels later, and the CPU reference keeps test goldens exact on macOS and Linux | Slint, egui, iced (built for high-DPI desktops); a second GPU renderer for the UI |
| Settings and data | TOML with `serde`; library index as one binary cache file | Build on crates | Human-readable, no database engine | SQLite (more than needed) |
| Init | BusyBox init with respawn | Reuse | Supervision for free; a custom PID 1 saves milliseconds and adds risk | Own Rust PID 1, systemd |
| Updates | RAUC, A/B slots, signed bundles | Reuse | A broken updater bricks devices; this is not a place to learn by doing | Own updater |
| Networking | `iwd` configured through its plain network files, in-kernel `ksmbd` with its small helper | Reuse | iwd needs a D-Bus daemon, which runs only while Wi-Fi is on; NEPTRON drives iwd through iwctl and its network files, with no D-Bus code of its own | NetworkManager, Samba; web uploader (after alpha) |
| Bluetooth | Deferred until after alpha |  | Pairing UI over BlueZ is a project of its own |  |
| Testing | `proptest`, the kernel's virtual KMS driver (`vkms`) under QEMU, golden frames | Reuse | Modeset logic tested in CI without a monitor | Manual testing only |
| Logging | `tracing` into a RAM ring buffer, exported on request | Reuse | No constant SD-card writes | syslog to disk |
| Box art | Small blocking HTTP client (`ureq`) fetching from the libretro thumbnails set, matched by file name | Build, small | No account, no API key, nothing running in the background | Account-based scraper APIs; full metadata scraping |

RePlayOS, the closest reference, is written in C with DRM, EGL, OpenGL ES and SDL3 for audio and input, according to its [FAQ](https://www.replayos.com/faq/). It links Switchres as a dynamic library, and its frontend is proprietary. NEPTRON keeps the same single-binary idea and the same two libraries, and is open source throughout.
