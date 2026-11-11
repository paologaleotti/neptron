# os

The Buildroot br2-external tree of NEPTRON. It defines the OS images. Today it has the two QEMU targets that tier T3 runs on. The real x86\_64 and Raspberry Pi images come later and share this tree.

## Pinned versions

| What | Version | Pin |
| --- | --- | --- |
| Buildroot | 2026.08.1, the newest stable release on 2026-11-11 | Tag commit `7fdeadb3b9f12f78e5fccca914d7fef47090ab9a`; `buildroot-2026.08.1.tar.gz` SHA-256 `38a6c152490096018720a9ade379ee1e571c9bf5f93a3657a166566945a98b0d`, from the signed release announcement. The development container fetches and checks it |
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

The kernel configuration of each QEMU target is the Buildroot QEMU configuration (`board/qemu/aarch64-virt/linux.config` or `board/qemu/x86_64/linux.config` in the Buildroot tree) plus the four fragments in `board/qemu`, one for each concern.

## The images

Both QEMU targets have:

- BusyBox init, with `mdev` for device nodes. There is no udev daemon and no systemd.
- A root shell on the serial console, with no login.
- An ext4 root filesystem, 256 MB, on a virtio disk.

## Licences

The files of this tree are `MIT OR Apache-2.0`, as the rest of NEPTRON. Buildroot is GPL-2.0-or-later. It runs as a build tool and is not part of the repository. The packages in the images keep their own licences, which `make legal-info` lists.
