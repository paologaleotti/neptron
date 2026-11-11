# Core definitions

One TOML file per libretro core. `neptron-library` embeds these files at build time and checks them at start.

## Format

| Key | Type | Meaning |
| --- | --- | --- |
| `format` | integer | Format version of the file. The only version is `1`. |
| `id` | string | Core id, equal to the file name. |
| `name` | string | Display name, equal to the core's `library_name`. |
| `library` | string | File name of the core's shared library. |
| `systems` | list of strings | Systems that the core runs, by the `folder` of `data/systems`. |
| `licence` | string | SPDX licence expression of the core. A licence with no SPDX id uses a `LicenseRef-` name. |

The format is append-only. A new key gets a default, or a new `format` value. A key is never renamed or removed. The parser rejects an unknown key. Each system has exactly one core.

## Sources

Read on 2026-11-11, from each core's `master` branch at the commit below. The Buildroot packages pin their own commits; when a pin changes, check the library name, the extensions and the licence again.

| Core | Repository and commit | `library_name` and `valid_extensions` | Licence |
| --- | --- | --- | --- |
| `fceumm` | `libretro/libretro-fceumm`, `e0f3086a8104f8fbbe49a2615b10401504a9533f` | `src/drivers/libretro/libretro.c`: `FCEUmm`, `fds\|nes\|unf\|unif` | GPL version 2 "or (at your option) any later version", from the file headers, for example `src/fceu.c` |
| `snes9x` | `libretro/snes9x`, `fae2fea08f74180759ef540ee94259213f503480` | `libretro/libretro.cpp`: `Snes9x`, `smc\|sfc\|swc\|fig\|bs\|st` | The Snes9x licence in `LICENSE`: non-commercial use only |
| `genesis_plus_gx` | `libretro/Genesis-Plus-GX`, `b9c2316441978d4015ba30e9a95d9b0e6f8ee981` | `libretro/libretro.c`: `Genesis Plus GX`, `m3u\|mdx\|md\|smd\|gen\|bin\|cue\|iso\|chd\|bms\|sms\|gg\|sg\|68k\|sgd` | The Genesis Plus GX licence in `LICENSE.txt`: no sale, no commercial use |

The library file names follow the libretro convention `<core>_libretro.so`. RetroPie uses the same names, for example `genesis_plus_gx_libretro.so` in `scriptmodules/libretrocores/lr-genesis-plus-gx.sh`.
