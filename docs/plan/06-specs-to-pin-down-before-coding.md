## Specs to pin down before coding

Four areas are too complex to start from a one-line description. Each gets a short spec file in `docs/spec/` as the first step of its work package.

| Area | Why it needs a spec | What the spec contains | What to lean on |
| --- | --- | --- | --- |
| Libretro host | Cores use dozens of environment callbacks, and hardware-rendered cores need exact GL context handling | The callback list in tiers (below), the order of one frame, the hardware-render context lifecycle | `libretro.h`; the [RePlayOS changelog](https://www.replayos.com/changelog/) as a checklist of what real cores needed; a logging run of the five reference cores |
| KMS layer | Custom modes, plane crops, buffer modifiers and forced connectors behave differently per driver | The exact atomic sequence that works on i915, vc4 and RP1 | The M1 hardware spike |
| Frame pacing and audio | Easy to get subtly wrong; errors show as stutter or crackle | The display-locked loop, the rate-control method, what happens when game and display rates differ | RetroArch's dynamic rate control, reused rather than reinvented |
| Settings schema and skin format | The whole UI is generated from them | Field list, value types, scopes; the five screen templates and their parameters | Written at the start of WP 3.1 and 3.2 |

**Libretro callbacks, starting tiers.** This list is from memory; the logging run of the reference cores decides the real one.

| Tier | Purpose | Callbacks |
| --- | --- | --- |
| Must, M2 | Software-rendered cores run | Pixel format, video refresh, audio batch, input state and bitmasks, core options, system and save directories, geometry and AV-info changes, rotation, log, input descriptors, controller info, serialization, save RAM, messages |
| Should, M2 to M3 | GL cores, disc systems, lower latency | Hardware render for GLES 2 and 3, disk control, VFS, rumble, frame-time callback, audio buffer status and minimum latency, software framebuffer, memory maps |
| Later |  | Vulkan render, MIDI, netpacket, camera, location |
