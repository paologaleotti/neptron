## Local emulation on macOS and Linux

NEPTRON runs in three places during development, each closer to the real thing and slower than the last; a Mac needs only Rust for the first, Rust and SDL3 for the second, and a container runtime plus a native QEMU for the third.

| Mode | Command | What is real | What is faked |
| --- | --- | --- | --- |
| Native headless | `cargo xtask verify --tier 2` | All NEPTRON logic, the UI, the libretro host, test cores | Display, GPU, audio device, input devices, time |
| Native window | `cargo run -p neptron --features window -- --backend window` (`apps/neptron/README.md`) | Same, drawn in a desktop window for a human to look at | Same, with real time and keyboard as a gamepad |
| Emulated OS | `cargo xtask qemu` | The Buildroot image: kernel, `BusyBox init`, DRM/KMS, evdev, ALSA, filesystems, updater | The hardware: a virtual display, virtual pads, a dummy sound card |

**Why native mode can be exact.** The UI never draws pixels itself: it emits a scene made of four primitive kinds. In T0 to T2 the CPU reference renderer draws that scene, so goldens are bit-identical on macOS and Linux. On hardware the GLES renderer draws the same scene, and T3 compares the two inside the Linux guest under Mesa's software rasteriser, with a small tolerance. Keeping the primitive set tiny is what keeps the two renderers in step.

**The container.** One `Dockerfile` in the repo holds the Buildroot host dependencies, a pinned Buildroot release and QEMU. Mesa is a package of the images. Buildroot needs Linux and a case-sensitive filesystem, so on macOS everything under `os/` builds inside this container. On Apple Silicon the container is arm64, which builds the Pi image natively and cross-compiles the x86 one. QEMU itself runs natively on macOS, outside the container, because the Mac's hypervisor is not available inside one.

**The emulated machine.** Two images boot in QEMU: the aarch64 test image (`neptron_qemu_aarch64_defconfig`), which shares the packages of the real targets with a generic kernel, and the PC image (`neptron_pc_x86_64_defconfig`), whose QEMU hooks act only inside QEMU (`os/README.md`, "The QEMU link"). Both have these test drivers:

| Need | Kernel feature | What the test can check |
| --- | --- | --- |
| A display without hardware | `vkms`, the [virtual KMS driver](https://www.kernel.org/doc/html/latest/gpu/vkms.html) | Atomic commits succeed, the mode read back equals the mode requested, per-frame CRCs, and the composed picture through its writeback connector |
| Gamepads, guns, wheels | `uinput` | Devices appear, hotplug, mapping, recorded device traces replayed |
| Sound | `snd-dummy` or `snd-aloop` | Sample counts, underruns, a test tone's pitch |
| Storage and power loss | virtio disks | Kill QEMU mid-write, reboot, check saves and the A/B slot |

- QEMU runs the `virt` machine for aarch64 and `q35` for x86\_64, accelerated by KVM on Linux and, for the aarch64 guest, by the Mac's hypervisor on Apple Silicon. The x86\_64 guest is emulated on a Mac and slow, so it runs in CI. It does not emulate a Raspberry Pi board; Pi-specific drivers and overlays are covered by build checks and by T5.
- **Control socket on the host (not built).** The plan forwards the guest's control socket to the host, so `neptronctl` drives a guest as it drives a native run. Today `neptronctl` takes only a local socket path (`--socket`).
- **Fast loop (not built).** A `cargo xtask qemu --hot` that rebuilds only the Rust binaries, overlays them on a cached base image and boots in seconds. `xtask/README.md` lists the `qemu` options that exist.
- `vkms` on kernel 6.18 accepts arbitrary user modelines, interlaced and doublescan modes (`os/README.md`, section "neptron-vkms-probe"). It has one connector; configfs for extra connectors arrives in 6.19.
