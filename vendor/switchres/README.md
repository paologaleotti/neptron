# Switchres calculation sources

This directory holds a plain copy of the Switchres files that calculate modelines. `crates/neptron-switchres-sys` compiles them. Do not change these files. A fix or an adaptation goes in the shim of `neptron-switchres-sys`.

## Origin

- Project: https://github.com/antonioginer/switchres
- Commit: `09272ac24640daee45f5be15b1b856ecd220dda2` (the same commit as the presets in `data/monitors`)
- Archive: https://codeload.github.com/antonioginer/switchres/tar.gz/09272ac24640daee45f5be15b1b856ecd220dda2

## Files

| File | What it holds | SHA-256 |
| --- | --- | --- |
| `modeline.cpp` | Modeline generation, scoring and adjustment | `c127401763d2213092a43f99de8ff60e6d7ff2c88118c5d832049e6fdcc8f9fc` |
| `modeline.h` | The `modeline` and `generator_settings` types | `0ec17a5e57c9a86d3d34b7cd58c2da617d92bd1832d669ceb23397560312c2e7` |
| `monitor.cpp` | The preset table and the range check | `2806a80beaabad785ac045e8160f4546c7a4455c1762169f0c825304769f2c24` |
| `monitor.h` | The `monitor_range` type | `f6203485577b767c1fa927ebf2367b658d008965d16e0ccfc0612eee5dc80238` |
| `log.cpp` | The log function pointers | `547779c27d3af804cd442698c3a82659d35e5f83ad7ebf85bba0dc611655e6a1` |
| `log.h` | The log declarations | `eb28315fa8f7a60ef7672d4505927b5555b57c8f37bbc2b87de56bbfece6af4c` |
| `LICENSE` | The text of the GNU GPL, version 2 | `edaef632cbb643e4e7a221717a6c441a4c1a7c918e6e4d56debc3d8739b233f6` |

The files are byte for byte as upstream, with their CRLF line ends. `.gitattributes` turns off line-end conversion for this directory.

The other upstream files are not copied. They hold the display backends (Windows, X11, DRM/KMS, SDL2), the mode manager, the C wrapper, the EDID writer and the standalone program. NEPTRON compiles none of them.

## Licence

Switchres is licensed GPL-2.0-or-later. Each source file states this in its header, and the upstream `README.md` states it in its "License" section. The upstream tree at this commit has no licence file, so `LICENSE` here is the text of the GPL version 2, the same text as `data/monitors/LICENSE`. The copyright holders are Chris Kennedy, Antonio Giner, Alexandre Wodarczyk and Gil Delescluse.

This directory is licensed GPL-2.0-or-later. The rest of the NEPTRON source stays `MIT OR Apache-2.0`. `docs/adr/0002-switchres-link-licence.md` records what the link means for the binary.

## Refresh

1. Pick the new commit. Get its archive from the URL above, with the new commit in place of the old one.
2. Copy the six source files over the files here. Do not change them.
3. Check that the calculation sources still include only each other and the C and C++ standard headers. A new include of a display backend needs a decision first.
4. Update the commit and the SHA-256 column in this file (`shasum -a 256 *`).
5. Convert the presets in `data/monitors` from the same commit (`cargo xtask monitors`).
6. Run `cargo xtask test -p neptron-modeline`. The golden test shows each modeline that changed. Check each change against the upstream standalone program, as `crates/neptron-modeline/README.md` describes, before you update the goldens.
