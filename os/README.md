# os

The Buildroot br2-external tree of NEPTRON. It defines the OS images. Today it has the two QEMU targets that tier T3 runs on. The real x86\_64 and Raspberry Pi images come later and share this tree.

## Pinned versions

| What | Version | Pin |
| --- | --- | --- |
| Buildroot | 2026.08.1, the newest stable release on 2026-11-11 | Tag commit `7fdeadb3b9f12f78e5fccca914d7fef47090ab9a`; `buildroot-2026.08.1.tar.gz` SHA-256 `38a6c152490096018720a9ade379ee1e571c9bf5f93a3657a166566945a98b0d`, from the signed release announcement. `Dockerfile` fetches and checks it |
| Linux | 6.18.63, the newest 6.18 LTS release on 2026-11-11 | SHA-256 in `patches/linux/linux.hash`, from kernel.org |
| C library | glibc, from the Buildroot toolchain | Buildroot 2026.08.1 |
| Container base | `debian:trixie-slim` | Image digest in `Dockerfile` |

Buildroot 2026.08.1 is a bug-fix release of 2026.08. It is not in the repository and not a submodule: the container downloads the release tarball and checks its hash.

## Layout

| Path | Contents |
| --- | --- |
| `external.desc`, `external.mk`, `Config.in` | The br2-external tree, name `NEPTRON` |
| `configs/neptron_qemu_aarch64_defconfig` | QEMU `virt` machine, 64-bit Arm. Runs with the hypervisor on an Apple Silicon Mac |
| `configs/neptron_qemu_x86_64_defconfig` | QEMU `q35` machine, x86-64. Runs with KVM on Linux CI |
| `board/qemu/video.config` | Kernel fragment: `vkms`, and debugfs for its CRCs. No fbdev emulation and no framebuffer console |
| `board/qemu/input.config` | Kernel fragment: `uinput` |
| `board/qemu/sound.config` | Kernel fragment: `snd-dummy`, `snd-aloop` |
| `board/qemu/host.config` | Kernel fragment: the QEMU firmware config in sysfs, and 9p |
| `board/common/busybox.config` | BusyBox fragment: the `timeout` applet |
| `board/common/rootfs-overlay` | `inittab` for BusyBox init, and the init scripts that mount debugfs and run the command from the host |
| `patches/` | The global patch and hash directory. It holds the kernel hashes |
| `package/` | NEPTRON packages. `libretro-fceumm`, `libretro-snes9x` and `libretro-genesis-plus-gx` are the three MVP cores; see below. `neptron-vkms-probe` is a test tool for `vkms`; see below |

The kernel configuration of each QEMU target is the Buildroot QEMU configuration (`board/qemu/aarch64-virt/linux.config` or `board/qemu/x86_64/linux.config` in the Buildroot tree) plus the four fragments in `board/qemu`, one for each concern.

## The images

Both QEMU targets have:

- BusyBox init, with `mdev` for device nodes. There is no udev daemon and no systemd.
- A root shell on the serial console, with no login.
- An ext4 root filesystem, 256 MB, on a virtio disk. QEMU runs it in snapshot mode, so a run never changes the image.
- libdrm with its test programs (`modetest`), and `neptron-vkms-probe`.
- Mesa with the softpipe software renderer: EGL, OpenGL ES and GBM, and `kmscube`.
- SDL3 with the KMS/DRM video driver and OpenGL ES, without libudev, and alsa-lib.
- The libretro cores FCEUmm, Snes9x and Genesis Plus GX in `/usr/lib/libretro`, with the library names of `data/cores`.

The build also installs the Rust toolchain of Buildroot (`host-rustc`, the prebuilt `rust-bin`, with the standard library for the target) in the output, not in the image. `cargo xtask gles-compare` uses it with the Buildroot C toolchain as the linker to build Rust programs for the guest. This toolchain (Rust 1.97.1 in Buildroot 2026.08.1) is not the one that `rust-toolchain.toml` pins for the host (1.97.0); the two can drift apart when Buildroot is bumped.

## Commands

All commands go through `xtask`.

```text
cargo xtask os build --arch aarch64              build the image
cargo xtask os build --arch aarch64 -- linux-reconfigure
                                                 rebuild the kernel after a change to its fragment
cargo xtask os build -- neptron-vkms-probe-rebuild
                                                 rebuild a package of this tree after a change to its source
cargo xtask qemu -- uname -a                     boot, run one command, exit with its exit code
cargo xtask qemu --script test.sh --share dir    run a script; dir is read-only at /mnt/host
cargo xtask qemu                                 interactive console; Ctrl-A then X stops QEMU
cargo xtask gles-compare                         build the GLES comparison for the guest and run it
cargo xtask core-trace                           run the test ROMs in the real cores (tier T4)
```

`os build` builds the development container from `Dockerfile`, then runs Buildroot in it with 4 jobs, 4 CPUs and 6 GB of memory. A machine-wide lock allows one Buildroot build at a time. Buildroot does not see a change to a kernel fragment, or to the source of a package in `package/`, by itself: give the `-reconfigure` or `-rebuild` target as above.

`qemu` boots the image with 2 GB of memory and 2 CPUs. A machine-wide lock allows one guest at a time. On macOS QEMU runs under `taskpolicy -c utility`. The aarch64 guest uses the hypervisor on an Apple Silicon Mac and KVM on an aarch64 Linux host. The x86\_64 guest uses KVM on an x86-64 Linux host. Everywhere else QEMU emulates the CPU, which is too slow for routine use: on a Mac, use the aarch64 target.

How a command runs:

1. `xtask` writes the command as a shell script and gives it to QEMU as the firmware config entry `opt/neptron/run`. Two more entries carry a new random nonce (`opt/neptron/nonce`) and the deadline in seconds (`opt/neptron/timeout`, from `--timeout`, default 120).
2. The init script `S99neptron-run` prints `NEPTRON-BEGIN <nonce> <uptime>`, runs the script under `timeout -s KILL <deadline>`, prints `NEPTRON-END <nonce> <exit code>` and reboots. `-no-reboot` turns the reboot into a QEMU exit. A command over its deadline ends with exit code 137, and the guest stops even when no host process watches it.
3. `xtask` accepts only markers with its nonce, so output that contains the marker text does not end the run. It prints the lines between the markers, the time to the first marker and the guest uptime, and exits with the exit code of the command. It writes the whole console to `target/qemu/<arch>-console.log`.
4. `xtask` stops QEMU 30 s after the deadline, and at once on SIGTERM, SIGHUP or SIGINT. It removes its script on exit, and the next run removes scripts that a killed run left.

## The cores

One package for each core, pinned to an exact upstream commit, with the SHA-256 of the GitHub archive of that commit and of the licence file in its `.hash` file. `BR2_DOWNLOAD_FORCE_CHECK_HASHES` makes Buildroot refuse a download with another hash.

| Package | Upstream and commit | Library | Licence in the package |
| --- | --- | --- | --- |
| `libretro-fceumm` | `libretro/libretro-fceumm` `e0f3086a8104f8fbbe49a2615b10401504a9533f` | `fceumm_libretro.so` | `GPL-2.0+`, file `Copying` |
| `libretro-snes9x` | `libretro/snes9x` `fae2fea08f74180759ef540ee94259213f503480` | `snes9x_libretro.so` | `LicenseRef-Snes9x (non-commercial)`, file `LICENSE` |
| `libretro-genesis-plus-gx` | `libretro/Genesis-Plus-GX` `b9c2316441978d4015ba30e9a95d9b0e6f8ee981` | `genesis_plus_gx_libretro.so` | `LicenseRef-GenesisPlusGX (non-commercial)`, file `LICENSE.txt` |

These are the commits that `data/cores/README.md` names. Each package runs the upstream libretro makefile with `platform=unix` and the Buildroot cross toolchain, and installs the library in `/usr/lib/libretro`. The makefiles ask git for the version, and the build directory is no git checkout, so each package gives `GIT_VERSION` from its pin. The packages have no architecture-specific flags, so the same files build for both targets. Only the aarch64 image has been built.

Snes9x and Genesis Plus GX allow non-commercial use only. `make legal-info` lists the licence of each package. Whether an image ships them is a decision of the owner; the QEMU images are test images and are not distributed.

## neptron-vkms-probe

A small C program on libdrm. For each mode on its command line it makes a framebuffer with a test pattern, and reports: the test-only and the real atomic commit, the mode read back from the CRTC, the vblank period and the flip-event period next to the period of the mode, three frame CRCs from debugfs, and a writeback capture compared pixel by pixel with the pattern. The exit code is the number of failed commits, readbacks, CRC reads and writeback captures. The periods are reported only, not judged. For an interlaced mode the `vkms` vblank interval on kernel 6.18 is half the field period: `crtc_vtotal` is already halved for interlace, and the timestamping code halves the frame duration again. The probe prints a line that says so.

```text
cargo xtask qemu -- neptron-vkms-probe \
    "320x240 6700 320 336 368 424 240 243 246 263 -hsync -vsync" \
    "720x480i 13500 720 739 801 858 480 488 494 525 -hsync -vsync interlace"
```

A mode is `NAME CLOCK_KHZ HDISP HSS HSE HTOTAL VDISP VSS VSE VTOTAL` and flags from `+hsync -hsync +vsync -vsync interlace doublescan`.

## What was verified

On an Apple Silicon Mac, on 2026-11-11, with the aarch64 image:

- The guest boots to the command in 0.2 to 1.2 s after QEMU starts (guest uptime 0.1 to 0.8 s). Exit codes and quoted words pass through. Output that holds `NEPTRON-END 0` does not end the run. A command over its deadline ends with 137 and the guest stops. After SIGTERM or SIGHUP to `xtask`, no QEMU process is left one second later. After SIGKILL to `xtask`, QEMU stops at the guest deadline.
- `neptron-vkms-probe` with 320x240 at 6.7 MHz, 640x480, 720x480i and a doublescan mode: every commit is accepted, the readback equals the request, the CRCs and the writeback work.
- `sleep 60 | kmscube -D /dev/dri/card0 -c 300` draws on `vkms` with Mesa softpipe at about 53 frames per second ("Rendered 299 frames in 5.630363 sec"), and the frame CRCs change from frame to frame while it runs. kmscube needs a standard input that stays open: with `/dev/null` it stops at once. `kmscube -A` (atomic) stops with "no eglDupNativeFenceFDANDROID", because softpipe has no native fence.

- EGL on the Mesa surfaceless platform with no device gives OpenGL ES 2 on softpipe. `cargo xtask gles-compare` uses it: an image build that adds `host-rustc` took 4 min, the cross build of the comparison 7 s from cold, and the guest run under 10 s.
- The three cores (2026-11-11): the incremental `os build` that added them took 130 s. The root filesystem went from 28,922 KiB to 44,717 KiB used. The libraries are 4.3 MiB (`fceumm_libretro.so`), 2.7 MiB (`snes9x_libretro.so`) and 12.0 MiB (`genesis_plus_gx_libretro.so`). `neptron-vkms-probe` passes on the new image.

Not verified:

- The x86\_64 image was never built or booted. Its defconfig resolves in Buildroot with every option set. On a Mac it runs only under CPU emulation; it is meant for Linux CI with KVM. `-vga none` keeps the q35 VGA card away from `card0`, but this is not checked on x86\_64.
- SDL3, ALSA (`snd-dummy`, `snd-aloop`) and `uinput` are in the image, but no program used them at run time.

Build time and size, aarch64, 4 jobs: the first full build took 48 min (toolchain, host tools, kernel, BusyBox). Mesa, SDL3 and kmscube added 19 min. A kernel reconfigure, or a change to the overlay or to a package of this tree, takes about 1 min. The kernel `Image` is 13.3 MB. The root filesystem has 28 MB used of 256 MB. The guest uses about 100 MB of memory after boot.

Disk use outside the repository after these builds: the downloads volume 1.2 GB, the output volume 10.9 GB, the compiler cache 0.3 GB, the container image 0.7 GB, and `~/.cache/neptron` 0.15 GB.

## Where the build data lives

Nothing large is in the repository.

| What | Where |
| --- | --- |
| Downloads | Docker volume `neptron-br-dl` |
| Compiler cache | Docker volume `neptron-br-ccache` |
| Buildroot output | Docker volume `neptron-br-out-<arch>` |
| Cargo registry and target directory of the guest builds | Docker volume `neptron-cross-<arch>` |
| Images for QEMU | `~/.cache/neptron/buildroot/images/<arch>` (`NEPTRON_CACHE_DIR` moves `~/.cache/neptron`) |

The downloads, the compiler cache and the output are Docker volumes because Buildroot needs a case-sensitive filesystem and the macOS default is not. All worktrees share them. `NEPTRON_OS_IMAGES` or `--images` boots an images directory from somewhere else, for example the base image that CI publishes.

## Licences

The files of this tree are `MIT OR Apache-2.0`, as the rest of NEPTRON. Buildroot is GPL-2.0-or-later. It runs as a build tool and is not part of the repository. The packages in the images keep their own licences, which `make legal-info` lists.
