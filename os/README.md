# os

The Buildroot br2-external tree of NEPTRON. It defines three OS images:

- the x86\_64 PC image: the USB image for the i3-6100T, and also the image that QEMU boots for the x86\_64 checks;
- the aarch64 QEMU test image, which tier T3 runs on a Mac;
- the 64-bit Raspberry Pi image for the Pi 3 B and 3 B+ (see "The Raspberry Pi image").

## Pinned versions

| What | Version | Pin |
| --- | --- | --- |
| Buildroot | 2026.08.1, the newest stable release on 2026-11-11 | Tag commit `7fdeadb3b9f12f78e5fccca914d7fef47090ab9a`; `buildroot-2026.08.1.tar.gz` SHA-256 `38a6c152490096018720a9ade379ee1e571c9bf5f93a3657a166566945a98b0d`, from the signed release announcement. `Dockerfile` fetches and checks it |
| Linux | 6.18.63, the newest 6.18 LTS release on 2026-11-11 | SHA-256 in `patches/linux/linux.hash`, from kernel.org |
| 15 kHz patch set (PC image) | `D0023R/linux_kernel_15khz`, folder `linux-6.18`, commit `d85a343379f7f54155f69ef56dc544b666635894` (2026-11-15), GPL-2.0-only | The nine patch URLs in the defconfig name the commit; their SHA-256 values are in `patches/linux/linux.hash`. See the licence below |
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
| `board/common/post-build.sh` | Post-build step of every image. It removes the init script of the `mdnsd` package. It writes `/etc/network/interfaces` with the loopback only (an output built with `BR2_SYSTEM_DHCP` keeps a DHCP stanza, because Buildroot does not install `ifupdown-scripts` again). It makes an empty `/mnt/host`, because the root of the PC and Pi images is read-only. It checks the files of the network share. It runs `check-arcade-index.sh`, which fails the build when the image has the FinalBurn Neo core and no arcade index (`/usr/share/neptron/arcade/fbneo.idx`). The `neptron` package writes that index in its install step, from the DAT of the `libretro-fbneo` source, so a rebuild of the frontend package alone keeps it. It runs `check-gamecontrollerdb.sh`, which fails the build when the configuration enables `sdl-gamecontrollerdb` and the image has no `/usr/share/sdl-gamecontrollerdb/gamecontrollerdb.txt`, or an empty one |
| `board/common/rootfs-overlay` | The init scripts of every image: debugfs and 9p, the state and share partitions, DHCP, the network share, the launcher, and the commands from the host; the configuration of the SMB server (`etc/ksmbd`) and of the mDNS service (`etc/neptron/mdns.d`) |
| `patches/` | The global patch and hash directory. It holds the kernel hashes and the hashes of the 15 kHz patch set |
| `package/` | NEPTRON packages: the frontend `neptron`, the launcher `neptron-launch`, the client `neptronctl`, the cores, the community gamepad database `sdl-gamecontrollerdb`, and the test tools `neptron-vkms-probe` and `neptron-capture`. `package/neptron/local-source.mk` holds the rsync hooks of the packages that build from this tree |

The kernel of the aarch64 test image is the Buildroot QEMU configuration (`board/qemu/aarch64-virt/linux.config` in the Buildroot tree) plus the five fragments in `board/qemu`. The kernel of the PC image is the `x86_64` defconfig of the kernel plus the fragments in `board/pc` and `board/qemu/host.config`.

The `x86_64` defconfig makes `userspace` the default CPU governor, and `intel_pstate` and `amd-pstate` then start in `powersave`. `system.config` makes `performance` the default, and `check-kernel-config.sh` requires it. With `performance`, `intel_pstate`, `amd-pstate` and `acpi-cpufreq` ask for the top rate (the active modes also set the energy preference to 0). A QEMU guest has no cpufreq driver, so the setting has no effect there.

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

The image holds no share partition: the first boot makes it. `S06neptron-storage` works only on the disk that holds the root filesystem. The script makes partition 4 only when the disk has no partition 4, the state partition is mounted, and at least 64 MiB are free after the state partition. It first looks for an old exFAT share in the free space: an exFAT boot sector at any 1 MiB boundary of the first 4 GiB. An old share at another start than the planned one leaves the disk untouched. Else the script writes the marker `/state/share-pending` (with a sync) and appends partition 4 (type `0x07`) at the planned start, to the end of the disk, with `printf '<start>,,7' | sfdisk --force --no-reread --no-tell-kernel --wipe never --wipe-partitions never --lock=nonblock --append`. An MBR entry ends at sector 2^32 - 2 at most, so on a disk of 2 TiB or more the share ends near 2 TiB and the rest stays unused. The start is given because a start that sfdisk chooses follows the optimal I/O size of the device, which can be 2, 4 or 16 MiB. The disk is in use, the other partitions stay as they are, sfdisk asks nothing, and it wipes no signature, because the free space can hold the share of an older image.

The exit status of sfdisk decides nothing. The script then runs `blockdev --flushbufs` on the disk and logs its exit status. (BLKFLSBUF in Linux 6.18 writes the dirty pages of the whole-disk device, drops its clean pages and returns 0. The mounted partitions have caches of their own, which it does not touch.) Then it reads the table back with `sfdisk --dump`, up to 5 times with 0.5 s between tries (`TABLE_READ_TRIES`, `TABLE_READ_RETRY_US`). When the flush exits with 0, this proves that the device returns the new table when it is asked again. The exit status of the flush is no proof by itself: the log line says "the table is read back after a flush of the cache", or "the cache was not flushed, so the table may be read back from the cache". It does not prove that a stick with a volatile write cache keeps the table through a power cut.

The script goes on only when partition 4 is there with type 7, the planned start and the planned size. A table that was read and has no such partition 4 removes the marker. A table that cannot be read keeps it, and the next boot, where the kernel reads the table, uses partition 4. Then `addpart <disk> 4 <start> <size>` gives the kernel the partition with the numbers of the table read back (BLKPG; it reads no table), up to 5 times with 0.5 s between tries (`ADD_TRIES`, `ADD_RETRY_US`). The kernel's list decides, not the exit status of addpart. Then the script waits up to 5 s for the device node (`NODE_WAIT_STEPS`, `NODE_WAIT_STEP_US`). A partition 4 that is in the table but not in the kernel (from a step that failed earlier in the same boot) goes to `addpart` directly, with no new `sfdisk`.

A partition 4 with a filesystem is mounted as it is and never formatted. An empty one is formatted exFAT only when all of these hold. A freshly written image owns the free space: the remains of a foreign file system there (an old ext4 or NTFS) are no user data, and these rules do not look for them. The one thing they keep is an old exFAT share:

- the marker `/state/share-pending` is there and names the start and the size of this partition 4 (`start=<sector>` and `size=<sectors>`, written before sfdisk runs): this system made this partition. A marker of an older image names none, so it allows nothing;
- its type is `0x07` and it starts at the first 1 MiB boundary after the state partition;
- its first 16 MiB read with no error, and `blkid -p` exits with no filesystem found and prints nothing on standard error;
- it holds no exFAT backup boot sector at sectors 12, 24, 48 or 96 (a share with a damaged main boot sector is the user's data);
- no exFAT boot sector starts at a 1 MiB boundary from the planned start through the first 4 GiB of partition 4 (an old share). The scan before the partition is made covers this window at the first boot, so that boot scans once. Here a sector that does not read counts as no boot sector: the share must come up, and an old share that does not read cannot be mounted either.

These are the rules of the first boot. The rules of a format that the user confirms read more, and refuse on a read error: see "A format that the user confirms".

**The life of the marker.** The script writes the marker before sfdisk, with a sync. It removes it:

- when partition 4 holds a filesystem signature (also when that filesystem then does not mount);
- when the table read back has no partition 4, or one off the plan;
- after the first mount;
- when a type change of a request wrote the table and the request is then refused.

It keeps it when the table cannot be read back, when the kernel does not take partition 4, when the device node does not appear, when a format rule refuses, and when `mkfs.exfat` fails. So the marker can outlive a failed boot, and the next boot that finds an empty partition 4 with the planned start and size formats it.

**Logs and `storage.toml`.** When any check fails, nothing is formatted or mounted, `/share` is a RAM folder, and one log line says that the share partition was left untouched and why. Every way out of the script that leaves no share partition sets such a reason, and the reason goes to `storage.toml` and to the notice of the frontend. Every line of the script goes to the console and to syslog (`logger -t neptron-storage`, `/var/log/messages`, a RAM folder). When `/state` is mounted, the script also appends the lines of the boot to `/state/log/storage.log` itself, with no syslog, and keeps the last 300 lines (`STORAGE_LOG_LINES`). So the release image, which has no SSH, keeps them for a later look on another computer. `cargo xtask os logs` prints both. Every variable that the script reads is set by the script first: init passes the kernel command line to its children as variables.

The state partition gets `e2fsck -p` first. When it leaves errors (exit 4 or more), `/state` is a RAM folder and nothing is written to the damaged filesystem. With `/state` in RAM the guard record still counts the failed boots of this power cycle, so the guard boots as usual.

At the end, the script writes `/run/neptron/storage.toml`: `state` and `share`, each `partition` or `ram`, and for a RAM folder `state_reason` or `share_reason`, the log line that names the reason. A format offer and the result of a format request add the fields of "A format that the user confirms". The frontend reads the file at boot, logs it, and shows a notice for each RAM folder until the player presses OK.

After the share is mounted, the script makes `roms/<system>` for each system of `data/systems`, and `bios`. Copy ROMs into `roms/nes`, `roms/snes` or `roms/megadrive` from any computer.

The boot check (`cargo xtask os boot-check`) expects a state and a share partition on x86\_64 (its disk copy has 128 MiB free after the state partition, so the first boot makes, formats and mounts partition 4) and two RAM folders on aarch64, whose notices it dismisses. `cargo xtask os share-check` boots copies of a stick written again, of a format offer and of a reused card (`xtask/README.md` describes both). The share cases A to G in "What was verified" are the proof of these rules.

**Limits of the share rules.** A power cut during `mkfs.exfat` can leave a filesystem that `blkid` sees but that does not mount. The script then does not format it again, the marker stays, and `/share` is a RAM folder at every boot. No data is lost, because the partition was new. To recover, format partition 4 as exFAT on another computer, or remove it so that the next boot makes it again. `Settings > System` offers no format here, because `blkid` sees a filesystem. The scan for an old share looks at the first 4 GiB of free space only, so an old share that starts beyond that is not found. When the kernel does not take partition 4 after the 5 tries of `addpart`, the share is a RAM folder for this boot, and the notice says "partition 4 is in the partition table, but the kernel does not see it after 5 tries of addpart (last exit N); restart to use it". The kernel reads the table at the next boot, so a restart mounts it. No such case is known; the line is there so that it is never silent.

**States where `/share` stays a RAM folder at every boot.** Each has its reason in `storage.toml` and in the notice; the lines start with `neptron-storage: `. States 10, 11, 12 and 17 also come with the offer of a format that the user confirms, when its checks hold (see "A format that the user confirms" below), and then stay only until the user accepts it.

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

**A format that the user confirms.** Some of the states above leave `/share` a RAM folder at every boot only because the script never formats a partition 4 that it did not make. For those, the script offers a format, and the user decides in `Settings > System > Format share` (`docs/spec/ui-navigation.md`). The script stays the only code that formats.

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
- On the offer and the request, a read that fails in these checks or in the scan for an old share refuses, with "cannot be read": a failed read is no proof that nothing is there. (The first boot goes on over a sector that does not read in its scan: see the rules above.)
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

**Writing a new image to a stick that has a share.** A new image writes a new partition table and a new state partition, with no marker. With the same layout, the first boot makes partition 4 again at the same start, finds the old exFAT share there, and mounts it: the games stay. An image of a different layout (other sizes of the root or the state partition) places the new partition 4 at another start, perhaps inside the old share. The script then finds the old share's boot sector at its own start, makes no partition, and logs `an old exFAT share starts at sector N`; the frontend shows an empty game list, because `/share` is a RAM folder. The games are still on the stick: copy them from the stick on another computer, with a tool that reads exFAT at that offset (or add a partition entry that starts at sector N by hand), then delete the old share and boot again. If the old share's boot sector was overwritten too, the script cannot see it and makes and formats the new partition. So copy the games off the stick before you write an image of another layout.

**Boot.** GRUB, `timeout=0`, no menu and no video module, so it sets no display mode. The kernel command line is `root=PARTUUID=<signature>-02 rootfstype=erofs ro rootwait quiet console=ttyS0,115200 panic=10 usbcore.old_scheme_first=1`.

`usbcore.old_scheme_first=1` makes the USB core enumerate a new device with the old scheme first: `SET_ADDRESS`, then a read of 8 bytes of the device descriptor. With the default scheme (a read of 64 bytes at address 0, then a second port reset), some controllers enumerate but give no input until they lose power. The Mad Catz PS3 FightStick TE (0738:8838) is one. Linux chooses the scheme before it reads the device descriptor, so no quirk for one device can select it. The USB drivers are built in and enumerate before user space, so only the command line covers a controller that is plugged in at power-on. `usbcore.use_both_schemes` stays at its default, so a device that refuses the old scheme enumerates with the other one. SuperSpeed devices always use the old scheme. The `xtask` test `every_image_kernel_enumerates_usb_with_the_old_scheme_first` checks that `grub.cfg` and the `cmdline.txt` of the Pi have the parameter, and the x86\_64 boot check reads it back from `/sys/module/usbcore/parameters/old_scheme_first`.

The kernel has the USB, SATA, NVMe and virtio disk drivers, EROFS, ext4, exFAT and FAT built in. There is no initramfs.

The build also makes `rootfs.ext4` (`BR2_TARGET_ROOTFS_EXT2_SIZE`, 512 MiB) from the same target tree. It is not in the disk image, and its size changes no partition. `cargo xtask qemu --arch x86_64` without `--disk` boots the kernel with it as a virtio disk, as the aarch64 test image does. With FinalBurn Neo the target tree is 443.7 MB (2026-11-16, development variant), and the 512 MiB filesystem keeps 65 MiB free; at 384 MiB, `mkfs.ext4` failed with "Could not allocate block". Check the free space again when a large package comes in.

**What a CRT gets before the frontend runs.** NEPTRON sets no mode until the frontend makes its first validated commit, but the PC sets one before it:

1. The BIOS or UEFI POST, and the GRUB text console (GRUB loads no video module, so it keeps that mode), show in the mode that the firmware set. That mode is 31 kHz or higher.
2. i915 inherits that mode and keeps it on the wire until the first validated commit of the frontend. In QEMU the first frame came 15 to 25 s after the start of the kernel under CPU emulation; on the i3-6100T this time is not measured.
3. A monitor that takes 15 kHz only gets an out-of-range signal for that whole time. Use an adapter that protects the monitor from such a signal, or switch the monitor on after the boot.
4. The definition has the launcher blank the output to shorten this window. This image does not do that step: the launcher sets no mode and makes no commit (`apps/neptron-launch/README.md`).

**The kernel sets no mode of its own** (the safety model, layer 8). There is no virtual terminal (`CONFIG_VT` off, which needs `CONFIG_EXPERT`), no fbdev emulation, no `CONFIG_FB`, no framebuffer console, no `simpledrm`, `efidrm` or `vesadrm`, no firmware framebuffer device, no DRM log client and no DRM panic screen. `post-build.sh` reads the final kernel configuration and fails the build when one of these is on. i915 is built in. amdgpu, radeon and vkms are modules. The kernel log goes to the first serial port only.

**Init.** BusyBox init with `mdev`. There is no getty and no login on any console. The init scripts mount the partitions and `/run` (tmpfs). Then init starts `/usr/libexec/neptron/launch`, which makes `/run/neptron` and runs `neptron-launch --power -- /usr/bin/neptron --backend kms`. Before that, the launch script sources `/usr/libexec/neptron/mesa-cache`: when `/state` is the ext4 state partition mounted read-write, Mesa keeps its shader cache in `/state/cache/mesa` (`MESA_SHADER_CACHE_DIR`, at most 64 MiB), else the cache is off (`MESA_SHADER_CACHE_DISABLE`). The read-only root has no place for it. The cache can be deleted at any time, and a folder that cannot be made only turns the cache off.

The launcher holds the DRM descriptor and starts the frontend again when it ends. Init starts the launcher again when it ends. So a "quit" is a restart of the frontend, not a power-off: the frontend ends with exit code 0, the launcher stops, and init starts the launch script again at once. To stop, use `Settings > System > Shut down` (or `Reboot`): the frontend ends with exit code 81 (80), and the launcher, which has `--power`, runs BusyBox `poweroff` (`reboot`). Init then runs the `::shutdown:` entries of `inittab` (the stop script, `rcK`, `umount -a -r`), so the save RAM and the settings are on disk and the ext4 state and exFAT share partitions are unmounted clean before the machine turns off. The PC turns off through ACPI (`CONFIG_ACPI`, which `check-kernel-config.sh` requires); the aarch64 QEMU image through PSCI. A power cut is still safe: the state and share partitions are written with care for it.

A stop of the launcher with SIGTERM (`kill $(pidof neptron-launch)`, or init at shutdown) forwards SIGTERM to the frontend, which ends its run as a quit. A frontend that has not ended 5 s later, for example one stuck inside a core, gets SIGKILL from the launcher. Only that timeout, the SIGKILL fallback of `os push`, or init ends a frontend that hangs. The frontend reads its folders from the image defaults: `/share`, `/state`, `/usr/share/neptron`, `/usr/lib/libretro`, `/run/neptron`.

**Packages.** `ksmbd-tools` and `mdnsd` for the network share (see "The network share"). Mesa 26.1 with LLVM: the iris driver for Intel gen8 and later (the HD 530 of the i3-6100T), llvmpipe, and softpipe; EGL, OpenGL ES and GBM. SDL3 with KMS/DRM and OpenGL ES, without libudev, and alsa-lib. `amixer` of alsa-utils, and nothing else of it. libdrm. The four cores (see "The cores"). The frontend, the launcher, `neptronctl` (with the `kms` feature, for the bench self-test), `neptron-vkms-probe` and `neptron-capture`. e2fsprogs, exfatprogs, and the basic and partition tools of util-linux, for the partitions. There is no firmware file: i915 drives the display without its DMC firmware, which is not loaded, and amdgpu does not work without its firmware (see "Licences").

**Sound.** There is no sound server: SDL3 talks to ALSA. The kernel leaves an HDA codec at its reset state, with the DAC at the bottom of its range and the output pins muted (seen on the i3-6100T with its ALC221). `S08neptron-mixer` runs in the background at boot. It waits up to 10 s for a card, then sets the playback controls `Master`, `PCM`, `Front`, `Surround`, `Center`, `LFE`, `Side`, `Headphone`, `Speaker`, `Line Out` and every `IEC958` switch of each card to 0 dB and unmutes them. 0 dB, not 100%: a control that goes above 0 dB adds gain and can clip. The input controls stay as they are. The volume of the frontend then scales the samples in software (`docs/spec/audio-settings.md`). A card plugged in after the boot keeps its own state.

**The frontend, the launcher and `neptronctl`** are Buildroot packages that build this checkout (`package/neptron`, `package/neptron-launch`, `package/neptronctl`). `xtask` mounts the checkout read-only at `/src/repo`. Each package copies it without `target/`, `.worktrees/` and `.git`, runs `cargo vendor --locked`, and builds offline in the workspace release profile (fat LTO, one codegen unit, `panic = "abort"`). The sdl3 crate adds `/usr/local/lib` to the link search on every Linux host, and the Buildroot compiler wrapper refuses that path. `package/neptron/link-filter.sh` drops only that path and links with the wrapper. With `panic = "abort"` no destructor runs. The launcher keeps its descriptor open, so the framebuffers and the mode stay on the wire until the next frontend commits (`docs/spec/kms.md`).

**The QEMU link.** The PC image boots in QEMU as on a PC. Its QEMU hooks act only when the QEMU firmware config exists, and a PC does not have one. `S07neptron-qemu` loads `vkms`, `snd-dummy` and `snd-aloop`. `S99neptron-run` runs the command of `cargo xtask qemu` instead of the launcher. The `once` entry `host-run check` runs the boot check beside the launcher. On x86\_64, `cargo xtask qemu` with no command gives no shell: the image has none.

### The network share

`docs/spec/network-share.md` is the spec. Every image has it, on by default: release, development and the aarch64 test image.

- **The network.** `S40neptron-network` starts `udhcpc` on each wired port (`eth*`, `en*`) as a background job, with the host name `neptron` in the request, and returns at once. No step of the boot waits for the network: a PC with no cable boots with no wait, and a cable plugged in later gets its address, because `udhcpc` keeps asking.
  - A port that appears after the script ran (a USB Ethernet chip, such as the one of a Raspberry Pi, or a USB adapter), or that is unplugged and plugged in again, gets its client from `mdev`. `board/common/post-build.sh` appends the rule `SUBSYSTEM=net;(eth|en).*` with `*/usr/libexec/neptron/net-hotplug` to `/etc/mdev.conf`; it replaces the line of an older build, because mdev takes the first rule that matches. On an add the hotplug script runs `S40neptron-network start`; on a remove, `S40neptron-network port-gone <port>`, which stops the client of that port and removes its pid file, so the port gets a new client when it comes back.
  - A pid file counts only when its process is a `udhcpc` for that port (`/proc/<pid>/cmdline`); any other pid file is stale and the port gets a new client. Each start writes the pid file of its client at once, so no port gets two clients.
  - Before `S40neptron-network` has run, the hotplug script does nothing, so it cannot start DHCP before the state and share partitions are mounted. The `mdev` daemon runs it, so it never holds up the boot, and its lines go to syslog (`logger -t neptron-network`). mdev runs commands with umask 0, so both scripts set umask 022 first. Two runs take turns through a lock folder (`/run/neptron/dhcp.lock`, at most 50 tries 0.1 s apart); a run that ends while it holds the lock frees it.
  - The SMB server and `mdnsd` need no restart for a late port: the kernel server has no interface list (`bind interfaces only = no`), so it opens a listener on each port that comes up, and `mdnsd` follows the link and address events of the kernel and checks the interfaces every 10 s.
  - The release image has no firmware file; the RTL8168h of the bench PC works without its firmware patch. There is no Wi-Fi.
- **The address file.** `/usr/libexec/neptron/net-address` writes the IPv4 addresses of the wired ports to `/run/neptron/network.toml`, in every image, at each event: the hook of `udhcpc` (`/usr/share/udhcpc/default.script.d/neptron-address`) runs it at `bound`, `renew`, `deconfig`, `leasefail` and `nak`, and `S40neptron-network` runs it at boot and when a port goes. The file holds `version = 1`, the host name and one `[[address]]` table (`port`, `ipv4`) for each port with a lease; the Network panel of the frontend reads it. It is written only when its content changes, through a temp file and a rename. `udhcpc` never waits for it: the lock is a `flock -n` on `/run/neptron/address.lock`, which the kernel frees when its holder ends; a run that finds it taken leaves a flag and returns, and the holder writes again before it ends. `S40neptron-network` waits at most 1 s for a stopped client to end, so its late `deconfig` cannot remove the address of a new client of the same port. A failed write gives a syslog line, and the script still exits 0. On the development image a changed file also prints `neptron: address eth0 A.B.C.D` on the console. Nothing about the address goes to `/share`; an old `neptron-ip.txt` of an older development image stays there as it is. `board/common/post-build.sh` fails when the `udhcpc` script of the image runs no hooks or the hook is not executable, and it removes the old `S41neptron-address` of an older output. `docs/spec/network-share.md` (section 3) has the rules. `cargo xtask os share-check --case address` checks the file on a lease, on a removed port and on a late port (`xtask/README.md`).
- **The share.** `\\NEPTRON\share`, or `\\neptron.local\share`, or `\\<address>\share`: the share partition `/share` (or its RAM folder), read-write, for guests only. The kernel server `ksmbd` (`CONFIG_SMB_SERVER=y`) serves SMB 2.1 to 3.1.1; `ksmbd.mountd` reads `/etc/ksmbd/ksmbd.conf`. A client with an unknown name or with no name is a guest; the name `neptron` with a wrong password gets a logon failure. One account exists for Windows 11 24H2, which refuses guest sessions: name `neptron`, password `neptron` (`/etc/ksmbd/ksmbdpwd.db`, committed). The password is public and gives no protection. The account and a guest both work as the user `nobody` (`force user`, `force group`). The system user `neptron` (`board/common/users.table`) has no password and the shell `/bin/false`; `board/common/post-fakeroot.sh` fails the build otherwise. The storage script mounts the share partition with `uid=65534,gid=65534,dmask=0022,fmask=0133,nosuid,nodev,noexec`, so the guest owns every file there and nothing on it runs as a program. `/state` and the root are not shared.
- **The name.** `mdnsd` answers for `neptron.local` and announces `_smb._tcp` on port 445, from `/etc/neptron/mdns.d`. macOS and the Linux file managers list the PC under "Network". There is no WS-Discovery and no NetBIOS name service, so the Windows "Network" view does not list it; `\\NEPTRON` works when the router resolves the DHCP host name.
- **Windows 11 24H2** refuses a guest share by default (insecure guest logons off, SMB signing required). Log in as `neptron` with the password `neptron`. A plain open in Explorer sends the Windows account name first, which becomes a guest that Windows refuses, so give the account first: `net use \\NEPTRON\share /user:neptron neptron`, or "Map network drive" with "Connect using different credentials", or a Windows credential for `NEPTRON` in Credential Manager. Or, as guest, turn on "Enable insecure guest logons" and turn off "Digitally sign communications (always)" on the Windows PC. See the spec.
- **The switch.** `S45neptron-share` starts `ksmbd.mountd` and `mdnsd` in a background job, unless `/state/network-share-off` exists. Then port 445 and port 5353 stay closed. `/etc/init.d/S45neptron-share restart` applies a change of the file at once. No settings row does that (OP-092).
- **The stop.** In `rcK`, before `umount -a -r`: `mdnsd` stops, `ksmbd.control --shutdown` closes every session, the script waits up to 5 s until no socket of the server is left and both daemons have ended, then `sync`. So the share partition unmounts clean after a copy.
- **What is open on the release image**: TCP 445 and UDP 5353, and UDP 68 for the DHCP answers while `udhcpc` has or asks for a lease. No SSH, no telnet.
- **What the network reaches.** Any host on the LAN can put a crafted ROM, archive or BIOS file on the share; the library scan and the cores parse it as root, with no sandbox, so a parser bug in a core or in the archive code is reachable from the network. `mdnsd` runs as root and parses multicast DNS from any host on UDP 5353. The published account makes the NTLMv2 and signed-session code of the kernel server reachable from any host, and any host can trigger its lockout delay (5 s in a server worker per failed login of `neptron` after 10 wrong passwords). `ksmbd.mountd` runs as root and parses the RPC of the automatic `IPC$` share from any client, before any login. `udhcpc` and its script run as root on the answers of any DHCP server on the LAN. The spec lists these and the limits.

The boot check waits, inside its deadline, until a socket listens on port 445 and `mdnsd` runs. The QEMU runs have no network card (`-nic none`), and the server listens on the loopback interface too. An image built before the network share fails this step only when the whole deadline is over, with a line that asks whether the image is older than the network share: rebuild the image.

### The 15 kHz kernel patch set

The PC kernel applies the nine patches of `D0023R/linux_kernel_15khz`, folder `linux-6.18`, at commit `d85a343379f7f54155f69ef56dc544b666635894`. All nine apply to 6.18.63 with no offset and no fuzz. Patch 09 (i915 gen9 interlace) acts only with `i915.no_ytiled_scanout=1`, which the image does not set. Every PC image, release and development, applies the set. `cargo xtask os build --arch x86_64 --no-kernel-15khz` builds the image without them, in its own output, for a comparison with the unpatched driver.

**Licence.** The set is GPL-2.0-only, the licence of the kernel (ADR 0018). At the pinned commit, the `LICENSE` file of the patch repository is the GPL version 2, and its `README.md` states GPL-2.0-only for everything except `xorg/`. The author confirmed it in issue 22 of the repository on 2026-11-15. `board/pc/LICENCES.txt` names the set, its commit and its licence in the image.

**Hashes.** `patches/linux/linux.hash` holds the SHA-256 of the kernel archive, of its licence files and of the nine patches. `patches/linux-headers/linux-headers.hash` is a link to it. With `BR2_KERNEL_HEADERS_AS_KERNEL`, the `linux-headers` package downloads the patches of `BR2_LINUX_KERNEL_PATCH` too, and Buildroot checks them against the hash file of `linux-headers` only. `BR2_DOWNLOAD_FORCE_CHECK_HASHES` makes a missing hash an error ("No hash found for 01_linux_15khz.patch", OP-111). An output that already holds the download stamp of `linux-headers` does not check its downloads again, so only a build from an empty output shows such an error.

**Change the pin.** Read `LICENSE` and the licence section of `README.md` at the new commit, and the history of any new author; a licence that does not allow distribution under GPL-2.0-only stops the change (ADR 0018). Download the nine files of `linux-6.18` at the new commit, compute their SHA-256 (`sha256sum *.patch`), and put the commit in the defconfig URLs and the values in `patches/linux/linux.hash`. A test in `xtask/src/os/build.rs` pins the commit and checks that each patch has a hash in both hash files.

**Checks after a pin change.** These need an image build and QEMU, not hardware. For the pin `d85a3433` they have not run yet.

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

On the development variant, a bench tool that needs the display (the self-test of `neptronctl`) runs only while the frontend and the launcher are stopped. Over SSH: `touch /run/neptron/hold && kill $(pidof neptron-launch)`, then for example `neptronctl selftest video --profile generic_15`. The launcher forwards SIGTERM to the frontend, waits for it and exits, and the descriptor closes. Init starts the launch script again, and the script waits while `/run/neptron/hold` exists. `rm /run/neptron/hold` starts the launcher again. The file is on a RAM file system, so a restart always starts the frontend.

### Working on the bench

The development variant lets a change reach the i3-6100T without a new image.

1. Build it once, with your public key: `cargo xtask os build --arch x86_64 --dev-key ~/.ssh/id_ed25519.pub`. Write `images/x86_64-dev/neptron-x86_64.img` to a stick (see below) and boot the PC with an Ethernet cable on a network with DHCP.
2. Find the address: ask the router, read the console, which prints `neptron: address eth0 A.B.C.D` when the address changes, or open the Network panel of Settings.
3. `cargo xtask os ssh --host A.B.C.D -- uptime` checks the connection. `os ssh` runs its command with the hot folder first on the PATH, as a login shell does, so `cargo xtask os ssh --host A.B.C.D -- neptronctl stats` runs the pushed `neptronctl`. A plain `ssh root@A.B.C.D neptronctl stats` runs no login shell and gets the image's own `neptronctl`, which can speak an older protocol; use `/state/hot/bin/neptronctl` there. `os ssh` uses your SSH agent or key and never a password; the image accepts no password. The first connection stores the host key in `~/.cache/neptron/bench_known_hosts`. The PC keeps its host key on the state partition (`/state/dropbear`), so it stays the same across reboots. A stick written again with a new image has a new state partition and a new key, so first remove the line of that host from the file.
4. Change the code, then `cargo xtask os push --host A.B.C.D`. It builds the frontend (with `control`), the launcher and `neptronctl` in the bench profile, puts them in `/state/hot/bin`, and restarts the launcher and the frontend. `--data` also replaces the data files (`/state/hot/share/neptron`) with the data of the image: the folders of `data/`, the controller database `input/` and the arcade index, which the push makes from the DAT in the Buildroot output. A warm push takes about a minute.
5. `cargo xtask os logs --host A.B.C.D` prints the log (`/state/log/neptron/current`; svlogd keeps it below 4 MiB and keeps two older files), `dmesg` and the hot folder; `--follow` follows the log. The log has the binary and data paths of each start, every mode the engine set with its read back, the guard events, the notices and the errors.
6. To go back to the image's own programs: `cargo xtask os ssh --host A.B.C.D -- touch /state/hot/DISABLED`, then restart (below). Remove the file to use the hot folder again.

The files go over the SSH connection itself (`cat > file` on the PC, and a tar stream for `--data`): Dropbear has no SFTP server, which the `scp` of current OpenSSH needs.

How the hot folder works: the launch script of the development variant takes `bin/neptron-launch`, `bin/neptron` and `share/neptron` from `/state/hot` when they are there and `/state/hot/DISABLED` is not. A hot binary goes through the same guard and validator as the image's own. The launcher holds the DRM descriptor, so a restart of the frontend alone keeps the mode on the wire. A new launcher binary needs a restart of the launcher, which `os push` does with the hold file: `touch /run/neptron/hold; kill $(pidof neptron-launch)`, wait until it is gone, `rm /run/neptron/hold`. The launcher forwards SIGTERM to the frontend, which ends its run as a quit (save RAM and settings written, exit code 0). `os push` sends SIGKILL only as a fallback, with a line that says so, to a frontend still there after 10 s. The bench profile differs from the release image: no LTO, line tables, no strip, and the `control` feature; `panic = "abort"` is the same.

The release image has none of this: no SSH server, no hot folder, no log file, no `control` feature. Its post-build fails when it finds any of them. Both variants have DHCP and the network share.

**The output now holds the development packages.** The development image of the bench PC was built in the x86\_64 release output (`--output x86_64 --dev-key …`, images in `images/x86_64-dev`), because the Docker disk had no room for a second 45 GB output. So `os push` for the PC uses the toolchain and sysroot of the `x86_64` output. That output holds Dropbear and the files of the development overlay in its target folder, so a release build from it fails its post-build check. The next release image comes from a build in an empty output (`--output x86_64-clean`, about 45 GB of Docker disk and 5 hours). The release image that passed the gate on 2026-11-13 is in `~/.cache/neptron/buildroot/images/x86_64-release-fb6dce6c` (SHA-256 `fb6dce6c965b9230e56e8abc675527302481a296ea9187fcc85014e3e0fcf050`). Do not ship it: it is older than the SIGTERM fix of the frontend, the launcher's stop timeout, the log queue and the strict format rules of the share partition.

**What QEMU proved for the bench** (the development image on a writable copy with 1 GiB free, `cargo xtask qemu --arch x86_64 --disk COPY --ssh-port 2222 --beside-launcher`, then the xtask commands with `--host 127.0.0.1 --port 2222`): SSH with the key works; the server offers only `publickey`, and a password login is denied; `os push` built, sent and restarted, and the log and `/proc/<pid>/exe` showed `/state/hot/bin/neptron-launch` and `/state/hot/bin/neptron`; `--data` gave `data /state/hot/share/neptron`; `DISABLED`, and then the removal of `/state/hot`, gave the image's own binaries again; `os logs` read the log, with the guard events and the modes; after a reboot the same host key was accepted. A warm push took 19 to 26 s, of which the build was 3 s and the rest the copy of 32 MB into the emulated guest; the first bench build took 45 s with the dependencies already in the cross volume. QEMU does not prove a real network, DHCP from a router or the serial console of the PC. The development image ran on the i3-6100T on 2026-11-13 (OP-078 to OP-084); `docs/hardware-checklist.md` holds the open checks.

**Write the image to a USB stick.** macOS: `diskutil list`, then `diskutil unmountDisk /dev/diskN` and `sudo dd if=$HOME/.cache/neptron/buildroot/images/x86_64/neptron-x86_64.img of=/dev/rdiskN bs=4m`. Linux: `sudo dd if=.../neptron-x86_64.img of=/dev/sdX bs=4M conv=fsync status=progress`. Check the device name twice: `dd` overwrites the whole disk.

## The Raspberry Pi image

`docs/spec/raspberry-pi.md` is the spec. `cargo xtask os build --arch rpi-arm64` writes `~/.cache/neptron/buildroot/images/rpi-arm64/neptron-rpi-arm64.img`, one SD card image for the Pi 3 B and the Pi 3 B+. The checklist names a Pi 3 B+; the tests of 2026-11-14 ran on a Pi 3 B. The image also holds the device tree of the Zero 2 W and its `[pi02]` lines in `config.txt`; the Zero 2 W is not tested and is not a target (no Ethernet, no analog audio). QEMU does not boot this image: its Pi machines have no `vc4` display. On 2026-11-14, on a composite TV, the picture locked in NTSC 240p, and NES, SNES and Mega Drive games held 60.054 frames per second with no missed field (`docs/spec/raspberry-pi.md`, OP-102, OP-103). `docs/hardware-checklist.md` lists the checks that are still open.

**What works.** The image boots the frontend on an HDMI display through the panel path, as on the PC, or on a TV through the composite output, with the network share, the cores, the state partition and the share partition. The composite connector `Composite-1` reports no display, so the connector choice of `display.output = "auto"` does not take it. When no HDMI display answers within 3 s (`COMPOSITE_FALLBACK_WAIT_NS`), the frontend opens `Composite-1` by name; an HDMI display that answers wins. The first run of a boot writes the output it drives to `run/output`, and every relaunch of that boot opens the same connector. `display.output = "Composite-1"` gives composite with HDMI connected. The TV standard is the setting `display.tv` (ADR 0012).

**Kernel.** The Raspberry Pi fork of Linux, branch `rpi-6.18.y`, commit `ea17ae02ca28483e508b05c7c44190c01a1ef8eb` (2026-11-13, Linux 6.18.62), as a GitHub archive whose SHA-256 is in `board/rpi/patches/linux/linux.hash` (and `linux-headers`). The configuration is `bcm2711_defconfig` with 4K pages and the fragments of `board/rpi`:

- `system`: the watchdog, and the `performance` CPU governor;
- `video`: vc4, v3d and the I2C controller of the HDMI DDC bus built in; no fbdev, no `CONFIG_FB`, no framebuffer console, no `simpledrm`, no DRM log client, no panic screen, no logo; no camera or tuner drivers;
- `console`: no `CONFIG_VT`; the serial port;
- `input`: the drivers of the PC image;
- `sound`: ALSA and ASoC built in, which vc4 needs; `snd_bcm2835` for the jack; USB audio;
- `storage`: SD host, EROFS with LZ4, ext4, exFAT, FAT, all built in; no initramfs;
- `network`: the USB Ethernet chips of the Pi 3 B and 3 B+; the SMB server as on the PC; no Wi-Fi, no Bluetooth, no CAN.

Other drivers of `bcm2711_defconfig` are modules, which `mdev` loads by their device ids. `board/rpi/check-kernel-config.sh` reads the final configuration in the post-build step and fails the build when a mode option is on, when the SMB server is not built in, or when vc4, the I2C controller of the HDMI DDC bus, the watchdog, or a driver of the root on the SD card (`MMC_BLOCK`, the sdhost driver `MMC_BCM2835`, `EROFS_FS`, `EROFS_FS_ZIP`) is not built in. The device trees (`bcm2710-rpi-3-b`, `bcm2710-rpi-3-b-plus`, `bcm2710-rpi-zero-2-w`) and the overlays (`vc4-kms-v3d`, and `disable-bt` for the development variant) come from this kernel, not from the firmware tarball.

**No kernel patch.** The 15 kHz patch set of the PC image does not reach the Pi kernel. The Pi defconfig sets no `BR2_LINUX_KERNEL_PATCH`, and its global patch directory is `board/rpi/patches`, which holds only the hash files. `board/rpi/check-kernel-patches.sh` fails the build when `BR2_LINUX_KERNEL_PATCH` is not empty, when `BR2_GLOBAL_PATCH_DIR` is not exactly `$(BR2_EXTERNAL_NEPTRON_PATH)/board/rpi/patches` (the PC uses `os/patches`, and a second directory would add its patches), or when that directory holds any file other than `linux/linux.hash` and `linux-headers/linux-headers.hash`. The set changes the DRM core and the AMD and Intel drivers of the PC, and the Pi needs nothing from it.

**Firmware and boot.** The Buildroot package `rpi-firmware` at commit `063bcab6` gives `bootcode.bin`, `start.elf` and `fixup.dat` (the Pi 0 to 3 files). `config.txt` is the file of the spec, with the reason of each key in it: 64-bit mode, the kernel `Image`, no splash, no `video=` from the firmware (`disable_fw_kms_setup=1`), `dtparam=audio=on` (before the overlay line, or it sets a parameter of the overlay), `dtoverlay=vc4-kms-v3d,composite`, `core_freq_fixed=1`, and `enable_tvout=1` for the Zero 2 W. The development variant appends `enable_uart=1` and `dtoverlay=disable-bt`: the PL011 UART goes to GPIO 14 and 15, Bluetooth is off, and the kernel log goes there (115200 baud, 3.3 V). `cmdline.txt` of the release image is `root=PARTUUID=<signature>-02 rootfstype=erofs ro rootwait quiet console=serial0,115200 panic=10 usbcore.old_scheme_first=1`. The file permits no comment: the PC section, "Boot", gives the reason of `usbcore.old_scheme_first=1`. The release image has no serial console: the firmware leaves the UART off. The development variant has no `quiet` (`board/rpi/cmdline.sh` writes the file), so its serial console shows the whole kernel log from the start.

**Disk layout.** MBR partition table. The disk signatures are `4e505052` (release) and `4e505044` (development).

| Partition | Filesystem | Start | Size | Contents |
| --- | --- | --- | --- | --- |
| 1 boot | FAT32, label `NEPTRONBOOT`, type `0x0c` | 4 MiB | 128 MiB | `bootcode.bin`, `start.elf`, `fixup.dat`, `config.txt`, `cmdline.txt`, `Image`, the three device trees, `overlays/`, `LICENCES.txt`, `legal/` |
| 2 root | EROFS (LZ4HC), read-only | 132 MiB | 384 MiB | As on the PC |
| 3 state | ext4, label `neptron-state` | 516 MiB | 256 MiB | As on the PC |
| 4 share | exFAT, label `NEPTRON` | 772 MiB | The rest of the card, made at the first boot | As on the PC |

Every partition starts on a 4 MiB boundary, the erase block of common SD cards. The storage script `S06neptron-storage` needs no change for the SD card: it finds the disk from the device of `/` and the partitions by their `partition` number in sysfs, never by a name it builds, and it reads partition 4 of `sfdisk --dump` with a pattern that matches `/dev/mmcblk0p4` as it matches `/dev/sda4`. The xtask test `the_storage_script_finds_mmcblk0p3_and_reads_mmcblk0p4` runs those two functions of the script on `mmcblk0` names. Not verified on a card: `sfdisk --append` and `addpart` on `/dev/mmcblk0`, and the time `mkfs.exfat` takes on an SD card.

**Packages.** As on the PC, without LLVM: Mesa with the Gallium `vc4` driver (OpenGL ES 2 on the VideoCore IV) and softpipe, EGL and GBM; SDL3 with KMS/DRM and OpenGL ES; alsa-lib and `amixer`; libdrm; the four cores; the frontend, the launcher and `neptronctl`; `ksmbd-tools` and `mdnsd`; e2fsprogs, exfatprogs and util-linux. The init table, the mixer script and the BusyBox options are those of the PC image (`board/pc/rootfs-overlay`, `board/pc/busybox.config`); its QEMU script does nothing on a Pi. No Wi-Fi or Bluetooth firmware, and no test tool of the QEMU display.

A Pi 4 needs at least these files that this image does not hold: `start4.elf` and `fixup4.dat`, its device tree `bcm2711-rpi-4-b.dtb`, the `v3d` driver of Mesa, and `overlays/overlay_map.dtb`, through which the firmware loads `vc4-kms-v3d-pi4` for the line `dtoverlay=vc4-kms-v3d`.

**Shut down and reboot.** The System panel works as on the PC: the frontend ends with exit code 81 (shut down) or 80 (reboot), the launcher (`--power`) runs BusyBox `poweroff` or `reboot`, and init runs the stop script, `rcK` and `umount -a -r`, so the state and share partitions unmount clean. A Pi has no ACPI and no power switch. On a power-off, the watchdog driver (`CONFIG_BCM2835_WDT`, which the kernel check requires) tells the firmware to halt: the picture goes off, the green ACT LED stops, the red power LED stays on, and the board still draws a little power. Only a power cycle (unplug the supply and plug it in again) starts it again. A reboot resets the board through the same watchdog.

**Variants.**

| Command | Output volume and images | What differs |
| --- | --- | --- |
| `cargo xtask os build --arch rpi-arm64` | `rpi-arm64` | The release image |
| `cargo xtask os build --arch rpi-arm64 --dev-key ~/.ssh/bench.pub` | `rpi-arm64-dev` | Dropbear with the one key, the hot folder, the log, the `control` feature (the development overlay of the PC image, and `board/rpi/dev-overlay` on top), and the serial console |

The release post-build step fails when the image holds a file of either development overlay or an SSH server. The bench commands work with a Pi as with the PC: `cargo xtask os ssh --host A.B.C.D -- uptime`, `cargo xtask os logs --host A.B.C.D`, and `cargo xtask os push --arch rpi-arm64 --host A.B.C.D`. The push builds the frontend, the launcher and `neptronctl` with the Buildroot toolchain and the sysroot of the `rpi-arm64-dev` output (`--output NAME` names another one), in its own cross volume `neptron-cross-rpi-arm64-dev`, so it links against the glibc of the image on the card and never against the glibc of the QEMU aarch64 output. Build the development image once before the first push.

**Write the image to an SD card from macOS.**

1. Insert the card, then run `diskutil list`. Find the card by its size and its "external, physical" label, for example `/dev/disk4`.
2. Check the device name twice. `dd` overwrites the whole device, and a wrong number destroys another disk.
3. `diskutil unmountDisk /dev/disk4`
4. `sudo dd if=$HOME/.cache/neptron/buildroot/images/rpi-arm64/neptron-rpi-arm64.img of=/dev/rdisk4 bs=4m`
5. `sync`, then `diskutil eject /dev/disk4`.

Use `rdisk4` (the raw device), not `disk4`: it is many times faster. For the development image, use `images/rpi-arm64-dev/neptron-rpi-arm64.img`. On Linux: `sudo dd if=.../neptron-rpi-arm64.img of=/dev/mmcblkN bs=4M conv=fsync status=progress`.

**First boot.** Put the card in the Pi, connect an HDMI display or a TV on the composite jack, a USB gamepad and Ethernet, then the power supply (2.5 A). The firmware loads the kernel; the storage script makes partition 4 from the free space of the card, formats it exFAT and mounts it at `/share`; the frontend shows the game list on the HDMI display, or on composite in NTSC when no HDMI display answers within 3 s. A flat panel, a TV and a monitor that gives no EDID start with no prompt and no notice (ADR 0013); only a monitor preset named in the settings for a display that is not known opens the boot prompt, which reverts 15 s after its picture settles. Copy games to `roms/<system>` over the network share (`\\NEPTRON\share`) or with the card in another computer. The development variant prints the address on the serial console at each change.

**What was verified** (on an Apple Silicon Mac, 2026-11-13, build only):

- The first build of the release image in an empty output took 5,549 s (1 h 32 min, 4 jobs: the toolchain, the host tools, the kernel with its modules, Mesa without LLVM, the Rust programs). Buildroot checked the kernel archive against its hash. The kernel check and the patch check passed in the post-build step; the final kernel configuration has `CONFIG_VT`, `CONFIG_FB` and `CONFIG_DRM_PANIC` off and vc4, v3d, `snd_bcm2835`, the watchdog, EROFS, exFAT and the SMB server built in.
- The kernel is `6.18.62-v8`; `Image` is 27,982,336 bytes. The SD card image is 809,500,672 bytes (772 MiB), 48 MB with gzip. `sfdisk --dump` of the image shows the signature `4e505052` and the three partitions at sectors 8192, 270336 and 1056768; the boot partition is FAT32 and holds the files of the table above; the overlay `vc4-kms-v3d.dtbo` has the `composite` fragment, and the device trees have their symbols for the overlays.
- The development variant then took 3,608 s in its own empty output, with the compiler cache of the first build. Its `config.txt` ends with `enable_uart=1` and `dtoverlay=disable-bt`, `overlays/` holds `disable-bt.dtbo` too, its root PARTUUID is `4e505044-02`, and the image holds Dropbear, the key and the development launch script.
- The two outputs take 18.4 GB each on the Docker disk.

This build check boots nothing, and QEMU does not boot this image. The boots on a Pi 3 B are in `docs/spec/raspberry-pi.md`. The sizes of the root with FinalBurn Neo are in "What was verified" below.

**Licences.** `board/rpi/LICENCES.txt` goes on the boot partition and into `/usr/share/licenses`. The firmware files are not open source: `LICENCE.broadcom` allows redistribution in binary form, without modification, only for use with a Raspberry Pi, with its copyright notice and disclaimer. Buildroot labels the package `BSD-3-Clause`, which is wrong for these terms; `LICENCES.txt` says so, gives the terms, and reproduces the notice and the disclaimer, and `legal/LICENCE.broadcom` holds the file. The image may be distributed free of charge, unchanged, for use on a Pi, with `LICENCES.txt`, `legal/` and the `legal-info` sources. It may not be sold (the non-commercial cores, OP-054).

## The aarch64 test image

- BusyBox init, with `mdev` for device nodes. There is no udev daemon and no systemd.
- A root shell on the serial console, with no login.
- An ext4 root filesystem, 256 MB, on a virtio disk. QEMU runs it in snapshot mode, so a run never changes the image.
- libdrm with its test programs (`modetest`), `neptron-vkms-probe` and `neptron-capture`.
- Mesa with the softpipe software renderer: EGL, OpenGL ES and GBM, and `kmscube`.
- SDL3 with the KMS/DRM video driver and OpenGL ES, without libudev, and alsa-lib.
- The libretro cores FCEUmm, Snes9x, Genesis Plus GX and FinalBurn Neo in `/usr/lib/libretro`. The first three have the library names of `data/cores`.
- The frontend, the launcher and `neptronctl`. Init starts the launcher only for a boot check (`launch-for-check`), so the other checks get the display.
- The network share, as in the PC image: `ksmbd`, `ksmbd-tools`, `mdnsd`, and the init scripts. `/share` is a RAM folder there.

The build also installs the Rust toolchain of Buildroot (`host-rustc`, the prebuilt `rust-bin`, with the standard library for the target) in the output, not in the image. `cargo xtask gles-compare` uses it with the Buildroot C toolchain as the linker to build Rust programs for the guest. This toolchain (Rust 1.97.1 in Buildroot 2026.08.1) is not the one that `rust-toolchain.toml` pins for the host (1.97.0); the two can drift apart when Buildroot is bumped.

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
```

`os build` builds the development container from `Dockerfile`, then runs Buildroot in it with 4 jobs, 4 CPUs and 6 GB of memory. It runs `make legal-info` before the build, because the image carries the licence texts and the manifest. A machine-wide lock allows one Buildroot build at a time. Buildroot does not see a change to a kernel fragment by itself: give the `-reconfigure` target as above.

Buildroot does not see a change to the source of a package that builds from this tree (`neptron`, `neptron-launch`, `neptronctl`, `neptron-capture`, `neptron-vkms-probe`) either. So `os build` adds `<package>-reconfigure` for each of them that the configuration turns on, before the targets after `--`; do not name them. `-reconfigure`, not `-rebuild`: the configure step runs `cargo vendor`, which a `Cargo.lock` with a new crate needs. A `-rebuild` or a `-reconfigure` of a local package copies its source again with `rsync -au`. That copy keeps the file times of the source and skips a file whose copy in the build folder is newer. So a file of another worktree, or a file that changed before the last build, can keep its old copy or get a time older than the build output, and Cargo then takes the old output as fresh. The rsync hooks in `package/neptron/local-source.mk` remove the old copy before the rsync and give each copied file the current time after it. The Cargo build output, the vendored crates and `.cargo/` stay, so only the workspace crates compile again.

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

Method. The image's own launcher and frontend ran by hand after the bench handover (`/run/neptron/hold`), with `--cores` on a folder where the library name of a known core points to `fbneo_libretro.so`, and the sets in a subfolder `arcade` of that system (FinalBurn Neo chooses its console mode from the name of the parent folder). The vertical game showed on its side, with no rotation; that does not change the cost. `ftrace` recorded `sched_switch` of the frontend threads, `drm_vblank_event` and `vc4_submit_cl_ioctl`. "Frame thread" is the on-CPU time of the frame thread in each field: core, upload, render and the audio resample. "Core bound" is the time from the flip event to the first GPU submit of the field: input, `retro_run` and the texture upload, an upper bound of the core time. The play rows follow a coin, a start and fire held; the picture was not checked.

| Run | Fields | Missed fields | Underruns | Frame thread p50 / p99 / max | Core bound p50 / p99 / max |
| --- | --- | --- | --- | --- | --- |
| `ddonpach` attract, 180 s | 10,810 | 0 | 0 | 7.31 / 8.64 / 8.99 ms | 5.83 / 7.16 / 7.50 ms |
| `ddonpach` play, 120 s | 7,207 | 0 | 0 | 7.46 / 8.78 / 9.38 ms | 5.98 / 7.28 / 7.74 ms |
| `mslugx` attract, 180 s | 10,810 | 0 | 0 | 8.88 / 10.83 / 11.33 ms | 7.42 / 9.37 / 9.79 ms |
| `mslugx` play, 120 s | 7,207 | 0 | 0 | 9.58 / 11.16 / 11.83 ms | 8.13 / 9.69 / 10.28 ms |

Both games run at full speed with margin: the worst field used 11.83 ms of 16.65 ms. The frame thread used 44 to 57 % of one core. These rows come from a build with the 7z support in. The shipped build (`INCLUDE_7Z_SUPPORT=0`) was checked again for 60 s of each attract mode: no missed field and no underrun, frame thread p99 8.15 ms (`ddonpach`) and 10.79 ms (`mslugx`), core bound p99 6.67 ms and 9.33 ms.

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
- The three first cores: the libraries are 4.3 MiB (`fceumm_libretro.so`), 2.7 MiB (`snes9x_libretro.so`) and 12.0 MiB (`genesis_plus_gx_libretro.so`).
- FinalBurn Neo (2026-11-16): the build step of `libretro-fbneo` took 763 s with 4 jobs from cold on the Pi output. `fbneo_libretro.so` after the strip is 82,563,440 bytes on aarch64 and 84,950,640 bytes on x86_64. With it, the target tree and the root are: Pi development image 164.1 MB, EROFS 72.5 MB in the 384 MiB partition; QEMU aarch64 image 142.9 MB, ext4 of 256 MiB with 97.5 MiB free; PC development image 443.7 MB, EROFS 205.9 MB in the 384 MiB partition, ext4 of 512 MiB with 65 MiB free.

On the same Mac, on 2026-11-12 and 2026-11-13, with the PC image and the aarch64 image that hold the frontend:

- `cargo xtask os boot-check --arch x86_64` passes with BIOS (SeaBIOS) and with UEFI (EDK II): the firmware boots the USB image from a USB stick on xHCI through GRUB, the kernel mounts the EROFS root by PARTUUID, init starts the launcher, and the frontend shows the game list. The `vkms` writeback picture equals `tests/golden/boot-game-list.ppm` byte for byte. Under CPU emulation (TCG, single thread for an x86 guest on an Arm host) the first frame of the frontend came at a guest uptime of 12.2 to 46.0 s over eight runs. On the image `fb6dce6c…` the first frame came at 15.09 s guest uptime with BIOS (19.06 s after QEMU start) and 16.07 s with UEFI (19.64 s). These are emulated times; they vary with the load of the machine, and they say nothing about the i3-6100T. The report calls the end of the wait "game list on screen".
- `cargo xtask os boot-check --arch aarch64` passes with the hypervisor: first frame at a guest uptime of 0.55 s, and the same picture byte for byte. These are times of a virtual machine, not hardware times.
- The picture is the 320x240 menu in the 1280x240 super-resolution mode that the safe start profile `generic_15` gives on `vkms` (26.027 MHz, 15.66 kHz, 60 Hz): the empty home screen, with nothing over it. `vkms` reports no EDID, so `display.monitor = auto` gives `generic_15`, which is also the safe default: the guard opens no boot prompt and shows no notice (ADR 0013). The check waits for the home screen (`neptron-capture --screen home`), which has no timer. The golden image holds this picture (captured on 2026-11-16 on x86_64 with SeaBIOS; EDK II and aarch64 give the same bytes).
- The share partition, on a writable copy of the development image of 2026-11-13 (`08ef1064…`) with 1 GiB of free space after the state partition (`cargo xtask qemu --arch x86_64 --disk COPY --script ...`), the steps in order on one copy:
  - A, a fresh stick: partition 4 made, found empty, formatted exFAT, mounted; the marker `/state/share-pending` removed; a file written.
  - B, the second boot: mounted, not formatted; the file is there.
  - C, the image written again with `dd` over the start of the copy (no partition 4 in the table, the old share on the disk): partition 4 made again at the same start, the old exFAT found and mounted, not formatted; the file is there.
  - D, a power cut after `sfdisk` (marker written by hand, first 8 MiB of partition 4 zeroed): the next boot formatted it once and mounted it; the boot after that did not format it, and a file written in between is there.
  - E, partition 4 zeroed and no marker: left untouched (the reason "holds no file system that blkid finds and has no /state/share-pending, so it is left untouched"), `/share` a RAM folder.
  - F, a share with files whose main boot sector is zeroed, the backup boot region intact, no marker: left untouched, `/share` a RAM folder.
  - G, the same share with the marker written by hand: left untouched ("it holds an exFAT backup boot sector, so it is a damaged share").
  - Not run: a read error of a real stick (QEMU gives none), and the scan that finds an old share at another start (no image of another layout exists yet).
- A first boot on a real 60 GB stick, written over an older NEPTRON image with a share at the same start, once gave `share = "ram"` with the reason "no share partition": the table on the disk had partition 4, the kernel did not, and the script of that time wrote no line about it. QEMU does not show this. The likely cause, not proven (the stick gave no log): in util-linux 2.41.5, `sfdisk` can exit with 1 after the table is written only when `fsync` or `close` of the disk fails, and a USB stick that refuses SYNCHRONIZE CACHE gives such an `fsync` error. So the script trusts no exit status, reads the table back from the device, and logs every step to syslog and to `/state/log/storage.log`. On 2026-11-13 a copy with 128 MiB free and a copy of 60 GB (120127488 sectors, the size of that stick), each written again over a share with files, made partition 4 around the old share and mounted it with the files. There `sfdisk` exits with 0: it prints "Partition #4 contains a exfat signature", does not ask (its standard input is not a terminal) and wipes nothing; the failed table read of the kernel (`EBUSY`) does not change its exit status. `cargo xtask os share-check` passes (fresh, rewritten, again) with the current script.
- After `kill -ABRT` of the frontend on the aarch64 image, while the launcher lives, the CRTC stays active with the 1280x240 mode and the plane keeps the framebuffer of the dead frontend; the launcher starts a new frontend 250 ms later (its crash backoff), and the mode is the same.
- The 15 kHz patch set applies in Buildroot, and the patched kernel builds and boots in both boot checks.
- `kms-check`, `gles-compare` and `core-trace` pass on the aarch64 image that holds the frontend; `os app-check` passes for aarch64 and x86\_64.

On the same Mac, on 2026-11-13, the aarch64 image with the network share (an incremental build of 178 s that added `ksmbd-tools`, GLib, libnl, `mdnsd` and the kernel server):

- `cargo xtask os boot-check --arch aarch64` passes, with its step that waits for port 445 and `mdnsd`; the frontend started at a guest uptime of 0.39 s.
- A guest script (`cargo xtask qemu --arch aarch64 --script`): `ksmbd.mountd` runs and port 445 listens; `/etc/network/interfaces` holds the loopback only; `/etc/passwd` has `neptron:x:1000:1000:NEPTRON network share account:/:/bin/false` and `/etc/shadow` has `neptron:*:::::::`; `/share` (a RAM folder) is owned by uid 65534 with `nosuid,nodev,noexec`; `ksmbd.control --shutdown` stops the server; `ksmbd.mountd -n -v` with the image's files logs `New user 'neptron'`, `New user 'nobody'`, `New share 'share'` and no "Invalid pwddb entry", so the user database parses.
- Not proved: a login over the network, as guest or as `neptron`. The image has no SMB client, and the QEMU runs have no network card for port 445. See `docs/hardware-checklist.md`.

In CI, on every push of `main` (first green run 2026-11-14): `kms-check`, `gles-compare`, `os selftest-check`, `core-trace` and `os share-check` run on the x86\_64 image under KVM (`docs/ci.md`).

Not verified:

- On the i3-6100T: the open checks of `docs/hardware-checklist.md` (the DP-to-VGA bridge, the boot time and the rest). The development image ran there on 2026-11-13: a game played, with audio and the SMB share.
- The aarch64 timing checks in CI (`kms-check`, `selftest-check`, `core-trace`): the arm64 runners have no KVM (OP-104).
- The local images come from an output that was built up step by step (the defconfig changed several times). Buildroot does not rebuild a package when its options change; `mesa3d` and `util-linux` needed a `-dirclean` by hand. No release image of the PC has a recorded local build in an empty output; it takes about 5 hours (`cargo xtask os build --arch x86_64 --output x86_64-clean`). One such clean build is needed before an image goes to the bench.

Build time, PC image, 4 jobs: the first full build with LLVM, clang and the 15 kHz kernel took 17,419 s (4 h 50 min; host LLVM and clang take most of it). A rebuild of the frontend and the launcher from clean (fat LTO), with the image, takes about 5 min (309 s). The kernel `bzImage` is 14.2 MiB. In the target directory, LLVM is 74 MiB and the unused clang libraries 104 MiB (see the open points). The USB image file is 739,246,080 bytes (705 MiB); most of it is the free space of the fixed partitions, so it compresses well.

Build time and size, aarch64, 4 jobs: a kernel reconfigure, or a change to the overlay or to a package of this tree, takes about 1 min. The kernel `Image` is 13.3 MB. The guest uses about 100 MB of memory after boot. `docs/ci.md` has the times and sizes of cold builds on the CI runners.

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

The files of this tree are `MIT OR Apache-2.0`, as the rest of NEPTRON. Buildroot is GPL-2.0-or-later. It runs as a build tool and is not part of the repository. The packages in the images keep their own licences, which `make legal-info` lists. The network share adds the kernel server `ksmbd` (GPL-2.0), `ksmbd-tools` (GPL-2.0-or-later), GLib and libnl (LGPL-2.1-or-later), PCRE2 (BSD-3-Clause), libffi (MIT) and `mdnsd` (BSD-3-Clause); `board/pc/LICENCES.txt` lists them. The PC image carries the manifest and the licence texts on its boot partition (`legal/`) and in `/usr/share/licenses`, with the notes of `board/pc/LICENCES.txt`. The source tarballs of the GPL and LGPL packages are in `legal-info/sources` of the Buildroot output; they must be published with an image. The frontend and launcher packages do not copy their source there: it is this repository.

The 15 kHz patch set is GPL-2.0-only (see "The 15 kHz kernel patch set" and ADR 0018). The `make legal-info` output holds the kernel source in `sources/linux-6.18.63`: the kernel archive, the nine patches and the file `series` with their order. `board/pc/LICENCES.txt` and `board/rpi/LICENCES.txt` carry the same written offer (GPL-2.0 section 3(b)) for the source of every GPL and LGPL package of the image, valid for at least three years from the last date on which the project gives out the image, with an issue in the public repository as the contact (ADR 0018). The rule for the source: the project gives out the source together with the image, from the same place; the written offer applies in every case, also for an image that reached somebody without its source. The binary-only Pi firmware is not covered. The QEMU aarch64 test image is not given to anybody and has no offer. Today `os build` leaves the sources in its Docker output volume, and CI keeps them as an artifact for 7 days, beside the image. Nothing yet publishes a lasting source bundle.

**Release requirement (M14).** The release publishes the `legal-info` sources archive of each image, PC and Raspberry Pi, beside that image, in the same place and for as long as the image. The project keeps the sources of every image it gives out for at least three years after the last time it gives it out, so it can answer the written offer.

**Release requirement (M14): name the image.** The image carries the NEPTRON commit and the build date in a file on its boot partition, so a person with only the image can name it in a request for the source. Today the image holds neither.

No firmware file is in the image. For a later decision: the i915 files of `linux-firmware` are under `LICENSE.i915` (Intel: redistribution in binary form allowed, no modification and no reverse engineering), and the amdgpu files under `LICENSE.amdgpu` (AMD: terms of the same kind). Neither is an open source licence.
