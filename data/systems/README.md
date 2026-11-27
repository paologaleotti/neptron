# System definitions

One TOML file per system. `neptron-library` embeds these files at build time and checks them at start. A system shows in the UI only when its folder holds games.

## Format

| Key | Type | Meaning |
| --- | --- | --- |
| `format` | integer | Format version of the file. The only version is `1`. |
| `folder` | string | Folder name under `share/roms/`. It is also the system id. |
| `aliases` | list of strings | Other folder names that hold games of this system. |
| `name` | string | Display name. |
| `extensions` | list of strings | File extensions of games, lower case, with no dot. The scan compares them with no regard to ASCII case. |
| `always_progressive` | boolean, optional, default `false` | Every frame of the system is progressive. The line count of a frame does not make it interlaced. An arcade board draws progressive frames, also tall ones. |
| `discs` | list of strings, optional, default empty | The disc formats of the system: `chd`, `cue` and `m3u`. Each one is also in `extensions`. `docs/spec/disc-games.md` gives the rules. |
| `bios` | list of tables, optional, default empty | The BIOS files that the core needs. Each table has `file` (the name that the core opens), `region` (optional: `ntsc-j`, `ntsc-u` or `pal`), `size` (bytes, 1 to 16 MiB) and `crc32` (a list of 8 lower-case hex digits). NEPTRON finds a file by its size and CRC-32, not by its name (`docs/spec/disc-games.md`, section 6). |
| `widths` | list of integers, optional, default empty | The widths in pixels that the games of the system draw in play with no new sync (the same lines and line timing), the most used first. Two to 8 different widths from 1 to 4096. A system with this key gets one wide mode on a CRT, so a change of the width needs no mode change (`docs/spec/interlaced-systems.md`, section 3). A system with no key keeps one mode per width. |

The format is append-only. A new key gets a default, or a new `format` value. `discs`, `bios` and `widths` came after the first version with a default, so `format` stays `1`. A key is never renamed or removed. The parser rejects an unknown key, so a typing error cannot pass.

## Sources

Folder names and aliases come from two projects, read on 2026-11-11:

- RetroPie-Setup, commit `427e76216633591861791cfb28807ac1039c5ea8`:
  - `platforms.cfg` names the systems `nes`, `snes` and `megadrive`, and gives the `_exts` lists.
  - `scriptmodules/helpers.sh`, function `mkRomDir`, makes a `genesis` link to `megadrive`.
- Batocera, commit `26a5315773250e736d1e9884a190dc43c425ee2e`:
  - `package/batocera/emulationstation/batocera-es-system/es_systems.yml` has the keys `nes`, `snes` and `megadrive`. Its `megadrive` entry lists the platforms `genesis, megadrive`.

Neither project has a separate folder for Famicom or Super Famicom.

The extension list of a system is the intersection of two lists, plus `zip`. The two lists are the RetroPie `_exts` list and the `valid_extensions` of the core that runs the system (see `data/cores/README.md`). A `.zip` file is one game: the core host reads the ROM from the archive at launch. RetroPie lists `.zip` for all three systems. `.7z` is not in the lists, because NEPTRON does not read `.7z` archives.

`zip` was added to the lists after the first version of these files. A new value in a list is an addition, so `format` stays `1`.

| System | RetroPie `_exts` | Core `valid_extensions` | Result |
| --- | --- | --- | --- |
| `nes` | `.7z .nes .zip` | `fds nes unf unif` | `nes zip` |
| `snes` | `.7z .bin .bs .smc .sfc .fig .swc .mgd .zip` | `smc sfc swc fig bs st` | `bs fig sfc smc swc zip` |
| `megadrive` | `.7z .smd .bin .gen .md .sg .zip` | `m3u mdx md smd gen bin cue iso chd bms sms gg sg 68k sgd` | `bin gen md sg smd zip` |

The display names are the RetroPie `_fullname` values, except for `megadrive`, which takes the Batocera name "Mega Drive".

## Arcade

`arcade` and `neogeo` hold FinalBurn Neo ROM sets. `docs/spec/arcade.md` (sections 3 and 4) gives the decision and its sources:

| System | Aliases | Extensions | Core |
| --- | --- | --- | --- |
| `arcade` | `fbneo` (the Batocera folder), `fba` (the RetroPie folder) | `zip` | `fbneo` |
| `neogeo` | none | `zip` | `fbneo` |

- A set is a `.zip` file, and its file name is the set name of FinalBurn Neo. The core opens the zip itself and finds the parent set and the BIOS set by name.
- `.7z` is not accepted: NEPTRON has no `.7z` reader to check a set.
- `mame`, `mame-libretro`, `mame2003` and `mame2003-plus` are not system folders. MAME sets need a MAME core, and NEPTRON has none yet.
- Both systems set `always_progressive`: an arcade board draws progressive frames, also tall ones.
- The scan hides a BIOS set of the core (for example `neogeo.zip`). The list is in `crates/neptron-library/src/catalog.rs`.

## Widths in play

The real console changes its width with no new sync: the dot clock changes, the line timing stays. NEPTRON follows it with one mode for every width (`docs/spec/interlaced-systems.md`, section 3).

| System | `widths` | The console | The core |
| --- | --- | --- | --- |
| `snes` | `256, 512` | The low resolution modes and the high resolution modes 5 and 6 (pseudo hires too) share the line timing of 341 dots. | Snes9x sends each frame at 256 or 512 pixels with no geometry event; the size of the frame tells the change. |
| `megadrive` | `320, 256` | H40 and H32 share the line timing of 3420 master clocks. | Genesis Plus GX sends each frame at 320 or 256 pixels (with the overscan option `disabled`) and reports a new geometry (`tests/traces/genesis_plus_gx.trace`: a frame of 256x192 at load, then 320x224 with a geometry event). |

`nes`, `neogeo` and `arcade` draw one width per game and have no key.
