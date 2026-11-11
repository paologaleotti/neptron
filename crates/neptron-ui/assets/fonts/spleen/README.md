# Spleen

The UI font: Spleen by Frederic Cambus, size 6x12, unchanged.

| Item | Value |
| --- | --- |
| Source | https://github.com/fcambus/spleen |
| Version | Release tag `2.2.0`, commit `0493c34e22791824767c618fed42c434b477662c` |
| Licence | BSD-2-Clause, in `LICENSE` (the upstream file, unchanged) |
| `spleen-6x12.bdf` SHA-256 | `fc0743d164690f99b7e2e1b9d503180e4c719a9831ae03fd8f6da18c857dee27` |
| `LICENSE` SHA-256 | `f33fe8679d5b2abecc4f1313ce6c6bfa58262964de5f7bca146596a7318047af` |

The licence asks that a binary distribution reproduces the copyright notice and the licence text. The licence manifest of the image must include `LICENSE`.

## How the font gets into the code

`cargo xtask font` reads `spleen-6x12.bdf` and writes `src/font/spleen_6x12.rs`, the glyph table, for printable ASCII and the Latin-1 Supplement (U+0020 to U+007E and U+00A0 to U+00FF). Do not edit the generated file. A test in `xtask` fails when the committed table differs from what the generator makes from this file.

## How to refresh

1. Download `spleen-6x12.bdf` and `LICENSE` of the new release tag from the upstream repository into this directory.
2. Update the tag, the commit and the SHA-256 values in this README (`shasum -a 256 <file>`).
3. Run `cargo xtask font`, then `cargo xtask golden`. Look at every changed scene with `cargo xtask shot <scene>` before `cargo xtask golden --bless`.

To use a different size, change the source and the output in `xtask/src/ui/font.rs` and the module name in `src/font.rs`.
