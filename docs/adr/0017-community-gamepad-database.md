# 0017: The images ship the community gamepad database under the zlib licence

- Status: accepted
- Date: 2026-11-17
- Task: none (the controller database task, after M04b)

## Context

A pad that SDL3 has no gamepad mapping for gets the generic layout (`docs/spec/input-generic-pads.md`), and some of its buttons can be in other places. The built-in database of SDL3 is fixed at the pinned SDL version. The community database `SDL_GameControllerDB` (github.com/mdqinc/SDL_GameControllerDB) has many more pads and gets corrections first. M04b did not ship it, because no licence for its entries was known.

The repository has a `LICENSE` file. It was read at the pinned commit `a69b1ac650f4faab73596c2d561edc2c7636978e` (2026-11-17). It is the zlib licence, verbatim:

```text
Copyright (C) 1997-2025 Sam Lantinga <slouken@libsdl.org>
  
This software is provided 'as-is', without any express or implied
warranty.  In no event will the authors be held liable for any damages
arising from the use of this software.

Permission is granted to anyone to use this software for any purpose,
including commercial applications, and to alter it and redistribute it
freely, subject to the following restrictions:
  
1. The origin of this software must not be misrepresented; you must not
   claim that you wrote the original software. If you use this software
   in a product, an acknowledgment in the product documentation would be
   appreciated but is not required. 
2. Altered source versions must be plainly marked as such, and must not be
   misrepresented as being the original software.
3. This notice may not be removed or altered from any source distribution.
```

The SHA-256 of that file is `97f35b302b361680ec1e891e95d2d52097bb95abff361434916d99dc1305f127`. SDL3 itself is under the same licence.

## Decision

1. The owner decided on 2026-11-17 to ship the database, so more pads get a known layout.
2. The Buildroot package `os/package/sdl-gamecontrollerdb` downloads the GitHub archive of the pinned commit. Its hash file holds the SHA-256 of the archive and of `LICENSE`. It installs `gamecontrollerdb.txt`, unchanged, as `/usr/share/sdl-gamecontrollerdb/gamecontrollerdb.txt`. The PC, QEMU aarch64 and Pi images enable it.
3. The licence text goes to the licence folder of each image through `make legal-info` (`legal/licenses/sdl-gamecontrollerdb`, and `/usr/share/licenses`). `os/board/pc/LICENCES.txt` and `os/board/rpi/LICENCES.txt` name the database and its licence.
4. The repository holds no copy of the database. Test fixtures are synthetic lines in its shape.
5. The NEPTRON mapping file (`data/input/gamecontrollerdb.txt`) keeps its own rule: an entry only with a source that can be cited. Its lines win over the community lines.

To update the pin: choose a commit of `master`, read its `LICENSE` and compare it with the text above, set `SDL_GAMECONTROLLERDB_VERSION`, put the SHA-256 of the new archive (`sdl-gamecontrollerdb-<commit>.tar.gz`, from `https://github.com/mdqinc/SDL_GameControllerDB/archive/<commit>.tar.gz`) and of `LICENSE` in the hash file, and record the new commit in the Status of this ADR. A licence that is no longer zlib stops the update: it is a new licence decision.

## Consequences

- Many more pads open with a known layout and no notice of the generic layout.
- A community line replaces the built-in line of SDL3 for the same GUID. A wrong community line changes a pad that worked before. The player corrects it with the map of the player (task M05), and NEPTRON can correct it for everyone with a line in its own file.
- The source reads some 2300 lines at each run, once, before the first frame. `docs/spec/input-generic-pads.md` gives the cost.
- The package changes the three defconfigs, so the Buildroot base of CI gets a new key, and the next build of each target on `main` is cold once.
- The zlib licence asks for nothing in the image beyond the notice, which the licence folder carries.

## Alternatives

- A copy of the file in `data/input`: no Buildroot package, but a third-party data file of some 500 kB in the repository, and a pin that is only a comment.
- A Linux-only copy, made by the build: a smaller file and a shorter load, but an altered copy (zlib rule 2) and one more build step. The parse already skips the other platforms.
- The file in `/usr/share/neptron/input`: the install step of the `neptron` package empties `/usr/share/neptron`, so a rebuild of the frontend package alone would lose it, unless that package copied it in, which ties the two packages together.
- No community database: the built-in database of SDL3 and the generic layout only, as after M04b.
