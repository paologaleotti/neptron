# os

The Buildroot br2-external tree of NEPTRON. It defines three OS images:

- the x86\_64 PC image: the USB image for the i3-6100T, and also the image that QEMU boots for the x86\_64 checks;
- the aarch64 QEMU test image, which tier T3 runs on a Mac;
- the 64-bit Raspberry Pi image for the Pi 3 B and 3 B+ (see "The Raspberry Pi image").

`os/REFERENCE.md` has the details: the build checks, the kernel configuration, the rules of the state and share partitions, the network share, the Pi board, the cores, and the measurements.

## Pinned versions

| What | Version | Pin |
| --- | --- | --- |
| Buildroot | 2026.08.1, the newest stable release on 2026-11-11 | Tag commit `7fdeadb3b9f12f78e5fccca914d7fef47090ab9a`; `buildroot-2026.08.1.tar.gz` SHA-256 `38a6c152490096018720a9ade379ee1e571c9bf5f93a3657a166566945a98b0d`, from the signed release announcement. `Dockerfile` fetches and checks it |
| Linux | 6.18.63, the newest 6.18 LTS release on 2026-11-11 | SHA-256 in `patches/linux/linux.hash`, from kernel.org |
| 15 kHz patch set (PC image) | `D0023R/linux_kernel_15khz`, folder `linux-6.18`, commit `d85a343379f7f54155f69ef56dc544b666635894` (2026-11-15), GPL-2.0-only | The nine patch URLs in the defconfig name the commit; their SHA-256 values are in `patches/linux/linux.hash` |
| Crates of the frontend and the launcher | `Cargo.lock` of the checkout | `cargo vendor --locked` checks each crate against its SHA-256 in `Cargo.lock`, and the build then runs offline. There is no `.hash` file for them in this tree |
| C library | glibc, from the Buildroot toolchain | Buildroot 2026.08.1 |
| Container base | `debian:trixie-slim` | Image digest in `Dockerfile` |

Buildroot is not in the repository and not a submodule: the container downloads the release tarball and checks its hash.

## Layout

| Path | Contents |
| --- | --- |
| `external.desc`, `external.mk`, `Config.in` | The br2-external tree, name `NEPTRON`. `external.mk` also holds the shared rules of the packages that build the Rust workspace |
| `configs/neptron_pc_x86_64_defconfig` | The PC image, x86-64. It boots from a USB stick (BIOS or UEFI) and in QEMU (`q35`) |
| `configs/neptron_pc_x86_64_dev.fragment` | The development variant: SSH with one public key, the `control` feature |
| `configs/neptron_pc_x86_64_no15khz.fragment` | The PC image without the 15 kHz patch set |
| `configs/neptron_qemu_aarch64_defconfig` | The QEMU test image, `virt` machine, 64-bit Arm. Runs with the hypervisor on an Apple Silicon Mac |
| `configs/neptron_rpi_arm64_defconfig`, `configs/neptron_rpi_arm64_dev.fragment` | The Raspberry Pi image, 64-bit, for the Pi 3 B and 3 B+, and its development variant |
| `board/rpi` | The Pi board: kernel fragments, the kernel hash in `patches/`, `config.txt`, `cmdline.txt`, `genimage.cfg`, the post-build and post-image steps, the kernel checks, `LICENCES.txt`, and the development overlay |
| `board/pc/*.config` | Kernel fragments of the PC image, one for each concern: `system` (the `performance` CPU governor), `video`, `console`, `input`, `sound`, `storage`, `network` |
| `board/pc/busybox.config` | BusyBox fragment of the PC image: no telnet server, no web server, no cron |
| `board/pc/grub.cfg`, `genimage.cfg`, `post-build.sh`, `post-image.sh` | The boot configuration and the USB image |
| `board/pc/LICENCES.txt` | The licence notes of the image, on the boot partition and in `/usr/share/licenses` |
| `board/pc/rootfs-overlay` | The init table of the PC image, the script that loads the QEMU modules, and the script that sets the mixer of each sound card |
| `board/qemu/*.config` | Kernel fragments of the aarch64 test image; `host.config` (firmware config in sysfs, 9p) is in the PC image too; `network.config` has the SMB server |
| `board/qemu/rootfs-overlay` | The init table of the aarch64 test image: a console shell, and the launcher for a boot check only |
| `board/common/busybox.config` | BusyBox fragment: the `timeout` applet |
| `board/common/users.table`, `board/common/post-fakeroot.sh` | The system user `neptron` of the share account, and the check after Buildroot makes the users that it allows no login |
| `board/common/post-build.sh` | Post-build step of every image, with its checks (`os/REFERENCE.md`, "Build checks") |
| `board/common/rootfs-overlay` | The init scripts of every image: debugfs and 9p, the state and share partitions, DHCP, the network share, the launcher, and the commands from the host; the configuration of the SMB server (`etc/ksmbd`) and of the mDNS service (`etc/neptron/mdns.d`) |
| `patches/` | The global patch and hash directory. It holds the kernel hashes and the hashes of the 15 kHz patch set |
| `package/` | NEPTRON packages: the frontend `neptron`, the launcher `neptron-launch`, the client `neptronctl`, the cores, the community gamepad database `sdl-gamecontrollerdb`, and the test tools `neptron-vkms-probe` and `neptron-capture`. `package/neptron/local-source.mk` holds the rsync hooks of the packages that build from this tree |

## The PC image

`cargo xtask os build --arch x86_64` writes `~/.cache/neptron/buildroot/images/x86_64/neptron-x86_64.img`, one disk image for BIOS and UEFI. GRUB boots the kernel with no menu and sets no display mode. The kernel sets no mode of its own. The first mode on the wire after the firmware is the first validated commit of the frontend (`os/REFERENCE.md`, "Boot" and "What a CRT gets before the frontend runs").

**Disk layout.** MBR partition table. The disk signature is fixed for each build variant, so two sticks of the same variant in one PC have the same root PARTUUID, and the kernel can mount the root of the other stick. Plug in one NEPTRON stick at a time. The signatures are `4e505452` (release), `4e505444` (development), `4e50544e` (release without the 15 kHz patches) and `4e505445` (development without them).

| Partition | Filesystem | Size | Contents |
| --- | --- | --- | --- |
| (before 1 MiB) | | | The GRUB boot sector and core image for BIOS |
| 1 boot | FAT32, label `NEPTRONBOOT`, type `0x0c`, active | 64 MiB | `EFI/BOOT/bootx64.efi` and `grub.cfg` for UEFI, `boot/grub/grub.cfg` for BIOS, `bzImage`, `LICENCES.txt`, `legal/` |
| 2 root | EROFS (LZ4HC), read-only | 384 MiB | The OS, the frontend, the cores, `/usr/share/neptron` |
| 3 state | ext4, label `neptron-state` | 256 MiB | Settings, saves, the guard record, the library cache |
| 4 share | exFAT, label `NEPTRON` | The rest of the stick | `roms/<system>`, `bios`. Not in the image: the first boot makes it |

**Storage.** `S06neptron-storage` checks the state partition with `e2fsck` and the share with `fsck.exfat` before each mount. The first boot makes partition 4 in the free space after the state partition, formats it exFAT and mounts it. The script formats only a partition that it made itself, and keeps an old exFAT share that it finds. When a rule refuses, `/share` (or `/state`) is a RAM folder, the reason goes to `/run/neptron/storage.toml` and to a notice, and for some states `Settings > System > Format share` offers a format. Copy ROMs into `roms/nes`, `roms/snes` or `roms/megadrive` from any computer. Copy the games off the stick before you write an image with other partition sizes. The rules, the 18 states and their log lines, and the format that the user confirms are in `os/REFERENCE.md`, "The state and share partitions".

**Power.** A quit restarts the frontend. `Settings > System > Shut down` (or `Reboot`) stops the machine, with the state and share partitions unmounted clean (`os/REFERENCE.md`, "Init, power and sound").

**The QEMU link.** The PC image boots in QEMU as on a PC. Its QEMU hooks act only when the QEMU firmware config exists, and a PC does not have one. `S07neptron-qemu` loads `vkms`, `snd-dummy` and `snd-aloop`. `S99neptron-run` runs the command of `cargo xtask qemu` instead of the launcher. The `once` entry `host-run check` runs the boot check beside the launcher. On x86\_64, `cargo xtask qemu` with no command gives no shell: the image has none.

### The network share

`docs/spec/network-share.md` is the spec, and `os/REFERENCE.md`, "Network share details", has the scripts, the packet filter and what the network reaches. Every image has the share, on by default: release, development and the aarch64 test image.

- **The network.** DHCP on each wired port, in the background: no step of the boot waits for the network, and a cable plugged in later gets its address. No Wi-Fi, no IPv6.
- **The share.** `\\NEPTRON\share`, or `\\neptron.local\share`, or `\\<address>\share`: the share partition `/share` (or its RAM folder), read-write, for guests only. One account exists for Windows 11 24H2, which refuses guest sessions: name `neptron`, password `neptron`. The password is public and gives no protection. `/state` and the root are not shared.
- **The name.** `mdnsd` answers for `neptron.local` and announces `_smb._tcp`. macOS and the Linux file managers list the PC under "Network". The Windows "Network" view does not list it; `\\NEPTRON` works when the router resolves the DHCP host name.
- **Windows 11 24H2** refuses a guest share by default (insecure guest logons off, SMB signing required). Log in as `neptron` with the password `neptron`. A plain open in Explorer sends the Windows account name first, which becomes a guest that Windows refuses, so give the account first: `net use \\NEPTRON\share /user:neptron neptron`, or "Map network drive" with "Connect using different credentials", or a Windows credential for `NEPTRON` in Credential Manager. Or, as guest, turn on "Enable insecure guest logons" and turn off "Digitally sign communications (always)" on the Windows PC. See the spec.
- **The switch.** `S45neptron-share` starts `ksmbd.mountd` and `mdnsd` in a background job, unless `/state/network-share-off` exists. Then port 445 and port 5353 stay closed. `/etc/init.d/S45neptron-share restart` applies a change of the file at once. No settings row does that (OP-092).
- **Only the local network.** A packet filter lets only loopback, `10.0.0.0/8`, `172.16.0.0/12`, `192.168.0.0/16` and `169.254.0.0/16` reach the share. A LAN on another range (for example `100.64.0.0/10`) cannot use it.
- **What is open on the release image**: TCP 445 and UDP 5353 for the local network only, and UDP 68 for the DHCP answers while `udhcpc` has or asks for a lease. No SSH, no telnet.

### The 15 kHz kernel patch set

The PC kernel applies the nine patches of `D0023R/linux_kernel_15khz`, folder `linux-6.18`, at commit `d85a343379f7f54155f69ef56dc544b666635894`. All nine apply to 6.18.63 with no offset and no fuzz. Patch 09 (i915 gen9 interlace) acts only with `i915.no_ytiled_scanout=1`, which the image does not set. Every PC image, release and development, applies the set. `cargo xtask os build --arch x86_64 --no-kernel-15khz` builds the image without them, in its own output, for a comparison with the unpatched driver.

**Licence.** The set is GPL-2.0-only, the licence of the kernel (ADR 0018). At the pinned commit, the `LICENSE` file of the patch repository is the GPL version 2, and its `README.md` states GPL-2.0-only for everything except `xorg/`. The author confirmed it in issue 22 of the repository on 2026-11-15. `board/pc/LICENCES.txt` names the set, its commit and its licence in the image.

**Hashes.** `patches/linux/linux.hash` holds the SHA-256 of the kernel archive, of its licence files and of the nine patches. `patches/linux-headers/linux-headers.hash` is a link to it. With `BR2_KERNEL_HEADERS_AS_KERNEL`, the `linux-headers` package downloads the patches of `BR2_LINUX_KERNEL_PATCH` too, and Buildroot checks them against the hash file of `linux-headers` only. `BR2_DOWNLOAD_FORCE_CHECK_HASHES` makes a missing hash an error ("No hash found for 01_linux_15khz.patch", OP-111). An output that already holds the download stamp of `linux-headers` does not check its downloads again, so only a build from an empty output shows such an error.

**Change the pin.** Read `LICENSE` and the licence section of `README.md` at the new commit, and the history of any new author; a licence that does not allow distribution under GPL-2.0-only stops the change (ADR 0018). Download the nine files of `linux-6.18` at the new commit, compute their SHA-256 (`sha256sum *.patch`), and put the commit in the defconfig URLs and the values in `patches/linux/linux.hash`. A test in `xtask/src/os/build.rs` pins the commit and checks that each patch has a hash in both hash files.

**Checks after a pin change.** These need an image build and QEMU, not hardware. For the pin `d85a3433`, CI ran them on 2026-11-19 (commit `62029d3e`, OP-114): a build of the PC image from an empty output with the patch check, both boot checks, `kms-check` and the video self-test.

1. A build from an empty output: `cargo xtask os build --arch x86_64 --output x86_64-clean` (about 5 hours and 45 GB). The download steps of `linux-headers` and `linux` print `01_linux_15khz.patch: OK (sha256: ...)` for each of the nine patches, and no "No hash found". `build/linux-6.18.63/.applied_patches_list` names the nine patches. `legal-info/sources/linux-6.18.63/` holds the kernel archive, the nine patches and `series`. Record the build time and the SHA-256 of the image.
2. `cargo xtask os boot-check --arch x86_64 --firmware bios`, then `--firmware uefi`, with `NEPTRON_OS_IMAGES` on the images directory of that build.
3. `cargo xtask kms-check --arch x86_64` and `cargo xtask os selftest-check --arch x86_64` on the same image: they run the patched DRM core on `vkms`.

The bench checks (the stick on the PC, a 15 kHz mode on a real CRT) are in `docs/hardware-checklist.md`.

### Variants

| Command | Output | What differs |
| --- | --- | --- |
| `cargo xtask os build --arch x86_64` | `images/x86_64` | The release image |
| `cargo xtask os build --arch x86_64 --dev-key ~/.ssh/bench.pub` | `images/x86_64-dev` | Dropbear with password logins off (`-s`) and only that public key in `/root/.ssh/authorized_keys`; the addresses of the wired ports on the console at each change; the `control` feature of the frontend |
| `cargo xtask os build --arch x86_64 --no-kernel-15khz` | `images/x86_64-no15khz` | No 15 kHz patches. It combines with `--dev-key` |

The key file must hold one public key and lie outside the checkout. No key is in the repository. Each variant has its own Buildroot output volume, so no file of the development variant can reach the release image. The release post-build fails when it finds an SSH or telnet server.

On the development variant, a bench tool that needs the display (the self-test of `neptronctl`) runs only while the frontend and the launcher are stopped. Over SSH: `touch /run/neptron/hold && kill $(pidof neptron-launch)`, then for example `neptronctl selftest video --profile generic_15`. The launcher forwards SIGTERM to the frontend, waits for it and exits, and the descriptor closes. Init starts the launch script again, and the script waits while `/run/neptron/hold` exists. `rm /run/neptron/hold` starts the launcher again. The file is on a RAM file system, so a restart always starts the frontend. `os push` uses the same hold file to restart the launcher.

### Working on the bench

The development variant lets a change reach the i3-6100T without a new image.

1. Build it once, with your public key: `cargo xtask os build --arch x86_64 --dev-key ~/.ssh/id_ed25519.pub`. Write `images/x86_64-dev/neptron-x86_64.img` to a stick (see below) and boot the PC with an Ethernet cable on a network with DHCP.
2. Find the address: ask the router, read the console, which prints `neptron: address eth0 A.B.C.D` when the address changes, or open the Network panel of Settings.
3. `cargo xtask os ssh --host A.B.C.D -- uptime` checks the connection. `os ssh` runs its command with the hot folder first on the PATH, as a login shell does, so `cargo xtask os ssh --host A.B.C.D -- neptronctl stats` runs the pushed `neptronctl`. A plain `ssh root@A.B.C.D neptronctl stats` runs no login shell and gets the image's own `neptronctl`, which can speak an older protocol; use `/state/hot/bin/neptronctl` there. `os ssh` uses your SSH agent or key and never a password; the image accepts no password. The first connection stores the host key in `~/.cache/neptron/bench_known_hosts`. The PC keeps its host key on the state partition (`/state/dropbear`), so it stays the same across reboots. A stick written again with a new image has a new state partition and a new key, so first remove the line of that host from the file.
4. Change the code, then `cargo xtask os push --host A.B.C.D`. It builds the frontend (with `control`), the launcher and `neptronctl` in the bench profile, puts them in `/state/hot/bin`, and restarts the launcher and the frontend. `--data` also replaces the data files (`/state/hot/share/neptron`) with the data of the image: the folders of `data/`, the controller database `input/` and the arcade index, which the push makes from the DAT in the Buildroot output. A warm push takes about a minute.
5. `cargo xtask os logs --host A.B.C.D` prints the log (`/state/log/neptron/current`; svlogd keeps it below 4 MiB and keeps two older files), `dmesg` and the hot folder; `--follow` follows the log. The log has the binary and data paths of each start, every mode the engine set with its read back, the guard events, the notices and the errors.
6. To go back to the image's own programs: `cargo xtask os ssh --host A.B.C.D -- touch /state/hot/DISABLED`, then restart (below). Remove the file to use the hot folder again.

The files go over the SSH connection itself (`cat > file` on the PC, and a tar stream for `--data`): Dropbear has no SFTP server, which the `scp` of current OpenSSH needs.

How the hot folder works: the launch script of the development variant takes `bin/neptron-launch`, `bin/neptron` and `share/neptron` from `/state/hot` when they are there and `/state/hot/DISABLED` is not. A hot binary goes through the same guard and validator as the image's own. The launcher holds the DRM descriptor, so a restart of the frontend alone keeps the mode on the wire. A new launcher binary needs a restart of the launcher, which `os push` does with the hold file (see "Variants"); the frontend ends its run as a quit (save RAM and settings written, exit code 0). `os push` sends SIGKILL only as a fallback, with a line that says so, to a frontend still there after 10 s. The bench profile differs from the release image: no LTO, line tables, no strip, and the `control` feature; `panic = "abort"` is the same.

The release image has none of this: no SSH server, no hot folder, no log file, no `control` feature. Its post-build fails when it finds any of them. Both variants have DHCP and the network share.

**The output now holds the development packages.** The development image of the bench PC was built in the x86\_64 release output (`--output x86_64 --dev-key …`, images in `images/x86_64-dev`), because the Docker disk had no room for a second 45 GB output. So `os push` for the PC uses the toolchain and sysroot of the `x86_64` output. That output holds Dropbear and the files of the development overlay in its target folder, so a release build from it fails its post-build check. Release images come from CI only (OP-077); a local release build needs an empty output (`--output x86_64-clean`, about 45 GB of Docker disk and 5 hours). The local release image that passed the gate on 2026-11-13 is in `~/.cache/neptron/buildroot/images/x86_64-release-fb6dce6c` (SHA-256 `fb6dce6c965b9230e56e8abc675527302481a296ea9187fcc85014e3e0fcf050`). Do not ship it: it is older than the SIGTERM fix of the frontend, the launcher's stop timeout, the log queue and the strict format rules of the share partition, and older than the test tooling of the local tier 3 commands (OP-114).

The development image ran on the i3-6100T on 2026-11-13 (OP-078 to OP-084). `os/REFERENCE.md`, "What QEMU proved for the bench", has the QEMU run of these steps.

**Write the image to a USB stick.** macOS: `diskutil list`, then `diskutil unmountDisk /dev/diskN` and `sudo dd if=$HOME/.cache/neptron/buildroot/images/x86_64/neptron-x86_64.img of=/dev/rdiskN bs=4m`. Linux: `sudo dd if=.../neptron-x86_64.img of=/dev/sdX bs=4M conv=fsync status=progress`. Check the device name twice: `dd` overwrites the whole disk.

## The Raspberry Pi image

`docs/spec/raspberry-pi.md` is the spec, and `os/REFERENCE.md`, "The Raspberry Pi board", has the kernel, the firmware, the disk layout and the packages. `cargo xtask os build --arch rpi-arm64` writes `~/.cache/neptron/buildroot/images/rpi-arm64/neptron-rpi-arm64.img`, one SD card image for the Pi 3 B and the Pi 3 B+. The tests of 2026-11-14 ran on a Pi 3 B: on a composite TV the picture locked in NTSC 240p, and NES, SNES and Mega Drive games held 60.054 frames per second with no missed field (`docs/spec/raspberry-pi.md`, OP-102, OP-103). The image also holds the device tree of the Zero 2 W and its `[pi02]` lines in `config.txt`; the Zero 2 W is not tested and is not a target (no Ethernet, no analog audio). QEMU does not boot this image: its Pi machines have no `vc4` display. `docs/hardware-checklist.md` lists the open checks.

**What works.** The image boots the frontend on an HDMI display through the panel path, as on the PC, or on a TV through the composite output, with the network share, the cores, the state partition and the share partition. The composite connector `Composite-1` reports no display, so the connector choice of `display.output = "auto"` does not take it. When no HDMI display answers within 3 s (`COMPOSITE_FALLBACK_WAIT_NS`), the frontend opens `Composite-1` by name; an HDMI display that answers wins. The first run of a boot writes the output it drives to `run/output`, and every relaunch of that boot opens the same connector. `display.output = "Composite-1"` gives composite with HDMI connected. The TV standard is the setting `display.tv` (ADR 0012). The Pi takes no kernel patch.

**Variants.**

| Command | Output volume and images | What differs |
| --- | --- | --- |
| `cargo xtask os build --arch rpi-arm64` | `rpi-arm64` | The release image |
| `cargo xtask os build --arch rpi-arm64 --dev-key ~/.ssh/bench.pub` | `rpi-arm64-dev` | Dropbear with the one key, the hot folder, the log, the `control` feature (the development overlay of the PC image, and `board/rpi/dev-overlay` on top), and the serial console (GPIO 14 and 15, 115200 baud, 3.3 V, Bluetooth off) |

The release post-build step fails when the image holds a file of either development overlay or an SSH server. The bench commands work with a Pi as with the PC: `cargo xtask os ssh --host A.B.C.D -- uptime`, `cargo xtask os logs --host A.B.C.D`, and `cargo xtask os push --arch rpi-arm64 --host A.B.C.D`. The push builds the frontend, the launcher and `neptronctl` with the Buildroot toolchain and the sysroot of the `rpi-arm64-dev` output (`--output NAME` names another one), in its own cross volume `neptron-cross-rpi-arm64-dev`, so it links against the glibc of the image on the card and never against the glibc of the QEMU aarch64 output. Build the development image once before the first push.

**Write the image to an SD card from macOS.**

1. Insert the card, then run `diskutil list`. Find the card by its size and its "external, physical" label, for example `/dev/disk4`.
2. Check the device name twice. `dd` overwrites the whole device, and a wrong number destroys another disk.
3. `diskutil unmountDisk /dev/disk4`
4. `sudo dd if=$HOME/.cache/neptron/buildroot/images/rpi-arm64/neptron-rpi-arm64.img of=/dev/rdisk4 bs=4m`
5. `sync`, then `diskutil eject /dev/disk4`.

Use `rdisk4` (the raw device), not `disk4`: it is many times faster. For the development image, use `images/rpi-arm64-dev/neptron-rpi-arm64.img`. On Linux: `sudo dd if=.../neptron-rpi-arm64.img of=/dev/mmcblkN bs=4M conv=fsync status=progress`.

**First boot.** Put the card in the Pi, connect an HDMI display or a TV on the composite jack, a USB gamepad and Ethernet, then the power supply (2.5 A). The firmware loads the kernel; the storage script makes partition 4 from the free space of the card, formats it exFAT and mounts it at `/share`; the frontend shows the game list on the HDMI display, or on composite in NTSC when no HDMI display answers within 3 s. A flat panel, a TV and a monitor that gives no EDID start with no prompt and no notice (ADR 0013); only a monitor preset named in the settings for a display that is not known opens the boot prompt, which reverts 15 s after its picture settles. Copy games to `roms/<system>` over the network share (`\\NEPTRON\share`) or with the card in another computer. The development variant prints the address on the serial console at each change.

**Shut down.** The System panel works as on the PC. A Pi has no power switch: after a shut down the red power LED stays on, and only a power cycle (unplug the supply and plug it in again) starts it again.

**Licences.** `board/rpi/LICENCES.txt` goes on the boot partition and into `/usr/share/licenses`. The Pi firmware files are not open source: they may be redistributed in binary form, unchanged, only for use with a Raspberry Pi (`os/REFERENCE.md`, "Licence of the firmware"). The image may be distributed free of charge, unchanged, for use on a Pi, with `LICENCES.txt`, `legal/` and the `legal-info` sources. It may not be sold (the non-commercial cores, OP-054).

## The aarch64 test image

- BusyBox init, with `mdev` for device nodes. There is no udev daemon and no systemd.
- A root shell on the serial console, with no login.
- An ext4 root filesystem, 256 MB, on a virtio disk. QEMU runs it in snapshot mode, so a run never changes the image.
- libdrm with its test programs (`modetest`), `neptron-vkms-probe` and `neptron-capture`.
- Mesa with the softpipe software renderer: EGL, OpenGL ES and GBM, and `kmscube`.
- SDL3 with the KMS/DRM video driver and OpenGL ES, without libudev, and alsa-lib.
- The libretro cores FCEUmm, Snes9x, Genesis Plus GX and FinalBurn Neo in `/usr/lib/libretro`. The first three have the library names of `data/cores`.
- The frontend, the launcher and `neptronctl`. Init starts the launcher only for a boot check (`launch-for-check`), so the other checks get the display.
- The network share, as in the PC image: `ksmbd`, `ksmbd-tools`, `mdnsd`, the packet filter (`iptables`), and the init scripts. `/share` is a RAM folder there.

The build also installs the Rust toolchain of Buildroot in the output, for `cargo xtask gles-compare` (`os/REFERENCE.md`, "The Rust toolchain of the aarch64 output").

## Commands

All commands go through `xtask`. `xtask/README.md` lists every flag.

```text
cargo xtask os build --arch aarch64              build the aarch64 test image
cargo xtask os build --arch x86_64               build the PC image (see "Variants" for the others)
cargo xtask os build --arch rpi-arm64            build the Raspberry Pi image
cargo xtask os build --arch aarch64 -- linux-reconfigure
                                                 rebuild the kernel after a change to its fragment
cargo xtask qemu -- uname -a                     boot, run one command, exit with its exit code
cargo xtask qemu --script test.sh --share dir    run a script; dir is read-only at /mnt/host
cargo xtask qemu                                 interactive console; Ctrl-A then X stops QEMU
cargo xtask gles-compare                         build the GLES comparison for the guest and run it
cargo xtask core-trace                           run the test ROMs in the real cores (tier T4)
cargo xtask os boot-check --arch x86_64 [--firmware uefi]
                                                 boot the image to the game list and compare the picture (tier T3)
cargo xtask os share-check                       boot copies of a stick through the share cases (tier T3)
```

`os build` builds the development container from `Dockerfile`, then runs Buildroot in it with 4 jobs, 4 CPUs and 6 GB of memory. It runs `make legal-info` before the build, because the image carries the licence texts and the manifest. A machine-wide lock allows one Buildroot build at a time. Buildroot does not see a change to a kernel fragment by itself: give the `-reconfigure` target as above. Buildroot does not rebuild a package when its options change either: after a defconfig change, a package can need a `-dirclean` by hand (`mesa3d` and `util-linux` did).

Buildroot does not see a change to the source of a package that builds from this tree (`neptron`, `neptron-launch`, `neptronctl`, `neptron-capture`, `neptron-vkms-probe`) either. So `os build` adds `<package>-reconfigure` for each of them that the configuration turns on, before the targets after `--`; do not name them. `-reconfigure`, not `-rebuild`: the configure step runs `cargo vendor`, which a `Cargo.lock` with a new crate needs. A `-rebuild` or a `-reconfigure` of a local package copies its source again with `rsync -au`. That copy keeps the file times of the source and skips a file whose copy in the build folder is newer. So a file of another worktree, or a file that changed before the last build, can keep its old copy or get a time older than the build output, and Cargo then takes the old output as fresh. The rsync hooks in `package/neptron/local-source.mk` remove the old copy before the rsync and give each copied file the current time after it. The Cargo build output, the vendored crates and `.cargo/` stay, so only the workspace crates compile again.

`qemu` boots the image with 2 GB of memory and 2 CPUs. A machine-wide lock allows one guest at a time. On macOS QEMU runs under `taskpolicy -c utility`. The aarch64 guest uses the hypervisor on an Apple Silicon Mac and KVM on an aarch64 Linux host. The x86\_64 guest uses KVM on an x86-64 Linux host. Everywhere else QEMU emulates the CPU, which is too slow for routine use: on a Mac, use the aarch64 target.

How a command runs:

1. `xtask` writes the command as a shell script and gives it to QEMU as the firmware config entry `opt/neptron/run`. Two more entries carry a new random nonce (`opt/neptron/nonce`) and the deadline in seconds (`opt/neptron/timeout`, from `--timeout`, default 120).
2. The init script `S99neptron-run` (through `/usr/libexec/neptron/host-run run`) prints `NEPTRON-BEGIN <nonce> <uptime>`, runs the script under `timeout -s KILL <deadline>`, prints `NEPTRON-END <nonce> <exit code>` and reboots. `-no-reboot` turns the reboot into a QEMU exit. A command over its deadline ends with exit code 137, and the guest stops even when no host process watches it.
3. `xtask` accepts only markers with its nonce, so output that contains the marker text does not end the run. It prints the lines between the markers, the time to the first marker and the guest uptime, and exits with the exit code of the command. It writes the whole console to `target/qemu/<arch>-console.log`.
4. `xtask` stops QEMU 30 s after the deadline, and at once on SIGTERM, SIGHUP or SIGINT. It removes its script on exit, and the next run removes scripts that a killed run left.

The boot check expects a state and a share partition on x86\_64 (its disk copy has 128 MiB free after the state partition, so the first boot makes, formats and mounts partition 4) and two RAM folders on aarch64, whose notices it dismisses. It also checks the network share (`os/REFERENCE.md`, "The boot check of the share"). `cargo xtask os share-check` boots copies of a stick written again, of a format offer and of a reused card (`xtask/README.md` describes both). In CI, on every push of `main`, `kms-check`, `gles-compare`, `os selftest-check`, `core-trace` and `os share-check` run on the x86\_64 image under KVM (`docs/ci.md`).

## Check a release

These commands read a release image and its sources archive with no Pi and no PC. They need `xz`, `sfdisk` and `jq` (Linux) and mtools (`brew install mtools` on macOS). The names are those of release 0.1.0 of the PC image; for the Pi image use `neptron-rpi-arm64`. A local build has the same files in `~/.cache/neptron/buildroot/images/<variant>` (the image and `boot-part/`).

```sh
img=neptron-pc-x86_64-0.1.0.img
xz -dk "$img.xz" && sha256sum "$img"            # equals the SHA-256 in the release notes
start=$(sfdisk --json "$img" | jq '.partitiontable.partitions[0].start')
boot="$img@@$((start * 512))"
export MTOOLS_SKIP_CHECK=1
mtype -i "$boot" ::LICENCES.txt | head -12      # release v0.1.0, the commit of the tag, Variant: release
mdir -i "$boot" ::legal/neptron-rust            # neptron.txt, neptron-launch.txt, neptronctl.txt
mdir -i "$boot" ::legal/licenses/neptron-0.1.0  # LICENSE-GPL-3.0, LICENSE-MIT, LICENSE-APACHE, vendor, data, crates
mtype -i "$boot" ::legal/manifest.csv | grep -E '^"?(neptron|neptronctl|libretro-genesis-plus-gx)'
tar -tf neptron-pc-x86_64-0.1.0-sources.tar | grep -E \
  'README.txt|buildroot-.*\.tar\.gz|neptron-[0-9a-f]{40}(-vendor)?\.tar\.xz|legal-info/(manifest.csv|sources/linux-6\.18\.63/series)'
```

Expected: the head of `LICENCES.txt` names the release, the 40-hex commit of the tag (no `-modified`), its date and `Variant: release`; the manifest gives `neptron` and `neptronctl` as `GPL-3.0+ (binary), ...` and `libretro-genesis-plus-gx` with its LGPL parts; the archive lists `README.txt`, the Buildroot archive, the NEPTRON and vendor archives of the commit, the manifest and, for the PC, the `series` file of the kernel patches. The root file system is EROFS: on Linux, `sudo mount -o ro,loop,offset=$((root_start * 512)) "$img" /mnt` with the start of partition 2 shows `/mnt/etc/neptron-release` and `/mnt/usr/share/licenses/neptron-rust`.

## neptron-vkms-probe

A small C program on libdrm. For each mode on its command line it makes a framebuffer with a test pattern, and reports: the test-only and the real atomic commit, the mode read back from the CRTC, the vblank period and the flip-event period next to the period of the mode, three frame CRCs from debugfs, and a writeback capture compared pixel by pixel with the pattern. The exit code is the number of failed commits, readbacks, CRC reads and writeback captures. The periods are reported only, not judged. For an interlaced mode the `vkms` vblank interval on kernel 6.18 is half the field period: `crtc_vtotal` is already halved for interlace, and the timestamping code halves the frame duration again. The probe prints a line that says so.

```text
cargo xtask qemu -- neptron-vkms-probe \
    "320x240 6700 320 336 368 424 240 243 246 263 -hsync -vsync" \
    "720x480i 13500 720 739 801 858 480 488 494 525 -hsync -vsync interlace"
```

A mode is `NAME CLOCK_KHZ HDISP HSS HSE HTOTAL VDISP VSS VSE VTOTAL` and flags from `+hsync -hsync +vsync -vsync interlace doublescan`. On `vkms` (kernel 6.18) the probe passes for 320x240 at 6.7 MHz, 640x480, 720x480i and a doublescan mode (`os/REFERENCE.md`, "What was verified").

## Not verified

`os/REFERENCE.md`, "What was verified", has what the QEMU runs and the builds proved.

- On the i3-6100T: the open checks of `docs/hardware-checklist.md` (the DP-to-VGA bridge, the boot time and the rest). The development image ran there on 2026-11-13: a game played, with audio and the SMB share.
- The aarch64 timing checks in CI (`kms-check`, `selftest-check`, `core-trace`): the arm64 runners have no KVM (OP-104).
- A network login to the share, as guest or as `neptron`: QEMU runs have no network card.
- The local images come from an output that was built up step by step (the defconfig changed several times). CI built the PC image from an empty output on 2026-11-19 (OP-114). No local build of the PC image in an empty output is recorded.

## Where the build data lives

Nothing large is in the repository.

| What | Where |
| --- | --- |
| Downloads | Docker volume `neptron-br-dl` |
| Compiler cache | Docker volume `neptron-br-ccache` |
| Buildroot output | Docker volume `neptron-br-out-<variant>`: `aarch64`, `x86_64`, `x86_64-dev`, `x86_64-no15khz`, `x86_64-dev-no15khz`, `rpi-arm64`, `rpi-arm64-dev` |
| Cargo registry and target directories of the guest builds | Docker volume `neptron-cross-<output>`: `aarch64`, `x86_64`, and `rpi-arm64-dev` for `os push` to a Pi. One target directory per checkout, `target/wt-<folder>-<hash>`; each guest build removes the directories of checkouts that no longer exist |
| Images | `~/.cache/neptron/buildroot/images/<variant>` (`NEPTRON_CACHE_DIR` moves `~/.cache/neptron`) |

The downloads, the compiler cache and the output are Docker volumes because Buildroot needs a case-sensitive filesystem and the macOS default is not. All worktrees share them. `NEPTRON_OS_IMAGES` or `--images` boots an images directory from somewhere else, for example the base image that CI publishes.

## Licences

The files of this tree are `MIT OR Apache-2.0`, as the rest of NEPTRON. An image is an aggregate: each program in it keeps its own licence, and the three non-commercial cores make an image that holds them not for sale (ADR 0019, OP-054, `os/REFERENCE.md`, "The cores"). Buildroot is GPL-2.0-or-later. It runs as a build tool and is not part of the repository. The packages in the images keep their own licences, which `make legal-info` lists. The network share adds the kernel server `ksmbd` (GPL-2.0), `ksmbd-tools` (GPL-2.0-or-later), GLib and libnl (LGPL-2.1-or-later), PCRE2 (BSD-3-Clause), libffi (MIT), `mdnsd` (BSD-3-Clause) and `iptables` (GPL-2.0); `board/pc/LICENCES.txt` lists them. `neptron` and `neptronctl` link Switchres and are GPL-3.0-or-later (ADR 0002).

The PC and Pi images carry the manifest, the licence texts and the notices of the Rust crates in each NEPTRON program (`legal/neptron-rust`, made by `cargo xtask rust-notices`) on their boot partition (`legal/`) and in `/usr/share/licenses`, with the notes of `board/pc/LICENCES.txt` or `board/rpi/LICENCES.txt`. `board/common/install-licences.sh` puts them in the image and writes `/etc/neptron-release`. The source tarballs of the packages are in `legal-info/sources` of the Buildroot output. legal-info does not save the source of the NEPTRON packages, because its copy of a local package ignores the rsync exclusions; the sources archive of CI holds the NEPTRON tree at the commit and the vendored crates instead (`docs/ci.md`, "The sources archive").

The 15 kHz patch set is GPL-2.0-only (see "The 15 kHz kernel patch set" and ADR 0018). The `make legal-info` output holds the kernel source in `sources/linux-6.18.63`: the kernel archive, the nine patches and the file `series` with their order. `board/pc/LICENCES.txt` and `board/rpi/LICENCES.txt` carry the same written offer (GPL-2.0 section 3(b)) for the source of every GPL and LGPL package of the image, valid for at least three years from the last date on which the project gives out the image, with an issue in the public repository as the contact (ADR 0018). The rule for the source: the project gives out the source together with the image, from the same place; the written offer applies in every case, also for an image that reached somebody without its source. The binary-only Pi firmware is not covered. The QEMU aarch64 test image is not given to anybody and has no offer. A local `os build` leaves the sources in its Docker output volume. CI makes a sources archive for each image that it uploads and keeps it for 7 days, beside the image. A release publishes each image with its sources archive on the GitHub Release page of the version, where release assets do not expire (`docs/ci.md`, "Release").

**Keep the source.** The project keeps the sources archive of every image it gives out for at least three years after the last time it gives it out, so it can answer the written offer: it keeps each release, with its assets, for that long.

**Name the image.** `LICENCES.txt` on the boot partition and `/etc/neptron-release` in the root name the NEPTRON commit (with `-modified` for a tree with changes), the commit date and the release, so a person with only the image can name it in a request for the source. The date is that of the commit, so two builds of one commit give the same files.

No firmware file is in the image. For a later decision: the i915 files of `linux-firmware` are under `LICENSE.i915` (Intel: redistribution in binary form allowed, no modification and no reverse engineering), and the amdgpu files under `LICENSE.amdgpu` (AMD: terms of the same kind). Neither is an open source licence.
