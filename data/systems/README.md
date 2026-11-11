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

The format is append-only. A new key gets a default, or a new `format` value. A key is never renamed or removed. The parser rejects an unknown key, so a typing error cannot pass.

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
