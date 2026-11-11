# NEPTRON

A Rust, Buildroot-based retro-gaming OS for CRTs. The milestone in progress is the MVP: the taskboard in `docs/plan/07-mvp-build-taskboard-for-claude-code.md`, with its status in `docs/taskboard.toml`.

## Where to read

- `docs/definition/`: what NEPTRON is. `11-monorepo-layout.md` has the crate list, the dependency rules and the build profiles.
- `docs/plan/`: how we build it. `01-verification-ladder.md` defines the tiers. `07-...` has the run rules, the contracts and the task briefs. `09-...` has CI and the definition of done. `10-code-standards.md` has the code standards in full.
- `docs/adr/`: decisions that change the design, numbered. See `docs/adr/README.md`.
- `docs/open-points.md`: questions with a default, and their ids.
- Each crate's `README.md`: purpose, public API sketch, what it must never depend on.

Do not edit `docs/definition/`, `docs/plan/` or `docs/review-notes.md` in a task. Report an error in them instead.

## Commands

All builds and tests go through `xtask`. Do not call `cargo` directly for a build or a test.

| Command | What it does |
| --- | --- |
| `cargo xtask verify --tier 0` | fmt, dependency rules, `unsafe` allow-list, licences, clippy with warnings denied |
| `cargo xtask verify --tier 1` | tier 0, plus every test through `cargo nextest`, plus doctests |
| `cargo xtask check -p <crate>` | `cargo check` for one crate |
| `cargo xtask test -p <crate>` | `cargo nextest run` for one crate |
| `cargo xtask clippy -p <crate>` | `cargo clippy` for one crate, tests included |
| `cargo xtask rules` | the dependency-rule and `unsafe` allow-list check alone |
| `cargo xtask worktree new <task>` | a task worktree with a cloned `target/` |
| `cargo xtask worktree cache-check` | proves that a cloned `target/` compiles no dependency again |

`verify` writes `target/verify/report.json` and prints a summary with the build time. Tier 2 and tier 3 come later. `cargo xtask help` lists the other commands (`font`, `shot`, `gallery`, `golden`, `bindgen`, `monitors`, `window-smoke`). `xtask/README.md` describes every check.

On macOS, xtask runs every command under `taskpolicy -c utility` and holds one of 2 build slots with `CARGO_BUILD_JOBS=5`. `NEPTRON_BUILD_SLOTS` and `NEPTRON_BUILD_JOBS` change this.

## Rules for the run (short form)

1. One task is one local branch `mvp/<task>` in its own worktree `.worktrees/<task>`. Nothing is pushed. No pull requests.
2. A task starts from current `main` when all its needs are merged.
3. A task edits only the paths in its "Owns" column. Only the orchestrator edits the root `Cargo.toml`, `Cargo.lock`, `CLAUDE.md`, `docs/taskboard.toml` and `docs/open-points.md`. A new external dependency goes in `[workspace.dependencies]`, in a separate `build(deps)` commit with the reason, and in the task report.
4. A needed change to a contract, a brief or earlier code goes on a separate small branch: the change, every caller fixed, and an ADR.
5. Conventional Commits, one logical change each with its tests. Each commit builds and passes T0 and T1. No `wip` or fixup commits. Rebase, never merge. No emojis. No mention of AI, Claude or any assistant, and no `Co-Authored-By` trailer, anywhere.
6. A "spec first" task commits its file in `docs/spec/` before any code.
7. A task is done when its gate passes, the definition of done holds, and a fresh review approves the diff and the commits.
8. A question without an answer becomes an open point: take a safe default, record it (question, default, where, how to change), continue.
9. Stop the task and report for: a new crate, a change to a dependency rule, a licence question, anything that weakens a safety rule, a change to the scope.
10. A step that needs hardware never stops the run. Add it to `docs/hardware-checklist.md` and claim no hardware result.
11. Never weaken validation, loosen a tolerance, skip a test or bless a changed golden to get a green run.
12. A new worktree gets its `target/` as an APFS clone of `main`'s warm `target/` (`cargo xtask worktree new`).
13. Machine load: every build, test and QEMU command goes through `xtask`. Only one QEMU guest at a time, with 2 GB of memory.
14. Compile time has priority. Build and test only your crate (`cargo xtask check -p`, `cargo xtask test -p`) in the dev profile. The full gate runs once, before merge. Only T28 builds the release profile.
15. Local flow: the review reads the local diff against `main`. The orchestrator rebases, runs the gate, and moves `main` forward (fast-forward only).
16. Commit quality: one logical change per commit, readable alone, and each commit builds. Nothing is squashed.
17. Every agent of the run uses the same model at medium effort.
18. An ADR is only for a decision that changes the design: a contract change, a new crate, a change to a dependency rule, a licence decision, a change to a safety rule. A routine choice goes in the commit body or the crate README.
19. Git safety. See the section below. It is binding for every agent.

## Git safety

The repository must stay clean and correct at all times.

- A task agent uses git only in its own worktree and on its own branch: `status`, `diff`, `log`, `add`, `commit`, and `rebase` onto `main`. It never changes `main`, a different branch, a different worktree or the main checkout.
- Only the orchestrator moves `main`, makes or removes worktrees, and deletes branches.
- Never run `git config`, `git init` inside the repository tree, `git push`, `git stash`, `git clean`, `git gc`, `git reset --hard` on `main`, or any forced operation. Never edit a file in `.git/`. If one of these looks necessary, stop and report.
- Never run a build or a test below a git operation: no `git rebase --exec`, no hooks, no `git bisect run`. Git exports `GIT_DIR` to such a command, and a child that runs git then changes the real repository. To check each commit, add a detached worktree in a temporary directory and run `cargo xtask verify` there from a normal shell.
- In code, every `git` call starts from `repo::git` in `xtask`, which removes the variables that carry the repository location. A test that needs a repository makes it in a temporary directory with that helper.
- Before a task starts and before and after each merge, the orchestrator checks: `core.bare` is false, the main checkout is clean, `main` is at the expected commit, and `git worktree list` shows only the expected worktrees.

## Definition of done

1. Every behaviour has a test, written with the code. A bug fix starts with a test that fails.
2. `cargo xtask verify` passes at the tier the work package names. Paste the report summary into the task report.
3. A visual change comes with a scene and a golden. Open the screenshot and look at it before blessing it.
4. A new setting is a schema entry, and the crawler finds it.
5. Any code that can change the display goes through `ValidatedMode` and has a test.
6. No sleeps in tests: use the simulated clock or `wait`. No network in tests: use the mock server.
7. A change that emulation cannot verify adds a line to `docs/hardware-checklist.md` and is labelled `needs-hardware`.
8. Never bless a golden, loosen a tolerance or skip a test to get a green run. Stop and report what failed instead.

## Code standards

The short form of `docs/plan/10-code-standards.md`. The full text is binding.

**Errors**
- Each crate or module owns its error enum, made with `thiserror`. No global error type.
- An error carries its context: the value, the limit, the path. `#[from]` only when the conversion has one meaning; otherwise `map_err` with context.
- Library crates do not call `unwrap`, `expect` or `panic!`, except for a proven invariant with a comment that gives the proof.
- No `let _ =` on a `Result` without a comment that says why the error does not matter.
- A constructor that can fail returns `Result`. `new()` cannot fail.
- No panic path in an `extern "C"` callback.

**Structure and idiom**
- Decide in pure code, execute in the backend. Policy, validator and UI decide and do no I/O. A backend decides nothing.
- One code path for preview and for run.
- Types of external libraries (`drm`, `gbm`, `glow`, `sdl3`, `libloading`, `evdev`) stay in their backend crate, behind the traits.
- Public items first in each file. Every public item has a `///` comment (`missing_docs` is denied).
- No `mod.rs`. A module root re-exports the public types of its private files.
- Split a large module by concern, as `impl` blocks in child files. No file for two methods.
- Signatures that only read take `&[T]` and `&str`.
- A function has one responsibility.
- Every external dependency is in the root `[workspace.dependencies]`, one version each.
- Formats on disk or on the wire are append-only: add a version, never rename or remove a field.

**Comments**
- The default is no comment. A comment gives a reason the code cannot show: a constraint, an order dependency, an external problem.
- A comment describes the code as it is now, not its history.
- No comment that only points to a document section or a task number. No banners.
- No `TODO` without an open-point id: `TODO(OP-NNN)`.

**Concurrency**
- No async runtime. The frame thread does no file I/O, takes no lock that another thread can hold for long, and never blocks on a channel.
- Threads talk over bounded, one-direction channels. A full channel never stops the frame thread: the sender drops or merges.
- The audio callback is real-time: no lock, no allocation, no I/O. It reads a lock-free ring buffer.
- No lock guard held across a channel receive, a call into a core, or a DRM commit.
- No allocation in the steady-state frame loop. Buffers are made at launch or at a mode change.
- Long work (ROM scan, cache build) checks for cancellation between work units.
- Every thread has an owner that stops and joins it. No detached threads.

**Unsafe**
- `unsafe` only in every `-sys` crate and in `neptron-libretro`, `neptron-kms`, `neptron-gfx`, `neptron-audio`, `neptron-input` and `neptron-testcore` (only on its exported libretro functions). There it sits in one module (`ffi.rs` or equivalent) with a safe wrapper. Every other crate carries `#![forbid(unsafe_code)]` in every target root.
- Every `unsafe` block has a `// SAFETY:` comment that states the invariant. `unsafe_op_in_unsafe_fn` is denied.
- Check a pointer or a length from a core (null, size, alignment) before use. A file descriptor has one owner (`OwnedFd`).

**User data**
- Never change or delete a ROM or a BIOS file.
- Settings, save RAM and save states are written as temp file, fsync, rename. Save RAM keeps one `.bak`.
- A cache can be deleted at any time with no loss of user data. A damaged cache is rebuilt and never stops the boot.

**Tests and reports**
- Pure crates are tested with no mocks. Tests cover the error paths too.
- A task report says what was done, why, what to watch, and which tests exist.

## Other rules

- Safety-critical code stays small and plain: `neptron-safety`, the hard rails, the commit path in `neptron-kms`. Every limit is a named constant with its unit in the name. `neptron-safety` stays near 500 lines, tests excluded.
- Integers for every contract value: frequencies in millihertz, times in nanoseconds.
- Hardware backends are Cargo features and stay off by default. `xtask/README.md` lists the feature names.
- No em dashes. Prose is simple technical English: short sentences, active voice, present tense.
