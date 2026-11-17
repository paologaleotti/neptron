# neptron-library

ROM scan, index cache, core and system metadata. The crate does not import `gamelist.xml` (OP-023).

The crate reads the folder layout of Batocera and RetroPie: `share/roms/<system>/`, with the folder names and aliases of `data/systems`. It never opens, changes or deletes a ROM file. The scan reads directory entries and their types only.

## Public API

- `Catalog`: the system and core definitions. `Catalog::builtin()` parses `data/systems` and `data/cores`, which the crate embeds at build time. `system_for_folder`, `core_for`. `CoreDef::options` holds the default core options of `data/cores` (the optional `options` table), sorted by key; the app passes them to the core at launch. `SystemDef::always_progressive` is the optional `always_progressive` key of `data/systems` (default `false`): every frame of the system is progressive. Nothing reads it yet; the session is to take it from the launch request (`docs/spec/arcade.md`, section 9).
- `scan(roms_root, catalog, arcade, cancel, progress)`: builds an `Index` and a `ScanReport`. `arcade` holds the arcade indexes, at most one per core. It reads every folder. It checks `cancel` before each directory entry and calls `progress` after each folder.
- `refresh(roms_root, catalog, arcade, previous, now, cancel, progress)`: the same result as `scan`, but it keeps the games of each folder whose stamp in `previous` has not changed, and reads only the other folders. See "Change detection".
- `changes(old, new)`: the games added, removed and changed between two indexes (a renamed file is one removed and one added), for the notice after a scan.
- `ScanWorker`: runs `scan` (`spawn`) or `refresh` (`spawn_refresh`) on a thread that it owns. `progress()` and `cancel()` use atomics and never wait. `is_finished()` tells when `join()` returns at once. Drop cancels and joins.
- `ensure_system_folders(roms_root, catalog)`: makes the empty system folders of the first boot.
- `Index` and `SystemGames`: the query side. The index also holds the folder stamps of the scan (`folders()`, `folder(path)`). `Game::size()` is the file size when the scan read the folder. An arcade game also has `parent()`, `details()` and `marks()` (`Marks`: vertical, not working, unknown set). `SystemGames::games()` leaves out the folded clones; `versions(stem)` gives the folded clones of a set, and `all()` every game. `systems()`, `system(id)`, `count(id)`, `page(id, start, len)`. Queries do no I/O and return slices of the index, so they allocate nothing.
- `arcade`: the arcade index of a core (`ArcadeIndex`): its sets from the DAT, with `arcade::encode`, `arcade::decode` and `arcade::read`. `ArcadeIndex::find(stem)` looks a zip stem up with no regard to ASCII case and allocates nothing; `bios_of(set)` follows `romof` to the BIOS set. `arcade::split_description` gives the title and the details of a DAT description. See "Arcade index".
- `cache::load`, `cache::read`, `cache::write`, `cache::encode`, `cache::decode`: the index cache.

The cache load and the scan both do file I/O, so neither runs on the frame thread.

## Owning a ScanWorker

`Drop` of a `ScanWorker` cancels the scan and joins its thread. The scan stops at its next directory entry, but an entry read on a dead or slow disk can block for a long time, and the join waits for it. So the frame thread never drops a running worker. It calls `cancel()` and hands the worker to a thread that may block, for example the thread that owns the background work, which drops or joins it there. The frame thread only calls `progress()`, `is_finished()`, and `join()` after `is_finished()` is true.

## Change detection

A scan stamps each folder that it reads with the folder's modification time, read with `stat` before the entries (`FolderStamp`). A `refresh` against an earlier index then does this:

1. It always reads `share/roms/`, so a new, removed or renamed system folder is always found.
2. It gives every known folder one `stat`. A folder whose time equals its stamp keeps its games and their sizes, unread. Its known subfolders are checked in turn, because an entry made in a subfolder does not change the time of the folder above it.
3. It reads every other folder: a new folder, a folder with another time, a folder with no stamp. A folder that is gone loses its games.

The rule is sound because a folder's time changes when an entry in it is made, removed or renamed, on ext4, tmpfs and the Linux exFAT driver. Its limits:

- A time is trusted only when it is at least `MTIME_TRUST_WINDOW_NS` (2 s) older than the scan. File systems keep times at a coarse step (exFAT 10 ms, ext4 the kernel tick, FAT 2 s), and an entry made in the same step after the read would leave the time as it was. A newer time is stamped as untrusted, and the next refresh reads that folder again. A clock set back makes every time untrusted, which costs a read and loses nothing.
- The content of a file does not change its folder's time. A refresh does not see a file that grows after the scan (a network copy that is still running). Its size stays the old one until a scan reads the folder. The list shows the game all the same; only the size is old.
- Another operating system may not change the time of an exFAT folder when it adds a file (not verified for Windows and macOS). The app therefore uses `refresh` only when a full `scan` of this power cycle reached the cache, where every change since came through the Linux driver of the device, and a full `scan` otherwise.
- Equal times are compared, not their order, so a RAM folder made again at boot gets new times and is read again.

## Where the cache lives

The cache file belongs on the state partition (ext4), for example `state/cache/library.cache`, and not on the share. The share is exFAT and open on the LAN, and exFAT does not survive power loss well. The folder must exist before `cache::write`.

## Definition files

`data/systems` and `data/cores` are compiled into the binary, so a build always reads the files of its own version, and an older build never sees a newer file. The parser rejects an unknown key, which catches a typing error at test time. If the files are ever loaded from disk, an unknown key must be ignored and reported on the diagnostics page instead, so that an older build still reads a newer file.

## Scan rules

| Case | Rule |
| --- | --- |
| System folders | A folder directly under `share/roms/` whose name equals a folder name or alias, with no regard to ASCII case (exFAT ignores case). Two folders of one system (for example `megadrive` and `genesis`) give one list. |
| Unknown folders | Not scanned. Listed in `ScanReport::unknown_folders` for the diagnostics page. |
| Nested folders | Read down to 8 levels below the system folder. Their games join the system list; the path keeps the subfolder. |
| Game files | Regular files whose extension the system accepts, with no regard to ASCII case. The name is the file name with no extension. |
| `.zip` files | One game each, listed by the file name with no extension, like any other ROM. The scan never opens the archive; the core host reads it at launch. `Game::is_zip()` tells an archive from a plain file by the extension, so the index and the cache need no extra field. |
| BIOS sets | A file whose name with no extension is a BIOS set of the system's core is not a game. The scan counts it in `ScanReport::bios_sets`. The match ignores ASCII case. With an arcade index for the core, a set with the `b` flag is a BIOS set. With none, the table of `catalog.rs` decides (`Catalog::is_bios_set`, for example `neogeo` for FinalBurn Neo); for FinalBurn Neo it holds the same 14 sets as the DAT of the pinned commit, so a missing index loses no BIOS set. |
| Arcade sets | For a system whose core has an arcade index, the scan looks up each file stem in the index, with no regard to ASCII case. The scan never opens a zip. A set gives the title, the parent, the details and the marks (vertical, not working). A stem that is not a set keeps its file name and gets the mark "unknown set". With no index the scan lists each zip by its file name, with no mark. |
| Clones | A game whose parent set is a file of the same system list is folded under it: the list leaves it out, and `SystemGames::versions` of the parent gives it ("Other versions"). A clone whose parent zip is not in the list is a game of its own. The folding is made again for each list, so it follows the files and not the cache. |
| Unknown extensions | Skipped and counted. This includes `.7z`, which NEPTRON does not read. |
| Hidden entries | A name that starts with `.` is skipped. This also skips the `._` files that macOS writes on exFAT. |
| Symbolic links | Not followed, for files and folders. A link cannot make a loop or show a game twice (RetroPie links `genesis` to `megadrive`). exFAT has no links. |
| Bad names | A name that is not UTF-8, or a path longer than 4096 bytes, is skipped and counted. |
| Duplicate names | Kept. Equal names sort by path, by bytes. A `.zip` and a plain ROM with the same base name in one folder are both listed; with lower-case extensions the plain ROM comes first, because every accepted extension sorts before `zip`. |
| Read errors | A folder that cannot be read is listed in the report, and the scan goes on. Only an unreadable `share/roms/` stops the scan. |
| Multi-disc sets | Not handled. The MVP systems are cartridge systems. |

## Sort order

Names compare by their Unicode lower-case characters, code point by code point. Equal names compare by their bytes, then by path. The order is total and the same on every machine. Leading articles stay in place ("The Lost Vikings" sorts under T). There is no locale collation and no accent folding ("Écran" sorts after "Zoo"). Digits compare as characters ("Game 10" sorts before "Game 2").

## Cache format

One binary file, all integers little-endian:

| Offset | Size | Field |
| --- | --- | --- |
| 0 | 8 | magic `NEPTRIDX` |
| 8 | 4 | format version (`CACHE_VERSION`, 3) |
| 12 | 8 | payload length in bytes |
| 20 | 4 | CRC-32 (IEEE) of the payload |
| 24 | n | payload: system count, then per system its id, its game count, and per game its name, path, size (`u64`), parent set (empty for none), details, and marks (one byte: bit 0 vertical, bit 1 not working, bit 2 unknown set). The folded clones of a system are among its games. Then the folder count, and per folder its path, a trust byte (1 or 0), the seconds (`i64`) and nanoseconds (`u32`) of its time (both 0 when the byte is 0). A count is a `u32`; a string is a `u32` byte length and UTF-8 bytes. |

Version 1 had no sizes and no folders; version 2 had no parent, details and marks. A file of an older version is discarded and rebuilt by the next scan.

`cache::load` deletes a file that has another magic or version, a wrong length or checksum, a payload that does not parse, more than `MAX_CACHE_BYTES` (16 MiB), a system that the catalog does not have, a mark bit that is not known, a game path that is empty, absolute, has an empty, `.` or `..` part, or does not start with a folder of its system, or a folder path that is not a system folder or a folder inside one, or that does not rise strictly from the one before. It returns `CacheLoad::Discarded` with the reason, and the caller scans again. A new layout gets a new version number; an old file is then discarded and rebuilt. `cache::write` removes any entry at the temporary path, makes the temporary file with `create_new` (so it never follows a link), fsyncs it, renames it and fsyncs the folder.

Every loop and allocation of the decoder has a bound: the system count is at most the number of systems in the catalog, each game count is at most the bytes left divided by 29 (the smallest valid game), the folder count at most the bytes left divided by 18 (the smallest valid folder), each string is at most 4096 bytes and the bytes left. Each system id is looked up once. A hostile file with a valid checksum therefore decodes in time linear in its size.

`MAX_CACHE_BYTES` is 16 MiB. With typical No-Intro names a game takes about 150 bytes, so the limit holds about 110,000 games, more than a complete set of every cartridge system; such a file decodes to about 30 MiB. The worst case is a crafted 16 MiB file of the smallest valid games: about 580,000 entries, less than 160 MiB for a short time at start, which fits a device with 512 MB of memory. A larger library gets no cache and is scanned at each start.

## Arcade index

`cargo xtask arcade-index` makes the index from the DAT of the pinned core (`docs/spec/arcade.md`, section 6), and the image holds it at `/usr/share/neptron/arcade/fbneo.idx`. The format is UTF-8 text, one record per line, fields split by one tab:

```text
neptron-arcade-index | 1
core | <core id> | <40-hex commit>
dat | <DAT file name> | <DAT version>
licence | <text>
set | <name> | <parent or -> | <romof or -> | <flags> | <year> | <maker> | <board> | <orientation> | <width> | <height> | <title> | <details>
rom | <name> | <size> | <crc or -> | <merge or -> | <flags>
```

(` | ` stands for one tab here.)

- The reader refuses another magic or a version above `FORMAT_VERSION` (1), with an error that names both.
- `set` lines are sorted by name in byte order, each name once and with no ASCII upper case. The `rom` lines of a set follow its `set` line in DAT order.
- Set flags, in this order, `-` for none: `b` BIOS, `v` vertical, `n` not working, `p` preliminary. `orientation` is `h` or `v`.
- ROM flags: `-`, or `d` for a ROM with no dump, whose CRC is `-`. A CRC is 8 lower-case hex digits; a size is decimal bytes.
- `-` means "none" in the parent, romof, CRC and merge fields, so the encoder refuses a name that is empty or `-`. It also refuses a field with a tab or a line break, and names the set and the field.
- The format is append-only. A reader ignores fields after the ones it knows, and skips a line whose first field it does not know. A field is never removed or moved. A change that an older reader would misread raises the version.

The index of FinalBurn Neo at the pin of `os/package/libretro-fbneo` has 8,531 sets (14 BIOS sets) and 168,288 ROMs: 7.4 MiB of text. `arcade::read` accepts at most `MAX_INDEX_BYTES` (32 MiB). The index in memory keeps two short strings for each ROM; the menu process reads it once, off the frame thread, and keeps it.

## Frame budget

A frame at 60 Hz lasts 16.6 ms. The test `ten_thousand_game_query_fits_the_frame_budget` pages through 10,000 games, 20 rows a page. It checks that every page is a view into the index, and that the fastest of 200 page queries takes at most 1 ms. It prints the measured times.

## Open points

- `gamelist.xml` import: not implemented. It needs an XML reader and tests. Add it as a new module that fills names and favourites after the scan.
- `.7z` archives are not game files. When NEPTRON reads them, add `7z` to `data/systems`.
- Sort order: no article rule, no locale collation, no natural number order. Change `compare_names` and add a cache version, because the stored order then changes.
- Symbolic links: never followed. To follow folder links, add loop detection by device and inode.
- Multi-disc sets (`.m3u`, `.cue`): not handled, because no MVP system needs them.

## Must never depend on

- Any hardware crate
- `neptron-ui`

## Rules

- `unsafe` is forbidden: the crate root carries `#![forbid(unsafe_code)]`.
- No features.
