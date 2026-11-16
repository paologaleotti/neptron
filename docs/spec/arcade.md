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
4. Each ROM of the set, except `nodump` ROMs and ROMs that are also ROMs of its BIOS set (the same CRC), must be in one of the found zips of the set, its parent or its BIOS, with the same CRC-32 and the same size. The match is by CRC and size, not by name, as FBNeo matches. This one rule covers split, non-merged and full non-merged sets. Otherwise: message 4.
5. The BIOS: if no found zip of the BIOS name holds any ROM of the BIOS set, message 5. If some but not all match, the launch goes on and the diagnostics page lists the differences. FBNeo does not require a ROM that its driver marks `BRF_OPT`, and a Neo Geo BIOS set lists many BIOS variants, of which the core needs only the selected one. The DAT has no such mark, so the check cannot tell which BIOS files are needed. A stricter rule is an open point (section 19, A3). The same mark can make a game ROM optional; the check then asks for a ROM that the core would not need. That is safe (it stops, never passes wrongly), and the same open point covers it.
6. A zip that cannot be read (damaged, ZIP64, multi-disk, too large a central directory): message 6.

With no index (section 6), the check gives message 7 and lets the user start the game.

### The messages

Each message shows in a dialog in the menu, and the full list goes to the diagnostics page. `<set>` is the zip stem, `<n>` a count.

| # | Text | Launch |
| --- | --- | --- |
| 1 | `<set>.zip` is not a set name of FinalBurn Neo. Check the file name. | Stopped |
| 2 | This game needs the BIOS file `<bios>.zip`. Put it in `bios`, or in the folder of the game. | Stopped |
| 3 | This game needs the file `<parent>.zip` in the same folder. | Stopped |
| 4 | `<n>` files of this game are missing or differ. The set may be for another version of FinalBurn Neo, or a file may be damaged. The diagnostics page lists them. | Stopped |
| 5 | The BIOS file `<bios>.zip` does not match this version of FinalBurn Neo. | Stopped |
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
- The policy places the upright picture as it does today: it swaps the axes and treats the long side as lines.
- `Scene` carries the rotation, and both renderers turn the game picture (ADR 0015). The UI canvas is never turned.
- `data/cores/fbneo.toml` pins `fbneo-vertical-mode = "disabled"`, the core's default, so that the turns and the aspect above do not change if the core's default changes.

### The setting for a 15 kHz screen

A vertical game has 256 to 320 lines on its long side. A horizontal 15 kHz CRT holds about 240 progressive lines. The Definition makes an interlaced mode with the game's lines 1:1 the default.

| Key | `display.vertical` |
| --- | --- |
| Label | Vertical games |
| Choices, in menu order | `interlaced` (default): lines 1:1 in an interlaced mode, every pixel exact, with interlace flicker. `fitted`: the picture scaled into the progressive mode, soft, with no flicker |
| Levels | game, system, global (`Levels::ALL`) |
| Where the row shows | Settings > Display, next to the picture style, which writes at the global level. And the Video page of the quick menu while a game runs, which writes at the game level; its keep prompt then moves the change to the system or the global level, as for the picture style |
| Applies to | `Crt` (a CRT and a TV) |
| Effect | Only for a turned source (`R90`, `R270`) on a screen of the 15 kHz class. On 31 kHz, on a flat panel and for a horizontal game it changes nothing |
| Safety | `Validated`, as the picture style |
| Help text | "How a vertical game shows on a horizontal 15 kHz screen or TV. Interlaced shows every pixel exact, with flicker. Fitted shows a soft picture with no flicker." |

- The choices are append-only. The Definition also names a fitted interlaced view and a cropped 1:1 view; they are later choices at the end of the list.
- With `fitted`, the policy drops the interlaced 1:1 rung for a turned source (a new `Dropped` reason, `VerticalFitted`), on the CRT ladder and on the TV ladder. The fitted rung then wins and carries its `fitted` notice.
- The crawler finds the setting, as every setting.

### The notice the first time

The first time a vertical game gets the interlaced rung on a 15 kHz CRT or a TV, the frontend shows this notice once:

> This is a vertical game on a horizontal screen. It shows interlaced, with every pixel exact. Settings > Display > Vertical games, or Video in the quick menu, can show it as a soft picture with no flicker. A screen turned on its side (TATE) comes in a later version.

The frontend keeps the notices already shown in `state/config/notices.toml`. The writer thread of `apps/neptron/src/outbox.rs`, which does every file write of the frame loop, writes it as temp file, fsync, rename; the frame thread only sends the job. The menu process reads the file at boot with the settings. A deleted file shows the notice again and loses nothing. (A store for one-time notices does not exist yet; open point A4.)

### Expected results for DoDonPachi (240x320 upright, 57.55 Hz)

| Display | Expected result |
| --- | --- |
| 15 kHz PC CRT | Rung 3: 480i at 57.55 Hz, 320 lines 1:1, centred, pillarboxed. With `fitted`: 240p, the picture fitted, sharp bilinear |
| 31 kHz PC CRT | 320 lines 1:1 at 57.55 Hz, centred in a taller mode, with the `padded` notice |
| Flat panel, 1080p | Upright, factor 3: 720x960, nearest, pillarboxed. The game runs at the panel rate |
| Flat panel, 720p | Upright, factor 2: 480x640, nearest |
| TV, NTSC (Pi composite) | 480i, 320 lines 1:1, about 264 of the 704 samples wide (the `aspect` width rule), sharp bilinear. 59.940 Hz: +4.15 % speed |
| TV, PAL | 576i, 320 lines 1:1, about 220 samples wide. 50.000 Hz: -13.1 % speed |

These follow from the present rules; the goldens of M08b check them (section 16). The 480i field order on the Pi `vc4` is not checked yet (OP-103 point 5).

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
| Midway Y/T-unit | about 54.7 Hz | UNVERIFIED |

### Expected result per display class

- **PC CRT, 15 kHz and 31 kHz:** Switchres solves each rate. The mode matches within `REFRESH_MATCH_MHZ` (5 mHz), with a pixel clock in whole kHz. The game runs at its own speed. No alert.
- **Flat panel:** the game runs at the panel's flip rate. The status shows the speed beyond `PANEL_SPEED_SILENT_PPM` (5,000 ppm).
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
- A horizontal game on a vertical screen: turn 90 or 270. The policy swaps the axes and the game gets the fitted or interlaced treatment that a vertical game gets today on a horizontal screen. `display.vertical` then applies to horizontal games: its meaning is "a game whose long side is across the tube's lines".
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
| M08b, rate goldens | Policy goldens for real boards: Neo Geo, CPS, Cave (horizontal, and vertical with `R90`, `R180` and `R270`), Taito F3, Irem M72, Psikyo, on every CRT profile, on a 1080p and a 720p panel, and on TV NTSC and PAL. The sizes come from the DAT of the pin (facts, not an excerpt). The expected results are those of sections 11 and 12. No new rule is expected; a golden that shows a wrong result stops the task and is reported | `crates/neptron-modeline/tests/golden`, `crates/neptron-modeline/tests/integration/policy_golden.rs` | M06 |
| M08a, rotation | ADR 0015: `Scene::rotation`, both renderers, every `Scene` caller, the testkit hash, `gles-compare` scenes with `R90`, `R180` and `R270`. The session: the source rotation, the aspect inversion (section 11), `always_progressive`. The setting `display.vertical` and its policy effect. The notice and its store | `crates/neptron-types`, `crates/neptron-gfx`, `crates/neptron-testkit`, `crates/neptron-video`, `crates/neptron-session`, `crates/neptron-modeline/src`, `crates/neptron-settings`, `crates/neptron-ui`, `crates/neptron-kms/src/kms/mode.rs`, `crates/neptron-kms/examples`, `crates/neptron-window/examples`, `apps/neptron`, `apps/neptronctl`, `docs/adr` | M07a, M07b, M07c, M08b |

- Build order: M07a; then M07b; then M07c and M08b; then M08a. M08b needs only M06 and can start at once.
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
| 480i on Intel and on the Pi `vc4` not proven (OP-103 point 5) | The bench session checks it. `display.vertical = fitted` is the way out |
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
