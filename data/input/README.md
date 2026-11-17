# data/input

`gamecontrollerdb.txt`: the NEPTRON gamepad mappings, in the `gamecontrollerdb` format of SDL3. The image installs it as `/usr/share/neptron/input/gamecontrollerdb.txt`. The gamepad source reads it when it opens and gives each good line to SDL3, where it replaces the built-in mapping of the same GUID. `docs/spec/input-generic-pads.md` has the order of the mapping sources.

- Add an entry only with a source that can be cited, in a comment above the entry.
- The community `SDL_GameControllerDB` is not copied here: its entries carry no licence that NEPTRON can cite.
- A damaged line gives a line in the log and is skipped. The other lines still load.
