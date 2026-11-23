# Arcade

This spec defines arcade games in NEPTRON 0.1: the core, the folders, the game list, the check of a ROM set before launch, the core host, vertical games, the rates and the controls.

The owner decided the main points on 2026-11-16. A statement about FinalBurn Neo (FBNeo) comes from a reading of its source at `libretro/FBNeo` `d7450cf1` (2026-11-15, the pin), of the RetroPie and Batocera sources and of the libretro documentation, unless it carries a mark:

- **UNVERIFIED**: nobody has checked it. A task must not rely on it without its own check.
- **trace**: a core trace confirms it.

No diagnostics page exists yet (OP-108). A line that this spec gives to the diagnostics page is planned. Until then, the set check writes its full list to the log.

## 1. Scope

### In the MVP

- One arcade core: FBNeo, the full build, on both images (PC x86_64 and Raspberry Pi aarch64). ADR 0014 records the core and its licence.
- Two system folders, `arcade` and `neogeo`. Both run FBNeo.
- ROM sets as `.zip` files only.
- A hard disk image that a set needs (CHD, or a raw dump) is passed through: the core opens it itself from the folder of the set. NEPTRON checks only that the file is present (section 10). No board with a disk image is a tested board family (Killer Instinct is the only one in the pinned DAT).
- A game list from an index that NEPTRON makes from the DAT of the pinned FBNeo commit: titles, BIOS sets hidden, clones folded under their parent, a mark for vertical games.
- A check of the set and its BIOS before launch, with NEPTRON's own messages.
- Vertical games turned upright by NEPTRON's renderer (ADR 0015).
- Initial core option values applied when the core declares its options during load (ADR 0016).
- The present rate policy for every display.
- Board families named as tested: Neo Geo, CPS-1, CPS-2 and Cave 68000, after the bench session. Other FBNeo boards are "may work".

### After the MVP

- MAME 2003-Plus as the second core, in a `mame` folder. Then a core setting per game. Then the choice of core by checksum.
- TATE: the screen on its side, with the UI turned (section 15, OP-107).
- Save states, `hiscore.dat`, samples.
- `.7z`, Neo Geo CD, CD images (CHD) of consoles. A check of the content of a disk image (its SHA-1).
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

The Pi 3 column is not a claim. NEPTRON's Pi image is aarch64. The FBNeo makefile does not set `USE_CYCLONE` for the `rpi3_64` platform, so the 68000 runs on the Musashi C core, not the Cyclone ARM32 dynarec. Measured on a Pi 3 B on 2026-11-16 (aarch64, composite NTSC 240p): DoDonPachi and Metal Slug X run with 0 missed fields and 0 underruns, worst field 11.83 ms of 16.65 ms. Late stages and HDMI are not measured (section 17).

**The Pi 3 board list in the release notes and on the About screen follows the measurement and nothing else.** A family that fails it is "may be slow" on the Pi 3. MAME 2003-Plus is the candidate for such a family after the MVP. If a board family needs another core, that is a change to the scope, and the owner decides.

## 3. Folders and aliases

| Folder | Aliases | Core | Extensions |
| --- | --- | --- | --- |
| `share/roms/arcade` | `fbneo` (the Batocera name), `fba` (the RetroPie name) | `fbneo` | `zip` |
| `share/roms/neogeo` | none | `fbneo` | `zip` |

- The folder gives the core. A system has exactly one core, as `data/cores/README.md` requires.
- `.7z` is not accepted. NEPTRON has no `.7z` reader for the set check.
- `mame`, `mame-libretro`, `mame2003` and `mame2003-plus` stay unknown folders in the MVP. The scan lists an unknown folder in `ScanReport::unknown_folders`. For these four names the diagnostics page shows this line:

  > `<folder>`: MAME sets need a MAME core. This version has no MAME core. Put FinalBurn Neo sets in `arcade` or `neogeo`.

### Growth path

1. MAME 2003-Plus runs the `mame` folder, as on Batocera.
2. A `core` setting at the game and system levels picks the core. The settings levels exist.
3. The set check tries the index of each core and takes the core whose CRCs match. The setting overrides it.
4. A merged "Arcade" list over every arcade folder.

## 4. The data files

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
fbneo-vertical-mode = "disabled"
```

- `name` equals the core's `library_name` (**trace**, `tests/traces/fbneo.trace`). Each option value must be one that the trace lists. A core option key can hold `-`.
- `fbneo-vertical-mode = "disabled"` is the core's own default. NEPTRON pins it so that the turns and the aspect of section 11 do not depend on a later change of that default.
- `fbneo-diagnostic-input` is not set. The free set of the trace (`robby`) does not declare it, so no trace shows its values (OP-108). Section 13 gives the value it is to get.
- `Catalog::builtin()` embeds a fixed list of data files with `include_str!` (`crates/neptron-library/src/catalog.rs`). A new file in `data/` is not read until the list names it.
- `always_progressive` is section 9.

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
| Disks | each `disk`: `name`, `sha1` | The set check: the file of each disk image must be present (message 8) |

Example, with an invented description: "Star Patrol - Mission One (XYZ-0001 ~ XYZ-0002)" gives the title "Star Patrol - Mission One" and the details "XYZ-0001 ~ XYZ-0002". A description with no bracket group has no details.

The DAT has no device sets, no refresh rate and no player count.

### The file format

The index is UTF-8 text, one record per line, fields split by one tab (`\t`). Text keeps the file readable in a review and easy to test. The index of the pin has 8,531 sets (14 BIOS sets) and 168,288 ROMs, 7.4 MiB. NEPTRON reads it once, off the frame thread.

```text
neptron-arcade-index	1
core	fbneo	<40-hex commit>
dat	<DAT file name>	<DAT version>
licence	FBNeo licence, see /usr/share/licenses/libretro-fbneo/license.txt
set	<name>	<parent or ->	<romof or ->	<flags>	<year>	<maker>	<board>	<orientation>	<width>	<height>	<title>	<details>
rom	<name>	<size>	<crc>	<merge or ->	<flags>
rom	...
disk	<name>	<sha1 or ->
set	...
```

- Line 1 is the magic and the format version. A reader refuses another magic or a higher version with an error that names both.
- `set` lines are sorted by name in byte order. The `rom` lines of a set follow its `set` line in DAT order.
- Set flags, in this order, `-` for none: `b` BIOS, `v` vertical, `n` not working, `p` preliminary.
- `orientation` is `h` or `v`. `width` and `height` are as the DAT gives them.
- ROM flags: `-`, or `d` for `status="nodump"`. A `nodump` ROM has no CRC (`-`) and the check skips it.
- `crc` is 8 lower-case hex digits. `size` is decimal bytes.
- A `disk` line names a hard disk image (CHD) of the set, from a `disk` element of the DAT. The `disk` lines of a set follow its `rom` lines, in DAT order. `sha1` is 40 lower-case hex digits, or `-` when the DAT gives none. The pinned DAT has 10 such sets, all of the Killer Instinct board (`midway/d_kinst.cpp`). The `disk` line is a new kind of line, so the version stays 1: an older reader skips it and checks the set as before.
- A field never holds a tab or a line break. The generator stops with an error that names the set and the field if the DAT has one.
- The format is append-only: a new field goes at the end of its line, and a reader ignores fields after the ones it knows. A new kind of line gets a new first field, and a reader skips a line whose first field it does not know. A field is never removed or moved. A change that an older reader would misread raises the version.

### How it is made

- `cargo xtask arcade-index --commit <hex> <dat> <out>` reads the DAT and writes the index. The DAT parser (`roxmltree`) lives in xtask only, so the image has no XML code.
- The encoder and the reader of the format live in `neptron-library` (module `arcade`), so xtask writes with the code that the frontend reads with.
- The tests use synthetic DAT text written for the test: invented set names, sizes and CRCs. No ROM, no BIOS and no DAT excerpt goes into the repository.

### How it is installed

- The install step of `os/package/neptron` runs the generator on the DAT in the source of the pinned `libretro-fbneo` package, the same tarball that Buildroot hashes. Each build of the `neptron` package writes the index again, so the index and the core agree. A local build after a new pin keeps the old index until the `neptron` package builds again; CI always builds it (OP-108).
- `os/board/common/check-arcade-index.sh` fails a build whose image has the FBNeo core and no index.
- Path in the image: `/usr/share/neptron/arcade/fbneo.idx` (`Paths::arcade_index`).
- The index ships under the FBNeo licence. `os/board/*/LICENCES.txt` has its line next to the lines of the core.

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
- The list sorts by title. The title, the parent, the flags and the details go into the library index and its cache, because a query does no I/O. A cache of an older version is rebuilt, as for any damaged cache.
- A merged set holds its clones inside the parent zip. Only the parent shows, and only the parent can start. This is how FBNeo works (Batocera wiki).

For an unknown name, the diagnostics page says:

> `<file>.zip` is not a set name of FinalBurn Neo. Check the file name, or the set version.

## 8. The core host

ADR 0016 and this section set what the core host does for arcade. `docs/spec/libretro-host.md` holds the details.

### 8.1 Initial option values

FBNeo declares its options inside `retro_load_game`, after it knows the DIP switches of the game. A value set before the load would be an unknown key. So the host holds the initial values and applies each one at the `SET_VARIABLES` call that declares its key (ADR 0016).

### 8.2 `SET_ROTATION`

- The host answers `true` for 0, 1, 2 and 3 and stores the value as `Rotation` (`R0`, `R90`, `R180`, `R270`, counter-clockwise, as `libretro.h` defines it). Another value: `false` and `Event::Refused(Refusal::Rotation { value })`.
- A call during `load_game` or `Host::open` sets `Host::rotation()`. A call during `run_frame` also adds `Event::Rotation(Rotation)`, in the output of that frame.
- FBNeo calls it in `retro_load_game` and when its vertical-mode option changes (`SetRotation()` in `src/burner/libretro/libretro.cpp`). The frame stays as the board draws it. With `fbneo-vertical-mode = "disabled"` (its default, and NEPTRON's pinned value) the core asks for:

  | Driver flags | Rotation |
  | --- | --- |
  | none (horizontal) | 0, `R0` |
  | `BDF_ORIENTATION_VERTICAL` | 1, `R90` |
  | `BDF_ORIENTATION_FLIPPED` | 2, `R180` |
  | `BDF_ORIENTATION_VERTICAL` and `BDF_ORIENTATION_FLIPPED` | 3, `R270` |

  The value `enabled` (mode 1) turns horizontal games too, for a screen on its side; it is not for the MVP. DoDonPachi is `BDF_ORIENTATION_VERTICAL` with an upright size of 240x320 in its driver (`ddp.cpp`), so it sends 320x240 and asks for `R90`. No trace of a vertical set exists: it needs the owner's set (bench session). The test core covers `R90`, `R180` and `R270`.

### 8.3 Input descriptors

`SET_INPUT_DESCRIPTORS` stores the list (port, device, index, id, description), at most `MAX_INPUT_DESCRIPTORS` (256) entries, each description at most `MAX_CORE_STRING_BYTES`. A longer list is cut and logged once. `Host::input_descriptors()` gives the list. Nothing reads it in the MVP.

### 8.4 The core trace

`core_trace` writes an `option` line, with its values, for each option that the core declares during `load_game` too, after the `load` logs. It writes the rotation after the `av` line (`rotation r90`). The trace format only grows. So the catalog test finds the FBNeo option values.

### 8.5 The zip central directory

`neptron-libretro` makes its central-directory reader public as `zip_entries`, which lists the entries of a zip: name, uncompressed size, CRC-32. It reads only the end record and the central directory, with the listing limits of `docs/spec/libretro-host.md`. `neptron-session` re-exports it as `neptron_session::zip_entries`, because the app reaches the core host only through the session (section 10).

### 8.6 The origin of the aspect

`GameGeometry::aspect_from_core` is `true` when the core reported an aspect above 0, and `false` when the host used its fallback, the base width over the base height (`crates/neptron-libretro/src/geometry.rs`). The session needs it for a turned source (section 11).

### 8.7 What does not change

- Content: FBNeo sets `need_fullpath` and `block_extract`, so the host passes the path of the zip and the core opens it, finds the parent and the BIOS by name, and matches the ROMs by CRC.
- Pixel formats: XRGB8888, or RGB565 for drivers that draw 16-bit only. Both are supported.
- `SET_CORE_OPTIONS_DISPLAY`, the update-display callback, `SET_SUBSYSTEM_INFO`, disk control and `GET_VFS_INTERFACE` stay refused. FBNeo uses them for DIP visibility, Neo Geo CD and consoles, or falls back to stdio.
- Rate: FBNeo reports `nBurnFPS / 100`, cut to 0.01 Hz. The host turns it into millihertz as for every core.

## 9. Interlace: "always progressive"

The session takes a frame of 289 lines or more as interlaced (`INTERLACE_MIN_LINES` in `crates/neptron-session/src/source.rs`). The cores do not say it, and 288 is the tallest progressive console picture. For arcade this guess is wrong: an arcade board draws progressive frames, also tall ones, and a core that turns a vertical game itself (MAME 2003-Plus) sends 320 lines upright. FBNeo sends a vertical frame on its side (240 lines), but a tall board would trip the guess.

The systems data has a key:

| Key | Type | Meaning |
| --- | --- | --- |
| `always_progressive` | boolean, optional, default `false` | Every frame of the system is progressive. The line count does not decide |

- The default keeps every other system as it is. The key is an addition, so `format` stays `1`.
- `arcade` and `neogeo` set it to `true`.
- The app passes it in the launch request (`apps/neptron/src/shell/boot.rs`), and the session sets `SourceGeometry::interlaced` to `false` for every frame (`FrameRules` in `crates/neptron-session/src/source.rs`).
- A tall progressive frame on a 15 kHz CRT then goes to rung 3 (interlaced 1:1), as the owner decided for a medium-resolution board (`crates/neptron-modeline/README.md`).

## 10. The set check before launch

### When and where

- The check runs in the menu process when the user starts a game of the core `fbneo`, before the menu writes the relaunch file and exits with code 75 (`RELAUNCH_EXIT_CODE`) for `neptron-launch` to start the game run. A failed check shows its message in the menu, with no mode change and no relaunch.
- The `Library` of `apps/neptron/src/library.rs` runs the check on a thread of its own, `neptron-set-check` (`Library::start_check`). It owns the thread as it owns the scan thread `neptron-scan`: drop cancels and joins it. The frame thread never reads a file: it shows the launch as pending and polls the result (`Shell::poll_set_check`). A second launch while a check runs is ignored.
- The thread reads only the central directory of each zip it finds for the set, its parent and its BIOS, with `neptron_session::zip_entries` (section 8.5). A central directory is a few KiB. The app gets no dependency on `neptron-libretro`: `neptron-session` mediates between the app and the core host (`docs/definition/11-monorepo-layout.md`). `neptron-library` stays free of I/O on zips and of any dependency on the host.
- The check is pure: `arcade::check_set` in `neptron-library` takes the index record and the entry lists, and returns a verdict.

### Where it looks for files

FBNeo searches for three zip names: the set, its parent and its BIOS (`BurnDrvGetZipName`, indexes 0 to 2). For each name it tries, in this order, the folder of the game, then `system/fbneo/`, then `system/`, and it opens **every** file that exists, not only the first. Then it matches each ROM by CRC in the opened files, in the order they were found; the first file that holds the CRC gives the ROM. A ROM of the wrong size is an error. (`locate_archive` and `open_archive` in `src/burner/libretro/libretro.cpp`. With `fbneo-allow-patched-romsets` on, a `system/fbneo/patched/` folder comes first; NEPTRON leaves that option at its default.)

NEPTRON's system folder is `share/bios`. The check does the same, in the same order:

1. The folder of the game zip.
2. `share/bios/fbneo/`.
3. `share/bios/`.

So `neogeo.zip` can be in `bios/`, in `bios/fbneo/` or next to the games, and a ROM can come from any of them. FBNeo builds each path from the lower-case DAT name and opens it as it is, so the file system decides the case rule. The check opens the same path the same way: on the exFAT share the case of a file name does not matter, on a file system that has case it does, for the check and the core alike.

### What it checks

The rules, in this order:

1. The zip of the game is in the index. Otherwise: message 1.
2. The BIOS of the set: follow `romof` from the set to the first set with the `b` flag. If one exists, a zip of that name must exist in one of the places. Otherwise: message 2.
3. Each zip found for the set, its parent and its BIOS can be read. A damaged zip, ZIP64, multi-disk or a central directory over the limit: message 6.
4. If the set is a clone and needs a ROM that no zip of the set's own name holds, a zip of the parent's name must exist. Otherwise: message 3.
5. The BIOS: if no found zip of the BIOS name holds any ROM of the BIOS set, message 5. If some but not all match, the launch goes on and the diagnostics page lists the differences. FBNeo does not require a ROM that its driver marks `BRF_OPT`, and a Neo Geo BIOS set lists many BIOS variants, of which the core needs only the selected one. The DAT has no such mark, so the check cannot tell which BIOS files are needed. A stricter rule is open point A3 (section 19). The same mark can make a game ROM optional; the check then asks for a ROM that the core would not need. That is safe (it stops, never passes wrongly), and A3 covers it.
6. Each ROM of the set, except `nodump` ROMs and ROMs that are also ROMs of its BIOS set (the same CRC), must be in one of the found zips of the set, its parent or its BIOS, with the same CRC-32 and the same size. The match is by CRC and size, not by name, as FBNeo matches. This one rule covers split, non-merged and full non-merged sets. Otherwise: message 4. When more than half of those ROMs (`WRONG_SET_PERCENT`, 50) are missing or differ, the set is for another emulator or another version: message 4b.
7. Each disk image of the set (`disk` line) has a file where the core looks for it. Otherwise: message 8. Without the file, FBNeo's driver init fails and the core shows its own error screen ("Why the check is needed"). The check comes after the zips, because the player fixes the zips first and the image is a large file to copy.

With no index (section 6), the check gives message 7 and lets the user start the game.

### Where it looks for disk images

FBNeo opens a disk image in `BurnHDDOpen` (`src/burn/burn_hdd.cpp`). The libretro port sets its HDD folder `szAppHDDPath` to the folder of the game zip (`g_rom_dir`, in `retro_load_game_common`). For the disk name of the driver (the `disk` name of the DAT, with no extension), it tries in this order, and opens the first file that exists:

1. `<game folder>/<set>/<disk>.chd`
2. `<game folder>/<parent>/<disk>.chd`, for a clone
3. `<game folder>/<set>/<disk>.img`
4. `<game folder>/<parent>/<disk>.img`, for a clone

The core knows a CHD by its signature, so the extension does not decide the format. The check (`arcade::disk_paths` gives the paths, the check thread looks for each one) does the same, with the same case rule as for the zips. It looks only for a file: it does not read it and does not check its SHA-1, because an image takes hundreds of megabytes and the launch path must stay fast. A file that the core cannot use gives the S2 notice of section 10.1 after the launch, with the words of the core (`HDD: can't use <path>`).

Message 8 names one folder: the folder of the parent for a clone, else the folder of the set. The clones of Killer Instinct use the disk of the parent (`kinst`), and a MAME CHD set keeps that disk in the folder of the parent, so this is the folder that users have.

### The messages

Each message shows in a dialog in the menu, and the full list goes to the log. `<set>` is the zip stem, `<n>` a count. `<names>` gives the DAT names of the first two wrong ROMs, each cut to 24 characters, and counts the others (`gg1.bin, gg2.bin and 1 more`), so the dialog fits the 320x240 canvas. `<version>` comes from the index: the DAT version and the short commit of the core, `1.0.0.03 (commit d7450cf)`. With one file, message 4 says `1 file of this game is missing or differs`.

| # | Text | Launch |
| --- | --- | --- |
| 1 | `<set>.zip` is not a set name of FinalBurn Neo. Check the file name. | Stopped |
| 2 | This game needs the BIOS file `<bios>.zip`. Put it in `bios`, or in the folder of the game. | Stopped |
| 3 | This game needs the file `<parent>.zip` in the same folder. | Stopped |
| 4 | `<n>` files of this game are missing or differ: `<names>`. This image needs sets for FinalBurn Neo `<version>`. The log lists every file. | Stopped |
| 4b | Most files of this game do not match. The set is for another emulator or another version. This image needs sets for FinalBurn Neo `<version>`. | Stopped |
| 5 | The BIOS file `<bios>.zip` does not match this version of FinalBurn Neo. This image needs sets for FinalBurn Neo `<version>`. | Stopped |
| 6 | The file `<file>.zip` cannot be read: `<reason>`. | Stopped |
| 7 | NEPTRON cannot find its list of arcade games, so it cannot check this game. | The user chooses: `Cancel` (the focus) or `Start` |
| 8 | This game needs the hard disk image `<disk>.chd`. Put it in the folder `<folder>`, next to `<set>.zip`. | Stopped |

The owner approved the texts of messages 4, 4b and 5 on 2026-11-17, and of message 8 on 2026-11-18. `<disk>` is the name of the first disk image of the set with no file, and `<folder>` the folder of the parent for a clone, else of the set.

The diagnostics page names the pinned FBNeo commit and the DAT version, so a user can get the right DAT for ClrMamePro.

### Why the check is needed

When a ROM is missing, FBNeo's `retro_load_game` returns `true`, sets a 60 Hz geometry, draws its own error screen and sends `SHUTDOWN` at a button press. It logs `ROM at index %d with name %s and CRC 0x%08x is required` at error level (`open_archive`). Without the check, a CRT would change its mode to show that screen. The host cannot tell this from a good load. (No trace of a missing-ROM case exists: this is the reading of `libretro.cpp`, OP-108.)

**trace** The same happens when the driver init fails for another reason. A core trace of `kinst.zip` with no disk image (2026-11-18, FBNeo `d7450cf`, aarch64 guest) shows: the ROMs all match; the load logs three lines at error level, `HDD: kinst.chd not found`, `[FBNeo] Failed initializing driver.` and `[FBNeo] This is unexpected, you should probably report it.`, the last of its 57 lines; `retro_load_game` returns `true` with a 640x480 geometry at 60 Hz; the core sends the same three lines with `SET_MESSAGE`; every frame is the same white error screen; and each frame with a button of port 0 held sends `SHUTDOWN` (the "press any button to exit" code at the error screen in `retro_run`). The core does not end the process. A launch button still held when the game run starts ends the game at its first frames. Section 10.1 makes sure that such a stop never returns to the list in silence.

### What the check does not prove

A set that passes can still fail in the core: a ROM the DAT marks as needed that the core loads in another way, a BIOS choice of a core option. The core's own screen then shows, after a mode change. The session ends the game at `SHUTDOWN`, and section 10.1 gives the notice.

### 10.1 A game that fails at its start, or that the core stops

This rule holds for every core, not only for arcade. NEPTRON never ends because a game does not start, and a stop by the core never returns to the game list in silence. The game run ends with a relaunch to the game list, as for any end of a game, and the list shows a notice of kind Error that says what happened. A failure in the load does not change the display mode: the first mode comes with the first frame of the session.

| Case | How the session sees it | Notice |
| --- | --- | --- |
| `retro_load_game` returns false | `LaunchError::Refused`, with the error-level core log lines of the load | S1 |
| The core sends `SHUTDOWN` during the load, or in a frame before the source settles | `LaunchError::CoreStopped`, with the error-level core log lines of the load and of those frames | S2 |
| The core sends `SHUTDOWN` before a loop frame ran it with a picture | `EndReason::FailedStart` in the frame report | S2 |
| The core sends `SHUTDOWN` within `START_WINDOW_NS` (10 s of the session clock) after the end of the launch, when it logged an error line or sent a message at the launch | `EndReason::FailedStart` in the frame report | S2 |
| Any other `SHUTDOWN`: a game that ran | `EndReason::CoreShutdown` | S3 |

`end_reason` in `crates/neptron-session/src/start.rs` holds this rule, in pure code. The window: a core that cannot run a game shows its own error screen and stops at the first button press (FinalBurn Neo does). A player who sees that screen presses a button within a few seconds, and 10 s leaves room for a slow reader. A game that runs is seldom stopped by its core so soon, and a stop after the window gives S3, so a wrong label never hides the stop. A picture that a refused mode hides, or the output off, still counts as a picture: the game has started.

Only error-level lines of the core count; info, warn, debug and host lines do not. The host keeps the first 16 error-level records of each call apart from its log queue (`Host::error_logs`, `docs/spec/libretro-host.md`), so a noisy load that fills the queue keeps its error lines. FinalBurn Neo logs 57 lines for Killer Instinct, the three error lines last.

The notice texts (`<name>` is the file name of the game with no extension). `<core text>` is, for S1, the first error-level core log line of the load that is not blank; for S2, that line of the launch, or else the last `SET_MESSAGE` text, or else the last error line of the core in a frame (a core that loads in silence can log its reason in the frame in which it stops); for S3, the last `SET_MESSAGE` text, or else the last error line of the core in the frames. Each control character of the core text, line breaks and tabs included, and each Unicode line or paragraph separator (U+2028, U+2029) becomes a space; the ends are trimmed; the text is cut to 60 characters, on a character, with `...`.

| # | Text |
| --- | --- |
| S1 | `<name>` did not start: the core refused the game file. The core says: "`<core text>`". Check that the file is complete and is for this system. |
| S2 | `<name>` did not start: the core stopped it at the start. The core says: "`<core text>`". Check that the game has all its files. |
| S3 | `<name>` stopped: the core ended the game. The core says: "`<core text>`". |

With no core text, the sentence "The core says" is left out. The owner approved S1, S2 and S3 on 2026-11-18. Every other launch error keeps its text, `<name> did not start: <error>.`

The game list shows one notice, and the notice that loses always goes to the log. After S2 there was no play to save, so S2 comes before the notice of a lost save. After a game that ran, a lost save is worth more to the player, so it comes before S3 and before a session error ("The game stopped: ...", for example a stalled display after an hour of play). `pick_notice` in `apps/neptron/src/shell/intents.rs` decides, in pure code.

The log gets one line for the failure or the stop with the same text, one line for each error-level core log line of the launch (at most 8), and a line with the count of log records of the launch that the host lost, when it lost some. The session collects the error lines during the launch only, and keeps the last error line of the frames in a buffer made at launch, so the frame loop does not allocate for them.

Killer Instinct with no disk image (`kinst.zip` alone) reaches none of these rows on an image whose index has `disk` lines: message 8 stops it before the launch. With an older index, it reaches the fourth row (three error lines and three messages at the launch, and a stop at the first button press). With a file at `kinst/kinst.chd` that the core cannot use, it reaches the fourth row too.

## 11. Vertical games

### What the core does

FBNeo draws a vertical game as the board draws it, on its side, and calls `SET_ROTATION` to make it upright (section 8.2 has the values). Its frame size comes from `BurnDrvGetFullSize`, which swaps the driver's upright size for a vertical driver: DoDonPachi's driver says 240x320, the frame is 320x240. Its aspect comes from `BurnDrvGetAspect`, the driver's own aspect with no swap when `fbneo-vertical-mode` is `disabled`: 3:4 for DoDonPachi, the shape of the upright picture. (`retro_get_system_av_info` in `libretro.cpp` and `burn.cpp`. The core swaps the aspect only for the `enabled` and `alternate` modes, and for the TATE modes on a vertical driver.) Its option `fbneo-vertical-mode` has the values `disabled` (the default), `enabled`, `alternate`, `TATE` and `TATE alternate`. FBNeo cannot send an upright frame to a horizontal screen.

### What NEPTRON does

- The host accepts the rotation (section 8.2).
- The session builds `SourceGeometry` with the rotation. `SourceGeometry::aspect_milli` is the aspect of the frame as the core draws it: the policy (`take_source` in `crates/neptron-modeline/src/policy/source.rs`) swaps the axes of a turned source and inverts its aspect.
- So for `R90` and `R270`, **and only when the core reported the aspect** (`GameGeometry::aspect_from_core`, section 8.6), the session sets `aspect_milli = round(1_000_000 / core_aspect_milli)`. DoDonPachi: the core gives 750, the source gets 1,333, the policy gets 750 back for 240x320. Without this step a vertical game is placed with the inverse shape.
- When the core gives no aspect (0 or less), the host falls back to the base width over the base height (`crates/neptron-libretro/src/geometry.rs`). That value is already the shape of the frame as drawn, so the session keeps it. A turned 320x240 frame then gets 1,333, and the policy gives the upright 240x320 picture square pixels.
- The policy places the upright picture by the rule below: it swaps the axes, treats the long side as lines, and keeps the shape of the upright picture.
- `Scene` carries the rotation, and both renderers turn the game picture (ADR 0015). The UI canvas is never turned.

### The owner's rule

The owner decided the video rule for arcade games on 2026-11-16. In substance:

- A sharp picture comes first. A soft picture is worse than a border.
- A game is never stretched. It keeps its shape on every display.
- Black borders above and below are acceptable when they are not too big. The owner does not like large borders, but he likes a soft picture less.
- On a 15 kHz screen a vertical game is 480i, with its lines 1:1.
- On a VGA CRT or a flat panel more modes exist, so the policy chooses a better fit: the largest whole-number scale of the game's lines that the display can show at the game's own rate, also when that mode has more lines than the 480p that a horizontal game gets.
- On a CRT the horizontal scale is free (the signal is analog). The policy chooses the mode width for the right shape. Only the line count must be a whole multiple of the game's lines.
- A soft "fit" is only an opt-in value of the setting.

### The decisions of 2026-11-16

The owner decided A7 to A12 on 2026-11-16. The rule below holds them:

| Item | Decision |
| --- | --- |
| CRT multiple (A7) | The smallest whole multiple that fills the screen. The picture modes `3x` and `4x` still ask for more |
| Interlace | Never an interlaced mode, and never any mode, that the monitor profile does not declare. Interlace is allowed on composite TV and on a profile that declares it (the 15 kHz and 25 kHz ranges). Nothing widens a profile range or a validator limit |
| Border too big (A8) | A picture below 60 % of the screen height. Below it, an interlaced 1:1 mode that fills more wins where the profile declares interlace; else the small exact picture stays. The policy never falls to a soft fit on its own when an exact mode exists |
| Flat panel, every game (A9) | When the scale that fills the height is 4.0 or more, the game fills the height, sharp bilinear, with its shape kept. Under 4.0 it takes the largest whole multiple that fits. The value `integer` of `display.panel_scale` forces whole multiples at every scale. A picture below 60 % of the height at 1x keeps 1x, with a notice |
| TV width (A10) | The systems `arcade` and `neogeo` fill the TV width by their aspect (a board fills its monitor), not by the console width rule |
| Double refresh (A11) | A board below every CRT range gets double refresh, each frame shown twice, for every style |
| `display.vertical` (A12) | The values `exact` (default) and `fit`, on a CRT, a TV and a panel |
| Scanlines on a panel | A flat panel with scanlines (`display.scanlines = on`) takes the largest whole multiple and never the fill: the renderer draws scanlines only at a whole vertical scale |
| Notices | Every result that is not exact tells the user through the notice of the quick menu: a soft picture because no exact mode exists (with the reason), a small picture, a speed change, double refresh. The policy returns them as data |
| Vertical games | A vertical game on a horizontal display is the best possible case: no odd modes and no odd default scaling. TATE is the right answer later (OP-107) |

### The rule

The rule applies to every arcade source and, where it says so, to a turned source. It decides in the policy and gives the validator every mode. It widens no limit: every mode stays inside a range of the profile, an interlaced mode needs an interlaced window of the range, and `neptron_safety::validate` judges every proposal.

Terms: `h` is the game's lines after the turn, `r` its rate, `a` its display aspect after the turn. A mode is **exact** when its game lines are `k x h` for a whole `k` (or `h` interlaced, rung 3), its field rate is `r` or `2r` within `REFRESH_MATCH_MHZ`, and its filter is nearest on the vertical axis. The **fill** of a mode is the game's lines over the picture lines of the range at that rate (`Padding::fill_permille`, which already counts the lines that the minimum line rate forces).

**R1. Shape, every display.** The game keeps its display aspect within one pixel. A game wider than the face takes the tallest height a range holds that is short of its shaped height by less than one pixel of the game, when no range holds the shaped height rounded up. No mode squeezes a game by more (decided, V7). On a CRT the mode width is the width at which the game's lines have the aspect `a` on the 4:3 face: `border_width` for every rung, bordered or not, and for the fit. A turned board is pillarboxed. The game's pixel width stays a whole multiple of its width (native or dynamic super width); only the mode width changes. On a TV and a panel, the present width rules hold, except R7.

**R2. Rates on a CRT.** The targets in order: the asked rate (`exact_x2`, `force_50`, `force_60`) when a range covers it; `r`; for any source that no range covers, `2r` when a range covers it (`below_ranges_double_target`), else the covered rate nearest `2r` with each frame twice when that changes the speed less than the nearest rate once (30 Hz on `ntsc`: -0.1 % against +99.8 %); the nearest covered rate; when the exact rungs gave no mode at `r` (or the nearest covered rate), the highest lower rate at which a range holds the game's lines (`arcade_25` holds 384 lines up to 60.000 Hz), ranked before the fit while it changes the speed at most `EXACT_SPEED_MARGIN_PPM` (2 percentage points) more than the fit, within `REFRESH_MATCH_MHZ` (decided, V6 of `docs/spec/video-policy-cases.md`); the last resort. On a profile of one field rate (`ntsc`, `pal`) a mode that misses the rate takes the next whole multiples of the game width until one hits it to the millihertz.

**R3. Ranges on a CRT.** Each range is its own window (`per_range` in `policy/crt.rs`). For each rung and each range that covers the rate, the policy shapes the mode inside that range: its own minimum (for the border), its own capacity (for the multiple and the crop), its own interlaced window (for rung 3). A range whose fewest progressive lines exceed what it holds at the rate adds no capacity. Rung 3 is open when the game's lines do not fit the progressive capacity of every range, or when the interlaced mode fills more than every progressive exact mode that is below the fill threshold (R4). A Switchres mode outside the line rates that the validator takes moves into them by lines of the vertical back porch at the same field rate, so a range whose lowest and highest line rate are equal gets a mode.

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

**R7. TV.** The fit of a board taller than the interlaced mode goes into the interlaced mode. An arcade board (the systems `arcade` and `neogeo`) gets the aspect width (`board_samples`: `aspect x lines x 2.2` on 525 lines, `x 1.8333` on 625 lines, at most 704 samples), not the console width (decided, A10). The rest of the TV rules hold: lines 1:1, the crop up to 8 lines per edge, the speed alert.

**R8. Panel, every game.** Let `s` be the scale at which the game, at its aspect, fills the height of the preferred mode (or its width, when the width is reached first), in thousandths. When `s` is at least `PANEL_FILL_MIN_SCALE_MILLI` (4,000), the game fills that height, centred, with its shape kept, sharp bilinear. Under 4,000 it takes the largest whole vertical multiple that fits, nearest on that axis, centred. `display.panel_scale = integer` takes the largest whole multiple at every scale, and so do scanlines on a panel (`display.scanlines = on`, `PolicySettings::panel_scanlines`): the renderer draws them only at a whole vertical scale of 2 or more. A fill whose height is a whole multiple of the lines takes the whole path (nearest, status exact). The scale rounds down, so 3.9995 stays under 4. Below 60 % of the height at 1x (the 480x640 board on 1080p, 59 %) the picture stays 1x with the small-picture notice; `display.vertical = fit` fills the height for a turned game. The panel never gets a mode that its own scaler stretches.

**R9. Horizontal boards.** They differ from consoles in four points, and the rest of the console rules hold:

- An arcade board fills its 4:3 monitor, so on a TV it gets the aspect width (R7).
- Board rates are 54.70 Hz to 60.606 Hz, and 30 Hz for Little Robin. A CRT shows each rate exactly (R2). A panel and a TV change the speed (section 12).
- Heights of 256 lines (R-Type, Mortal Kombat) are 1:1 on 15 kHz at 55 Hz and below, and 2x 512 lines on 31 kHz; on NTSC they lose 8 lines per edge.
- Medium-resolution boards (384 lines, System 24) are interlaced 1:1 on 15 kHz, 1:1 on 25 kHz, 1:1 in about 500 lines on 31 kHz (74 % to 77 %), and on a multi-sync tube 1x or 2x by A7.

**R10. Safety.** Nothing in R1 to R9 changes a validator rule, a range, the hard rails or `ValidatedMode`. A change of the mode width, of the range that makes a mode, or of the target rate is a different modeline that the validator judges as always.

### The overscan crop

The crop hides at most 8 source lines per edge (`OVERSCAN_CROP_MAX_LINES_PER_EDGE`) and at most 8 lines of a 240-line screen per edge as a share of the lines the range holds (`OVERSCAN_CROP_SCREEN_LINES`, rounded up): 9 of 256 lines at 15 kHz, 16 of 480 at 31 kHz. A Game Boy at 2x on a 256-line 15 kHz tube would lose 16 screen lines per edge, so it shows 1x.

### The results

Appendix A ([`arcade-table.md`](arcade-table.md)) holds the result of the policy for each reference board (Appendix B) on 36 displays. Every cell follows the rule or carries an allowed reason (run of 2026-11-23, equal to the run of 2026-11-16):

| Verdict | Cells |
| --- | --- |
| ok | 619 |
| ok: no exact mode exists on this profile | 21 |
| ok: the profile declares no interlace | 3 |
| ok: no window at the game's rate (the exact rung at the nearest rate that holds it) | 1 |
| ok: overscan crop | 4 |

`tests/golden/policy_arcade.toml` (`crates/neptron-modeline`) holds every cell that follows the rule as a golden.

The main cases, DoDonPachi (240x320 upright, 57.55 Hz) and the 480x640 board (Ikaruga, 60 Hz):

| Display | DoDonPachi | 480x640 board |
| --- | --- | --- |
| 15 kHz (`generic_15`) | Rung 3: 448i at 57.550 Hz, 320 lines 1:1, 71 %, pillarboxed. With `fit`: 240p, fitted, sharp bilinear | Fit into 480i, 0.75, right shape; notice: soft, too tall |
| 31 kHz only (`edid_31`, `arcade_31`, `pc_31_120`) | 320 lines 1:1 at 57.550 Hz in 501 to 520 lines (62 % to 64 %), pillarboxed | Fit into 509p, 0.80, right shape; notice: soft, no interlace |
| Multi-sync to 54 kHz | 2x, 427x640p, right shape | 1x, 853x640p, right shape |
| Multi-sync to 96 kHz | 2x, 427x640p (A7: the smallest that fills) | 1x, 853x640p |
| Panel 720p | 2x, 480x640, nearest, 89 % | 1x, 480x640, nearest, 89 % |
| Panel 1080p | 3x, 720x960, nearest, 89 % (scale 3.4, under 4) | 1x, 480x640, 59 %, small-picture notice |
| Panel 4K | Fills the height: 1620x2160, sharp (scale 6.75); with scanlines on, 6x, nearest | 3x, 1440x1920 (scale 3.4) |
| TV, NTSC (Pi composite) | 480i, 320 lines 1:1, 67 %, 264 of the 704 samples wide, sharp bilinear. 59.940 Hz: +4.15 % speed | Fit into 480i, 0.75 |
| TV, PAL | 576i, 320 lines 1:1, 220 samples wide. 50.000 Hz: -13.1 % speed | Fit into 576i, 0.90 |

The 480i field order on the Pi `vc4` is not checked yet (OP-103 point 5).

Results that the rule accepts:

- Borders: a 320-line board on a 31 kHz-only tube is 1:1 in 482 to 520 lines (62 % to 66 %); Pac-Man is 288 lines in 473 to 506 (57 % to 61 %); Donkey Kong is 256 lines in 479 on `arcade_31` (53 %). No whole multiple above 1 fits these tubes, and the overscan crop (8 lines per edge at most) does not reach it. The rule refuses a soft fit, so these stay.
- 15 kHz mode size: the interlaced 1:1 mode on a 15 kHz preset is 448i, the smallest interlaced mode of the range, not 480i. The game's lines are the same on the tube: the rest of the field is black lines or blanking.
- Panel filter: on a panel the vertical scale is whole and nearest. The horizontal scale is whole only when the board's pixels are square at that scale (DoDonPachi 3x: 720 pixels for 240). Gunbird (224 pixels, 3:4 at 320 lines) gets 720 pixels at 3x, 3.21 times its width, with sharp bilinear on that axis. This is the panel rule for every source with pixels that are not square.
- The overscan crop keeps the width, so the 240 lines that a 256-line board shows on NTSC are 6.2 % narrow: the full width would need 751 of the 704 safe samples.
- No exact mode exists for the 480x640 board on 15 kHz, on a 31 kHz profile with no interlace, and on a TV: the fit, with its reason in the notice.

### The notices

The policy returns the reason for every result that is not exact as data: `Notice::reasons`, a list of `NoticeReason` values, beside the flags that `Notice` holds. The app shows them in the quick menu with the other notices (`apps/neptron/src/shell/picture.rs`, which owns the texts). The reasons of the picture mode (`FillFallback`, `WholeSteps`) are in `docs/spec/video-policy-cases.md`, section 8.9.

| `NoticeReason` | When | Text |
| --- | --- | --- |
| `Soft(SoftReason::TooTall { lines })` | The game's lines fit no exact mode of the display at its rate | "Soft picture: this screen cannot show the game's {lines} lines." |
| `Soft(SoftReason::NoInterlace { lines })` | The game needs an interlaced mode, and neither the profile nor the GPU offers one | "Soft picture: the game needs {lines} lines, and this monitor profile has no interlaced mode." |
| `Soft(SoftReason::NoRate)` | No exact mode at any rate the display covers | "Soft picture: no sharp mode at this game's rate." |
| `Soft(SoftReason::Chosen)` | `display.vertical = fit` | "Soft picture: the setting display.vertical is set to fit." |
| `Soft(SoftReason::FillMode)` | The picture mode `fit` | "Soft picture: Picture mode is set to Fill." |
| `SmallPicture { fill_permille }` | Below 60 % of the screen height | "Small picture: the game uses {fill} % of the screen height, to stay sharp." |
| `SpeedChange { game_mhz, shown_mhz }` | The game runs at another rate | "The game runs at {shown} Hz, not {game} Hz: {percent} % speed." |
| `DoubleRefresh { field_mhz }` | Each frame shows twice | "Each frame shows twice at {shown} Hz. The game speed does not change." |

A panel that fills the height at a scale of 4.0 or more gives no notice: that is the panel rule.

### The setting `display.panel_scale`

Since the picture mode (`docs/spec/video-policy-cases.md`, section 8), this key is an expert key: the menus do not show it, and a settings file keeps it. `sharp` and `fit` set the scale of a panel themselves.

| Key | `display.panel_scale` |
| --- | --- |
| Label | Panel scale |
| Choices, in menu order | `auto` (default): rule R8. `integer`: the largest whole multiple at every scale |
| Levels | game, system, global (`Levels::ALL`) |
| Applies to | A flat panel only |
| Safety | `Validated` |
| Help text | "How a game fills a flat panel. Auto fills the height when the picture is large, and uses whole steps when it is small. Integer always uses whole steps, with black borders." |

### The setting `display.vertical`

Since the picture mode (`docs/spec/video-policy-cases.md`, section 8), this key is an expert key: the menus do not show it, and a settings file keeps it. The picture mode `fit` gives its `fit` for every progressive source, and `sharp` reads it as `exact`. The rows below are the rules of the key as a file sets it.

| Key | `display.vertical` |
| --- | --- |
| Label | Vertical games |
| Choices, in menu order | `exact` (default): every pixel exact, with the largest picture that a whole scale gives; on 15 kHz and on a TV interlaced, with flicker. `fit`: the picture fills the height of the screen with its shape kept, scaled, soft; on 15 kHz and on a TV progressive, with no flicker |
| Levels | game, system, global (`Levels::ALL`) |
| Where the row shows | No row: `display.vertical` is an expert key of the settings file. The picture mode covers it (`PICTURE_MODE_COVERS`): a pick of a picture mode at a level removes it at that level, and Fill gives the fit of a turned game. A value that a file still holds acts, and the notice names it |
| Applies to | Every display: a CRT, a TV and a flat panel |
| Effect | Only for a turned source (`R90`, `R270`). `exact` is rule R1 to R8. `fit` drops every exact rung below the fill threshold for that source (a `Dropped` reason, `VerticalFit`), so the fit wins where the exact picture is small: on 15 kHz and on a TV the progressive fit (240p or 288p, sharp bilinear); on 31 kHz only the fit into about 500 lines; on a panel the fractional scale. Where an exact mode fills at least the threshold (a multi-sync tube, a panel at 3x and above), `fit` changes nothing |
| Safety | `Validated`, as the picture mode |
| Help text | "How a vertical game shows on a horizontal screen. Exact shows every pixel exact, with black borders, and on a 15 kHz screen or a TV with flicker. Fit fills the height, with a soft picture." |

- The choices are append-only. A cropped 1:1 view and the TATE choices are later values at the end of the list.
- The picture mode still applies: `3x` or `4x` on a multi-sync tube asks for that multiple of a vertical game.
- The crawler finds the setting, as every setting.

### The notice the first time

The first time a vertical game gets the interlaced rung on a 15 kHz CRT or a TV with `display.vertical = exact`, the frontend shows this notice once (`VERTICAL_INTERLACED_TEXT` in `apps/neptron/src/notices.rs`):

> This is a vertical game on a horizontal screen. It shows interlaced, with every pixel exact. Picture mode Fill, in Settings > Display or in Video in the quick menu, shows it as a soft picture with no flicker. A screen turned on its side (TATE) comes in a later version.

The frontend keeps the notices already shown in `state/config/notices.toml` (open point A4). The writer thread of `apps/neptron/src/outbox.rs`, which does every file write of the frame loop, writes it as temp file, fsync, rename; the frame thread only sends the job. The menu process reads the file at boot with the settings. A deleted file shows the notice again and loses nothing.

## 12. Rates

FBNeo reports each board's rate, cut to 0.01 Hz (`BurnSetRefreshRate` in `burn.cpp` stores `(INT32)(100.0 * rate)`). The about 95 ppm that the cut takes from the Neo Geo cannot be heard.

| Board | Rate reported | Source (FBNeo driver) |
| --- | --- | --- |
| Neo Geo | 59.18 Hz (board: 15625/264 = 59.1856) | `neo_run.cpp` |
| CPS-1, CPS-2 | 59.63 Hz | `cps.cpp` |
| Toaplan 2 | 59.63 Hz (59.637, cut) | `toaplan.h` |
| Psikyo | 59.29 Hz (15625/263.5 = 59.2979, cut) | `d_psikyo.cpp`: `BurnSetRefreshRate(15625.0 / 263.5)` |
| Taito F3 | 58.94 Hz (58.944, cut) | `d_taitof3.cpp` |
| Cave 68000 | 57.55 Hz (15625/271.5) | `cave.h` |
| Data East dec0 | 57.41 Hz | `d_dec0.cpp` |
| Irem M72 | 55.00 Hz | `d_m72.cpp` |
| Midway Y Unit | 54.70 Hz (54.82 for a 400-line screen) | `d_yunit.cpp`: `BurnSetRefreshRate((nScreenHeight == 400) ? 54.82 : 54.70)`. Mortal Kombat is 400x256 |
| Midway T Unit | about 54.7 Hz | UNVERIFIED |
| CPS-3 | 59.59 Hz (59.59949, cut) | `cps3run.cpp`. 384x224, or 496x224 in the wide mode |
| Pac-Man, Galaga, Donkey Kong | 60.00 Hz (board: 60.606 Hz) | `d_pacman.cpp`, `d_galaga.cpp`, `d_dkong.cpp` set no rate, so `nBurnFPS` keeps 6000 (`burn.cpp`). The game runs 1 % slower than the board on every display |
| Little Robin | 30.00 Hz | `d_littlerb.cpp`. A CRT needs double refresh for it (section 11, R2) |

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
- OP-099: FBNeo gives the Neo Geo 304 pixels wide, not 320 (research reading; trace). At 6 MHz, 304 pixels are about 684 samples, against the 644 of the console rule: about 6 % narrow on composite, not the 11 % that OP-099 states for 320. Rule R7 gives an arcade board the aspect width on a TV.

## 13. Controls

- FBNeo maps the arcade inputs to the RetroPad. Select is the coin, Start is start.
- For `RETRO_DEVICE_JOYPAD` FBNeo uses its "Classic" layout (the Neo Geo CD pad): Neo Geo A to D on the face buttons; L and R are the 5th and 6th buttons of a six-button game. The host passes `RETRO_DEVICE_JOYPAD` on every port (`JOYPAD_PORTS`, ports 0 to 3), so a four-player board gets four pads.
- DIP switches are core options. Each game declares its own (section 8.1).
- `fbneo-diagnostic-input` opens the service menu. Its default is "Hold Start" (`retro_common.cpp`), and NEPTRON keeps it (section 4). NEPTRON's setting `input.menu_hold_start_seconds` opens the quick menu with a Start hold, for an arcade panel with no Select. Its default is 0 (off), so the two clash only when the user turns it on, and then one hold does two things. The value for NEPTRON is "Start + L + R", one of the values in the core's option table; it goes into `data/cores/fbneo.toml` when a trace of a set that declares the option shows it (OP-108, ADR 0016). A combination with Select is not used, because Select is the coin and a hold of Select with Start opens the quick menu.
- Select is the coin, so a player presses Select and Start in a quick sequence, and the two can overlap for a moment. Such an overlap opens no quick menu, and the game gets the coin and the start. Only a hold of both for 0.5 s opens the quick menu (section 13.1).
- A later item: show the descriptors of section 8.3, and a layout per system or per game (Classic, Modern, 6-Panel) as the libretro device subclass.

### 13.1 The quick menu hold

The owner decided on 2026-11-18: the quick menu opens only when Select and Start stay held together for 0.5 s. A player who inserts a coin and presses Start in a quick sequence overlaps the two for a moment, and that must not open the menu.

The rule, on each port (`Hotkeys` in `neptron-input`):

- The hold starts at the first `Hotkeys::update` that sees Select and Start down together on the port. The time is the `now_ns` of `update`: the frame clock, in nanoseconds.
- The quick menu fires once, when the hold has lasted `QUICK_MENU_HOLD_NS` (500 000 000 ns): at the first `update` at or after that time, not one nanosecond before, and not at the release.
- A release of Select or Start ends the hold. A shorter hold opens nothing.
- A hold fires once. While both buttons stay down, it does not fire again. A new hold starts when both are down again: the player releases one button and presses it again, or releases both and presses both. This is the rule of the blind recovery hold too.
- `Hotkeys::cancel_quick_menu` (the session calls it at resume): the hold in progress gives no quick menu. The next hold, after a release, can.
- The Start hold (`input.menu_hold_start_seconds`, off by default) is a separate rule. On one port, one press of Start gives one quick menu at most: when the Start hold fired, Select that joins later on the same press of Start gives no second one. Select down first stops the Start hold.
- Every source follows the rule, because it reads only the port state: any pad on any port, a generic pad with Select (button 8) and Start (button 9), and the keyboard pair Right Shift (Select) and Enter (Start).
- The time is a constant. A setting can come later, in the same way as `input.menu_hold_start_seconds`.

The blind recovery: `BLIND_RECOVERY_COMBO` is the same two buttons, held for `BLIND_RECOVERY_HOLD_NS` (5 s), and it fires once per hold, also while the quick menu is open. A hold has no way to know at 0.5 s that it goes on to 5 s. So a 5 s hold in a game opens the quick menu at 0.5 s, and the recovery fires at 5 s under the open menu. The game is paused under the safe picture, and the notice of the safe mode shows at once over the menu. In the game list and the other menus the quick menu hotkey does nothing, so there the hold gives the recovery only.

What the game gets (`PadMask` in `neptron-session`), on each port of the core:

- A button of the combination down alone reaches the core as usual. Select pressed first is a coin.
- When the combination goes down, the button that completes it is held back from the core: both buttons, when they go down in the same poll. The button that was down already stays down for the core. So the core does not see both buttons down while the combination is down.
- When the combination ends before the quick menu opens, the held-back button reaches the core. If it is still down, the core sees it down until its release. If it is up, the core sees it down for as many core frames as it was held back, and then up. So a coin and a quick Start give the game one coin and one start. The start is late by the time of the overlap, less than 0.5 s. A late press can show with the other button still held: that is what the player pressed.
- A late press lasts at least `LATE_PRESS_MIN_FRAMES` (3) core frames. Some arcade games read a coin only when it stays down for 2 frames or more, so a late press of one frame can be lost. 3 frames are 50 ms at 60 Hz.
- A late press lasts at most `LATE_PRESS_MAX_FRAMES` (30) core frames, about the 0.5 s of the hold at 60 Hz. A combination that cannot open the menu (Select joins a press of Start whose Start hold fired) can be held back for seconds, and a replay of seconds would be a press that the player did not make. A press held back for longer replays for 30 frames.
- Only frames in which the core runs count, for the time held back and for the late press. So a late press does not run out while the core is paused or the output is off.
- A short tap of Select and Start reaches the game, as on the original hardware, so a game's own Select and Start combination (for example a soft reset with other buttons) works with a tap; only a hold of 0.5 s opens the menu.
- When the quick menu opens, the held-back press is dropped: it was for the menu. When the menu closes, every button still down stays up for the core until its release. So the menu leaves no stuck press and no phantom press in the game. The menu side is the same: when the quick menu opens, the app calls `Navigator::hold_back`, so a direction held in the game (a crouch on the lever) gives no action and no repeat in the menu until its release. Without it the repeat runs the cursor down to `Quit game`.

## 14. Save files

- FBNeo writes NVRAM and EEPROM files itself (Cave high scores and settings) to `<save directory>/fbneo/`, that is `state/saves/arcade/fbneo/` or `state/saves/neogeo/fbneo/`. The same set in the two folders has two save places.
- FBNeo has no `RETRO_MEMORY_SAVE_RAM`. `copy_save_ram` gives 0 bytes. A test checks that a core with no save RAM gives no `.srm` file and no `.bak` (`crates/neptron-session/tests/integration/arcade.rs`).
- The core's own writes are not atomic: a power cut during a write can damage an NVRAM file. NEPTRON cannot change this without a patch to the core. It is a risk (section 18), not a rule break: the code standard covers the files that NEPTRON writes.
- Save states are after the MVP.
- `fbneo-hiscores` stays at the core's default. Without `system/fbneo/hiscore.dat` it does nothing. Shipping `hiscore.dat` is a later item.

## 15. TATE (planned, after the MVP)

TATE is a screen turned on its side. It gives a vertical game its native mode: DoDonPachi as 320x240 at 57.55 Hz, 240p, every pixel exact, no flicker. This is the best picture a CRT can give a vertical game, and a strong reason for a vertical cabinet. It needs the UI turned too, so it is a separate task: the first task after version 0.1 (OP-107).

### The owner's decision (2026-11-19, OP-107)

- One system setting gives the screen orientation: 0 (YOKO), 90, 180 or 270 degrees. The UI follows it.
- In Auto, a vertical game runs at its own lines, full screen. A horizontal game shows upright and small, with a notice.
- Each game has an orientation override (Auto, 0, 90, 180, 270) in its display settings.

### Design notes

- The screen orientation is a global setting, safety `ConfirmOrRevert`: the user must be able to read the keep or revert dialog in the new orientation, or the revert must come with no input.
- The game turn that the renderer applies is the core's rotation less the screen rotation, modulo 360 degrees.
- A vertical game on a vertical screen: turn 0. The policy gets the frame as drawn (320x240) and finds its native mode.
- A horizontal game on a vertical screen: turn 90 or 270. The policy swaps the axes and the game gets the treatment of section 11 that a vertical game gets on a horizontal screen. `display.vertical` then applies to horizontal games: its meaning is "a game whose long side is across the tube's lines".
- The core keeps `fbneo-vertical-mode = "disabled"`, so it always reports the turn that the board needs (`R0`, `R90`, `R180` or `R270`, section 8.2) and the aspect of the upright picture. NEPTRON does all turns, so one code path serves both orientations. The core's `TATE` modes are not used: they move the turn into the core and change the aspect it reports.
- The validator and the hard rails see the real raster of the tube. TATE changes no safety rule.
- `Scene` gets a turn for the UI canvas (a second field, by a new ADR), or one screen turn that applies to the game and the UI. ADR 0015 leaves both open.
- The UI needs a portrait layout: a 240x320 canvas, or the 320x240 canvas turned and scaled. A turned 320x240 canvas does not fit the 240-pixel width of a turned 15 kHz screen at 1:1, so the portrait layout is the likely path. That is the largest part of the task (`neptron-ui` screens, scenes and goldens, the crawler in both orientations).
- The menu directions follow the viewer: up on the d-pad is up on the turned screen. The game's input is not turned: the controls of a cabinet do not move.
- Every screen of the frontend must be turned: the boot prompt, the keep or revert dialog, the safe mode, the recovery screens, the notices.
- Tests: goldens of the policy for turned screens on every profile; scene goldens of each UI screen in portrait; `gles-compare` scenes with a turned UI; a bench check with a turned monitor (a hardware checklist line). Whether the owner can turn a monitor for this test is a question for the owner.

## 16. Where the code is

| Part | Where |
| --- | --- |
| Held option values, `SET_ROTATION`, `aspect_from_core`, input descriptors, `zip_entries` | `crates/neptron-libretro`; the re-export in `crates/neptron-session` |
| Rotation and aspect cases, a core with no save RAM | `crates/neptron-testcore`, `crates/neptron-session/tests/integration/arcade.rs` |
| The core package | `os/package/libretro-fbneo` |
| The data files | `data/systems/arcade.toml`, `data/systems/neogeo.toml`, `data/cores/fbneo.toml`; the trace `tests/traces/fbneo.trace` |
| The index format, the set check | `crates/neptron-library` (module `arcade`) |
| The index generator | `cargo xtask arcade-index`; the install step of `os/package/neptron` |
| The check thread, the notices S1 to S3 | `apps/neptron` (`library.rs`, `shell/intents.rs`, `start_failure.rs`) |
| The video rule | `crates/neptron-modeline` (`policy/`); the table `tests/integration/arcade_table.rs`; the goldens `tests/golden/policy_arcade.toml` |
| The turn of the game picture | `Scene::rotation` (ADR 0015), both renderers |
| `display.vertical`, `display.panel_scale` | `crates/neptron-settings`, `crates/neptron-modeline/src/policy/settings.rs` |

## 17. Measurement plan

### Free material

- MAME's free sets (mamedev.org/roms) are "approved for free distribution on this site only". They are downloaded to the development machine and never committed (OP-053 rule). FBNeo runs Gridlee, Robby Roto, Alien Arena, Targ, Spectar, Side Trak, Teeter Torture, Hard Hat, Circus, Robot Bowl, Rip Cord and Crash (research list). They are light 8-bit boards: they test the package, the scan, the set check, the options, the rates and the core trace, not the speed. The trace of the data files uses Robby Roto (`robby`).
- The DAT has `test240p` (the 240p Test Suite for Neo Geo). Its licence is UNVERIFIED. It needs `neogeo.zip`, which is SNK code.
- The NeoOpen BIOS (`neopen.sp1`, ISC) is an optional entry of FBNeo's `neogeo` set. Whether FBNeo boots with it in place of the SNK files is UNVERIFIED.
- No free Cave, CPS or vertical set is known. `neptron-testcore` covers the rotation path: a 320x240 frame on its side with `SET_ROTATION` 1, 2 and 3 (`R90`, `R180`, `R270`), with an aspect of 3:4 and with no aspect.

### What only the owner's sets show

The owner's own sets and `neogeo.zip` stay outside the repository, on the development machine and the Pi. Section 2 has the first Pi 3 result. Still to do:

1. The heaviest scenes (DoDonPachi late stages, Metal Slug X boss fights), with the owner's play input recorded once with `neptron-testkit` and replayed. Report p50, p99 and the maximum of the CPU time of `retro_run` against the core budget. The research estimates the budget at about 11 ms on the Pi 3 (UNVERIFIED); the frame statistics of OP-103 set it.
2. The same on the PC.
3. `fields_missed = 0` on HDMI.
4. The 480i picture of DoDonPachi on the PC CRT and on the TV.

The results go into `docs/hardware-checklist.md`. No hardware result is claimed before that.

## 18. Risks

| Risk | What we do |
| --- | --- |
| Pi 3 speed with no Cyclone on aarch64. Cave and CPS-2 are most at risk | The measurement decides the Pi 3 list (section 2). A failing family is "may be slow" |
| FBNeo changes its sets between commits ("might change from time to time") | One pinned commit. The index and the diagnostics name it |
| The donation clause of the FBNeo licence | ADR 0014: the project takes no donations; a later plan removes FBNeo first |
| FBNeo's own error screen when a set passes the check and still fails | The check before launch; the session ends the game at `SHUTDOWN`, and the game list says why (section 10.1) |
| 480i on Intel and on the Pi `vc4` not proven (OP-103 point 5) | The bench session checks it. `display.vertical = fit` is the way out |
| Build time and image size: the full FBNeo build is large (a 21.4 MB zip on the libretro buildbot) | The subset builds are the later size option |
| The core's NVRAM writes are not atomic | Recorded. A patch to the core is a later item |
| A coin and a Start that overlap open the quick menu | Only a hold of 0.5 s opens it (section 13.1). Bench check with a stick |
| PAL composite runs every board 9 to 16 % slow | Open point A5 |
| The aspect of a turned source: the core's value is the upright shape | The session inverts a reported aspect, never the host's fallback (section 11); test core cases and the bench check |

## 19. Open points

Questions with a default. `docs/open-points.md` records them as OP-106.

| Label | Question | Default | Where | How to change |
| --- | --- | --- | --- | --- |
| A1 | Which XML reader for the DAT in xtask? | `roxmltree` (built) | `xtask`, root `Cargo.toml` | Another reader, or a small parser of the ClrMamePro XML subset |
| A2 | How does the image build run the index generator? | The install step of `os/package/neptron` (built) | `os/package/neptron/neptron.mk` | A step in `cargo xtask os build` before Buildroot |
| A3 | Which BIOS files must a Neo Geo BIOS zip hold, and which game ROMs are optional (`BRF_OPT`)? | For the BIOS, stop only when no file matches and list the rest on the diagnostics page; ask for every game ROM of the DAT | `neptron-library` (`arcade::check_set`) | Read the `BRF_OPT` and `BRF_BIOS` flags of FBNeo's Neo Geo driver into the index, and require the files that the selected BIOS needs |
| A4 | Where does the frontend keep the one-time notices it has shown? | `state/config/notices.toml`, atomic write (built) | `apps/neptron` | A settings key, or another state file |
| A5 | PAL composite runs arcade boards 9 to 16 % slow. Accept for 0.1? | Accept, with the speed alert, as for NTSC | `crates/neptron-modeline/src/policy/tv.rs` | The "keep game speed" choice |
| A6 | TATE: `screen_orientation` with one `vertical` choice and a direction, or two choices? | Decided by the owner on 2026-11-19: one setting of 0, 90, 180 or 270 degrees (section 15, OP-107) | `crates/neptron-settings` | The TATE ADR |

The owner decided A7 to A12 on 2026-11-16. Section 11 ("The decisions of 2026-11-16") holds the decisions.

## Appendix A: the arcade policy table

The table is in [`arcade-table.md`](arcade-table.md). `crates/neptron-modeline/tests/integration/arcade_table.rs` runs each board of Appendix B through `decide` and `choose` with `neptron_safety::validate`, with the default settings (`auto`, `exact`), and prints it. To make it again, run the command below and replace the tables in that file with the output:

```text
cargo xtask test -p neptron-modeline --no-capture arcade_table
```

The displays:

- Every preset of `data/monitors`, with a GPU that has no pixel clock floor and sends interlace, as the policy goldens do. A pixel clock floor changes the width of a mode, not its lines.
- Four profiles that `profile_from_edid` builds from the range limits of typical PC tubes (one range, GTF porches, no interlace), with the GPU's pixel clock limited to the EDID's: `edid_31` (31 to 32 kHz, 50 to 75 Hz, 40 MHz), `edid_30_54` (30 to 54 kHz, 50 to 120 Hz, 80 MHz), `edid_30_70` (30 to 70 kHz, 50 to 160 Hz, 110 MHz), `edid_30_96` (30 to 96 kHz, 50 to 160 Hz, 230 MHz). The test copies the GTF rule of `neptron-platform`, and a test holds the copy to the platform's result for a 30 to 71 kHz tube.
- Flat panels with their 60 Hz mode: `panel_720`, `panel_768`, `panel_1080` (the panel of the policy goldens, which also lists 50 Hz and 720p), `panel_1440`, `panel_2160`.
- `tv_ntsc` and `tv_pal`, on the composite output of the Raspberry Pi.

The columns:

- Rung, mode (active size and scan), total lines (a frame of both fields when interlaced), line rate and field rate of the winner.
- Rate: `exact` when the field rate is the game's rate within 5 mHz, `exact x2` at twice it; else the refresh kind and the speed change. On a panel and a TV, the speed change.
- Scale: the vertical scale, whole or a fraction for a fit. Filter: of the game picture.
- Height: the game's share of the active lines of the mode. Border: the black lines of the mode.
- Shape: the shape on the screen against the game's own shape. A CRT shows its active area on a 4:3 face, a TV shows 704 of its 720 samples as 4:3, a panel has square pixels. +77.8 % is a 3:4 picture stretched to 4:3.
- Best whole: the largest whole scale that a proposal of the plan has at the game's rate and that the validator accepts.
- Rule: `ok` when the winner follows the owner's rule, or `ok:` with an allowed reason; else the first rule it breaks (`no mode`, `soft fit`, `shape` beyond 2 %, `rate` when a range covers the game's rate or twice it, `smaller scale` when a larger whole scale keeps the rate on a CRT).

The table is a report of the policy, not a golden. The cells that follow the rule are goldens in `tests/golden/policy_arcade.toml`. The verdict counts are in section 11, "The results".

## Appendix B: the reference boards

The policy table (Appendix A) runs these boards (`BOARDS` in `arcade_table.rs`). The size is the frame as the core draws it, and the turn is the one the core asks for. Unless the source names MAME, the size is the one of the FBNeo driver entry and the rate is the one FBNeo reports: its `BurnSetRefreshRate` value cut to 0.01 Hz, or 60.00 Hz when the driver sets no rate (`nBurnFPS` starts at 6000 in `burn.cpp`). The drivers were read at `libretro/FBNeo` master on 2026-11-16, and the MAME drivers at `mamedev/mame` master on the same day.

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
