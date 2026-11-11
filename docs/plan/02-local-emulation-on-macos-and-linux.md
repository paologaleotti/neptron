## Local emulation on macOS and Linux

NEPTRON runs in three places during development, each closer to the real thing and slower than the last; a Mac needs only Rust for the first two, and a container runtime plus a native QEMU for the third.

&#91;embedded content: development environments · native process and QEMU guest\]

| Mode | Command | What is real | What is faked |
| --- | --- | --- | --- |
| Native headless | `cargo xtask verify --tier 2` | All NEPTRON logic, the UI, the libretro host, test cores | Display, GPU, audio device, input devices, time |
| Native window | `cargo run -p neptron -- --backend window` | Same, drawn in a desktop window for a human to look at | Same, with real time and keyboard as a gamepad |
| Emulated OS | `cargo xtask qemu` | The Buildroot image: kernel, `BusyBox init`, DRM/KMS, evdev, ALSA, filesystems, updater | The hardware: a virtual display, virtual pads, a dummy sound card |

**Why native mode can be exact.** The UI never draws pixels itself: it emits a scene made of four primitive kinds. In T0 to T2 the CPU reference renderer draws that scene, so goldens are bit-identical on macOS and Linux. On hardware the GLES renderer draws the same scene, and T3 compares the two inside the Linux guest under Mesa's software rasteriser, with a small tolerance. Keeping the primitive set tiny is what keeps the two renderers in step.

**The container.** One `Dockerfile` in the repo holds the Buildroot host dependencies, QEMU, Mesa and the Switchres command-line tool for cross-checking a mode by hand. Buildroot needs Linux and a case-sensitive filesystem, so on macOS everything under `os/` builds inside this container. On Apple Silicon the container is arm64, which builds the Pi image natively and cross-compiles the x86 one. QEMU itself runs natively on macOS, outside the container, because the Mac's hypervisor is not available inside one.

**The emulated machine.** Two extra Buildroot configs, `neptron_qemu_aarch64` and `neptron_qemu_x86_64`, share every package with the real targets but use a generic kernel with these test drivers enabled:

| Need | Kernel feature | What the test can check |
| --- | --- | --- |
| A display without hardware | `vkms`, the [virtual KMS driver](https://www.kernel.org/doc/html/latest/gpu/vkms.html) | Atomic commits succeed, the mode read back equals the mode requested, per-frame CRCs, and the composed picture through its writeback connector |
| Gamepads, guns, wheels | `uinput` | Devices appear, hotplug, mapping, recorded device traces replayed |
| Sound | `snd-dummy` or `snd-aloop` | Sample counts, underruns, a test tone's pitch |
| Storage and power loss | virtio disks | Kill QEMU mid-write, reboot, check saves and the A/B slot |

- QEMU runs the generic `virt` machine, accelerated by KVM on Linux and, for the aarch64 guest, by the Mac's hypervisor on Apple Silicon. The x86\_64 guest is emulated on a Mac and slow, so it runs in CI. It does not emulate a Raspberry Pi board; Pi-specific drivers and overlays are covered by build checks and by T5.
- The control socket is forwarded to the host, so `neptronctl --target qemu` works exactly like the native one.
- **Fast loop.** CI publishes the base image. `cargo xtask qemu --hot` rebuilds only the Rust binaries, overlays them on the cached image and boots in seconds. A full Buildroot build is needed only when `os/` changes.
- **First task of WP 1.5:** confirm that `vkms` accepts arbitrary user modelines and interlaced modes. The 6.18 source checks only the mode size, so this probably passes; configfs for extra connectors arrives in 6.19. If it does not, a small test-only kernel patch in the QEMU config lifts the limit; the fallback is the fake display in `neptron-testkit`.
