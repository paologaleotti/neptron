# Continuous integration

NEPTRON runs its checks and builds its OS images on GitHub Actions. The repository is public, so the standard runners cost nothing. `docs/plan/09-ci-pipeline-and-definition-of-done.md` is the design; this file describes what exists, where it differs from the design, and why.

Three files do the work:

| File | What it holds |
| --- | --- |
| `.github/workflows/ci.yml` | The triggers, the Rust checks (`lint`, `test-linux`, `test-macos`), the call of the image workflow, and `ci-ok` |
| `.github/workflows/images.yml` | The OS image builds and the tier 3 checks in QEMU, one matrix job per image target |
| `.github/image-targets.json` | One entry per image target: runner, defconfig, `os build` arguments, checks, artifact name |

## What runs when

| Event | Jobs |
| --- | --- |
| Pull request | `lint`, `test-linux` and `images` start at once. A change to documents only (`docs/`, `*.md`, `LICENSE-*`) runs `lint` only; the test job ends green in a few seconds. A change that does not touch the OS, the device crates or the image workflow skips the image builds (see "Path filters") |
| Push to `main` | Every job except `test-macos` in full, with no path filter. `main` saves the Cargo caches and the Buildroot downloads |
| Nightly (02:23 UTC) | The same as a push to `main`, plus `test-macos` |
| Manual (`Run workflow` on the Actions page, or `gh workflow run ci.yml --ref BRANCH`) | Every job in full. The input `images` turns the image builds off. A manual run saves the Cargo caches of its own branch |
| Merge queue (`merge_group`) | Every job in full, if the owner turns the merge queue on |

`ci-ok` runs last. It fails when any other job failed or was cancelled, and passes when every job passed or was skipped by a path filter. It is the one check to make required (see "Repository settings").

A new push to a pull request cancels the older run of that pull request. Runs of `main`, nightly runs and manual runs are never cancelled, because a cold image build takes hours. Two image builds of the same target on `main` wait for each other, so a second cold build never starts while the first one runs.

### The jobs

| Job | Runner | What it runs |
| --- | --- | --- |
| `lint (T0)` | `ubuntu-24.04` | `cargo xtask verify --tier 0`: fmt, the dependency rules, the `unsafe` allow-list, `cargo deny`, clippy with warnings denied |
| `test-linux (T0 to T2)` | `ubuntu-24.04` | `cargo xtask verify --tier 2` |
| `test-macos (T0 to T2)` | `macos-15` (Apple Silicon) | `cargo xtask verify --tier 2`, in the nightly run and in a manual run only |
| `images / plan` | `ubuntu-24.04` | Reads `.github/image-targets.json` and the path filter, and gives the matrix |
| `images / image x86_64` | `ubuntu-24.04` | The PC image with the 15 kHz patch set, then the tier 3 checks under KVM |
| `images / image aarch64` | `ubuntu-24.04-arm` | The aarch64 QEMU test image, then the tier 3 checks that do not measure time |
| `images / image rpi-arm64` | `ubuntu-24.04-arm` | The Raspberry Pi image (release variant), then the check of its SD card layout |
| `ci-ok` | `ubuntu-24.04` | The summary |

The checks of each image, in this order, each one also after an earlier one failed:

| Target | Checks |
| --- | --- |
| `x86_64` (KVM) | `os boot-check --firmware bios`, `os boot-check --firmware uefi`, `os app-check`, `kms-check`, `gles-compare`, `os selftest-check`, `core-trace --allow-missing-roms`, `os share-check` (all cases) |
| `aarch64` (TCG) | `os boot-check`, `os app-check`, `gles-compare` |
| `rpi-arm64` | The build runs `board/rpi/check-kernel-config.sh` and `board/rpi/check-kernel-patches.sh`; then `.github/check-pi-image.sh` reads the SD card image |

`ubuntu-24.04-arm` has no KVM, so the aarch64 guest runs under TCG, about 100 times slower than with a hypervisor. The checks that measure time (`kms-check`, `os selftest-check`, `core-trace`) cannot pass at that speed. The owner decided: CI runs them on x86\_64 under KVM only. For aarch64 they run on a developer machine with a hypervisor (an Apple Silicon Mac, or an arm64 Linux host with KVM), before a change to the aarch64 image or to the device crates merges. A self-hosted arm64 runner with KVM would bring them back into CI: add them to the `checks` of the `aarch64` entry.

QEMU does not boot the Pi image (its Pi machines have no `vc4` display). `.github/check-pi-image.sh` checks what can be checked without a Pi: the MBR table with the release signature `4e505052`, the boot, root and state partitions at 4, 132 and 516 MiB with their sizes and types, no partition 4, the files of the boot partition (firmware, kernel, device trees, the `vc4` overlay, `LICENCES.txt`, `legal/`), no overlay of the development variant, and the release `cmdline.txt` with `quiet` and the release root.

The tier 2 job is the same command as on a laptop, so a green local run predicts a green pull request.

### Path filters

They apply to pull requests only.

- `test-linux` runs unless every changed file is under `docs/`, a `*.md` file or a `LICENSE-*` file.
- The image builds run when a pull request changes `os/`, `Dockerfile`, the OS commands of `xtask` (`xtask/src/os.rs`, `xtask/src/os/`), the image workflow, its target file or `.github/check-pi-image.sh`, the launcher (`apps/neptron-launch`) or a device crate (`neptron-kms`, `neptron-gfx`, `neptron-input`, `neptron-audio`, `neptron-libretro`, `neptron-libretro-sys`).

A change to the frontend alone does not build an image in a pull request, as the plan asks: an image build takes much longer than the other checks, and the frontend runs in tier 2 on the fakes. `main` and the nightly run build the image with it.

## Runners

Measured on 2026-11-13.

| Runner | CPUs and memory | Free disk | KVM |
| --- | --- | --- | --- |
| `ubuntu-24.04` | 4, 15 GiB | `/` 86 GB, 120 GB after "Free disk space"; no `/mnt` | Yes, after the udev rule |
| `ubuntu-24.04-arm` | 4, 15 GiB | `/` 107 GB, 123 GB after "Free disk space"; no `/mnt` | No: there is no `/dev/kvm`, so the aarch64 guest runs under emulation (TCG) |

The runners have a 145 GB root disk, so the 44 GB output of the PC image fits with room for its packed base. The image job deletes SDKs that it does not use (.NET, Android, GHC, CodeQL, Swift, PowerShell, Chromium, Azure CLI, Julia, Java): 35 GB on `ubuntu-24.04`, 16 GB on `ubuntu-24.04-arm`. It keeps the Buildroot data in `/mnt/neptron` when `/mnt` has 50 GB free, else in `/home/runner/neptron-work`. The Docker volumes of `os build` are bind volumes on that directory, so the runner user can cache and pack them.

## Measured times

From the runs of 2026-11-13 and 2026-11-14 (among them manual runs 43622509874 and 43633804171, and pull request run 43624818679). In run 43633804171, warm on all three targets, `image x86_64` took 23 min 24 s and `image aarch64` 14 min 55 s. Cold means no cache and no base. Warm means the Cargo cache of the job, or a complete base for the key.

| Job | Cold | Warm |
| --- | --- | --- |
| `lint` | 33 to 46 s | 31 s (`verify --tier 0`: 13 s) |
| `test-linux` | 1 min 54 s to 3 min 45 s (`verify --tier 2`: 98 s) | 1 min 43 s (`verify --tier 2`: 80 s) |
| `test-macos` | 2 min 49 s to 5 min 33 s (`verify --tier 2`: 145 s) | 2 min 21 s (`verify --tier 2`: 109 s) |
| `image x86_64`, build step | more than 320 min (the step limit; it stopped in the kernel build), 4 h 26 min, 3 h 45 min | 208 to 219 s |
| `image aarch64`, build step | 40 min, 57 min, 63 min | 208 to 209 s |
| `image rpi-arm64`, build step | 83 min and 90 min | 135 s |
| `image x86_64`, whole job | about 4 h | 20 min 23 s: base download 4 min 48 s, unpacking 4 min 54 s, build 3 min 28 s, both boot checks 1 min 26 s, `app-check` 41 s, `kms-check`, `gles-compare`, `selftest-check` and `core-trace` together 65 s, `share-check` 55 s, the uploads |
| `image aarch64`, whole job | 1 h 10 min to 1 h 16 min | 19 min 43 s: base download 4 min 57 s, unpacking 1 min 23 s, build 3 min 29 s, boot check 73 s, `app-check` 43 s, `gles-compare` under TCG 6 min 38 s |
| `image rpi-arm64`, whole job | 1 h 28 min and 1 h 38 min, with the uploads of the image, its sources and the base | 7 min 24 s: base download 2 min 10 s, unpacking 1 min 30 s, build 2 min 15 s, the layout check, the uploads |

With warm caches every check that does not build an image stays under the 5-minute target; a cold `test-macos` can take up to 5 min 33 s. A pull request restores only the caches of `main` and of its own ref. The Cargo caches are small: 53 MB for `lint`, 118 MB for `test-linux`, 115 MB for `test-macos`.

A warm image build compiles every NEPTRON package again (`<package>-reconfigure`), so a change to the frontend alone costs the same 3.5 minutes as any other warm build. On top of a base, the NEPTRON packages compile quickly, because their Cargo target directories are in the base: `neptron` (fat LTO) 33 s, `neptron-launch` 5 s, `neptronctl` 23 s on `ubuntu-24.04`. Getting the base onto the runner takes most of the time of a warm job: the download of the 9 GB PC base took between 2 and 10 minutes, and the unpacking 3 to 5 minutes. A change to a path of the key makes a new key: the pull request of that change builds on an older base of its own branch or of `main` (incremental; its time depends on what changed, and it is not measured yet), or cold.

The shared runners vary a lot: the same cold PC build took more than 320 minutes once, and 225 minutes another time. The partial base (see below) makes the slow case finish in a second run instead of failing every time.

Sizes of a cold build:

| Target | Buildroot output | Downloads | Compiler cache | Base artifact (output, compiler cache, downloads) |
| --- | --- | --- | --- | --- |
| x86\_64 | 44 GB (`build/` 38 GB; host and target LLVM and clang about 3 GB each) | 1.8 GB | 1.1 GB | 9.31 GB; packing 653 s |
| aarch64 | 17 GB (`build/` 15 GB) | 1.6 GB | 0.3 GB | 4.69 GB; packing 185 to 200 s |
| rpi-arm64 | 17 GB (`build/` 16 GB) | 1.9 GB | 0.6 GB | 5.51 GB; packing 198 to 349 s |

An upload of the base runs at about 160 MB/s. The Pi image artifact is 49 MB (the 772 MiB image, compressed), and its sources artifact 1.0 GB.

## Tier 3 results

From pull request run 43624818679 of 2026-11-14 (commit b36f2b7), warm on both QEMU targets.

| Check | x86\_64 (KVM) | aarch64 (TCG on the arm runner) |
| --- | --- | --- |
| `os boot-check` | BIOS PASS: first frame 4.9 s after QEMU start, 41 s for the step. UEFI PASS: first frame 5.2 s, 45 s | PASS: first frame 59.4 s, 73 s for the step |
| `os app-check` | PASS, 41 s | PASS, 43 s |
| `kms-check` | PASS, 0 failed, 20 s | Not in CI (see "The jobs") |
| `gles-compare` | PASS, 119 checks, 9 s | PASS, 6 min 38 s |
| `os selftest-check` | PASS, 2 profiles, 0 failures, 29 s | Not in CI |
| `core-trace --allow-missing-roms` | PASS: 6 cases, 2 ran (NES), 4 skipped (no SNES and Mega Drive ROMs, OP-053), 0 failures, 7 s | Not in CI |
| `os share-check` | PASS: 12 of 12 boots (cases `rewrite`, `offer`, `retype`, and a second `rewrite` on a reused disk), 55 s | Not in CI |

## Caches

| What | Where | Key | Who saves |
| --- | --- | --- | --- |
| Cargo registry and `target/` of `lint` | Actions cache | OS, CPU, job, hash of `rust-toolchain.toml` and `Cargo.lock` | `main` and manual runs |
| Cargo registry and `target/` of `test-linux` and `test-macos` | Actions cache | The same, per runner | `main` and manual runs |
| Buildroot downloads (`neptron-br-dl`) | Actions cache | Target and hash of every `*.hash` file, `os/configs/*` and `Dockerfile`; an older key is the fallback | `main` |
| Buildroot output, compiler cache and downloads: the "base" | Workflow artifact `br-base-v<format>-<target>-<key>`, 30 days | `BASE_EPOCH`, and the hash of `Dockerfile`, the defconfig and fragments of the target, and the files of its `base_paths` in `.github/image-targets.json`: the patch directory (`os/patches`, or `os/board/rpi/patches` for `rpi-arm64`), the core packages `os/package/libretro-*`, and the kernel and BusyBox fragments of its boards (`os/board/*/*.config`) | Any run that built it |

A pull request restores the caches of `main` and saves none. CI sets `CARGO_INCREMENTAL=0` and no debug info, because both make the cache large and help nothing on a fresh runner. `cargo-nextest` and `cargo-deny` are prebuilt binaries at pinned versions (`taiki-e/install-action`); CI never runs `cargo install`.

### The Buildroot base

A full build of the PC image builds the toolchain, LLVM and clang for the host and the target, Mesa and the kernel. That takes hours. A later build reuses all of it and builds only the NEPTRON packages and the image step.

The base is the Buildroot output directory after a full build, with these parts removed: the object files (`*.o`, `*.lo`) and the CMake build trees (`buildroot-build/`) of every installed package, `legal-info/` and `images/`, which every build makes again. Buildroot decides what to build from the stamp files in `build/<package>/`, and those stay, except `.stamp_images_installed`: a job deletes it after unpacking, so the kernel and the boot loaders copy their files into the empty `images/` again (without that, a warm build fails on the missing `bootx64.efi` and `Image`). The compiler cache and the Buildroot downloads go in the same artifact. The output marks every package as downloaded, so Buildroot never fetches those archives again, but `make legal-info` copies them, and it fails on a base without them. The output and the compiler cache are tar files compressed with zstd; the downloads are a plain tar file, because the archives are compressed already.

How an image job finds its base, in order:

1. `br-base-v<format>-<target>-<key>`: a complete base for this key. The build is warm.
2. `br-partial-v<format>-<target>-<key>`: a build for this key that failed or ran out of time. The build goes on from there.
3. An older base of the target, from `main` or from the branch of the run. A pull request builds on top of it: quick, but not clean, because Buildroot does not remove what an old configuration installed. `main`, the nightly run and a manual run use only its compiler cache, and build cold.
4. Nothing: a cold build.

Only artifacts of this repository are used. A fork can upload an artifact with any name, so the job checks the head repository of the run that made each artifact; only a writer can push a branch to this repository. A complete or partial base with the exact key comes from any branch: the key hashes every input of the base, so the first run of `main` after a merge finds the base that the branch built and is warm. An older base, built from other inputs, comes only from `main` or from the branch of the run.

A run that built cold, or went on from a partial base, uploads a new base. A run that ran out of time or failed uploads a partial base. A warm run uploads nothing.

Why an artifact and not the Actions cache: the Actions cache of a repository holds 10 GB, and GitHub evicts the oldest entries above that. The bases of the targets are larger than that together, beside the Cargo caches. A workflow artifact has no such limit.

Why not a container image on GHCR: it would also work, and container storage on GHCR is free at present. It needs `packages: write` on `main` runs and a visibility decision by the owner. Artifacts need no setting. If artifact storage turns out to be billed for this repository, GHCR is the next step (see "Open questions and limits").

**Commit times.** Make compares the stamps in the base with the files of `os/` (the kernel and BusyBox fragments). A fresh checkout gives every file the current time, so the kernel would build again in every run. The job sets each file of `os/` to the time of its last commit. A fragment that changed after the base was built is newer than the stamps, and Buildroot configures and builds that package again, as it does on a laptop.

**Local packages.** The packages that build this checkout (`neptron`, `neptron-launch`, `neptronctl`, `neptron-capture`, `neptron-vkms-probe`) are built again in every run (`<package>-reconfigure`), because Buildroot does not see a change to a local source. `-rebuild` is not enough: the configure step runs `cargo vendor`, and when `Cargo.lock` gains a crate, a build with the old vendor directory fails offline ("no matching package named ...").

**Force a cold build.** Change `BASE_EPOCH` in `.github/workflows/images.yml`. A change to what the base holds also changes `BASE_FORMAT`, so that no job uses a base of the old layout, not even as an older base for a pull request.

## Licence rules in CI

- **The 15 kHz kernel patch set (ADR 0018).** The set is GPL-2.0-only at the pinned commit, so the PC image applies it and CI uploads that image with its sources. How each job handles kernel patches:
  - `image x86_64`: the build applies the nine patches that `os/configs/neptron_pc_x86_64_defconfig` names (its entry in `.github/image-targets.json` has no `--no-kernel-15khz`). The tier 3 checks run on that patched kernel.
  - `image aarch64`: its defconfig names no kernel patch, and its global patch directory holds only hash files. It uploads no image.
  - `image rpi-arm64`: its defconfig names no kernel patch, and `board/rpi/check-kernel-patches.sh` fails the build when `BR2_LINUX_KERNEL_PATCH` is set, when the global patch directory is not `board/rpi/patches`, or when that directory holds a file other than the two hash files.
  - Every image job: the step "Check the kernel patches of the output" compares `BR2_LINUX_KERNEL_PATCH` in the final `.config` of the output with the value that the defconfig and the fragments of the target name. For a target that names no patch it also fails on a 15 kHz entry in the list of patches that the kernel applied (`build/linux-*/.applied_patches_list`). The image upload, the sources upload, the base and the downloads cache run only after it passed.
  - `lint`, `test-linux`, `test-macos`: they build no kernel.
- **OP-054, non-commercial cores.** The PC and Pi images hold Snes9x, Genesis Plus GX and FinalBurn Neo. The owner accepts that for a free, non-commercial product. The image artifact carries the licence manifest (`legal/manifest.csv`, the licence texts), `LICENCES.txt` and a `README.txt` that says that the image must never be sold. The sources of the GPL and LGPL packages (`make legal-info`) are a second artifact of the same run, with the same retention. There is no GitHub Release: artifacts with a short retention are enough for now.
- **OP-053, test ROMs.** The SNES and Mega Drive ROMs are not in the repository, and CI never downloads a ROM. `core-trace --allow-missing-roms` reports their cases as skipped; the NES cases must pass, and a ROM with a wrong hash still fails the run.

## Artifacts

| Artifact | From | Retention | Contents |
| --- | --- | --- | --- |
| `neptron-pc-x86_64` | `image x86_64`, on success | 7 days | `neptron-x86_64.img`, `SHA256SUMS`, `legal/` (manifest, licence texts, `neptron-rust/`), `LICENCES.txt` (from the boot partition of the image, with its commit), `README.txt` |
| `neptron-pc-x86_64-sources` | `image x86_64`, on success | 7 days | `neptron-pc-x86_64-sources.tar`: the sources archive of the image (see "The sources archive"); it goes out with the image |
| `neptron-rpi-arm64` | `image rpi-arm64`, on success | 7 days | `neptron-rpi-arm64.img`, `SHA256SUMS`, `legal/`, `LICENCES.txt` (of the Pi boot partition, with the terms of the Broadcom firmware), `README.txt` |
| `neptron-rpi-arm64-sources` | `image rpi-arm64`, on success | 7 days | `neptron-rpi-arm64-sources.tar`, the same for the Pi image |
| `t3-<target>` | Each QEMU image job | 14 days | The boot check report, the captured picture (PPM and PNG), the expected and diff pictures on a failure, the QEMU console logs, the core traces |
| `verify-lint`, `verify-linux`, `verify-macos` | A failed check job | 14 days | `target/verify/report.json`; for a golden failure the expected, actual and diff pictures of `cargo xtask golden` and the actual pictures of the `neptron-gfx` goldens |
| `br-base-v<format>-<target>-<key>`, `br-partial-v<format>-<target>-<key>` | Image jobs | 30 days | The Buildroot base (see above) |

### The sources archive

The step "Collect the image, its checksum, the licences and the sources" makes one tar file per uploaded image, with everything needed to build that image again:

- `legal-info/`: `sources/` of `make legal-info` (the archive of each package that Buildroot gives out, with its patches and `series`; the PC kernel with the 15 kHz patch set is in `sources/linux-6.18.63`), `manifest.csv` and `README`. Host packages (`host-sources`) are not in it: they build the image and are not in it.
- `buildroot-<version>.tar.gz`: the Buildroot release of the `Dockerfile`, downloaded from buildroot.org and checked against the SHA-256 of the `Dockerfile`.
- `neptron-<commit>.tar.xz`: `git archive` of the checked-out commit, the commit that the image names. It holds `os/` (the br2-external tree: defconfigs, board files and scripts, package files, kernel configuration fragments), the `Dockerfile` and `xtask`. It leaves out `tests/roms/nes`: no image holds the GPL test ROM, so the archive gives out no binary without its source.
- `neptron-<commit>-vendor.tar.xz`: `VENDOR/` and `.cargo/config.toml` of the build folder of the `neptron` package: the crates of `Cargo.lock` that the build used.
- `README.txt`: what each part is and how to build again.

Legal-info does not save the source of the NEPTRON packages (`neptron`, `neptron-launch`, `neptronctl`): its copy of a local package ignores the rsync exclusions and would take the build output of the checkout. The NEPTRON archive and the vendor archive take its place. The Pi sources artifact was 1.0 GB before this change; the Buildroot archive adds about 8 MB, the NEPTRON archive about 2 MB and the vendored crates about 16 MB (the `.crate` files of `Cargo.lock`, compressed). The size of the archive is in the summary of each run.

Before the upload, the step checks the image against the SHA-256 that "Record the checksums of the built images" wrote right after the build, so the file that goes out is the file that the checks ran on. "Check the release variant" (`.github/check-release-variant.sh`) fails the job when the output holds the SSH server or a file of the development variant, or when the boot partition of the image file does not say `Variant: release`.

Get them on the run page (Summary, Artifacts) or with `gh run download RUN_ID --name NAME`. A downloaded image is written to a stick as `os/README.md` describes. Check it first: `sha256sum -c SHA256SUMS`.

## Re-run and run by hand

- Re-run a failed job on the run page (Re-run failed jobs), or `gh run rerun RUN_ID --failed`.
- Run everything on a branch: `gh workflow run ci.yml --ref BRANCH`, add `-f images=false` for the Rust checks only.
- Force a cold image build: see "Force a cold build".

## Add an image target

1. Add the defconfig to `os/configs` and the target to `xtask` (`os build --arch NAME` and the commands of tier 3), with tests.
2. Add an entry to `.github/image-targets.json`: `name`, `runner`, `arch`, `defconfig`, `fragments`, `base_paths` (the paths whose files go into the key of the base, beside the Dockerfile, the defconfig and the fragments), `build_args` (the arguments of `cargo xtask os build`), `images_dir` (the directory under `~/.cache/neptron/buildroot/images`), `image_files` (an uploaded target has one raw `.img` file), `artifact`, `upload_image`, `rebuild` (the local packages that the defconfig selects), `checks` (from `boot`, `boot-bios`, `boot-uefi`, `app`, `kms`, `gles`, `selftest`, `core`, `share`, `pi-layout`) and `build_minutes` (the time limit of the build step; the job stops at 355 minutes).
3. Set `"enabled": true`.

A target names its kernel patches through its defconfig and its fragments. The patch check step runs for every target, so an output whose patches differ from what the target names fails before any upload.

### The Raspberry Pi image

The `rpi-arm64` target builds the release variant (`cargo xtask os build --arch rpi-arm64`) on `ubuntu-24.04-arm`, so the Buildroot host is arm64, as on a Mac. It rebuilds the local packages `neptron`, `neptron-launch` and `neptronctl` (its defconfig has no QEMU test tools). The development variant is never built in CI: it holds an SSH server and a key.

## Repository settings

These are the settings that only the owner can change. Nothing in CI needs a secret.

1. **Required check.** Settings, Branches (or Rules), a rule for `main`: require the status check `ci-ok`. Do not require the other jobs: a path filter skips them, and a skipped required check of another workflow can block a pull request.
2. **Fork pull requests.** Settings, Actions, General: keep "Require approval for first-time contributors". The workflows use `pull_request`, never `pull_request_target`, so a fork gets a read-only token and no secret.
3. **Workflow permissions.** Settings, Actions, General, "Workflow permissions": "Read repository contents and packages permissions" is enough. Each job asks for its own permissions (`contents: read`, `actions: read` to find the base, `pull-requests: read` for the path filters).
4. **Artifact retention.** The workflows set it per artifact. The repository default (90 days) is only an upper limit.
5. **Merge queue** (optional). The workflow already listens to `merge_group`.

## Open questions and limits

- **Artifact storage.** GitHub's billing pages say that standard runners are free in a public repository. They do not say that artifact storage is free. The bases are 5 to 9 GB each and stay 30 days. If GitHub bills them or refuses an upload for the storage quota, move the base to a container image on GHCR (`packages: write` on `main`; the owner decides the package visibility; its sources must go out beside it).
- **Cold builds.** A change to a defconfig, a fragment, a path of `base_paths` or the Dockerfile makes a new key. The pull request of that change builds cold (or on an older base of its own branch or of `main`), uploads the base, and the first `main` run after the merge takes that base. A `main` run builds cold only when no branch built the key: about 4 hours for the PC image.
- **Not proven yet.** The resume from a partial base is implemented but not yet proven in a run.
- **`os build` on Linux.** The container user `br` has uid 1000. On a Linux host whose user has another uid, the copy of the images and the export of the guest programs fail with "Permission denied". CI works around it (it gives the directories to uid 1000, and runs the guest checks with `umask 0000`). A fix in `xtask` would help every Linux developer.
