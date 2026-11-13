# Continuous integration

NEPTRON runs its checks and builds its OS images on GitHub Actions. The repository is public, so the standard runners cost nothing. `docs/plan/09-ci-pipeline-and-definition-of-done.md` is the design; this file describes what exists, where it differs from the design, and why.

Two workflow files do the work:

| File | What it holds |
| --- | --- |
| `.github/workflows/ci.yml` | The triggers, the Rust checks (`lint`, `test-linux`, `test-macos`), the call of the image workflow, and `ci-ok` |
| `.github/workflows/images.yml` | The OS image builds and the tier 3 checks in QEMU, one matrix job per image target |
| `.github/image-targets.json` | One entry per image target: runner, defconfig, `os build` arguments, checks, artifact name |

## What runs when

| Event | Jobs |
| --- | --- |
| Pull request | `lint`, `test-linux`, `test-macos` and `images` start at once. A change to documents only (`docs/`, `*.md`, `LICENSE-*`) runs `lint` only; the test jobs end green in a few seconds. A change that does not touch the OS, the device crates or the image workflow skips the image builds (see "Path filters") |
| Push to `main` | Every job in full, with no path filter. `main` saves the Cargo caches and the Buildroot downloads |
| Nightly (02:23 UTC) | The same as a push to `main`, plus the check of the 15 kHz kernel patch set |
| Manual (`Run workflow` on the Actions page, or `gh workflow run ci.yml --ref BRANCH`) | Every job in full. The input `images` turns the image builds off. A manual run saves the Cargo caches of its own branch |
| Merge queue (`merge_group`) | Every job in full, if the owner turns the merge queue on |

`ci-ok` runs last. It fails when any other job failed or was cancelled, and passes when every job passed or was skipped by a path filter. It is the one check to make required (see "Repository settings").

A new push to a pull request cancels the older run of that pull request. Runs of `main`, nightly runs and manual runs are never cancelled, because a cold image build takes hours. Two image builds of the same target on `main` wait for each other, so a second cold build never starts while the first one runs.

### The jobs

| Job | Runner | What it runs |
| --- | --- | --- |
| `lint (T0)` | `ubuntu-24.04` | `cargo xtask verify --tier 0`: fmt, the dependency rules, the `unsafe` allow-list, `cargo deny`, clippy with warnings denied |
| `test-linux (T0 to T2)` | `ubuntu-24.04` | `cargo xtask verify --tier 2` |
| `test-macos (T0 to T2)` | `macos-15` (Apple Silicon) | `cargo xtask verify --tier 2` |
| `images / plan` | `ubuntu-24.04` | Reads `.github/image-targets.json` and the path filter, and gives the matrix |
| `images / image x86_64` | `ubuntu-24.04` | The PC image without the 15 kHz patch set, then the tier 3 checks under KVM |
| `images / image aarch64` | `ubuntu-24.04-arm` | The aarch64 QEMU test image, then the tier 3 checks |
| `ci-ok` | `ubuntu-24.04` | The summary |

The tier 3 checks of each image, in this order, each one also after an earlier one failed: `os boot-check` (x86\_64: BIOS and UEFI), `os app-check`, `kms-check`, `gles-compare`, `os selftest-check`, `core-trace --allow-missing-roms`. The tier 2 job is the same command as on a laptop, so a green local run predicts a green pull request.

### Path filters

They apply to pull requests only.

- `test-linux` and `test-macos` run unless every changed file is under `docs/`, a `*.md` file or a `LICENSE-*` file.
- The image builds run when a pull request changes `os/`, `Dockerfile`, the OS commands of `xtask` (`xtask/src/os.rs`, `xtask/src/os/`), the image workflow or its target file, the launcher (`apps/neptron-launch`) or a device crate (`neptron-kms`, `neptron-gfx`, `neptron-input`, `neptron-audio`, `neptron-libretro`, `neptron-libretro-sys`).

A change to the frontend alone does not build an image in a pull request, as the plan asks: an image build takes much longer than the other checks, and the frontend runs in tier 2 on the fakes. `main` and the nightly run build the image with it.

## Runners

Measured on 2026-11-13.

| Runner | CPUs and memory | Free disk | KVM |
| --- | --- | --- | --- |
| `ubuntu-24.04` | 4, 15 GiB | `/` 86 GB, 120 GB after "Free disk space"; no `/mnt` | Yes, after the udev rule |
| `ubuntu-24.04-arm` | 4, 15 GiB | `/` 107 GB, 123 GB after "Free disk space"; no `/mnt` | No: there is no `/dev/kvm`, so the aarch64 guest runs under emulation (TCG) |

The runners now have a 145 GB root disk. The often quoted 14 GB free is out of date, and the 44 GB output of the PC image fits with room for its packed base. The image job deletes SDKs that it does not use (.NET, Android, GHC, CodeQL, Swift, PowerShell, Chromium, Azure CLI, Julia, Java): 35 GB on `ubuntu-24.04`, 16 GB on `ubuntu-24.04-arm`. It keeps the Buildroot data in `/mnt/neptron` when `/mnt` has 50 GB free, else in `/home/runner/neptron-work`. The Docker volumes of `os build` are bind volumes on that directory, so the runner user can cache and pack them.

## Measured times

From the runs of pull request 1 on 2026-11-13. Cold means no cache and no base.

| Job | Cold | Warm |
| --- | --- | --- |
| `lint` | 42 s and 44 s | not measured yet |
| `test-linux` | 1 min 57 s and 2 min 0 s (`verify --tier 2`: 98 s) | not measured yet |
| `test-macos` | 2 min 49 s and 3 min 3 s (`verify --tier 2`: 145 s) | not measured yet |
| `image aarch64`, build step | 57 min and 40 min | not measured yet |
| `image x86_64`, build step | more than 320 min (the step limit; it stopped in the kernel build), then 4 h 26 min (15,941 s) | not measured yet |

The two cold PC builds show how much the shared runners vary: the same work took over 320 minutes once and 266 minutes the next time. The partial base (see below) makes the slow case finish in a second run instead of failing for ever.

Sizes of a cold build:

| Target | Buildroot output | Downloads | Compiler cache | Base artifact |
| --- | --- | --- | --- | --- |
| x86\_64 | 44 GB (`build/` 38 GB; host and target LLVM and clang about 3 GB each) | 1.8 GB | 1.1 GB | 7.45 GB packed (10.36 GB as a partial base, which is not pruned); packing 495 to 641 s |
| aarch64 | 17 GB (`build/` 15 GB) | 1.6 GB | 0.3 GB | 3.05 GB packed (4.40 GB as a partial base); packing 142 to 172 s |

An upload of the base runs at about 160 MB/s.

Tier 3 on the images:

| Check | x86\_64 (KVM) | aarch64 (TCG on the arm runner) |
| --- | --- | --- |
| `os boot-check` | BIOS: first frame 4.6 s after QEMU start, the notice at 14.1 s. UEFI: 5.5 s and 15.1 s. Both PASS | First frame 96.8 s, the notice at 103.3 s. PASS |
| `os app-check` | PASS, 44 s | PASS, 28 s |

## Caches

| What | Where | Key | Who saves |
| --- | --- | --- | --- |
| Cargo registry and `target/` of `lint` | Actions cache | OS, CPU, job, hash of `rust-toolchain.toml` and `Cargo.lock` | `main` and manual runs |
| Cargo registry and `target/` of `test-linux` and `test-macos` | Actions cache | The same, per runner | `main` and manual runs |
| Buildroot downloads (`neptron-br-dl`) | Actions cache | Target and hash of every `*.hash` file, `os/configs/*` and `Dockerfile`; an older key is the fallback | `main` |
| Buildroot output and compiler cache: the "base" | Workflow artifact `br-base-<target>-<key>`, 30 days | Target and hash of `Dockerfile`, the defconfig and fragments of the target, `os/patches/` and the core packages `os/package/libretro-*` | Any run that built it |

A pull request restores the caches of `main` and saves none. CI sets `CARGO_INCREMENTAL=0` and no debug info, because both make the cache large and help nothing on a fresh runner. `cargo-nextest` and `cargo-deny` are prebuilt binaries at pinned versions (`taiki-e/install-action`); CI never runs `cargo install`.

### The Buildroot base

A full build of the PC image builds the toolchain, LLVM and clang for the host and the target, Mesa and the kernel. That takes hours. A later build reuses all of it and builds only the NEPTRON packages and the image step.

The base is the Buildroot output directory after a full build, with these parts removed: the object files (`*.o`, `*.lo`) and the CMake build trees (`buildroot-build/`) of every installed package, `legal-info/` and `images/`, which every build makes again. Buildroot decides what to build from the stamp files in `build/<package>/`, and those stay. The compiler cache and the Buildroot downloads go in the same artifact. The output marks every package as downloaded, so Buildroot never fetches those archives again, but `make legal-info` copies them, and it failed on a base without them. The output and the compiler cache are tar files compressed with zstd; the downloads are a plain tar file, because the archives are compressed already.

How an image job finds its base, in order:

1. `br-base-<target>-<key>`: a complete base for this key. The build is warm.
2. `br-partial-<target>-<key>`: a build for this key that failed or ran out of time. The build goes on from there.
3. An older base of the target. A pull request builds on top of it: quick, but not clean, because Buildroot does not remove what an old configuration installed. `main`, the nightly run and a manual run use only its compiler cache, and build cold.
4. Nothing: a cold build.

Only artifacts of this repository, from `main` or from the branch of the run, are used. A fork can upload an artifact with any name, so the job checks the head repository and the branch of the run that made each artifact.

A run that built cold, or went on from a partial base, uploads a new base. A run that ran out of time or failed uploads a partial base. A warm run uploads nothing.

Why an artifact and not the Actions cache: the Actions cache of a repository holds 10 GB, and GitHub evicts the oldest entries above that. The bases of the two targets are larger than that together, beside the Cargo caches. A workflow artifact has no such limit.

Why not a container image on GHCR: it would also work, and container storage on GHCR is free at present. It needs `packages: write` on `main` runs and a visibility decision by the owner. Artifacts need no setting. If artifact storage turns out to be billed for this repository, GHCR is the next step (see "Open questions").

**Commit times.** Make compares the stamps in the base with the files of `os/` (the kernel and BusyBox fragments). A fresh checkout gives every file the current time, so the kernel would build again in every run. The job sets each file of `os/` to the time of its last commit. A fragment that changed after the base was built is newer than the stamps, and Buildroot configures and builds that package again, as it does on a laptop.

**Local packages.** The packages that build this checkout (`neptron`, `neptron-launch`, `neptronctl`, `neptron-capture`, `neptron-vkms-probe`) are built again in every run (`<package>-reconfigure`), because Buildroot does not see a change to a local source. `-rebuild` is not enough: the configure step runs `cargo vendor`, and when `Cargo.lock` gains a crate, a build with the old vendor directory fails offline ("no matching package named ..."). A run on 2026-11-13 failed this way.

**Force a cold build.** Change `BASE_EPOCH` in `images.yml`.

## Licence rules in CI

- **OP-070, the 15 kHz kernel patch set.** Its licence is not clear, so no image, kernel or Buildroot output with those patches may be distributed. The artifacts and the caches of a public repository can be read by others, so they count as distribution. Every image build in CI that uploads or caches anything uses `--no-kernel-15khz` (the `x86_64` entry of `.github/image-targets.json`). The base is built without the patches too. The nightly run then builds the patched kernel in the same job, after every upload and after the base is packed, boots it once with BIOS, and uploads nothing of it. No step may come after that one.
- **OP-054, non-commercial cores.** The PC image holds Snes9x and Genesis Plus GX. The owner accepts that for a free, non-commercial product. The image artifact carries the licence manifest (`legal/manifest.csv`, the licence texts), `LICENCES.txt` and a `README.txt` that says that the image must never be sold. The sources of the GPL and LGPL packages (`make legal-info`) are a second artifact of the same run, with the same retention. There is no GitHub Release: artifacts with a short retention are enough for now.
- **OP-053, test ROMs.** The SNES and Mega Drive ROMs are not in the repository, and CI never downloads a ROM. `core-trace --allow-missing-roms` reports their cases as skipped; the NES cases must pass, and a ROM with a wrong hash still fails the run.

## Artifacts

| Artifact | From | Retention | Contents |
| --- | --- | --- | --- |
| `neptron-pc-x86_64-no15khz` | `image x86_64`, on success | 7 days | `neptron-x86_64.img`, `SHA256SUMS`, `legal/`, `LICENCES.txt`, `README.txt` |
| `neptron-pc-x86_64-no15khz-sources` | `image x86_64`, on success | 7 days | The source archives of `make legal-info` |
| `t3-<target>` | Each image job | 14 days | The boot check report, the captured picture (PPM and PNG), the expected and diff pictures on a failure, the QEMU console logs, the core traces |
| `verify-lint`, `verify-linux`, `verify-macos` | A failed check job | 14 days | `target/verify/report.json`; for a golden failure the expected, actual and diff pictures of `cargo xtask golden` and the actual pictures of the `neptron-gfx` goldens |
| `br-base-<target>-<key>`, `br-partial-<target>-<key>` | Image jobs | 30 days | The Buildroot base (see above) |

Get them on the run page (Summary, Artifacts) or with `gh run download RUN_ID --name NAME`. A downloaded image is written to a stick as `os/README.md` describes. Check it first: `sha256sum -c SHA256SUMS`.

## Re-run and run by hand

- Re-run a failed job on the run page (Re-run failed jobs), or `gh run rerun RUN_ID --failed`.
- Run everything on a branch: `gh workflow run ci.yml --ref BRANCH`, add `-f images=false` for the Rust checks only.
- Force a cold image build: change `BASE_EPOCH` in `.github/workflows/images.yml`.

## Add an image target

1. Add the defconfig to `os/configs` and the target to `xtask` (`os build --arch NAME` and the commands of tier 3), with tests.
2. Add an entry to `.github/image-targets.json`: `name`, `runner`, `arch`, `defconfig`, `fragments`, `build_args` (the arguments of `cargo xtask os build`), `images_dir` (the directory under `~/.cache/neptron/buildroot/images`), `image_files`, `artifact`, `upload_image`, `rebuild` (the local packages that the defconfig selects), `checks` (from `boot`, `boot-bios`, `boot-uefi`, `app`, `kms`, `gles`, `selftest`, `core`) and `build_minutes` (the time limit of the build step; the job stops at 355 minutes).
3. Set `"enabled": true`.

A target whose image could hold the 15 kHz patch set gets `--no-kernel-15khz` in `build_args`.

### The Raspberry Pi image

The `rpi-arm64` entry is in the file and disabled: the board support does not exist yet. The task that adds the Pi image must provide:

- the defconfig `os/configs/neptron_rpi_arm64_defconfig`, with its board files under `os/board/rpi`;
- the target name `rpi-arm64` in `xtask` (`cargo xtask os build --arch rpi-arm64`, images in `~/.cache/neptron/buildroot/images/rpi-arm64`);
- the image file `neptron-rpi-arm64.img`, with the licence manifest on its boot partition as the PC image has;
- the list of local packages in its defconfig (the entry names `neptron`, `neptron-launch`, `neptronctl`);
- the tier 3 checks that apply to it, if any (the Pi image does not boot in QEMU; the aarch64 test image covers the guest checks);
- then `"enabled": true`. The artifact is `neptron-rpi-arm64`. The runner is `ubuntu-24.04-arm`, so the Buildroot host is arm64, as on a Mac.

## Repository settings

These are the settings that only the owner can change. Nothing in CI needs a secret.

1. **Required check.** Settings, Branches (or Rules), a rule for `main`: require the status check `ci-ok`. Do not require the other jobs: a path filter skips them, and a skipped required check of another workflow can block a pull request.
2. **Fork pull requests.** Settings, Actions, General: keep "Require approval for first-time contributors". The workflows use `pull_request`, never `pull_request_target`, so a fork gets a read-only token and no secret.
3. **Workflow permissions.** Settings, Actions, General, "Workflow permissions": "Read repository contents and packages permissions" is enough. Each job asks for its own permissions (`contents: read`, `actions: read` to find the base, `pull-requests: read` for the path filters).
4. **Artifact retention.** The workflows set it per artifact. The repository default (90 days) is only an upper limit.
5. **Merge queue** (optional). The workflow already listens to `merge_group`.
