# Every USB gamepad: the mapping file, the generic layout and the input notices

This spec settles task M04b. It decides where the map of a pad comes from, how a joystick that SDL3 has no gamepad mapping for is read, which input notices reach the player, and what task M05 (remapping) takes over. The port rules of `docs/spec/input-players.md` do not change.

## Facts that drive the design

- `SdlGamepads` opens a joystick only when SDL3 has a gamepad mapping for it (`is_gamepad`). Any other joystick is ignored with an `InputNotice`, and the app never reads the notices. So an unknown pad does nothing, and nobody sees why.
- The kernel exposes every HID gamepad and joystick (`hid-generic`) as an event node. SDL3 lists it as a joystick with a number of buttons, hats and axes, whether or not it has a gamepad mapping.
- SDL3 reads the state of a plain joystick with the same `SDL_UpdateGamepads` call that the source makes in each poll (it updates every joystick). No event pump is needed.
- `SDL_AddGamepadMapping` adds one line of the `gamecontrollerdb` format. A line for a GUID that SDL3 knows replaces its built-in line. `SDL_AddGamepadMappingsFromFile` adds a whole file, but it returns only a count: it cannot say which line is damaged.
- In SDL 3.4 (`SDL_gamepad.c`, `SDL_PrivateAddMappingForGUID`) every mapping has a priority: the built-in lines have the lowest, a line from `SDL_AddGamepadMapping` the next. A line replaces the line of the same GUID when its priority is the same or higher. So an added line always replaces a built-in line, and of two added lines for one GUID, the later call wins.
- The community `SDL_GameControllerDB` (github.com/mdqinc/SDL_GameControllerDB) has a `LICENSE` file with the zlib licence. NEPTRON ships it in every image (ADR 0017).
- The Mad Catz FightStick TE (`0738:8838`) is in the built-in database of SDL3. Its fault is elsewhere: it enumerates and sends neutral reports, and works only behind a USB hub. This task adds no entry for it, and the silent pad notice covers that case.

## The sources of a map

A **map** is what turns the raw input of one pad into RetroPad controls. Its kind is **known** (the standard layout of SDL3, from a gamepad mapping) or **generic** (the layout by position below). The map of a pad comes from the first source that has one, in this order:

1. **The map of the player**, from the settings, keyed by the identity of the controller (`DeviceIdentity`, the hash of the SDL GUID). Task M05 adds it (`docs/spec/input-remapping.md`): `PlayerMaps`, the file `config/input/pads.toml`, and `PadMap::Player`, which reads raw joystick indices for a gamepad too.
2. **The NEPTRON mapping file**, `data/input/gamecontrollerdb.txt` in the repository, `/usr/share/neptron/input/gamecontrollerdb.txt` in the image. Its lines go to SDL3 when the source opens, so they replace a built-in line for the same GUID.
3. **The community database**, `SDL_GameControllerDB`, `/usr/share/sdl-gamecontrollerdb/gamecontrollerdb.txt` in the image (section below).
4. **The built-in database of SDL3.**
5. **The generic layout**, for a joystick with no gamepad mapping.

Sources 2, 3 and 4 make SDL3 report a gamepad mapping: the source opens the device as a gamepad, and the map is known. Source 5 opens it as a plain joystick, and the map is generic.

In code, the map is a value (`PadMap`: `Known` or `Generic(GenericLayout)`) that the mapper keeps for each connected pad. The source asks a map source (the trait `PadMaps`) for it, with the identity of the pad and what SDL3 knows of it (`PadLayout`: a gamepad mapping, or the counts of a joystick). The built-in map source (`BuiltinMaps`) gives `Known` for a gamepad mapping and the generic layout for a joystick. M05 puts a source in front of it that reads the settings. The roster entry of a pad says which kind of map it uses (`MapKind`).

## The mapping file

- The file has the `gamecontrollerdb` format: one mapping a line, `GUID,name,field:value,...`. A line that starts with `#` is a comment. An empty line is skipped.
- The file ships with a header comment and no entry. An entry is added only with a source that can be cited in a comment above it. No entry is in it now.
- The source reads the file once, when it opens, before the first frame. A pure parser (`MappingFile::parse`) checks each line and gives the good lines and the problems, with the line number. The source gives each good line to `SDL_AddGamepadMapping`. This replaces the "add mappings from file" call of SDL3 on purpose: that call reports only a count, so a damaged line could not be named, and the parser can be tested with no SDL3.
- A good line has: a GUID of 32 hex digits (or `xinput`, a special GUID of SDL3, in any case), a name that is not empty, and at least one `field:value` pair; every pair has a field and a value. A line with a `platform:` field for another platform (SDL3 names it, `Linux` on the image) is skipped with no problem, as SDL3 does. The platform compares without case, as in SDL 3.4.12 (`SDL_PrivateIsGamepadPlatformMatch` uses `SDL_strncasecmp`). SDL3 compares only the length of the field, so it also takes a prefix such as `Lin`; NEPTRON takes only the whole name.
- A line for this platform with the special GUID `default` (in any case) is refused with its own notice. In SDL 3.4.12 that line sets the fallback mapping (`s_pDefaultMapping` in `SDL_gamepad.c`): `SDL_IsGamepadNameAndGUID` then calls every joystick a gamepad, and a joystick with no mapping of its own and no automatic mapping gets the fallback layout. So the generic layout and its notices would never apply. The pin of the community database has no such line; the notice shows a later pin that adds one in the log.
- A missing file, a file that cannot be read, a damaged line, or a line that SDL3 refuses each give one notice. A damaged line loses only itself. Input always opens. These notices go to the log only.

## The community database

The community database `SDL_GameControllerDB` knows many more pads than the built-in database of a pinned SDL3. The owner decided on 2026-11-17 to ship it, so more pads get a known layout. ADR 0017 records the licence (zlib) and the pin.

- **Where it comes from.** The Buildroot package `os/package/sdl-gamecontrollerdb` downloads the repository at a pinned commit, checks the archive and `LICENSE` against its hash file, and installs `gamecontrollerdb.txt` as `/usr/share/sdl-gamecontrollerdb/gamecontrollerdb.txt`. The three images (PC, QEMU aarch64, Pi) enable it. The repository holds no copy of the file. The file is not under `/usr/share/neptron`, because the install step of the `neptron` package empties that folder: a rebuild of the frontend package alone would lose the file. The licence text goes to the licence folder of the image through `make legal-info`, as for every package.
- **The image check.** `os/board/common/check-gamecontrollerdb.sh`, run by the common post-build step, fails the build when the configuration enables the package and the image has no file, or an empty file.
- **The order of the files.** `SdlGamepads::open` takes the mapping files in priority order: the NEPTRON file, then the community file. It gives the lines to SDL3 in the reverse order, the community file first. Because the later call wins for a GUID, a NEPTRON line replaces a community line, and both replace a built-in line.
- **The platform.** The file has lines for every platform. The same parser reads it, and keeps only the lines with `platform:Linux` (the platform name that SDL3 gives on the image). At the pin there are 752 Linux lines of 2329.
- **A pad that SDL3 knows already.** A community line for a GUID replaces the built-in line of that GUID. This is intended: the community database is newer and wider than the built-in database of a pinned SDL3, and it gets its corrections first. Example: for the Mad Catz stick `0738:8838` (GUID `03000000380700003888000010010000`) the built-in line of SDL 3.4.12 also binds the left stick press to axis 0; the community line does not.
- **A wrong community line.** The player maps the pad again with `Set up this controller` (task M05): the map of the player comes first of all the sources. A line that is wrong for every owner of a pad goes to the NEPTRON file, with the source cited, and is reported upstream.
- **A missing file.** An image built without the package, or a run on a host, has no community file. That is not an error: one notice goes to the log, and input opens. A damaged line is skipped with a notice, as in the NEPTRON file.
- **The cost.** The source reads the file once, when it opens, before the first frame and off the frame loop. The pure parse of the file at the pin (606 kB, 2329 mappings, 752 for Linux) takes 10.4 ms on the development host (Apple M1 Pro, the dev profile with no optimisation, at the low priority of xtask). The image uses the release profile, which is many times faster for this loop: the estimate is 1 ms or less on the host, and 5 to 10 ms on a Raspberry Pi 3 (Cortex-A53, 1.2 GHz). The 752 calls of `SDL_AddGamepadMapping` add to that: each call searches the list of mappings of SDL3 for the GUID, some 1000 entries, which is a few milliseconds more on the Pi 3. The total is once per run, before the first frame, and accepted. A measure on the Pi is in `docs/hardware-checklist.md`. When it shows more than 50 ms, the parser checks the platform of a line before its pairs, so the lines of other platforms (two thirds of the file) cost almost nothing; a further step is a Linux-only copy of the file made by the package.

## The path of a pad

The path keeps two pads of one model apart (the port rules, the pad notices, the focus of `Set up this controller`). The source reads it with `SDL_GetJoystickPathForID`, for a plain joystick and a gamepad alike, through a small safe wrapper (`ffi.rs` of `neptron-input`): the wrapper checks the pointer for null, copies the text with a lossy conversion, and keeps no pointer. The `sdl3` crate wraps only the gamepad call of it, which panics on text that is not UTF-8.

In SDL 3.4 (3.4.12 in the image), `SDL_GetGamepadPathForID` only calls `SDL_GetJoystickPathForID`, with no look-up of a gamepad mapping, so a plain joystick had its path before this change too. The joystick call removes the dependency on that detail and the UTF-8 unwrap. The Linux joystick driver gives every device its node path. A joystick for which SDL3 gives no path gets the empty path.

## The generic layout

The decision is pure code (`generic_layout.rs`, next to `pad_map.rs`). Its input is the counts of buttons, hats and axes. Its output is a table: for each raw button index, hat index and axis index, the RetroPad controls it drives. The SDL3 source reads only the indices of the table and gives them to the mapper, which applies the table. SDL3 types stay in `sdl.rs`.

### Buttons

The order of the HID pads with no mapping is the order of the PlayStation 3 pad. The face buttons map by position, as `PAD_BUTTON_MAP` does for a known pad: the left button is Y, the bottom B, the right A, the top X.

| Raw button (from 0) | PS3 name | Position | RetroPad |
| --- | --- | --- | --- |
| 0 | Square | West | Y |
| 1 | Cross | South | B |
| 2 | Circle | East | A |
| 3 | Triangle | North | X |
| 4 | L1 | Left shoulder | L |
| 5 | R1 | Right shoulder | R |
| 6 | L2 | Left trigger | L2 |
| 7 | R2 | Right trigger | R2 |
| 8 | Select | Back | Select |
| 9 | Start | Start | Start |
| 10 | L3 | Left stick press | L3 |
| 11 | R3 | Right stick press | R3 |
| 12 | PS | Guide | nothing |
| 13 and up | | | nothing |

A pad with fewer buttons maps the ones it has, in this order.

### Hats and axes

- Hat 0 is the D-pad. Other hats drive nothing.
- Axes 0 and 1 are the left stick (X, Y), axes 2 and 3 the right stick, through the stick dead zone (`STICK_DEAD_ZONE`). Other axes drive nothing.
- A pad with no hat also drives the D-pad with axes 0 and 1: a direction is down when the axis is at `GENERIC_DPAD_THRESHOLD` (16384, half travel) or past it. Many arcade sticks with no mapping report their lever as axes 0 and 1.

### What is not a gamepad

A joystick with fewer than 2 buttons, or with no hat and no axis, is not a gamepad. It stays ignored with a notice. A pad needs a direction and two buttons (accept and back) to drive the menus.

### Start and Select

A pad with 10 buttons or more has Select (8) and Start (9). So the quick menu combination and `BLIND_RECOVERY_COMBO` work on it, as on a known pad. A pad with fewer buttons has no Select and no Start. Its player can:

- drive the menus with it: the D-pad, A (accept) and B (back) need 3 buttons at most;
- open the quick menu with a keyboard (Right Shift and Enter) or with another pad: the hotkeys fire on any port;
- in M05, choose its buttons with "Set up this controller".

The tests show what holds: a pad with 8 buttons drives the menus and has no Start or Select.

### Examples

| Pad | Buttons | D-pad | Sticks | Start and Select |
| --- | --- | --- | --- | --- |
| 13 buttons, 1 hat, 4 axes | 0 to 11 as above | hat 0 | left and right | yes |
| 8 buttons, 1 hat | Y B A X L R L2 R2 | hat 0 | none | no |
| 12 buttons, no hat, 2 axes | 0 to 11 as above | axes 0 and 1 | left | yes |
| 1 button, 1 hat | | | | not a gamepad |

## The input notices

The source gives each notice once, when it happens. The app takes them in the frame (`PadRoster::take_notices`), off the hot path: the call moves nothing and allocates nothing when the queue is empty, and the app reads no file in the frame. Every notice goes to the log. These notices also go to the player, as UI notices:

| When | Kind | Text | Second button |
| --- | --- | --- | --- |
| A pad opens with the generic layout | Info | `<name> has no known layout, so it uses a generic one. Some buttons can be in other places.` | `Set up this controller` |
| A joystick is not a gamepad | Info | `<name> is not used: it does not look like a gamepad.` | none |
| A pad finds every port in use (a ninth pad) | Info | `<name> is not used: 8 controllers are in use.` | none |
| A pad sends no input (rule below) | Info | `<name> is connected but sends no input. Try another USB port or a USB hub.` | none |

- `<name>` is the name of the pad, cut to 32 characters with `...`, with `?` for a character that the font does not have (as on the Input panel).
- **Once.** A run is one process, and each run opens the pads again. So the app keeps, for this power cycle, the pads that it told already: the kind of notice, the identity and the path. A pad is told when the player answers its notice (either button), not when the notice is queued: in a game a notice waits for the quick menu, and a run that ends first shows it again in the next run. While a notice of a pad waits, the same pad gives no second one. The writer thread writes the list to `run/pad-notes.toml` in the RAM folder when it grows, and the next run reads it at boot. A file of its own, and not a field of the relaunch file, because the list must also survive a run that ends with no relaunch file, and the frame thread writes no file. A damaged file reads as an empty list: a notice can show again, and nothing else is lost. A pad that is plugged again at a new path is a new plug and gets its notices again.
- The second button is wider than half of the dialog. A dialog button now gets the width of its label when the labels of the row fit with their own widths; a dialog whose labels fit an equal share keeps its layout.
- **In a game** the UI shows a notice only while the quick menu is open (the rule of every notice). The pad notices follow that rule.
- The mapping file notices, a failed joystick list, and a defect of the source go to the log only.

### Set up this controller

In the game list the button starts the guided mapping of that pad (M05, `docs/spec/input-remapping.md`). In a game, or when the pad has no port any more, the button opens `Settings > Input` with the focus on the row of that pad: the app finds the port of the pad by its identity and path in the roster, and asks the UI to open the Input panel with the focus on the row of that port (`Screens::open_panel`). When the pad has no port, the panel opens on its first row.

The button always shows the panel. The UI opens it in its next frame, by this rule:

1. The Input panel is on show: only its focus moves.
2. A Settings page is open: the screens above it close, as by back, and the Input panel opens from it. There is never a second Settings page.
3. Else, when the stack has no room for the Settings page and the panel, the screens above the quick menu, or above the home screen, close first.
4. Else the Settings page and the Input panel open on top of the screen on show: the game list, the home screen, the quick menu or one of its pages.

Back from the panel goes to the Settings page, and back again to the screen under it. A notice shows only when the UI is on show, so a running game with no menu never gets the panel.

### The silent pad

A pad that the kernel enumerates can send only neutral reports: a PS3 arcade stick on a PC does this, and works behind a USB hub. The player then sees a pad on the Input panel that does nothing. The rule for its notice must never nag:

- A pad is **heard** when it sends a value that is not neutral (a button down, a stick out of its dead zone). A heard pad never gets this notice in this plug.
- The notice comes for a pad that has not been heard for `SILENT_PAD_AFTER_NS` (20 s) since it was connected, when the other devices have pressed at least `SILENT_PAD_OTHER_PRESSES` (8) buttons in that time. So somebody uses the machine, and this pad does not answer.
- Only in a run of the game list. A game run never shows it, but it records a pad that is heard in the game.
- Once per pad per plug, with the list of told pads above. A pad that was heard in an earlier run of the power cycle is not checked again: the list carries it as "checked".
- The decision is pure code (`SilentPads` in `neptron-input`) with the time of the frame. The tests use the simulated clock.
- A pad that nobody uses on a couch also gets the notice once. The text is true for it too, and it comes only once per plug.

## The Input panel

A player row and an extra pad row of a pad with the generic layout show ` (generic layout)` after the name: `USB Joystick (generic layout)`. A known pad shows its name alone, and a pad with a map of the player ` (your layout)`. A present pad opens the page of its controller (M05), so its row has the arrow and keeps a fit of 43 characters: `Player 1`, a space, and 34 characters, so the name before ` (generic layout)` has at most 17. A held pad shows ` (away)` as before, with no kind, and the fit of 44 characters.

## What M05 takes over

- `PadMaps`: M05 adds a map source that reads the map of the player from the settings, keyed by the identity, and falls back to `BuiltinMaps`.
- `PadMap`: M05 adds a kind for a map of the player. The mapper applies it with no change of the port rules.
- `Set up this controller`: the button opens the Input panel now. M05 makes it start the guided mapping for the pad of the notice, from the identity and path that the app keeps with the notice.
- The joystick indices: a generic pad gives raw indices to the mapper (`RawInput::JoyButton`, `JoyHat`, `JoyAxis`). A guided mapping records these indices.
- The device of a plain joystick: the SDL3 source keeps a plain joystick only together with its generic table (`Device::Joystick`), and a gamepad only with the standard layout. `Device::with_map` refuses any other pair, and the pad is then ignored with a notice (a `debug_assert` in a test build). A map of the player for a plain joystick is a new `PadMap` kind that reads raw indices; the next task adds it, and adds its pair to `Device::with_map` with the table that the backend reads. The match there has no wildcard, so a new kind does not compile until it is handled.

## Tests

- Pure, the generic layout: 13 buttons, a hat and 4 axes; 8 buttons and a hat; 12 buttons, no hat and 2 axes; too few buttons; no hat and no axis; Start and Select from 10 buttons.
- Pure, the mapping file: a valid file; a missing file; a damaged line, with the other lines kept; a line of another platform; the special GUIDs `default` and `xinput`; a file in the shape of the community database, of which only the Linux lines stay.
- The image: the three images enable the community package; it is pinned to a commit, with the hash of its archive and of its licence file, and installs the file at the path that the frontend reads; the image check fails with the package and no file.
- The mapper: a generic pad gives its buttons, its hat and its axes; a generic pad gives the quick menu combination to the hotkeys; the roster says the kind.
- The silent pad rule with the simulated clock: no notice before the time, no notice without presses of other devices, no notice for a heard pad, once per plug, none in a game run.
- App: the notices once per pad, across a relaunch (the pad notes file); the second button opens the Input panel on the row of the pad; the Input row with the generic suffix; the key of the row of a port.
- UI: `Screens::open_panel` from home and from the quick menu, never over a running game; a long dialog button gets its own width.
- Tier 2, with scripted pads: a scripted plain joystick plugs, gives its notice, drives the menus, and opens the quick menu in a game.
- Scenes and goldens: `screens/input` gets a pad with the generic layout; `screens/notice-generic-pad` shows the notice with its button.

## Hardware checks

Added to `docs/hardware-checklist.md`, section "Input (M04b)", labelled `needs-hardware`: an unmapped pad on the PC and on the Pi gives all its buttons and the D-pad in the menus and in a game; the notice and its button; a silent pad shows its notice; a pad that only the community database knows gets its layout with no notice of the generic layout. No hardware result is claimed.

## Open questions

| Question | Default | Where | How to change |
| --- | --- | --- | --- |
| Ship the community `SDL_GameControllerDB`? | Yes, at a pinned commit, under the zlib licence (ADR 0017) | `os/package/sdl-gamecontrollerdb` | A new pin in the package, its hash file and the ADR |
| The button order of a pad with no mapping | The PS3 order | `generic_layout.rs` | Another table, or M05 per controller |
| The path of a joystick with no path | The empty path: two such pads of one model can swap ports | `ffi.rs`, `sdl.rs` | None needed while SDL3 gives a path for every Linux joystick |
| The times of the silent pad | 20 s and 8 presses | `neptron-input`, `silent.rs` | The constants |
| A pad with fewer than 10 buttons has no Start | Keyboard or another pad for the quick menu | `generic_layout.rs` | M05 |

## Not built

- The picker of the pad of each player (M05b). The guided mapping and the stored maps are in M05.
- Pressure-sensitive buttons, Bluetooth, rumble.
- The kernel option `usbhid.jspoll=1` (an `os/` change of its own).
- A start request or a driver for PS3-type pads.
