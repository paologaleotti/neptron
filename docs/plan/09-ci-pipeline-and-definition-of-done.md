## CI pipeline and definition of done

CI runs the same `cargo xtask verify` the agent runs locally, so a green local run predicts a green pull request.

| Trigger | Jobs |
| --- | --- |
| Every pull request | macOS: T0 to T2. Linux: T0 to T3 on the x86\_64 QEMU config with KVM. Golden diff images attached for any changed golden |
| Nightly | T4 real cores; T3 on the aarch64 QEMU config; full image builds for `rpi-arm64` and `x86_64`; boot-time and memory trends; 512 MB memory-budget run |
| Hardware runners, when present | T5: self-test with sync probe; Pi benchmarks |
| Release | Everything above, plus signed images and the generated hardware matrix |

**Fast CI.** The default host is GitHub Actions on a public repository, where the standard runners are free. The target for a pull request is less than 5 minutes with warm caches. This is a target to measure from T01, not a result.

- **Parallel jobs, no chains.** A pull request starts four jobs at the same time: `lint` (format, clippy, `cargo-deny`, dependency rules), `test-linux` (T0 to T2), `test-macos` (T0 to T2) and `qemu-x86_64` (T3 with KVM). No job waits for a different job.
- **No image build in a pull request.** The `qemu-x86_64` job downloads the base image that `main` published, overlays the new Rust binaries and boots it. A full Buildroot build runs only when a file under `os/` changes, and in the nightly run.
- **Path filters.** A change to documents only runs `lint` only. A change that does not touch `os/` or the device crates skips `qemu-x86_64`.
- **Caches.** The Cargo registry and `target/` are cached with a key from `Cargo.lock` and the toolchain. Only runs on `main` save the cache; pull requests only read it. The Buildroot download directory and compiler cache are cached in the same way. CI sets `CARGO_INCREMENTAL=0`, because incremental data makes the cache large and gives no benefit in a clean runner.
- **Same profile as local.** Pull request jobs use the dev profile and the headless feature set. The release profile builds only in the nightly and release runs.
- **Prebuilt tools.** `cargo-nextest`, `cargo-deny` and the linker install as binaries. CI never runs `cargo install`.
- **Cancel old runs.** A new push to a branch cancels the run in progress for that branch.
- **During the MVP run.** The run is local and opens no pull requests (run rule 15). CI runs for the first time on the push of `main` when the MVP is finished, with the same four jobs.

**Definition of done, to copy into `CLAUDE.md`**

1. Every behaviour has a test, written with the code. A bug fix starts with a test that fails.
2. `cargo xtask verify` passes at the tier the work package names. Paste the report summary into the pull request.
3. A visual change comes with a scene and a golden. Open the screenshot and look at it before blessing it.
4. A new setting is a schema entry, and the crawler finds it.
5. Any code that can change the display goes through `ValidatedMode` and has a test.
6. No sleeps in tests: use the simulated clock or `wait`. No network in tests: use the mock server.
7. A change that emulation cannot verify adds a line to `docs/hardware-checklist.md` and labels the pull request `needs-hardware`.
8. Never bless a golden, loosen a tolerance or skip a test to get a green run. Stop and report what failed instead.

**Code and comment rules, to copy into `CLAUDE.md`.** The code is written without human review, so it has to be readable by a human who arrives later.

- Every public item has a doc comment: what it does, its units, its errors, when it can panic. Each crate sets `#![deny(missing_docs)]`.
- Other comments exist for three reasons only: a limit and where its number comes from, an invariant the code relies on, or a reason that the code cannot show. A comment never restates the code, narrates a change, or decorates.
- No `TODO` without the id of an open point in `docs/open-points.md`.
- Every `unsafe` block has a `// SAFETY:` comment stating why it is sound. Clippy's `undocumented_unsafe_blocks` lint enforces it.
- Safety-critical code stays small and plain: `neptron-safety`, the hard rails, and the commit path in `neptron-kms`. No macros, no clever generics, no hidden defaults. Every limit is a named constant with its unit in the name.
- `neptron-safety` must be readable end to end in one sitting: a budget of about 500 lines, tests excluded. Growing past it needs your approval.
- T0 enforces what a lint can enforce; the rest is part of the task brief's acceptance check.

**What the agent cannot sign off alone:** changed goldens, changes to safety limits or monitor profiles, kernel patches, and anything labelled `needs-hardware`. Those need a human approval in review. During the MVP run the review agent gives that approval, under the rules of the run.
