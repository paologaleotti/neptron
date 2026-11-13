# os

The Buildroot br2-external tree of NEPTRON. It defines the OS images: the x86\_64 PC image, which is the USB image for the i3-6100T and also the image that QEMU boots for the x86\_64 checks, and the aarch64 QEMU test image that tier T3 runs on a Mac. The Raspberry Pi image comes later and shares this tree.

## Pinned versions

| What | Version | Pin |
| --- | --- | --- |
| Buildroot | 2026.08.1, the newest stable release on 2026-11-11 | Tag commit `7fdeadb3b9f12f78e5fccca914d7fef47090ab9a`; `buildroot-2026.08.1.tar.gz` SHA-256 `38a6c152490096018720a9ade379ee1e571c9bf5f93a3657a166566945a98b0d`, from the signed release announcement. `Dockerfile` fetches and checks it |
| Linux | 6.18.63, the newest 6.18 LTS release on 2026-11-11 | SHA-256 in `patches/linux/linux.hash`, from kernel.org |
| 15 kHz patch set (PC image) | `D0023R/linux_kernel_15khz`, folder `linux-6.18`, commit `8c96dd9f0b343a6d1ef3c61041783ee106957886` (2026-11-07) | The nine patch URLs in the defconfig name the commit; their SHA-256 values are in `patches/linux/linux.hash`. See the licence question below |
| Crates of the frontend and the launcher | `Cargo.lock` of the checkout | `cargo vendor --locked` checks each crate against its SHA-256 in `Cargo.lock`, and the build then runs offline. There is no `.hash` file for them in this tree |
| C library | glibc, from the Buildroot toolchain | Buildroot 2026.08.1 |
| Container base | `debian:trixie-slim` | Image digest in `Dockerfile` |

Buildroot 2026.08.1 is a bug-fix release of 2026.08. It is not in the repository and not a submodule: the container downloads the release tarball and checks its hash.

## Layout

| Path | Contents |
| --- | --- |
| `external.desc`, `external.mk`, `Config.in` | The br2-external tree, name `NEPTRON`. `external.mk` also holds the shared rules of the packages that build the Rust workspace |
| `configs/neptron_pc_x86_64_defconfig` | The PC image, x86-64. It boots from a USB stick (BIOS or UEFI) and in QEMU (`q35`) |
| `configs/neptron_pc_x86_64_dev.fragment` | The development variant: SSH with one public key, the `control` feature |
| `configs/neptron_pc_x86_64_no15khz.fragment` | The PC image without the 15 kHz patch set |
| `configs/neptron_qemu_aarch64_defconfig` | The QEMU test image, `virt` machine, 64-bit Arm. Runs with the hypervisor on an Apple Silicon Mac |
| `board/pc/*.config` | Kernel fragments of the PC image, one for each concern: `video`, `console`, `input`, `sound`, `storage`, `network` |
| `board/pc/busybox.config` | BusyBox fragment of the PC image: no telnet server, no web server, no cron |
| `board/pc/grub.cfg`, `genimage.cfg`, `post-build.sh`, `post-image.sh` | The boot configuration and the USB image |
| `board/pc/LICENCES.txt` | The licence notes of the image, on the boot partition and in `/usr/share/licenses` |
| `board/pc/rootfs-overlay` | The init table of the PC image, the script that loads the QEMU modules, and the script that sets the mixer of each sound card |
| `board/qemu/*.config` | Kernel fragments of the aarch64 test image; `host.config` (firmware config in sysfs, 9p) is in the PC image too; `network.config` has the SMB server |
| `board/qemu/rootfs-overlay` | The init table of the aarch64 test image: a console shell, and the launcher for a boot check only |
| `board/common/busybox.config` | BusyBox fragment: the `timeout` applet |
| `board/common/users.table`, `board/common/post-fakeroot.sh` | The system user `neptron` of the share account, and the check after Buildroot makes the users that it allows no login |
| `board/common/post-build.sh` | Post-build step of both images: removes the init script of the `mdnsd` package, writes `/etc/network/interfaces` with the loopback only (an output built with `BR2_SYSTEM_DHCP` keeps a DHCP stanza, because Buildroot does not install `ifupdown-scripts` again), and checks the files of the network share |
| `board/common/rootfs-overlay` | The init scripts of both images: debugfs and 9p, the state and share partitions, DHCP, the network share, the launcher, and the commands from the host; the configuration of the SMB server (`etc/ksmbd`) and of the mDNS service (`etc/neptron/mdns.d`) |
| `patches/` | The global patch and hash directory. It holds the kernel hashes and the hashes of the 15 kHz patch set |
| `package/` | NEPTRON packages: the frontend `neptron`, the launcher `neptron-launch`, the client `neptronctl`, the three MVP cores, and the test tools `neptron-vkms-probe` and `neptron-capture` |

The kernel of the aarch64 test image is the Buildroot QEMU configuration (`board/qemu/aarch64-virt/linux.config` in the Buildroot tree) plus the five fragments in `board/qemu`. The kernel of the PC image is the `x86_64` defconfig of the kernel plus the fragments in `board/pc` and `board/qemu/host.config`.

## The PC image

`cargo xtask os build --arch x86_64` writes `~/.cache/neptron/buildroot/images/x86_64/neptron-x86_64.img`, one disk image for BIOS and UEFI.

**Disk layout.** MBR partition table. The disk signature is fixed for each build variant, so two sticks of the same variant in one PC have the same root PARTUUID, and the kernel can mount the root of the other stick. Plug in one NEPTRON stick at a time. The signatures are `4e505452` (release), `4e505444` (development), `4e50544e` (release without the 15 kHz patches) and `4e505445` (development without them).

| Partition | Filesystem | Size | Contents |
| --- | --- | --- | --- |
| (before 1 MiB) | | | The GRUB boot sector and core image for BIOS |
| 1 boot | FAT32, label `NEPTRONBOOT`, type `0x0c`, active | 64 MiB | `EFI/BOOT/bootx64.efi` and `grub.cfg` for UEFI, `boot/grub/grub.cfg` for BIOS, `bzImage`, `LICENCES.txt`, `legal/` |
| 2 root | EROFS (LZ4HC), read-only | 384 MiB | The OS, the frontend, the cores, `/usr/share/neptron` |
| 3 state | ext4, label `neptron-state` | 256 MiB | Settings, saves, the guard record, the library cache |
| 4 share | exFAT, label `NEPTRON` | The rest of the stick | `roms/<system>`, `bios`. Not in the image: see below |

The definition asks for no share partition in the image: the first boot makes it. `S06neptron-storage` works only on the disk that holds the root filesystem. When that disk has no partition 4, the state partition is mounted, and at least 64 MiB are free after the state partition, the script first looks for an old exFAT share in the free space: an exFAT boot sector at any 1 MiB boundary of the first 4 GiB. An old share at another start than the planned one leaves the disk untouched. Else the script writes the marker `/state/share-pending` (with a sync), appends partition 4 (type `0x07`) over the free space with `sfdisk --force --no-reread --append` (the disk is in use, and the other partitions stay as they are), and adds it to the kernel with `partx`.

A partition 4 with a filesystem is mounted as it is and never formatted. An empty one is formatted exFAT only when all of these hold:

- the marker `/state/share-pending` is there: this system made the partition; the script removes the marker after the first mount;
- its type is `0x07` and it starts at the first 1 MiB boundary after the state partition;
- its first 16 MiB read with no error, and `blkid -p` exits with no filesystem found and prints nothing on standard error;
- its sector 12 holds no exFAT backup boot sector (a share with a damaged main boot sector is the user's data).

When any check fails, nothing is formatted or mounted, `/share` is a RAM folder, and one log line says that the share partition was left untouched and why. The state partition gets `e2fsck -p` first; when it leaves errors (exit 4 or more), `/state` is a RAM folder and nothing is written to the damaged filesystem. At the end, the script writes `/run/neptron/storage.toml`: `state` and `share`, each `partition` or `ram`, and for a RAM folder `state_reason` or `share_reason`, the last log line before it. The frontend reads it at boot, logs it, and shows a notice for each RAM folder until the player presses OK. With `/state` in RAM the guard record still counts the failed boots of this power cycle, so the guard boots as usual. The boot check (`cargo xtask os boot-check`) states the storage of its disk: on x86\_64 a state and a share partition (the check boots a copy of the disk with free space after the state partition, so the first boot makes the share as on a real stick), on aarch64 two RAM folders (the root file system boots alone; the QEMU test image has the control commands for the dismissal). It checks `storage.toml` and dismisses the notice of each RAM folder before it waits for the picture of the golden. After the share is mounted, the script makes `roms/<system>` for each system of `data/systems`, and `bios`. Copy ROMs into `roms/nes`, `roms/snes` or `roms/megadrive` from any computer.

**Limits of the share rules.** A power cut during `mkfs.exfat` can leave a filesystem that `blkid` sees but that does not mount. The script then does not format it again, the marker stays, and `/share` is a RAM folder at every boot; no data is lost, because the partition was new. To recover, format partition 4 as exFAT on another computer, or remove it so that the next boot makes it again. The scan for an old share looks at the first 4 GiB of free space only, so an old share that starts beyond that is not found. The x86\_64 boot check runs the first-boot share path: it boots a copy of the disk with 128 MiB free after the state partition, so the script makes, formats and mounts partition 4. The share cases A to G below are the proof of these rules.

**Writing a new image to a stick that has a share.** A new image writes a new partition table and a new state partition, with no marker. With the same layout, the first boot makes partition 4 again at the same start, finds the old exFAT share there, and mounts it: the games stay. An image of a different layout (other sizes of the root or the state partition) would place the new partition 4 at another start, perhaps inside the old share. The script then finds the old share's boot sector at its own start, makes no partition, and logs `an old exFAT share starts at sector N`; the frontend shows an empty game list, because `/share` is a RAM folder. The games are still on the stick: copy them from the stick on another computer, with a tool that reads exFAT at that offset (or add a partition entry that starts at sector N by hand), then delete the old share and boot again. If the old share's boot sector was overwritten too, the script cannot see it and makes and formats the new partition; copy the games off the stick before writing an image of another layout.

**Boot.** GRUB, `timeout=0`, no menu and no video module, so it sets no display mode. The kernel command line is `root=PARTUUID=<signature>-02 rootfstype=erofs ro rootwait quiet console=ttyS0,115200 panic=10`. The kernel has the USB, SATA, NVMe and virtio disk drivers, EROFS, ext4, exFAT and FAT built in. There is no initramfs.

**What a CRT gets before the frontend runs.** NEPTRON sets no mode until the frontend makes its first validated commit, but the PC sets one before it:

1. The BIOS or UEFI POST, and the GRUB text console (GRUB loads no video module, so it keeps that mode), show in the mode that the firmware set. That mode is 31 kHz or higher.
2. i915 inherits that mode and keeps it on the wire until the first validated commit of the frontend. In QEMU the first frame came 15 to 25 s after the start of the kernel under CPU emulation; on the i3-6100T this time is not measured.
3. A monitor that takes 15 kHz only gets an out-of-range signal for that whole time. Use an adapter that protects the monitor from such a signal, or switch the monitor on after the boot.
4. The definition has the launcher blank the output to shorten this window. This image does not do that step: the launcher sets no mode and makes no commit (`apps/neptron-launch/README.md`).

**The kernel sets no mode of its own** (the safety model, layer 8). There is no virtual terminal (`CONFIG_VT` off, which needs `CONFIG_EXPERT`), no fbdev emulation, no `CONFIG_FB`, no framebuffer console, no `simpledrm`, `efidrm` or `vesadrm`, no firmware framebuffer device, no DRM log client and no DRM panic screen. `post-build.sh` reads the final kernel configuration and fails the build when one of these is on. i915 is built in. amdgpu, radeon and vkms are modules. The kernel log goes to the first serial port only.

**Init.** BusyBox init with `mdev`. There is no getty and no login on any console. The init scripts mount the partitions and `/run` (tmpfs). Then init starts `/usr/libexec/neptron/launch`, which makes `/run/neptron` and runs `neptron-launch --power -- /usr/bin/neptron --backend kms`. Before that, the launch script sources `/usr/libexec/neptron/mesa-cache`: when `/state` is the ext4 state partition mounted read-write, Mesa keeps its shader cache in `/state/cache/mesa` (`MESA_SHADER_CACHE_DIR`, at most 64 MiB), else the cache is off (`MESA_SHADER_CACHE_DISABLE`). The read-only root has no place for it. The cache can be deleted at any time, and a folder that cannot be made only turns the cache off. The launcher holds the DRM descriptor and starts the frontend again when it ends. Init starts the launcher again when it ends. So a "quit" is a restart of the frontend, not a power-off: the frontend ends with exit code 0, the launcher stops, and init starts the launch script again at once. To stop, use `Settings > System > Shut down` (or `Reboot`): the frontend ends with exit code 81 (80), and the launcher, which has `--power`, runs BusyBox `poweroff` (`reboot`). Init then runs the `::shutdown:` entries of `inittab` (the stop script, `rcK`, `umount -a -r`), so the save RAM and the settings are on disk and the ext4 state and exFAT share partitions are unmounted clean before the machine turns off. The PC turns off through ACPI (`CONFIG_ACPI`, which `check-kernel-config.sh` requires); the aarch64 QEMU image through PSCI. A power cut is still safe: the state and share partitions are written with care for it.

A stop of the launcher with SIGTERM (`kill $(pidof neptron-launch)`, or init at shutdown) forwards SIGTERM to the frontend, which ends its run as a quit. A frontend that has not ended 5 s later, for example one stuck inside a core, gets SIGKILL from the launcher. Only that timeout, the SIGKILL fallback of `os push`, or init ends a frontend that hangs. The frontend reads its folders from the image defaults: `/share`, `/state`, `/usr/share/neptron`, `/usr/lib/libretro`, `/run/neptron`.

**Packages.** `ksmbd-tools` and `mdnsd` for the network share (see "The network share"). Mesa 26.1 with LLVM: the iris driver for Intel gen8 and later (the HD 530 of the i3-6100T), llvmpipe, and softpipe; EGL, OpenGL ES and GBM. SDL3 with KMS/DRM and OpenGL ES, without libudev, and alsa-lib. `amixer` of alsa-utils, and nothing else of it. libdrm. The three cores. The frontend, the launcher, `neptronctl` (with the `kms` feature, for the bench self-test), `neptron-vkms-probe` and `neptron-capture`. e2fsprogs, exfatprogs, and the basic and partition tools of util-linux, for the partitions. There is no firmware file: i915 drives the display without its DMC firmware, which is not loaded, and amdgpu does not work without its firmware (see "Licences").

**Sound.** There is no sound server: SDL3 talks to ALSA. The kernel leaves an HDA codec at its reset state, with the DAC at the bottom of its range and the output pins muted (seen on the i3-6100T with its ALC221). `S08neptron-mixer` runs in the background at boot: it waits up to 10 s for a card, then sets the playback controls `Master`, `PCM`, `Front`, `Surround`, `Center`, `LFE`, `Side`, `Headphone`, `Speaker`, `Line Out` and every `IEC958` switch of each card to 0 dB and unmutes them. 0 dB, not 100%: a control that goes above 0 dB would add gain and could clip. The input controls stay as they are. The volume of the frontend then scales the samples in software (`docs/spec/audio-settings.md`). A card plugged in after the boot keeps its own state.

**The frontend, the launcher and `neptronctl`** are Buildroot packages that build this checkout (`package/neptron`, `package/neptron-launch`, `package/neptronctl`). `xtask` mounts the checkout read-only at `/src/repo`. Each package copies it without `target/`, `.worktrees/` and `.git`, runs `cargo vendor --locked`, and builds offline in the workspace release profile (fat LTO, one codegen unit, `panic = "abort"`). The sdl3 crate adds `/usr/local/lib` to the link search on every Linux host, and the Buildroot compiler wrapper refuses that path. `package/neptron/link-filter.sh` drops only that path and links with the wrapper. With `panic = "abort"` no destructor runs. The launcher keeps its descriptor open, so the framebuffers and the mode stay on the wire until the next frontend commits (`docs/spec/kms.md`).

**The QEMU link.** The PC image boots in QEMU as on a PC. Its QEMU hooks act only when the QEMU firmware config exists, and a PC does not have one. `S07neptron-qemu` loads `vkms`, `snd-dummy` and `snd-aloop`. `S99neptron-run` runs the command of `cargo xtask qemu` instead of the launcher. The `once` entry `host-run check` runs the boot check beside the launcher. On x86\_64, `cargo xtask qemu` with no command gives no shell: the image has none.

### The network share

`docs/spec/network-share.md` is the spec. Both images, release and development, and the aarch64 test image have it, on by default.

- **The network.** `S40neptron-network` starts `udhcpc` on each wired port (`eth*`, `en*`) as a background job, with the host name `neptron` in the request, and returns at once. No step of the boot waits for the network: a PC with no cable boots as fast as before, and a cable plugged in later gets its address. The release image has no firmware file; the RTL8168h of the bench PC works without its firmware patch. There is no Wi-Fi.
- **The share.** `\\NEPTRON\share`, or `\\neptron.local\share`, or `\\<address>\share`: the share partition `/share` (or its RAM folder), read-write, for guests only. The kernel server `ksmbd` (`CONFIG_SMB_SERVER=y`) serves SMB 2.1 to 3.1.1; `ksmbd.mountd` reads `/etc/ksmbd/ksmbd.conf`. A client with an unknown name or with no name is a guest; the name `neptron` with a wrong password gets a logon failure. One account exists for Windows 11 24H2, which refuses guest sessions: name `neptron`, password `neptron` (`/etc/ksmbd/ksmbdpwd.db`, committed). The password is public and gives no protection. The account and a guest both work as the user `nobody` (`force user`, `force group`). The system user `neptron` (`board/common/users.table`) has no password and the shell `/bin/false`; `board/common/post-fakeroot.sh` fails the build otherwise. The storage script mounts the share partition with `uid=65534,gid=65534,dmask=0022,fmask=0133,nosuid,nodev,noexec`, so the guest owns every file there and nothing on it runs as a program. `/state` and the root are not shared.
- **The name.** `mdnsd` answers for `neptron.local` and announces `_smb._tcp` on port 445, from `/etc/neptron/mdns.d`. macOS and the Linux file managers list the PC under "Network". There is no WS-Discovery and no NetBIOS name service, so the Windows "Network" view does not list it; `\\NEPTRON` works when the router resolves the DHCP host name.
- **Windows 11 24H2** refuses a guest share by default (insecure guest logons off, SMB signing required). Log in as `neptron` with the password `neptron`. A plain open in Explorer sends the Windows account name first, which becomes a guest that Windows refuses, so give the account first: `net use \\NEPTRON\share /user:neptron neptron`, or "Map network drive" with "Connect using different credentials", or a Windows credential for `NEPTRON` in Credential Manager. Or, as guest, turn on "Enable insecure guest logons" and turn off "Digitally sign communications (always)" on the Windows PC. See the spec.
- **The switch.** `S45neptron-share` starts `ksmbd.mountd` and `mdnsd` in a background job, unless `/state/network-share-off` exists. Then port 445 and port 5353 stay closed. `/etc/init.d/S45neptron-share restart` applies a change of the file at once; a later settings row will do that.
- **The stop.** In `rcK`, before `umount -a -r`: `mdnsd` stops, `ksmbd.control --shutdown` closes every session, the script waits up to 5 s until no socket of the server is left and both daemons have ended, then `sync`. So the share partition unmounts clean after a copy.
- **What is open on the release image**: TCP 445 and UDP 5353, and UDP 68 for the DHCP answers while `udhcpc` has or asks for a lease. No SSH, no telnet.
- **What the network reaches.** Any host on the LAN can put a crafted ROM, archive or BIOS file on the share; the library scan and the cores parse it as root, with no sandbox, so a parser bug in a core or in the archive code is reachable from the network. `mdnsd` runs as root and parses multicast DNS from any host on UDP 5353. The published account makes the NTLMv2 and signed-session code of the kernel server reachable from any host, and any host can trigger its lockout delay (5 s in a server worker per failed login of `neptron` after 10 wrong passwords). `ksmbd.mountd` runs as root and parses the RPC of the automatic `IPC$` share from any client, before any login. `udhcpc` and its script run as root on the answers of any DHCP server on the LAN. The spec lists these and the limits.

The boot check waits, inside its deadline, until a socket listens on port 445 and `mdnsd` runs. The QEMU runs have no network card (`-nic none`), and the server listens on the loopback interface too. An image built before the network share fails this step only when the whole deadline is over, with a line that asks whether the image is older than the network share: rebuild the image.

### The 15 kHz kernel patch set

The PC kernel applies the nine patches of `D0023R/linux_kernel_15khz`, folder `linux-6.18`, at commit `8c96dd9f0b343a6d1ef3c61041783ee106957886`. All nine apply to 6.18.63 with no offset and no fuzz. Patch 09 (i915 gen9 interlace) acts only with `i915.no_ytiled_scanout=1`, which the image does not set.

**Open licence question.** The `LICENSE` file of the patch repository says GPL-3.0, and the kernel is GPL-2.0 only. The owner has asked the author to clarify. **No image built with these patches may be distributed until that is resolved.** `cargo xtask os build --arch x86_64 --no-kernel-15khz` builds the image without them, in its own output. The defconfig, `patches/linux/linux.hash` and `board/pc/LICENCES.txt` repeat this note.

### Variants

| Command | Output | What differs |
| --- | --- | --- |
| `cargo xtask os build --arch x86_64` | `images/x86_64` | The release image |
| `cargo xtask os build --arch x86_64 --dev-key ~/.ssh/bench.pub` | `images/x86_64-dev` | Dropbear with password logins off (`-s`) and only that public key in `/root/.ssh/authorized_keys`; the address of `eth0` on the console and in `/share/neptron-ip.txt`; the `control` feature of the frontend |
| `cargo xtask os build --arch x86_64 --no-kernel-15khz` | `images/x86_64-no15khz` | No 15 kHz patches. It combines with `--dev-key` |

The key file must hold one public key and lie outside the checkout. No key is in the repository. Each variant has its own Buildroot output volume, so no file of the development variant can reach the release image. The release post-build fails when it finds an SSH or telnet server.

On the development variant, a bench tool that needs the display (the self-test of `neptronctl`) runs only while the frontend and the launcher are stopped. Over SSH: `touch /run/neptron/hold && kill $(pidof neptron-launch)`, then for example `neptronctl selftest video --profile generic_15`. The launcher forwards SIGTERM to the frontend, waits for it and exits, and the descriptor closes. Init starts the launch script again, and the script waits while `/run/neptron/hold` exists. `rm /run/neptron/hold` starts the launcher again. The file is on a RAM file system, so a restart always starts the frontend.

### Working on the bench

The development variant lets a change reach the i3-6100T without a new image.

1. Build it once, with your public key: `cargo xtask os build --arch x86_64 --dev-key ~/.ssh/id_ed25519.pub`. Write `images/x86_64-dev/neptron-x86_64.img` to a stick (see below) and boot the PC with an Ethernet cable on a network with DHCP.
2. Find the address: the serial console prints `neptron: eth0 address A.B.C.D`, and the share partition holds `neptron-ip.txt` (plug the stick into another computer), or ask the router.
3. `cargo xtask os ssh --host A.B.C.D -- uptime` checks the connection. It uses your SSH agent or key and never a password; the image accepts no password. The first connection stores the host key in `~/.cache/neptron/bench_known_hosts`. The PC keeps its host key on the state partition (`/state/dropbear`), so it stays the same across reboots; a stick written again with a new image has a new state partition and a new key, so remove the line of that host from the file first.
4. Change the code, then `cargo xtask os push --host A.B.C.D`. It builds the frontend (with `control`), the launcher and `neptronctl` in the bench profile, puts them in `/state/hot/bin`, and restarts the launcher and the frontend. `--data` also replaces the data files (`/state/hot/share/neptron`). A warm push takes about a minute.
5. `cargo xtask os logs --host A.B.C.D` prints the log (`/state/log/neptron/current`; svlogd keeps it below 4 MiB and keeps two older files), `dmesg` and the hot folder; `--follow` follows the log. The log has the binary and data paths of each start, every mode the engine set with its read back, the guard events, the notices and the errors.
6. To go back to the image's own programs: `cargo xtask os ssh --host A.B.C.D -- touch /state/hot/DISABLED`, then restart (below). Remove the file to use the hot folder again.

The files go over the SSH connection itself (`cat > file` on the PC, and a tar stream for `--data`): Dropbear has no SFTP server, which the `scp` of current OpenSSH needs.

How the hot folder works: the launch script of the development variant takes `bin/neptron-launch`, `bin/neptron` and `share/neptron` from `/state/hot` when they are there and `/state/hot/DISABLED` is not. A hot binary goes through the same guard and validator as the image's own. The launcher holds the DRM descriptor, so a restart of the frontend alone keeps the mode on the wire; a new launcher binary needs a restart of the launcher, which `os push` does with the hold file: `touch /run/neptron/hold; kill $(pidof neptron-launch)`, wait until it is gone, `rm /run/neptron/hold`. The launcher forwards SIGTERM to the frontend, which ends its run as a quit (save RAM and settings written, exit code 0); `os push` sends SIGKILL only as a fallback, with a line that says so, to a frontend still there after 10 s. The bench profile differs from the release image: no LTO, line tables, no strip, and the `control` feature; `panic = "abort"` is the same.

The release image has none of this: no SSH server, no hot folder, no log file, no `control` feature. Its post-build fails when it finds any of them. Both variants have DHCP and the network share.

**The output now holds the development packages.** The Docker disk had no room for a second 45 GB output, so the development image of 2026-11-13 was built in the x86\_64 output (`--output x86_64 --dev-key …`); its images are in `images/x86_64-dev`. That output now holds Dropbear and the files of the development overlay in its target folder, so a release build from it fails its post-build check until the release image is built in a clean output (`--output x86_64-clean`, about 45 GB of Docker disk and 5 hours). The release image that passed the gate on 2026-11-13 is kept in `~/.cache/neptron/buildroot/images/x86_64-release-fb6dce6c` (SHA-256 `fb6dce6c965b9230e56e8abc675527302481a296ea9187fcc85014e3e0fcf050`), but it is not the image to ship: it predates the SIGTERM fix of the frontend, the launcher's stop timeout, the log queue and the strict format rules of the share partition. The next release image comes from a build in an empty output.

**What QEMU proved for the bench** (the development image on a writable copy with 1 GiB free, `cargo xtask qemu --arch x86_64 --disk COPY --ssh-port 2222 --beside-launcher`, then the xtask commands with `--host 127.0.0.1 --port 2222`): SSH with the key works; the server offers only `publickey`, and a password login is denied; `os push` built, sent and restarted, and the log and `/proc/<pid>/exe` showed `/state/hot/bin/neptron-launch` and `/state/hot/bin/neptron`; `--data` gave `data /state/hot/share/neptron`; `DISABLED`, and then the removal of `/state/hot`, gave the image's own binaries again; `os logs` read the log, with the guard events and the modes; `/share/neptron-ip.txt` held the address (10.0.2.15, the QEMU user network); after a reboot the same host key was accepted. A warm push took 19 to 26 s, of which the build was 3 s and the rest the copy of 32 MB into the emulated guest; the first bench build took 45 s with the dependencies already in the cross volume. Not proved: a real network, DHCP from a router, the serial console of the PC, and any of it on the i3-6100T.

**Write the image to a USB stick.** macOS: `diskutil list`, then `diskutil unmountDisk /dev/diskN` and `sudo dd if=$HOME/.cache/neptron/buildroot/images/x86_64/neptron-x86_64.img of=/dev/rdiskN bs=4m`. Linux: `sudo dd if=.../neptron-x86_64.img of=/dev/sdX bs=4M conv=fsync status=progress`. Check the device name twice: `dd` overwrites the whole disk.

## The aarch64 test image

- BusyBox init, with `mdev` for device nodes. There is no udev daemon and no systemd.
- A root shell on the serial console, with no login.
- An ext4 root filesystem, 256 MB, on a virtio disk. QEMU runs it in snapshot mode, so a run never changes the image.
- libdrm with its test programs (`modetest`), `neptron-vkms-probe` and `neptron-capture`.
- Mesa with the softpipe software renderer: EGL, OpenGL ES and GBM, and `kmscube`.
- SDL3 with the KMS/DRM video driver and OpenGL ES, without libudev, and alsa-lib.
- The libretro cores FCEUmm, Snes9x and Genesis Plus GX in `/usr/lib/libretro`, with the library names of `data/cores`.
- The frontend, the launcher and `neptronctl`. Init starts the launcher only for a boot check (`launch-for-check`), so the other checks get the display.
- The network share, as in the PC image: `ksmbd`, `ksmbd-tools`, `mdnsd`, and the init scripts. `/share` is a RAM folder there.

The build also installs the Rust toolchain of Buildroot (`host-rustc`, the prebuilt `rust-bin`, with the standard library for the target) in the output, not in the image. `cargo xtask gles-compare` uses it with the Buildroot C toolchain as the linker to build Rust programs for the guest. This toolchain (Rust 1.97.1 in Buildroot 2026.08.1) is not the one that `rust-toolchain.toml` pins for the host (1.97.0); the two can drift apart when Buildroot is bumped.

## Commands

All commands go through `xtask`.

```text
cargo xtask os build --arch aarch64              build the aarch64 test image
cargo xtask os build --arch x86_64               build the PC image (see "Variants" for the others)
cargo xtask os build --arch aarch64 -- linux-reconfigure
                                                 rebuild the kernel after a change to its fragment
cargo xtask os build -- neptron-vkms-probe-rebuild
                                                 rebuild a package of this tree after a change to its source
cargo xtask qemu -- uname -a                     boot, run one command, exit with its exit code
cargo xtask qemu --script test.sh --share dir    run a script; dir is read-only at /mnt/host
cargo xtask qemu                                 interactive console; Ctrl-A then X stops QEMU
cargo xtask gles-compare                         build the GLES comparison for the guest and run it
cargo xtask core-trace                           run the test ROMs in the real cores (tier T4)
cargo xtask os boot-check --arch x86_64 [--firmware uefi]
                                                 boot the image to the game list and compare the picture (tier T3)
```

`os build` builds the development container from `Dockerfile`, then runs Buildroot in it with 4 jobs, 4 CPUs and 6 GB of memory. It runs `make legal-info` before the build, because the image carries the licence texts and the manifest. A machine-wide lock allows one Buildroot build at a time. Buildroot does not see a change to a kernel fragment, or to the source of a package in `package/`, by itself: give the `-reconfigure` or `-rebuild` target as above.

`qemu` boots the image with 2 GB of memory and 2 CPUs. A machine-wide lock allows one guest at a time. On macOS QEMU runs under `taskpolicy -c utility`. The aarch64 guest uses the hypervisor on an Apple Silicon Mac and KVM on an aarch64 Linux host. The x86\_64 guest uses KVM on an x86-64 Linux host. Everywhere else QEMU emulates the CPU, which is too slow for routine use: on a Mac, use the aarch64 target.

How a command runs:

1. `xtask` writes the command as a shell script and gives it to QEMU as the firmware config entry `opt/neptron/run`. Two more entries carry a new random nonce (`opt/neptron/nonce`) and the deadline in seconds (`opt/neptron/timeout`, from `--timeout`, default 120).
2. The init script `S99neptron-run` (through `/usr/libexec/neptron/host-run run`) prints `NEPTRON-BEGIN <nonce> <uptime>`, runs the script under `timeout -s KILL <deadline>`, prints `NEPTRON-END <nonce> <exit code>` and reboots. `-no-reboot` turns the reboot into a QEMU exit. A command over its deadline ends with exit code 137, and the guest stops even when no host process watches it.
3. `xtask` accepts only markers with its nonce, so output that contains the marker text does not end the run. It prints the lines between the markers, the time to the first marker and the guest uptime, and exits with the exit code of the command. It writes the whole console to `target/qemu/<arch>-console.log`.
4. `xtask` stops QEMU 30 s after the deadline, and at once on SIGTERM, SIGHUP or SIGINT. It removes its script on exit, and the next run removes scripts that a killed run left.

## The cores

One package for each core, pinned to an exact upstream commit, with the SHA-256 of the GitHub archive of that commit and of the licence file in its `.hash` file. `BR2_DOWNLOAD_FORCE_CHECK_HASHES` makes Buildroot refuse a download with another hash.

| Package | Upstream and commit | Library | Licence in the package |
| --- | --- | --- | --- |
| `libretro-fceumm` | `libretro/libretro-fceumm` `e0f3086a8104f8fbbe49a2615b10401504a9533f` | `fceumm_libretro.so` | `GPL-2.0+`, file `Copying` |
| `libretro-snes9x` | `libretro/snes9x` `fae2fea08f74180759ef540ee94259213f503480` | `snes9x_libretro.so` | `LicenseRef-Snes9x (non-commercial)`, file `LICENSE` |
| `libretro-genesis-plus-gx` | `libretro/Genesis-Plus-GX` `b9c2316441978d4015ba30e9a95d9b0e6f8ee981` | `genesis_plus_gx_libretro.so` | `LicenseRef-GenesisPlusGX (non-commercial)`, file `LICENSE.txt` |

These are the commits that `data/cores/README.md` names. Each package runs the upstream libretro makefile with `platform=unix` and the Buildroot cross toolchain, and installs the library in `/usr/lib/libretro`. The makefiles ask git for the version, and the build directory is no git checkout, so each package gives `GIT_VERSION` from its pin. The packages have no architecture-specific flags, so the same files build for both targets.

Snes9x and Genesis Plus GX allow non-commercial use only. The owner decided (OP-054): NEPTRON is open source and not commercial, so the image holds them, with their licence texts and the `make legal-info` output, and it is never sold. `board/pc/LICENCES.txt` says so on the boot partition.

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
- `neptron-vkms-probe` and `neptron-capture` refuse a device whose driver is not `vkms` before any commit or signal (both images hold them, because the PC image is also the QEMU image of x86\_64). In the aarch64 guest, pointed at `/dev/null` while the launcher ran, both exited with 1 and the launcher was not stopped; on `card0` the probe reported `driver: vkms` and went on. No other DRM driver exists in the guest, so the refusal of a real non-vkms driver (i915 on the PC) is shown by the code path only.
- `neptron-vkms-probe` with 320x240 at 6.7 MHz, 640x480, 720x480i and a doublescan mode: every commit is accepted, the readback equals the request, the CRCs and the writeback work.
- `sleep 60 | kmscube -D /dev/dri/card0 -c 300` draws on `vkms` with Mesa softpipe at about 53 frames per second ("Rendered 299 frames in 5.630363 sec"), and the frame CRCs change from frame to frame while it runs. kmscube needs a standard input that stays open: with `/dev/null` it stops at once. `kmscube -A` (atomic) stops with "no eglDupNativeFenceFDANDROID", because softpipe has no native fence.

- EGL on the Mesa surfaceless platform with no device gives OpenGL ES 2 on softpipe. `cargo xtask gles-compare` uses it: an image build that adds `host-rustc` took 4 min, the cross build of the comparison 7 s from cold, and the guest run under 10 s.
- The three cores (2026-11-11): the incremental `os build` that added them took 130 s. The root filesystem went from 28,922 KiB to 44,717 KiB used. The libraries are 4.3 MiB (`fceumm_libretro.so`), 2.7 MiB (`snes9x_libretro.so`) and 12.0 MiB (`genesis_plus_gx_libretro.so`). `neptron-vkms-probe` passes on the new image.

On the same Mac, on 2026-11-12 and 2026-11-13, with the PC image and the aarch64 image that hold the frontend:

- `cargo xtask os boot-check --arch x86_64` passes with BIOS (SeaBIOS) and with UEFI (EDK II): the firmware boots the USB image from a USB stick on xHCI through GRUB, the kernel mounts the EROFS root by PARTUUID, init starts the launcher, and the frontend shows the first-boot picture. The `vkms` writeback picture equals `tests/golden/boot-game-list.ppm` byte for byte. Under CPU emulation (TCG, single thread for an x86 guest on an Arm host) over eight runs the first frame of the frontend came at a guest uptime of 12.2 to 46.0 s, and the notice at 20.3 to 47.4 s, which is 24.4 to 55.7 s after QEMU started. The report calls the second time "first-boot notice on screen". The last runs on the final image (`fb6dce6c…`): BIOS first frame 15.09 s and notice 23.21 s guest uptime (19.06 s and 27.18 s after QEMU start); UEFI 16.07 s and 24.20 s (19.64 s and 27.77 s). These are emulated times; they vary with the load of the machine, and they say nothing about the i3-6100T.
- `cargo xtask os boot-check --arch aarch64` passes with the hypervisor: first frame at a guest uptime of 0.55 s, the notice at 10.91 s (after the 10 s of the boot prompt), and the same picture byte for byte. These are times of a virtual machine, not hardware times.
- The picture is the 320x240 menu in the 1280x240 super-resolution mode that the safe start profile `generic_15` gives on `vkms` (26.027 MHz, 15.66 kHz, 60 Hz): the empty home screen with the notice that the display change went back. On a first boot nothing is confirmed, so the guard opens the boot prompt, reverts to the safe default after 10 s, and shows that notice until the player presses OK. The check waits for that notice because it has no timer.
- The share partition, on a writable copy of the development image of 2026-11-13 (`08ef1064…`) with 1 GiB of free space after the state partition (`cargo xtask qemu --arch x86_64 --disk COPY --script ...`), the steps in order on one copy:
  - A, a fresh stick: partition 4 made, found empty, formatted exFAT, mounted; the marker `/state/share-pending` removed; a file written.
  - B, the second boot: mounted, not formatted; the file is there.
  - C, the image written again with `dd` over the start of the copy (no partition 4 in the table, the old share on the disk): partition 4 made again at the same start, the old exFAT found and mounted, not formatted; the file is there.
  - D, a power cut after `sfdisk` (marker written by hand, first 8 MiB of partition 4 zeroed): the next boot formatted it once and mounted it; the boot after that did not format it, and a file written in between is there.
  - E, partition 4 zeroed and no marker: left untouched ("it holds no filesystem, and this system did not make it"), `/share` a RAM folder.
  - F, a share with files whose main boot sector is zeroed, the backup boot region intact, no marker: left untouched, `/share` a RAM folder.
  - G, the same share with the marker written by hand: left untouched ("it holds an exFAT backup boot sector, so it is a damaged share").
  - Not run: a read error of a real stick (QEMU gives none), and the scan that finds an old share at another start (no image of another layout exists yet).
- After `kill -ABRT` of the frontend on the aarch64 image, while the launcher lives, the CRTC stays active with the 1280x240 mode and the plane keeps the framebuffer of the dead frontend; the launcher starts a new frontend 250 ms later (its crash backoff), and the mode is the same.
- The 15 kHz patch set applies in Buildroot, and the patched kernel builds and boots in both boot checks.
- `kms-check`, `gles-compare` and `core-trace` pass on the aarch64 image that holds the frontend; `os app-check` passes for aarch64 and x86\_64.

On the same Mac, on 2026-11-13, the aarch64 image with the network share (an incremental build of 178 s that added `ksmbd-tools`, GLib, libnl, `mdnsd` and the kernel server):

- `cargo xtask os boot-check --arch aarch64` passes, with its step that waits for port 445 and `mdnsd`; the frontend started at a guest uptime of 0.39 s, as before.
- A guest script (`cargo xtask qemu --arch aarch64 --script`): `ksmbd.mountd` runs and port 445 listens; `/etc/network/interfaces` holds the loopback only; `/etc/passwd` has `neptron:x:1000:1000:NEPTRON network share account:/:/bin/false` and `/etc/shadow` has `neptron:*:::::::`; `/share` (a RAM folder) is owned by uid 65534 with `nosuid,nodev,noexec`; `ksmbd.control --shutdown` stops the server; `ksmbd.mountd -n -v` with the image's files logs `New user 'neptron'`, `New user 'nobody'`, `New share 'share'` and no "Invalid pwddb entry", so the user database parses.
- Not proved: a login over the network, as guest or as `neptron`. The image has no SMB client, and the QEMU runs have no network card for port 445. See `docs/hardware-checklist.md`.

Not verified:

- Anything on the i3-6100T: iris on the HD 530, the DP-to-VGA bridge, the sound card, USB pads, the boot from a real stick, the boot time. See `docs/hardware-checklist.md`.
- The x86\_64 checks under KVM. `kms-check`, `gles-compare` and `core-trace` on the x86\_64 image (they need KVM to be quick; the PC image has the same hooks).
- The development variant and the image without the 15 kHz patches were not built.
- The images come from an output that was built up step by step (the defconfig changed several times). Buildroot does not rebuild a package when its options change; `mesa3d` and `util-linux` needed a `-dirclean` by hand. No build of the release image in an empty output was done yet; it takes about 5 hours (`cargo xtask os build --arch x86_64 --output x86_64-clean`). One such clean build is needed before an image goes to the bench.

Build time, PC image, 4 jobs: the first full build with LLVM, clang and the 15 kHz kernel took 17,419 s (4 h 50 min; host LLVM and clang take most of it). A rebuild of the frontend and the launcher from clean (fat LTO), with the image, takes about 5 min (309 s); the aarch64 build that added the frontend, the launcher, `neptronctl` and `neptron-capture` took 301 s. The kernel `bzImage` is 14.2 MiB. `rootfs.erofs` is 158 MiB; the target directory is 330 MiB, of which LLVM is 74 MiB and the unused clang libraries 104 MiB (see the open points). The USB image file is 739,246,080 bytes (705 MiB); most of it is the free space of the fixed partitions, so it compresses well.

Build time and size, aarch64, 4 jobs: the first full build took 48 min (toolchain, host tools, kernel, BusyBox). Mesa, SDL3 and kmscube added 19 min. A kernel reconfigure, or a change to the overlay or to a package of this tree, takes about 1 min. The kernel `Image` is 13.3 MB. The root filesystem has 28 MB used of 256 MB. The guest uses about 100 MB of memory after boot.

Disk use outside the repository after these builds: the downloads volume 1.2 GB, the output volume 10.9 GB, the compiler cache 0.3 GB, the container image 0.7 GB, and `~/.cache/neptron` 0.15 GB.

## Where the build data lives

Nothing large is in the repository.

| What | Where |
| --- | --- |
| Downloads | Docker volume `neptron-br-dl` |
| Compiler cache | Docker volume `neptron-br-ccache` |
| Buildroot output | Docker volume `neptron-br-out-<variant>`: `aarch64`, `x86_64`, `x86_64-dev`, `x86_64-no15khz`, `x86_64-dev-no15khz` |
| Cargo registry and target directory of the guest builds | Docker volume `neptron-cross-<arch>` |
| Images | `~/.cache/neptron/buildroot/images/<variant>` (`NEPTRON_CACHE_DIR` moves `~/.cache/neptron`) |

The downloads, the compiler cache and the output are Docker volumes because Buildroot needs a case-sensitive filesystem and the macOS default is not. All worktrees share them. `NEPTRON_OS_IMAGES` or `--images` boots an images directory from somewhere else, for example the base image that CI publishes.

## Licences

The files of this tree are `MIT OR Apache-2.0`, as the rest of NEPTRON. Buildroot is GPL-2.0-or-later. It runs as a build tool and is not part of the repository. The packages in the images keep their own licences, which `make legal-info` lists. The network share adds the kernel server `ksmbd` (GPL-2.0), `ksmbd-tools` (GPL-2.0-or-later), GLib and libnl (LGPL-2.1-or-later), PCRE2 (BSD-3-Clause), libffi (MIT) and `mdnsd` (BSD-3-Clause); `board/pc/LICENCES.txt` lists them. The PC image carries the manifest and the licence texts on its boot partition (`legal/`) and in `/usr/share/licenses`, with the notes of `board/pc/LICENCES.txt`. The source tarballs of the GPL and LGPL packages are in `legal-info/sources` of the Buildroot output; they must be published with an image. The frontend and launcher packages do not copy their source there: it is this repository.

The 15 kHz patch set has an open licence question (see "The 15 kHz kernel patch set"): no image built with it may be distributed until the author answers.

No firmware file is in the image. For a later decision: the i915 files of `linux-firmware` are under `LICENSE.i915` (Intel: redistribution in binary form allowed, no modification and no reverse engineering), and the amdgpu files under `LICENSE.amdgpu` (AMD: terms of the same kind). Neither is an open source licence.
