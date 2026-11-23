## System architecture

NEPTRON is one frontend process that hosts the cores and talks to the kernel through three interfaces: DRM/KMS for video, used directly; ALSA for audio, through SDL3; and evdev for input, through SDL3 for gamepads and directly for keyboards and pointers.

The video path is the only code that can change the display, and it passes validated modes only. `BusyBox init` starts a small launcher, which starts the frontend and restarts it after a crash. Background services never run in the frame loop, and a control socket (`neptron-ipc`) is the single entry point for tools such as `neptronctl` and a future web companion.
