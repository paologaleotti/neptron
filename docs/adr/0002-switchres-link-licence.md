# 0002: Licence of the link with Switchres

- Status: accepted
- Date: 2026-11-11
- Task: T05

## Context

NEPTRON calculates modelines with Switchres, linked as a library (Definition, "Technology choices"). Switchres is licensed GPL-2.0-or-later. The NEPTRON source is licensed `MIT OR Apache-2.0`. `neptron-switchres-sys` compiles a vendored copy of the Switchres calculation sources (`vendor/switchres`) with the `cc` crate and links the result statically into every binary that depends on `neptron-modeline`.

The presets in `data/monitors` come from the same Switchres commit. That directory is already licensed GPL-2.0-or-later (OP-007).

## Decision

1. `vendor/switchres` holds the unchanged Switchres files with the text of the GPL version 2. The directory is licensed GPL-2.0-or-later.
2. The source of `neptron-switchres-sys` (the build script, the C++ shim and the Rust code) and of `neptron-modeline` stays `MIT OR Apache-2.0`. Both crates declare that licence in `Cargo.toml`. The shim is new code: it calls Switchres and copies no Switchres code.
3. A binary that links `neptron-switchres-sys` contains Switchres and is a combined work. It is distributed under GPL-3.0-or-later, as the root `README.md` states. GPL-2.0-or-later allows GPL-3.0, and Apache-2.0 is compatible with GPL-3.0 but not with GPL-2.0 alone.
4. Every distribution of such a binary (the OS image, a release archive) carries the corresponding source, or a written offer for it. The licence manifest of the image names Switchres with its commit.
5. `cargo-deny` checks the licences of crates only. It does not see `vendor/switchres`. This ADR and `vendor/switchres/README.md` are the record for that directory.

## Consequences

- The NEPTRON source stays permissive. A person can take a crate that does not link Switchres (for example `neptron-safety` or `neptron-ui`) under `MIT OR Apache-2.0` alone.
- The `neptron` binary and the images are GPL-3.0-or-later. A proprietary fork of the frontend is not possible while it links Switchres.
- The licence manifest of the image must list Switchres. The image task must check this.
- A future change that replaces Switchres with a Rust port written from the monitor specifications removes the GPL from the binary. The port must not be a translation of the Switchres code.

## Alternatives

- Load Switchres as a dynamic library at run time: rejected. A dynamic link to a GPL library still makes a combined work in the view of the FSF, and it adds a library to the image and a failure mode at start.
- A Rust port of the Switchres maths: rejected for now. The Definition chooses Switchres because real monitors have tested it, and a port would have to earn that trust again. A translation of GPL code is also a derived work.
- License the whole NEPTRON source as GPL: rejected. Most crates do not need Switchres, and a permissive licence keeps them reusable.
