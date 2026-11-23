# data/input

`gamecontrollerdb.txt`: the NEPTRON gamepad mappings, in the `gamecontrollerdb` format of SDL3. The image installs it as `/usr/share/neptron/input/gamecontrollerdb.txt`. The gamepad source reads it when it opens and gives each good line to SDL3, where it replaces the built-in mapping of the same GUID. `docs/spec/input-generic-pads.md` has the order of the mapping sources.

- Add an entry only with a source that can be cited, in a comment above the entry.
- The community `SDL_GameControllerDB` is not copied here. The images ship it, unchanged and under the zlib licence, from its own Buildroot package (`os/package/sdl-gamecontrollerdb`, ADR 0017), and the lines of this file win over its lines.
- A damaged line gives a line in the log and is skipped. The other lines still load.
