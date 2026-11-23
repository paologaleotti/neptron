# os reference

The details behind `os/README.md`: the build checks, the kernel configuration, the rules of the state and share partitions, the network share, the Pi board, the cores, and the measurements.

## Build checks

`board/common/post-build.sh` runs for every image. It:

- removes the init scripts of the `mdnsd` and `iptables` packages (`S35iptables` too);
- writes `/etc/network/interfaces` with the loopback only (an output built with `BR2_SYSTEM_DHCP` keeps a DHCP stanza, because Buildroot does not install `ifupdown-scripts` again);
- makes an empty `/mnt/host`, because the root of the PC and Pi images is read-only;
- gives the `/run` tmpfs of `/etc/fstab` a size of 8 MiB (`RUN_SIZE_MIB`), and fails when the image has no such line;
- checks the files of the network share, and runs `check-share-filter.sh` on its packet filter;
- runs `check-arcade-index.sh`, which fails when the image has the FinalBurn Neo core and no arcade index (`/usr/share/neptron/arcade/fbneo.idx`). The `neptron` package writes that index in its install step, from the DAT of the `libretro-fbneo` source, so a rebuild of the frontend package alone keeps it;
- runs `check-gamecontrollerdb.sh`, which fails when the configuration enables `sdl-gamecontrollerdb` and the image has no `/usr/share/sdl-gamecontrollerdb/gamecontrollerdb.txt`, or an empty one;
- appends the hotplug rule of the network ports to `/etc/mdev.conf`, fails when the `udhcpc` script runs no hooks or the address hook is not executable, and removes the old `S41neptron-address` of an older output (see "Network share details").

`board/common/post-fakeroot.sh` fails the build when the system user `neptron` has a password or a shell other than `/bin/false`. The release post-build of the PC and the Pi fails when it finds an SSH or telnet server, a file of a development overlay, the hot folder, the log file or the `control` feature. The PC and Pi `check-kernel-config.sh` read the final kernel configuration (see "The PC kernel" and "The Raspberry Pi board").

## The PC kernel

The kernel is the `x86_64` defconfig of the kernel plus the fragments in `board/pc` and `board/qemu/host.config`. The kernel of the aarch64 test image is the Buildroot QEMU configuration (`board/qemu/aarch64-virt/linux.config` in the Buildroot tree) plus the five fragments in `board/qemu`.

**CPU governor.** The `x86_64` defconfig makes `userspace` the default, and `intel_pstate` and `amd-pstate` then start in `powersave`. `system.config` makes `performance` the default, and `check-kernel-config.sh` requires it. With `performance`, `intel_pstate`, `amd-pstate` and `acpi-cpufreq` ask for the top rate (the active modes also set the energy preference to 0). A QEMU guest has no cpufreq driver, so the setting has no effect there.

**The kernel sets no mode of its own** (the safety model, layer 8). There is no virtual terminal (`CONFIG_VT` off, which needs `CONFIG_EXPERT`), no fbdev emulation, no `CONFIG_FB`, no framebuffer console, no `simpledrm`, `efidrm` or `vesadrm`, no firmware framebuffer device, no DRM log client and no DRM panic screen. `post-build.sh` reads the final kernel configuration and fails the build when one of these is on. i915 is built in. amdgpu, radeon and vkms are modules. The kernel log goes to the first serial port only.

The kernel has the USB, SATA, NVMe and virtio disk drivers, EROFS, ext4, exFAT and FAT built in. There is no initramfs. `CONFIG_ACPI` is required by `check-kernel-config.sh`, for the power-off.

**`rootfs.ext4`.** The build also makes `rootfs.ext4` (`BR2_TARGET_ROOTFS_EXT2_SIZE`, 512 MiB) from the same target tree. It is not in the disk image, and its size changes no partition. `cargo xtask qemu --arch x86_64` without `--disk` boots the kernel with it as a virtio disk, as the aarch64 test image does. With FinalBurn Neo the target tree is 443.7 MB (2026-11-16, development variant), and the 512 MiB filesystem keeps 65 MiB free; at 384 MiB, `mkfs.ext4` failed with "Could not allocate block". Check the free space again when a large package comes in.

### Boot

GRUB, `timeout=0`, no menu and no video module, so it sets no display mode. The kernel command line is `root=PARTUUID=<signature>-02 rootfstype=erofs ro rootwait quiet console=ttyS0,115200 panic=10 usbcore.old_scheme_first=1`.

`usbcore.old_scheme_first=1` makes the USB core enumerate a new device with the old scheme first: `SET_ADDRESS`, then a read of 8 bytes of the device descriptor. With the default scheme (a read of 64 bytes at address 0, then a second port reset), some controllers enumerate but give no input until they lose power. The Mad Catz PS3 FightStick TE (0738:8838) is one. Linux chooses the scheme before it reads the device descriptor, so no quirk for one device can select it. The USB drivers are built in and enumerate before user space, so only the command line covers a controller that is plugged in at power-on. `usbcore.use_both_schemes` stays at its default, so a device that refuses the old scheme enumerates with the other one. SuperSpeed devices always use the old scheme. The `xtask` test `every_image_kernel_enumerates_usb_with_the_old_scheme_first` checks that `grub.cfg` and the `cmdline.txt` of the Pi have the parameter, and the x86\_64 boot check reads it back from `/sys/module/usbcore/parameters/old_scheme_first`.

### What a CRT gets before the frontend runs

NEPTRON sets no mode until the frontend makes its first validated commit, but the PC sets one before it:

1. The BIOS or UEFI POST, and the GRUB text console (GRUB loads no video module, so it keeps that mode), show in the mode that the firmware set. That mode is 31 kHz or higher.
2. i915 inherits that mode and keeps it on the wire until the first validated commit of the frontend. In QEMU the first frame came 15 to 25 s after the start of the kernel under CPU emulation; on the i3-6100T this time is not measured.
3. A monitor that takes 15 kHz only gets an out-of-range signal for that whole time. Use an adapter that protects the monitor from such a signal, or switch the monitor on after the boot.
4. The definition has the launcher blank the output to shorten this window. This image does not do that step: the launcher sets no mode and makes no commit (`apps/neptron-launch/README.md`).

## Init, power and sound

**Init.** BusyBox init with `mdev`. There is no getty and no login on any console. The init scripts mount the partitions and `/run` (tmpfs). Then init starts `/usr/libexec/neptron/launch`, which makes `/run/neptron` and runs `neptron-launch --power -- /usr/bin/neptron --backend kms`. Before that, the launch script sources `/usr/libexec/neptron/mesa-cache`: when `/state` is the ext4 state partition mounted read-write, Mesa keeps its shader cache in `/state/cache/mesa` (`MESA_SHADER_CACHE_DIR`, at most 64 MiB), else the cache is off (`MESA_SHADER_CACHE_DISABLE`). The read-only root has no place for it. The cache can be deleted at any time, and a folder that cannot be made only turns the cache off.

**Quit, shut down, reboot.** The launcher holds the DRM descriptor and starts the frontend again when it ends. Init starts the launcher again when it ends. So a "quit" is a restart of the frontend, not a power-off: the frontend ends with exit code 0, the launcher stops, and init starts the launch script again at once. `Settings > System > Shut down` (or `Reboot`) ends the frontend with exit code 81 (80), and the launcher, which has `--power`, runs BusyBox `poweroff` (`reboot`). Init then runs the `::shutdown:` entries of `inittab` (the stop script, `rcK`, `umount -a -r`), so the save RAM and the settings are on disk and the state and share partitions are unmounted clean before the machine turns off. The PC turns off through ACPI; the aarch64 QEMU image through PSCI. A power cut is still safe: the state and share partitions are written with care for it.

A stop of the launcher with SIGTERM (`kill $(pidof neptron-launch)`, or init at shutdown) forwards SIGTERM to the frontend, which ends its run as a quit. A frontend that has not ended 5 s later, for example one stuck inside a core, gets SIGKILL from the launcher. Only that timeout, the SIGKILL fallback of `os push`, or init ends a frontend that hangs. The frontend reads its folders from the image defaults: `/share`, `/state`, `/usr/share/neptron`, `/usr/lib/libretro`, `/run/neptron`.

**Sound.** There is no sound server: SDL3 talks to ALSA. The kernel leaves an HDA codec at its reset state, with the DAC at the bottom of its range and the output pins muted (seen on the i3-6100T with its ALC221). `S08neptron-mixer` runs in the background at boot. It waits up to 10 s for a card, then sets the playback controls `Master`, `PCM`, `Front`, `Surround`, `Center`, `LFE`, `Side`, `Headphone`, `Speaker`, `Line Out` and every `IEC958` switch of each card to 0 dB and unmutes them. 0 dB, not 100%: a control that goes above 0 dB adds gain and can clip. The input controls stay as they are. The volume of the frontend then scales the samples in software (`docs/spec/audio-settings.md`). A card plugged in after the boot keeps its own state.

**Packages of the PC image.** `ksmbd-tools`, `mdnsd` and `iptables` (legacy tables) for the network share. Mesa 26.1 with LLVM: the iris driver for Intel gen8 and later (the HD 530 of the i3-6100T), llvmpipe, and softpipe; EGL, OpenGL ES and GBM. SDL3 with KMS/DRM and OpenGL ES, without libudev, and alsa-lib. `amixer` of alsa-utils, and nothing else of it. libdrm. The four cores. The frontend, the launcher, `neptronctl` (with the `kms` feature, for the bench self-test), `neptron-vkms-probe` and `neptron-capture`. e2fsprogs, exfatprogs, and the basic and partition tools of util-linux. There is no firmware file: i915 drives the display without its DMC firmware, which is not loaded, and amdgpu does not work without its firmware (see "Licences" in `os/README.md`).

**How the frontend packages build.** `package/neptron`, `package/neptron-launch` and `package/neptronctl` build this checkout. `xtask` mounts the checkout read-only at `/src/repo`. Each package copies it without `target/`, `.worktrees/` and `.git`, runs `cargo vendor --locked`, and builds offline in the workspace release profile (fat LTO, one codegen unit, `panic = "abort"`). The sdl3 crate adds `/usr/local/lib` to the link search on every Linux host, and the Buildroot compiler wrapper refuses that path. `package/neptron/link-filter.sh` drops only that path and links with the wrapper. With `panic = "abort"` no destructor runs. The launcher keeps its descriptor open, so the framebuffers and the mode stay on the wire until the next frontend commits (`docs/spec/kms.md`).

## The state and share partitions

`S06neptron-storage` does this work at each boot, on the disk that holds the root filesystem only. The PC and the Pi use the same script.

### The first boot makes the share

The image holds no share partition. The script makes partition 4 only when the disk has no partition 4, the state partition is mounted, and at least 64 MiB are free after the state partition. It first looks for an old exFAT share in the free space: an exFAT boot sector at any 1 MiB boundary of the first 4 GiB. An old share at another start than the planned one leaves the disk untouched.

Else the script writes the marker `/state/share-pending` (with a sync) and appends partition 4 (type `0x07`) at the planned start, to the end of the disk, with `printf '<start>,,7' | sfdisk --force --no-reread --no-tell-kernel --wipe never --wipe-partitions never --lock=nonblock --append`. An MBR entry ends at sector 2^32 - 2 at most, so on a disk of 2 TiB or more the share ends near 2 TiB and the rest stays unused. The start is given because a start that sfdisk chooses follows the optimal I/O size of the device, which can be 2, 4 or 16 MiB. The disk is in use, the other partitions stay as they are, sfdisk asks nothing, and it wipes no signature, because the free space can hold the share of an older image.

The exit status of sfdisk decides nothing. The script runs `blockdev --flushbufs` on the disk and logs its exit status. (BLKFLSBUF in Linux 6.18 writes the dirty pages of the whole-disk device, drops its clean pages and returns 0. The mounted partitions have caches of their own, which it does not touch.) Then it reads the table back with `sfdisk --dump`, up to 5 times with 0.5 s between tries (`TABLE_READ_TRIES`, `TABLE_READ_RETRY_US`). When the flush exits with 0, this proves that the device returns the new table when it is asked again. The exit status of the flush is no proof by itself: the log line says "the table is read back after a flush of the cache", or "the cache was not flushed, so the table may be read back from the cache". It does not prove that a stick with a volatile write cache keeps the table through a power cut.

The script goes on only when partition 4 is there with type 7, the planned start and the planned size. A table that was read and has no such partition 4 removes the marker. A table that cannot be read keeps it, and the next boot, where the kernel reads the table, uses partition 4. Then `addpart <disk> 4 <start> <size>` gives the kernel the partition with the numbers of the table read back (BLKPG; it reads no table), up to 5 times with 0.5 s between tries (`ADD_TRIES`, `ADD_RETRY_US`). The kernel's list decides, not the exit status of addpart. Then the script waits up to 5 s for the device node (`NODE_WAIT_STEPS`, `NODE_WAIT_STEP_US`). A partition 4 that is in the table but not in the kernel (from a step that failed earlier in the same boot) goes to `addpart` directly, with no new `sfdisk`.

A partition 4 with a filesystem is mounted as it is and never formatted. An empty one is formatted exFAT only when all of these hold. A freshly written image owns the free space: the remains of a foreign file system there (an old ext4 or NTFS) are no user data, and these rules do not look for them. The one thing they keep is an old exFAT share:

- the marker `/state/share-pending` is there and names the start and the size of this partition 4 (`start=<sector>` and `size=<sectors>`, written before sfdisk runs): this system made this partition. A marker of an older image names none, so it allows nothing;
- its type is `0x07` and it starts at the first 1 MiB boundary after the state partition;
- its first 16 MiB read with no error, and `blkid -p` exits with no filesystem found and prints nothing on standard error;
- it holds no exFAT backup boot sector at sectors 12, 24, 48 or 96 (a share with a damaged main boot sector is the user's data);
- no exFAT boot sector starts at a 1 MiB boundary from the planned start through the first 4 GiB of partition 4 (an old share). The scan before the partition is made covers this window at the first boot, so that boot scans once. Here a sector that does not read counts as no boot sector: the share must come up, and an old share that does not read cannot be mounted either.

These are the rules of the first boot. The rules of a format that the user confirms read more, and refuse on a read error: see "A format that the user confirms".

After the share is mounted, the script makes `roms/<system>` for each system of `data/systems`, and `bios`.

### The life of the marker

The script writes the marker before sfdisk, with a sync. It removes it:

- when partition 4 holds a filesystem signature (also when that filesystem then does not mount);
- when the table read back has no partition 4, or one off the plan;
- after the first mount;
- when a type change of a request wrote the table and the request is then refused.

It keeps it when the table cannot be read back, when the kernel does not take partition 4, when the device node does not appear, when a format rule refuses, and when `mkfs.exfat` fails. So the marker can outlive a failed boot, and the next boot that finds an empty partition 4 with the planned start and size formats it.

The first-boot format adds `format=started` to the marker before `mkfs.exfat` and `format=done` after it exits 0, each with a sync. These lines are information for the notice only: no rule of a format reads them. When the marker goes because partition 4 holds a filesystem and the marker says `format=started` with no `format=done`, the script keeps a copy as `/state/share-format-interrupted`; the copy goes when the share mounts.

### Logs and `storage.toml`

When any check fails, nothing is formatted or mounted, `/share` is a RAM folder, and one log line says that the share partition was left untouched and why. Every way out of the script that leaves no share partition sets such a reason, and the reason goes to `storage.toml` and to the notice of the frontend. Every line of the script goes to the console and to syslog (`logger -t neptron-storage`, `/var/log/messages`, a RAM folder). When `/state` is mounted, the script also appends the lines of the boot to `/state/log/storage.log` itself, with no syslog, and keeps the last 300 lines (`STORAGE_LOG_LINES`). So the release image, which has no SSH, keeps them for a later look on another computer. `cargo xtask os logs` prints both. Every variable that the script reads is set by the script first: init passes the kernel command line to its children as variables.

At the end, the script writes `/run/neptron/storage.toml`: `state` and `share`, each `partition` or `ram`, and for a RAM folder `state_reason` or `share_reason`, the log line that names the reason. `state_repair` names a repair of `e2fsck -fy`, and `share_read_only` with `share_read_only_reason` a share mounted read-only after its check. A format offer and the result of a format request add the fields of "A format that the user confirms". The frontend reads the file at boot, logs it, and shows a notice for each RAM folder until the player presses OK.

### The checks before a mount

**The state partition** gets `e2fsck -p` first. When the safe repair of `-p` leaves errors and nothing else (exit 4 to 7), the script runs `e2fsck -fy` once. An exit of 8 or more is an error of the check itself, for example a read error of a failing card: then `-fy` never runs, because it could write wrong blocks, and `/state` is a RAM folder with a reason that says so. `e2fsck -fy` answers yes to every repair question, because the settings and saves are worth more than a refusal at every boot, and the release image has no other repair path. `-fy` can clear a damaged file or move it to `lost+found`, so `storage.toml` then holds `state_repair` and the frontend shows a notice that asks the player to check the saves and settings. The last 20 lines of the `e2fsck -fy` output go to the storage log. When `-fy` also leaves errors, `/state` is a RAM folder and nothing is written to the damaged filesystem. The state partition is never formatted. It is mounted with `errors=remount-ro`, so an error that the kernel finds at run time stops the writes before more damage, and the next boot checks it. With `/state` in RAM the guard record still counts the failed boots of this power cycle, so the guard boots as usual.

**The share partition** (exFAT, no journal) is checked in two steps. `fsck.exfat -n` reads it and writes nothing, under `timeout 120` (`SHARE_CHECK_SECONDS`), so a slow or failing stick never stops the boot, and a check that is stopped has changed nothing. Exit 0 is clean. Only exit 4 (errors found) runs the repair `fsck.exfat -p`, with no time limit, because a repair stopped in the middle can leave the FAT half written; the check before it read the same volume within the limit. A repair that exits 0, 1 or 2 mounts the share read-write. A repair that leaves errors mounts the share read-only, with `share_read_only = true` and `share_read_only_reason` in `storage.toml`, and the frontend shows a notice. A read-only share is never written, never formatted and gets no new folder: its files are the user's. The games run, and the user can copy the files off over the network and repair the partition on a computer. A check that does not end in time, or that fails as a program (any other exit), proves no error of the share: the share is mounted read-write, as with no check, and only the log says so. An image with no `fsck.exfat` mounts the share unchecked, and the log says so. The exit codes of `fsck.exfat -n` follow the fsck convention of exfatprogs; this is not verified on a real image yet (`docs/hardware-checklist.md`).

### Limits of the share rules

A power cut during `mkfs.exfat` can leave a filesystem that `blkid` sees but that does not mount or check clean. The script never formats it again by itself: the marker goes, because `blkid` sees a filesystem, and `/share` is a RAM folder at every boot. `storage.toml` then holds `share_first_format = "interrupted"`, and the frontend adds the notice "The first format of the share partition did not finish, for example because of a power cut. It holds no files. Format partition 4 as exFAT on a computer to use it." No data is lost, because the partition was new. To recover, format partition 4 as exFAT on another computer. Removing partition 4 does not help: the next boot finds the partial boot sector at the planned start and keeps it as an old share. `Settings > System` offers no format here, because `blkid` sees a filesystem. A user-confirmed format of this state is not built: it would need the format rules to skip the signature, backup boot sector and old share checks.

The scan for an old share looks at the first 4 GiB of free space only, so an old share that starts beyond that is not found.

When the kernel does not take partition 4 after the 5 tries of `addpart`, the share is a RAM folder for this boot, and the notice says "partition 4 is in the partition table, but the kernel does not see it after 5 tries of addpart (last exit N); restart to use it". The kernel reads the table at the next boot, so a restart mounts it. No such case is known; the line is there so that it is never silent.

### States where `/share` stays a RAM folder at every boot

Each has its reason in `storage.toml` and in the notice; the lines start with `neptron-storage: `. States 10, 11, 12 and 17 also come with the offer of a format that the user confirms, when its checks hold, and then stay only until the user accepts it.

1. The root filesystem is not on a partition (the aarch64 test image): "the root filesystem is not on a partition of a disk, so there is no share partition".
2. No partition 3: "the boot disk has no partition 3, so no share partition is made".
3. Too little free space: "<n> free sectors after the state partition, less than 131072, so the disk has no room for a share partition".
4. An old exFAT share at another start: "an old exFAT share starts at sector <n>, not at <m>; no share partition made, the disk is left untouched".
5. The state partition does not mount: "the state partition is not mounted, so no share partition is made".
6. The marker cannot be written: "/state/share-pending cannot be written, so no share partition is made".
7. The table cannot be read before sfdisk: "the partition table of <disk> cannot be read, so no share partition is made".
8. sfdisk writes nothing, at every boot: "sfdisk exit <code>, and the partition table has no partition 4, so no share partition is made".
9. sfdisk writes a partition 4 off the plan; later boots then end in 10 or 11: "sfdisk exit <code>, and the table holds partition 4 at sector <n> with <m> sectors and type <t>, not type 7 at sector <p> with <q> sectors; the share partition is left untouched".
10. A partition 4 with no file system that blkid finds and no marker: "the share partition <dev> holds no file system that blkid finds and has no /state/share-pending, so it is left untouched".
11. An empty partition 4 of another type or start: "the share partition <dev> was left untouched: its type or its start is not those of the share partition of this image".
12. A marker that names another start or size: "the share partition <dev> was left untouched: /state/share-pending does not name start <n> and size <m> ...".
13. The first 16 MiB do not read: "... was left untouched: its first 16 MiB do not read cleanly".
14. A damaged share: "... was left untouched: it holds an exFAT backup boot sector (sector <n>), so it is a damaged share".
15. The blkid probe is not clean: "... was left untouched: blkid could not probe it cleanly".
16. A filesystem that does not mount: "<dev> does not mount".
17. `mkfs.exfat` fails at every boot: "mkfs.exfat failed on <dev>".
18. The device node never appears: "the kernel sees partition 4, but its device node <dev> does not appear within 5000 ms".

For one boot only, and the next boot recovers: the table cannot be read back after sfdisk (the marker stays), and the kernel does not take partition 4 after 5 tries of addpart ("restart to use it").

### A format that the user confirms

Some of the states above leave `/share` a RAM folder at every boot only because the script never formats a partition 4 that it did not make. For those, the script offers a format, and the user decides in `Settings > System > Format share` (`docs/spec/ui-navigation.md`). The script stays the only code that formats.

- Offered: an empty partition 4 with no marker (state 10), at any start and of any type (state 11), or with a marker of other numbers (state 12); and a retry after a failed `mkfs.exfat` (state 17).
- The offer needs every one of these on the disk as it is:
  - partition 4 of the boot disk, whose start and size are the same in the kernel and in the table on the disk;
  - an MBR table (`label: dos`);
  - a type that is no extended type (`5`, `f`, `85`);
  - a start at or after the end of the state partition, which is mounted, so the request has a place, and an end at or before the end of the disk;
  - no overlap with any other entry of the table, logical partitions too;
  - `blkid -p` finds no filesystem or partition table, and its probe is clean;
  - the first 16 MiB read cleanly;
  - no exFAT backup boot sector at sectors 12, 24, 48, 96;
  - no copy of another file system header that `blkid -p` does not read and that belongs to a file system at the start of partition 4: an ext2, ext3 or ext4 superblock (magic `0xEF53`) at byte 1024 with block group number 0, or at the backup of block group 1 for blocks of 1, 2 and 4 KiB (bytes 8389632, 33554432, 134217728) with block group number 1 (`s_block_group_nr` at 0x5A); a FAT32 backup boot sector at sector 6 whose hidden sectors (0x1C) are the start of partition 4 or 0 (some tools write 0, so 0 counts); an NTFS backup boot sector in the last 512 or 4096 bytes whose volume (total sectors at 0x28, sector size at 0x0B) fits in partition 4; a btrfs superblock mirror at 64 MiB whose `bytenr` (0x30) is 64 MiB. A copy whose fields place it in a file system at another start is the remain of an older layout: the offer appears, and the user decides. A file system made with other group sizes is not found;
  - no exFAT boot sector at a 1 MiB boundary from the planned start of the share (the end of the state partition, rounded up to 1 MiB) through the first 4 GiB of partition 4, for the offer and for the request alike (an old share that starts before partition 4 can run into it). A partition 4 that starts more than 4 GiB after the planned start is refused, because that space cannot be checked in a boot.
- On the offer and the request, a read that fails in these checks or in the scan for an old share refuses, with "cannot be read": a failed read is no proof that nothing is there. (The first boot goes on over a sector that does not read in its scan: see "The first boot makes the share".)
- `storage.toml` then holds `share_fix = "format"`, `share_fix_device`, `share_fix_start`, `share_fix_size` (512-byte sectors) and `share_fix_type`, and the log says `a format of <dev> (start <n>, size <m>, type <t>) is offered in Settings > System`. When a check fails, the log says `the share partition <dev> cannot be offered a format: <why>`.
- Not detected by these checks, so a format can be offered over them: FAT12 and FAT16 with a wiped head (they keep no backup boot sector), FAT32 with sectors of 4096 bytes (its backup boot sector is not at byte 3072), XFS (no backup superblock at a fixed place), ext2, ext3 and ext4 made with other group sizes than the default, plain dm-crypt and VeraCrypt volumes (no header that a check can see), and an old exFAT share whose boot sector is not at a 1 MiB boundary. For this reason, the question of the dialog says only that NEPTRON finds no file system it can read.
- Never offered: an old exFAT share at another start (state 4: there is no partition 4), a damaged share (14), a probe that is not clean (15), a first 16 MiB that does not read (13), a filesystem that does not mount (16), the device node that never appears (18), and every state that a format cannot fix: too little free space (3), the state partition not mounted (5), a table that cannot be read (7), no partition 3 (2), the root not on a partition (1), and states 6, 8 and 9, which have no partition 4 at that boot. For state 16 an erase with the filesystem type and label shown is a later task.
- The request. On the confirm the frontend writes `/state/share-format-request` (`device=`, `start=`, `size=`, `type=`, one line each, the numbers of the offer; temp file, fsync, rename) and reboots through the launcher.
- Its removal. At the next boot, as soon as `/state` is mounted and before anything else for the share, the script reads the request, removes it and syncs. So a request that fails, or a boot that stops on the way, never formats later. A request that cannot be removed is not used. No test proves that a stick keeps the removal through a power cut: a stick with a volatile write cache can bring the request back at the next boot. Such a replay only repeats a format that the user confirmed, and only after every rule is checked again on the disk as it is then. A partition 4 that holds a file system by then is refused.
- Its checks. When partition 4 reaches the format rules, the script checks that the request names it: the same device, and the same start, size and type in the table on the disk now. Then it checks every rule of the offer again.
- The type change. Only for a partition 4 of another type than 7, the script runs `sfdisk --force --no-reread --no-tell-kernel --lock=nonblock --part-type <disk> 4 7`. Its exit status decides nothing. After `blockdev --flushbufs`, the table read back must hold partition 4 with the same start and size and type 7. It must equal the table before in everything else: the label, its id and entries 1 to 3. The boot code of the MBR (bytes 0 to 439, `cksum` before and after) must be the same too. Else nothing is formatted, the log holds both tables and says to check the table on another computer, the normal rules do not run on partition 4 at that boot, and the marker is removed, so no later boot formats a partition 4 that the change left as type 7. sfdisk writes the whole MBR sector, with the boot code and the four entries, in one write of 512 bytes, as for the append of partition 4 at the first boot. A power cut or a stick that fails during that write can leave the sector damaged, and then the stick does not boot. The type change is the only table write of a request.
- The format. Then the script runs `mkfs.exfat -L NEPTRON`. The log says `<dev> is formatted exFAT on the request of the user`, and `storage.toml` holds `share_format = "done"`. Any mismatch formats nothing: the log says `the format request is refused: <why>`, the normal rules go on (a filesystem is mounted as it is), and `storage.toml` holds `share_format = "refused"` with `share_format_reason`. A failed `mkfs.exfat` gives `share_format = "failed"` and the offer again. The frontend shows the result as a notice.
- No read of these checks starts or ends past the end of partition 4 or of the disk: Linux refuses a seek past the end of a block device, and `dd` then reads every byte up to the offset, which would hold the boot for hours. A check whose offset lies past the end of a small partition does not run; such a partition cannot hold that header there.
- The cost of the scan for an old share. One step reads 8 bytes at one 1 MiB boundary with one `dd` that seeks to them: one fork and one exec. The offer and the request run the same scan, from the planned start of the share through the first 4 GiB of partition 4 (`OLD_SHARE_SCAN_SECTORS`), so an offer never promises what the request refuses. With partition 4 at the planned start that is up to 4096 steps. A gap before partition 4 adds one step per MiB, and a gap of more than 4 GiB (`SHARE_GAP_MAX_SECTORS`) is refused with "it cannot be checked", so the worst case is 8192 steps. The boot of the request always pays this. A boot with an offer pays it once: the result of its scan is a cache in `/state/cache/share-scan`, keyed by the size of the disk, its `label-id` in the table, the entry of partition 4 and the start of the scan, so a later boot with the same key reads the result and scans nothing. The cache can be deleted at any time; a file that does not parse, or a result with a read error, is scanned again. A request does not trust the cache. The first boot that makes partition 4 scans 4096 steps of free space. The time of one step on the i3-6100T is not measured: at 1 to 3 ms per step, 4096 steps take 4 to 12 s and 8192 steps 8 to 25 s of black screen before the frontend starts.

### Writing a new image to a stick that has a share

A new image writes a new partition table and a new state partition, with no marker. With the same layout, the first boot makes partition 4 again at the same start, finds the old exFAT share there, and mounts it: the games stay. An image of a different layout (other sizes of the root or the state partition) places the new partition 4 at another start, perhaps inside the old share. The script then finds the old share's boot sector at its own start, makes no partition, and logs `an old exFAT share starts at sector N`; the frontend shows an empty game list, because `/share` is a RAM folder. The games are still on the stick: copy them from the stick on another computer, with a tool that reads exFAT at that offset (or add a partition entry that starts at sector N by hand), then delete the old share and boot again. If the old share's boot sector was overwritten too, the script cannot see it and makes and formats the new partition. So copy the games off the stick before you write an image of another layout.

## Network share details

`docs/spec/network-share.md` is the spec. `os/README.md`, "The network share", has the use.

- **DHCP.** `S40neptron-network` starts `udhcpc` on each wired port (`eth*`, `en*`) as a background job, with the host name `neptron` in the request, and returns at once. No step of the boot waits for the network: a PC with no cable boots with no wait, and a cable plugged in later gets its address, because `udhcpc` keeps asking.
  - A port that appears after the script ran (a USB Ethernet chip, such as the one of a Raspberry Pi, or a USB adapter), or that is unplugged and plugged in again, gets its client from `mdev`. `board/common/post-build.sh` appends the rule `SUBSYSTEM=net;(eth|en).*` with `*/usr/libexec/neptron/net-hotplug` to `/etc/mdev.conf`; it replaces the line of an older build, because mdev takes the first rule that matches. On an add the hotplug script runs `S40neptron-network start`; on a remove, `S40neptron-network port-gone <port>`, which stops the client of that port and removes its pid file, so the port gets a new client when it comes back.
  - A pid file counts only when its process is a `udhcpc` for that port (`/proc/<pid>/cmdline`); any other pid file is stale and the port gets a new client. Each start writes the pid file of its client at once, so no port gets two clients.
  - Before `S40neptron-network` has run, the hotplug script does nothing, so it cannot start DHCP before the state and share partitions are mounted. The `mdev` daemon runs it, so it never holds up the boot, and its lines go to syslog (`logger -t neptron-network`). mdev runs commands with umask 0, so both scripts set umask 022 first. Two runs take turns through a lock folder (`/run/neptron/dhcp.lock`, at most 50 tries 0.1 s apart); a run that ends while it holds the lock frees it.
  - The SMB server and `mdnsd` need no restart for a late port: the kernel server has no interface list (`bind interfaces only = no`), so it opens a listener on each port that comes up, and `mdnsd` follows the link and address events of the kernel and checks the interfaces every 10 s.
  - The release image has no firmware file; the RTL8168h of the bench PC works without its firmware patch. There is no Wi-Fi.
  - No IPv6. Each `network.config` (PC, Pi, QEMU) sets `# CONFIG_IPV6 is not set`, so IPv6 autoconfiguration cannot give a port a global address that the internet reaches. The PC and Pi `check-kernel-config.sh` fail the build when `CONFIG_IPV6` is built in, or is a module and the kernel command line does not set `ipv6.disable=1`. ksmbd then listens on IPv4 only, and `mdnsd` logs one `Failed creating UDP6 socket` line and goes on with IPv4.
- **The address file.** `/usr/libexec/neptron/net-address` writes the IPv4 addresses of the wired ports to `/run/neptron/network.toml`, in every image, at each event: the hook of `udhcpc` (`/usr/share/udhcpc/default.script.d/neptron-address`) runs it at `bound`, `renew`, `deconfig`, `leasefail` and `nak`, and `S40neptron-network` runs it at boot and when a port goes. The file holds `version = 1`, the host name and one `[[address]]` table (`port`, `ipv4`) for each port with a lease; the Network panel of the frontend reads it. It is written only when its content changes, through a temp file and a rename. `udhcpc` never waits for it: the lock is a `flock -n` on `/run/neptron/address.lock`, which the kernel frees when its holder ends; a run that finds it taken leaves a flag and returns, and the holder writes again before it ends. `S40neptron-network` waits at most 1 s for a stopped client to end, so its late `deconfig` cannot remove the address of a new client of the same port. A failed write gives a syslog line, and the script still exits 0. On the development image a changed file also prints `neptron: address eth0 A.B.C.D` on the console. Nothing about the address goes to `/share`; an old `neptron-ip.txt` of an older development image stays there as it is. `docs/spec/network-share.md` (section 3) has the rules. `cargo xtask os share-check --case address` checks the file on a lease, on a removed port and on a late port (`xtask/README.md`).
- **The server.** The kernel server `ksmbd` (`CONFIG_SMB_SERVER=y`) serves SMB 2.1 to 3.1.1; `ksmbd.mountd` reads `/etc/ksmbd/ksmbd.conf`. A client with an unknown name or with no name is a guest; the name `neptron` with a wrong password gets a logon failure. The account `neptron` is in `/etc/ksmbd/ksmbdpwd.db` (committed). The account and a guest both work as the user `nobody` (`force user`, `force group`). The system user `neptron` (`board/common/users.table`) has no password and the shell `/bin/false`. The storage script mounts the share partition with `uid=65534,gid=65534,dmask=0022,fmask=0133,nosuid,nodev,noexec`, so the guest owns every file there and nothing on it runs as a program. Its RAM folder has the same owner and options and a size of 64 MiB (`SHARE_RAM_SIZE_MIB`): a client that fills it gets "disk full", and nothing is killed. `/state` and the root are not shared.
- **The name.** `mdnsd` reads `/etc/neptron/mdns.d`. There is no WS-Discovery and no NetBIOS name service.
- **The stop.** In `rcK`, before `umount -a -r`: `mdnsd` stops, `ksmbd.control --shutdown` closes every session, the script waits up to 5 s until no socket of the server is left and both daemons have ended, then `sync`. So the share partition unmounts clean after a copy.
- **The packet filter.** `/etc/neptron/share-filter.rules`, loaded with `iptables-legacy-restore` of the `iptables` package, lets only the ranges of `os/README.md`, "The network share", reach TCP 445 and UDP 5353; other sources get no answer. `S45neptron-share` loads it before it starts `ksmbd.mountd` and `mdnsd`, and starts neither when the load fails. ksmbd-tools 3.5.2 has no address range option: `hosts allow` compares exact strings. The kernels build in the legacy IPv4 filter table (`board/*/network.config`), and both kernel checks require it. `check-share-filter.sh` fails the build when the rule lines differ, the tool is missing, or the share script does not load the rules.
- **What the network reaches.** Any host on the LAN can put a crafted ROM, archive or BIOS file on the share; the library scan and the cores parse it as root, with no sandbox, so a parser bug in a core or in the archive code is reachable from the network. `mdnsd` runs as root and parses multicast DNS from any host on UDP 5353. The published account makes the NTLMv2 and signed-session code of the kernel server reachable from any host, and any host can trigger its lockout delay (5 s in a server worker per failed login of `neptron` after 10 wrong passwords). `ksmbd.mountd` runs as root and parses the RPC of the automatic `IPC$` share from any client, before any login. `udhcpc` and its script run as root on the answers of any DHCP server on the LAN. The spec lists these and the limits.

**The boot check of the share.** The boot check waits, inside its deadline, until a socket listens on port 445 and `mdnsd` runs. It then fails when `/proc/net/if_inet6` exists (the kernel has IPv6), when the chain `neptron-lan` of `iptables-legacy -S` does not end with `-j DROP`, or when `/run` (and `/share` when it is a RAM folder) is not a tmpfs with a `size=` in `/proc/mounts`. The QEMU runs have no network card (`-nic none`), and the server listens on the loopback interface too. An image built before the network share fails this step only when the whole deadline is over, with a line that asks whether the image is older than the network share: rebuild the image.

## The Raspberry Pi board

`docs/spec/raspberry-pi.md` is the spec.

**Kernel.** The Raspberry Pi fork of Linux, branch `rpi-6.18.y`, commit `ea17ae02ca28483e508b05c7c44190c01a1ef8eb` (2026-11-13, Linux 6.18.62), as a GitHub archive whose SHA-256 is in `board/rpi/patches/linux/linux.hash` (and `linux-headers`). The configuration is `bcm2711_defconfig` with 4K pages and the fragments of `board/rpi`:

- `system`: the watchdog, and the `performance` CPU governor;
- `video`: vc4, v3d and the I2C controller of the HDMI DDC bus built in; no fbdev, no `CONFIG_FB`, no framebuffer console, no `simpledrm`, no DRM log client, no panic screen, no logo; no camera or tuner drivers;
- `console`: no `CONFIG_VT`; the serial port;
- `input`: the drivers of the PC image;
- `sound`: ALSA and ASoC built in, which vc4 needs; `snd_bcm2835` for the jack; USB audio;
- `storage`: SD host, EROFS with LZ4, ext4, exFAT, FAT, all built in; no initramfs;
- `network`: the USB Ethernet chips of the Pi 3 B and 3 B+; the SMB server as on the PC; no Wi-Fi, no Bluetooth, no CAN.

Other drivers of `bcm2711_defconfig` are modules, which `mdev` loads by their device ids. `board/rpi/check-kernel-config.sh` fails the build when a mode option is on, when the SMB server is not built in, or when vc4, the I2C controller of the HDMI DDC bus, the watchdog, or a driver of the root on the SD card (`MMC_BLOCK`, the sdhost driver `MMC_BCM2835`, `EROFS_FS`, `EROFS_FS_ZIP`) is not built in, or when the packet filter of the share is not built in or IPv6 is on. The device trees (`bcm2710-rpi-3-b`, `bcm2710-rpi-3-b-plus`, `bcm2710-rpi-zero-2-w`) and the overlays (`vc4-kms-v3d`, and `disable-bt` for the development variant) come from this kernel, not from the firmware tarball.

**No kernel patch.** The 15 kHz patch set of the PC image does not reach the Pi kernel. The Pi defconfig sets no `BR2_LINUX_KERNEL_PATCH`, and its global patch directory is `board/rpi/patches`, which holds only the hash files. `board/rpi/check-kernel-patches.sh` fails the build when `BR2_LINUX_KERNEL_PATCH` is not empty, when `BR2_GLOBAL_PATCH_DIR` is not exactly `$(BR2_EXTERNAL_NEPTRON_PATH)/board/rpi/patches` (the PC uses `os/patches`, and a second directory would add its patches), or when that directory holds any file other than `linux/linux.hash` and `linux-headers/linux-headers.hash`. The set changes the DRM core and the AMD and Intel drivers of the PC, and the Pi needs nothing from it.

**Firmware and boot.** The Buildroot package `rpi-firmware` at commit `063bcab6` gives `bootcode.bin`, `start.elf` and `fixup.dat` (the Pi 0 to 3 files). `config.txt` is the file of the spec, with the reason of each key in it: 64-bit mode, the kernel `Image`, no splash, no `video=` from the firmware (`disable_fw_kms_setup=1`), `dtparam=audio=on` (before the overlay line, or it sets a parameter of the overlay), `dtoverlay=vc4-kms-v3d,composite`, `core_freq_fixed=1`, and `enable_tvout=1` for the Zero 2 W. The development variant appends `enable_uart=1` and `dtoverlay=disable-bt`: the PL011 UART goes to GPIO 14 and 15, Bluetooth is off, and the kernel log goes there (115200 baud, 3.3 V). `cmdline.txt` of the release image is `root=PARTUUID=<signature>-02 rootfstype=erofs ro rootwait quiet console=serial0,115200 panic=10 usbcore.old_scheme_first=1`. The file permits no comment: "Boot" above gives the reason of `usbcore.old_scheme_first=1`. The release image has no serial console: the firmware leaves the UART off. The development variant has no `quiet` (`board/rpi/cmdline.sh` writes the file), so its serial console shows the whole kernel log from the start.

**Disk layout.** MBR partition table. The disk signatures are `4e505052` (release) and `4e505044` (development).

| Partition | Filesystem | Start | Size | Contents |
| --- | --- | --- | --- | --- |
| 1 boot | FAT32, label `NEPTRONBOOT`, type `0x0c` | 4 MiB | 128 MiB | `bootcode.bin`, `start.elf`, `fixup.dat`, `config.txt`, `cmdline.txt`, `Image`, the three device trees, `overlays/`, `LICENCES.txt`, `legal/` |
| 2 root | EROFS (LZ4HC), read-only | 132 MiB | 384 MiB | As on the PC |
| 3 state | ext4, label `neptron-state` | 516 MiB | 256 MiB | As on the PC |
| 4 share | exFAT, label `NEPTRON` | 772 MiB | The rest of the card, made at the first boot | As on the PC |

Every partition starts on a 4 MiB boundary, the erase block of common SD cards. `S06neptron-storage` needs no change for the SD card: it finds the disk from the device of `/` and the partitions by their `partition` number in sysfs, never by a name it builds, and it reads partition 4 of `sfdisk --dump` with a pattern that matches `/dev/mmcblk0p4` as it matches `/dev/sda4`. The xtask test `the_storage_script_finds_mmcblk0p3_and_reads_mmcblk0p4` runs those two functions of the script on `mmcblk0` names. Not verified on a card: `sfdisk --append` and `addpart` on `/dev/mmcblk0`, and the time `mkfs.exfat` takes on an SD card.

**Packages.** As on the PC, without LLVM: Mesa with the Gallium `vc4` driver (OpenGL ES 2 on the VideoCore IV) and softpipe, EGL and GBM; SDL3 with KMS/DRM and OpenGL ES; alsa-lib and `amixer`; libdrm; the four cores; the frontend, the launcher and `neptronctl`; `ksmbd-tools`, `mdnsd` and `iptables`; e2fsprogs, exfatprogs and util-linux. The init table, the mixer script and the BusyBox options are those of the PC image (`board/pc/rootfs-overlay`, `board/pc/busybox.config`); its QEMU script does nothing on a Pi. No Wi-Fi or Bluetooth firmware, and no test tool of the QEMU display.

**A Pi 4** needs at least these files that this image does not hold: `start4.elf` and `fixup4.dat`, its device tree `bcm2711-rpi-4-b.dtb`, the `v3d` driver of Mesa, and `overlays/overlay_map.dtb`, through which the firmware loads `vc4-kms-v3d-pi4` for the line `dtoverlay=vc4-kms-v3d`.

**Shut down and reboot.** As on the PC (see "Init, power and sound"). A Pi has no ACPI and no power switch. On a power-off, the watchdog driver (`CONFIG_BCM2835_WDT`, which the kernel check requires) tells the firmware to halt: the picture goes off, the green ACT LED stops, the red power LED stays on, and the board still draws a little power. Only a power cycle (unplug the supply and plug it in again) starts it again. A reboot resets the board through the same watchdog.

**Licence of the firmware.** The firmware files are not open source: `LICENCE.broadcom` allows redistribution in binary form, without modification, only for use with a Raspberry Pi, with its copyright notice and disclaimer. Buildroot labels the package `BSD-3-Clause`, which is wrong for these terms; `board/rpi/LICENCES.txt` says so, gives the terms, and reproduces the notice and the disclaimer, and `legal/LICENCE.broadcom` holds the file.

## The cores

One package for each core, pinned to an exact upstream commit, with the SHA-256 of the GitHub archive of that commit and of the licence file in its `.hash` file. `BR2_DOWNLOAD_FORCE_CHECK_HASHES` makes Buildroot refuse a download with another hash.

| Package | Upstream and commit | Library | Licence in the package |
| --- | --- | --- | --- |
| `libretro-fceumm` | `libretro/libretro-fceumm` `e0f3086a8104f8fbbe49a2615b10401504a9533f` | `fceumm_libretro.so` | `GPL-2.0+`, file `Copying` |
| `libretro-snes9x` | `libretro/snes9x` `fae2fea08f74180759ef540ee94259213f503480` | `snes9x_libretro.so` | `LicenseRef-Snes9x (non-commercial)`, file `LICENSE` |
| `libretro-genesis-plus-gx` | `libretro/Genesis-Plus-GX` `b9c2316441978d4015ba30e9a95d9b0e6f8ee981` | `genesis_plus_gx_libretro.so` | `LicenseRef-GenesisPlusGX (non-commercial)`, file `LICENSE.txt` |
| `libretro-fbneo` | `libretro/FBNeo` `d7450cf138ef553c9ba00c86dd4fb5fb1d7646bb` | `fbneo_libretro.so` | `LicenseRef-FBNeo (non-commercial)`, file `src/license.txt` |

The first three are the commits that `data/cores/README.md` names. Each package runs the upstream libretro makefile with `platform=unix` and the Buildroot cross toolchain, and installs the library in `/usr/lib/libretro`. The makefiles ask git for the version, and the build directory is no git checkout, so each package gives `GIT_VERSION` from its pin. These three packages have no architecture-specific flags, so the same files build for every target.

`libretro-fbneo` builds the full FinalBurn Neo core from `src/burner/libretro`:

- It also gives `GIT_DATE`, the author date of the pin, which changes with the pin.
- When `BR2_cortex_a53` is set it uses the makefile's platform `rpi3_64`, which adds `-march=armv8-a+crc+simd -mtune=cortex-a53`; elsewhere it uses `platform=unix`. Buildroot selects `BR2_cortex_a53` by default for aarch64, so the QEMU aarch64 image builds with `rpi3_64` too. The makefile adds `-O3 -ffast-math -frounding-math` on every platform; there is no LTO.
- On aarch64 the 68000 CPU core is Musashi, in C: the Cyclone 68000 core is ARM32 code, and the makefile turns it on only for the 32-bit Pi platforms.
- The package sets `INCLUDE_7Z_SUPPORT=0`, because NEPTRON reads zip sets only. The CHD code stays in: at the pin `intf/cd/cd_img.inc` uses it with no condition, and the build fails with `INCLUDE_CHD_SUPPORT=0`. `USE_X64_DRC` stays unset: no source reads it at the pin, and the tree has no Xbyak.
- The dependency files of the build name the third-party code in the library: zlib, MiniZip, libretro-common, uGUI with a Source Han Sans bitmap font, libchdr, the LZMA decoder of the LZMA SDK, the Zstandard decoder, dr_libs and stb_vorbis. `LIBRETRO_FBNEO_LICENSE_FILES` lists the licence text of each, and `board/*/LICENCES.txt` lists them.
- The three images build the core, and `data/cores/fbneo.toml` gives it to the `arcade` and `neogeo` folders.

Snes9x, Genesis Plus GX and FinalBurn Neo allow non-commercial use only. The owner decided (OP-054): NEPTRON is open source and not commercial, so the image holds them, with their licence texts and the `make legal-info` output, and it is never sold. The FB Neo licence also forbids requests for donations for a project that uses its source; the owner accepted it, and NEPTRON takes no donations (ADR 0014). `board/pc/LICENCES.txt` and `board/rpi/LICENCES.txt` say so on the boot partition.

### The community gamepad database

`package/sdl-gamecontrollerdb` downloads `mdqinc/SDL_GameControllerDB` at commit `a69b1ac650f4faab73596c2d561edc2c7636978e`, checked like the cores: the SHA-256 of the GitHub archive and of `LICENSE` (zlib) are in its `.hash` file. It builds nothing. It installs `gamecontrollerdb.txt`, unchanged, as `/usr/share/sdl-gamecontrollerdb/gamecontrollerdb.txt`, where the frontend reads its Linux lines after the NEPTRON mapping file (`docs/spec/input-generic-pads.md`). The file is not under `/usr/share/neptron`, because the install step of the `neptron` package empties that folder. The three images enable it. ADR 0017 records the licence and how to update the pin; `board/*/LICENCES.txt` names the pin, and an xtask test checks that the defconfigs, the hash file, the install path and the licence notes agree.

### FinalBurn Neo speed on the Raspberry Pi 3

Measured on 2026-11-16 on a Raspberry Pi 3 B with the development image (aarch64), the `performance` governor at 1.2 GHz, about 48 °C, on composite NTSC 240p: one field is 16.652 ms (60.054 Hz), and the game runs one frame per field. The core is the build of this package with `platform=rpi3_64` (`-O3 -ffast-math -frounding-math -march=armv8-a+crc+simd -mtune=cortex-a53`, and `-mcpu=cortex-a53` from the Buildroot wrapper; Musashi 68000 in C, no LTO). The sets are the owner's `ddonpach` (Cave, 57.55 Hz) and `mslugx` with `neogeo` (59.18 Hz); their files match the DAT of the pin by CRC and size.

Method: the image's own launcher and frontend ran by hand with the launcher held (`/run/neptron/hold`), and `ftrace` recorded `sched_switch` of the frontend threads, `drm_vblank_event` and `vc4_submit_cl_ioctl`. "Frame thread" is the on-CPU time of the frame thread in each field (core, upload, render, audio resample). "Core bound" is the time from the flip event to the first GPU submit of the field, an upper bound of the core time. The play rows follow a coin, a start and fire held.

| Run | Fields | Missed fields | Underruns | Frame thread p50 / p99 / max | Core bound p50 / p99 / max |
| --- | --- | --- | --- | --- | --- |
| `ddonpach` attract, 180 s | 10,810 | 0 | 0 | 7.31 / 8.64 / 8.99 ms | 5.83 / 7.16 / 7.50 ms |
| `ddonpach` play, 120 s | 7,207 | 0 | 0 | 7.46 / 8.78 / 9.38 ms | 5.98 / 7.28 / 7.74 ms |
| `mslugx` attract, 180 s | 10,810 | 0 | 0 | 8.88 / 10.83 / 11.33 ms | 7.42 / 9.37 / 9.79 ms |
| `mslugx` play, 120 s | 7,207 | 0 | 0 | 9.58 / 11.16 / 11.83 ms | 8.13 / 9.69 / 10.28 ms |

Both games run at full speed with margin: the worst field used 11.83 ms of 16.65 ms. The frame thread used 44 to 57 % of one core. These rows come from a build with the 7z support in. The shipped build (`INCLUDE_7Z_SUPPORT=0`) was checked again for 60 s of each attract mode: no missed field and no underrun, frame thread p99 8.15 ms (`ddonpach`) and 10.79 ms (`mslugx`), core bound p99 6.67 ms and 9.33 ms.

## The Rust toolchain of the aarch64 output

The aarch64 build also installs the Rust toolchain of Buildroot (`host-rustc`, the prebuilt `rust-bin`, with the standard library for the target) in the output, not in the image. `cargo xtask gles-compare` uses it with the Buildroot C toolchain as the linker to build Rust programs for the guest. This toolchain (Rust 1.97.1 in Buildroot 2026.08.1) is not the one that `rust-toolchain.toml` pins for the host (1.97.0); the two can drift apart when Buildroot is bumped.

## What was verified

The dates and the machine of each measurement are given. Emulated times say nothing about the i3-6100T. `docs/ci.md` has the times and sizes of the CI runs.

**QEMU runs, aarch64 image, Apple Silicon Mac, 2026-11-11.**

- The guest boots to the command in 0.2 to 1.2 s after QEMU starts (guest uptime 0.1 to 0.8 s). Exit codes and quoted words pass through. Output that holds `NEPTRON-END 0` does not end the run. A command over its deadline ends with 137 and the guest stops. After SIGTERM or SIGHUP to `xtask`, no QEMU process is left one second later. After SIGKILL to `xtask`, QEMU stops at the guest deadline.
- `neptron-vkms-probe` and `neptron-capture` refuse a device whose driver is not `vkms` before any commit or signal (both images hold them, because the PC image is also the QEMU image of x86\_64): on `/dev/null` both exited with 1 and the launcher was not stopped. No other DRM driver exists in the guest, so the refusal of i915 is shown by the code path only.
- `neptron-vkms-probe` with 320x240 at 6.7 MHz, 640x480, 720x480i and a doublescan mode: every commit is accepted, the readback equals the request, the CRCs and the writeback work.
- `sleep 60 | kmscube -D /dev/dri/card0 -c 300` draws on `vkms` with softpipe at about 53 frames per second. kmscube needs a standard input that stays open. `kmscube -A` (atomic) stops with "no eglDupNativeFenceFDANDROID", because softpipe has no native fence.
- EGL on the Mesa surfaceless platform with no device gives OpenGL ES 2 on softpipe, which `cargo xtask gles-compare` uses: the cross build of the comparison takes 7 s from cold, and the guest run under 10 s.
- The libraries of the first three cores are 4.3 MiB (`fceumm_libretro.so`), 2.7 MiB (`snes9x_libretro.so`) and 12.0 MiB (`genesis_plus_gx_libretro.so`).

**FinalBurn Neo, 2026-11-16.** The build step of `libretro-fbneo` took 763 s with 4 jobs from cold on the Pi output. `fbneo_libretro.so` after the strip is 82,563,440 bytes on aarch64 and 84,950,640 bytes on x86_64. With it, the target tree and the root are: Pi development image 164.1 MB, EROFS 72.5 MB in the 384 MiB partition; QEMU aarch64 image 142.9 MB, ext4 of 256 MiB with 97.5 MiB free; PC development image 443.7 MB, EROFS 205.9 MB in the 384 MiB partition, ext4 of 512 MiB with 65 MiB free.

**Boot checks, PC and aarch64 images with the frontend, same Mac, 2026-11-12 and 2026-11-13.**

- `cargo xtask os boot-check --arch x86_64` passes with BIOS (SeaBIOS) and with UEFI (EDK II): the firmware boots the USB image from a USB stick on xHCI through GRUB, the kernel mounts the EROFS root by PARTUUID, and the frontend shows the game list. The `vkms` writeback picture equals `tests/golden/boot-game-list.ppm` byte for byte. Under CPU emulation (TCG, single thread) the first frame came at a guest uptime of 12.2 to 46.0 s over eight runs, with the load of the machine.
- `cargo xtask os boot-check --arch aarch64` passes with the hypervisor: first frame at a guest uptime of 0.55 s, and the same picture byte for byte.
- The picture is the empty home screen, 320x240 in the 1280x240 super-resolution mode of the safe profile `generic_15` (26.027 MHz, 15.66 kHz, 60 Hz). `vkms` reports no EDID, so `display.monitor = auto` gives `generic_15`, and the guard opens no boot prompt (ADR 0013). The check waits for the home screen (`neptron-capture --screen home`), which has no timer. The golden was captured on 2026-11-16 on x86_64 with SeaBIOS; EDK II and aarch64 give the same bytes.
- After `kill -ABRT` of the frontend on the aarch64 image, while the launcher lives, the CRTC stays active with the 1280x240 mode and the plane keeps the framebuffer of the dead frontend; the launcher starts a new frontend 250 ms later (its crash backoff), and the mode is the same.
- The 15 kHz patch set applies in Buildroot, and the patched kernel builds and boots in both boot checks.
- `kms-check`, `gles-compare` and `core-trace` pass on the aarch64 image that holds the frontend; `os app-check` passes for aarch64 and x86\_64.

**The share partition**, on a writable copy of the development image of 2026-11-13 (`08ef1064…`) with 1 GiB of free space after the state partition (`cargo xtask qemu --arch x86_64 --disk COPY --script ...`), the steps in order on one copy:

- A, a fresh stick: partition 4 made, found empty, formatted exFAT, mounted; the marker removed; a file written.
- B, the second boot: mounted, not formatted; the file is there.
- C, the image written again with `dd` over the start of the copy (no partition 4 in the table, the old share on the disk): partition 4 made again at the same start, the old exFAT found and mounted, not formatted; the file is there.
- D, a power cut after `sfdisk` (marker written by hand, first 8 MiB of partition 4 zeroed): the next boot formatted it once and mounted it; the boot after that did not format it, and a file written in between is there.
- E, partition 4 zeroed and no marker: left untouched (state 10), `/share` a RAM folder.
- F, a share with files whose main boot sector is zeroed, the backup boot region intact, no marker: left untouched, `/share` a RAM folder.
- G, the same share with the marker written by hand: left untouched (state 14).
- Not run: a read error of a real stick (QEMU gives none), and the scan that finds an old share at another start (no image of another layout exists yet).

**Why the script trusts no exit status.** A first boot on a real 60 GB stick, written over an older image with a share at the same start, once gave a RAM share: the table on the disk had partition 4 and the kernel did not. The likely cause, not proven (the stick gave no log): in util-linux 2.41.5, `sfdisk` exits with 1 after the table is written only when `fsync` or `close` of the disk fails, and a USB stick that refuses SYNCHRONIZE CACHE gives such an `fsync` error. So the script reads the table back and logs every step. On 2026-11-13, copies of 128 MiB free and of 60 GB (120127488 sectors), each written again over a share with files, made partition 4 around the old share and mounted it with the files. There `sfdisk` exits with 0, prints "Partition #4 contains a exfat signature", does not ask (its standard input is not a terminal) and wipes nothing; the kernel's failed table read (`EBUSY`) does not change its exit status.

**The network share, aarch64 image, same Mac, 2026-11-13** (an incremental build of 178 s that added `ksmbd-tools`, GLib, libnl, `mdnsd` and the kernel server):

- `cargo xtask os boot-check --arch aarch64` passes, with its step that waits for port 445 and `mdnsd`; the frontend started at a guest uptime of 0.39 s.
- A guest script: port 445 listens; `/etc/network/interfaces` holds the loopback only; the user `neptron` has `/bin/false` and no password (`neptron:*` in `/etc/shadow`); `/share` (a RAM folder) is owned by uid 65534 with `nosuid,nodev,noexec`; `ksmbd.control --shutdown` stops the server; `ksmbd.mountd -n -v` logs `New user 'neptron'`, `New user 'nobody'`, `New share 'share'` and no "Invalid pwddb entry", so the user database parses.
- Not proved: a login over the network, as guest or as `neptron`. The image has no SMB client, and the QEMU runs have no network card for port 445. See `docs/hardware-checklist.md`.

**What QEMU proved for the bench** (the development image on a writable copy, `cargo xtask qemu --arch x86_64 --disk COPY --ssh-port 2222 --beside-launcher`, then the xtask commands with `--host 127.0.0.1 --port 2222`): the server offers only `publickey` and denies a password login; `os push` ran the hot binaries (the log and `/proc/<pid>/exe` showed `/state/hot/bin/...`), and `--data` the hot data; `DISABLED`, and the removal of `/state/hot`, gave the image's own binaries again; `os logs` read the log; after a reboot the same host key was accepted. A warm push took 19 to 26 s under emulation (build 3 s, the rest the copy of 32 MB). QEMU does not prove a real network, DHCP from a router or the serial console of the PC.

**The Pi image, build only, Apple Silicon Mac, 2026-11-13.**

- The release image in an empty output took 5,549 s (1 h 32 min, 4 jobs). The kernel hash, the kernel check and the patch check passed. The kernel is `6.18.62-v8`, `Image` 27,982,336 bytes. The SD card image is 809,500,672 bytes (772 MiB), 48 MB with gzip, with the signature `4e505052` and the partitions at sectors 8192, 270336 and 1056768; the overlay `vc4-kms-v3d.dtbo` has the `composite` fragment.
- The development variant took 3,608 s in its own empty output, with the compiler cache of the first build; its `config.txt` ends with `enable_uart=1` and `dtoverlay=disable-bt`, and its root PARTUUID is `4e505044-02`.
- The two outputs take 18.4 GB each on the Docker disk.

The boots on a Pi 3 B are in `docs/spec/raspberry-pi.md`.

**Build times and sizes, local, 4 jobs.**

- PC image: the first full build with LLVM, clang and the 15 kHz kernel took 17,419 s (4 h 50 min; host LLVM and clang take most of it). A rebuild of the frontend and the launcher from clean (fat LTO), with the image, takes about 5 min (309 s). The kernel `bzImage` is 14.2 MiB. In the target directory, LLVM is 74 MiB and the unused clang libraries 104 MiB (see the open points). The USB image file is 739,246,080 bytes (705 MiB); most of it is the free space of the fixed partitions, so it compresses well.
- aarch64 image: a kernel reconfigure, or a change to the overlay or to a package of this tree, takes about 1 min. The kernel `Image` is 13.3 MB. The guest uses about 100 MB of memory after boot.
