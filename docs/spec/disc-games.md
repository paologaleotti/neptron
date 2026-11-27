# Disc games

This spec defines disc games in NEPTRON 0.2: the disc files in the game list, the check of the files before launch, BIOS files, the disc change in a game, and memory cards.

The owner decided the main points on 2026-11-27 (OP-119). Task S04 builds everything that does not need a real disc core, and proves it with `neptron-testcore` and synthetic files. Task S10 adds the PlayStation core and its system file. Section 12 lists the facts that S10 must settle with the real core.

A statement about libretro comes from `libretro.h` as `neptron-libretro-sys` holds it.

## 1. Scope

### In this task

- The disc formats CHD, cue (with its bin files) and m3u.
- A system file can declare disc formats and BIOS files (section 2).
- One entry in the game list for each game: a cue hides its bin files, an m3u hides the files it names, and the files of one game with "(Disc N)" in the name make one entry (section 3).
- A check of the disc files before launch (section 5).
- BIOS files: found by their content in the `bios` folder and its subfolders, checked before launch, with a notice that names the file (section 6). A page Settings > System > BIOS files (section 10).
- The disc control interface of libretro, and a row "Disc" in the quick menu to change the disc (sections 7 and 9).
- Memory cards as save data (section 8).

### Not in this task

- A real system file for a disc system. No disc core is in the images yet.
- A reader for the inside of a CHD or a bin file. The core reads the disc. NEPTRON uses the file only as a path, and reads only the text of cue and m3u files.
- A BIOS replacement inside a core (the "high-level emulation" BIOS of some cores). S10 and S11 can use one where they find it good (OP-119, point 7).
- `.iso`, `.pbp`, `.ccd`, `.cdi`, `.gdi` and other disc formats. A later task adds one with a new value in the list of section 2.
- Save states, a memory card manager, a shared memory card.

## 2. The system file

Two keys join the system format of `data/systems/README.md`. Both have a default, so `format` stays `1`.

| Key | Type | Meaning |
| --- | --- | --- |
| `discs` | list of strings, optional, default empty | The disc formats of the system: a subset of `chd`, `cue` and `m3u`. Each one must also be in `extensions`. A system with a non-empty list is a disc system. |
| `bios` | list of tables, optional, default empty | The BIOS files that the core of the system needs. |

A table of `bios` has these keys:

| Key | Type | Meaning |
| --- | --- | --- |
| `file` | string | The file name that the core opens, for example `scph5501.bin`. ASCII letters, digits, `.`, `-` and `_`, not starting with `.`. |
| `region` | string, optional | `ntsc-j`, `ntsc-u` or `pal`. A file with no region is needed by every game. |
| `size` | integer | The size of a good file, in bytes, 1 to 16 MiB. |
| `crc32` | list of strings | The CRC-32 (IEEE) of each good dump, 8 lower-case hex digits. At least one. |

Example, for a test system (not a real one):

```toml
format = 1
folder = "testdisc"
aliases = []
name = "Test Disc"
extensions = ["chd", "cue", "m3u"]
discs = ["chd", "cue", "m3u"]

[[bios]]
file = "test-us.bin"
region = "ntsc-u"
size = 16
crc32 = ["0123abcd"]
```

The parser refuses a disc format that is not in `extensions`, an unknown disc format, a region that is not one of the three, a size of 0 or above 16 MiB, a checksum that is not 8 lower-case hex digits, an empty `crc32` list, and two `bios` tables with the same file name (ASCII case ignored).

Why CRC-32 and the size: the BIOS lists of RetroArch (`System.dat` of `libretro-database`) and of No-Intro give a CRC-32 for each file, and `neptron-library` has CRC-32 already. The check finds a wrong file, not an attack: nothing gains from a forged BIOS. A file must match the size and one checksum.

## 3. The scan

### 3.1 Which files are games

The scan rules of `crates/neptron-library/README.md` stay. In a disc system:

| File | Rule |
| --- | --- |
| `.chd` | A game: one disc. |
| `.cue` | A game: one disc. The scan does not open it. |
| `.bin` | Not a game, because `bin` is not an extension of a disc system. It counts as an ignored file. So the bin files of a cue never show. |
| `.m3u` | A game: a list of discs. The scan reads the file (section 3.2). |
| Other | As before: an extension that the system does not accept is skipped. |

### 3.2 The m3u file

The scan reads the text of each `.m3u` file of a disc system, at most `M3U_MAX_BYTES` (64 KiB). The rules:

- One line is one disc. A line ends with LF or CR LF. Spaces at the start and the end of a line are removed.
- An empty line, and a line that starts with `#`, is not a disc. (The `#EXTM3U` lines of other players and the `#COVER` lines are comments.)
- A path is relative to the folder of the m3u file. `/` and `\` both separate the parts.
- At most `M3U_MAX_DISCS` (64) discs. The scan ignores the lines after that.
- The order of the lines is the order of the discs.
- A file of more than 64 KiB, or a file that is not UTF-8, gives a list with no discs. The scan counts it in the report. The game still shows, and its check at launch says what is wrong (section 5).

The scan keeps the paths of the discs with the game, as paths below `share/roms/`. A line with an empty part, a `.` or `..` part, or an absolute path does not give such a path. Such a line hides nothing, and the check at launch looks for it as the core does, relative to the folder of the m3u file.

A line that names an `.m3u` file (in any case) is an error of its list. It hides nothing, so a list that names another list, or two lists that name each other, or a list that names itself, never makes a game go from the list. Such a list shows, and its launch stops with the notice of a list that names no disc (section 11).

An m3u file can change with no change of the time of its folder. Then the list keeps the old discs until a scan reads the folder again (a rescan, or any change in the folder).

### 3.3 Files that an m3u names

A game file whose path is a disc of an m3u file of the same system does not show. The share is exFAT, which ignores case, so the paths compare with no regard to case (Unicode lower case). On a file system that keeps case (ext4, the test folders), every file whose path equals a line with no regard to case is hidden. The m3u file shows in its place. This is true in every folder of the system, so the discs can be in a subfolder of the folder of the m3u file (for example `Game.m3u` and `discs/Game (Disc 1).chd`). A file that two m3u files name does not show. Both m3u files show.

A folder whose name starts with `.` is not read (the scan rule for hidden entries), so discs in a `.hidden` folder never show. The m3u file then names them, and the core reads them from there.

### 3.4 Files grouped by "(Disc N)"

The files of a disc system that no m3u names are grouped by name:

- A disc tag is a group in brackets in the file name with no extension: `(Disc N)` or `(Disc N of M)`. `Disc` ignores ASCII case. `N` and `M` are 1 to 99, in decimal digits, with no sign. `Disk` is not a tag.
- When a name has more than one disc tag, the first one counts.
- The group name is the name with the tag removed, and with one space before the tag removed. "Final Fantasy VII (USA) (Disc 1)" gives "Final Fantasy VII (USA)". "Game (Disc 2) (Europe)" gives "Game (Europe)".
- The files of one group are in the same folder, have the same group name (byte for byte) and the same extension (ASCII case ignored). So the cue files and the CHD files of one game make two entries.
- A group needs at least two files with different numbers. A file with a tag and no other file of its group is a game of its own, under its full name.
- Two files of one group with the same number: the first in the byte order of the path joins the group. The other is a game of its own, under its full name.
- The order of the discs is the order of the numbers. A gap is allowed: "Disc 1" and "Disc 3" make one game of two discs.
- The entry of a group shows the group name. Its path is the path of its first disc, so its game id, its relaunch reference and its settings key come from that file.

The grouping is made for each list, from the files, as the folding of arcade clones is. So it follows the files and not the cache.

### 3.5 The cache

The cache gets version 4: each game also holds its list of discs (a `u32` count and the paths as strings). The scan writes the discs of an m3u file. A group entry is not written: the cache holds its files, and the list makes the group again. An older cache is discarded and the scan writes version 4.

## 4. Region

The region of a game decides which BIOS it gets (section 6.4). NEPTRON reads it from the file name, because it does not read inside a disc (open question Q1).

- The region comes from the first group in brackets of the name whose first word, before a `,`, is a known region word.
- `ntsc-j`: `Japan`, `J`, `NTSC-J`.
- `ntsc-u`: `USA`, `US`, `U`, `Canada`, `NTSC-U`.
- `pal`: `Europe`, `E`, `PAL`, `UK`, `Germany`, `France`, `Italy`, `Spain`, `Australia`, `Netherlands`, `Sweden`.
- The words ignore ASCII case.
- No known word: the region is not known.

"Game (USA, Europe)" is `ntsc-u`. "Game (Japan) (En)" is `ntsc-j`.

A PAL game is a 50 Hz source for the picture policy (OP-119, point 11). NEPTRON needs no rule for this: the core gives the frame rate of the region of the disc in `retro_system_av_info`, and the policy takes it as it takes any source rate.

## 5. The check of the disc files

The game run checks the files of a disc game before it opens the core. The check reads only text files, at most 64 KiB each:

| Game | Check |
| --- | --- |
| `.chd` | The file exists. |
| `.cue` | Every `FILE` line of the cue names a file that exists, relative to the folder of the cue. A `FILE` name is in double quotes, or is the first word after `FILE`. A cue of more than 64 KiB, or with no `FILE` line, is not checked: the core decides. |
| `.m3u` | The list has at least one disc, no line names an `.m3u` file, and every disc exists. A disc that is a cue gets the cue check. |
| A group | Every disc exists, and a cue disc gets the cue check. |
| A file of a group that is not its first disc | One disc, with the number of its tag: the list shows it alone (an m3u file names the others), so the core and the Disc page hold that disc only. |

Each part of a path in a cue or an m3u file matches a name of its folder with no regard to case, as on exFAT. The exact name comes first; of two names that differ only in case (possible on ext4, not on exFAT), the first in byte order.

The first file that fails stops the launch, with a notice (section 11). The check never changes a file.

## 6. BIOS files

### 6.1 Where NEPTRON looks

NEPTRON looks for BIOS files in `share/bios` and in its subfolders, down to `BIOS_MAX_DEPTH` (3) levels below it. It does not follow links, and it skips names that start with `.`. It reads at most `BIOS_MAX_ENTRIES` (4096) entries. It never writes into `share/bios`.

### 6.2 How a file is checked

A file is a good copy of a BIOS entry when its size equals `size` and its CRC-32 is in `crc32`. The name does not count (OP-119, point 6). NEPTRON reads a file only when its size equals the size of some entry, so a folder of other files costs only the directory reads.

The status of each BIOS entry of each system:

| Status | Rule |
| --- | --- |
| Found | A file in the folder or a subfolder is a good copy. The first one in this order counts: `share/bios` first, then the subfolders level by level, and in each folder the names in byte order. |
| Wrong | No file is a good copy, and a file with the name of the entry (ASCII case ignored) is in the folder or a subfolder. |
| Missing | No file is a good copy, and no file has the name of the entry. |

### 6.3 When NEPTRON checks

- The game list run makes the status of every entry on the scan thread, after each scan, for the BIOS files page.
- The launch of a game of a system with BIOS entries makes the status of that system's entries only, before it opens the core, for the check of section 6.4. This costs a few directory reads and one read of each file of one of its sizes. A game of a system with no BIOS entries reads nothing.
- With no status held (a game run, or before the first scan of the list ends), the BIOS files page asks for one when it opens. It is made on a thread, and the page shows `Reading...` until it is there.

### 6.4 The check before launch

For a game of a system with BIOS entries:

1. Each entry with no region must be found.
2. When the system has entries with a region, one of them must be found: the first found entry of the region of the game; else the first found entry of any region, in the order of the system file (OP-119, point 11).

When a rule fails, the game does not start, and the notice names one file:

- Rule 1: the first entry that is not found.
- Rule 2: the first entry of the region of the game; when the region is not known, or the system has no entry of that region, the first entry with a region.

Its status gives the text: missing or wrong (section 11).

### 6.5 The system folder of the core

The core asks for its system folder (`GET_SYSTEM_DIRECTORY`) and opens the BIOS by the name that it knows. A good copy can have another name, or be in a subfolder. So for a system with BIOS entries, the core does not get `share/bios`. It gets `state/system/<system>/`, which NEPTRON fills before the launch:

- For each found entry, a copy of the file with the name of the entry (temp file, fsync, rename). NEPTRON checks the size and the CRC-32 of the bytes again before the write, and writes nothing when the copy in the folder is equal (size and CRC-32), so a launch does not write each time.
- A file with the name of an entry that is not found now is removed: it is an old copy of NEPTRON. A file of any other name stays: a core can keep its own files there (NVRAM, a memory card).

A copy, not a link: a core that writes into its system folder must never change the user's file.

A system with no BIOS entries keeps `share/bios` as its system folder, as before.

## 7. The core host

### 7.1 The disc control interface

| Call | Answer |
| --- | --- |
| `GET_DISK_CONTROL_INTERFACE_VERSION` | `true`, writes 1: the host takes the extended interface. |
| `SET_DISK_CONTROL_INTERFACE` | `true`. The host keeps the seven callbacks. |
| `SET_DISK_CONTROL_EXT_INTERFACE` | `true`. The host keeps the ten callbacks. It replaces an earlier interface of either kind. |

A null struct pointer is refused (`false`). The host keeps a callback that is null as "none", and a call that needs it fails with an error.

The host gives safe calls: the number of discs, the index of the disc in the drive, the tray state, open and close the tray, set the index, and add a disc by its path (`add_image_index`, then `replace_image_index` with a `retro_game_info` that holds only the path). The host keeps each path of an added disc until the host drops, so a core can keep the pointer. The extended callbacks `set_initial_image`, `get_image_path` and `get_image_label` are kept and not used in this task.

### 7.2 The folders

| Call | Answer |
| --- | --- |
| `GET_SYSTEM_DIRECTORY` | `HostConfig::system_dir`: `share/bios`, or `state/system/<system>/` for a system with BIOS entries (section 6.5). |
| `GET_SAVE_DIRECTORY` | `HostConfig::save_dir`. For a disc system it is the save folder of the game, which follows the folders of the game below its system folder: `state/saves/<system>/<folders>/`. For every other system it stays `state/saves/<system>/`. |

## 8. Memory cards

`libretro.h` has no memory type for a memory card. A core keeps a card in one of these ways:

| Case | What the core does | How NEPTRON maps it to one card for each game |
| --- | --- | --- |
| A | It gives the card as its save RAM (`RETRO_MEMORY_SAVE_RAM`). | The save RAM of the game is the card. NEPTRON keeps it as any save RAM: `<save folder>/<save name>.srm`, temp file, fsync, rename, one `.bak` per session, flushed on a timer, at the quick menu and at exit. Nothing new is needed. |
| B | It reads and writes card files itself, in the save folder that it asks for, under the name of the content or of the disc. | The core gets the save folder of the game (section 7.2), so the card files are with the other saves and follow the folders of the game. NEPTRON does not write these files, so the temp file, fsync, rename rule and the `.bak` are not NEPTRON's. |
| C | It keeps one card for all games in its system folder. | Not one card for each game. NEPTRON does not use this case. |

The decision: case A for each disc core. S10 sets the core options that put the first card in the save RAM (open question Q2, and section 12). Cases B and C are recorded so that S10 can see which case a core takes.

The save name of a disc game is its group name: the name with no extension and no disc tag (section 3.4). For a group it is the name of the entry; for an m3u file the name of the m3u file. So all the discs of a game share one card, and a game that a user later puts in an m3u file keeps it when the m3u file has the group name. The settings file of the game takes the same name. The card is made automatically: a game with no save file starts with an empty save RAM, and the first flush writes the file.

## 9. The session

- The launch gives the core the first disc. For a group of files (section 3.4), it then adds each other disc in order with the disc control interface. An m3u file goes to the core as it is: the core reads the list.
- A core that has no disc control interface plays the first disc only, and the quick menu has no "Disc" row.
- To change the disc, the session opens the tray, sets the new index, and runs `DISC_TRAY_OPEN_FRAMES` (60) core frames before it closes the tray. A game that reads the lid state sees it open. The frames start when the player resumes the game. The count is open for S10 (section 12).
- A change while the tray is open from an earlier change sets the new index and starts the count again.

## 10. The app

### 10.1 The Disc row of the quick menu

The quick menu gets a row "Disc" after "Video" when the game has a disc control interface with two discs or more. It opens the page "Disc":

- One row for each disc: "Disc 1", "Disc 2", ... A group of files takes the numbers of its tags ("Disc 1", "Disc 3"); every other game takes the position in the list of the core.
- The row of the disc in the drive is disabled and shows "In the drive".
- A row of another disc changes the disc (section 9). The page then shows the new disc in the drive. The player resumes the game from the quick menu.

### 10.2 The BIOS files page

Settings > System gets a row "BIOS files", before "Reboot". It opens the page "BIOS files". The row is enabled in a game too, because the page changes nothing.

- One row for each BIOS entry of each system that has entries, in catalog order, then file order.
- The label is the system name, with the region in brackets for an entry with a region: "PlayStation (USA)". `ntsc-j` is "Japan", `ntsc-u` "USA", `pal` "Europe".
- The value is "Found: <file>", "Missing: <file>" or "Wrong file: <file>".
- With no system that needs a BIOS, one row: "BIOS files" with "None needed".
- Before the first status of the run: one row "BIOS files" with "Reading...".

## 11. Texts

Every text that the player sees. `<game>` is the name in the list, `<system>` the system name, `<file>` a file name.

| Where | Text |
| --- | --- |
| Notice, BIOS missing | `<game> did not start: the <system> BIOS is missing. Put <file> in the bios folder.` |
| Notice, BIOS wrong | `<game> did not start: <file> is not a good <system> BIOS. Put a good copy of <file> in the bios folder.` |
| Notice, disc file missing (a cue names it, an m3u names it, or a disc of a group) | `<game> did not start: the disc file <file> is missing. Put it beside <list file>.` `<list file>` is the cue or m3u file that names it. For a disc of a group it is the file name of the first disc. |
| Notice, empty or unreadable m3u | `<game> did not start: <list file> names no disc file.` |
| Notice, the system folder of the core cannot be made | `<game> did not start: the BIOS files could not be prepared.` The reason goes to the log. |
| Quick menu row | `Disc` |
| Disc page title | `Disc` |
| Disc page rows | `Disc <n>`; reason of the disc in the drive: `In the drive` |
| System panel row | `BIOS files` |
| BIOS page title | `BIOS files` |
| BIOS page values | `Found: <file>`, `Missing: <file>`, `Wrong file: <file>` |
| BIOS page, no BIOS needed | `BIOS files`: `None needed` |
| BIOS page, before the first status | `BIOS files`: `Reading...` |
| Region words | `Japan`, `USA`, `Europe` |

A notice cuts no text: the notice dialog wraps it. A file name in a notice is the name only, with no folder.

## 12. Facts for S10

S10 settles these with the real PlayStation core, and records them in its spec:

1. The system file: folder `psx` and its aliases, the extensions, the core, and the BIOS entries with their sizes and CRC-32 values from a pinned `System.dat`.
2. The memory card case of the core (section 8), and the core options that give case A for the first card.
3. Whether the core needs a file in its system folder other than the BIOS entries (section 6.5).
4. `DISC_TRAY_OPEN_FRAMES`: the frames that a real game needs to see the lid open.
5. Whether the core takes discs added with `replace_image_index` after the load, for a group with no m3u file (section 9). If not, the fallback of Q3.
6. Whether the core reports 50 Hz for a PAL disc, so the picture policy gets it (section 4).
7. Which region the core takes when more than one BIOS is in its system folder (section 6.5 copies every found entry).

## Questions for the owner

These questions are not answered by the eleven decisions of OP-119. Each one has a default, which this task builds.

**Q1. The region of a game when the name has none.** NEPTRON reads the region from the name. A CHD or a bin file holds the true region, but NEPTRON would need a CHD reader (a new dependency) to read it.
- (a) The name only; an unknown region takes any found BIOS. Recommended: Redump names always carry the region, and decision 11 already takes any region when the right one is missing.
- (b) Add a CHD and bin reader to read the region from the disc (a new dependency, task of its own).
Default: (a).

**Q2. Which memory card case.** Case A (the card as save RAM) gives the backup and the safe write of save RAM. Case B gives a card file that other players read, but NEPTRON cannot back it up.
- (a) Case A for every disc core. Recommended.
- (b) Case B where a core offers it.
Default: (a).

**Q3. A group of files with no m3u file, when the core does not take discs added after the load.**
- (a) Add the discs with the disc control interface after the load. Recommended: no file is written, and the libretro API has the call for this.
- (b) Write an m3u file in `state/` for the group and give it to the core.
Default: (a). S10 checks it (section 12, fact 5).

**Q4. A lone file with a disc tag.** "Game (Disc 1).chd" with no other disc of its group.
- (a) It shows under its full name, "Game (Disc 1)". Recommended: the player sees that the other discs are missing.
- (b) It shows under the group name, "Game".
Default: (a).

**Q5. A missing disc of an m3u list.**
- (a) The game does not start, and the notice names the missing file. Recommended: the player learns it at once, not at the change of disc hours later.
- (b) The game starts with the discs that exist.
Default: (a).
