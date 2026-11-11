# Test ROMs

The 240p Test Suite builds for NES, SNES and Mega Drive. The tier T4 check (`cargo xtask core-trace`) runs each one in its real core. These files are test data: no NEPTRON image contains them.

Only the NES build is in the repository. The SNES and Mega Drive builds stay outside it, in a cache directory on each machine (see below).

## Manifest

Downloaded on 2026-11-11. `SHA256SUMS` holds the hash of all three files. `cargo xtask core-trace` checks each file against it, in the repository and in the cache.

| File | Build | Bytes | SHA-256 | Where |
| --- | --- | --- | --- | --- |
| `nes/240pee.nes` | 240p Test Suite for NES (`240pee`), version 0.23, by Damian Yerrick | 65,552 | `04f01d7372f66ea2befe325b9bd655a9fcc395a31fa46d9466286bb8f9d2e62e` | This folder |
| `snes/240pSuite.sfc` | 240p Test Suite for SNES, version 1.10, NTSC, by Artemio Urbina | 786,432 | `8c3f6028bb1246d2a4c5a695f9fb7b237b73dbb7c8242f042f11d8a85476cef8` | The cache, as `240pSuite.sfc` |
| `megadrive/240pSuite-1.32.bin` | 240p Test Suite for Mega Drive, version 1.32, the regular ROM (not `_emu`), by Artemio Urbina | 262,144 | `fe448a879eda25daffa1548385aff464d5373c687db0cccf18f071c31d0976c7` | The cache, as `240pSuite-1.32.bin` |

## Why two ROMs are not in the repository

The SNES and Mega Drive builds contain art of other owners: the scroll test backgrounds come from Sonic the Hedgehog (Sega) and Kiki Kaikai (Taito), and the "Donna" pictures have no stated licence. Both ROMs hold the text "Sonic is a trademark of Sega Enterprises Ltd. Kiki Kaikai is a trademark of Taito." The author licenses his code under the GPL, but he cannot license that art. The licence of these two files is therefore not clear, and the owner has not decided about them. Until the owner decides, they are not committed.

The NES build has its own art and a tune under CC BY that the ROM credits. It is GPL-2.0-or-later, so it is in this folder.

## The cache

`cargo xtask core-trace` looks for a ROM that is not in this folder by its file name in the cache directory: `~/.cache/neptron/test-roms` (`NEPTRON_CACHE_DIR` moves `~/.cache/neptron`; `NEPTRON_TEST_ROMS` names another directory). A missing file, or a file with another SHA-256, skips its two cases and fails the run. The command prints which file is missing and where to put it.

To fill the cache:

1. Open `https://artemiourbina.itch.io/240p-test-suite`, the official download page of the author. The downloads are free.
2. Download "240p test Suite for SNES 1.10" (`240pTestSuite-SNES-1.10.zip`, 254,068 bytes, SHA-256 `880696689fa9bfdf395cc47dd4be8a312192f63594f249382945e31f00db7b0c`) and "240p test Suite for MegaDrive/Sega Genesis 1.32" (`240pTestSuite-MD_Genesis-1.32.zip`, 172,633 bytes, SHA-256 `0db29b9739cc75c44dcbf2f3fa74608fff57cf05835e88cf28ffeb1faa4b7970`).
3. Extract `240pSuite.sfc` from the first archive and `240pSuite-1.32.bin` from the second, and put both in the cache directory.

itch.io serves only the newest upload of each build. When the author publishes a newer version, these exact files are no longer available there, and the hashes above, the traces in `tests/traces` and the case table change together in a new task.

## Sources

| File | Downloaded from | Source code |
| --- | --- | --- |
| `nes/240pee.nes` | `https://github.com/pinobatch/240p-test-mini/releases/download/v0.23/240pee.nes`, the release asset of tag `v0.23` | `https://github.com/pinobatch/240p-test-mini`, tag `v0.23`, commit `019a524b6b042c1ea1c2e8414e79f152d5997ab3` |
| `snes/240pSuite.sfc` | `https://artemiourbina.itch.io/240p-test-suite`, upload of 2026-09-26 | `https://github.com/ArtemioUrbina/240pTestSuite`, directory `240psuite/SNES`. Commit `89adfdd66853746fa9db9105104314ac58021791` (2026-09-26) is the nearest to the build |
| `megadrive/240pSuite-1.32.bin` | `https://artemiourbina.itch.io/240p-test-suite`, upload of 2026-04-03 | `https://github.com/ArtemioUrbina/240pTestSuite`, directory `240psuite/Genesis`. Commit `ff52715155e04a28e89cb7babb890396b904b7e6` (2026-04-03, "Updated release date and version") is the nearest to the build |

## Licence of the NES ROM

`nes/240pee.nes` is GPL-2.0-or-later, from the `README.md` of `pinobatch/240p-test-mini` at `v0.23`. `COPYING` is the text of the GPL version 2 and applies to this file. The corresponding source is the tag `v0.23` of the upstream repository, named in the table above; this folder passes on that source information as the upstream project gives it.
