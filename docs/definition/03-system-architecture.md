## System architecture

NEPTRON is one frontend process that hosts the cores and talks to the kernel through three interfaces: DRM/KMS for video, used directly, and ALSA for audio and evdev for input, both through SDL3.

&#91;embedded content: architecture · frontend process, kernel, hardware\]

The video path is the only code that can change the display, and it passes validated modes only. Arrows show the direction data flows; `BusyBox init` starts a small launcher, which starts the frontend and restarts it after a crash. Background services never run in the frame loop, and a control socket (`neptron-ipc`) is the single entry point for tools such as `neptronctl` and a future web companion.
