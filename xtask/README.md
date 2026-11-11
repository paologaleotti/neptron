# xtask

Build, verification and worktree automation for the NEPTRON workspace. Run it as `cargo xtask <command>`. All automation lives here, not in shell scripts.

## Commands

- `cargo xtask check [cargo args]`: `cargo check` under the machine-load rules.
- `cargo xtask test [nextest args]`: `cargo nextest run` under the machine-load rules.
- `cargo xtask clippy [cargo args]`: `cargo clippy` under the machine-load rules.

## Machine-load rules (run rule 13)

On macOS, every Cargo command that xtask starts runs under `taskpolicy -c utility`. A build holds one of the build slots. The lock files are in `<git common dir>/neptron-build-slots/`, so all worktrees use the same slots. A build that finds no free slot waits.

| Variable | Default on macOS | Default on other hosts |
| --- | --- | --- |
| `NEPTRON_BUILD_SLOTS` | 2 | no lock |
| `NEPTRON_BUILD_JOBS` (sets `CARGO_BUILD_JOBS`) | 5 | Cargo default |

Cargo compiles `xtask` itself before xtask starts, so that one small compile runs outside `taskpolicy` and the slots.

## Dependencies

- `thiserror`: each module owns its error enum, as the code standards ask.

Arguments are parsed by hand, and the slot lock uses `std::fs::File::try_lock`, to keep the compile time low.

## Must never depend on

- Any NEPTRON crate except `neptron-ui` (for `shot`, `gallery` and `golden`).
- Heavy crates: every dependency must justify its compile time.

## Rules

- `unsafe` is forbidden: the crate root carries `#![forbid(unsafe_code)]`.
