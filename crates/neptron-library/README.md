# neptron-library

ROM scan, index cache, core and system metadata. Import of `gamelist.xml` is not in the crate yet.

The crate reads the folder layout of Batocera and RetroPie: `share/roms/<system>/`, with the folder names and aliases of `data/systems`. It never opens, changes or deletes a ROM file. The scan reads directory entries and their types only.

## Public API

- `Catalog`: the system and core definitions. `Catalog::builtin()` parses `data/systems` and `data/cores`, which the crate embeds at build time. `system_for_folder`, `core_for`. `CoreDef::options` holds the default core options of `data/cores` (the optional `options` table), sorted by key; the app passes them to the core at launch.
- `scan(roms_root, catalog, cancel, progress)`: builds an `Index` and a `ScanReport`. It checks `cancel` before each directory entry and calls `progress` after each folder.
- `ScanWorker`: runs `scan` on a thread that it owns. `progress()` and `cancel()` use atomics and never wait. `is_finished()` tells when `join()` returns at once. Drop cancels and joins.
- `ensure_system_folders(roms_root, catalog)`: makes the empty system folders of the first boot.
- `Index` and `SystemGames`: the query side. `systems()`, `system(id)`, `count(id)`, `page(id, start, len)`. Queries do no I/O and return slices of the index, so they allocate nothing.
- `cache::load`, `cache::read`, `cache::write`, `cache::encode`, `cache::decode`: the index cache.

The cache load and the scan both do file I/O, so neither runs on the frame thread.

## Owning a ScanWorker

`Drop` of a `ScanWorker` cancels the scan and joins its thread. The scan stops at its next directory entry, but an entry read on a dead or slow disk can block for a long time, and the join waits for it. So the frame thread never drops a running worker. It calls `cancel()` and hands the worker to a thread that may block, for example the thread that owns the background work, which drops or joins it there. The frame thread only calls `progress()`, `is_finished()`, and `join()` after `is_finished()` is true.

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
| 8 | 4 | format version, now 1 |
| 12 | 8 | payload length in bytes |
| 20 | 4 | CRC-32 (IEEE) of the payload |
| 24 | n | payload: system count, then per system its id, its game count, and per game its name and path. A count is a `u32`; a string is a `u32` byte length and UTF-8 bytes. |

`cache::load` deletes a file that has another magic or version, a wrong length or checksum, a payload that does not parse, more than `MAX_CACHE_BYTES` (16 MiB), a system that the catalog does not have, or a game path that is empty, absolute, has an empty, `.` or `..` part, or does not start with a folder of its system. It returns `CacheLoad::Discarded` with the reason, and the caller scans again. A new layout gets a new version number; an old file is then discarded and rebuilt. `cache::write` removes any entry at the temporary path, makes the temporary file with `create_new` (so it never follows a link), fsyncs it, renames it and fsyncs the folder.

Every loop and allocation of the decoder has a bound: the system count is at most the number of systems in the catalog, each game count is at most the bytes left divided by 12 (the smallest valid game), each string is at most 4096 bytes and the bytes left. Each system id is looked up once. A hostile file with a valid checksum therefore decodes in time linear in its size.

`MAX_CACHE_BYTES` is 16 MiB. With typical No-Intro names a game takes about 130 bytes, so the limit holds about 130,000 games, more than a complete set of every cartridge system; such a file decodes to about 30 MiB. The worst case is a crafted 16 MiB file of the smallest valid games: about 1.4 million entries, about 160 MiB for a short time at start, which fits a device with 512 MB of memory. A larger library gets no cache and is scanned at each start.

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
