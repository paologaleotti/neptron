# Arcade

This spec defines arcade games in NEPTRON 0.1: the core, the folders, the game list, the check of a ROM set before launch, the changes to the core host, vertical games, the rates, the controls and the tasks that build it.

The owner decided the main points on 2026-11-16. The research for this spec read the FinalBurn Neo (FBNeo) source at `libretro/FBNeo` master `d7450cf1` (2026-11-15), the RetroPie and Batocera sources, and the libretro documentation. A statement about FBNeo in this spec comes from that reading unless it carries a mark:

- **UNVERIFIED**: nobody has checked it. A task must not rely on it without its own check.
- **trace**: the core trace of M07b confirms it. Until then it is the reading of the source.

## 1. Scope

### In the MVP

- One arcade core: FBNeo, the full build, on both images (PC x86_64 and Raspberry Pi aarch64). ADR 0014 records the core and its licence.
- Two system folders, `arcade` and `neogeo`. Both run FBNeo.
- ROM sets as `.zip` files only.
- A game list from an index that NEPTRON makes from the DAT of the pinned FBNeo commit: titles, BIOS sets hidden, clones folded under their parent, a mark for vertical games.
- A check of the set and its BIOS before launch, with NEPTRON's own messages.
- Vertical games turned upright by NEPTRON's renderer (ADR 0015).
- Initial core option values applied when the core declares its options during load (ADR 0016).
- The present rate policy for every display.
- Board families named as tested: Neo Geo, CPS-1, CPS-2 and Cave 68000, after the bench session. Other FBNeo boards are "may work".

### After the MVP

- MAME 2003-Plus as the second core, in a `mame` folder. Then a core setting per game. Then the choice of core by checksum.
- TATE: the screen on its side, with the UI turned (section 15).
- Save states, `hiscore.dat`, samples.
- `.7z`, CHD, Neo Geo CD.
- The Modern and 6-Panel layouts, light guns, trackballs, keyboard encoders.
- Filters by orientation and by player count. The DAT has no player count.
- A "keep game speed" choice for a display that cannot show the game's rate.
- The FBNeo DAT on the share for users who rebuild their sets.
- A "check all arcade sets" job in the diagnostics.
- The `neogeo` and `cps12` subset builds as a size option for the Pi image.

## 2. Board families and the recommended core

| Board family | PC | Raspberry Pi 3 |
| --- | --- | --- |
| Neo Geo MVS | FBNeo | FBNeo, if the measurement passes |
| CPS-1, CPS-2 | FBNeo | FBNeo, if the measurement passes |
| Cave 68000 (DoDonPachi and others) | FBNeo | FBNeo, if the measurement passes |
| Toaplan, Psikyo, Taito F3, Irem, Data East and other 2D boards | FBNeo ("may work") | FBNeo ("may work") |
| CPS-3, Cave CV1000 (SH-3) | FBNeo ("may work") | Not expected at full speed (research judgement, UNVERIFIED) |

The Pi 3 column is not a claim. NEPTRON's Pi image is aarch64. The FBNeo makefile does not set `USE_CYCLONE` for the `rpi3_64` platform, so the 68000 runs on the Musashi C core, not the Cyclone ARM32 dynarec. No published measurement of FBNeo on an aarch64 Pi 3 exists for Neo Geo or Cave games. The speed on the Pi 3 is measured in a separate task that runs now. **The Pi 3 board list in the release notes and on the About screen follows that measurement and nothing else.** A family that fails it is "may be slow" on the Pi 3. MAME 2003-Plus is the candidate for such a family after the MVP.

## 3. Folders and aliases

| Folder | Aliases | Core | Extensions |
| --- | --- | --- | --- |
| `share/roms/arcade` | `fbneo` (the Batocera name), `fba` (the RetroPie name) | `fbneo` | `zip` |
| `share/roms/neogeo` | none | `fbneo` | `zip` |

- The folder gives the core. A system has exactly one core, as `data/cores/README.md` requires.
- `.7z` is not accepted. NEPTRON has no `.7z` reader for the set check.
- `mame`, `mame-libretro`, `mame2003` and `mame2003-plus` stay unknown folders in the MVP. The scan lists an unknown folder in `ScanReport::unknown_folders` today. For these four names the diagnostics page shows this line:

  > `<folder>`: MAME sets need a MAME core. This version has no MAME core. Put FinalBurn Neo sets in `arcade` or `neogeo`.

### Growth path

1. MAME 2003-Plus runs the `mame` folder, as on Batocera.
2. A `core` setting at the game and system levels picks the core. The settings levels exist today.
3. The set check tries the index of each core and takes the core whose CRCs match. The setting overrides it.
4. A merged "Arcade" list over every arcade folder.

## 4. The data files

`data/systems/arcade.toml`, `data/systems/neogeo.toml` and `data/cores/fbneo.toml` are not in this task. The existing code does not accept them:

1. `Catalog::builtin()` embeds a fixed list of files with `include_str!` (`crates/neptron-library/src/catalog.rs`). A new file in `data/` is not read and not checked until the list names it. That is a code change.
2. `check_option_key` accepts ASCII letters, digits and `_` in a core option key. FBNeo keys have `-` (`fbneo-diagnostic-input`). The parser refuses the core file.
3. The test `builtin_core_options_are_announced_by_the_cores` checks each default option against the `option` lines of a core trace. `core_trace` writes those lines only for options that the core declares in `Host::open`. FBNeo declares its options in `retro_load_game`, so its trace has no `option` line yet.
4. The `always_progressive` key (section 9) is unknown to the parser, which rejects an unknown key.
5. Every system must have exactly one core, so the system files and the core file must arrive together.

**M07b adds the three files**, with the embed list, `-` in option keys, the `always_progressive` key and the FBNeo trace. It needs M07a for the trace (section 8.4). The proposed contents:

```toml
# data/systems/arcade.toml
format = 1
folder = "arcade"
aliases = ["fbneo", "fba"]
name = "Arcade"
extensions = ["zip"]
always_progressive = true
```

```toml
# data/systems/neogeo.toml
format = 1
folder = "neogeo"
aliases = []
name = "Neo Geo"
extensions = ["zip"]
always_progressive = true
```

```toml
# data/cores/fbneo.toml
format = 1
id = "fbneo"
name = "FinalBurn Neo"
library = "fbneo_libretro.so"
systems = ["arcade", "neogeo"]
licence = "LicenseRef-FBNeo"

[options]
fbneo-diagnostic-input = "Start + L + R"
fbneo-vertical-mode = "disabled"
```

`name` must equal the core's `library_name`, and each option value must be one that the trace lists. `name` is UNVERIFIED until M07b reads the trace. Both option values are in the option table of `src/burner/libretro/retro_common.cpp` (read at master on 2026-11-16); the trace of the pin confirms them. Section 13 gives the reason for `fbneo-diagnostic-input`. `fbneo-vertical-mode = "disabled"` is the core's own default (checked in `src/burner/libretro/retro_common.cpp`); NEPTRON pins it so that the turns of section 11 do not depend on a later change of that default.

## 5. How a game gets its core

In the MVP: from its folder, through `data/systems` and `data/cores`. No setting and no check of the set chooses another core. Section 3 has the growth path.

## 6. The arcade index

### What it holds

For each set of the DAT of the pinned FBNeo commit (`FinalBurn Neo (ClrMame Pro XML, Arcade only).dat` in `dats/`):

| Field | From the DAT | Use |
| --- | --- | --- |
| name | `name` | The key: the zip file stem |
| parent | `cloneof` | Folding, the set check |
| romof | `romof` | The BIOS of the set, through the chain |
| BIOS | `isbios="yes"` | Hidden in the list |
| title | `description`, less its last bracket group | The list |
| details | the last bracket group of `description` | The details line |
| year, maker | `year`, `manufacturer` | The details line |
| board | `sourcefile` | Diagnostics, the board list |
| orientation | `video/@orientation` | The vertical mark |
| width, height | `video/@width`, `video/@height` | Diagnostics |
| status | `driver/@status` | A "not working" mark |
| ROMs | each `rom`: `name`, `size`, `crc`, `merge`, `status` | The set check |

Example, with an invented description: "Star Patrol - Mission One (XYZ-0001 ~ XYZ-0002)" gives the title "Star Patrol - Mission One" and the details "XYZ-0001 ~ XYZ-0002". A description with no bracket group has no details.

The research reports these DAT attributes and the absence of device sets, of a refresh rate and of a player count. M07c checks them against the DAT of the pin.

### The file format

The index is UTF-8 text, one record per line, fields split by one tab (`\t`). Text keeps the file readable in a review and easy to test. About 8,500 sets and their ROMs give a few MiB; NEPTRON reads it once, off the frame thread.

```text
neptron-arcade-index	1
core	fbneo	<40-hex commit>
dat	<DAT file name>	<DAT version>
licence	FBNeo licence, see /usr/share/licenses/libretro-fbneo/license.txt
set	<name>	<parent or ->	<romof or ->	<flags>	<year>	<maker>	<board>	<orientation>	<width>	<height>	<title>	<details>
rom	<name>	<size>	<crc>	<merge or ->	<flags>
rom	...
set	...
```

- Line 1 is the magic and the format version. A reader refuses another magic or a higher version with an error that names both.
- `set` lines are sorted by name in byte order. The `rom` lines of a set follow its `set` line in DAT order.
- Set flags, in this order, `-` for none: `b` BIOS, `v` vertical, `n` not working, `p` preliminary.
- `orientation` is `h` or `v`. `width` and `height` are as the DAT gives them.
- ROM flags: `-`, or `d` for `status="nodump"`. A `nodump` ROM has no CRC (`-`) and the check skips it.
- `crc` is 8 lower-case hex digits. `size` is decimal bytes.
- A field never holds a tab or a line break. The generator stops with an error that names the set and the field if the DAT has one.
- The format is append-only: a new field goes at the end of its line, and a reader ignores fields after the ones it knows. A new kind of line gets a new first field, and a reader skips a line whose first field it does not know. A field is never removed or moved. A change that an older reader would misread raises the version.

### How it is made

- `cargo xtask arcade-index <dat> <out>` reads the DAT and writes the index. The DAT parser lives in xtask only, so the image has no XML code. It needs an XML reader in xtask: a new external dependency. The orchestrator adds it to `[workspace.dependencies]` of the root `Cargo.toml` in a `build(deps)` commit (`roxmltree`, MIT or Apache-2.0, no dependencies, is the proposal); M07c adds it to `xtask/Cargo.toml` and the command to `xtask/src/main.rs`.
- The encoder and the reader of the format live in `neptron-library` (module `arcade`), so xtask writes with the code that the frontend reads with. xtask already depends on `neptron-ui` in the same way.
- The tests use synthetic DAT text written for the test: invented set names, sizes and CRCs. No ROM, no BIOS and no DAT excerpt goes into the repository.

### How it is installed

- The image build makes the index from the DAT in the source tarball of the pinned `libretro-fbneo` package, the same tarball that Buildroot hashes. The default: a post-install hook of `libretro-fbneo` runs the generator as a host tool. M07c may choose another step with the same result, and records the choice in the package's `.mk` comment.
- Path in the image: `/usr/share/neptron/arcade/fbneo.idx` (`Paths::data`).
- The index ships under the FBNeo licence. M07b adds its line to `os/board/*/LICENCES.txt` with the core's own lines.
- A pin change of the core makes a new index in the same build, so the index and the core always agree.

### How it is read

- The menu process reads the index once, on the thread that runs the library scan (`neptron-scan`), and keeps it for the life of the process. The frame thread never reads it.
- A game run does not load the index. The set check runs in the menu process, before the game run starts (section 10).
- A missing or damaged index never stops the boot. The arcade folders then list each zip by its file name with no mark, the set check reports "no index", and the diagnostics page says why.

## 7. The scan

The scan rules of `crates/neptron-library/README.md` hold. The arcade systems add a lookup by name. The scan never opens a zip.

| Zip stem in the index | Shown |
| --- | --- |
| A parent | One entry, by title |
| A clone whose parent zip is in the same list | Folded under the parent. The parent's entry gives "Other versions (N)" |
| A clone whose parent zip is not in the list | Its own entry, by title |
| A BIOS (`b` flag) | Hidden. Counted on the diagnostics page |
| Not in the index | Its own entry, by file name, with the mark "unknown set" |

- The lookup ignores ASCII case, as exFAT does. The DAT names are lower case.
- A vertical set gets a mark in the list. A set with the `n` flag gets the mark "not working".
- The list sorts by title. The title, the parent, the flags and the details go into the library index and its cache, because a query does no I/O. The cache format gets a new version for this. A cache of the old version is rebuilt, as for any damaged cache.
- A merged set holds its clones inside the parent zip. Only the parent shows, and only the parent can start. This is how FBNeo works (Batocera wiki).

For an unknown name, the diagnostics page says:

> `<file>.zip` is not a set name of FinalBurn Neo. Check the file name, or the set version.

## 8. Changes to the core host

ADR 0016 and this section change `docs/spec/libretro-host.md`. M07a updates that spec with the code.

### 8.1 Initial option values

FBNeo declares its options inside `retro_load_game`, after it knows the DIP switches of the game. Today the session calls `set_option` before `load_game`, so each FBNeo value gives `OptionError::UnknownKey` and counts as refused (`Counters::refused_options`). ADR 0016 adds held values: the host keeps the initial values and applies each one at the `SET_VARIABLES` call that declares its key.

### 8.2 `SET_ROTATION`

- The host answers `true` for 0, 1, 2 and 3 and stores the value as `Rotation` (`R0`, `R90`, `R180`, `R270`, counter-clockwise, as `libretro.h` defines it). Another value: `false` and `Event::Refused(Refusal::Rotation { value })`.
- A call during `load_game` or `Host::open` sets `Host::rotation()`. A call during `run_frame` also adds `Event::Rotation(Rotation)`, in the output of that frame.
- FBNeo calls it in `retro_load_game` and when its vertical-mode option changes (`SetRotation()` in `src/burner/libretro/libretro.cpp`, read for this spec). The frame stays as the board draws it. With `fbneo-vertical-mode = "disabled"` (its default, and NEPTRON's pinned value) the core asks for:

  | Driver flags | Rotation |
  | --- | --- |
  | none (horizontal) | 0, `R0` |
  | `BDF_ORIENTATION_VERTICAL` | 1, `R90` |
  | `BDF_ORIENTATION_FLIPPED` | 2, `R180` |
  | `BDF_ORIENTATION_VERTICAL` and `BDF_ORIENTATION_FLIPPED` | 3, `R270` |

  The value `enabled` (mode 1) turns horizontal games too, for a screen on its side; it is not for the MVP. DoDonPachi is `BDF_ORIENTATION_VERTICAL` with an upright size of 240x320 in its driver (`ddp.cpp`), so it sends 320x240 and asks for `R90`. A trace of a vertical set needs the owner's set, so it waits for the bench session. The test core covers `R90`, `R180` and `R270`.

### 8.3 Input descriptors

`SET_INPUT_DESCRIPTORS` stores the list (port, device, index, id, description), at most `MAX_INPUT_DESCRIPTORS` (256) entries, each description at most `MAX_CORE_STRING_BYTES`. A longer list is cut and logged once. `Host::input_descriptors()` gives the list. Nothing reads it in the MVP. M05 (remapping) shows it.

### 8.4 The core trace

`core_trace` writes an `option` line, with its values, for each option that the core declares during `load_game` too, after the `load` logs. It writes the rotation after the `av` line (`rotation r90`). The trace format only grows. The catalog test then finds the FBNeo option values.

### 8.5 The zip central directory

`neptron-libretro` makes its central-directory reader public as a function that lists the entries of a zip: name, uncompressed size, CRC-32. It reads only the end record and the central directory, at most `MAX_CENTRAL_DIRECTORY_BYTES`. `neptron-session` re-exports it as `neptron_session::zip_entries`, because the app reaches the core host only through the session (section 10). The reader exists and has tests; this is a new public function, not new parsing code.

### 8.6 The origin of the aspect

`GameGeometry` gets `aspect_from_core: bool`: `true` when the core reported an aspect above 0, `false` when the host used its fallback, the base width over the base height (`crates/neptron-libretro/src/geometry.rs`). The session needs it for a turned source (section 11).

### 8.7 What does not change

- Content: FBNeo sets `need_fullpath` and `block_extract`, so the host passes the path of the zip and the core opens it, finds the parent and the BIOS by name, and matches the ROMs by CRC. This works today.
- Pixel formats: XRGB8888, or RGB565 for drivers that draw 16-bit only. Both are supported.
- `SET_CORE_OPTIONS_DISPLAY`, the update-display callback, `SET_SUBSYSTEM_INFO`, disk control and `GET_VFS_INTERFACE` stay refused. FBNeo uses them for DIP visibility, Neo Geo CD and consoles, or falls back to stdio.
- Rate: FBNeo reports `nBurnFPS / 100`, cut to 0.01 Hz. The host turns it into millihertz as for every core.

## 9. Interlace: "always progressive"

The session takes a frame of 289 lines or more as interlaced (`INTERLACE_MIN_LINES` in `crates/neptron-session/src/source.rs`). The cores do not say it, and 288 is the tallest progressive console picture. For arcade this guess is wrong: an arcade board draws progressive frames, also tall ones, and a core that turns a vertical game itself (MAME 2003-Plus) sends 320 lines upright. For FBNeo the guess rarely fires today, because it sends a vertical frame on its side (240 lines), but a tall board would.

The systems data gets a key:

| Key | Type | Meaning |
| --- | --- | --- |
| `always_progressive` | boolean, optional, default `false` | Every frame of the system is progressive. The line count does not decide |

- The default keeps every present system as it is. The key is an addition, so `format` stays `1`.
- `arcade` and `neogeo` set it to `true`.
- The session takes it from the launch request and sets `SourceGeometry::interlaced` to `false` for every frame.
- A tall progressive frame on a 15 kHz CRT then goes to rung 3 (interlaced 1:1), as the owner decided for a medium-resolution board (`crates/neptron-modeline/README.md`).

M07b adds the key to the parser and the data files. M08a uses it in the session.

## 10. The set check before launch

### When and where

- The check runs in the menu process when the user starts an arcade game, before the menu writes the relaunch file and exits with code 75 (`RELAUNCH_EXIT_CODE`) for `neptron-launch` to start the game run. A failed check shows its message in the menu, with no mode change and no relaunch.
- **Caller and thread:** `apps/neptron` (the `Library` of `apps/neptron/src/library.rs`) runs the check on a thread of its own, `neptron-set-check`, that `Library` owns, as it owns the scan thread `neptron-scan`: it starts it per check, and cancels and joins it on drop. The frame thread never reads a file: it shows the launch as pending and polls the result with `is_finished()` and `join()`, as for the scan.
- **Who reads the zip:** the reader is the central-directory reader of `neptron-libretro` (section 8.5). `apps/neptron` does not depend on `neptron-libretro`, and it must not get that edge: the Definition (`docs/definition/11-monorepo-layout.md`) makes `neptron-session` the crate that mediates between the app and the core host. So `neptron-session` re-exports the function (`neptron_session::zip_entries`), and the app calls it there. The edges app to session and session to libretro exist today; no new edge is added. `neptron-library` stays free of I/O on zips and of any dependency on the host.
- It reads only the central directory of each zip it finds for the set, its parent and its BIOS (section "Where it looks"). A central directory is a few KiB.
- The check is pure: a function in `neptron-library` (`arcade::check_set`) takes the index record and the entry lists, and returns a verdict.

### Where it looks for files

FBNeo searches for three zip names: the set, its parent and its BIOS (`BurnDrvGetZipName`, indexes 0 to 2). For each name it tries, in this order, the folder of the game, then `system/fbneo/`, then `system/`, and it opens **every** file that exists, not only the first. Then it matches each ROM by CRC in the opened files, in the order they were found; the first file that holds the CRC gives the ROM. A ROM of the wrong size is an error. (`locate_archive` and `open_archive` in `src/burner/libretro/libretro.cpp`, read for this spec. With `fbneo-allow-patched-romsets` on, a `system/fbneo/patched/` folder comes first; NEPTRON leaves that option at its default.)

NEPTRON's system folder is `share/bios`. The check does the same, in the same order:

1. The folder of the game zip.
2. `share/bios/fbneo/`.
3. `share/bios/`.

So `neogeo.zip` can be in `bios/`, in `bios/fbneo/` or next to the games, and a ROM can come from any of them. FBNeo builds each path from the lower-case DAT name and opens it as it is, so the file system decides the case rule. The check opens the same path the same way: on the exFAT share the case of a file name does not matter, on a file system that has case it does, for the check and the core alike.

### What it checks

1. The zip of the game is in the index. Otherwise: message 1.
2. The BIOS of the set: follow `romof` from the set to the first set with the `b` flag. If one exists, a zip of that name must exist in one of the places. Otherwise: message 2.
3. If the set is a clone and needs a ROM that no zip of the set's own name holds, a zip of the parent's name must exist. Otherwise: message 3.
4. Each ROM of the set, except `nodump` ROMs and ROMs that are also ROMs of its BIOS set (the same CRC), must be in one of the found zips of the set, its parent or its BIOS, with the same CRC-32 and the same size. The match is by CRC and size, not by name, as FBNeo matches. This one rule covers split, non-merged and full non-merged sets. Otherwise: message 4. When more than half of those ROMs (`WRONG_SET_PERCENT`, 50) are missing or differ, the set is for another emulator or another version: message 4b.
5. The BIOS: if no found zip of the BIOS name holds any ROM of the BIOS set, message 5. If some but not all match, the launch goes on and the diagnostics page lists the differences. FBNeo does not require a ROM that its driver marks `BRF_OPT`, and a Neo Geo BIOS set lists many BIOS variants, of which the core needs only the selected one. The DAT has no such mark, so the check cannot tell which BIOS files are needed. A stricter rule is an open point (section 19, A3). The same mark can make a game ROM optional; the check then asks for a ROM that the core would not need. That is safe (it stops, never passes wrongly), and the same open point covers it.
6. A zip that cannot be read (damaged, ZIP64, multi-disk, too large a central directory): message 6.

With no index (section 6), the check gives message 7 and lets the user start the game.

### The messages

Each message shows in a dialog in the menu, and the full list goes to the log (a diagnostics page is a later item). `<set>` is the zip stem, `<n>` a count. `<names>` gives the DAT names of the first two wrong ROMs, each cut to 24 characters, and counts the others (`gg1.bin, gg2.bin and 1 more`), so the dialog fits the 320x240 canvas. `<version>` comes from the index: the DAT version and the short commit of the core, `1.0.0.03 (commit d7450cf)`. With one file, message 4 says `1 file of this game is missing or differs`.

| # | Text | Launch |
| --- | --- | --- |
| 1 | `<set>.zip` is not a set name of FinalBurn Neo. Check the file name. | Stopped |
| 2 | This game needs the BIOS file `<bios>.zip`. Put it in `bios`, or in the folder of the game. | Stopped |
| 3 | This game needs the file `<parent>.zip` in the same folder. | Stopped |
| 4 | `<n>` files of this game are missing or differ: `<names>`. This image needs sets for FinalBurn Neo `<version>`. The log lists every file. | Stopped |
| 4b | Most files of this game do not match. The set is for another emulator or another version. This image needs sets for FinalBurn Neo `<version>`. | Stopped |
| 5 | The BIOS file `<bios>.zip` does not match this version of FinalBurn Neo. This image needs sets for FinalBurn Neo `<version>`. | Stopped |
| 6 | The file `<file>.zip` cannot be read: `<reason>`. | Stopped |
| 7 | NEPTRON cannot find its list of arcade games, so it cannot check this game. | The user chooses: start or cancel |

The diagnostics page names the pinned FBNeo commit and the DAT version, so a user can get the right DAT for ClrMamePro.

### Why the check is needed

When a ROM is missing, FBNeo's `retro_load_game` returns `true`, sets a 60 Hz geometry, draws its own error screen and sends `SHUTDOWN` at a button press. It logs `ROM at index %d with name %s and CRC 0x%08x is required` at error level (that log line is in `open_archive`, read for this spec). Without the check, a CRT would change its mode to show that screen. The host cannot tell this from a good load. (The `true` return and the error screen are the research reading of `libretro.cpp`; the trace of a missing-ROM case in M07b confirms them.)

### What the check does not prove

A set that passes can still fail in the core: a ROM the DAT marks as needed that the core loads in another way, a BIOS choice of a core option. The core's own screen then shows, after a mode change. The session already ends a game at `SHUTDOWN`.

## 11. Vertical games

### What the core does

FBNeo draws a vertical game as the board draws it, on its side, and calls `SET_ROTATION` to make it upright (section 8.2 has the values). Its frame size comes from `BurnDrvGetFullSize`, which swaps the driver's upright size for a vertical driver: DoDonPachi's driver says 240x320, the frame is 320x240. Its aspect comes from `BurnDrvGetAspect`, the driver's own aspect with no swap when `fbneo-vertical-mode` is `disabled`: 3:4 for DoDonPachi, the shape of the upright picture. (`retro_get_system_av_info` in `libretro.cpp` and `burn.cpp`, read for this spec. The core swaps the aspect only for the `enabled` and `alternate` modes, and for the TATE modes on a vertical driver.) Its option `fbneo-vertical-mode` has the values `disabled` (the default), `enabled`, `alternate`, `TATE` and `TATE alternate`. FBNeo cannot send an upright frame to a horizontal screen.

### What NEPTRON does in the MVP

- The host accepts the rotation (section 8.2).
- The session builds `SourceGeometry` with the rotation. `SourceGeometry::aspect_milli` is the aspect of the frame as the core draws it: the policy (`take_source` in `crates/neptron-modeline/src/policy/source.rs`) swaps the axes of a turned source and inverts its aspect. FBNeo reports the aspect of the upright picture (above).
- So for `R90` and `R270`, **and only when the core reported the aspect** (`GameGeometry::aspect_from_core`, section 8.6), the session sets `aspect_milli = round(1_000_000 / core_aspect_milli)`. DoDonPachi: the core gives 750, the source gets 1,333, the policy gets 750 back for 240x320.
- When the core gives no aspect (0 or less), the host falls back to the base width over the base height (`crates/neptron-libretro/src/geometry.rs`). That value is already the shape of the frame as drawn, so the session keeps it. A turned 320x240 frame then gets 1,333, and the policy gives the upright 240x320 picture square pixels.
- The research does not cover this step; without it a vertical game is placed with the inverse shape.
- The policy places the upright picture by the rule below: it swaps the axes, treats the long side as lines, and keeps the shape of the upright picture.
- `Scene` carries the rotation, and both renderers turn the game picture (ADR 0015). The UI canvas is never turned.
- `data/cores/fbneo.toml` pins `fbneo-vertical-mode = "disabled"`, the core's default, so that the turns and the aspect above do not change if the core's default changes.

### The owner's rule

The owner decided the video rule for arcade games on 2026-11-16. In substance:

- A sharp picture comes first. A soft picture is worse than a border.
- A game is never stretched. It keeps its shape on every display.
- Black borders above and below are acceptable when they are not too big. The owner does not like large borders, but he likes a soft picture less.
- On a 15 kHz screen a vertical game is 480i, with its lines 1:1.
- On a VGA CRT or a flat panel more modes exist, so the policy chooses a better fit: the largest whole-number scale of the game's lines that the display can show at the game's own rate, also when that mode has more lines than the 480p that a horizontal game gets.
- On a CRT the horizontal scale is free (the signal is analog). The policy chooses the mode width for the right shape. Only the line count must be a whole multiple of the game's lines.
- A soft "fit" is only an opt-in value of the setting.

### The reference boards

The policy table (next section) runs these boards. The size is the frame as the core draws it, and the turn is the one the core asks for. Unless the source names MAME, the size is the one of the FBNeo driver entry and the rate is the one FBNeo reports: its `BurnSetRefreshRate` value cut to 0.01 Hz, or 60.00 Hz when the driver sets no rate (`nBurnFPS` starts at 6000 in `burn.cpp`). The drivers were read at `libretro/FBNeo` master on 2026-11-16, and the MAME drivers at `mamedev/mame` master on the same day.

| Name in the table | Game, board | Frame, turn | Upright | Rate | Source |
| --- | --- | --- | --- | --- | --- |
| `ddonpach` | DoDonPachi, Cave 68000 | 320x240, R90 | 240x320 | 57.55 Hz | `d_dodonpachi.cpp`, `CAVE_REFRESHRATE` 15625 / 271.5 |
| `batsugun` | Batsugun, Toaplan 2 | 320x240, R90 | 240x320 | 59.63 Hz | `d_batsugun.cpp`, `TOA_REFRESHRATE` 59.637405 |
| `gunbird` | Gunbird, Psikyo | 320x224, R90 | 224x320 | 59.29 Hz | `d_psikyo.cpp`, 15625 / 263.5 |
| `gunlock` | Gunlock, Taito F3 | 320x224, R270 | 224x320 | 58.94 Hz | `d_taitof3.cpp`, vertical and flipped, 58.943844 |
| `pacman` | Pac-Man | 288x224, R270 | 224x288 | 60.00 Hz | `d_pacman.cpp`: no rate set |
| `pacman_60606` | Pac-Man at the board's rate | 288x224, R270 | 224x288 | 60.606 Hz | MAME timing, 6.144 MHz / 384 / 264 (`tests/golden/systems.toml`) |
| `dkong` | Donkey Kong | 256x224, R270 | 224x256 | 60.00 Hz | `d_dkong.cpp`: no rate set |
| `gground` | Gain Ground, Sega System 24 | 496x384, R270 | 384x496 | 57.524 Hz | MAME `segas24.cpp`: ROT270, 16 MHz / 656 / 424; 24.39 kHz |
| `ikaruga` | Ikaruga, NAOMI | 640x480, R270 | 480x640 | 60.00 Hz | MAME `naomi.cpp`: ROT270. The rate is the nominal VGA rate. UNVERIFIED: the MAME raw screen (26.917 MHz, 820 by 532) gives 61.70 Hz |
| `neogeo` | Neo Geo MVS | 304x224, R0 | | 59.18 Hz | section 12; the width is the research reading of OP-099 |
| `cps2` | CPS-1 and CPS-2 | 384x224, R0 | | 59.63 Hz | section 12 |
| `pwrinst2` | Power Instinct 2, Cave 68000 | 320x240, R0 | | 57.55 Hz | `d_pwrinst2.cpp` |
| `dariusg` | Darius Gaiden, Taito F3 | 320x232, R0 | | 58.94 Hz | `d_taitof3.cpp` |
| `rtype` | R-Type, Irem M72 | 384x256, R0 | | 55.00 Hz | `d_m72.cpp` |
| `mk` | Mortal Kombat, Midway Y Unit | 400x256, R0 | | 54.70 Hz | `d_yunit.cpp`: 54.70, or 54.82 for a 400-line screen |
| `sfiii3ws` | Street Fighter III 3rd Strike, CPS-3 wide mode | 496x224, R0 | | 59.59 Hz | `cps3run.cpp`: 59.59949; the normal mode is 384x224 |
| `bnzabros` | Bonanza Bros, Sega System 24 | 496x384, R0 | | 57.524 Hz | MAME `segas24.cpp`: ROT0 |
| `littlerb` | Little Robin | 338x288, R0 | | 30.00 Hz | `d_littlerb.cpp` |

Notes on the owner's candidates:

- Mortal Kombat is 400x256 in FBNeo, not 400x254, at 54.70 Hz.
- Pac-Man, Galaga and Donkey Kong: FBNeo sets no rate for them, so it reports 60.00 Hz. The boards run at 60.606 Hz. The game then runs 1 % slower than the board on every display, also on a CRT that shows 60.00 Hz exactly. The policy cannot know the board's rate. `pacman_60606` shows the result for a core that reports the board's rate. Galaga (224x288) gives the same cells as Pac-Man.
- Radiant Silvergun: MAME `stv.cpp` lists `rsgun` as ROT0, so it is not a vertical board. ST-V has no FBNeo driver. It is not in the table.
- Rainbow Islands (`d_taitomisc.cpp`, 320x224, no rate set, so 60.00 Hz) gives the same cells as other 224-line boards at 60 Hz. It is not in the table.
- A horizontal board of 288 lines: the only ones in FBNeo are Little Robin and its clone (338x288). Little Robin reports 30.00 Hz, so it is also the case of a board below every CRT range.
- 24 kHz boards: Sega System 24 is the medium-resolution case (496x384 at 24.39 kHz). Gain Ground and Scramble Spirits are System 24 boards that are vertical (ROT270): 496 lines upright. No FBNeo driver exists for System 24, so these need a MAME core, after the MVP.
- No board in the list runs above 60.606 Hz. NAOMI is the 31 kHz board.

### The policy today: the table

`crates/neptron-modeline/tests/integration/arcade_table.rs` runs each board through `decide` and `choose` with `neptron_safety::validate`, with the default settings (`auto`, `exact`), on these displays:

- Every preset of `data/monitors`, with a GPU that has no pixel clock floor and sends interlace, as the policy goldens do. A pixel clock floor changes the width of a mode, not its lines.
- Four profiles that `profile_from_edid` builds from the range limits of typical PC tubes (one range, GTF porches, no interlace), with the GPU's pixel clock limited to the EDID's: `edid_31` (31 to 32 kHz, 50 to 75 Hz, 40 MHz), `edid_30_54` (30 to 54 kHz, 50 to 120 Hz, 80 MHz), `edid_30_70` (30 to 70 kHz, 50 to 160 Hz, 110 MHz), `edid_30_96` (30 to 96 kHz, 50 to 160 Hz, 230 MHz). The test copies the GTF rule of `neptron-platform`, and a test holds the copy to the platform's result for a 30 to 71 kHz tube.
- Flat panels with their 60 Hz mode: `panel_720`, `panel_768`, `panel_1080` (the panel of the policy goldens, which also lists 50 Hz and 720p), `panel_1440`, `panel_2160`.
- `tv_ntsc` and `tv_pal`, on the composite output of the Raspberry Pi.

To make the table again: `cargo xtask test -p neptron-modeline --no-capture arcade_table`. Appendix A holds the whole table. The columns:

- Rung, mode (active size and scan), total lines (a frame of both fields when interlaced), line rate and field rate of the winner.
- Rate: `exact` when the field rate is the game's rate within 5 mHz, `exact x2` at twice it; else the refresh kind and the speed change. On a panel and a TV, the speed change.
- Scale: the vertical scale, whole or a fraction for a fit. Filter: of the game picture.
- Height: the game's share of the active lines of the mode. Border: the black lines of the mode.
- Shape: the shape on the screen against the game's own shape. A CRT shows its active area on a 4:3 face, a TV shows 704 of its 720 samples as 4:3, a panel has square pixels. +77.8 % is a 3:4 picture stretched to 4:3.
- Best whole: the largest whole scale that a proposal of the plan has at the game's rate and that the validator accepts.
- Rule: `ok` when the winner follows the owner's rule; else the first rule it breaks (`no mode`, `soft fit`, `shape` beyond 2 %, `rate` when a range covers the game's rate or twice it, `smaller scale` when a larger whole scale keeps the rate on a CRT).

#### The main cases

DoDonPachi (240x320 upright, 57.55 Hz) and the 480x640 board (Ikaruga, 60 Hz). "Proposed" is the rule below, without the panel rule R8 of 2026-11-16.

| Display | DoDonPachi today | DoDonPachi proposed | 480x640 today | 480x640 proposed |
| --- | --- | --- | --- | --- |
| 15 kHz (`generic_15`) | 448i, 320 lines 1:1, 57.550 Hz, 71 % of the lines, right shape | The same | Fit into 480i, 0.75, soft, stretched to 4:3 | Fit into 480i, 0.75, soft, right shape, `fitted` notice. No exact mode exists |
| 31 kHz only (`edid_31`) | 520p, 320 lines 1:1, 57.550 Hz, 62 %, right shape | The same | Fit into 509p, 0.80, soft, stretched | Fit into 509p, 0.80, soft, right shape. No exact mode exists |
| Multi-sync to 54 kHz (`edid_30_54`) | 2x, 640p at 38.2 kHz, 57.550 Hz, full height, stretched to 4:3 | 2x, 640p, 427 pixels wide, right shape | 1x, 640p at 39.9 kHz, stretched | 1x, 640p, 853 pixels wide, right shape |
| Multi-sync to 96 kHz (`edid_30_96`) | 2x, 640p, stretched; 4x 1280p is possible | 2x, 640p, right shape: the smallest multiple that fills (A7) | 1x, 640p, stretched; 2x 1280p is possible | 1x, 640p, right shape (A7) |
| Panel 1080p | 3x, 720x960, nearest, 89 %, +4.26 % speed | The same | 1x, 480x640, nearest, 59 % | The same, with the small-picture notice (decided, A9) |
| Panel 4K | 6x, 1440x1920, nearest, 89 % | The same | 3x, 1440x1920, nearest, 89 % | The same |
| Composite NTSC | 480i, 320 lines 1:1, 67 %, +4.15 % speed | The same | Fit into 240p, 0.38, soft | Fit into 480i, 0.75, soft, right shape |

#### What the table showed before the change

These findings describe the policy as it was before the change of 2026-11-16 (section "After the change" gives the result).

1. **Stretch (rule `shape`).** A turned board in a CRT mode with no border of the policy's own fills the whole 4:3 width: +77.8 % for a 3:4 picture. This is every multi-sync tube (2x 640p for a 320-line board), `d9400` and `d9800` (240x320p), Donkey Kong on every 15 kHz preset (224x240p to 224x256p), the 496-line Gain Ground on every display that holds 496 lines, and every fit of a turned board. Cause: `shape` (`policy/shape.rs`) gives a mode the board's aspect width (`border_width`) only when the mode is bordered; otherwise `base_width` is the game width, and `crt_placement` fills the width with it. The fitted rung fills the active area (`Vertical::Fill`). A turned board shows correctly today only by chance, in a bordered mode. Some existing goldens of `policy.toml` hold such stretched results for turned systems: `donkey_kong` on `arcade_15`, `pc_31_120` and `edid_npt_0071`; `pacman` on `edid_npt_0071`; `space_invaders` on `generic_15`, `arcade_15`, `arcade_31`, `pc_31_120` and `edid_npt_0071`.
2. **Soft fit on multi-range presets (rule `soft fit`).** On `arcade_15_25`, `arcade_15_31`, `m3129`, `d9200`, `ms929`, `ms2930`, `r666b` and `pstar`, a 320-line vertical board gets the fit, stretched, although a 25 kHz or 31 kHz range holds 320 lines with a border and the 15 kHz range holds them interlaced 1:1. The 384-line Bonanza Bros gets the fit on `arcade_15_31`, `m3129` and `d9200` the same way. Cause: `progressive_min` and `interlaced_min` take the smallest minimum of all ranges that cover the rate, so 320 lines count as "needs no border"; Switchres then puts the mode in a range whose window starts above 320 lines (`LinesChanged`). Rung 3 drops with `FitsProgressive` because `progressive_capacity` is the largest capacity of all ranges. The policy looks at the ranges as one window, but they have gaps.
3. **No mode (rule `no mode`).** Gunlock on `arcade_15_25` and `arcade_15_25_31`, the 480x640 board on `arcade_15_25`, `arcade_15_25_31` and `arcade_25`, and Pac-Man at 60.606 Hz on `arcade_25` get no mode at all. The validator refuses each proposal for its line rate: 24.989 kHz and 24.961 kHz on a range of exactly 24.960 kHz. The same cause gives DoDonPachi, Batsugun, Pac-Man and Gain Ground the fit on the `ntsc` preset (checked for DoDonPachi): its interlaced 1:1 mode has 15.734353 kHz against a range of exactly 15.734260 kHz. The validator is right. The policy must propose a mode inside a range whose lowest and highest line rate are equal.
4. **Rate (rule `rate`).** Little Robin (30.00 Hz) runs 33 % to 67 % fast on every CRT: no range covers 30 Hz, so the target is the nearest rate a range covers (`Nearest`). Every range covers 60 Hz, where each frame shows twice at the right speed. With `exact`, `refresh_targets` offers twice the rate only for the `native` style (`native_double_target`). A panel and a TV already show each frame twice (`repeat` 2).
5. **TV fit (rule `soft fit`).** A board taller than the interlaced mode (Gain Ground 496 lines, the 480x640 board) gets the fit into the progressive mode (0.48 and 0.38 of its lines) and not into the interlaced mode (0.97 and 0.75). `tv_plan` puts the fitted rung on the progressive mode only.
6. **TV width (rule `shape`).** On PAL, 224-line boards are 18.7 % too wide and 256-line boards 3.9 % too wide. On NTSC, a 240-line board is 9.1 % too narrow and a 256-line board 6.2 % too narrow (the width stops at the 704 safe samples). The `aspect` width rule gives a 4:3 source of 224 to 240 lines the console width (644 or 650 samples), because a console does not fill the line. An arcade board fills its monitor. OP-099 covers the Neo Geo on NTSC only.
7. **Smaller scale (rule `smaller scale`).** On a multi-sync tube a horizontal board gets 2x (full height) where 3x or 4x also keep the rate; on a tube with 15 kHz and 31 kHz ranges it gets 1x on 15 kHz where 2x on 31 kHz keeps the rate; Bonanza Bros gets 1x where 2x to 4x keep the rate. On a CRT these give the same picture size, and the smallest multiple that fills is the rule (A7).
8. **Borders that the rule accepts.** A 320-line board on a 31 kHz-only tube is 1:1 in 482 to 520 lines (62 % to 66 %); Pac-Man is 288 lines in 473 to 506 (57 % to 61 %); Donkey Kong is 256 lines in 479 on `arcade_31` (53 %). No whole multiple above 1 fits these tubes, and the overscan crop (8 lines per edge at most) does not reach it. The rule refuses a soft fit, so these stay. On a 1080p panel the 480x640 board is 1x, 59 % of the height.
9. **15 kHz mode size.** The interlaced 1:1 mode on a 15 kHz preset is 448i, the smallest interlaced mode of the range, not 480i. The game's lines are the same on the tube: the rest of the field is black lines or blanking. The rule holds.
10. **Panel filter.** On a panel the vertical scale is whole and nearest. The horizontal scale is whole only when the board's pixels are square at that scale (DoDonPachi 3x: 720 pixels for 240). Gunbird (224 pixels, 3:4 at 320 lines) gets 720 pixels at 3x, 3.21 times its width, with sharp bilinear on that axis. This is the present panel rule for every source with pixels that are not square.

Before the change, 319 of the 648 cells (18 boards on 36 displays) followed the rule; they were the first goldens of `tests/golden/policy_arcade.toml`.

### The decisions of 2026-11-16

The owner decided the open points of the first version of this section (A7 to A13) on 2026-11-16. The rule below holds them:

| Item | Decision |
| --- | --- |
| CRT multiple (A7) | The smallest whole multiple that fills the screen. The picture styles `3x` and `4x` still ask for more |
| Interlace | Never an interlaced mode, and never any mode, that the monitor profile does not declare. Interlace is allowed on composite TV and on a profile that declares it (the 15 kHz and 25 kHz ranges). Nothing widens a profile range or a validator limit |
| Border too big (A8) | A picture below 60 % of the screen height. Below it, an interlaced 1:1 mode that fills more wins where the profile declares interlace; else the small exact picture stays. The policy never falls to a soft fit on its own when an exact mode exists |
| Flat panel, every game (A9) | When the scale that fills the height is 4.0 or more, the game fills the height, sharp bilinear, with its shape kept. Under 4.0 it takes the largest whole multiple that fits. The value `integer` of `display.panel_scale` forces whole multiples at every scale. A picture below 60 % of the height at 1x keeps 1x, with a notice |
| TV width (A10) | The systems `arcade` and `neogeo` fill the TV width by their aspect (a board fills its monitor), not by the console width rule |
| Double refresh (A11) | A board below every CRT range gets double refresh, each frame shown twice, for every style |
| `display.vertical` (A12) | The values `exact` (default) and `fit`, on a CRT, a TV and a panel |
| Task order (A13) | The policy change is done on the branch of M08b, before M08a |
| Scanlines on a panel | A flat panel with scanlines (`display.scanlines = on`) takes the largest whole multiple and never the fill: the renderer draws scanlines only at a whole vertical scale |
| Notices | Every result that is not exact tells the user through the notice of the quick menu: a soft picture because no exact mode exists (with the reason), a small picture, a speed change, double refresh. The policy returns them as data |
| Vertical games | A vertical game on a horizontal display is the best possible case: no odd modes and no odd default scaling. TATE is the right answer later (OP-107); it is not designed here |

### The rule

The rule applies to every arcade source and, where it says so, to a turned source. It decides in the policy and gives the validator every mode, as today. It widens no limit: every mode stays inside a range of the profile, an interlaced mode needs an interlaced window of the range, and `neptron_safety::validate` judges every proposal.

Terms: `h` is the game's lines after the turn, `r` its rate, `a` its display aspect after the turn. A mode is **exact** when its game lines are `k x h` for a whole `k` (or `h` interlaced, rung 3), its field rate is `r` or `2r` within `REFRESH_MATCH_MHZ`, and its filter is nearest on the vertical axis. The **fill** of a mode is the game's lines over the picture lines of the range at that rate (`Padding::fill_permille`, which already counts the lines that the minimum line rate forces).

**R1. Shape, every display.** The game keeps its display aspect within one pixel. A game wider than the face takes the tallest height a range holds that is short of its shaped height by less than one pixel of the game, when no range holds the shaped height rounded up. No mode squeezes a game by more (decided, V7). On a CRT the mode width is the width at which the game's lines have the aspect `a` on the 4:3 face: `border_width` for every rung, bordered or not, and for the fit. A turned board is pillarboxed. The game's pixel width stays a whole multiple of its width (native or dynamic super width); only the mode width changes. On a TV and a panel, the present width rules hold, except R7.

**R2. Rates on a CRT.** The targets in order: the asked rate (`exact_x2`, `force_50`, `force_60`) when a range covers it; `r`; for any source that no range covers, `2r` when a range covers it, else the covered rate nearest `2r` with each frame twice when that changes the speed less than the nearest rate once (30 Hz on `ntsc`: -0.1 % against +99.8 %); the nearest covered rate; when the exact rungs gave no mode at `r` (or the nearest covered rate), the highest lower rate at which a range holds the game's lines (`arcade_25` holds 384 lines up to 60.000 Hz), ranked before the fit while it changes the speed at most `EXACT_SPEED_MARGIN_PPM` (2 percentage points) more than the fit, within `REFRESH_MATCH_MHZ` (decided, V6 of `docs/spec/video-policy-cases.md`); the last resort. On a profile of one field rate (`ntsc`, `pal`) a mode that misses the rate takes the next whole multiples of the game width until one hits it to the millihertz.

**R3. Ranges on a CRT.** Each range is its own window. For each rung and each range that covers the rate, the policy shapes the mode inside that range: its own minimum (for the border), its own capacity (for the multiple and the crop), its own interlaced window (for rung 3). Rung 3 is open when the game's lines do not fit the progressive capacity of every range, or when the interlaced mode fills more than every progressive exact mode that is below the fill threshold (R4).

**R4. Order on a CRT.** Among exact modes at one target (the fill thresholds are `PADDED_FILL_MIN_PERCENT`, 75 %, and `SMALL_PICTURE_MIN_PERCENT`, 60 %):

1. Exact modes that are not padded (fill of 75 % or more), in ladder order: the smallest whole multiple that fills the screen first (decided, A7), then rung 3.
2. Exact modes with an overscan crop.
3. Progressive exact modes with a fill of at least 60 %, the fuller first.
4. Every other exact mode, interlaced 1:1 included, the fuller first. A mode below 60 % carries the small-picture notice.

The present order of the targets holds above this. The fit comes after every exact mode at every target. With `display.vertical = exact` it carries the `fitted` notice and keeps the shape (R1).

So on 15 kHz a 320-line board is interlaced 1:1 (448i or 480i, 71 %); on 25 kHz it is 1:1 in 384 to 400 lines (80 % to 83 %); on 31 kHz only it is 1:1 in about 500 lines (62 % to 66 %); on a 15 kHz and 31 kHz tube it is 1:1 on 31 kHz (64.5 %, no flicker) before interlaced 1:1 on 15 kHz; on a multi-sync tube it is 2x, 640p, full height.

**R5. When the border is too big.** Below 60 % fill, the next option is an exact interlaced 1:1 mode that fills more, on a range whose profile declares interlace (the 15 kHz and 25 kHz ranges of the presets; no EDID profile and no 31 kHz preset declares it). When no such mode exists, the small exact mode stays, with the small-picture notice. A soft fit is never the next option; it comes only from `display.vertical = fit`.

**R6. When not even 1x fits.** The 480x640 board needs 640 lines at 60 Hz:

- On 15 kHz: no exact mode (the interlaced maximum at 60 Hz is about 480 lines). The fit, 0.75, into the interlaced mode, with the shape kept and the `fitted` notice.
- On 25 kHz (`arcade_25`, `pstar`): interlaced 1:1, 640 lines in 768i (83 %). Exact, inside the profile.
- On 31 kHz only: an interlaced 31 kHz mode would give 640 lines 1:1 in about 1,000 lines, but no 31 kHz profile and no EDID profile declares an interlaced window. A profile without one must not get an interlaced mode: that widens a limit. So the fit, 0.80, progressive, with the shape kept and the `fitted` notice. A crop does not help: 640 lines need 66 lines per edge, and the limit is 8.
- On a multi-sync tube from 40 kHz: 1x, 640p.
- On a panel: the largest whole scale (1x on 720p to 1080p, 2x on 1440p, 3x on 4K).
- On a TV: the fit into 480i (0.75) or 576i (0.90), not into 240p or 288p (R7).

**R7. TV.** The fit of a board taller than the interlaced mode goes into the interlaced mode. An arcade board (the systems `arcade` and `neogeo`) gets the aspect width (`aspect x lines x 2.2` on 525 lines, `x 1.8333` on 625 lines, at most 704 samples), not the console width (decided, A10). The rest of the TV rules hold: lines 1:1, the crop up to 8 lines per edge, the speed alert.

**R8. Panel, every game.** Let `s` be the scale at which the game, at its aspect, fills the height of the preferred mode (or its width, when the width is reached first), in thousandths. When `s` is at least `PANEL_FILL_MIN_SCALE_MILLI` (4,000), the game fills that height, centred, with its shape kept, sharp bilinear. Under 4,000 it takes the largest whole vertical multiple that fits, nearest on that axis, centred (the rule before 2026-11-16). `display.panel_scale = integer` takes the largest whole multiple at every scale, and so do scanlines on a panel (`display.scanlines = on`, `PolicySettings::panel_scanlines`): the renderer draws them only at a whole vertical scale of 2 or more. A fill whose height is a whole multiple of the lines takes the whole path (nearest, status exact). The scale rounds down, so 3.9995 stays under 4. Below 60 % of the height at 1x (the 480x640 board on 720p to 1080p) the picture stays 1x with the small-picture notice; `display.vertical = fit` fills the height for a turned game. The panel never gets a mode that its own scaler stretches.

**R9. Horizontal boards.** They differ from consoles in four points, and the rest of the console rules hold:

- An arcade board fills its 4:3 monitor, so on a TV it gets the aspect width (R7).
- Board rates are 54.70 Hz to 60.606 Hz, and 30 Hz for Little Robin. A CRT shows each rate exactly (R2). A panel and a TV change the speed (section 12).
- Heights of 256 lines (R-Type, Mortal Kombat) are 1:1 on 15 kHz at 55 Hz and below, and 2x 512 lines on 31 kHz; on NTSC they lose 8 lines per edge.
- Medium-resolution boards (384 lines, System 24) are interlaced 1:1 on 15 kHz (the present rule), 1:1 on 25 kHz, 1:1 in about 500 lines on 31 kHz (74 % to 77 %), and on a multi-sync tube 1x or 2x by A7.

**R10. Safety.** Nothing in R1 to R9 changes a validator rule, a range, the hard rails or `ValidatedMode`. A change of the mode width, of the range that makes a mode, or of the target rate is a different modeline that the validator judges as today.

### The work for the policy change

Every item is done on the branch of M08b (task M08c), in its own commit with its tests:

| Item | Finding | Change |
| --- | --- | --- |
| W1 | 1 | The mode width keeps the shape of every source that is not 4:3 (`border_width` for every exact rung, the crop and the fit); a game wider than the face is bordered with the lines its shape needs, and takes no crop |
| W2 | 2 | Each range of a multi-range profile is shaped on its own (`per_range` in `policy/crt.rs`), whenever two or more ranges cover the rate; a range whose fewest progressive lines exceed what it holds at the rate adds no capacity |
| W3 | 3 | A Switchres mode outside the line rates that the validator takes moves into them by lines of the vertical back porch at the same field rate (`onto_fixed_line_rate`); the whole-kHz clock aims for a field rate inside the range and ranks a mode inside the range first |
| W4 | 4 | An arcade board below every range gets twice its rate (`below_ranges_double_target`), for every style |
| W5 | 5 | An arcade board taller than the progressive TV mode fits into the interlaced mode |
| W6 | 6 | An arcade board gets the TV width of its aspect (`board_samples`) |
| W7 | 7 | The order of R4: `SMALL_PICTURE_MIN_PERCENT` (60 %); an interlaced 1:1 mode only over a small progressive picture |
| W8 | | `display.vertical` (`VerticalMode`, `Dropped::VerticalFit`) and `display.panel_scale` (`PanelScale`), in the schema, the policy and the app |
| R8 | | The panel rule: fill the height from a scale of 4 (`PANEL_FILL_MIN_SCALE_MILLI`) |
| Notices | | `Notice::soft`, `Notice::small_fill_permille`, `Notice::speed_change`, `Notice::reasons` |

The invariant sweep (`docs/spec/video-policy-cases.md`) found four more faults, fixed the same way: a mode above the top line rate of a band range (Switchres's), a field rate one millihertz off a range of one field rate, a wide or deinterlaced game with the wrong shape or no border, and a whole-profile mode that the validator refuses where one range alone holds the game.

### The overscan crop

The crop hides at most 8 source lines per edge (`OVERSCAN_CROP_MAX_LINES_PER_EDGE`) and at most 8 lines of a 240-line screen per edge as a share of the lines the range holds (`OVERSCAN_CROP_SCREEN_LINES`, rounded up): 9 of 256 lines at 15 kHz, 16 of 480 at 31 kHz. A Game Boy at 2x on a 256-line 15 kHz tube would lose 16 screen lines per edge, so it shows 1x.

### After the change

The table after the change, made by the same test, is Appendix A. Every cell follows the rule or carries an allowed reason:

| Verdict | Before (2026-11-16, first table) | After |
| --- | --- | --- |
| ok | 319 | 619 |
| ok: no exact mode exists on this profile | 0 | 21 |
| ok: the profile declares no interlace | 0 | 3 |
| ok: no window at the game's rate (the exact rung at the nearest rate that holds it) | 0 | 1 |
| ok: overscan crop | 0 | 4 |
| shape | 113 | 0 |
| smaller scale (rule before A7) | 94 | 0 |
| soft fit | 90 | 0 |
| rate | 26 | 0 |
| no mode | 6 | 0 |

The main cases:

| Display | DoDonPachi before | DoDonPachi after | 480x640 before | 480x640 after |
| --- | --- | --- | --- | --- |
| 15 kHz (`generic_15`) | 448i, 320 lines 1:1, 71 % | The same | Fit into 480i, 0.75, stretched to 4:3 | Fit into 480i, 0.75, right shape; notice: soft, too tall |
| 31 kHz only (`edid_31`) | 520p, 1:1, 62 % | The same | Fit into 509p, 0.80, stretched | Fit into 509p, 0.80, right shape; notice: soft, no interlace |
| Multi-sync to 54 kHz | 2x, 640p, stretched to 4:3 | 2x, 427x640p, right shape | 1x, 640p, stretched | 1x, 853x640p, right shape |
| Multi-sync to 96 kHz | 2x, 640p, stretched | 2x, 427x640p (A7: the smallest that fills) | 1x, 640p, stretched | 1x, 853x640p |
| Panel 1080p | 3x, 720x960, nearest, 89 % | The same (scale 3.4, under 4) | 1x, 480x640, 59 % | The same, with the small-picture notice |
| Panel 4K | 6x, 1440x1920, nearest | Fills the height: 1620x2160, sharp (scale 6.75); with scanlines on, 6x, nearest | 3x, 1440x1920 | The same (scale 3.4) |
| Composite NTSC | 480i, 1:1, 67 %, +4.15 % | The same | Fit into 240p, 0.38 | Fit into 480i, 0.75 |

The existing goldens that the change corrected (each commit body gives the reason):

- W1, nine cases of `policy.toml` that recorded a stretched turned system: `donkey_kong` on `arcade_15`, `edid_npt_0071` and `pc_31_120`; `pacman` on `edid_npt_0071`; `space_invaders` on `arcade_15`, `arcade_31`, `edid_npt_0071`, `generic_15` and `pc_31_120`. The lines, rates and game rectangles stay; the mode is wider and the game centred in it.
- R8, 25 cases of `policy.toml` on `panel_1080`: every source of 270 lines or fewer now fills the 1080 lines at its aspect with sharp bilinear, in place of the largest whole multiple with borders. No case on 720p changes.
- Arcade goldens added on this branch moved with W3 (1 mHz), W2 (Donkey Kong on `ms2930` and `r666b`: the smallest multiple that fills) and W6 (657 samples in place of 644).
- V3, three cases of `policy_tv.toml`: `pal_576i tv_ntsc` with each width rule, from the fit into 240p to the fit into 480i.
- V5, ten cases: `md_pal_h40 edid_npt_0071` in `policy.toml` and the nine `edid_npt_0071 md_pal_h40` cases of `policy_gpus.toml`, from 2x at 50.000 Hz to 1x at 99.40 Hz, each frame twice.
- The handhelds of the goldens now have the aspect their cores report (Game Boy 10:9, Game Boy Advance 3:2, WonderSwan 14:9; the shipped systems data holds no aspect): 18 cases of `policy.toml` and 18 of `policy_gpus.toml` move in width, offset or multiple to keep that shape.

The exceptions that stay, each an allowed reason of the table and the sweep:

- The overscan crop keeps the width, so the 240 lines that a 256-line board shows on NTSC are 6.2 % narrow: the full width would need 751 of the 704 safe samples.
- No exact mode exists for the 480x640 board on 15 kHz, on a 31 kHz profile with no interlace, and on a TV: the fit, with its reason in the notice.

### The notices

The policy returns the reason for every result that is not exact as data: `Notice::reasons`, a list of `NoticeReason` values, beside the flags that `Notice` already holds. The app shows them in the quick menu with the other notices. The app texts (the app owns them):

| `NoticeReason` | When | Proposed text |
| --- | --- | --- |
| `Soft(NoExactMode::TooTall { lines, capacity })` | The game's lines do not fit any mode of the profile at its rate | "Soft picture: this screen cannot show the game's {lines} lines (at most {capacity})." |
| `Soft(NoExactMode::NoInterlace { lines })` | The game needs an interlaced mode, and the profile declares none | "Soft picture: the game needs {lines} lines, and this monitor profile has no interlaced mode." |
| `Soft(NoExactMode::NoRate)` | No exact mode at any rate the profile covers | "Soft picture: no sharp mode at this game's rate." |
| `Soft(NoExactMode::Chosen)` | `display.vertical = fit` | "Soft picture: Vertical games is set to Fit." |
| `Soft(NoExactMode::PanelFill { scale_milli })` | A panel fills the height at 4.0 or more | No notice: this is the panel rule |
| `SmallPicture { fill_permille }` | Below 60 % of the screen height | "Small picture: the game uses {fill} % of the screen height, to stay sharp." |
| `SpeedChange { game_mhz, shown_mhz }` | The game runs at another rate | "The game runs at {shown} Hz, not {game} Hz: {percent} % speed." |
| `DoubleRefresh` | Each frame shows twice | "Each frame shows twice at {shown} Hz. The game speed does not change." |

### The setting `display.panel_scale`

| Key | `display.panel_scale` |
| --- | --- |
| Label | Panel scale |
| Choices, in menu order | `auto` (default): rule R8. `integer`: the largest whole multiple at every scale |
| Levels | game, system, global (`Levels::ALL`) |
| Applies to | A flat panel only |
| Safety | `Validated` |
| Help text | "How a game fills a flat panel. Auto fills the height when the picture is large, and uses whole steps when it is small. Integer always uses whole steps, with black borders." |

### The setting `display.vertical`

| Key | `display.vertical` |
| --- | --- |
| Label | Vertical games |
| Choices, in menu order | `exact` (default): every pixel exact, with the largest picture that a whole scale gives; on 15 kHz and on a TV interlaced, with flicker. `fit`: the picture fills the height of the screen with its shape kept, scaled, soft; on 15 kHz and on a TV progressive, with no flicker |
| Levels | game, system, global (`Levels::ALL`) |
| Where the row shows | Settings > Display, next to the picture style, which writes at the global level. And the Video page of the quick menu while a game runs, which writes at the game level; its keep prompt then moves the change to the system or the global level, as for the picture style |
| Applies to | Every display: a CRT, a TV and a flat panel |
| Effect | Only for a turned source (`R90`, `R270`). `exact` is rule R1 to R8. `fit` drops every exact rung below the fill threshold for that source (a `Dropped` reason, `VerticalFit`), so the fit wins where the exact picture is small: on 15 kHz and on a TV the progressive fit (240p or 288p, sharp bilinear); on 31 kHz only the fit into about 500 lines; on a panel the fractional scale. Where an exact mode fills at least the threshold (a multi-sync tube, a panel at 3x and above), `fit` changes nothing |
| Safety | `Validated`, as the picture style |
| Help text | "How a vertical game shows on a horizontal screen. Exact shows every pixel exact, with black borders, and on a 15 kHz screen or a TV with flicker. Fit fills the height, with a soft picture." |

- This replaces the choices `interlaced` and `fitted` of the first version of this spec. The setting is not built yet (M08a), so no stored value changes.
- The choices are append-only from M08a on. A cropped 1:1 view and the TATE choices are later values at the end of the list.
- The picture style still applies: `3x` or `4x` on a multi-sync tube asks for that multiple of a vertical game.
- The crawler finds the setting, as every setting.

### The notice the first time

The first time a vertical game gets the interlaced rung on a 15 kHz CRT or a TV with `display.vertical = exact`, the frontend shows this notice once:

> This is a vertical game on a horizontal screen. It shows interlaced, with every pixel exact. Settings > Display > Vertical games, or Video in the quick menu, can show it as a soft picture with no flicker. A screen turned on its side (TATE) comes in a later version.

The frontend keeps the notices already shown in `state/config/notices.toml`. The writer thread of `apps/neptron/src/outbox.rs`, which does every file write of the frame loop, writes it as temp file, fsync, rename; the frame thread only sends the job. The menu process reads the file at boot with the settings. A deleted file shows the notice again and loses nothing. (A store for one-time notices does not exist yet; open point A4.)

### Expected results for DoDonPachi (240x320 upright, 57.55 Hz)

| Display | Today (the goldens) | With the proposed rule |
| --- | --- | --- |
| 15 kHz PC CRT | Rung 3: 448i at 57.550 Hz, 320 lines 1:1, centred, pillarboxed | The same. With `fit`: 240p, the picture fitted, sharp bilinear |
| 31 kHz PC CRT | 320 lines 1:1 at 57.550 Hz, centred in 501 to 520 lines (62 % to 64 %), pillarboxed | The same, with the small-picture notice only below 60 % |
| Multi-sync PC CRT | 2x, 640p, stretched to 4:3 | 2x, 640p, about 427 pixels wide, pillarboxed |
| Flat panel, 1080p | Upright, factor 3: 720x960, nearest, pillarboxed. The game runs at the panel rate | The same |
| Flat panel, 720p | Upright, factor 2: 480x640, nearest | The same |
| TV, NTSC (Pi composite) | 480i, 320 lines 1:1, 264 of the 704 samples wide (the `aspect` width rule), sharp bilinear. 59.940 Hz: +4.15 % speed | The same |
| TV, PAL | 576i, 320 lines 1:1, 220 samples wide. 50.000 Hz: -13.1 % speed | The same |

The goldens of M08b check the column "Today" (`tests/golden/policy_arcade.toml`), except the multi-sync row, which breaks the rule. The 480i field order on the Pi `vc4` is not checked yet (OP-103 point 5).

## 12. Rates

The present policy stays. FBNeo reports each board's rate, cut to 0.01 Hz (`BurnSetRefreshRate` in `burn.cpp` stores `(INT32)(100.0 * rate)`; read for this spec). The about 95 ppm that the cut takes from the Neo Geo cannot be heard.

| Board | Rate reported | Source (FBNeo driver) |
| --- | --- | --- |
| Neo Geo | 59.18 Hz (board: 15625/264 = 59.1856) | `neo_run.cpp` |
| CPS-1, CPS-2 | 59.63 Hz | `cps.cpp` |
| Toaplan 2 | 59.63 Hz (59.637, cut) | `toaplan.h` |
| Psikyo | 59.29 Hz (15625/263.5 = 59.2979, cut) | `d_psikyo.cpp`: `BurnSetRefreshRate(15625.0 / 263.5)`, read for this spec |
| Taito F3 | 58.94 Hz (58.944, cut) | `d_taitof3.cpp` |
| Cave 68000 | 57.55 Hz (15625/271.5) | `cave.h` |
| Data East dec0 | 57.41 Hz | `d_dec0.cpp` |
| Irem M72 | 55.00 Hz | `d_m72.cpp` |
| Midway Y Unit | 54.70 Hz (54.82 for a 400-line screen) | `d_yunit.cpp`: `BurnSetRefreshRate((nScreenHeight == 400) ? 54.82 : 54.70)`, read for M08b. Mortal Kombat is 400x256 |
| Midway T Unit | about 54.7 Hz | UNVERIFIED |
| CPS-3 | 59.59 Hz (59.59949, cut) | `cps3run.cpp`, read for M08b. 384x224, or 496x224 in the wide mode |
| Pac-Man, Galaga, Donkey Kong | 60.00 Hz (board: 60.606 Hz) | `d_pacman.cpp`, `d_galaga.cpp`, `d_dkong.cpp` set no rate, so `nBurnFPS` keeps 6000 (`burn.cpp`), read for M08b. The game runs 1 % slower than the board on every display |
| Little Robin | 30.00 Hz | `d_littlerb.cpp`, read for M08b. A CRT needs double refresh for it (section 11, R2) |

### Expected result per display class

- **PC CRT, 15 kHz and 31 kHz:** Switchres solves each rate. The mode matches within `REFRESH_MATCH_MHZ` (5 mHz), with a pixel clock in whole kHz. The game runs at its own speed. No alert.
- **Flat panel:** the game runs at the panel's flip rate. The status shows the speed beyond `PANEL_SPEED_SILENT_PPM` (5,000 ppm).
- **A known case where neither choice is close:** Mortal Kombat (54.70 Hz) on a panel with a 720p 60 Hz mode and a 1080p 50 Hz mode (`panel_1080`) takes the 1080p fill at 50 Hz, -8.6 %. The whole multiple at 720p 60 Hz would be +9.7 %. The default does not change: the two are about as far from the board's rate, and the fill is the panel rule R8.
- **TV (Pi composite):** the game runs one frame per field, at the field rate of the standard. The pitch follows. The status shows the speed beyond `TV_SPEED_SILENT_PPM` (5,000 ppm).

Speed change of the game (field rate divided by the reported rate, less 1). Every case is beyond 5,000 ppm, so every one shows the speed alert:

| Board | Panel 60.000 Hz | Panel 59.940 Hz | NTSC 240p, 60.054 Hz | NTSC 480i, 59.940 Hz | PAL 288p, 50.080 Hz |
| --- | --- | --- | --- | --- | --- |
| Neo Geo | +1.39 % | +1.28 % | +1.48 % | +1.28 % | -15.4 % |
| CPS-1, CPS-2, Toaplan 2 | +0.62 % | +0.52 % | +0.71 % | +0.52 % | -16.0 % |
| Psikyo | +1.20 % | +1.10 % | +1.29 % | +1.10 % | -15.5 % |
| Taito F3 | +1.80 % | +1.70 % | +1.89 % | +1.70 % | -15.0 % |
| Cave 68000 | +4.26 % | +4.15 % | +4.35 % | +4.15 % | -13.0 % |
| Data East dec0 | +4.51 % | +4.41 % | +4.61 % | +4.41 % | -12.8 % |
| Irem M72 | +9.09 % | +8.98 % | +9.19 % | +8.98 % | -9.0 % |

- The owner accepts the speed change on composite for 0.1, with the existing alert (Cave +4.35 % on NTSC).
- PAL is much worse: every arcade board runs 9 to 16 % slow. The owner's decision names NTSC; PAL follows the same rule. Open point A5.
- The other rule (keep the game's rate and repeat a field every 20 to 25 fields) judders in a scrolling shooter. RetroArch with its default timing-skew limit also speeds such games up (from memory, UNVERIFIED). A per-game "keep game speed" choice is a later item.
- OP-099: FBNeo gives the Neo Geo 304 pixels wide, not 320 (research reading; trace). At 6 MHz, 304 pixels are about 684 samples, against the 644 of the console rule: about 6 % narrow on composite, not the 11 % that OP-099 states for 320.

## 13. Controls

- FBNeo maps the arcade inputs to the RetroPad. Select is the coin, Start is start.
- For `RETRO_DEVICE_JOYPAD` FBNeo uses its "Classic" layout (the Neo Geo CD pad): Neo Geo A to D on the face buttons; L and R are the 5th and 6th buttons of a six-button game. NEPTRON passes `RETRO_DEVICE_JOYPAD` on ports 0 and 1 today. Nothing in the input path changes in the MVP.
- DIP switches are core options. Each game declares its own (section 8.1).
- `fbneo-diagnostic-input` opens the service menu. Its default is "Hold Start" (`retro_common.cpp`, read for this spec). NEPTRON's setting `input.menu_hold_start_seconds` opens the quick menu with a Start hold, for an arcade panel with no Select. Its default is 0 (off), so the two only clash when the user turns it on, but then one hold does two things. `data/cores/fbneo.toml` sets the option to "Start + L + R", one of the values in the core's option table. A combination with Select is not used, because Select is the coin and Select with Start opens the quick menu.
- Watch item: Select and Start held together open the quick menu at their release. A player who holds the coin and presses Start opens it. Most players press them one after the other. The bench session checks it with a stick.
- M04 (four players) calls `retro_set_controller_port_device` on ports 2 and 3 too. Several arcade boards have four players.
- M05 (remapping) shows the descriptors of section 8.3 and adds the layout per system or per game (Classic, Modern, 6-Panel) as the libretro device subclass.

## 14. Save files

- FBNeo writes NVRAM and EEPROM files itself (Cave high scores and settings) to `<save directory>/fbneo/`, that is `state/saves/arcade/fbneo/` or `state/saves/neogeo/fbneo/`. The same set in the two folders has two save places.
- FBNeo has no `RETRO_MEMORY_SAVE_RAM`. `copy_save_ram` gives 0 bytes. M07a checks with a test that a core with no save RAM gives no `.srm` file and no `.bak`.
- The core's own writes are not atomic: a power cut during a write can damage an NVRAM file. NEPTRON cannot change this without a patch to the core. It is a risk (section 18), not a rule break: the code standard covers the files that NEPTRON writes.
- Save states are after the MVP.
- `fbneo-hiscores` stays at the core's default. Without `system/fbneo/hiscore.dat` it does nothing. Shipping `hiscore.dat` is a later item.

## 15. TATE (planned, after the MVP)

TATE is a screen turned on its side. It gives a vertical game its native mode: DoDonPachi as 320x240 at 57.55 Hz, 240p, every pixel exact, no flicker. This is the best picture a CRT can give a vertical game, and a strong reason for a vertical cabinet. It needs the UI turned too, so it is a separate task. These notes are the start for that task.

### Settings

- The Definition names `screen_orientation` (horizontal or vertical) and `rotation` (auto, 0, 90, 180, 270). A screen can be turned either way, so the TATE task decides between two choices `vertical_left` and `vertical_right` and one choice `vertical` with a separate direction. This is a question for the owner.
- `screen_orientation` is a global setting, safety `ConfirmOrRevert`: the user must be able to read the keep or revert dialog in the new orientation, or the revert must come with no input.
- `rotation` (per game, per system) overrides what the core asks.

### The rule

- The game turn that the renderer applies is the core's rotation less the screen rotation, modulo 360 degrees.
- A vertical game on a vertical screen: turn 0. The policy gets the frame as drawn (320x240) and finds its native mode.
- A horizontal game on a vertical screen: turn 90 or 270. The policy swaps the axes and the game gets the treatment of section 11 that a vertical game gets on a horizontal screen. `display.vertical` then applies to horizontal games: its meaning is "a game whose long side is across the tube's lines".
- The core keeps `fbneo-vertical-mode = "disabled"`, so it always reports the turn that the board needs (`R0`, `R90`, `R180` or `R270`, section 8.2) and the aspect of the upright picture. NEPTRON does all turns, so one code path serves both orientations. The core's `TATE` modes are not used: they move the turn into the core and change the aspect it reports.
- The validator and the hard rails see the real raster of the tube, as today. TATE changes no safety rule.

### The scene and the UI

- `Scene` gets a turn for the UI canvas (a second field, by a new ADR), or one screen turn that applies to the game and the UI. ADR 0015 leaves both open.
- The UI needs a portrait layout: a 240x320 canvas, or the 320x240 canvas turned and scaled. A turned 320x240 canvas does not fit the 240-pixel width of a turned 15 kHz screen at 1:1, so the portrait layout is the likely path. That is the largest part of the task (`neptron-ui` screens, scenes and goldens, the crawler in both orientations).
- The menu directions follow the viewer: up on the d-pad is up on the turned screen. The game's input is not turned: the controls of a cabinet do not move.
- Every screen of the frontend must be turned: the boot prompt, the keep or revert dialog, the safe mode, the recovery screens, the notices.

### Tests

- Goldens of the policy for turned screens on every profile.
- Scene goldens of each UI screen in portrait.
- `gles-compare` scenes with a turned UI.
- A bench check with a turned monitor (a hardware checklist line). Whether the owner can turn a monitor for this test is a question for the owner.

## 16. Task split

M06 is this spec and ADRs 0014, 0015 and 0016. The tasks below replace M07 and M08 of the taskboard.

| Task | Content | Owns | Needs |
| --- | --- | --- | --- |
| M07a, host | ADR 0016: held option values. `SET_ROTATION` (`Host::rotation`, `Event::Rotation`, `Refusal::Rotation`). `GameGeometry::aspect_from_core` (section 8.6). Input descriptors stored. The public zip entry list and its re-export from `neptron-session` (section 8.5). `core_trace` writes options declared at load and the rotation. Session: held values in place of `set_option` before load. Test core cases: options declared in `retro_load_game`; a rotation at load and in a frame; a 320x240 frame with `R90`, `R180` and `R270`; an aspect of 0 with a turn; no save RAM. `docs/spec/libretro-host.md` updated | `crates/neptron-libretro`, `crates/neptron-testcore`, `crates/neptron-session` (launch, the re-export), `docs/spec/libretro-host.md` | M06 |
| M07b, package and data | `os/package/libretro-fbneo`: pinned commit, sha256, `platform=unix-rpi3_64` on the Pi and `unix` with `USE_X64_DRC=1` on the PC (the Batocera settings, research reading), `license.txt` verbatim and the licence texts of the third-party parts in each built `.so` (ADR 0014). `LICENCES.txt` lines for the core and for the arcade index. Build time and `.so` size measured and recorded. The three data files (section 4), the embed list, `-` in option keys, the `always_progressive` key. Core traces of free sets: the ROMs stay outside the repository (OP-053 rule), the traces go into `tests/traces`, where the catalog test reads them with `include_str!`. A trace of a missing-ROM case. The README of `data/systems` and `data/cores` | `os/package/libretro-fbneo`, `os/Config.in`, `os/configs`, `os/board/*/LICENCES.txt`, `xtask/src/os`, `data/systems`, `data/cores`, `crates/neptron-library/src/catalog*`, `tests/traces` | M07a (for the trace; the package work can start at once, the merge waits) |
| M07c, library and set check | The index format, reader and encoder (`neptron-library`, module `arcade`). `cargo xtask arcade-index`. The index in the image. The scan lookup: title, folding, BIOS hidden, marks. The cache version. The set check, its thread `neptron-set-check` and its messages. "Other versions" in the list. The diagnostics lines (`mame` folders, unknown sets, the index state, the full check list) | `crates/neptron-library` (except `catalog*`), `xtask/src/arcade*`, `xtask/src/main.rs`, `xtask/Cargo.toml`, `os/package/libretro-fbneo` (install hook), `crates/neptron-ui`, `apps/neptron` | M07a, M07b, and the orchestrator's `build(deps)` commit for the XML reader in the root `Cargo.toml` |
| M08b, arcade policy table and goldens | The policy table of section 11 and Appendix A (`tests/integration/arcade_table.rs`), made from the policy as it is. The owner's rule, the findings, the proposed rule set and `display.vertical`, in section 11. Policy goldens (`tests/golden/policy_arcade.toml`) for every cell that follows the rule, and a test that a turn of 270 degrees gives the decision of 90, and 180 that of 0. No change of the policy source | `crates/neptron-modeline/tests`, `docs/spec/arcade.md` | M06 |
| M08c, arcade video rule | The work items W1 to W8 of section 11, the panel rule R8, `display.panel_scale` and the notices as data, with the decisions of 2026-11-16. Done on the branch of M08b. Every golden that the rule changes, with the reason in the commit; the cells of Appendix A that follow the rule become goldens | `crates/neptron-modeline`, `crates/neptron-settings`, and the callers of the settings | M08b |
| M08a, rotation | ADR 0015: `Scene::rotation`, both renderers, every `Scene` caller, the testkit hash, `gles-compare` scenes with `R90`, `R180` and `R270`. The session: the source rotation, the aspect inversion (section 11), `always_progressive`. The setting `display.vertical` and its policy effect. The notice and its store | `crates/neptron-types`, `crates/neptron-gfx`, `crates/neptron-testkit`, `crates/neptron-video`, `crates/neptron-session`, `crates/neptron-modeline/src`, `crates/neptron-settings`, `crates/neptron-ui`, `crates/neptron-kms/src/kms/mode.rs`, `crates/neptron-kms/examples`, `crates/neptron-window/examples`, `apps/neptron`, `apps/neptronctl`, `docs/adr` | M07a, M07b, M07c, M08b |

- Build order: M07a; then M07b; then M07c and M08b; then M08c; then M08a. M08b needs only M06 and can start at once.
- M08a needs W1 of M08c: without it a turned game shows stretched on a multi-sync tube and on every fit. M08c is done on the branch of M08b and merges with it, before M08a (decided 2026-11-16). M08c adds `display.vertical` to the schema and to the policy; M08a passes the source rotation, which makes it take effect.
- M08a needs M07c because both own `crates/neptron-ui` and `apps/neptron`.
- M08a needs M08b because both touch `neptron-modeline`: M08a its source, M08b its golden tests. If M08a adds a field to `PolicySettings` (for `display.vertical`), every golden case must set it. M08b merges first, and M08a rebases onto it and fixes the golden cases it then breaks, with no change of an expected result.
- M13 (the bench sessions) needs M07c and M08a in place of M07.
- The Pi 3 measurement task that runs now gives the board list for the Pi 3 (section 2). If it finds that a board family needs another core, that is a change to the scope, and the owner decides.

## 17. Measurement plan

### Free material

- MAME's free sets (mamedev.org/roms) are "approved for free distribution on this site only". They are downloaded to the development machine and never committed (OP-053 rule). FBNeo runs Gridlee, Robby Roto, Alien Arena, Targ, Spectar, Side Trak, Teeter Torture, Hard Hat, Circus, Robot Bowl, Rip Cord and Crash (research list; M07b checks each name against the DAT of the pin). They are light 8-bit boards: they test the package, the scan, the set check, the options, the rates and the core trace, not the speed.
- The DAT has `test240p` (the 240p Test Suite for Neo Geo). Its licence is UNVERIFIED. It needs `neogeo.zip`, which is SNK code.
- The NeoOpen BIOS (`neopen.sp1`, ISC) is an optional entry of FBNeo's `neogeo` set. Whether FBNeo boots with it in place of the SNK files is UNVERIFIED.
- No free Cave, CPS or vertical set is known. `neptron-testcore` covers the rotation path: a 320x240 frame on its side with `SET_ROTATION` 1, 2 and 3 (`R90`, `R180`, `R270`), with an aspect of 3:4 and with no aspect.

### What only the owner's sets show

The owner's own sets and `neogeo.zip` stay outside the repository, on the development machine and the Pi.

1. A headless run of the attract mode of `mslugx` and `ddonpach`, 3 minutes, with no input. Report p50, p99 and the maximum of the CPU time of `retro_run` against the core budget. The research estimates the budget at about 11 ms on the Pi 3 (UNVERIFIED); the measurement task sets it from the frame statistics of OP-103.
2. The owner's play input, recorded once with `neptron-testkit` and replayed, for the heaviest scenes (DoDonPachi late stages, Metal Slug X boss fights).
3. The same on the PC.
4. `fields_missed = 0` on composite and on HDMI.
5. The 480i picture of DoDonPachi on the PC CRT and on the TV.

The results go into `docs/hardware-checklist.md` through M13. No hardware result is claimed before that.

## 18. Risks

| Risk | What we do |
| --- | --- |
| Pi 3 speed with no Cyclone on aarch64. Cave and CPS-2 are most at risk | The separate measurement decides the Pi 3 list. A failing family is "may be slow" |
| FBNeo changes its sets between commits ("might change from time to time") | One pinned commit. The index and the diagnostics name it |
| The donation clause of the FBNeo licence | ADR 0014: the project takes no donations; a later plan removes FBNeo first |
| FBNeo's own error screen when a set passes the check and still fails | The check before launch; the session ends the game at `SHUTDOWN` |
| 480i on Intel and on the Pi `vc4` not proven (OP-103 point 5) | The bench session checks it. `display.vertical = fit` is the way out |
| A turned game shows stretched on a multi-sync tube and on every fit (section 11, finding 1) | W1 of M08c, before M08a |
| A display profile with a range of one line rate gets no mode for some boards (section 11, finding 3) | W3 of M08c. The validator refuses the mode, so nothing unsafe reaches the display |
| Build time and image size: the full FBNeo build is large (a 21.4 MB zip on the libretro buildbot) | M07b measures both. The subset builds are the later size option |
| The core's NVRAM writes are not atomic | Recorded. A patch to the core is a later item |
| Select and Start held together open the quick menu | Bench check with a stick |
| PAL composite runs every board 9 to 16 % slow | Open point A5 |
| The aspect of a turned source: the core's value is the upright shape | The session inverts a reported aspect, never the host's fallback (section 11); test core cases and the bench check |

## 19. Open points

These are questions with a default. The orchestrator records each in `docs/open-points.md` with its id.

| Label | Question | Default | Where | How to change |
| --- | --- | --- | --- | --- |
| A1 | Which XML reader for the DAT in xtask? | `roxmltree` | `xtask`, root `Cargo.toml` | Another reader, or a small parser of the ClrMamePro XML subset |
| A2 | How does the image build run the index generator? | A post-install hook of `libretro-fbneo` | `os/package/libretro-fbneo` | A step in `cargo xtask os build` before Buildroot |
| A3 | Which BIOS files must a Neo Geo BIOS zip hold, and which game ROMs are optional (`BRF_OPT`)? | For the BIOS, stop only when no file matches and list the rest on the diagnostics page; ask for every game ROM of the DAT | `neptron-library` (`arcade::check_set`) | Read the `BRF_OPT` and `BRF_BIOS` flags of FBNeo's Neo Geo driver into the index, and require the files that the selected BIOS needs |
| A4 | Where does the frontend keep the one-time notices it has shown? | `state/config/notices.toml`, atomic write | `apps/neptron` | A settings key, or another state file |
| A5 | PAL composite runs arcade boards 9 to 16 % slow. Accept for 0.1? | Accept, with the speed alert, as for NTSC | `crates/neptron-modeline/src/policy/tv.rs` | The "keep game speed" choice |
| A6 | TATE: `screen_orientation` with one `vertical` choice and a direction, or two choices? | Decided by the TATE task with the owner | `crates/neptron-settings` | The TATE ADR |

The owner decided A7 to A13 on 2026-11-16. Section 11 ("The decisions of 2026-11-16") holds the decisions.

## Appendix A: the arcade policy table

The table of section 11 after the change, made by `cargo xtask test -p neptron-modeline --no-capture arcade_table` on 2026-11-16. Section 11 explains the columns. The rule column gives "ok", an allowed reason, or the first rule that the cell breaks. The table before the change is in the history of this file (commit "docs(spec): decide the arcade video rule from the policy table").

### `ddonpach`: 320x240 frame, R90, 57.550 Hz

| Display | Rung | Mode | Total | kHz | Hz | Rate | Scale | Filter | Height | Border | Shape | Best whole | Rule |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| generic_15 | interlaced 1:1 | 597x448i | 545 | 15.682 | 57.550 | exact | 1 | nearest | 71.4 % | 128 | +0.1 % | 1x 320i | ok |
| arcade_15 | interlaced 1:1 | 597x448i | 545 | 15.682 | 57.550 | exact | 1 | nearest | 71.4 % | 128 | +0.1 % | 1x 320i | ok |
| arcade_15ex | interlaced 1:1 | 597x448i | 545 | 15.682 | 57.550 | exact | 1 | nearest | 71.4 % | 128 | +0.1 % | 1x 320i | ok |
| h9110 | interlaced 1:1 | 597x448i | 545 | 15.682 | 57.550 | exact | 1 | nearest | 71.4 % | 128 | +0.1 % | 1x 320i | ok |
| polo | interlaced 1:1 | 597x448i | 545 | 15.682 | 57.550 | exact | 1 | nearest | 71.4 % | 128 | +0.1 % | 1x 320i | ok |
| k7131 | interlaced 1:1 | 597x448i | 545 | 15.682 | 57.550 | exact | 1 | nearest | 71.4 % | 128 | +0.1 % | 1x 320i | ok |
| k7000 | interlaced 1:1 | 597x448i | 545 | 15.682 | 57.550 | exact | 1 | nearest | 71.4 % | 128 | +0.1 % | 1x 320i | ok |
| ntsc | interlaced 1:1 | 597x448i | 525 | 15.734 | 59.940 | nearest in range +4.15 % | 1 | nearest | 71.4 % | 128 | +0.1 % | none | ok |
| pal | interlaced 1:1 | 597x448i | 625 | 15.625 | 50.000 | nearest in range -13.12 % | 1 | nearest | 71.4 % | 128 | +0.1 % | none | ok |
| arcade_15_25 | native | 533x400p | 433 | 24.919 | 57.550 | exact | 1 | nearest | 80.0 % | 80 | +0.1 % | 1x 320p | ok |
| arcade_15_25_31 | native | 533x400p | 433 | 24.919 | 57.550 | exact | 1 | nearest | 80.0 % | 80 | +0.1 % | 1x 320p | ok |
| arcade_15_31 | native | 668x501p | 546 | 31.422 | 57.550 | exact | 1 | nearest | 63.8 % | 181 | +0.0 % | 1x 320p | ok |
| m3129 | native | 659x494p | 539 | 31.019 | 57.550 | exact | 1 | nearest | 64.7 % | 174 | -0.1 % | 1x 320p | ok |
| d9200 | native | 659x494p | 539 | 31.019 | 57.550 | exact | 1 | nearest | 64.7 % | 174 | -0.1 % | 1x 320p | ok |
| d9400 | native | 427x320p | 357 | 20.545 | 57.550 | exact | 1 | nearest | 100.0 % | 0 | -0.1 % | 1x 320p | ok |
| d9800 | native | 427x320p | 357 | 20.545 | 57.550 | exact | 1 | nearest | 100.0 % | 0 | -0.1 % | 1x 320p | ok |
| ms929 | native | 512x384p | 425 | 24.458 | 57.550 | exact | 1 | nearest | 83.3 % | 64 | +0.0 % | 1x 320p | ok |
| ms2930 | native | 512x384p | 427 | 24.573 | 57.550 | exact | 1 | nearest | 83.3 % | 64 | +0.0 % | 1x 320p | ok |
| r666b | native | 512x384p | 427 | 24.573 | 57.550 | exact | 1 | nearest | 83.3 % | 64 | +0.0 % | 1x 320p | ok |
| pstar | native | 533x400p | 440 | 25.322 | 57.550 | exact | 1 | nearest | 80.0 % | 80 | +0.1 % | 1x 320p | ok |
| arcade_25 | native | 533x400p | 433 | 24.919 | 57.550 | exact | 1 | nearest | 80.0 % | 80 | +0.1 % | 1x 320p | ok |
| arcade_31 | native | 668x501p | 546 | 31.422 | 57.550 | exact | 1 | nearest | 63.8 % | 181 | +0.0 % | 1x 320p | ok |
| pc_31_120 | native | 683x512p | 546 | 31.422 | 57.550 | exact | 1 | nearest | 62.5 % | 192 | +0.0 % | 1x 320p | ok |
| pc_70_120 | 2x | 427x640p | 668 | 38.443 | 57.550 | exact | 2 | nearest | 100.0 % | 0 | -0.1 % | 3x 960p | ok |
| m2929 | 2x | 427x640p | 662 | 38.098 | 57.550 | exact | 2 | nearest | 100.0 % | 0 | -0.1 % | 2x 640p | ok |
| edid_31 | native | 693x520p | 539 | 31.019 | 57.550 | exact | 1 | nearest | 61.5 % | 200 | +0.0 % | 1x 320p | ok |
| edid_30_54 | 2x | 427x640p | 664 | 38.213 | 57.550 | exact | 2 | nearest | 100.0 % | 0 | -0.1 % | 2x 640p | ok |
| edid_30_70 | 2x | 427x640p | 664 | 38.213 | 57.550 | exact | 2 | nearest | 100.0 % | 0 | -0.1 % | 3x 960p | ok |
| edid_30_96 | 2x | 427x640p | 665 | 38.270 | 57.550 | exact | 2 | nearest | 100.0 % | 0 | -0.1 % | 4x 1280p | ok |
| panel_720 | panel | 1280x720p | 750 | 45.000 | 60.000 | +4.26 % | 2 | nearest | 88.8 % | 80 | +0.0 % | - | ok |
| panel_768 | panel | 1366x768p | 798 | 47.712 | 59.790 | +3.89 % | 2 | nearest | 83.3 % | 128 | +0.0 % | - | ok |
| panel_1080 | panel | 1920x1080p | 1125 | 67.500 | 60.000 | +4.26 % | 3 | nearest | 88.8 % | 120 | +0.0 % | - | ok |
| panel_1440 | panel | 2560x1440p | 1481 | 88.786 | 59.951 | +4.17 % | 4.50 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| panel_2160 | panel | 3840x2160p | 2250 | 135.000 | 60.000 | +4.26 % | 6.75 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| tv_ntsc | interlaced 1:1 | 720x480i | 525 | 15.734 | 59.940 | +4.15 % | 1 | sharp | 66.6 % | 160 | +0.0 % | - | ok |
| tv_pal | interlaced 1:1 | 720x576i | 625 | 15.625 | 50.000 | -13.12 % | 1 | sharp | 55.5 % | 256 | +0.0 % | - | ok |

### `batsugun`: 320x240 frame, R90, 59.630 Hz

| Display | Rung | Mode | Total | kHz | Hz | Rate | Scale | Filter | Height | Border | Shape | Best whole | Rule |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| generic_15 | interlaced 1:1 | 597x448i | 527 | 15.712 | 59.630 | exact | 1 | nearest | 71.4 % | 128 | +0.1 % | 1x 320i | ok |
| arcade_15 | interlaced 1:1 | 597x448i | 527 | 15.712 | 59.630 | exact | 1 | nearest | 71.4 % | 128 | +0.1 % | 1x 320i | ok |
| arcade_15ex | interlaced 1:1 | 597x448i | 527 | 15.712 | 59.630 | exact | 1 | nearest | 71.4 % | 128 | +0.1 % | 1x 320i | ok |
| h9110 | interlaced 1:1 | 597x448i | 527 | 15.712 | 59.630 | exact | 1 | nearest | 71.4 % | 128 | +0.1 % | 1x 320i | ok |
| polo | interlaced 1:1 | 597x448i | 527 | 15.712 | 59.630 | exact | 1 | nearest | 71.4 % | 128 | +0.1 % | 1x 320i | ok |
| k7131 | interlaced 1:1 | 597x448i | 527 | 15.712 | 59.630 | exact | 1 | nearest | 71.4 % | 128 | +0.1 % | 1x 320i | ok |
| k7000 | interlaced 1:1 | 597x448i | 527 | 15.712 | 59.630 | exact | 1 | nearest | 71.4 % | 128 | +0.1 % | 1x 320i | ok |
| ntsc | interlaced 1:1 | 597x448i | 525 | 15.734 | 59.940 | nearest in range +0.52 % | 1 | nearest | 71.4 % | 128 | +0.1 % | none | ok |
| pal | interlaced 1:1 | 597x448i | 625 | 15.625 | 50.000 | nearest in range -16.15 % | 1 | nearest | 71.4 % | 128 | +0.1 % | none | ok |
| arcade_15_25 | native | 515x386p | 418 | 24.925 | 59.630 | exact | 1 | nearest | 82.9 % | 66 | -0.1 % | 1x 320p | ok |
| arcade_15_25_31 | native | 515x386p | 418 | 24.925 | 59.630 | exact | 1 | nearest | 82.9 % | 66 | -0.1 % | 1x 320p | ok |
| arcade_15_31 | native | 643x482p | 527 | 31.424 | 59.630 | exact | 1 | nearest | 66.3 % | 162 | -0.1 % | 1x 320p | ok |
| m3129 | native | 633x475p | 520 | 31.007 | 59.630 | exact | 1 | nearest | 67.3 % | 155 | +0.1 % | 1x 320p | ok |
| d9200 | native | 633x475p | 520 | 31.007 | 59.630 | exact | 1 | nearest | 67.3 % | 155 | +0.1 % | 1x 320p | ok |
| d9400 | native | 427x320p | 355 | 21.168 | 59.630 | exact | 1 | nearest | 100.0 % | 0 | -0.1 % | 1x 320p | ok |
| d9800 | native | 427x320p | 355 | 21.168 | 59.630 | exact | 1 | nearest | 100.0 % | 0 | -0.1 % | 1x 320p | ok |
| ms929 | interlaced 1:1 | 597x448i | 521 | 15.533 | 59.630 | exact | 1 | nearest | 71.4 % | 128 | +0.1 % | 1x 320i | ok |
| ms2930 | native | 640x480p | 528 | 31.484 | 59.630 | exact | 1 | nearest | 66.6 % | 160 | +0.0 % | 1x 320p | ok |
| r666b | native | 631x473p | 520 | 31.007 | 59.630 | exact | 1 | nearest | 67.6 % | 153 | -0.1 % | 1x 320p | ok |
| pstar | native | 533x400p | 425 | 25.342 | 59.630 | exact | 1 | nearest | 80.0 % | 80 | +0.1 % | 1x 320p | ok |
| arcade_25 | native | 515x386p | 418 | 24.925 | 59.630 | exact | 1 | nearest | 82.9 % | 66 | -0.1 % | 1x 320p | ok |
| arcade_31 | native | 643x482p | 527 | 31.424 | 59.630 | exact | 1 | nearest | 66.3 % | 162 | -0.1 % | 1x 320p | ok |
| pc_31_120 | native | 679x509p | 527 | 31.425 | 59.630 | exact | 1 | nearest | 62.8 % | 189 | +0.0 % | 1x 320p | ok |
| pc_70_120 | 2x | 427x640p | 669 | 39.892 | 59.630 | exact | 2 | nearest | 100.0 % | 0 | -0.1 % | 3x 960p | ok |
| m2929 | 2x | 427x640p | 663 | 39.535 | 59.630 | exact | 2 | nearest | 100.0 % | 0 | -0.1 % | 2x 640p | ok |
| edid_31 | native | 668x501p | 520 | 31.007 | 59.629 | exact | 1 | nearest | 63.8 % | 181 | +0.0 % | 1x 320p | ok |
| edid_30_54 | 2x | 427x640p | 665 | 39.654 | 59.630 | exact | 2 | nearest | 100.0 % | 0 | -0.1 % | 2x 640p | ok |
| edid_30_70 | 2x | 427x640p | 665 | 39.654 | 59.630 | exact | 2 | nearest | 100.0 % | 0 | -0.1 % | 3x 960p | ok |
| edid_30_96 | 2x | 427x640p | 666 | 39.713 | 59.630 | exact | 2 | nearest | 100.0 % | 0 | -0.1 % | 4x 1280p | ok |
| panel_720 | panel | 1280x720p | 750 | 45.000 | 60.000 | +0.62 % | 2 | nearest | 88.8 % | 80 | +0.0 % | - | ok |
| panel_768 | panel | 1366x768p | 798 | 47.712 | 59.790 | +0.27 % | 2 | nearest | 83.3 % | 128 | +0.0 % | - | ok |
| panel_1080 | panel | 1920x1080p | 1125 | 67.500 | 60.000 | +0.62 % | 3 | nearest | 88.8 % | 120 | +0.0 % | - | ok |
| panel_1440 | panel | 2560x1440p | 1481 | 88.786 | 59.951 | +0.54 % | 4.50 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| panel_2160 | panel | 3840x2160p | 2250 | 135.000 | 60.000 | +0.62 % | 6.75 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| tv_ntsc | interlaced 1:1 | 720x480i | 525 | 15.734 | 59.940 | +0.52 % | 1 | sharp | 66.6 % | 160 | +0.0 % | - | ok |
| tv_pal | interlaced 1:1 | 720x576i | 625 | 15.625 | 50.000 | -16.15 % | 1 | sharp | 55.5 % | 256 | +0.0 % | - | ok |

### `gunbird`: 320x224 frame, R90, 59.290 Hz

| Display | Rung | Mode | Total | kHz | Hz | Rate | Scale | Filter | Height | Border | Shape | Best whole | Rule |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| generic_15 | interlaced 1:1 | 558x448i | 529 | 15.682 | 59.290 | exact | 1 | nearest | 71.4 % | 128 | -0.1 % | 1x 320i | ok |
| arcade_15 | interlaced 1:1 | 558x448i | 529 | 15.682 | 59.290 | exact | 1 | nearest | 71.4 % | 128 | -0.1 % | 1x 320i | ok |
| arcade_15ex | interlaced 1:1 | 558x448i | 529 | 15.682 | 59.290 | exact | 1 | nearest | 71.4 % | 128 | -0.1 % | 1x 320i | ok |
| h9110 | interlaced 1:1 | 558x448i | 529 | 15.682 | 59.290 | exact | 1 | nearest | 71.4 % | 128 | -0.1 % | 1x 320i | ok |
| polo | interlaced 1:1 | 558x448i | 529 | 15.682 | 59.290 | exact | 1 | nearest | 71.4 % | 128 | -0.1 % | 1x 320i | ok |
| k7131 | interlaced 1:1 | 558x448i | 529 | 15.682 | 59.290 | exact | 1 | nearest | 71.4 % | 128 | -0.1 % | 1x 320i | ok |
| k7000 | interlaced 1:1 | 558x448i | 529 | 15.682 | 59.290 | exact | 1 | nearest | 71.4 % | 128 | -0.1 % | 1x 320i | ok |
| ntsc | interlaced 1:1 | 558x448i | 525 | 15.734 | 59.940 | nearest in range +1.10 % | 1 | nearest | 71.4 % | 128 | -0.1 % | none | ok |
| pal | interlaced 1:1 | 558x448i | 625 | 15.625 | 50.000 | nearest in range -15.67 % | 1 | nearest | 71.4 % | 128 | -0.1 % | none | ok |
| arcade_15_25 | native | 483x388p | 420 | 24.901 | 59.290 | exact | 1 | nearest | 82.4 % | 68 | +0.0 % | 1x 320p | ok |
| arcade_15_25_31 | native | 483x388p | 420 | 24.901 | 59.290 | exact | 1 | nearest | 82.4 % | 68 | +0.0 % | 1x 320p | ok |
| arcade_15_31 | native | 604x485p | 530 | 31.423 | 59.290 | exact | 1 | nearest | 65.9 % | 165 | -0.1 % | 1x 320p | ok |
| m3129 | native | 595x478p | 523 | 31.008 | 59.289 | exact | 1 | nearest | 66.9 % | 158 | +0.0 % | 1x 320p | ok |
| d9200 | native | 595x478p | 523 | 31.008 | 59.289 | exact | 1 | nearest | 66.9 % | 158 | +0.0 % | 1x 320p | ok |
| d9400 | native | 398x320p | 355 | 21.047 | 59.289 | exact | 1 | nearest | 100.0 % | 0 | +0.1 % | 1x 320p | ok |
| d9800 | native | 398x320p | 355 | 21.047 | 59.289 | exact | 1 | nearest | 100.0 % | 0 | +0.1 % | 1x 320p | ok |
| ms929 | interlaced 1:1 | 558x448i | 523 | 15.504 | 59.289 | exact | 1 | nearest | 71.4 % | 128 | -0.1 % | 1x 320i | ok |
| ms2930 | native | 597x480p | 527 | 31.245 | 59.290 | exact | 1 | nearest | 66.6 % | 160 | +0.1 % | 1x 320p | ok |
| r666b | native | 592x476p | 523 | 31.008 | 59.289 | exact | 1 | nearest | 67.2 % | 156 | +0.1 % | 1x 320p | ok |
| pstar | native | 498x400p | 427 | 25.317 | 59.290 | exact | 1 | nearest | 80.0 % | 80 | +0.0 % | 1x 320p | ok |
| arcade_25 | native | 483x388p | 420 | 24.901 | 59.290 | exact | 1 | nearest | 82.4 % | 68 | +0.0 % | 1x 320p | ok |
| arcade_31 | native | 604x485p | 530 | 31.423 | 59.290 | exact | 1 | nearest | 65.9 % | 165 | -0.1 % | 1x 320p | ok |
| pc_31_120 | native | 637x512p | 530 | 31.423 | 59.290 | exact | 1 | nearest | 62.5 % | 192 | +0.0 % | 1x 320p | ok |
| pc_70_120 | 2x | 398x640p | 669 | 39.664 | 59.290 | exact | 2 | nearest | 100.0 % | 0 | +0.1 % | 3x 960p | ok |
| m2929 | 2x | 398x640p | 663 | 39.309 | 59.290 | exact | 2 | nearest | 100.0 % | 0 | +0.1 % | 2x 640p | ok |
| edid_31 | native | 627x504p | 523 | 31.008 | 59.290 | exact | 1 | nearest | 63.4 % | 184 | +0.0 % | 1x 320p | ok |
| edid_30_54 | 2x | 398x640p | 665 | 39.428 | 59.290 | exact | 2 | nearest | 100.0 % | 0 | +0.1 % | 2x 640p | ok |
| edid_30_70 | 2x | 398x640p | 665 | 39.428 | 59.290 | exact | 2 | nearest | 100.0 % | 0 | +0.1 % | 3x 960p | ok |
| edid_30_96 | 2x | 398x640p | 666 | 39.487 | 59.290 | exact | 2 | nearest | 100.0 % | 0 | +0.1 % | 4x 1280p | ok |
| panel_720 | panel | 1280x720p | 750 | 45.000 | 60.000 | +1.20 % | 2 | sharp | 88.8 % | 80 | +0.0 % | - | ok |
| panel_768 | panel | 1366x768p | 798 | 47.712 | 59.790 | +0.84 % | 2 | sharp | 83.3 % | 128 | +0.0 % | - | ok |
| panel_1080 | panel | 1920x1080p | 1125 | 67.500 | 60.000 | +1.20 % | 3 | sharp | 88.8 % | 120 | +0.0 % | - | ok |
| panel_1440 | panel | 2560x1440p | 1481 | 88.786 | 59.951 | +1.11 % | 4.50 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| panel_2160 | panel | 3840x2160p | 2250 | 135.000 | 60.000 | +1.20 % | 6.75 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| tv_ntsc | interlaced 1:1 | 720x480i | 525 | 15.734 | 59.940 | +1.10 % | 1 | sharp | 66.6 % | 160 | +0.0 % | - | ok |
| tv_pal | interlaced 1:1 | 720x576i | 625 | 15.625 | 50.000 | -15.67 % | 1 | nearest | 55.5 % | 256 | +1.8 % | - | ok |

### `gunlock`: 320x224 frame, R270, 58.940 Hz

| Display | Rung | Mode | Total | kHz | Hz | Rate | Scale | Filter | Height | Border | Shape | Best whole | Rule |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| generic_15 | interlaced 1:1 | 558x448i | 533 | 15.707 | 58.940 | exact | 1 | nearest | 71.4 % | 128 | -0.1 % | 1x 320i | ok |
| arcade_15 | interlaced 1:1 | 558x448i | 533 | 15.707 | 58.940 | exact | 1 | nearest | 71.4 % | 128 | -0.1 % | 1x 320i | ok |
| arcade_15ex | interlaced 1:1 | 558x448i | 533 | 15.707 | 58.940 | exact | 1 | nearest | 71.4 % | 128 | -0.1 % | 1x 320i | ok |
| h9110 | interlaced 1:1 | 558x448i | 533 | 15.707 | 58.940 | exact | 1 | nearest | 71.4 % | 128 | -0.1 % | 1x 320i | ok |
| polo | interlaced 1:1 | 558x448i | 533 | 15.707 | 58.940 | exact | 1 | nearest | 71.4 % | 128 | -0.1 % | 1x 320i | ok |
| k7131 | interlaced 1:1 | 558x448i | 533 | 15.707 | 58.940 | exact | 1 | nearest | 71.4 % | 128 | -0.1 % | 1x 320i | ok |
| k7000 | interlaced 1:1 | 558x448i | 533 | 15.707 | 58.940 | exact | 1 | nearest | 71.4 % | 128 | -0.1 % | 1x 320i | ok |
| ntsc | interlaced 1:1 | 558x448i | 525 | 15.734 | 59.940 | nearest in range +1.70 % | 1 | nearest | 71.4 % | 128 | -0.1 % | none | ok |
| pal | interlaced 1:1 | 558x448i | 625 | 15.625 | 50.000 | nearest in range -15.17 % | 1 | nearest | 71.4 % | 128 | -0.1 % | none | ok |
| arcade_15_25 | native | 487x391p | 423 | 24.931 | 58.940 | exact | 1 | nearest | 81.8 % | 71 | -0.1 % | 1x 320p | ok |
| arcade_15_25_31 | native | 487x391p | 423 | 24.931 | 58.940 | exact | 1 | nearest | 81.8 % | 71 | -0.1 % | 1x 320p | ok |
| arcade_15_31 | native | 607x488p | 533 | 31.415 | 58.940 | exact | 1 | nearest | 65.5 % | 168 | +0.0 % | 1x 320p | ok |
| m3129 | native | 599x481p | 526 | 31.002 | 58.940 | exact | 1 | nearest | 66.5 % | 161 | -0.1 % | 1x 320p | ok |
| d9200 | native | 599x481p | 526 | 31.002 | 58.940 | exact | 1 | nearest | 66.5 % | 161 | -0.1 % | 1x 320p | ok |
| d9400 | native | 398x320p | 355 | 20.923 | 58.940 | exact | 1 | nearest | 100.0 % | 0 | +0.1 % | 1x 320p | ok |
| d9800 | native | 398x320p | 355 | 20.923 | 58.940 | exact | 1 | nearest | 100.0 % | 0 | +0.1 % | 1x 320p | ok |
| ms929 | interlaced 1:1 | 558x448i | 527 | 15.530 | 58.940 | exact | 1 | nearest | 71.4 % | 128 | -0.1 % | 1x 320i | ok |
| ms2930 | native | 597x480p | 527 | 31.061 | 58.940 | exact | 1 | nearest | 66.6 % | 160 | +0.1 % | 1x 320p | ok |
| r666b | native | 596x479p | 526 | 31.002 | 58.941 | exact | 1 | nearest | 66.8 % | 159 | +0.0 % | 1x 320p | ok |
| pstar | native | 498x400p | 430 | 25.344 | 58.940 | exact | 1 | nearest | 80.0 % | 80 | +0.0 % | 1x 320p | ok |
| arcade_25 | native | 487x391p | 423 | 24.931 | 58.940 | exact | 1 | nearest | 81.8 % | 71 | -0.1 % | 1x 320p | ok |
| arcade_31 | native | 607x488p | 533 | 31.415 | 58.940 | exact | 1 | nearest | 65.5 % | 168 | +0.0 % | 1x 320p | ok |
| pc_31_120 | native | 637x512p | 533 | 31.415 | 58.940 | exact | 1 | nearest | 62.5 % | 192 | +0.0 % | 1x 320p | ok |
| pc_70_120 | 2x | 398x640p | 669 | 39.430 | 58.940 | exact | 2 | nearest | 100.0 % | 0 | +0.1 % | 3x 960p | ok |
| m2929 | 2x | 398x640p | 662 | 39.017 | 58.939 | exact | 2 | nearest | 100.0 % | 0 | +0.1 % | 2x 640p | ok |
| edid_31 | native | 631x507p | 526 | 31.002 | 58.940 | exact | 1 | nearest | 63.1 % | 187 | +0.0 % | 1x 320p | ok |
| edid_30_54 | 2x | 398x640p | 665 | 39.195 | 58.940 | exact | 2 | nearest | 100.0 % | 0 | +0.1 % | 2x 640p | ok |
| edid_30_70 | 2x | 398x640p | 665 | 39.195 | 58.940 | exact | 2 | nearest | 100.0 % | 0 | +0.1 % | 3x 960p | ok |
| edid_30_96 | 2x | 398x640p | 666 | 39.254 | 58.940 | exact | 2 | nearest | 100.0 % | 0 | +0.1 % | 4x 1280p | ok |
| panel_720 | panel | 1280x720p | 750 | 45.000 | 60.000 | +1.80 % | 2 | sharp | 88.8 % | 80 | +0.0 % | - | ok |
| panel_768 | panel | 1366x768p | 798 | 47.712 | 59.790 | +1.44 % | 2 | sharp | 83.3 % | 128 | +0.0 % | - | ok |
| panel_1080 | panel | 1920x1080p | 1125 | 67.500 | 60.000 | +1.80 % | 3 | sharp | 88.8 % | 120 | +0.0 % | - | ok |
| panel_1440 | panel | 2560x1440p | 1481 | 88.786 | 59.951 | +1.72 % | 4.50 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| panel_2160 | panel | 3840x2160p | 2250 | 135.000 | 60.000 | +1.80 % | 6.75 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| tv_ntsc | interlaced 1:1 | 720x480i | 525 | 15.734 | 59.940 | +1.70 % | 1 | sharp | 66.6 % | 160 | +0.0 % | - | ok |
| tv_pal | interlaced 1:1 | 720x576i | 625 | 15.625 | 50.000 | -15.17 % | 1 | nearest | 55.5 % | 256 | +1.8 % | - | ok |

### `pacman`: 288x224 frame, R270, 60.000 Hz

| Display | Rung | Mode | Total | kHz | Hz | Rate | Scale | Filter | Height | Border | Shape | Best whole | Rule |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| generic_15 | interlaced 1:1 | 619x448i | 523 | 15.689 | 60.000 | exact | 1 | nearest | 64.2 % | 160 | +0.1 % | 1x 288i | ok |
| arcade_15 | interlaced 1:1 | 619x448i | 523 | 15.689 | 60.000 | exact | 1 | nearest | 64.2 % | 160 | +0.1 % | 1x 288i | ok |
| arcade_15ex | interlaced 1:1 | 619x448i | 523 | 15.689 | 60.000 | exact | 1 | nearest | 64.2 % | 160 | +0.1 % | 1x 288i | ok |
| h9110 | interlaced 1:1 | 619x448i | 523 | 15.689 | 60.000 | exact | 1 | nearest | 64.2 % | 160 | +0.1 % | 1x 288i | ok |
| polo | interlaced 1:1 | 619x448i | 523 | 15.689 | 60.000 | exact | 1 | nearest | 64.2 % | 160 | +0.1 % | 1x 288i | ok |
| k7131 | interlaced 1:1 | 619x448i | 523 | 15.689 | 60.000 | exact | 1 | nearest | 64.2 % | 160 | +0.1 % | 1x 288i | ok |
| k7000 | interlaced 1:1 | 619x448i | 523 | 15.689 | 60.000 | exact | 1 | nearest | 64.2 % | 160 | +0.1 % | 1x 288i | ok |
| ntsc | interlaced 1:1 | 619x448i | 525 | 15.734 | 59.940 | nearest in range -0.10 % | 1 | nearest | 64.2 % | 160 | +0.1 % | none | ok |
| pal | native | 398x288p | 312 | 15.600 | 50.000 | nearest in range -16.67 % | 1 | nearest | 100.0 % | 0 | +0.1 % | none | ok |
| arcade_15_25 | native | 531x384p | 416 | 24.959 | 59.999 | exact | 1 | nearest | 75.0 % | 96 | +0.0 % | 1x 288p | ok |
| arcade_15_25_31 | native | 531x384p | 416 | 24.959 | 59.999 | exact | 1 | nearest | 75.0 % | 96 | +0.0 % | 1x 288p | ok |
| arcade_15_31 | native | 662x479p | 524 | 31.440 | 60.000 | exact | 1 | nearest | 60.1 % | 191 | +0.0 % | 1x 288p | ok |
| m3129 | native | 653x472p | 517 | 31.019 | 60.000 | exact | 1 | nearest | 61.0 % | 184 | -0.1 % | 1x 288p | ok |
| d9200 | 2x | 398x576p | 617 | 37.019 | 59.999 | exact | 2 | nearest | 100.0 % | 0 | +0.1 % | 2x 576p | ok |
| d9400 | native | 398x288p | 312 | 18.720 | 60.000 | exact | 1 | nearest | 100.0 % | 0 | +0.1 % | 2x 576p | ok |
| d9800 | native | 398x288p | 312 | 18.720 | 60.000 | exact | 1 | nearest | 100.0 % | 0 | +0.1 % | 2x 576p | ok |
| ms929 | interlaced 1:1 | 619x448i | 517 | 15.509 | 60.000 | exact | 1 | nearest | 64.2 % | 160 | +0.1 % | 1x 288i | ok |
| ms2930 | native | 664x480p | 528 | 31.679 | 60.000 | exact | 1 | nearest | 60.0 % | 192 | +0.0 % | 1x 288p | ok |
| r666b | native | 650x470p | 517 | 31.020 | 60.000 | exact | 1 | nearest | 61.2 % | 182 | +0.0 % | 1x 288p | ok |
| pstar | native | 553x400p | 422 | 25.320 | 60.000 | exact | 1 | nearest | 72.0 % | 112 | +0.0 % | 1x 288p | ok |
| arcade_25 | native | 531x384p | 416 | 24.959 | 59.999 | exact | 1 | nearest | 75.0 % | 96 | +0.0 % | 1x 288p | ok |
| arcade_31 | native | 662x479p | 524 | 31.440 | 60.000 | exact | 1 | nearest | 60.1 % | 191 | +0.0 % | 1x 288p | ok |
| pc_31_120 | native | 700x506p | 524 | 31.439 | 60.000 | exact | 1 | nearest | 56.9 % | 218 | +0.0 % | 1x 288p | ok |
| pc_70_120 | 2x | 398x576p | 602 | 36.119 | 60.000 | exact | 2 | nearest | 100.0 % | 0 | +0.1 % | 3x 864p | ok |
| m2929 | 2x | 398x576p | 597 | 35.819 | 60.000 | exact | 2 | nearest | 100.0 % | 0 | +0.1 % | 2x 576p | ok |
| edid_31 | native | 689x498p | 517 | 31.019 | 60.000 | exact | 1 | nearest | 57.8 % | 210 | -0.1 % | 1x 288p | ok |
| edid_30_54 | 2x | 398x576p | 599 | 35.939 | 60.000 | exact | 2 | nearest | 100.0 % | 0 | +0.1 % | 3x 864p | ok |
| edid_30_70 | 2x | 398x576p | 599 | 35.939 | 60.000 | exact | 2 | nearest | 100.0 % | 0 | +0.1 % | 4x 1120p | ok |
| edid_30_96 | 2x | 398x576p | 599 | 35.939 | 60.000 | exact | 2 | nearest | 100.0 % | 0 | +0.1 % | 4x 1152p | ok |
| panel_720 | panel | 1280x720p | 750 | 45.000 | 60.000 | +0.00 % | 2 | nearest | 80.0 % | 144 | +0.0 % | - | ok |
| panel_768 | panel | 1366x768p | 798 | 47.712 | 59.790 | -0.35 % | 2 | nearest | 75.0 % | 192 | +0.0 % | - | ok |
| panel_1080 | panel | 1920x1080p | 1125 | 67.500 | 60.000 | +0.00 % | 3 | sharp | 80.0 % | 216 | +0.0 % | - | ok |
| panel_1440 | panel | 2560x1440p | 1481 | 88.786 | 59.951 | -0.08 % | 5 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| panel_2160 | panel | 3840x2160p | 2250 | 135.000 | 60.000 | +0.00 % | 7.50 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| tv_ntsc | interlaced 1:1 | 720x480i | 525 | 15.734 | 59.940 | -0.10 % | 1 | sharp | 60.0 % | 192 | +0.2 % | - | ok |
| tv_pal | native | 720x288p | 312 | 15.625 | 50.080 | -16.53 % | 1 | sharp | 100.0 % | 0 | +0.0 % | - | ok |

### `pacman_60606`: 288x224 frame, R270, 60.606 Hz

| Display | Rung | Mode | Total | kHz | Hz | Rate | Scale | Filter | Height | Border | Shape | Best whole | Rule |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| generic_15 | interlaced 1:1 | 619x448i | 517 | 15.666 | 60.606 | exact | 1 | nearest | 64.2 % | 160 | +0.1 % | 1x 288i | ok |
| arcade_15 | interlaced 1:1 | 619x448i | 517 | 15.666 | 60.606 | exact | 1 | nearest | 64.2 % | 160 | +0.1 % | 1x 288i | ok |
| arcade_15ex | interlaced 1:1 | 619x448i | 517 | 15.666 | 60.606 | exact | 1 | nearest | 64.2 % | 160 | +0.1 % | 1x 288i | ok |
| h9110 | interlaced 1:1 | 619x448i | 517 | 15.666 | 60.606 | exact | 1 | nearest | 64.2 % | 160 | +0.1 % | 1x 288i | ok |
| polo | interlaced 1:1 | 619x448i | 517 | 15.666 | 60.606 | exact | 1 | nearest | 64.2 % | 160 | +0.1 % | 1x 288i | ok |
| k7131 | interlaced 1:1 | 619x448i | 517 | 15.666 | 60.606 | exact | 1 | nearest | 64.2 % | 160 | +0.1 % | 1x 288i | ok |
| k7000 | interlaced 1:1 | 619x448i | 517 | 15.666 | 60.606 | exact | 1 | nearest | 64.2 % | 160 | +0.1 % | 1x 288i | ok |
| ntsc | interlaced 1:1 | 619x448i | 525 | 15.734 | 59.940 | nearest in range -1.10 % | 1 | nearest | 64.2 % | 160 | +0.1 % | none | ok |
| pal | native | 398x288p | 312 | 15.600 | 50.000 | nearest in range -17.50 % | 1 | nearest | 100.0 % | 0 | +0.1 % | none | ok |
| arcade_15_25 | interlaced 1:1 | 619x448i | 517 | 15.666 | 60.606 | exact | 1 | nearest | 64.2 % | 160 | +0.1 % | 1x 288i | ok |
| arcade_15_25_31 | native | 654x473p | 519 | 31.454 | 60.606 | exact | 1 | nearest | 60.8 % | 185 | +0.0 % | 1x 288p | ok |
| arcade_15_31 | native | 654x473p | 519 | 31.454 | 60.606 | exact | 1 | nearest | 60.8 % | 185 | +0.0 % | 1x 288p | ok |
| m3129 | native | 646x467p | 512 | 31.029 | 60.605 | exact | 1 | nearest | 61.6 % | 179 | +0.0 % | 1x 288p | ok |
| d9200 | 2x | 398x576p | 611 | 37.030 | 60.606 | exact | 2 | nearest | 100.0 % | 0 | +0.1 % | 2x 576p | ok |
| d9400 | native | 398x288p | 312 | 18.908 | 60.606 | exact | 1 | nearest | 100.0 % | 0 | +0.1 % | 2x 576p | ok |
| d9800 | native | 398x288p | 312 | 18.908 | 60.606 | exact | 1 | nearest | 100.0 % | 0 | +0.1 % | 2x 576p | ok |
| ms929 | interlaced 1:1 | 619x448i | 511 | 15.484 | 60.606 | exact | 1 | nearest | 64.2 % | 160 | +0.1 % | 1x 288i | ok |
| ms2930 | interlaced 1:1 | 619x448i | 511 | 15.484 | 60.606 | exact | 1 | nearest | 64.2 % | 160 | +0.1 % | 1x 288i | ok |
| r666b | native | 643x465p | 512 | 31.030 | 60.606 | exact | 1 | nearest | 61.9 % | 177 | +0.0 % | 1x 288p | ok |
| pstar | native | 552x399p | 418 | 25.333 | 60.606 | exact | 1 | nearest | 72.1 % | 111 | -0.1 % | 1x 288p | ok |
| arcade_25 | native | 531x384p | 416 | 24.959 | 59.999 | nearest in range -1.00 % | 1 | nearest | 75.0 % | 96 | +0.0 % | none | ok: no window at the game's rate |
| arcade_31 | native | 654x473p | 519 | 31.454 | 60.606 | exact | 1 | nearest | 60.8 % | 185 | +0.0 % | 1x 288p | ok |
| pc_31_120 | native | 693x501p | 519 | 31.454 | 60.606 | exact | 1 | nearest | 57.4 % | 213 | +0.0 % | 1x 288p | ok |
| pc_70_120 | 2x | 398x576p | 603 | 36.545 | 60.606 | exact | 2 | nearest | 100.0 % | 0 | +0.1 % | 3x 864p | ok |
| m2929 | 2x | 398x576p | 597 | 36.181 | 60.606 | exact | 2 | nearest | 100.0 % | 0 | +0.1 % | 2x 576p | ok |
| edid_31 | native | 682x493p | 512 | 31.030 | 60.606 | exact | 1 | nearest | 58.4 % | 205 | +0.0 % | 1x 288p | ok |
| edid_30_54 | 2x | 398x576p | 599 | 36.303 | 60.606 | exact | 2 | nearest | 100.0 % | 0 | +0.1 % | 3x 852p | ok |
| edid_30_70 | 2x | 398x576p | 599 | 36.303 | 60.606 | exact | 2 | nearest | 100.0 % | 0 | +0.1 % | 4x 1104p | ok |
| edid_30_96 | 2x | 398x576p | 600 | 36.363 | 60.606 | exact | 2 | nearest | 100.0 % | 0 | +0.1 % | 4x 1152p | ok |
| panel_720 | panel | 1280x720p | 750 | 45.000 | 60.000 | -1.00 % | 2 | nearest | 80.0 % | 144 | +0.0 % | - | ok |
| panel_768 | panel | 1366x768p | 798 | 47.712 | 59.790 | -1.35 % | 2 | nearest | 75.0 % | 192 | +0.0 % | - | ok |
| panel_1080 | panel | 1920x1080p | 1125 | 67.500 | 60.000 | -1.00 % | 3 | sharp | 80.0 % | 216 | +0.0 % | - | ok |
| panel_1440 | panel | 2560x1440p | 1481 | 88.786 | 59.951 | -1.08 % | 5 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| panel_2160 | panel | 3840x2160p | 2250 | 135.000 | 60.000 | -1.00 % | 7.50 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| tv_ntsc | interlaced 1:1 | 720x480i | 525 | 15.734 | 59.940 | -1.10 % | 1 | sharp | 60.0 % | 192 | +0.2 % | - | ok |
| tv_pal | native | 720x288p | 312 | 15.625 | 50.080 | -17.37 % | 1 | sharp | 100.0 % | 0 | +0.0 % | - | ok |

### `dkong`: 256x224 frame, R270, 60.000 Hz

| Display | Rung | Mode | Total | kHz | Hz | Rate | Scale | Filter | Height | Border | Shape | Best whole | Rule |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| generic_15 | native | 373x240p | 261 | 15.659 | 60.000 | exact | 1 | nearest | 100.0 % | 0 | +0.1 % | 1x 240p | ok |
| arcade_15 | native | 386x248p | 269 | 16.140 | 60.000 | exact | 1 | nearest | 100.0 % | 0 | -0.1 % | 1x 248p | ok |
| arcade_15ex | native | 392x252p | 273 | 16.380 | 60.000 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 252p | ok |
| h9110 | native | 398x256p | 277 | 16.620 | 60.000 | exact | 1 | nearest | 100.0 % | 0 | +0.1 % | 1x 256p | ok |
| polo | native | 398x256p | 277 | 16.620 | 60.000 | exact | 1 | nearest | 100.0 % | 0 | +0.1 % | 1x 256p | ok |
| k7131 | native | 398x256p | 277 | 16.620 | 60.000 | exact | 1 | nearest | 100.0 % | 0 | +0.1 % | 1x 256p | ok |
| k7000 | native | 376x242p | 262 | 15.720 | 60.000 | exact | 1 | nearest | 100.0 % | 0 | +0.1 % | 1x 242p | ok |
| ntsc | native | 373x240p | 262 | 15.704 | 59.940 | nearest in range -0.10 % | 1 | nearest | 100.0 % | 0 | +0.1 % | none | ok |
| pal | native | 398x256p | 312 | 15.600 | 50.000 | nearest in range -16.67 % | 1 | nearest | 100.0 % | 0 | +0.1 % | none | ok |
| arcade_15_25 | native | 386x248p | 269 | 16.140 | 60.000 | exact | 1 | nearest | 100.0 % | 0 | -0.1 % | 1x 256p | ok |
| arcade_15_25_31 | native | 386x248p | 269 | 16.140 | 60.000 | exact | 1 | nearest | 100.0 % | 0 | -0.1 % | 1x 256p | ok |
| arcade_15_31 | native | 386x248p | 269 | 16.140 | 60.000 | exact | 1 | nearest | 100.0 % | 0 | -0.1 % | 1x 256p | ok |
| m3129 | native | 389x250p | 273 | 16.380 | 60.000 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 484p | ok |
| d9200 | 2x | 398x512p | 617 | 37.019 | 59.999 | exact | 2 | nearest | 100.0 % | 0 | +0.1 % | 2x 512p | ok |
| d9400 | native | 398x256p | 279 | 16.739 | 60.000 | exact | 1 | nearest | 100.0 % | 0 | +0.1 % | 2x 512p | ok |
| d9800 | native | 398x256p | 279 | 16.739 | 60.000 | exact | 1 | nearest | 100.0 % | 0 | +0.1 % | 2x 512p | ok |
| ms929 | native | 380x244p | 266 | 15.960 | 60.000 | exact | 1 | nearest | 100.0 % | 0 | -0.1 % | 1x 244p | ok |
| ms2930 | native | 376x242p | 267 | 16.020 | 60.001 | exact | 1 | nearest | 100.0 % | 0 | +0.1 % | 2x 484p | ok |
| r666b | native | 376x242p | 267 | 16.020 | 60.001 | exact | 1 | nearest | 100.0 % | 0 | +0.1 % | 2x 488p | ok |
| pstar | native | 398x256p | 270 | 16.200 | 60.000 | exact | 1 | nearest | 100.0 % | 0 | +0.1 % | 2x 500p | ok |
| arcade_25 | native | 597x384p | 416 | 24.959 | 60.000 | exact | 1 | nearest | 66.6 % | 128 | +0.1 % | 1x 256p | ok |
| arcade_31 | native | 745x479p | 524 | 31.439 | 60.000 | exact | 1 | nearest | 53.4 % | 223 | +0.0 % | 1x 256p | ok |
| pc_31_120 | 2x | 392x504p | 524 | 31.439 | 60.000 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 2x 504p | ok |
| pc_70_120 | 2x | 398x512p | 535 | 32.100 | 60.000 | exact | 2 | nearest | 100.0 % | 0 | +0.1 % | 4x 1024p | ok |
| m2929 | 2x | 398x512p | 530 | 31.800 | 60.000 | exact | 2 | nearest | 100.0 % | 0 | +0.1 % | 2x 512p | ok |
| edid_31 | 2x | 398x512p | 531 | 31.859 | 60.000 | exact | 2 | nearest | 100.0 % | 0 | +0.1 % | 2x 512p | ok |
| edid_30_54 | 2x | 398x512p | 532 | 31.919 | 60.000 | exact | 2 | nearest | 100.0 % | 0 | +0.1 % | 3x 768p | ok |
| edid_30_70 | 2x | 398x512p | 532 | 31.919 | 60.000 | exact | 2 | nearest | 100.0 % | 0 | +0.1 % | 4x 1024p | ok |
| edid_30_96 | 2x | 398x512p | 533 | 31.980 | 60.000 | exact | 2 | nearest | 100.0 % | 0 | +0.1 % | 4x 1024p | ok |
| panel_720 | panel | 1280x720p | 750 | 45.000 | 60.000 | +0.00 % | 2 | nearest | 71.1 % | 208 | +0.0 % | - | ok |
| panel_768 | panel | 1366x768p | 798 | 47.712 | 59.790 | -0.35 % | 3 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| panel_1080 | panel | 1920x1080p | 1125 | 67.500 | 60.000 | +0.00 % | 4.22 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| panel_1440 | panel | 2560x1440p | 1481 | 88.786 | 59.951 | -0.08 % | 5.62 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| panel_2160 | panel | 3840x2160p | 2250 | 135.000 | 60.000 | +0.00 % | 8.44 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| tv_ntsc | native | 720x240p | 262 | 15.734 | 60.054 | +0.09 % | 1 | sharp | 100.0 % | 0 | -0.1 % | - | ok |
| tv_pal | native | 720x288p | 312 | 15.625 | 50.080 | -16.53 % | 1 | sharp | 88.8 % | 32 | +0.0 % | - | ok |

### `gground`: 496x384 frame, R270, 57.524 Hz

| Display | Rung | Mode | Total | kHz | Hz | Rate | Scale | Filter | Height | Border | Shape | Best whole | Rule |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| generic_15 | interlaced 1:1 | 683x496i | 545 | 15.675 | 57.524 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 496i | ok |
| arcade_15 | interlaced 1:1 | 683x496i | 545 | 15.675 | 57.524 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 496i | ok |
| arcade_15ex | interlaced 1:1 | 683x496i | 545 | 15.675 | 57.524 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 496i | ok |
| h9110 | interlaced 1:1 | 683x496i | 545 | 15.675 | 57.524 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 496i | ok |
| polo | interlaced 1:1 | 683x496i | 545 | 15.675 | 57.524 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 496i | ok |
| k7131 | interlaced 1:1 | 683x496i | 545 | 15.675 | 57.524 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 496i | ok |
| k7000 | interlaced 1:1 | 683x496i | 545 | 15.675 | 57.524 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 496i | ok |
| ntsc | fitted | 640x480i | 525 | 15.734 | 59.940 | nearest in range +4.20 % | 0.97 | sharp | 100.0 % | 0 | +0.0 % | none | ok: no exact mode exists on this profile |
| pal | interlaced 1:1 | 683x496i | 625 | 15.625 | 50.000 | nearest in range -13.08 % | 1 | nearest | 100.0 % | 0 | +0.0 % | none | ok |
| arcade_15_25 | interlaced 1:1 | 683x496i | 545 | 15.675 | 57.524 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 496i | ok |
| arcade_15_25_31 | native | 683x496p | 546 | 31.408 | 57.524 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 496p | ok |
| arcade_15_31 | native | 683x496p | 546 | 31.408 | 57.524 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 496p | ok |
| m3129 | native | 683x496p | 540 | 31.063 | 57.524 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 496p | ok |
| d9200 | native | 683x496p | 540 | 31.063 | 57.524 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 496p | ok |
| d9400 | native | 683x496p | 557 | 32.040 | 57.524 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 496p | ok |
| d9800 | native | 683x496p | 557 | 32.040 | 57.524 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 496p | ok |
| ms929 | interlaced 1:1 | 683x496i | 541 | 15.560 | 57.524 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 496i | ok |
| ms2930 | native | 683x496p | 543 | 31.235 | 57.524 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 496p | ok |
| r666b | native | 683x496p | 543 | 31.235 | 57.524 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 496p | ok |
| pstar | native | 683x496p | 548 | 31.523 | 57.524 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 496p | ok |
| arcade_25 | interlaced 1:1 | 1057x768i | 867 | 24.936 | 57.524 | exact | 1 | nearest | 64.5 % | 272 | +0.0 % | 1x 496i | ok |
| arcade_31 | native | 683x496p | 546 | 31.408 | 57.524 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 496p | ok |
| pc_31_120 | native | 683x496p | 546 | 31.408 | 57.524 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 496p | ok |
| pc_70_120 | native | 683x496p | 522 | 30.027 | 57.524 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 992p | ok |
| m2929 | native | 683x496p | 522 | 30.027 | 57.524 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 496p | ok |
| edid_31 | native | 683x496p | 539 | 31.005 | 57.525 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 496p | ok |
| edid_30_54 | native | 683x496p | 522 | 30.027 | 57.524 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 496p | ok |
| edid_30_70 | native | 683x496p | 522 | 30.027 | 57.524 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 992p | ok |
| edid_30_96 | native | 683x496p | 522 | 30.027 | 57.524 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 3x 1488p | ok |
| panel_720 | panel | 1280x720p | 750 | 45.000 | 60.000 | +4.30 % | 1 | sharp | 68.8 % | 224 | +0.0 % | - | ok |
| panel_768 | panel | 1366x768p | 798 | 47.712 | 59.790 | +3.94 % | 1 | sharp | 64.5 % | 272 | +0.0 % | - | ok |
| panel_1080 | panel | 1920x1080p | 1125 | 67.500 | 60.000 | +4.30 % | 2 | nearest | 91.8 % | 88 | +0.0 % | - | ok |
| panel_1440 | panel | 2560x1440p | 1481 | 88.786 | 59.951 | +4.22 % | 2 | nearest | 68.8 % | 448 | +0.0 % | - | ok |
| panel_2160 | panel | 3840x2160p | 2250 | 135.000 | 60.000 | +4.30 % | 4.35 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| tv_ntsc | fitted | 720x480i | 525 | 15.734 | 59.940 | +4.20 % | 0.97 | sharp | 100.0 % | 0 | +0.0 % | - | ok: no exact mode exists on this profile |
| tv_pal | interlaced 1:1 | 720x576i | 625 | 15.625 | 50.000 | -13.08 % | 1 | sharp | 86.1 % | 80 | +0.0 % | - | ok |

### `ikaruga`: 640x480 frame, R270, 60.000 Hz

| Display | Rung | Mode | Total | kHz | Hz | Rate | Scale | Filter | Height | Border | Shape | Best whole | Rule |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| generic_15 | fitted | 640x480i | 523 | 15.689 | 60.000 | exact | 0.75 | sharp | 100.0 % | 0 | +0.0 % | none | ok: no exact mode exists on this profile |
| arcade_15 | fitted | 661x496i | 539 | 16.169 | 60.000 | exact | 0.78 | sharp | 100.0 % | 0 | +0.1 % | none | ok: no exact mode exists on this profile |
| arcade_15ex | fitted | 672x504i | 547 | 16.409 | 60.000 | exact | 0.79 | sharp | 100.0 % | 0 | +0.0 % | none | ok: no exact mode exists on this profile |
| h9110 | fitted | 682x512i | 555 | 16.650 | 60.000 | exact | 0.80 | sharp | 100.0 % | 0 | +0.1 % | none | ok: no exact mode exists on this profile |
| polo | fitted | 682x512i | 555 | 16.650 | 60.000 | exact | 0.80 | sharp | 100.0 % | 0 | +0.1 % | none | ok: no exact mode exists on this profile |
| k7131 | fitted | 682x512i | 555 | 16.650 | 60.000 | exact | 0.80 | sharp | 100.0 % | 0 | +0.1 % | none | ok: no exact mode exists on this profile |
| k7000 | fitted | 640x480i | 523 | 15.689 | 60.000 | exact | 0.75 | sharp | 100.0 % | 0 | +0.0 % | none | ok: no exact mode exists on this profile |
| ntsc | fitted | 640x480i | 525 | 15.734 | 59.940 | nearest in range -0.10 % | 0.75 | sharp | 100.0 % | 0 | +0.0 % | none | ok: no exact mode exists on this profile |
| pal | fitted | 768x576i | 625 | 15.625 | 50.000 | nearest in range -16.67 % | 0.90 | sharp | 100.0 % | 0 | +0.0 % | none | ok: no exact mode exists on this profile |
| arcade_15_25 | interlaced 1:1 | 1024x768i | 831 | 24.929 | 60.000 | exact | 1 | nearest | 83.3 % | 128 | +0.0 % | 1x 640i | ok |
| arcade_15_25_31 | interlaced 1:1 | 1024x768i | 831 | 24.929 | 60.000 | exact | 1 | nearest | 83.3 % | 128 | +0.0 % | 1x 640i | ok |
| arcade_15_31 | fitted | 640x480p | 525 | 31.500 | 60.000 | exact | 0.75 | sharp | 100.0 % | 0 | +0.0 % | none | ok: no exact mode exists on this profile |
| m3129 | fitted | 640x480p | 525 | 31.500 | 60.000 | exact | 0.75 | sharp | 100.0 % | 0 | +0.0 % | none | ok: no exact mode exists on this profile |
| d9200 | fitted | 800x600p | 628 | 37.680 | 60.000 | exact | 0.94 | sharp | 100.0 % | 0 | +0.0 % | none | ok: no exact mode exists on this profile |
| d9400 | fitted | 800x600p | 628 | 37.680 | 60.000 | exact | 0.94 | sharp | 100.0 % | 0 | +0.0 % | none | ok: no exact mode exists on this profile |
| d9800 | fitted | 800x600p | 628 | 37.680 | 60.000 | exact | 0.94 | sharp | 100.0 % | 0 | +0.0 % | none | ok: no exact mode exists on this profile |
| ms929 | fitted | 650x488i | 533 | 15.989 | 59.999 | exact | 0.76 | sharp | 100.0 % | 0 | +0.1 % | none | ok: no exact mode exists on this profile |
| ms2930 | fitted | 640x480p | 528 | 31.679 | 60.000 | exact | 0.75 | sharp | 100.0 % | 0 | +0.0 % | none | ok: no exact mode exists on this profile |
| r666b | fitted | 650x488p | 537 | 32.220 | 60.000 | exact | 0.76 | sharp | 100.0 % | 0 | +0.1 % | none | ok: no exact mode exists on this profile |
| pstar | interlaced 1:1 | 1024x768i | 845 | 25.349 | 60.000 | exact | 1 | nearest | 83.3 % | 128 | +0.0 % | 1x 640i | ok |
| arcade_25 | interlaced 1:1 | 1024x768i | 831 | 24.929 | 60.000 | exact | 1 | nearest | 83.3 % | 128 | +0.0 % | 1x 640i | ok |
| arcade_31 | fitted | 640x480p | 525 | 31.500 | 60.000 | exact | 0.75 | sharp | 100.0 % | 0 | +0.0 % | none | ok: the profile declares no interlace |
| pc_31_120 | fitted | 672x504p | 524 | 31.440 | 60.000 | exact | 0.79 | sharp | 100.0 % | 0 | +0.0 % | none | ok: the profile declares no interlace |
| pc_70_120 | native | 853x640p | 669 | 40.140 | 60.000 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 640p | ok |
| m2929 | native | 853x640p | 663 | 39.780 | 60.000 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 640p | ok |
| edid_31 | fitted | 678x509p | 528 | 31.679 | 60.000 | exact | 0.80 | sharp | 100.0 % | 0 | -0.1 % | none | ok: the profile declares no interlace |
| edid_30_54 | native | 853x640p | 665 | 39.900 | 60.000 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 640p | ok |
| edid_30_70 | native | 853x640p | 666 | 39.959 | 60.000 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 640p | ok |
| edid_30_96 | native | 853x640p | 666 | 39.959 | 60.000 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 1280p | ok |
| panel_720 | panel | 1280x720p | 750 | 45.000 | 60.000 | +0.00 % | 1 | nearest | 88.8 % | 80 | +0.0 % | - | ok |
| panel_768 | panel | 1366x768p | 798 | 47.712 | 59.790 | -0.35 % | 1 | nearest | 83.3 % | 128 | +0.0 % | - | ok |
| panel_1080 | panel | 1920x1080p | 1125 | 67.500 | 60.000 | +0.00 % | 1 | nearest | 59.2 % | 440 | +0.0 % | - | ok |
| panel_1440 | panel | 2560x1440p | 1481 | 88.786 | 59.951 | -0.08 % | 2 | nearest | 88.8 % | 160 | +0.0 % | - | ok |
| panel_2160 | panel | 3840x2160p | 2250 | 135.000 | 60.000 | +0.00 % | 3 | nearest | 88.8 % | 240 | +0.0 % | - | ok |
| tv_ntsc | fitted | 720x480i | 525 | 15.734 | 59.940 | -0.10 % | 0.75 | sharp | 100.0 % | 0 | +0.0 % | - | ok: no exact mode exists on this profile |
| tv_pal | fitted | 720x576i | 625 | 15.625 | 50.000 | -16.67 % | 0.90 | sharp | 100.0 % | 0 | +0.0 % | - | ok: no exact mode exists on this profile |

### `neogeo`: 304x224 frame, R0, 59.180 Hz

| Display | Rung | Mode | Total | kHz | Hz | Rate | Scale | Filter | Height | Border | Shape | Best whole | Rule |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| generic_15 | native | 304x224p | 265 | 15.682 | 59.180 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 224p | ok |
| arcade_15 | native | 304x224p | 265 | 15.682 | 59.180 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 224p | ok |
| arcade_15ex | native | 304x224p | 265 | 15.682 | 59.180 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 224p | ok |
| h9110 | native | 304x224p | 265 | 15.682 | 59.180 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 224p | ok |
| polo | native | 304x224p | 265 | 15.682 | 59.180 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 224p | ok |
| k7131 | native | 304x224p | 265 | 15.682 | 59.180 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 224p | ok |
| k7000 | native | 304x224p | 265 | 15.682 | 59.180 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 224p | ok |
| ntsc | native | 304x224p | 262 | 15.704 | 59.940 | nearest in range +1.28 % | 1 | nearest | 100.0 % | 0 | +0.0 % | none | ok |
| pal | native | 304x224p | 312 | 15.600 | 50.000 | nearest in range -15.51 % | 1 | nearest | 100.0 % | 0 | +0.0 % | none | ok |
| arcade_15_25 | native | 304x224p | 265 | 15.682 | 59.180 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 224p | ok |
| arcade_15_25_31 | native | 304x224p | 265 | 15.682 | 59.180 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 448p | ok |
| arcade_15_31 | native | 304x224p | 265 | 15.682 | 59.180 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 448p | ok |
| m3129 | native | 304x224p | 258 | 15.268 | 59.180 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 448p | ok |
| d9200 | native | 304x224p | 258 | 15.268 | 59.180 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 448p | ok |
| d9400 | native | 304x224p | 258 | 15.268 | 59.180 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 448p | ok |
| d9800 | native | 304x224p | 258 | 15.268 | 59.180 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 448p | ok |
| ms929 | native | 304x224p | 262 | 15.505 | 59.180 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 224p | ok |
| ms2930 | native | 304x224p | 262 | 15.505 | 59.180 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 448p | ok |
| r666b | native | 304x224p | 262 | 15.505 | 59.180 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 448p | ok |
| pstar | native | 304x224p | 266 | 15.741 | 59.180 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 448p | ok |
| arcade_25 | native | 528x389p | 421 | 24.914 | 59.180 | exact | 1 | nearest | 57.5 % | 165 | +0.0 % | 1x 224p | ok |
| arcade_31 | 2x | 304x448p | 531 | 31.424 | 59.180 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 2x 448p | ok |
| pc_31_120 | 2x | 304x448p | 531 | 31.424 | 59.180 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 2x 448p | ok |
| pc_70_120 | 2x | 304x448p | 507 | 30.005 | 59.182 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 4x 896p | ok |
| m2929 | 2x | 304x448p | 507 | 30.005 | 59.182 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 3x 636p | ok |
| edid_31 | 2x | 304x448p | 524 | 31.010 | 59.180 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 2x 448p | ok |
| edid_30_54 | 2x | 304x448p | 507 | 30.005 | 59.182 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 4x 872p | ok |
| edid_30_70 | 2x | 304x448p | 507 | 30.005 | 59.182 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 4x 896p | ok |
| edid_30_96 | 2x | 304x448p | 507 | 30.005 | 59.182 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 4x 896p | ok |
| panel_720 | panel | 1280x720p | 750 | 45.000 | 60.000 | +1.39 % | 3 | sharp | 93.3 % | 48 | +0.0 % | - | ok |
| panel_768 | panel | 1366x768p | 798 | 47.712 | 59.790 | +1.03 % | 3 | sharp | 87.5 % | 96 | +0.0 % | - | ok |
| panel_1080 | panel | 1920x1080p | 1125 | 67.500 | 60.000 | +1.39 % | 4.82 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| panel_1440 | panel | 2560x1440p | 1481 | 88.786 | 59.951 | +1.30 % | 6.43 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| panel_2160 | panel | 3840x2160p | 2250 | 135.000 | 60.000 | +1.39 % | 9.64 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| tv_ntsc | native | 720x240p | 262 | 15.734 | 60.054 | +1.48 % | 1 | sharp | 93.3 % | 16 | +0.0 % | - | ok |
| tv_pal | native | 720x288p | 312 | 15.625 | 50.080 | -15.38 % | 1 | sharp | 77.7 % | 64 | -0.1 % | - | ok |

### `cps2`: 384x224 frame, R0, 59.630 Hz

| Display | Rung | Mode | Total | kHz | Hz | Rate | Scale | Filter | Height | Border | Shape | Best whole | Rule |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| generic_15 | native | 384x224p | 263 | 15.682 | 59.630 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 224p | ok |
| arcade_15 | native | 384x224p | 263 | 15.682 | 59.630 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 224p | ok |
| arcade_15ex | native | 384x224p | 263 | 15.682 | 59.630 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 224p | ok |
| h9110 | native | 384x224p | 263 | 15.682 | 59.630 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 224p | ok |
| polo | native | 384x224p | 263 | 15.682 | 59.630 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 224p | ok |
| k7131 | native | 384x224p | 263 | 15.682 | 59.630 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 224p | ok |
| k7000 | native | 384x224p | 263 | 15.682 | 59.630 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 224p | ok |
| ntsc | native | 384x224p | 262 | 15.704 | 59.940 | nearest in range +0.52 % | 1 | nearest | 100.0 % | 0 | +0.0 % | none | ok |
| pal | native | 384x224p | 312 | 15.600 | 50.000 | nearest in range -16.15 % | 1 | nearest | 100.0 % | 0 | +0.0 % | none | ok |
| arcade_15_25 | native | 384x224p | 263 | 15.682 | 59.630 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 224p | ok |
| arcade_15_25_31 | native | 384x224p | 263 | 15.682 | 59.630 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 448p | ok |
| arcade_15_31 | native | 384x224p | 263 | 15.682 | 59.630 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 448p | ok |
| m3129 | native | 384x224p | 256 | 15.265 | 59.630 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 448p | ok |
| d9200 | native | 384x224p | 256 | 15.265 | 59.630 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 448p | ok |
| d9400 | native | 384x224p | 256 | 15.265 | 59.630 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 448p | ok |
| d9800 | native | 384x224p | 256 | 15.265 | 59.630 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 448p | ok |
| ms929 | native | 384x224p | 260 | 15.503 | 59.630 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 224p | ok |
| ms2930 | native | 384x224p | 260 | 15.504 | 59.631 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 448p | ok |
| r666b | native | 384x224p | 260 | 15.504 | 59.631 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 448p | ok |
| pstar | native | 384x224p | 264 | 15.742 | 59.630 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 448p | ok |
| arcade_25 | native | 662x386p | 418 | 24.925 | 59.630 | exact | 1 | nearest | 58.0 % | 162 | +0.0 % | 1x 224p | ok |
| arcade_31 | 2x | 384x448p | 527 | 31.425 | 59.630 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 2x 448p | ok |
| pc_31_120 | 2x | 384x448p | 527 | 31.425 | 59.630 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 2x 448p | ok |
| pc_70_120 | 2x | 384x448p | 504 | 30.053 | 59.630 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 4x 896p | ok |
| m2929 | 2x | 384x448p | 504 | 30.053 | 59.630 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 3x 636p | ok |
| edid_31 | 2x | 384x448p | 520 | 31.008 | 59.631 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 2x 448p | ok |
| edid_30_54 | 2x | 384x448p | 504 | 30.053 | 59.630 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 4x 864p | ok |
| edid_30_70 | 2x | 384x448p | 504 | 30.053 | 59.630 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 4x 896p | ok |
| edid_30_96 | 2x | 384x448p | 504 | 30.053 | 59.630 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 4x 896p | ok |
| panel_720 | panel | 1280x720p | 750 | 45.000 | 60.000 | +0.62 % | 3 | sharp | 93.3 % | 48 | +0.0 % | - | ok |
| panel_768 | panel | 1366x768p | 798 | 47.712 | 59.790 | +0.27 % | 3 | sharp | 87.5 % | 96 | +0.0 % | - | ok |
| panel_1080 | panel | 1920x1080p | 1125 | 67.500 | 60.000 | +0.62 % | 4.82 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| panel_1440 | panel | 2560x1440p | 1481 | 88.786 | 59.951 | +0.54 % | 6.43 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| panel_2160 | panel | 3840x2160p | 2250 | 135.000 | 60.000 | +0.62 % | 9.64 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| tv_ntsc | native | 720x240p | 262 | 15.734 | 60.054 | +0.71 % | 1 | sharp | 93.3 % | 16 | +0.0 % | - | ok |
| tv_pal | native | 720x288p | 312 | 15.625 | 50.080 | -16.02 % | 1 | sharp | 77.7 % | 64 | -0.1 % | - | ok |

### `pwrinst2`: 320x240 frame, R0, 57.550 Hz

| Display | Rung | Mode | Total | kHz | Hz | Rate | Scale | Filter | Height | Border | Shape | Best whole | Rule |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| generic_15 | native | 320x240p | 272 | 15.653 | 57.551 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 240p | ok |
| arcade_15 | native | 320x240p | 272 | 15.653 | 57.551 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 240p | ok |
| arcade_15ex | native | 320x240p | 272 | 15.653 | 57.551 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 240p | ok |
| h9110 | native | 320x240p | 272 | 15.653 | 57.551 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 240p | ok |
| polo | native | 320x240p | 272 | 15.653 | 57.551 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 240p | ok |
| k7131 | native | 320x240p | 272 | 15.653 | 57.551 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 240p | ok |
| k7000 | native | 320x240p | 272 | 15.653 | 57.551 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 240p | ok |
| ntsc | native | 320x240p | 262 | 15.704 | 59.940 | nearest in range +4.15 % | 1 | nearest | 100.0 % | 0 | +0.0 % | none | ok |
| pal | native | 320x240p | 312 | 15.600 | 50.000 | nearest in range -13.12 % | 1 | nearest | 100.0 % | 0 | +0.0 % | none | ok |
| arcade_15_25 | native | 320x240p | 272 | 15.653 | 57.551 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 240p | ok |
| arcade_15_25_31 | native | 320x240p | 272 | 15.653 | 57.551 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 480p | ok |
| arcade_15_31 | native | 320x240p | 272 | 15.653 | 57.551 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 480p | ok |
| m3129 | native | 320x240p | 265 | 15.250 | 57.550 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 480p | ok |
| d9200 | native | 320x240p | 265 | 15.250 | 57.550 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 480p | ok |
| d9400 | native | 320x240p | 265 | 15.250 | 57.550 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 480p | ok |
| d9800 | native | 320x240p | 265 | 15.250 | 57.550 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 480p | ok |
| ms929 | native | 320x240p | 269 | 15.480 | 57.550 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 240p | ok |
| ms2930 | native | 320x240p | 269 | 15.480 | 57.550 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 480p | ok |
| r666b | native | 320x240p | 269 | 15.480 | 57.550 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 480p | ok |
| pstar | native | 320x240p | 273 | 15.711 | 57.550 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 480p | ok |
| arcade_25 | native | 533x400p | 433 | 24.919 | 57.550 | exact | 1 | nearest | 60.0 % | 160 | +0.1 % | 1x 240p | ok |
| arcade_31 | 2x | 320x480p | 546 | 31.422 | 57.550 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 2x 480p | ok |
| pc_31_120 | 2x | 320x480p | 546 | 31.422 | 57.550 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 2x 480p | ok |
| pc_70_120 | 2x | 320x480p | 522 | 30.041 | 57.550 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 4x 960p | ok |
| m2929 | 2x | 320x480p | 522 | 30.041 | 57.550 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 2x 480p | ok |
| edid_31 | 2x | 320x480p | 539 | 31.019 | 57.550 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 2x 480p | ok |
| edid_30_54 | 2x | 320x480p | 522 | 30.041 | 57.551 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 4x 904p | ok |
| edid_30_70 | 2x | 320x480p | 522 | 30.041 | 57.551 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 4x 960p | ok |
| edid_30_96 | 2x | 320x480p | 522 | 30.041 | 57.551 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 4x 960p | ok |
| panel_720 | panel | 1280x720p | 750 | 45.000 | 60.000 | +4.26 % | 3 | nearest | 100.0 % | 0 | +0.0 % | - | ok |
| panel_768 | panel | 1366x768p | 798 | 47.712 | 59.790 | +3.89 % | 3 | nearest | 93.7 % | 48 | +0.0 % | - | ok |
| panel_1080 | panel | 1920x1080p | 1125 | 67.500 | 60.000 | +4.26 % | 4.50 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| panel_1440 | panel | 2560x1440p | 1481 | 88.786 | 59.951 | +4.17 % | 6 | nearest | 100.0 % | 0 | +0.0 % | - | ok |
| panel_2160 | panel | 3840x2160p | 2250 | 135.000 | 60.000 | +4.26 % | 9 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| tv_ntsc | native | 720x240p | 262 | 15.734 | 60.054 | +4.35 % | 1 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| tv_pal | native | 720x288p | 312 | 15.625 | 50.080 | -12.98 % | 1 | sharp | 83.3 % | 48 | +0.1 % | - | ok |

### `dariusg`: 320x232 frame, R0, 58.940 Hz

| Display | Rung | Mode | Total | kHz | Hz | Rate | Scale | Filter | Height | Border | Shape | Best whole | Rule |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| generic_15 | native | 320x232p | 266 | 15.677 | 58.940 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 232p | ok |
| arcade_15 | native | 320x232p | 266 | 15.677 | 58.940 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 232p | ok |
| arcade_15ex | native | 320x232p | 266 | 15.677 | 58.940 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 232p | ok |
| h9110 | native | 320x232p | 266 | 15.677 | 58.940 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 232p | ok |
| polo | native | 320x232p | 266 | 15.677 | 58.940 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 232p | ok |
| k7131 | native | 320x232p | 266 | 15.677 | 58.940 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 232p | ok |
| k7000 | native | 320x232p | 266 | 15.677 | 58.940 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 232p | ok |
| ntsc | native | 320x232p | 262 | 15.704 | 59.940 | nearest in range +1.70 % | 1 | nearest | 100.0 % | 0 | +0.0 % | none | ok |
| pal | native | 320x232p | 312 | 15.600 | 50.000 | nearest in range -15.17 % | 1 | nearest | 100.0 % | 0 | +0.0 % | none | ok |
| arcade_15_25 | native | 320x232p | 266 | 15.677 | 58.940 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 232p | ok |
| arcade_15_25_31 | native | 320x232p | 266 | 15.677 | 58.940 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 464p | ok |
| arcade_15_31 | native | 320x232p | 266 | 15.677 | 58.940 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 464p | ok |
| m3129 | native | 320x232p | 259 | 15.265 | 58.940 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 464p | ok |
| d9200 | native | 320x232p | 259 | 15.265 | 58.940 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 464p | ok |
| d9400 | native | 320x232p | 259 | 15.265 | 58.940 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 464p | ok |
| d9800 | native | 320x232p | 259 | 15.265 | 58.940 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 464p | ok |
| ms929 | native | 320x232p | 263 | 15.501 | 58.940 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 232p | ok |
| ms2930 | native | 320x232p | 263 | 15.501 | 58.940 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 464p | ok |
| r666b | native | 320x232p | 263 | 15.501 | 58.940 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 464p | ok |
| pstar | native | 320x232p | 267 | 15.736 | 58.940 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 464p | ok |
| arcade_25 | native | 539x391p | 423 | 24.931 | 58.940 | exact | 1 | nearest | 59.3 % | 159 | +0.1 % | 1x 232p | ok |
| arcade_31 | 2x | 320x464p | 533 | 31.414 | 58.940 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 2x 464p | ok |
| pc_31_120 | 2x | 320x464p | 533 | 31.415 | 58.940 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 2x 464p | ok |
| pc_70_120 | 2x | 320x464p | 509 | 30.000 | 58.939 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 4x 928p | ok |
| m2929 | 2x | 320x464p | 509 | 30.000 | 58.939 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 2x 464p | ok |
| edid_31 | 2x | 320x464p | 526 | 31.002 | 58.940 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 2x 464p | ok |
| edid_30_54 | 2x | 320x464p | 509 | 30.000 | 58.939 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 4x 880p | ok |
| edid_30_70 | 2x | 320x464p | 509 | 30.000 | 58.939 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 4x 928p | ok |
| edid_30_96 | 2x | 320x464p | 509 | 30.000 | 58.939 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 4x 928p | ok |
| panel_720 | panel | 1280x720p | 750 | 45.000 | 60.000 | +1.80 % | 3 | sharp | 96.6 % | 24 | +0.0 % | - | ok |
| panel_768 | panel | 1366x768p | 798 | 47.712 | 59.790 | +1.44 % | 3 | sharp | 90.6 % | 72 | +0.0 % | - | ok |
| panel_1080 | panel | 1920x1080p | 1125 | 67.500 | 60.000 | +1.80 % | 4.66 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| panel_1440 | panel | 2560x1440p | 1481 | 88.786 | 59.951 | +1.72 % | 6.21 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| panel_2160 | panel | 3840x2160p | 2250 | 135.000 | 60.000 | +1.80 % | 9.31 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| tv_ntsc | native | 720x240p | 262 | 15.734 | 60.054 | +1.89 % | 1 | sharp | 96.6 % | 8 | -0.1 % | - | ok |
| tv_pal | native | 720x288p | 312 | 15.625 | 50.080 | -15.03 % | 1 | sharp | 80.5 % | 56 | +0.0 % | - | ok |

### `rtype`: 384x256 frame, R0, 55.000 Hz

| Display | Rung | Mode | Total | kHz | Hz | Rate | Scale | Filter | Height | Border | Shape | Best whole | Rule |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| generic_15 | native | 384x256p | 285 | 15.674 | 54.999 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 256p | ok |
| arcade_15 | native | 384x256p | 285 | 15.674 | 54.999 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 256p | ok |
| arcade_15ex | native | 384x256p | 285 | 15.674 | 54.999 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 256p | ok |
| h9110 | native | 384x256p | 285 | 15.674 | 54.999 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 256p | ok |
| polo | native | 384x256p | 285 | 15.674 | 54.999 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 256p | ok |
| k7131 | native | 384x256p | 285 | 15.674 | 54.999 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 256p | ok |
| k7000 | native | 384x256p | 285 | 15.674 | 54.999 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 256p | ok |
| ntsc | native | 384x240p | 262 | 15.704 | 59.940 | nearest in range +8.98 % | 1 | nearest | 100.0 % | 0 | -6.2 % | none | ok: overscan crop |
| pal | native | 384x256p | 312 | 15.600 | 50.000 | nearest in range -9.09 % | 1 | nearest | 100.0 % | 0 | +0.0 % | none | ok |
| arcade_15_25 | native | 384x256p | 285 | 15.674 | 54.999 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 256p | ok |
| arcade_15_25_31 | native | 384x256p | 285 | 15.674 | 54.999 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 512p | ok |
| arcade_15_31 | native | 384x256p | 285 | 15.674 | 54.999 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 512p | ok |
| m3129 | native | 384x256p | 278 | 15.290 | 55.000 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 512p | ok |
| d9200 | native | 384x256p | 278 | 15.290 | 55.000 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 512p | ok |
| d9400 | native | 384x256p | 278 | 15.290 | 55.000 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 512p | ok |
| d9800 | native | 384x256p | 278 | 15.290 | 55.000 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 512p | ok |
| ms929 | native | 384x256p | 281 | 15.455 | 55.000 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 256p | ok |
| ms2930 | native | 384x256p | 281 | 15.455 | 55.000 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 512p | ok |
| r666b | native | 384x256p | 281 | 15.455 | 55.000 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 512p | ok |
| pstar | native | 384x256p | 286 | 15.730 | 55.000 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 512p | ok |
| arcade_25 | native | 600x400p | 453 | 24.914 | 55.000 | exact | 1 | nearest | 64.0 % | 144 | +0.0 % | 1x 256p | ok |
| arcade_31 | 2x | 384x512p | 571 | 31.404 | 55.000 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 2x 512p | ok |
| pc_31_120 | 2x | 384x512p | 571 | 31.404 | 55.000 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 2x 512p | ok |
| pc_70_120 | 2x | 384x512p | 546 | 30.030 | 55.000 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 4x 1024p | ok |
| m2929 | 2x | 384x512p | 546 | 30.030 | 55.000 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 2x 512p | ok |
| edid_31 | 2x | 384x512p | 564 | 31.020 | 55.001 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 2x 512p | ok |
| edid_30_54 | 2x | 384x512p | 546 | 30.030 | 55.001 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 3x 768p | ok |
| edid_30_70 | 2x | 384x512p | 546 | 30.030 | 55.001 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 4x 1024p | ok |
| edid_30_96 | 2x | 384x512p | 546 | 30.030 | 55.001 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 4x 1024p | ok |
| panel_720 | panel | 1280x720p | 750 | 45.000 | 60.000 | +9.09 % | 2 | nearest | 71.1 % | 208 | -0.1 % | - | ok |
| panel_768 | panel | 1366x768p | 798 | 47.712 | 59.790 | +8.71 % | 3 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| panel_1080 | panel | 1920x1080p | 1125 | 67.500 | 60.000 | +9.09 % | 4.22 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| panel_1440 | panel | 2560x1440p | 1481 | 88.786 | 59.951 | +9.00 % | 5.62 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| panel_2160 | panel | 3840x2160p | 2250 | 135.000 | 60.000 | +9.09 % | 8.44 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| tv_ntsc | native | 720x240p | 262 | 15.734 | 60.054 | +9.19 % | 1 | sharp | 100.0 % | 0 | -6.2 % | - | ok: overscan crop |
| tv_pal | native | 720x288p | 312 | 15.625 | 50.080 | -8.95 % | 1 | sharp | 88.8 % | 32 | +0.1 % | - | ok |

### `mk`: 400x256 frame, R0, 54.700 Hz

| Display | Rung | Mode | Total | kHz | Hz | Rate | Scale | Filter | Height | Border | Shape | Best whole | Rule |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| generic_15 | native | 400x256p | 286 | 15.644 | 54.700 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 256p | ok |
| arcade_15 | native | 400x256p | 286 | 15.644 | 54.700 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 256p | ok |
| arcade_15ex | native | 400x256p | 286 | 15.644 | 54.700 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 256p | ok |
| h9110 | native | 400x256p | 286 | 15.644 | 54.700 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 256p | ok |
| polo | native | 400x256p | 286 | 15.644 | 54.700 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 256p | ok |
| k7131 | native | 400x256p | 286 | 15.644 | 54.700 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 256p | ok |
| k7000 | native | 400x256p | 286 | 15.644 | 54.700 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 256p | ok |
| ntsc | native | 400x240p | 262 | 15.704 | 59.940 | nearest in range +9.58 % | 1 | nearest | 100.0 % | 0 | -6.2 % | none | ok: overscan crop |
| pal | native | 400x256p | 312 | 15.600 | 50.000 | nearest in range -8.59 % | 1 | nearest | 100.0 % | 0 | +0.0 % | none | ok |
| arcade_15_25 | native | 400x256p | 286 | 15.644 | 54.700 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 256p | ok |
| arcade_15_25_31 | native | 400x256p | 286 | 15.644 | 54.700 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 512p | ok |
| arcade_15_31 | native | 400x256p | 286 | 15.644 | 54.700 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 512p | ok |
| m3129 | native | 400x256p | 279 | 15.261 | 54.700 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 512p | ok |
| d9200 | native | 400x256p | 279 | 15.261 | 54.700 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 512p | ok |
| d9400 | native | 400x256p | 279 | 15.261 | 54.700 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 512p | ok |
| d9800 | native | 400x256p | 279 | 15.261 | 54.700 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 512p | ok |
| ms929 | native | 400x256p | 283 | 15.480 | 54.700 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 256p | ok |
| ms2930 | native | 400x256p | 283 | 15.480 | 54.701 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 512p | ok |
| r666b | native | 400x256p | 283 | 15.480 | 54.701 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 512p | ok |
| pstar | native | 400x256p | 288 | 15.753 | 54.700 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 512p | ok |
| arcade_25 | native | 625x400p | 456 | 24.943 | 54.700 | exact | 1 | nearest | 64.0 % | 144 | +0.0 % | 1x 256p | ok |
| arcade_31 | 2x | 400x512p | 575 | 31.452 | 54.700 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 2x 512p | ok |
| pc_31_120 | 2x | 400x512p | 575 | 31.452 | 54.700 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 2x 512p | ok |
| pc_70_120 | 2x | 400x512p | 549 | 30.029 | 54.699 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 4x 1024p | ok |
| m2929 | 2x | 400x512p | 549 | 30.030 | 54.700 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 2x 512p | ok |
| edid_31 | 2x | 400x512p | 567 | 31.014 | 54.699 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 2x 512p | ok |
| edid_30_54 | 2x | 400x512p | 549 | 30.030 | 54.700 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 3x 768p | ok |
| edid_30_70 | 2x | 400x512p | 549 | 30.030 | 54.700 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 4x 1024p | ok |
| edid_30_96 | 2x | 400x512p | 549 | 30.030 | 54.700 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 4x 1024p | ok |
| panel_720 | panel | 1280x720p | 750 | 45.000 | 60.000 | +9.69 % | 2 | nearest | 71.1 % | 208 | -0.1 % | - | ok |
| panel_768 | panel | 1366x768p | 798 | 47.712 | 59.790 | +9.31 % | 3 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| panel_1080 | panel | 1920x1080p | 1125 | 56.250 | 50.000 | -8.59 % | 4.22 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| panel_1440 | panel | 2560x1440p | 1481 | 88.786 | 59.951 | +9.60 % | 5.62 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| panel_2160 | panel | 3840x2160p | 2250 | 135.000 | 60.000 | +9.69 % | 8.44 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| tv_ntsc | native | 720x240p | 262 | 15.734 | 60.054 | +9.79 % | 1 | sharp | 100.0 % | 0 | -6.2 % | - | ok: overscan crop |
| tv_pal | native | 720x288p | 312 | 15.625 | 50.080 | -8.45 % | 1 | sharp | 88.8 % | 32 | +0.1 % | - | ok |

### `sfiii3ws`: 496x224 frame, R0, 59.590 Hz

| Display | Rung | Mode | Total | kHz | Hz | Rate | Scale | Filter | Height | Border | Shape | Best whole | Rule |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| generic_15 | native | 496x224p | 263 | 15.672 | 59.591 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 224p | ok |
| arcade_15 | native | 496x224p | 263 | 15.672 | 59.591 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 224p | ok |
| arcade_15ex | native | 496x224p | 263 | 15.672 | 59.591 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 224p | ok |
| h9110 | native | 496x224p | 263 | 15.672 | 59.591 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 224p | ok |
| polo | native | 496x224p | 263 | 15.672 | 59.591 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 224p | ok |
| k7131 | native | 496x224p | 263 | 15.672 | 59.591 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 224p | ok |
| k7000 | native | 496x224p | 263 | 15.672 | 59.591 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 224p | ok |
| ntsc | native | 496x224p | 262 | 15.704 | 59.940 | nearest in range +0.59 % | 1 | nearest | 100.0 % | 0 | +0.0 % | none | ok |
| pal | native | 496x224p | 312 | 15.600 | 50.000 | nearest in range -16.09 % | 1 | nearest | 100.0 % | 0 | +0.0 % | none | ok |
| arcade_15_25 | native | 496x224p | 263 | 15.672 | 59.591 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 224p | ok |
| arcade_15_25_31 | native | 496x224p | 263 | 15.672 | 59.591 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 448p | ok |
| arcade_15_31 | native | 496x224p | 263 | 15.672 | 59.591 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 448p | ok |
| m3129 | native | 496x224p | 256 | 15.255 | 59.590 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 448p | ok |
| d9200 | native | 496x224p | 256 | 15.255 | 59.590 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 448p | ok |
| d9400 | native | 496x224p | 256 | 15.255 | 59.590 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 448p | ok |
| d9800 | native | 496x224p | 256 | 15.255 | 59.590 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 448p | ok |
| ms929 | native | 496x224p | 260 | 15.493 | 59.589 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 224p | ok |
| ms2930 | native | 496x224p | 260 | 15.493 | 59.591 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 448p | ok |
| r666b | native | 496x224p | 260 | 15.493 | 59.591 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 448p | ok |
| pstar | native | 496x224p | 264 | 15.731 | 59.590 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 448p | ok |
| arcade_25 | native | 855x386p | 418 | 24.908 | 59.590 | exact | 1 | nearest | 58.0 % | 162 | +0.0 % | 1x 224p | ok |
| arcade_31 | 2x | 496x448p | 527 | 31.404 | 59.590 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 2x 448p | ok |
| pc_31_120 | 2x | 496x448p | 527 | 31.403 | 59.590 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 2x 448p | ok |
| pc_70_120 | 2x | 496x448p | 504 | 30.033 | 59.590 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 4x 896p | ok |
| m2929 | 2x | 496x448p | 504 | 30.033 | 59.590 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 3x 636p | ok |
| edid_31 | 2x | 496x448p | 521 | 31.046 | 59.590 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 2x 448p | ok |
| edid_30_54 | 2x | 496x448p | 504 | 30.033 | 59.590 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 4x 872p | ok |
| edid_30_70 | 2x | 496x448p | 504 | 30.033 | 59.590 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 4x 896p | ok |
| edid_30_96 | 2x | 496x448p | 504 | 30.033 | 59.590 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 4x 896p | ok |
| panel_720 | panel | 1280x720p | 750 | 45.000 | 60.000 | +0.69 % | 3 | nearest | 93.3 % | 48 | +0.0 % | - | ok |
| panel_768 | panel | 1366x768p | 798 | 47.712 | 59.790 | +0.34 % | 3 | nearest | 87.5 % | 96 | +0.0 % | - | ok |
| panel_1080 | panel | 1920x1080p | 1125 | 67.500 | 60.000 | +0.69 % | 4.82 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| panel_1440 | panel | 2560x1440p | 1481 | 88.786 | 59.951 | +0.61 % | 6.43 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| panel_2160 | panel | 3840x2160p | 2250 | 135.000 | 60.000 | +0.69 % | 9.64 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| tv_ntsc | native | 720x240p | 262 | 15.734 | 60.054 | +0.78 % | 1 | sharp | 93.3 % | 16 | +0.0 % | - | ok |
| tv_pal | native | 720x288p | 312 | 15.625 | 50.080 | -15.96 % | 1 | sharp | 77.7 % | 64 | -0.1 % | - | ok |

### `bnzabros`: 496x384 frame, R0, 57.524 Hz

| Display | Rung | Mode | Total | kHz | Hz | Rate | Scale | Filter | Height | Border | Shape | Best whole | Rule |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| generic_15 | interlaced 1:1 | 579x448i | 545 | 15.675 | 57.524 | exact | 1 | nearest | 85.7 % | 64 | +0.0 % | 1x 384i | ok |
| arcade_15 | interlaced 1:1 | 579x448i | 545 | 15.675 | 57.524 | exact | 1 | nearest | 85.7 % | 64 | +0.0 % | 1x 384i | ok |
| arcade_15ex | interlaced 1:1 | 579x448i | 545 | 15.675 | 57.524 | exact | 1 | nearest | 85.7 % | 64 | +0.0 % | 1x 384i | ok |
| h9110 | interlaced 1:1 | 579x448i | 545 | 15.675 | 57.524 | exact | 1 | nearest | 85.7 % | 64 | +0.0 % | 1x 384i | ok |
| polo | interlaced 1:1 | 579x448i | 545 | 15.675 | 57.524 | exact | 1 | nearest | 85.7 % | 64 | +0.0 % | 1x 384i | ok |
| k7131 | interlaced 1:1 | 579x448i | 545 | 15.675 | 57.524 | exact | 1 | nearest | 85.7 % | 64 | +0.0 % | 1x 384i | ok |
| k7000 | interlaced 1:1 | 579x448i | 545 | 15.675 | 57.524 | exact | 1 | nearest | 85.7 % | 64 | +0.0 % | 1x 384i | ok |
| ntsc | interlaced 1:1 | 579x448i | 525 | 15.734 | 59.940 | nearest in range +4.20 % | 1 | nearest | 85.7 % | 64 | +0.0 % | none | ok |
| pal | interlaced 1:1 | 579x448i | 625 | 15.625 | 50.000 | nearest in range -13.08 % | 1 | nearest | 85.7 % | 64 | +0.0 % | none | ok |
| arcade_15_25 | native | 496x384p | 433 | 24.907 | 57.524 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 384p | ok |
| arcade_15_25_31 | native | 496x384p | 433 | 24.907 | 57.524 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 384p | ok |
| arcade_15_31 | native | 647x501p | 546 | 31.408 | 57.524 | exact | 1 | nearest | 76.6 % | 117 | +0.0 % | 1x 384p | ok |
| m3129 | native | 638x494p | 539 | 31.005 | 57.523 | exact | 1 | nearest | 77.7 % | 110 | +0.0 % | 1x 384p | ok |
| d9200 | native | 638x494p | 539 | 31.005 | 57.523 | exact | 1 | nearest | 77.7 % | 110 | +0.0 % | 1x 384p | ok |
| d9400 | native | 496x384p | 425 | 24.447 | 57.524 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 384p | ok |
| d9800 | native | 496x384p | 425 | 24.447 | 57.524 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 384p | ok |
| ms929 | native | 496x384p | 425 | 24.447 | 57.524 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 384p | ok |
| ms2930 | native | 496x384p | 427 | 24.562 | 57.524 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 384p | ok |
| r666b | native | 496x384p | 427 | 24.562 | 57.524 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 384p | ok |
| pstar | native | 496x384p | 440 | 25.310 | 57.524 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 384p | ok |
| arcade_25 | native | 496x384p | 433 | 24.907 | 57.524 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 384p | ok |
| arcade_31 | native | 647x501p | 546 | 31.408 | 57.524 | exact | 1 | nearest | 76.6 % | 117 | +0.0 % | 1x 384p | ok |
| pc_31_120 | native | 661x512p | 546 | 31.408 | 57.524 | exact | 1 | nearest | 75.0 % | 128 | +0.1 % | 1x 384p | ok |
| pc_70_120 | native | 646x500p | 522 | 30.027 | 57.524 | exact | 1 | nearest | 76.8 % | 116 | +0.0 % | 2x 768p | ok |
| m2929 | native | 496x384p | 522 | 30.028 | 57.525 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 384p | ok |
| edid_31 | native | 672x520p | 539 | 31.005 | 57.525 | exact | 1 | nearest | 73.8 % | 136 | +0.0 % | 1x 384p | ok |
| edid_30_54 | native | 496x384p | 522 | 30.027 | 57.524 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 768p | ok |
| edid_30_70 | native | 496x384p | 522 | 30.027 | 57.524 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 3x 1152p | ok |
| edid_30_96 | native | 496x384p | 522 | 30.027 | 57.524 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 4x 1536p | ok |
| panel_720 | panel | 1280x720p | 750 | 45.000 | 60.000 | +4.30 % | 1 | nearest | 53.3 % | 336 | +0.0 % | - | ok |
| panel_768 | panel | 1366x768p | 798 | 47.712 | 59.790 | +3.94 % | 2 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| panel_1080 | panel | 1920x1080p | 1125 | 67.500 | 60.000 | +4.30 % | 2 | sharp | 71.1 % | 312 | +0.0 % | - | ok |
| panel_1440 | panel | 2560x1440p | 1481 | 88.786 | 59.951 | +4.22 % | 3 | sharp | 80.0 % | 288 | +0.0 % | - | ok |
| panel_2160 | panel | 3840x2160p | 2250 | 135.000 | 60.000 | +4.30 % | 5.62 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| tv_ntsc | interlaced 1:1 | 720x480i | 525 | 15.734 | 59.940 | +4.20 % | 1 | sharp | 80.0 % | 96 | +0.0 % | - | ok |
| tv_pal | interlaced 1:1 | 720x576i | 625 | 15.625 | 50.000 | -13.08 % | 1 | sharp | 66.6 % | 192 | +0.0 % | - | ok |

### `littlerb`: 338x288 frame, R0, 30.000 Hz

| Display | Rung | Mode | Total | kHz | Hz | Rate | Scale | Filter | Height | Border | Shape | Best whole | Rule |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| generic_15 | interlaced 1:1 | 526x448i | 523 | 15.690 | 60.000 | exact x2 | 1 | nearest | 64.2 % | 160 | +0.0 % | 1x 288i | ok |
| arcade_15 | interlaced 1:1 | 526x448i | 523 | 15.690 | 60.000 | exact x2 | 1 | nearest | 64.2 % | 160 | +0.0 % | 1x 288i | ok |
| arcade_15ex | interlaced 1:1 | 526x448i | 523 | 15.690 | 60.000 | exact x2 | 1 | nearest | 64.2 % | 160 | +0.0 % | 1x 288i | ok |
| h9110 | interlaced 1:1 | 526x448i | 523 | 15.690 | 60.000 | exact x2 | 1 | nearest | 64.2 % | 160 | +0.0 % | 1x 288i | ok |
| polo | interlaced 1:1 | 526x448i | 523 | 15.690 | 60.000 | exact x2 | 1 | nearest | 64.2 % | 160 | +0.0 % | 1x 288i | ok |
| k7131 | interlaced 1:1 | 526x448i | 523 | 15.690 | 60.000 | exact x2 | 1 | nearest | 64.2 % | 160 | +0.0 % | 1x 288i | ok |
| k7000 | interlaced 1:1 | 526x448i | 523 | 15.690 | 60.000 | exact x2 | 1 | nearest | 64.2 % | 160 | +0.0 % | 1x 288i | ok |
| ntsc | interlaced 1:1 | 526x448i | 525 | 15.734 | 59.940 | nearest in range -0.10 % | 1 | nearest | 64.2 % | 160 | +0.0 % | none | ok |
| pal | native | 338x288p | 312 | 15.600 | 50.000 | nearest in range -16.67 % | 1 | nearest | 100.0 % | 0 | +0.0 % | none | ok |
| arcade_15_25 | native | 451x384p | 416 | 24.959 | 59.999 | exact x2 | 1 | nearest | 75.0 % | 96 | +0.0 % | 1x 288p | ok |
| arcade_15_25_31 | native | 451x384p | 416 | 24.959 | 59.999 | exact x2 | 1 | nearest | 75.0 % | 96 | +0.0 % | 1x 288p | ok |
| arcade_15_31 | native | 562x479p | 524 | 31.440 | 60.000 | exact x2 | 1 | nearest | 60.1 % | 191 | +0.1 % | 1x 288p | ok |
| m3129 | native | 554x472p | 517 | 31.020 | 60.000 | exact x2 | 1 | nearest | 61.0 % | 184 | +0.0 % | 1x 288p | ok |
| d9200 | 2x | 338x576p | 617 | 37.020 | 60.000 | exact x2 | 2 | nearest | 100.0 % | 0 | +0.0 % | 2x 576p | ok |
| d9400 | native | 338x288p | 312 | 18.719 | 60.000 | exact x2 | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 576p | ok |
| d9800 | native | 338x288p | 312 | 18.719 | 60.000 | exact x2 | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 576p | ok |
| ms929 | interlaced 1:1 | 526x448i | 517 | 15.510 | 60.000 | exact x2 | 1 | nearest | 64.2 % | 160 | +0.0 % | 1x 288i | ok |
| ms2930 | native | 563x480p | 528 | 31.679 | 60.000 | exact x2 | 1 | nearest | 60.0 % | 192 | +0.1 % | 1x 288p | ok |
| r666b | native | 552x470p | 517 | 31.019 | 59.999 | exact x2 | 1 | nearest | 61.2 % | 182 | +0.0 % | 1x 288p | ok |
| pstar | native | 470x400p | 422 | 25.320 | 60.000 | exact x2 | 1 | nearest | 72.0 % | 112 | -0.1 % | 1x 288p | ok |
| arcade_25 | native | 451x384p | 416 | 24.959 | 59.999 | exact x2 | 1 | nearest | 75.0 % | 96 | +0.0 % | 1x 288p | ok |
| arcade_31 | native | 562x479p | 524 | 31.440 | 60.000 | exact x2 | 1 | nearest | 60.1 % | 191 | +0.1 % | 1x 288p | ok |
| pc_31_120 | native | 594x506p | 524 | 31.440 | 60.000 | exact x2 | 1 | nearest | 56.9 % | 218 | +0.0 % | 1x 288p | ok |
| pc_70_120 | 2x | 338x576p | 602 | 36.120 | 60.000 | exact x2 | 2 | nearest | 100.0 % | 0 | +0.0 % | 3x 864p | ok |
| m2929 | 2x | 338x576p | 597 | 35.820 | 60.000 | exact x2 | 2 | nearest | 100.0 % | 0 | +0.0 % | 2x 576p | ok |
| edid_31 | native | 585x498p | 517 | 31.020 | 60.000 | exact x2 | 1 | nearest | 57.8 % | 210 | -0.1 % | 1x 288p | ok |
| edid_30_54 | 2x | 338x576p | 599 | 35.939 | 60.000 | exact x2 | 2 | nearest | 100.0 % | 0 | +0.0 % | 3x 864p | ok |
| edid_30_70 | 2x | 338x576p | 599 | 35.939 | 60.000 | exact x2 | 2 | nearest | 100.0 % | 0 | +0.0 % | 4x 1120p | ok |
| edid_30_96 | 2x | 338x576p | 599 | 35.939 | 60.000 | exact x2 | 2 | nearest | 100.0 % | 0 | +0.0 % | 4x 1152p | ok |
| panel_720 | panel | 1280x720p | 750 | 45.000 | 60.000 | +0.00 % | 2 | sharp | 80.0 % | 144 | +0.0 % | - | ok |
| panel_768 | panel | 1366x768p | 798 | 47.712 | 59.790 | -0.35 % | 2 | sharp | 75.0 % | 192 | +0.0 % | - | ok |
| panel_1080 | panel | 1920x1080p | 1125 | 67.500 | 60.000 | +0.00 % | 3 | sharp | 80.0 % | 216 | +0.0 % | - | ok |
| panel_1440 | panel | 2560x1440p | 1481 | 88.786 | 59.951 | -0.08 % | 5 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| panel_2160 | panel | 3840x2160p | 2250 | 135.000 | 60.000 | +0.00 % | 7.50 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| tv_ntsc | interlaced 1:1 | 720x480i | 525 | 15.734 | 59.940 | -0.10 % | 1 | sharp | 60.0 % | 192 | -0.1 % | - | ok |
| tv_pal | native | 720x288p | 312 | 15.625 | 50.080 | -16.53 % | 1 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
