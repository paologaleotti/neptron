# 0004: The UI font is Spleen under BSD-2-Clause

- Status: accepted
- Date: 2026-11-11
- Task: T07

## Context

The UI draws text on a 320x240 canvas with a bitmap font. The project owner chose an existing, well-known font instead of a font drawn for the project. The NEPTRON source is `MIT OR Apache-2.0`. A licence that differs from it is a licence decision (run rule 18).

## Decision

- The font is Spleen by Frederic Cambus, release 2.2.0 (commit `0493c34e22791824767c618fed42c434b477662c`), size 6x12, licence BSD-2-Clause.
- The unchanged BDF file and the upstream `LICENSE` are in `crates/neptron-ui/assets/fonts/spleen/`, with a README that gives the source, the SHA-256 of each file and how to refresh them.
- `cargo xtask font` generates `crates/neptron-ui/src/font/spleen_6x12.rs` from the BDF file. Its header holds the copyright notice and the full licence text, as the licence asks for a redistribution in source form.
- `crates/neptron-ui/Cargo.toml` declares `license = "(MIT OR Apache-2.0) AND BSD-2-Clause"`. `deny.toml` already allows BSD-2-Clause.
- The licence manifest of every image must list `crates/neptron-ui/assets/fonts/spleen/LICENSE`, because the licence asks a binary distribution to reproduce the notice.

## Consequences

- Every binary that contains `neptron-ui` (the `neptron` app) carries BSD-2-Clause code. The notice must ship with the image (T28 licence manifest).
- A new font release replaces the vendored files and the generated table in one change, and the goldens change with it.
- The font cannot drift from its source: a test fails when the committed table differs from the generator output.

## Alternatives

- A font drawn for the project, under `MIT OR Apache-2.0`. It has no licence question, but the owner preferred a known, maintained font.
- The mono fonts of `embedded-graphics`. They come with a crate that the UI does not use for drawing, and that crate adds compile time.
