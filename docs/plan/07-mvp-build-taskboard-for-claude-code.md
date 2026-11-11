## MVP build: taskboard for Claude Code

The MVP is one playable path on the i3-6100T: boot, game list, launch, correct CRT mode, sound, gamepad, quit. It is 29 tasks in 7 waves, built by Claude Code with no human review. Tasks in one wave own different folders, so Claude Code can run a whole wave in parallel. The longest dependency chain is 8 tasks.

&#91;embedded content: MVP waves · 29 tasks, critical path highlighted\]

Highlighted boxes are the critical path. Every other task has slack. The taskboard table gives the exact dependencies.

### Scope

| In the MVP | After the MVP |
| --- | --- |
| x86\_64 image (QEMU and USB stick) and a desktop window backend | Raspberry Pi image: composite on your Pi 3B first, GPIO DPI when the VGA666 arrives |
| Software-rendered cores: NES (FCEUmm), SNES (Snes9x), Mega Drive (Genesis Plus GX), and the scripted test core | Hardware-rendered cores, PlayStation, arcade |
| OpenGL ES 2 game path on GBM buffers, allocated once, with linear modifiers | Shaders, rotation, plane crops |
| ODREN: full decision procedure for 15 kHz, 31 kHz and flat-panel profiles, and a profile built from a PC CRT's EDID | Signal geometry adjustment |
| Safety layers 1 to 8, with the launcher | Proof of layer 7 on AMD hardware |
| UI: home, game list, quick menu, display panel with confirm-or-revert, one grid test pattern | First-boot wizard, full settings tree, skins, save-state UI, search |
| Three-level settings with the typed schema | Network, updates (RAUC), installer, guns, wheels, box art |
| SDL3 audio with rate control; SDL3 gamepads, evdev keyboards | Guided mapping, mice, spinners |
| Control API and tiers T0 to T3; T4 for the three real cores | T5 hardware runners |

### Rules for the run

1. One task is one branch (`mvp/T05-modeline`) in its own git worktree. The branch stays local (rule 15).
2. A task starts from current `main`, and only when every task in its "Needs" column is merged.
3. A task edits only the paths in its "Owns" column. The orchestrator alone edits the root `Cargo.toml`, `Cargo.lock`, `CLAUDE.md`, `docs/taskboard.toml` and `docs/open-points.md`. A task that needs a new external dependency adds it to `[workspace.dependencies]` in the root `Cargo.toml`, in a separate `build(deps)` commit with the reason in the body, and lists it in its task report. The orchestrator resolves conflicts in the root `Cargo.toml` and `Cargo.lock` at the rebase.
4. The contracts and the task briefs below are the starting point, not a cage. A task that needs a contract, a brief or an earlier task's code to change makes that change itself, on a separate small branch: the change, every caller in the workspace fixed so that all crates still build, and an ADR that says why. The orchestrator verifies and merges it before anything else, updates this document and the taskboard, and tells every running task to rebase onto main. A running task rebases, adapts, and continues.
5. Commits are Conventional Commits (`feat(modeline): add crt_range parser`), one logical change each, as many as the work needs. Each commit builds and passes T0 and T1. No `wip` or fixup commits: rebase before merge. Always rebase, never merge: a branch gets new commits from main with git rebase only, and no merge commit exists anywhere in the history. Merges are fast-forward only, so `main` stays linear and bisectable. No emojis. No mention of AI, Claude or any assistant, and no Co-Authored-By trailer, in commits, branch names, pull requests, code or comments. CLAUDE.md files are tracked in the repository as usual.
6. A task marked "spec first" commits its file in `docs/spec/` before any code. The reviewer checks the spec against the Definition before the code.
7. A task is done when its gate passes, the definition of done holds, and a review agent with a fresh context approves the diff and the commit structure (rule 16) against this document. No human reviews during the MVP: the reviewer approves new goldens after it opens each image, and the generated safety limits after it checks them against the presets.
8. A question this document does not answer becomes an open point. With a safe default, the task takes the default, records it in its task report (question, default, where in the code, how to change it), and continues. The orchestrator copies it to `docs/open-points.md`.
9. An architecture decision stops only that task. These are: a new crate or a change to a dependency rule, a licence question, any change that weakens a safety rule, a change to the scope table. The orchestrator records it, continues the other tasks, and asks you.
10. A step that needs hardware never stops the run. The task adds its checks to `docs/hardware-checklist.md` and claims no hardware result.
11. Never weaken validation, loosen a tolerance, skip a test or bless a changed golden to get a green run.
12. Build cache. A new worktree gets its `target/` as an APFS clone of the warm `target/` of `main` (`cp -cRp`). Dependencies do not compile again, and the clone uses almost no disk space. The orchestrator deletes the worktree and its `target/` at merge, and builds `main` again after each wave to keep the source warm. If the clone does not keep the dependencies fresh, the run uses `sccache` instead.
13. Machine load. The development machine is an M1 Pro with 10 cores (8 performance, 2 efficiency) and 16 GB of memory, and it must stay usable during the run. Every build, test and QEMU command goes through `xtask`, which starts it with `taskpolicy -c utility`: the work uses all cores when they are free and gives way to the applications in use. `xtask` also holds a lock with two build slots, each with `CARGO_BUILD_JOBS=5`, so builds fill the 10 cores and no more. A third build waits for a slot. Only one QEMU guest runs at a time, with 2 GB of memory. Memory is the hard limit: if the machine starts to swap, the orchestrator goes down to one build slot. No agent calls `cargo` directly for a build or a test.
14. Fast builds. Compile time has priority over run-time speed for the whole MVP. A task builds and tests only its own crate (`cargo check -p`, `cargo nextest run -p`) in the dev profile. The full workspace gate runs one time, before merge. Only T28 builds the release profile.
15. Local flow. The MVP run opens no pull requests and pushes no task branch. The review agent reads the local diff of the branch against `main`. After approval, the orchestrator rebases the branch, runs the local gate on it, moves `main` forward (fast-forward) and deletes the branch and the worktree. The local gate includes T3 in the aarch64 QEMU guest for the tasks that need T3. The orchestrator pushes `main` one time, when the MVP is finished, with the full commit history and no squash. CI runs on that push. The x86\_64 checks run only in CI, so this first run can show failures that the local gate cannot see; the orchestrator repairs them with new commits. Where this document says "pull request", the MVP run uses the task report. Pull requests become the normal flow after the MVP.
16. Commit quality. The history is part of the product, and the work must be clean. A commit holds one logical change with its tests, and a person can read it alone. No commit is a dump of a full task. Each commit builds. The agent decides the number and the size of the commits. Nothing is squashed, at any time.
17. Model. Every agent of the run (orchestrator, implementer, reviewer) uses Opus 5.5 (`claude-opus-5-5`) at medium effort.
18. ADRs. An ADR (architecture decision record, a short file in `docs/adr/`) is only for a decision that changes the design: a contract change, a new crate, a change to a dependency rule, a licence decision, a change to a safety rule. A routine choice goes in the commit body or the crate README, not in an ADR.
19. Git safety. The repository must stay clean and correct at all times.
    - A task agent uses git only in its own worktree and on its own branch: `status`, `diff`, `log`, `add`, `commit`, and `rebase` onto `main`. It never changes `main`, a different branch, a different worktree or the main checkout.
    - Only the orchestrator moves `main`, makes or removes worktrees, and deletes branches.
    - Nobody runs `git config`, `git init` inside the repository tree, `git push`, `git stash`, `git clean`, `git gc`, `git reset --hard` on `main`, or any forced operation, and nobody edits a file in `.git/`. If one of these looks necessary, the task stops and reports.
    - No build and no test runs below a git operation: no `git rebase --exec`, no hooks, no `git bisect run`. Git exports `GIT_DIR` to such a command, and a child that runs git then changes the real repository. To check each commit, use a detached worktree in a temporary directory and run `cargo xtask verify` there from a normal shell.
    - In code, every `git` call starts from `repo::git` in `xtask`, which removes the variables that carry the repository location. A test that needs a repository makes it in a temporary directory with that helper.
    - Before it starts a task and before and after each merge, the orchestrator checks the repository: `core.bare` is false, the main checkout is clean, `main` is at the expected commit, and `git worktree list` shows only the expected worktrees. If a check fails, the run stops until the repository is repaired.

### Contracts

T02 turned this sketch into code. The code in `crates/neptron-types`, `crates/neptron-safety` and `crates/neptron-backend` is now the contract, and rule 4 says how it changes. The block below is the original sketch; `docs/adr/0001-contract-corrections-in-code.md` lists the differences (a tenth `Rejection` variant `Malformed`, payloads on every variant, `FlipEvent`, `InputEvent` and `Control` in `neptron-types`, `DisplayError`, and snake_case names on the wire). All values are integers: frequencies in millihertz, times in nanoseconds.

```rust
// neptron-types: plain data with serde, no I/O, #![forbid(unsafe_code)]
pub struct Modeline {                 // integer timings, as the hardware gets them
    pub pclock_hz: u32,
    pub hactive: u16, pub hsync_start: u16, pub hsync_end: u16, pub htotal: u16,
    pub vactive: u16, pub vsync_start: u16, pub vsync_end: u16, pub vtotal: u16,
    pub interlace: bool,              // vertical values count both fields, as in DRM
    pub doublescan: bool,
    pub hsync_positive: bool, pub vsync_positive: bool,
}
impl Modeline { pub fn line_rate_mhz(&self) -> u64; pub fn field_rate_mhz(&self) -> u64; }

pub struct CrtRange {                 // one Switchres crt_range line
    pub h_min_mhz: u64, pub h_max_mhz: u64, pub v_min_mhz: u64, pub v_max_mhz: u64,
    pub h_front_porch_ns: u32, pub h_sync_ns: u32, pub h_back_porch_ns: u32,
    pub v_front_porch_ns: u32, pub v_sync_ns: u32, pub v_back_porch_ns: u32,
    pub hsync_positive: bool, pub vsync_positive: bool,
    pub progressive_lines_min: u16, pub progressive_lines_max: u16,
    pub interlaced_lines_min: u16, pub interlaced_lines_max: u16,
}
pub enum MonitorKind { Crt { ranges: Vec<CrtRange> }, FlatPanel { modes: Vec<Modeline> } }
pub struct MonitorProfile { pub id: String, pub kind: MonitorKind }
pub struct GpuCaps {
    pub pclock_min_hz: u32, pub pclock_max_hz: u32,
    pub interlace: bool, pub flip_per_field: bool,
    pub max_width: u16, pub max_height: u16,
}
pub enum Rotation { R0, R90, R180, R270 }
pub struct SourceGeometry {
    pub width: u16, pub height: u16, pub refresh_mhz: u32,
    pub aspect_milli: u32, pub interlaced: bool, pub rotation: Rotation,
}
pub enum PixelFormat { Xrgb8888, Rgb565, Xrgb1555 }
pub struct Frame<'a> { pub data: &'a [u8], pub width: u16, pub height: u16, pub pitch: usize, pub format: PixelFormat }
pub struct Rect { pub x: i32, pub y: i32, pub w: u32, pub h: u32 }
pub enum Filter { Nearest, SharpBilinear }
pub struct Scene<'a> {                // everything one flip shows
    pub game: Option<Frame<'a>>, pub crop: Rect, pub dest: Rect, pub filter: Filter,
    pub ui: Option<Frame<'a>>,        // the 320x240 canvas, drawn over the game
}

// neptron-safety: depends on neptron-types only
pub struct ValidatedMode { /* private fields: only validate() builds one */ }
impl ValidatedMode { pub fn mode(&self) -> &Modeline; }
pub fn validate(mode: &Modeline, profile: &MonitorProfile, caps: &GpuCaps)
    -> Result<ValidatedMode, Rejection>;
pub enum Rejection {                  // each variant carries the value and the limit
    LineRate, FieldRate, LineCount, SyncWidth, Porch, PixelClock, Interlace, HardRail, NotAPanelMode,
}

// neptron-backend: depends on neptron-types and neptron-safety
pub trait Display {
    fn caps(&self) -> &GpuCaps;
    fn set_mode(&mut self, mode: &ValidatedMode) -> Result<Modeline, DisplayError>; // the mode read back
    fn present(&mut self, scene: &Scene) -> Result<(), DisplayError>;               // queues one flip
    fn wait_flip(&mut self) -> Result<FlipEvent, DisplayError>;                     // FlipEvent { time_ns, sequence }
}
pub trait AudioSink {
    fn rate_hz(&self) -> u32;
    fn queued_frames(&self) -> usize;
    fn write(&mut self, stereo: &[i16]) -> usize;
}
pub trait InputSource { fn poll(&mut self, out: &mut Vec<InputEvent>); }  // InputEvent { device, control, value }
pub trait Clock { fn now_ns(&self) -> u64; }
```

`neptron-gfx` has two renderers for a `Scene`: `ReferenceRenderer` (CPU, pure Rust) defines the correct pixels, and `GlesRenderer` must match it. The fake display and the window backend use the first; `neptron-kms` uses the second.

### Taskboard

| ID | Wave | Task | Owns | Needs | Gate |
| --- | --- | --- | --- | --- | --- |
| T01 | 0 | Scaffold | Whole repo | None | T0 green on both hosts in CI |
| T02 | 0 | Contracts and monitor presets | `crates/neptron-types`, `crates/neptron-backend`, API of `crates/neptron-safety`, `data/monitors` | T01 | T1; code matches the contracts block |
| T03 | 1 | Test fakes | `crates/neptron-testkit` | T02, T04 | A 1,000-frame fake session gives identical output twice |
| T04 | 1 | Safety validator | `crates/neptron-safety` | T02 | Timing check 2 |
| T05 | 1 | Switchres wrapper | `crates/neptron-modeline`, `crates/neptron-switchres-sys`, `vendor/switchres` | T02 | Timing check 1, identical on macOS and Linux |
| T06 | 1 | Reference renderer | `crates/neptron-gfx` except `src/gles` | T02 | Pixel goldens |
| T07 | 1 | UI core | `crates/neptron-ui`, `xtask/src/ui` | T01 | First scene golden is bit-identical on both hosts |
| T08 | 1 | Settings | `crates/neptron-settings` | T01 | Unit tests and fixtures |
| T09 | 1 | Libretro bindings and test core | `crates/neptron-libretro-sys`, `crates/neptron-testcore` | T01 | Test core builds as a dynamic library; layout tests |
| T10 | 1 | Library | `crates/neptron-library`, `data/systems`, `data/cores` | T01 | Fixture trees; a 10,000-game query fits the frame budget |
| T11 | 1 | GPU detection and capability table | `crates/neptron-platform`, `data/hardware` | T02 | Unit tests on fixture sysfs trees |
| T12 | 1 | Control protocol and `neptronctl` client | `crates/neptron-ipc`, `apps/neptronctl` | T02 | Round-trip tests |
| T13 | 1 | Window backend | `crates/neptron-window` | T02, T04 | Builds on both hosts; smoke test |
| T14 | 1 | OS base and QEMU | `os/`, `Dockerfile`, `xtask/src/os` | T01 | Image boots in QEMU; `vkms` findings in the task report |
| T15 | 2 | ODREN policy | `crates/neptron-modeline/src/policy` | T05, T11 | Timing checks 3 and 4 |
| T16 | 2 | Libretro host (spec first) | `crates/neptron-libretro`, `docs/spec/libretro-host.md` | T03, T09 | Test-core frame hashes |
| T17 | 2 | Audio (spec first) | `crates/neptron-audio`, `docs/spec/pacing-audio.md` | T03 | Audio part of timing check 6 |
| T18 | 2 | Input | `crates/neptron-input` | T03 | Scripted device tests; mapping fixtures |
| T19 | 2 | OpenGL ES renderer | `crates/neptron-gfx/src/gles` | T06, T14 | T3: matches the reference renderer under Mesa software GL |
| T20 | 2 | Screens | `crates/neptron-ui/src/screens`, `data/skins/default` | T07 | Scene goldens |
| T21 | 2 | KMS spike tool (WP 1.1) | `tools/kms-spike` | T04 | Runs on `vkms`; you run it on the i3-6100T when you can |
| T22 | 3 | ODREN engine | `crates/neptron-video` except `src/guard` | T03, T04, T15 | Timing check 5 |
| T23 | 3 | KMS backend (spec first) | `crates/neptron-kms`, `docs/spec/kms.md` | T04, T14, T19 | Timing check 7 on `vkms` |
| T24 | 3 | Three real cores and test ROMs | `os/package/libretro-*`, `tests/roms`, `tests/traces` | T14, T16 | T4 frame hashes; traces committed |
| T25 | 4 | Session | `crates/neptron-session` | T06, T16, T17, T18, T22 | Timing check 6; scripted session |
| T26 | 4 | Safety behaviours | `crates/neptron-video/src/guard` | T08, T22 | Scripted safety scenarios |
| T27 | 5 | App wiring and launcher | `apps/neptron`, `apps/neptron-launch`, `xtask/src/verify` | T10, T11, T12, T13, T20, T25, T26 | T2 end to end: list, launch, mode switches, quick menu, quit; UI crawler |
| T28 | 6 | x86\_64 image | `os/` | T23, T24, T27 | T3: QEMU boots to the game list; `vkms` writeback equals the golden; boot time recorded |
| T29 | 6 | Hardware test kit | `apps/neptronctl/src/selftest`, `docs/hardware-checklist.md` | T23, T27 | Self-test passes on `vkms` |

`docs/taskboard.toml` holds the same rows plus a status field. The orchestrator reads it to find ready tasks and updates it on each merge.

### Task briefs

Each brief lists what the task must deliver and the choices that are already made. The implementer also reads the Definition section in brackets.

| ID | Must deliver | Already decided |
| --- | --- | --- |
| T01 | Cargo workspace with every crate of the monorepo layout as a stub with a README; `xtask verify` for T0 and T1; dependency-rule and `unsafe` allow-list checks; CI; `CLAUDE.md`, `docs/adr/`, `docs/taskboard.toml`, `docs/open-points.md` \[Monorepo layout\] | Edition 2024, toolchain pinned in `rust-toolchain.toml`. `LICENSE-MIT` and `LICENSE-APACHE`. Licence check with `cargo-deny`. Automation lives in `xtask`, not in shell scripts. Build profiles as in the Definition. The headless build for T0 to T2 compiles no SDL3, EGL or KMS code. An xtask command makes a task worktree with its target directory cloned from main (rule 12), and a check proves that a build in a new clone compiles no dependency again |
| T02 | The contracts block as code; monitor presets converted from Switchres into `data/monitors/*.toml`, with a parse test | Integers only, no floats in any contract type. `validate` is `todo!()` until T04 |
| T03 | Fake display that records every mode and scene, with programmable refresh, jitter and late flips; fake audio sink; scripted input; simulated clock \[Verification ladder\] | The simulated clock is the only time source. No threads, no sleeps |
| T04 | `validate` with every range check, and the hard rails \[CRT safety model\] | Integer maths only. It judges the modeline it is given. A build script generates the rails from `data/monitors`. No dependency except `neptron-types` |
| T05 | Switchres as a vendored copy of the calculation sources at a pinned commit, in `vendor/switchres` with its licence file and the commit in a README, compiled with the `cc` crate; no git submodule (rule 19); a safe function from source geometry, profile and GPU capabilities to a list of modelines; the golden list of about 40 systems | Compile only the calculation sources, with `-ffp-contract=off`, in `neptron-switchres-sys`. That crate holds the C++ build and the raw calls, and nothing else. `neptron-modeline` wraps it and has no `unsafe`. Calls are serialised behind one lock. An ADR records the licence |
| T06 | `ReferenceRenderer`: scale, crop, both filters, UI overlay, all three pixel formats | Pure Rust, no dependencies. Its output is the definition of correct |
| T07 | Canvas trait and CPU canvas, bitmap font, the five widget templates, scene registry, snapshot and intent types; `xtask shot`, `gallery`, `golden` \[Frontend and UX\] | No dependency on any NEPTRON crate. 320x240 XRGB. Try `embedded-graphics` first and record the choice in the crate README. The font has a permissive licence, committed with it |
| T08 | Typed schema, three-level resolution, TOML files \[Settings model\] | Order is game, system, global, default. Writes are temp file, fsync, rename |
| T09 | Bindings for `libretro.h`; `neptron-testcore`, a core that follows a script | `xtask bindgen` generates the bindings and they are committed. `libretro.h` is vendored. The test core reads its script from a file named in an environment variable. In the test core, `unsafe` is only on the exported libretro functions, each with a `// SAFETY:` comment |
| T10 | ROM scan, index cache, metadata for the three MVP systems, the folder layout of the Definition (Batocera and RetroPie names) | The cache is one versioned binary file, rebuilt on a version mismatch. Scans never run on the frame thread |
| T11 | GPU detection from sysfs; `matrix.toml` with capabilities and quirks; a monitor profile built from the range limits in a monitor's EDID \[Hardware support matrix\] | An unknown GPU gets conservative values: 25 MHz minimum pixel clock, no interlace |
| T12 | Message types for every command of the agent control API; `neptronctl` client | JSON lines on a Unix socket. A TCP listener exists only with the `control` feature |
| T13 | SDL3 window that shows the reference renderer's output; keyboard as a gamepad | The `sdl3` crate, pinned. Tests never use this backend |
| T14 | Dev container; br2-external tree; both QEMU configs; BusyBox init with `mdev`; `xtask qemu`; the `vkms` check for user modes and interlace \[OS layer, Local emulation\] | Buildroot 2026.08 at a pinned commit, kernel 6.18 LTS, glibc. Mesa software GL in the QEMU configs. SDL3 without libudev. On a Mac, QEMU runs natively with the aarch64 config |
| T15 | The decision procedure, horizontal strategies, picture styles, the pixel-perfect ladder, sources that do not fit \[CRT video pipeline\] | Implement the eight steps literally, one test row per rule. No I/O. Each candidate carries its reason |
| T16 | Core loading, the "Must" callbacks, save RAM, core options | `libloading`. An unknown environment call returns false and is logged once. One core per process |
| T17 | Resampler, dynamic rate control, SDL3 sink | `rubato`. RetroArch's method with at most 0.5% deviation. 16-bit stereo. SDL3 audio subsystem only |
| T18 | Gamepads and keyboards mapped to RetroPad, hotplug \[Input\] | SDL3 gamepad subsystem with no video subsystem. Keyboards through the `evdev` crate, grabbed. One read per frame |
| T19 | `GlesRenderer` for a `Scene` | EGL and OpenGL ES 2 through `khronos-egl` and `glow`. Two textured quads, no other shaders. Nearest is exact; sharp-bilinear is within 1/255 per channel |
| T20 | Home, game list, quick menu, display panel, confirm dialog, notice, grid pattern | Each screen is a pure function of snapshot and input. D-pad and two buttons only. One golden per scene |
| T21 | A standalone program that shows 240p, 480i and 480p test modes for a named preset, each for 10 seconds, and prints what the driver programmed | Depends on `drm`, `neptron-types` and `neptron-safety` only. Every mode passes `validate` first. It reverts on exit |
| T22 | The engine that joins policy, validator and display: debounce, commit, mute, switch trace \[Switching, Timing\] | It uses the `Display` trait, never `neptron-kms`. Debounce is 3 frames. An equal mode changes only scale and crop |
| T23 | `Display` on DRM/KMS: connector choice, atomic modeset, GBM buffers, flips | Atomic API only, with a test-only commit first. Buffers once at the largest size, linear modifier. The DRM descriptor comes from the launcher through `NEPTRON_DRM_FD`. Connector rule as in the Definition |
| T24 | Buildroot packages for FCEUmm, Snes9x and Genesis Plus GX at pinned commits; test ROMs; core traces | Each core's licence goes in its metadata and the licence manifest. ROMs are the 240p Test Suite builds. Hashes after a fixed frame count |
| T25 | Launch, the display-locked loop, pause, quick menu, exit, save RAM flush | The core runs one frame per flip. Double refresh presents each frame twice. Save RAM is flushed on exit and on a timer |
| T26 | Confirm-or-revert, last known good, safe mode, blind recovery \[CRT safety model\] | 10-second confirm, 5-second recovery hold, safe mode after two failed boots. All tested in simulated time |
| T27 | `neptron` (wiring only), `neptron-launch`, control handlers, `xtask verify --tier 2`, the UI crawler | The launcher opens the DRM device, passes the descriptor, and respawns the frontend with a backoff. No logic in the app crate |
| T28 | Kernel fragments, the 15 kHz patch set, the frontend and cores in the image, licence manifest, USB image | fbdev emulation, framebuffer console and virtual terminals are off. GRUB boots on BIOS and UEFI. The patch set is pinned |
| T29 | `neptronctl selftest video`; the checklist for your bench | Each mode family shows for 10 seconds with readback. The report is one JSON file |

### How the orchestrator runs a task

**Pilot first.** Wave 0 (T01 and T02) runs alone. The contracts block is a sketch that was never compiled, so T02 may correct it where the compiler demands, and records each correction in an ADR and in this document. After T02 merges, the orchestrator proves that the stubs of every wave-1 crate build against the contracts, sends you a short report, and waits for your go. Wave 1 then starts its twelve tasks. T03 and T13 are written with the others, but they merge after T04, because only `validate` can make the `ValidatedMode` that a display needs. Later contract changes follow rule 4.

1. Pick every task whose needs are merged. Start one implementer agent per task, each in its own worktree. The brief is the task's two table rows, the contracts block, the Definition sections it names, and the crate README.
2. The implementer writes tests with the code, runs the gate, and sends the orchestrator a task report with the gate summary and any open points.
3. A verifier runs `cargo xtask verify` at the task's tier on a clean checkout. A reviewer with a fresh context reads the diff against this document. Neither can edit the code. A rejected task goes back to its implementer, at most twice; a third failure becomes an open point for you.
4. On approval the orchestrator rebases the branch, merges fast-forward in ID order, and runs T0 and T1 on `main`. A failure reverts that merge.

### MVP exit

| Gate | Who | Pass when |
| --- | --- | --- |
| Automated | Claude Code | `cargo xtask verify --tier 3` is green on Linux, tier 2 is green on macOS, and T4 is green for the three cores |
| Any monitor | You, 10 minutes | The USB image boots to the game list; each core plays with sound and a gamepad; the quick menu quits to the list |
| SyncMaster 740s, profile from its EDID | You, 15 minutes | The self-test report shows the programmed modes; 480p, the 2x picture style and 240p at 120 Hz are stable; confirm-or-revert reverts with no button press; blind recovery restores the safe mode |
| SCART TV, 15 kHz profile | Not required for the MVP; you or the community, when an adapter is there | 288p at 50 Hz, 240p at 60 Hz and 576i are stable |

The taskboard covers WP 0.1 to 0.6, 1.1 to 1.7 and 2.1 to 2.5, and the minimum of 2.6, 2.7, 3.1 to 3.3. The work packages below stay the full plan to the alpha.
