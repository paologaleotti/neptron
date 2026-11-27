# xtask

Build, verification and worktree automation for the NEPTRON workspace. Run it as `cargo xtask <command>`. All automation lives here, not in shell scripts.

## Commands

### Workspace

- `cargo xtask verify --tier <N>`: runs tier N of the verification ladder and every tier below it.
  - Tier 0: `fmt`, `rules`, `cargo deny check licenses bans sources`, `clippy` with warnings denied.
  - Tier 1: `cargo nextest run --workspace --no-run` (timed as build time), `cargo nextest run --workspace --no-tests=warn` and `cargo test --workspace --doc`.
  - Tier 2: `cargo nextest run -p neptron --run-ignored only`: the frontend on the fakes of `neptron-testkit` with the built test core. It runs the end-to-end runs (game list, launch, mode switches, quick menu, quit), the blind recovery, a stalled display and the UI crawler. It needs no display server, no QEMU and no sound device; its budget is 5 minutes, and it takes about 10 s after the tier 1 build. These are ignored tests of the app, so tier 1 does not run them.
  - Every step runs, also after a failure. The command always writes `target/verify/report.json`, also when it cannot start the steps, and prints a summary with the time of each step, the build time and the tier budgets. A run over budget is reported, not failed. It exits non-zero when a step fails.
- `cargo xtask check [cargo args]`: `cargo check` under the machine-load rules.
- `cargo xtask test [nextest args]`: `cargo nextest run` under the machine-load rules.
- `cargo xtask clippy [cargo args]`: `cargo clippy --all-targets` under the machine-load rules, so that it checks test code as `verify` does.
- `cargo xtask worktree new <task>`: makes `.worktrees/<task>` in the main checkout, on branch `mvp/<task>` from `main`. Its `target/` is an APFS clone (`cp -cRp`) of the `target/` of the main checkout, so dependencies do not compile again (run rule 12). If the clone fails, the command removes the new worktree and branch again.
- `cargo xtask worktree cache-check [--from <checkout>]`: proves rule 12. It builds the source checkout (default: the main checkout) to warm it, makes a temporary worktree with a cloned `target/`, repeats the test build and clippy there, and counts the artifacts that Cargo compiles again. It fails when a dependency compiles again. It deletes the temporary worktree at the end, also on failure, and removes clones that an interrupted check left.
- Both worktree commands refuse to run when Cargo builds the checkout into a directory other than `<checkout>/target` (for example with `CARGO_TARGET_DIR`), because the clone would then not be the build cache.
- `cargo xtask rules`: checks the crate boundaries (see "Rule checker").

### OS images and QEMU

`os/README.md` describes the images and how a QEMU run works.

- `cargo xtask os build [--arch aarch64|x86_64|rpi-arm64] [--jobs N] [--dev-key FILE] [--no-kernel-15khz] [--output NAME] [-- make targets...]`: builds the development container from `Dockerfile` (tagged `neptron-dev:<hash of the Dockerfile>`), then runs Buildroot in it. The steps: the defconfig of the image (`aarch64`: the QEMU test image; `x86_64`: the PC image, which QEMU also boots; `rpi-arm64`: the Raspberry Pi 3 image, which QEMU does not boot), the merge of the variant fragments, the rebuild targets of the NEPTRON packages, the extra make targets in order, `make legal-info` (the image carries the licence manifest, so it comes before the build), the full build, and a copy of the images to `~/.cache/neptron/buildroot/images/<variant>`.
  - The checkout is mounted read-only at `/src/repo`: the frontend and launcher packages build its workspace.
  - The image names its source. `os build` reads the commit (`git rev-parse HEAD`, with `-modified` when `git status --porcelain` shows a change), the commit date in UTC (`%ct`, not the time of the build, so two builds of one commit give the same files) and the release tag that names exactly that commit (`git describe --tags --exact-match --match 'v[0-9]*'`, else `none`), and passes them to every container step as `NEPTRON_SOURCE_COMMIT`, `NEPTRON_SOURCE_DATE` and `NEPTRON_SOURCE_RELEASE`. A value that git cannot give is `unknown`. `board/common/install-licences.sh` writes them to `/etc/neptron-release` and to the head of `LICENCES.txt` (in the image and on the boot partition), with the variant (`release` or `development`).
  - Every `os build` adds `<package>-reconfigure` for each package that builds from this tree and that the configuration turns on (`neptron`, `neptron-launch`, `neptronctl`, `neptron-capture`, `neptron-vkms-probe`; the Pi image has only the first three), before the extra targets. An extra target that is also a rebuild target runs once. `os/README.md` ("Commands") gives the reasons and the rsync hooks.
  - `--dev-key FILE` builds the development variant of the PC image or of the Pi image: an SSH server that accepts only that public key, and the `control` feature of the frontend. DHCP is in every image. The file must hold one public key and lie outside the checkout.
  - The PC image applies the pinned 15 kHz kernel patch set (GPL-2.0-only, ADR 0018). `--no-kernel-15khz` leaves it out, for a comparison with the unpatched drivers. It is for the PC image only; the Pi image takes no kernel patch.
  - Each variant has its own output volume (`neptron-br-out-x86_64-dev`, `-no15khz`, `-dev-no15khz`) and images directory, so nothing of a variant reaches the release image. `--output NAME` names the output volume (`neptron-br-out-NAME`) in place of the variant name, for example for a build in an empty output. The images directory is NAME with the variant suffixes, so `--output x86_64 --dev-key FILE` builds the development variant in the release output and writes its images to `images/x86_64-dev`. That release output then holds the development packages, and the post-build check refuses a release image from it until a clean output is built.
- `cargo xtask qemu [--arch A] [--images DIR] [--timeout SECS] [--share DIR] [--disk FILE] [--ssh-port PORT] [--beside-launcher] [--script FILE | -- command...]`: boots the QEMU image of the target. This is the base of tier T3.
  - With a command or a script it runs it in the guest, prints its output and exits with its exit code. Without one it gives the serial console to the terminal.
  - `--disk FILE` (x86\_64 only): the BIOS boots that disk image as a USB stick, and the writes of the guest stay in the file, so a later run sees them. Give it a copy of the image, never the build output.
  - `--ssh-port PORT` gives the guest an emulated Ethernet card (e1000e) with QEMU user networking and forwards `127.0.0.1:PORT` on the host to port 22 of the guest, for the development variant. A check can ask for the same card with no forwarded port and `restrict=on` (the `nic` field of the QEMU arguments): QEMU's DHCP server still gives 10.0.2.15, and the guest has no route out.
  - `--beside-launcher` runs the command through the boot check entry, beside the launcher and the frontend, instead of before them.

### Tier 3 and tier 4 checks

The guest builds of these commands run in the development container with the Rust and C toolchains of the Buildroot output (`host-rustc` in the defconfig), so they need a built image (`os build`). They hold the Buildroot build lock for the build and the QEMU lock for the run.

- `cargo xtask os boot-check [--arch A] [--firmware bios|uefi] [--timeout SECS] [--images DIR] [--bless]`: the tier T3 gate of the image. Default timeout 900 s.
  - Boot. On `x86_64` the firmware (SeaBIOS, or EDK II with `--firmware uefi`) boots the USB disk image `neptron-x86_64.img` from a USB stick on an xHCI controller, through GRUB, as a PC does. On `aarch64` QEMU loads the kernel of the test image. In the guest, init starts the launcher and the frontend as on a PC, and the boot check script (firmware config entry `opt/neptron/check`) runs beside them. It starts with the launcher, before the frontend, so it first waits for the control socket and the storage status. One deadline (the timeout less 30 s) covers every wait of the script, so a late hang still leaves time for the dump.
  - Network share. It waits until the SMB server listens on port 445 (a socket in `/proc/net/tcp` or `/proc/net/tcp6`) and `mdnsd` runs. The guest has no network card, and the server listens on the loopback interface too. An image built before the network share fails this step at the end of the deadline with a line that says to rebuild it.
  - Storage. It checks `/run/neptron/storage.toml` against `expected_storage`. On `x86_64`: a state and a share partition, because the check boots a copy of the disk with 128 MiB free after the state partition (`SHARE_ROOM_BYTES`), so the first boot makes and formats the share partition as on a real stick. On `aarch64`: two RAM folders, because the root file system boots alone. For a RAM folder it dismisses the Warning notice with `neptronctl input`, after a check that the frontend serves `input_send` (the `control` feature, which the aarch64 test image has). Another storage, a missing notice or no `input_send` fails the check with a clear line.
  - CPU governor. Every CPU that has `/sys/devices/system/cpu/cpu*/cpufreq/scaling_governor` must run `performance`, else the check fails with a line that names the file and its governor. A QEMU guest has no cpufreq driver and no such file, so this step acts only when a machine with a cpufreq driver runs the script.
  - USB. On `x86_64`, `/sys/module/usbcore/parameters/old_scheme_first` (`USB_OLD_SCHEME_FIRST`) must be `Y`, the value that `usbcore.old_scheme_first=1` in `grub.cfg` gives, else the check fails with a line that names the value. A missing file fails too: the PC kernel has USB built in. The `aarch64` guest gets the kernel command line of QEMU, not of an image, so it skips this step.
  - Picture. `neptron-capture` waits on the control socket for the first frame and for the screen `home`. `vkms` reports no EDID, so `display.monitor = auto` gives `generic_15`, which is also the safe default, and the guard opens no boot prompt and shows no notice (ADR 0013). The home screen of the game list has no timer. Then the tool stops the frontend and the launcher, takes a copy of the launcher's DRM descriptor (`pidfd_getfd`, so it is DRM master), routes the `vkms` writeback connector to the CRTC, sets the console log level of the kernel to 1, and prints the picture in base64, with its size and CRC-32 on the end marker.
  - The kernel log and the picture share the serial console, and a kernel message can land inside the picture (an RCU stall on a starved CI runner did). So the command skips a line inside the picture that is not base64, and refuses the picture when its size or CRC-32 does not match the end marker. An end marker with no size and CRC (an older image) gets the strict rule: every line must be base64.
  - The command compares the picture with `os/tests/golden/boot-game-list.ppm` byte for byte with the comparison of `cargo xtask golden`. It writes `<label>.ppm`, `<label>.png`, the console log and a report with the boot times to `target/os/boot-check`; for a difference it also writes the expected, actual and diff PNG files there. The times are from the start of QEMU and the guest uptime; under CPU emulation they are emulated times, not hardware times. `--bless` writes the picture as the golden image; look at it first (rule 11).
- `cargo xtask os share-check [--arch x86_64] [--timeout SECS] [--images DIR] [--case rewrite|offer|retype|reused|address]...`: the tier T3 check of the share partition and of the address file. Each case boots its own copy of `neptron-x86_64.img` with 128 MiB free after the state partition, one boot after the other, through `cargo xtask qemu --disk`, with the writes kept in the copy (`target/os/share-check/<case>/disk.img`). With no `--case` it runs all five. Default timeout 300 s for each boot.
  - `rewrite`, a stick that is written again. `fresh`: the first boot makes, formats and mounts partition 4, and the guest writes `share-check.txt` to the share. `rewritten`: the host first writes the image again over the start of the copy, as `dd` does to a used stick (a new table with no partition 4, a new state partition, the old share at the planned start); the boot must make partition 4 around the old share and mount it with no format, and the file must be there. `again`: a normal boot keeps the file and makes nothing.
  - `offer`: the host writes an MBR entry 4 of type 7 at the planned start over the empty free space, with no marker, and an old ext superblock of block group 3 inside partition 4 (the offer must still appear, so the field read of the checks runs in the guest). `offered`: `/share` is a RAM folder and `storage.toml` offers the format (`share_fix` with the device, start and size of partition 4); the guest writes `/state/share-format-request` from the offer as the frontend does. `formatted`: the script removes the request, formats and mounts partition 4, and says `share_format = "done"`; the guest writes the file. `kept`: a normal boot, no format; the guest plants a request for partition 4, which now holds exFAT. `refused`: no format, the request is removed, `share_format = "refused"` with the reason, and the file is there.
  - `retype`: the same entry with type 0x83. `retype-offered` writes the request, `retyped` must set type 7 with `sfdisk --part-type`, format and mount, and `retype-again` must boot the disk whose MBR sfdisk wrote and mount the share with no format.
  - `reused`: the host writes an old ext superblock (block group 7) into the free space after the image, as a card that held another system keeps it. `fresh` must make, format and mount partition 4 over it, and `again` must mount it.
  - `address` boots one copy with a network card (QEMU user network, `restrict=on`, no forwarded port). `/run/neptron/network.toml` must hold the lease 10.0.2.15. The guest removes the PCI device of `eth0`, and the file must list no address. A PCI rescan brings the port back after the boot, as the USB port of a Pi, and the file must hold the address again. The share must hold no `neptron-ip.txt`. Each of the three waits ends within 60 s.
  - Each boot checks `/run/neptron/storage.toml`, `/proc/mounts` and the lines of the storage script in `/var/log/messages`; a failure prints both. It holds the QEMU lock for each boot.
- `cargo xtask os selftest-check [--arch A] [--timeout SECS]`: the gate of the video self-test (tier T3). It builds `neptronctl` with the `kms` feature for the guest (every target checked first with warnings denied, as `kms-check` does; no host gate compiles the Linux code of the feature), copies `data/monitors` into `target/selftest-check/share/monitors`, and boots the guest with that share (default timeout 600 s). The guest script runs `neptronctl selftest video` once with a confirmation of another profile, which must exit with 6 and name the confirmation on standard error. Then it runs once per profile (`pc_31_120` and `generic_15`) on `/dev/dri/card0` with `--seconds-per-mode 2` and the confirmation, and prints each exit code and each report on one line. The command checks each report: version 2, not a dry run, the display driver `vkms`, state `finished`, `pass` true, as many modes as the profile's plan has (4 for each profile, `PROFILES`; a run with fewer modes fails), every mode passed with its read back equal to the request and valid, and the restore succeeded. It prints one result line and exits non-zero on any failure.
- `cargo xtask os app-check [--arch A]`: checks every target of `neptron` with the `kms` feature and of `neptron-launch` for the Linux target of the guest, with warnings denied, then builds their binaries. No host gate compiles the Linux code of the frontend's `kms` feature, so this is its check. The link goes through `os/package/neptron/link-filter.sh`, as in the image packages: it drops the `/usr/local/lib` search path that the `sdl3` crate adds and that the Buildroot compiler wrapper refuses. It boots no guest and prints one result line.
- `cargo xtask kms-check [--arch A] [--timeout SECS]`: timing check 7 (tier T3) of `neptron-kms` on `vkms`. It builds the example `kms_check` with the `kms` feature for the guest. First it checks every target of `neptron-kms` (library, tests, examples) for the guest: no host gate compiles the Linux code of the feature, and the Buildroot toolchain has no clippy, so this check is the warnings gate for it. It then runs three steps in the guest (default timeout 900 s), each in its own process because a process takes the launcher descriptor once: `kms_check --not-drm` with a pipe on descriptor 3 and `NEPTRON_DRM_FD=3`, `kms_check --standard-fd` with `NEPTRON_DRM_FD=1`, then the main run with the device open on descriptor 3 and `NEPTRON_DRM_FD=3`, as the launcher passes it. The first two check that a wrong descriptor is refused before the process owns it. It exits with the sum of the failed checks of the three steps. `docs/spec/kms.md` lists the checks.
- `cargo xtask gles-compare [--arch A] [--timeout SECS]`: the tier T3 check of the GLES renderer of `neptron-gfx`. It builds the example `gles_compare` with the `gles` feature for the guest, then runs it in the guest with `qemu --share target/gles-compare` (default timeout 1200 s) and exits with its exit code. It mounts the checkout read-only, the Buildroot output volume read-only, and the volume `neptron-cross-<arch>` as the home of the build user, which holds the Cargo registry and the target directories (see "Guest builds from many worktrees"). `crates/neptron-gfx/README.md` describes what the example checks.
- `cargo xtask core-trace [--arch A] [--timeout SECS] [--bless] [--dump FRAME[,FRAME...]] [--allow-missing-roms]`: the tier T4 check of the real cores.
  - It finds each test ROM in `tests/roms` or, when the repository cannot hold it, by its file name in `~/.cache/neptron/test-roms` (`NEPTRON_TEST_ROMS` names another directory), and checks it against `tests/roms/SHA256SUMS`. It copies the ROMs and a zip file of each into `target/core-trace/share`, builds the example `core_trace` of `neptron-libretro` for the guest, and runs it in the guest once per case with `qemu --share target/core-trace/share` (default timeout 600 s).
  - Each case loads one core from `/usr/lib/libretro` in the image, runs its ROM for 900 frames with a fixed input script, and prints a trace. The command compares each trace with `tests/traces/<case>.trace`, skipping the `arch` line, and checks that the trace of each zip case shows the same frames and audio as its plain case. The trace of each run is in `target/core-trace/actual`.
  - It exits non-zero when a trace differs or is missing, when a ROM is missing or has a wrong hash (the case is reported as SKIPPED), or when a program fails. With `--allow-missing-roms`, a case whose ROM is in neither the repository nor the cache is still reported as SKIPPED but does not fail the run; a ROM with a wrong hash still fails it, and so does a run in which no case ran. CI uses this option, because the SNES and Mega Drive ROMs are not in the repository (OP-053) and CI downloads no ROM.
  - `--bless` writes the traces of this run to `tests/traces`; read them first (rule 11). `--dump` writes the given frames of each plain case as PNG files to `target/core-trace/frames`. The command downloads no ROM; `tests/roms/README.md` says where a person gets the ones outside the repository. `tests/traces/README.md` describes the trace lines.

Every guest build (`gles-compare`, `kms-check`, `core-trace`) runs with `RUSTFLAGS=-D warnings`, one setting for all, so the commands of one checkout share one target directory in the cross volume.

### The bench

For a PC or a Raspberry Pi that runs a development image (`os/README.md`, "Working on the bench").

- `cargo xtask os push --host H [--user U] [--port P] [--arch x86_64|rpi-arm64] [--output NAME] [--data] [--no-restart]`: builds `neptron` (features `kms` and `control`), `neptron-launch` and `neptronctl` (`kms`) for the architecture of the image in the development container, with the cross plan of `app-check`. It uses the toolchain and the sysroot of a Buildroot output (`x86_64` for the PC, `rpi-arm64-dev` for the Pi; `--output NAME` names another one).
  - The bench profile is the release profile with no LTO, 16 codegen units, incremental builds, line tables and no strip (`BENCH_PROFILE` in `src/os/cross.rs`). `panic = "abort"` stays, as in the image: the panic hook prints the backtrace before the abort. The release image is built with fat LTO, one codegen unit and stripped symbols, so the bench binaries are larger and a little slower.
  - It sends the binaries over the SSH connection itself (`cat > file`; Dropbear has no SFTP server, which current `scp` needs) to `/state/hot/bin/.<name>.new` and renames each one over the old file. Then it restarts the launcher and the frontend with the hold file: SIGTERM to the launcher, which forwards it to the frontend, and SIGKILL to the frontend only as a fallback after 10 s, with a line that says so. `--no-restart` leaves the running programs alone.
  - `--data` also replaces `/state/hot/share/neptron` with the data that the image holds in `/usr/share/neptron`: the folders `monitors`, `cores`, `systems`, `skins`, `hardware` and `input` of `data/` in the checkout, and the arcade index `arcade/fbneo.idx`. The push makes the index as the `neptron` package does in the image build. It copies the DAT of FinalBurn Neo out of the build folder of the core in the Buildroot output (`build/libretro-fbneo-<commit>/`, the commit that `os/package/libretro-fbneo/libretro-fbneo.mk` pins) to `target/os-push/fbneo.dat`, then runs the generator of `arcade-index` on it into `target/os-push/data/arcade/fbneo.idx`. An output that has not built the pinned core has no such folder, and the push stops before it connects to the machine: build the development image first.
  - It holds the Buildroot build lock for the build.
- `cargo xtask os logs --host H [--user U] [--port P] [--follow]`: prints the log `/state/log/neptron/current`, the lines of the storage script in `/var/log/messages` (tag `neptron-storage`, a RAM folder: this boot only), the last 100 lines of the storage log `/state/log/storage.log` (the last boots), `/run/neptron/storage.toml`, `dmesg` and the hot folder of that machine. `--follow` follows the log.
- `cargo xtask os ssh --host H -- command...`: runs one command there.

Every SSH call of these three commands uses the SSH agent or key of the user and never a password (`BatchMode=yes`, `PasswordAuthentication=no`). It accepts a new host key once (`StrictHostKeyChecking=accept-new`) into `~/.cache/neptron/bench_known_hosts`, so the user's own `known_hosts` does not change. The default user is `root` and the default port 22.

### Generators and UI tools

- `cargo xtask window-smoke`: builds `neptron-window` with its `sdl3` feature and runs its smoke example. No tier compiles the `sdl3` code, so this command and `cargo xtask clippy -p neptron-window --features sdl3` are the checks for it. The example opens a desktop window, shows a 240p and a super-width mode, injects a key press and checks it. It needs a display and the SDL3 library, so no tier runs it.
- `cargo xtask monitors <monitor.cpp> [dir]`: converts the Switchres preset table in a local copy of `monitor.cpp` into one TOML file per preset in `data/monitors` (or `dir`). Each value is converted from its decimal text with integer maths. `data/monitors/README.md` names the source commit and the units.
- `cargo xtask arcade-index --commit <hex> <dat> <out>`: writes the arcade index of `neptron-library` (module `arcade`, format in its README) from the ClrMamePro XML DAT of FinalBurn Neo, `dats/FinalBurn Neo (ClrMame Pro XML, Arcade only).dat` in the source of the pinned core.
  - `--commit` is the 40-hex commit of that source; the index names it, and the diagnostics show it.
  - The command stops with an error that names the set and the field for a value the index cannot hold: a tab or a line break, an orientation other than `horizontal` or `vertical`, a driver status other than `good` or `preliminary`, a ROM with no CRC and no `nodump` status, a disk with no name or with a SHA-1 that is not 40 hex digits.
  - FinalBurn Neo writes `preliminary` for each driver that does not work, so that status gives the flags `n` and `p`. Each `disk` element becomes a `disk` line (its name and its SHA-1 in lower case), so the set check asks for the file of each disk image. Samples and comments are not read. The file is written next to `out` and renamed.
  - The image build runs the command in the install step of `os/package/neptron`, which empties `/usr/share/neptron` first, so every rebuild of the frontend package writes the index again. `board/common/check-arcade-index.sh`, run by the common post-build step, fails a build whose image has the FinalBurn Neo core and no index.
- `cargo xtask rust-notices --program NAME --crates FILE --vendor DIR --std DIR... --out FILE`: writes the licence notices of the crates in one Rust program of the image. Rust links every crate into the binary, and the MIT, ISC, BSD and Apache licences ask that the notice goes with the binary.
  - `--crates` is the output of `cargo tree --target <triple> --edges normal,no-proc-macro --prefix none` for the program. Build-time and proc-macro crates run on the build machine and are not in the binary. A crate of the workspace (a path in parentheses) is NEPTRON's own and is left out.
  - `--vendor` is the `cargo vendor` folder of the build, or a Cargo registry folder: a crate is in `name-version` or in `name` with that version.
  - For each crate the file gives its `license` field and every top-level file whose name starts with `LICENSE`, `LICENCE`, `COPYING`, `COPYRIGHT`, `NOTICE` or `UNLICENSE`, and the `license-file` of its manifest. A text that several crates ship is printed once, with their names. A crate with no licence file gets its authors. A crate that is not vendored, or that has no licence field and no licence file, stops the command.
  - `--std` names the folders where the licence of the Rust standard library may be: the first folder with `LICENSE-MIT` and `LICENSE-APACHE` (the root of a Rust release archive), or with `MIT.txt` and `Apache-2.0.txt` (`share/doc/rust/licenses`), gives the texts. An empty value is skipped. No such folder stops the command.
  - The image build runs it in the install step of `neptron`, `neptron-launch` and `neptronctl` (`NEPTRON_WORKSPACE_NOTICES` in `os/external.mk`), into `neptron-notices/` of the Buildroot output. `board/common/install-licences.sh` copies the file of each program in the image to `/usr/share/licenses/neptron-rust`, and fails the build for a program with no file; the post-image step copies them to `legal/neptron-rust` on the boot partition. On 2026-11-23 the files are 165 kB (`neptron`, 55 crates), 99 kB (`neptronctl`, 30 crates) and 54 kB (`neptron-launch`, 10 crates); `drm-fourcc` and `gbm-sys` ship no licence file.
- `cargo xtask release-names --tag vX.Y.Z --artifact NAME --image FILE.img`: prints `version=`, `image=` and `sources=` lines with the asset names of one image of a GitHub Release: `NAME-X.Y.Z.img.xz` and `NAME-X.Y.Z-sources.tar`, so a person with one file finds the other. It refuses a tag that is not `v` and a version (`v0.1.0`, `v0.2.0-rc.1`), an artifact name with other characters than `a-z`, `0-9`, `-` and `_`, and an image that is not one raw `.img` file. `.github/workflows/release.yml` and the release step of `images.yml` use it (`docs/ci.md`, "Release").
- `cargo xtask bindgen`: generates the libretro bindings of `neptron-libretro-sys` from `crates/neptron-libretro-sys/vendor/libretro.h`. It writes `src/bindings.rs` (every `retro_*` type and `RETRO_*` constant, no functions) and `src/core_api_decls.rs` (the core entry points, no types), then formats both with `rustfmt`. The generated files are committed, so no build needs libclang. The command needs the `bindgen` command-line tool (on macOS: `brew install bindgen`; the committed files come from bindgen 0.73.2). `NEPTRON_BINDGEN` names another executable. The clang target is always `x86_64-unknown-linux-gnu` with `-ffreestanding`, so the output does not depend on the host. The layout checks that bindgen writes are valid for every 64-bit target with the LP64 data model.
- `cargo xtask font`: generates the glyph table `crates/neptron-ui/src/font/spleen_6x12.rs` from the vendored BDF file `crates/neptron-ui/assets/fonts/spleen/spleen-6x12.bdf`, for printable ASCII and Latin-1. The parser reads the BDF text format by hand. A test fails when the committed table differs from the output of the generator.
- `cargo xtask shot <shot>`: renders one shot of the `neptron-ui` registry to `target/ui/shot/<shot>.png` and a 4x enlarged copy `<shot>@4x.png`. A shot is a scene name (landscape) or `portrait/<scene>` (portrait, 240x320); any scene renders in portrait.
- `cargo xtask gallery`: renders the shots of each form onto a contact sheet, `target/ui/gallery.png` (landscape) and `target/ui/gallery-portrait.png` (the portrait goldens), each tile under its name.
- `cargo xtask golden [--bless] [shot...]`: compares each shot (default: all) with its golden image in `crates/neptron-ui/tests/golden/<shot>.ppm`, byte for byte. For a difference it prints the pixel count, the bounding box and the first differing pixels, and writes `<scene>.expected.png`, `<scene>.actual.png` and `<scene>.diff.png` (differing pixels in red over a dark copy of the golden) to `target/ui/golden`. A golden without a scene is reported as stale. It exits non-zero on any failure. `--bless` writes the golden of each scene that differs or has none; nothing blesses automatically. The format is binary PPM, described in `crates/neptron-ui/README.md`. The test `crates/neptron-ui/tests/golden.rs` runs the same comparison in tier 1.

### Screen simulator

The simulator lives in `neptron_testkit::screen`; `docs/spec/screen-simulator.md` defines the model, the measures and the flags. xtask does not depend on it: `cargo xtask screen ARGS` runs `cargo run -p neptron-testkit --example screen -- ARGS` under the machine-load rules, then turns each PPM file the simulator wrote under `target/screen/` into a PNG file (the writer of `src/ui/png.rs`) and draws the labels of the contact sheets with the UI font.

- `cargo xtask screen --display D --source S [--set KEY=VALUE]...`: one case. It writes `frame.png`, `wire.png` (the frame buffer of the mode), `glass.png` (the model of the screen) and `measures.json` to `target/screen/<case>/` and prints the measures as one table row.
- `cargo xtask screen survey [--display D]... [--source S]...`: every picture mode and every refresh rate for each display and source (default: 7 displays, 13 sources, 1001 cases, a few minutes). It writes `target/screen/survey/`: a contact sheet `<display>.png` per display, `survey.md` and `survey.json` with the rows to look at first, and the glass of each row to look at in `look/<display>/`.
- `cargo xtask screen golden [--bless]`: compares the glass of the reference cases with `crates/neptron-testkit/tests/golden/screen/<case>.ppm`; `--bless` writes them. Look at each PNG in `target/screen/golden/` first (rule 11). A unit test of `neptron-testkit` runs the same comparison and asserts the measures of each case.
- `cargo xtask screen gate`: the first gate before a test on a real screen. It runs every stored case, measures the test card on the glass against the shape of the core and the shape of the real console, prints one table with a verdict per case, and exits with a failure when a case fails. It writes `target/screen/gate/`: `gate.md`, one contact sheet per screen type (`crt_15khz`, `crt_25khz`, `crt_31khz`, `ntsc`, `pal`, `panel`), the two fields of each interlaced case in `fields/`. About two minutes.

## Rule checker

`cargo xtask rules` reads `cargo metadata` and checks the rules below. The rule tables are at the top of `src/rules.rs`. A change to them is a change to a dependency rule (run rule 9).

**Unsafe allow-list.** `unsafe` is allowed in every `-sys` crate and in `neptron-libretro`, `neptron-kms`, `neptron-gfx`, `neptron-audio`, `neptron-input` and `neptron-testcore`. Every other crate carries `#![forbid(unsafe_code)]` among the inner attributes at the head of every target root: library, binaries, tests, benches, examples and build script. The checker parses that head (comments, then inner attributes). An attribute inside a module, a comment or a string does not count. A workspace lint cannot do this job, because a crate cannot change one workspace lint.

**Dependency rules.** The layout rules become these checks:

| Rule in the layout | Check |
| --- | --- |
| `neptron-safety` depends only on the shared types | Normal dependencies of `neptron-safety`: `neptron-types` only |
| `neptron-modeline` depends only on the shared types, and wraps `neptron-switchres-sys` | Normal dependencies of `neptron-modeline`: `neptron-types` and `neptron-switchres-sys` only. Build and dev dependencies are free |
| `neptron-switchres-sys` holds the C++ build and the raw calls, nothing else | It depends on no workspace crate, and only `neptron-modeline` depends on it |
| Shared types sit at the bottom | `neptron-types` depends on no workspace crate |
| `neptron-ui` depends on no other NEPTRON crate, and only the app depends on it | `neptron-ui` depends on no workspace crate. Only `neptron` and `xtask` depend on it; `xtask` holds `shot`, `gallery` and `golden` |
| `neptron-video` is the single crate that joins solver, safety and display | Among the library crates, only `neptron-video` depends on `neptron-modeline`. `neptron-video` never depends on `neptron-kms` |
| `kms-spike` depends on `drm`, `neptron-types` and `neptron-safety` only | Normal dependencies of `kms-spike`: those three only |
| `neptron-libretro-sys` is raw bindings | It depends on no workspace crate |
| No async runtime in the frontend | No crate except `xtask` pulls in `tokio`, `async-std`, `smol` or `async-executor` |

Every crate also has a `README.md`.

**Headless build.** Hardware backends are Cargo features, off by default:

| Crate | Features |
| --- | --- |
| `neptron-kms` | `kms` |
| `neptron-gfx` | `gles` |
| `neptron-window` | `sdl3` |
| `neptron-audio` | `sdl3` |
| `neptron-input` | `sdl3`, `evdev` |
| `neptron` (app) | `kms`, `window`, `control` (the commands that change the frontend, not hardware) |
| `neptronctl` | `kms` |
| `kms-spike` | `kms` |
| `neptron-ipc` | `control` (the TCP listener, not hardware) |

The checker reads the features that Cargo resolves for the default build. It fails when a workspace crate gets `kms`, `gles`, `sdl3`, `evdev` or `window` in any way (a `default` list, a chain of features, or `features = [...]` on a dependency entry), and when the default build contains a hardware crate (SDL3, EGL, GL, DRM, GBM, evdev, librashader).

## Machine-load rules (run rule 13)

On macOS, every Cargo command that xtask starts runs under `taskpolicy -c utility`. A build holds one of the build slots. The lock files are in `<git common dir>/neptron-build-slots/`, so all worktrees use the same slots. A build that finds no free slot waits.

| Variable | Default on macOS | Default on other hosts |
| --- | --- | --- |
| `NEPTRON_BUILD_SLOTS` | 2 | no lock |
| `NEPTRON_BUILD_JOBS` (sets `CARGO_BUILD_JOBS`) | 5 | Cargo default |

Cargo compiles `xtask` itself before xtask starts, so that one small compile runs outside `taskpolicy` and the slots.

The OS commands have their own limits, machine-wide through lock files in the cache directory (`NEPTRON_CACHE_DIR`, default `~/.cache/neptron`):

- `os build`: one Buildroot build at a time, 4 jobs by default (`--jobs`, at most 8), a container with as many CPUs as jobs and 6 GB of memory.
- `qemu`: one guest at a time, with 2 GB of memory and 2 CPUs, under `taskpolicy -c utility` on macOS. The guest stops its command at `--timeout` (default 120 s) and powers off; xtask stops QEMU 30 s after that, and at once on SIGTERM, SIGHUP or SIGINT.

### Guest builds from many worktrees

The guest builds (`os push`, `os app-check`, `os selftest-check`, `gles-compare`, `kms-check`, `core-trace`) mount the checkout at `/src/repo` and use the volume `neptron-cross-<output>`. Each checkout has its own Cargo target directory in the volume: `/home/br/target/wt-<folder>-<hash>`, where `<folder>` is the last part of the checkout path and `<hash>` is a 64-bit FNV-1a hash of the whole path. A worktree keeps its directory from build to build.

Cargo decides with file times whether an output is fresh, and it names the outputs of a workspace crate by its path relative to the workspace. So two checkouts that share one target directory take the outputs of each other as fresh, and a build can link code of a different branch. A mount at a different path per checkout does not help, because the relative paths stay the same. With one target directory per checkout, only the file times of that checkout count. The dependencies compile once per checkout: Cargo has no safe way to share them. The Cargo registry in the volume stays shared.

Before its first step, each guest build removes the target directories of checkouts that no longer exist: it keeps the keys of the paths in `git worktree list` that are directories, and its own key. It also removes the files of the one shared target directory of older builds. The guest builds hold the Buildroot build lock, so no other guest build runs during the removal.

## Dependencies

- `neptron-ui`: the scene registry, the CPU canvas and the golden format for `shot`, `gallery` and `golden`. The PNG files are written by a small encoder in `src/ui/png.rs` (one deflate block of fixed Huffman codes over a greedy LZ77 match), so no image crate is needed.
- `signal-hook`: sets a flag on SIGTERM, SIGHUP and SIGINT, so that `qemu` stops its QEMU child before it exits. It needs `unsafe` code to install a handler, which xtask forbids in its own code. It is small: it pulls in `libc` (already in the build) and `signal-hook-registry`.
- `neptron-library`: the encoder of the arcade index, so the index is written by the code that the frontend reads it with.
- `roxmltree`: reads the DAT for `arcade-index`. A read-only XML tree with no dependency but `memchr`, MIT or Apache-2.0. Only xtask has XML code; the image has none.
- `serde_json`: reads `cargo metadata` and Cargo build messages, and writes the report.
- `thiserror`: each module owns its error enum, as the code standards ask.

Arguments are parsed by hand, and the slot lock uses `std::fs::File::try_lock`, to keep the compile time low.

## Must never depend on

- Any NEPTRON crate except `neptron-ui` (for `shot`, `gallery` and `golden`) and `neptron-library` (for `arcade-index`, as `docs/spec/arcade.md` section 6 asks).
- Heavy crates: every dependency must justify its compile time.

## Rules

- `unsafe` is forbidden: the crate root carries `#![forbid(unsafe_code)]`.
