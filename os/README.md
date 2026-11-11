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
| `package/` | NEPTRON packages. `neptron-vkms-probe` is a test tool for `vkms`; see below |

The kernel configuration of each QEMU target is the Buildroot QEMU configuration (`board/qemu/aarch64-virt/linux.config` or `board/qemu/x86_64/linux.config` in the Buildroot tree) plus the four fragments in `board/qemu`, one for each concern.

## The images

Both QEMU targets have:

- BusyBox init, with `mdev` for device nodes. There is no udev daemon and no systemd.
- A root shell on the serial console, with no login.
- An ext4 root filesystem, 256 MB, on a virtio disk. QEMU runs it in snapshot mode, so a run never changes the image.
- libdrm with its test programs (`modetest`), and `neptron-vkms-probe`.

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
```

`os build` builds the development container from `Dockerfile`, then runs Buildroot in it with 4 jobs, 4 CPUs and 6 GB of memory. A machine-wide lock allows one Buildroot build at a time. Buildroot does not see a change to a kernel fragment, or to the source of a package in `package/`, by itself: give the `-reconfigure` or `-rebuild` target as above.

`qemu` boots the image with 2 GB of memory and 2 CPUs. A machine-wide lock allows one guest at a time. On macOS QEMU runs under `taskpolicy -c utility`. The aarch64 guest uses the hypervisor on an Apple Silicon Mac and KVM on an aarch64 Linux host. The x86\_64 guest uses KVM on an x86-64 Linux host. Everywhere else QEMU emulates the CPU, which is too slow for routine use: on a Mac, use the aarch64 target.

How a command runs:

1. `xtask` writes the command as a shell script and gives it to QEMU as the firmware config entry `opt/neptron/run`. Two more entries carry a new random nonce (`opt/neptron/nonce`) and the deadline in seconds (`opt/neptron/timeout`, from `--timeout`, default 120).
2. The init script `S99neptron-run` prints `NEPTRON-BEGIN <nonce> <uptime>`, runs the script under `timeout -s KILL <deadline>`, prints `NEPTRON-END <nonce> <exit code>` and reboots. `-no-reboot` turns the reboot into a QEMU exit. A command over its deadline ends with exit code 137, and the guest stops even when no host process watches it.
3. `xtask` accepts only markers with its nonce, so output that contains the marker text does not end the run. It prints the lines between the markers, the time to the first marker and the guest uptime, and exits with the exit code of the command. It writes the whole console to `target/qemu/<arch>-console.log`.
4. `xtask` stops QEMU 30 s after the deadline, and at once on SIGTERM, SIGHUP or SIGINT. It removes its script on exit, and the next run removes scripts that a killed run left.

## neptron-vkms-probe

A small C program on libdrm. For each mode on its command line it makes a framebuffer with a test pattern, and reports: the test-only and the real atomic commit, the mode read back from the CRTC, the vblank period and the flip-event period next to the period of the mode, three frame CRCs from debugfs, and a writeback capture compared pixel by pixel with the pattern. The exit code is the number of failed commits, readbacks, CRC reads and writeback captures. The periods are reported only, not judged. For an interlaced mode the `vkms` vblank interval on kernel 6.18 is half the field period: `crtc_vtotal` is already halved for interlace, and the timestamping code halves the frame duration again. The probe prints a line that says so.

```text
cargo xtask qemu -- neptron-vkms-probe \
    "320x240 6700 320 336 368 424 240 243 246 263 -hsync -vsync" \
    "720x480i 13500 720 739 801 858 480 488 494 525 -hsync -vsync interlace"
```

A mode is `NAME CLOCK_KHZ HDISP HSS HSE HTOTAL VDISP VSS VSE VTOTAL` and flags from `+hsync -hsync +vsync -vsync interlace doublescan`.

## Where the build data lives

Nothing large is in the repository.

| What | Where |
| --- | --- |
| Downloads | Docker volume `neptron-br-dl` |
| Compiler cache | Docker volume `neptron-br-ccache` |
| Buildroot output | Docker volume `neptron-br-out-<arch>` |
| Images for QEMU | `~/.cache/neptron/buildroot/images/<arch>` (`NEPTRON_CACHE_DIR` moves `~/.cache/neptron`) |

The downloads, the compiler cache and the output are Docker volumes because Buildroot needs a case-sensitive filesystem and the macOS default is not. All worktrees share them. `NEPTRON_OS_IMAGES` or `--images` boots an images directory from somewhere else, for example the base image that CI publishes.

## Licences

The files of this tree are `MIT OR Apache-2.0`, as the rest of NEPTRON. Buildroot is GPL-2.0-or-later. It runs as a build tool and is not part of the repository. The packages in the images keep their own licences, which `make legal-info` lists.
