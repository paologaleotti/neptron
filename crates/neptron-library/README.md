# neptron-library

ROM scan, index cache, core and system metadata. The crate reads the Batocera and RetroPie layout `share/roms/<system>/`, with the folder names and aliases of `data/systems`. It never opens, changes or deletes a ROM file: the scan reads directory entries and their types only. It does not import `gamelist.xml` (OP-023).

## Must never depend on

- Any hardware crate
- `neptron-ui`

## Rules

- `#![forbid(unsafe_code)]` at the crate root. No features.
- The cache load and the scan do file I/O, so neither runs on the frame thread. Index queries do no I/O and return slices of the index, so they allocate nothing.

## Public API

- `Catalog::builtin()` parses `data/systems` and `data/cores`, embedded at build time. `system_for_folder`, `core_for`, `is_bios_set`. `CoreDef::options`: the default core options of `data/cores`, sorted by key, which the app passes to the core at launch. `CoreDef::internal_resolution` (`InternalResolution`: the option key, the native lines, the value for each scale `k`): the option of a GL core that the session sets to the `k` of `docs/spec/hw-render.md`, section 4. `SystemDef::always_progressive` (default `false`): every frame of the system is progressive. The app passes it in `GameLaunch::always_progressive` (`docs/spec/arcade.md`, section 9).
- `scan(roms_root, catalog, arcade, cancel, progress)` builds an `Index` and a `ScanReport`. `arcade` holds at most one arcade index per core. It checks `cancel` before each directory entry and calls `progress` after each folder.
- `refresh(roms_root, catalog, arcade, previous, now, cancel, progress)` gives the same result as `scan`, but reads only the folders whose stamp changed ("Change detection").
- `changes(old, new)`: the games added, removed and changed (a rename is one removed and one added), for the notice after a scan.
- `ScanWorker` runs `scan` (`spawn`) or `refresh` (`spawn_refresh`) on a thread it owns. `progress()` and `cancel()` use atomics and never wait. `is_finished()` tells when `join()` returns at once. Drop cancels and joins.
- `ensure_system_folders(roms_root, catalog)` makes the empty system folders of the first boot.
- `Index`, `SystemGames`, `Game`, `Marks`: the query side. `Index::folders()` and `folder(path)` give the folder stamps. `Game::size()` is the size when the scan read the folder. An arcade game has `parent()`, `details()` and `marks()` (vertical, not working, unknown set). `SystemGames::games()` leaves out folded clones, `versions(stem)` gives the clones of a set, `all()` every game. `systems()`, `system(id)`, `count(id)`, `page(id, start, len)`.
- `arcade`: `ArcadeIndex` (`find(stem)` ignores ASCII case and allocates nothing, `bios_of(set)` follows `romof`), `encode`, `decode`, `read`, `split_description`, `check_set`, `wanted`, `wanted_version`, `disk_paths`.
- `cache`: `load`, `read`, `write`, `encode`, `decode`.

## Owning a ScanWorker

Drop of a `ScanWorker` cancels the scan and joins its thread. An entry read on a dead or slow disk can block for a long time, and the join waits for it. So the frame thread never drops a running worker. It calls `cancel()` and hands the worker to a thread that may block (for example the owner of the background work). The frame thread only calls `progress()`, `is_finished()`, and `join()` after `is_finished()` is true.

## Change detection

A scan stamps each folder it reads with the folder's modification time, read with `stat` before the entries (`FolderStamp`). A `refresh` against an earlier index:

1. Always reads `share/roms/`, so a new, removed or renamed system folder is found.
2. Gives every known folder one `stat`. A folder whose time equals its stamp keeps its games and sizes, unread. Its known subfolders are checked in turn, because an entry made in a subfolder does not change the time of the folder above it.
3. Reads every other folder: new, with another time, or with no stamp. A folder that is gone loses its games.

A folder's time changes when an entry in it is made, removed or renamed, on ext4, tmpfs and the Linux exFAT driver. Limits:

- A time is trusted only when it is at least `MTIME_TRUST_WINDOW_NS` (2 s) older than the scan. File systems keep times at a coarse step (exFAT 10 ms, ext4 the kernel tick, FAT 2 s), so an entry made in the same step after the read leaves the time as it was. A newer time is stamped as untrusted, and the next refresh reads that folder again. A clock set back makes every time untrusted, which costs a read and loses nothing.
- The content of a file does not change its folder's time. A file that grows after the scan keeps its old size until a scan reads the folder. The game still shows.
- Another operating system may not change the time of an exFAT folder when it adds a file (not verified for Windows and macOS). So the app uses `refresh` only when a full `scan` of this power cycle reached the cache, and a full `scan` otherwise.
- Times are compared for equality, not order, so a RAM folder made again at boot is read again.

## Where the cache lives

On the state partition (ext4), for example `state/cache/library.cache`, not on the share: the share is exFAT, open on the LAN, and does not survive power loss well. The folder must exist before `cache::write`.

## Definition files

`data/systems` and `data/cores` are compiled into the binary, so a build always reads the files of its own version. The parser rejects an unknown key, which catches a typing error at test time. If the files are ever loaded from disk, an unknown key must be ignored and reported on the diagnostics page instead, so an older build still reads a newer file.

## Scan rules

| Case | Rule |
| --- | --- |
| System folders | A folder directly under `share/roms/` whose name equals a folder name or alias, ignoring ASCII case (exFAT ignores case). Two folders of one system (for example `megadrive` and `genesis`) give one list. |
| Unknown folders | Not scanned. Listed in `ScanReport::unknown_folders`. |
| Nested folders | Read down to `MAX_FOLDER_DEPTH` (8) levels below the system folder. Their games join the system list, and the path keeps the subfolder. |
| Game files | Regular files whose extension the system accepts, ignoring ASCII case. The name is the file name with no extension. |
| `.zip` files | One game each, like any other ROM. The scan never opens the archive; the core host reads it at launch. `Game::is_zip()` tells an archive by its extension. |
| BIOS sets | A file whose stem is a BIOS set of the system's core is not a game, and is counted in `ScanReport::bios_sets`. The match ignores ASCII case. With an arcade index for the core, a set with the `b` flag is a BIOS set. With none, `Catalog::is_bios_set` decides (for example `neogeo` for FinalBurn Neo). For FinalBurn Neo it holds the same 14 sets as the pinned DAT, so a missing index loses no BIOS set. |
| Arcade sets | For a core with an arcade index, the scan looks up each stem in the index, ignoring ASCII case, and never opens a zip. A set gives the title, parent, details and marks. A stem that is not a set keeps its file name and gets the mark "unknown set". With no index, each zip shows by its file name with no mark. |
| Clones | A game whose parent set is a file of the same system list is folded under it: the list leaves it out, and `SystemGames::versions` of the parent gives it ("Other versions"). A clone with no parent zip in the list is a game of its own. The folding is made for each list, so it follows the files and not the cache. |
| Unknown extensions | Skipped and counted. This includes `.7z`. |
| Hidden entries | A name that starts with `.` is skipped, which also skips the macOS `._` files on exFAT. |
| Symbolic links | Not followed, for files and folders. So a link cannot make a loop or show a game twice (RetroPie links `genesis` to `megadrive`). |
| Bad names | A name that is not UTF-8, or a path longer than `MAX_TEXT_BYTES` (4096 bytes), is skipped and counted. |
| Duplicate names | Kept. Equal names sort by path, by bytes. A `.zip` and a plain ROM with the same stem in one folder are both listed; with lower-case extensions the plain ROM comes first, because every accepted extension sorts before `zip`. |
| Read errors | A folder that cannot be read is listed in the report, and the scan goes on. Only an unreadable `share/roms/` stops the scan. |
| Multi-disc sets | Not handled. The MVP systems are cartridge systems. |

## Sort order

Names compare by their Unicode lower-case characters, code point by code point. Equal names compare by their bytes, then by path. The order is total and the same on every machine. Leading articles stay in place ("The Lost Vikings" sorts under T). No locale collation and no accent folding ("Écran" sorts after "Zoo"). Digits compare as characters ("Game 10" sorts before "Game 2").

## Cache format

One binary file, all integers little-endian:

| Offset | Size | Field |
| --- | --- | --- |
| 0 | 8 | magic `NEPTRIDX` |
| 8 | 4 | format version (`CACHE_VERSION`, 3) |
| 12 | 8 | payload length in bytes |
| 20 | 4 | CRC-32 (IEEE) of the payload |
| 24 | n | payload: system count, then per system its id, its game count, and per game its name, path, size (`u64`), parent set (empty for none), details, and marks (one byte: bit 0 vertical, bit 1 not working, bit 2 unknown set). The folded clones of a system are among its games. Then the folder count, and per folder its path, a trust byte (1 or 0), the seconds (`i64`) and nanoseconds (`u32`) of its time (both 0 when the byte is 0). A count is a `u32`; a string is a `u32` byte length and UTF-8 bytes. |

`cache::load` deletes the file and returns `CacheLoad::Discarded` with the reason, and the caller scans again, when the file has: another magic or version, a wrong length or checksum, a payload that does not parse, more than `MAX_CACHE_BYTES` (16 MiB), a system the catalog does not have, an unknown mark bit, a game path that is empty, absolute, has an empty, `.` or `..` part, or does not start with a folder of its system, or a folder path that is not a system folder or inside one, or that does not rise strictly from the one before. A new layout gets a new version, so an older file is rebuilt.

`cache::write` removes any entry at the temporary path, makes the temporary file with `create_new` (so it never follows a link), fsyncs it, renames it and fsyncs the folder.

Every loop and allocation of the decoder has a bound: the system count is at most the catalog's system count, each game count at most the bytes left divided by 29 (the smallest valid game), the folder count at most the bytes left divided by 18 (the smallest valid folder), each string at most 4096 bytes and the bytes left. Each system id is looked up once. So a hostile file with a valid checksum decodes in time linear in its size.

A No-Intro game takes about 150 bytes, so 16 MiB holds about 110,000 games and decodes to about 30 MiB. A crafted 16 MiB file of the smallest valid games gives about 580,000 entries, less than 160 MiB for a short time at start, which fits a device with 512 MB. A larger library gets no cache and is scanned at each start.

## Arcade index

`cargo xtask arcade-index` makes the index from the DAT of the pinned core. The image holds it at `/usr/share/neptron/arcade/fbneo.idx`. `docs/spec/arcade.md`, section 6, "The file format", is the format. In code: the reader refuses another magic or a version above `FORMAT_VERSION` (1). The encoder refuses a `disk` SHA-1 that is not 40 lower-case hex digits or `-` (`IndexError::DiskSha1`), a name that is empty or `-`, and a field with a tab or a line break, and names the set and the field. `SetRecord::disks` holds the `disk` lines. `arcade::read` accepts at most `MAX_INDEX_BYTES` (32 MiB). The index in memory keeps two short strings for each ROM. The menu process reads it once, off the frame thread, and keeps it.

### The set check

`arcade::check_set(index, stem, found)` is pure and implements `docs/spec/arcade.md`, section 10. `arcade::wanted` gives the zip names the core opens (the set, its parent, its BIOS through `romof`). The caller lists the entries of each zip it finds (`FoundZips`, in the order the core searches the places) and the disk files present (`FoundZips::disks`, at the paths of `arcade::disk_paths`). The check returns a `Verdict`, by the rules in this order:

1. `UnknownSet` (message 1).
2. `MissingBios` (message 2).
3. `Unreadable` (message 6).
4. `MissingParent` (message 3).
5. `BiosMismatch` (message 5). When some BIOS ROMs match, the set passes, and `Verdict::details` lists the others (open point A3 of the spec).
6. `Roms` (message 4), with every ROM in `details`. The first zip that holds the CRC gives the ROM, as in the core. No diagnostics page exists yet (OP-108), so message 4 says that the log lists the ROMs, and the app writes each line of `details` to the log.
7. `MissingDisk` (message 8). `disk_paths` gives the FinalBurn Neo order: for `.chd` then `.img` (`DISK_EXTENSIONS`), the folder of the set, then of its parent.

`Verdict::message(core)` gives the text for the player. Message 4 names the first `NAMED_ROMS_MAX` (2) wrong ROMs, each cut to `NAMED_ROM_CHARS_MAX` (24) characters, and counts the rest, so the longest text fits a dialog on the 320x240 canvas. When more than `WRONG_SET_PERCENT` (50) percent of the checked ROMs are wrong, `Verdict::wrong_set` is true and the text says the set is for another emulator or version. Messages 4 and 5 name `arcade::wanted_version`: the DAT version and the short commit of the index. `Verdict::NoIndex` (message 7) does not stop the launch by itself.

## Open points

- `gamelist.xml` import (OP-023): not implemented. It needs an XML reader. Add it as a new module that fills names and favourites after the scan.
- `.7z` archives are not game files. When NEPTRON reads them, add `7z` to `data/systems`.
- Sort order: no article rule, no locale collation, no natural number order. To change it, change `compare_names` and raise the cache version, because the stored order changes.
- Symbolic links: to follow folder links, add loop detection by device and inode.
- Multi-disc sets (`.m3u`, `.cue`): not handled, because no MVP system needs them.

## Tests

`cargo xtask test -p neptron-library`. The test `ten_thousand_game_query_fits_the_frame_budget` pages through 10,000 games, 20 rows a page. It checks that every page is a view into the index, and that the fastest of 200 page queries takes at most 1 ms of the 16.6 ms frame at 60 Hz. It prints the measured times.
