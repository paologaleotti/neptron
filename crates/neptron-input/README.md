# neptron-input

Input: SDL3 gamepads, evdev keyboards, RetroPad mapping and hotplug.

## Layers

1. **Device backends** read devices and decide nothing. `SdlGamepads` (feature `sdl3`) and `EvdevKeyboards` (feature `evdev`, Linux only) turn what they read into `RawInput` values (button, axis, hat, key) for a device id.
2. **The mapper** (`Mapper`, pure) gives each device its port and device id by the port rule of the run, turns raw input into `InputEvent`s with the mapping tables, and releases every held control on a disconnect or a move.
3. **The port state** (`PortStates`, pure) merges the events of every source into the RetroPad state of each port.
4. **The hotkeys** (`Hotkeys`, pure) read the port state and a time that the caller passes in.

The tests drive layers 2 to 4 with scripted devices and scripted input, with no hardware and no sleeps.

## Public API

- `Mapper`: `new()` (the rule of the menus), `with_rule(rule, earlier)`, `connect_scan(infos, out)` (the pads of one scan, the first one settles) and `connect_scan_with(pads, out)` (each pad with its `PadMap`), `connect_gamepad(info) -> PadId` and `connect_gamepad_with(info, map)`, `set_map(pad, map, out)` (a new map, with the release of the held controls), `map_of(pad)`, `map_by_key(key)`, `release(pad, out)`, `activity()` (the connected pads for the silent pad rule, no allocation), `disconnect_gamepad(pad, out)`, `apply_gamepad(pad, raw, out)`, `device_of(pad)`, `settle(out)`, `roster()`, `roster_changes()`, and by device id `connect_keyboard()`, `disconnect(device, out)`, `apply(device, raw, out)`, `is_connected(device)`. `MapperError`.
- The roster: `PadInfo` (identity, path, name, `key()`), `PadKey`, `PortRule` (`Menus`, `Game`), `Roster` and `RosterEntry` (the pad of each port, present or away, and its `MapKind`), `RosterError`, `PLAYER_PORTS` (4), and the trait `PadRoster` (`roster()`, `roster_changes()`, `take_notices(out)`, `activity()`, `set_player_maps(maps)`, `capture(pad)`, `captured() -> Capture`, `pad_map(key)`) that a source of pads gives the frontend. `take_notices`, `activity`, `captured` and `pad_map` take a shared reference, because a game session owns the source; they allocate nothing when there is nothing new. `set_player_maps` and `capture` take effect at the next poll.
- The maps: `PadMap` (`Known`, `Generic(GenericLayout)`, `Player(PlayerMap)`), `MapKind` (`Known`, `Generic`, `Player`), `PadLayout` (`Gamepad(counts)`, a gamepad mapping with the counts of its joystick, or `Joystick(counts)`, a plain joystick), the trait `PadMaps` (the map of a pad by its identity), `BuiltinMaps`, and `PlayerMaps` (the maps of the player by identity, in front of `BuiltinMaps`). `PadMap::raw_reads()` gives the raw indices a backend reads.
- The map of the player (`docs/spec/input-remapping.md`): `PlayerMap` (`get`, `set`, `bindings`, `reads`, `menu_gaps`, `has_menu_combo`), `Binding` (`Button`, `Hat`, `HalfAxis`, `Stick`) with its text (`Binding::parse`, `Display`: `b3`, `h0up`, `a2+`, `a4+full`, `a0`, `-a0`), `HatDirection`, `BindingError`, `CONTROL_KEYS`, `control_key`, `control_of_key`, `is_axis`, `MENU_CONTROLS`, `MAP_AXIS_THRESHOLD`.
- The raw state of a joystick: `RawState` (every button, hat and axis by index, fixed size) and `RawReads`, with the limits `MAX_RAW_BUTTONS` (32), `MAX_RAW_HATS` (4), `MAX_RAW_AXES` (8).
- The guided mapping: `GuidedMapping::update(state, now_ns) -> Guided` (`Prompt`, `Done(PlayerMap)`, `TimedOut`), `Prompt` (with `release`: the pad stays held between steps), `GuidedNote`, `GUIDED_STEPS`, `MAP_SKIP_HOLD_NS` (2 s), `MAP_IDLE_TIMEOUT_NS` (15 s), `MAP_TOTAL_TIMEOUT_NS` (300 s), `MAP_AXIS_RELEASE` (8192), `MAP_RELEASE_HINT_NS` (1 s).
- The generic layout: `GenericLayout::for_counts(counts)`, `NotAGamepad`, `AxisRole`, `GENERIC_BUTTON_ORDER`, `GENERIC_DPAD_THRESHOLD`, `MIN_GENERIC_BUTTONS`, `GENERIC_AXES`, `dpad_axis_values`.
- The mapping files: `MAPPING_FILE` (under the data folder), `COMMUNITY_MAPPING_FILE` (the absolute path of the community database in the image), `read_mapping_file(path, platform)`, `parse_mappings(text, path, platform)`, `Mappings`, `MappingLineError`.
- The silent pad rule: `SilentPads` (`new(checked)`, `update(activity, presses, now_ns, check)`, `checked()`, `checked_count()`), `PadActivity`, `PadSignal`, `SilentPad`, `SILENT_PAD_AFTER_NS`, `SILENT_PAD_OTHER_PRESSES`.
- `PadInfo::from_device(identity, path, name_bytes)`: a name that is not UTF-8 is read with a lossy conversion.
- `ScriptedPads`: gamepads with no hardware for tests of the frontend (`plug`, `plug_joystick`, `unplug`, `replug`, `set`, `set_joy_button`, `set_joy_hat`, `set_joy_axis`, `device`), through the same mapper, maps and notices as the SDL3 source. `InputSource` and `PadRoster`.
- `PortStates`: `apply(events)`, `value(port, control)`, `is_held(port, control)`.
- `Hotkeys`: `update(ports, now_ns, out)` gives `Hotkey::QuickMenu { port }` and `Hotkey::BlindRecovery { port }`. `set_start_hold_ns(hold_ns)` and `start_hold_ns()` give the Start hold time (0 is off).
- `port_of(device)`, `DeviceIdentity`, the id ranges and limits.
- The mapping tables and rules: `PAD_BUTTON_MAP`, `pad_control`, `axis_target`, `stick_value`, `trigger_value`, `hat_values`, `KEY_MAP`, `key_control`, `looks_like_keyboard`.
- `InputNotice`: what a backend tells the frontend, with a log text (`Display`). The frontend shows `GenericLayout`, `NotAGamepad` and `NoFreePort` to the player.
- Feature `sdl3`: `SdlGamepads: InputSource + PadRoster`, with `open(rule, earlier, mappings)` and `take_notices(out)`. `SdlInputError`.
- Feature `evdev`, on Linux: `EvdevKeyboards: InputSource`, with `new()`, `in_dir(dir)` and `take_notices(out)`.

No SDL3 or evdev type is in the public API.

## Gamepad map

SDL3 maps every controller of its database to one standard layout and names the face buttons by position. The RetroPad is laid out like a SNES pad, so the map goes by position, as in RetroArch. On an Xbox pad the button labelled A is the RetroPad B.

| SDL3 standard layout | RetroPad |
| --- | --- |
| South, East, West, North | B, A, Y, X |
| Back | Select |
| Start | Start |
| Left and right shoulder | L, R |
| Left and right stick press | L3, R3 |
| D-pad | D-pad |
| Left and right trigger | L2, R2, down from `TRIGGER_THRESHOLD` (16384, half travel) |
| Left and right stick | LeftX, LeftY, RightX, RightY |
| Guide, paddles, touchpad, misc | nothing |

- A stick value with a magnitude up to `STICK_DEAD_ZONE` (4096) gives 0. Above it the value is scaled, so the output starts at 0 and reaches 32767. -32768 gives -32767. Negative Y is up, in SDL3 and in libretro.
- A hat sets the D-pad. SDL3 already turns the hat of a known gamepad into D-pad buttons.
- A joystick that SDL3 has no gamepad mapping for opens as a plain joystick with the generic layout (below).

## The sources of a map

`docs/spec/input-generic-pads.md` and `docs/spec/input-remapping.md` are the specs. The map of a pad comes from the first source that has one: the map of the player in the settings (`PlayerMaps`, by the identity of the controller), the NEPTRON mapping file, the community database, the database of SDL3, the generic layout. The source asks `PadMaps` for the map when a pad connects and when the maps change, and the mapper keeps it per pad.

- **The map of the player** reads raw joystick indices, for a plain joystick and for a gamepad alike, so a map means the same for both. The SDL3 source keeps the joystick of every pad open, and `Device::with_map` pairs a gamepad with a map of the player to a raw reader of its joystick; it keeps the gamepad open, so a reset reads the standard layout again with no reconnect. A scripted gamepad has the buttons of `PAD_BUTTON_MAP` as raw buttons 0 to 13.
- **The counts of the pad**: `PlayerMaps` gives a pad only the bindings that its joystick has (`PlayerMap::fitted`, with the counts of `PadLayout`), and the source logs one `InputNotice::LayoutTrimmed` with the number dropped. `Binding::value` reads an index past the counts of a raw state as not down.
- **New maps** (`set_player_maps`) reach each open pad at the next poll: the mapper releases its held controls (`Mapper::set_map`), and the roster shows the new kind. A pad that only the map of the player could read is dropped, as a pad that is not a gamepad.
- **The capture** (`capture(Some(key))`) is for the guided mapping: from the next poll the source releases the controls of the pad, sends no event for it, and gives its whole raw state (`Capture::Pad`), every index that the joystick has, up to the limits of `RawState`. `capture(None)` ends it. A pad that goes away gives `Capture::Gone`. A captured pad counts as heard for the silent pad rule (`Mapper::mark_heard`).
- **The guided mapping** (`GuidedMapping`) is pure: the D-pad first, then A, B, Start, Select, the other buttons and the sticks. The first raw state gives the rest of each axis, so a trigger that rests at full travel counts from its rest. A press counts at its release, and an axis press ends only past `MAP_AXIS_RELEASE`; a hold of 2 s skips the step; an input used twice is refused; the pad must be neutral between steps; 15 s with no press or release, or 300 s in all, ends it.

- **The mapping file**, `data/input/gamecontrollerdb.txt` (`/usr/share/neptron/input/` on the image), in the `gamecontrollerdb` format. `SdlGamepads::open(rule, earlier, mappings)` takes the mapping files in priority order and reads each, `parse_mappings` checks each line, and each good line goes to `SDL_AddGamepadMapping`, which replaces a built-in line of the same GUID. A line of another platform is skipped. A missing file, a damaged line or a line that SDL3 refuses each give one `InputNotice::MappingFile`; the other lines still load, and input always opens. The file ships with no entry: an entry needs a source that can be cited. The GUID `xinput` is good, as in SDL3. A line with the GUID `default` is refused with a notice: SDL3 would make it the fallback layout of every joystick, and the generic layout would never apply.
- **The community database**, `SDL_GameControllerDB` at a pinned commit, under the zlib licence (ADR 0017). The package `os/package/sdl-gamecontrollerdb` installs it as `/usr/share/sdl-gamecontrollerdb/gamecontrollerdb.txt` (`COMMUNITY_MAPPING_FILE`). The frontend passes it after the NEPTRON file. SDL3 keeps the line of the later call for one GUID, and an added line always replaces a built-in line, so the source gives the files to SDL3 in the reverse order: the community file first, then the NEPTRON file. Only the lines with `platform:Linux` load (752 of 2329 at the pin). A missing file (a host run, an image without the package) gives one notice to the log. A community line replaces the built-in line of SDL3 for the same GUID on purpose: the community database is newer. A player corrects a wrong line with the map of the player.
- **The generic layout**, for a joystick with no mapping. The backend reads only the indices of its table (`RawInput::JoyButton`, `JoyHat`, `JoyAxis`), and the mapper applies the table.

| Raw button | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| PS3 name | Square | Cross | Circle | Triangle | L1 | R1 | L2 | R2 | Select | Start | L3 | R3 | PS |
| RetroPad | Y | B | A | X | L | R | L2 | R2 | Select | Start | L3 | R3 | nothing |

- Hat 0 is the D-pad. Axes 0 and 1 are the left stick, 2 and 3 the right stick, through the dead zone. With no hat, axes 0 and 1 also drive the D-pad from `GENERIC_DPAD_THRESHOLD` (16384).
- A pad with fewer buttons maps the ones it has, in this order. From 10 buttons it has Select and Start, so the quick menu and the blind recovery work on it. With fewer, its player opens the quick menu from a keyboard or another pad.
- A joystick with fewer than 2 buttons, or with no hat and no axis, is not a gamepad: it is ignored with `InputNotice::NotAGamepad`.

## The silent pad

A pad can enumerate and send only neutral reports (a PS3 arcade stick on a PC, which works behind a USB hub). The mapper marks a pad heard at its first value that is not neutral. `SilentPads` gives a pad that is not heard `SILENT_PAD_AFTER_NS` (20 s) after its connect, when the other devices pressed `SILENT_PAD_OTHER_PRESSES` (8) buttons in that time, once per plug, and only when the caller allows it (the frontend: in the game list).

## Keyboard map

The defaults of the RetroArch keyboard driver for player one, by physical key (Linux key codes). Every keyboard plays on port 0. A key repeat gives no event.

| Keys | RetroPad |
| --- | --- |
| Arrows | D-pad |
| X, Z, S, A | A, B, X, Y |
| Q, W | L, R |
| Enter | Start |
| Right Shift | Select |

`neptron-window` has the same defaults by SDL scancode. The shared fixture `tests/fixtures/retroarch-keyboard.txt` lists the pairs by RetroArch key name, and a test holds `KEY_MAP` to it. No test checks the window table against the fixture (see open points).

The frontend actions of a keyboard come from the hotkeys: Right Shift and Enter held together for 0.5 s open the quick menu.

A key held before a keyboard opens gives no kernel event. So when the source opens a keyboard, it reads the key state and sends a press for each mapped key that is down (`mapped_keys_down`). A combination held from power-on, or held while a keyboard opens again after a read error, reaches the hotkeys.

## Device ids and ports

| Ids | Devices | Port |
| --- | --- | --- |
| 0 to 7 | Direct ids: the window keyboard (id 0), scripted input, the control API | the id |
| `0x100` to `0x107` | SDL3 gamepads and scripted pads | the id minus `0x100` |
| `0x200` to `0x207` | evdev keyboards | 0 |

`port_of(device)` gives the port. Ids outside the ranges have no port and `PortStates` ignores them. The window keyboard keeps its id 0 and needs no change. A gamepad id holds its port, so it changes when the pad moves down in the menus.

## Ports and hotplug

`docs/spec/input-players.md` is the spec. In short:

- A pad gets its port in plug order: the lowest free port. At the start of a run, the order in which SDL3 lists the pads.
- Ports 0 to 3 (`PLAYER_PORTS`) are players 1 to 4: the core reads them. Ports 4 to 7 are extra ports: their pads drive the menus and the hotkeys only.
- The rule of the run (`PortRule`) decides what a disconnect does:
  - `Menus` (no game): the port is freed, and every pad on a higher port moves down by one. So the ports in use are always 0 to n-1. A pad that moves gets the release of every control it held, on its old device id, and starts neutral on the new one; the next read gives its controls again. A pad that comes back is a new pad, at the end.
  - `Game`: the port is held for the pad, and nothing moves, so a running core never sees a pad change its port and never loses player 1 to another pad.
- A new pad takes a held port with the same identity and path, else a held port with the same identity, else the lowest free port.
- `Mapper::with_rule(rule, earlier)` starts with every pad of `earlier` held on its port: the roster of the run before, which the app carries in the relaunch file. Each scan of a source goes to `connect_scan` as one batch. A pad whose identity and path match a held port connects first, so a pad of the same model at a new path cannot take the port held for a pad later in the same scan. The first scan then settles: in the menus the held ports that no pad took are freed and the pads move down, and a pad of the scan that found every port in use or held tries again; in a game they stay held.
- A disconnect gives the release of every control that the device held, so no button stays down.
- The identity of a gamepad is its SDL GUID (vendor, product, version): the key of everything kept per pad. Two pads of one model have the same identity. The path (the event node `/dev/input/eventN`, the classic node `/dev/input/jsN` when the `SDL_JOYSTICK_LINUX_CLASSIC` hint is set, or, with the HIDAPI drivers of SDL3, the hidraw node `/dev/hidrawN`) tells them apart while they stay plugged, also across a relaunch. When both go away in a game and come back at new paths, they can swap ports.
- The `sdl3` crate unwraps the serial number, the gamepad name and the path as UTF-8, and panics on other bytes. So the source does not read the serial number. It reads the name through the joystick API, with a lossy conversion (a gamepad with no name in its mapping takes the device name, which can be any bytes). It reads the path with `SDL_GetJoystickPathForID` through `ffi.rs`, for a plain joystick and a gamepad alike, with a lossy conversion, because the `sdl3` crate wraps only the gamepad call, which unwraps as UTF-8. A joystick with no path gets the empty one.
- At most 8 gamepads and 8 keyboards. A ninth gamepad, or a pad that finds every port in use or held, is ignored, with a notice (`NoFreePort`), until it reconnects.
- A scan whose joystick list fails still goes to the mapper as an empty scan, so the first scan of a run always settles the ports.
- A port merges its devices: a button is down while any device holds it, an axis takes the value with the largest magnitude.
- A backend keeps the `PadId` of each pad: it stays the same while the pad is connected, and `device_of` gives its device id now.

**What hotplug relies on.** `SdlGamepads` calls `SDL_UpdateGamepads` in each poll, which runs the device detection of SDL3. The source asks SDL3 for the joystick list every `SCAN_INTERVAL_POLLS` (30) polls and checks each open gamepad for a disconnect in every poll. With no libudev, SDL3 on Linux finds devices through inotify on `/dev/input`, or by a scan of that folder when inotify is not there. `EvdevKeyboards` scans `/dev/input` every `KEYBOARD_SCAN_INTERVAL_POLLS` (120) polls. Both rely on `mdev` making the nodes in `/dev/input` with read access for the frontend. This is unverified on the target until the image runs it.

**A line on stderr when a keyboard is lost.** evdev 0.13.2 ungrabs a grabbed `evdev::Device` on drop and prints a failed ungrab with `eprintln!`; a device never grabbed drops silently. So the source grabs a keyboard as the last step before it keeps it, and the scan of other nodes prints nothing. A grabbed keyboard that goes away (unplugged, read error) prints one line, `Failed to ungrab device: ...`, and the crate gives no safe way to skip it. The source also gives an `InputNotice::Lost` for it.

## The SDL event pump

SDL allows one event pump for each process. In a window build the window keyboard owns it. `SdlGamepads` needs none:

- It turns off SDL's joystick and gamepad events, so SDL queues none of them, and the window's pump does not read the devices.
- Each `poll` calls `SDL_UpdateGamepads`, which reads the devices, and then reads the state of each button and axis of each open gamepad.

The same code runs on the device, where no video subsystem and no pump exist, and in a window build. With the `sdl3` crate, every `sdl3::init` of the process must run on the thread of the first one, so make the window, the audio sink and this source on one thread, and poll this source there.

## Hotkeys

| Hotkey | Combination on one port | Fires |
| --- | --- | --- |
| Quick menu | Select + Start (`QUICK_MENU_COMBO`) | when it has stayed down for `QUICK_MENU_HOLD_NS` (0.5 s) |
| Blind recovery | Select + Start (`BLIND_RECOVERY_COMBO`) | when it has stayed down for `BLIND_RECOVERY_HOLD_NS` (5 s) |
| Quick menu | Start alone (`START_HOLD_BUTTON`), off by default | when it has stayed down alone for the time of `set_start_hold_ns` |

- Each fires once per hold, at the first `update` at or after its hold time, not at the release. A release of one of the buttons ends the hold, and the next time both are down a new hold starts.
- A shorter hold of Select and Start fires nothing. On an arcade stick Select is the coin, and a coin and a Start pressed in a quick sequence overlap for a moment.
- The hold counts from the first `update` that sees the combination, so call `update` every frame, also with no event.
- The two combinations share their buttons, so a recovery hold opens the quick menu at 0.5 s and fires the recovery at 5 s. The frontend acts on the recovery also when the menu is open.
- `cancel_quick_menu()` ends every quick menu hold in progress: it fires no menu. The session calls it when the menu closes, so a combination held across the close does not open the menu again. The recovery hold goes on.
- Every source gives the same port state, so the rule is the same for every pad on every port, a generic pad with Select and Start, and the keyboard pair Right Shift and Enter.
- The quick menu hold is a constant, not a setting.
- The recogniser reads the port state only. It needs no picture and no UI, and the control API and scripted input can trigger it.

### The Start hold

An arcade panel can have no Select button. For it, Start held alone can open the quick menu. The setting `input.menu_hold_start_seconds` of `neptron-settings` holds the time in seconds. 0 is off, and 0 is the default.

The rule for "Start alone", on each port:

- The hold starts at the first `update` that sees Start down and Select (`START_HOLD_BLOCKER`) up.
- It fires one `Hotkey::QuickMenu` when it has lasted the hold time: at exactly the hold time, not one nanosecond before.
- A release of Start ends the hold. The next press starts a new one.
- Select down on the same port ends the hold, and no new hold starts until Start is released. So Select and Start give only their own quick menu, after their 0.5 s hold, also when Select goes up again while Start stays down.
- A Start hold that fired is the quick menu of that press of Start: Select that joins later gives no second quick menu until Start is released. The recovery hold that follows on the same press of Start still fires at 5 s.
- Other buttons on the port, and every button on other ports, do not matter.
- A disconnect releases Start, so it ends the hold.
- A new hold time ends every hold in progress. The same time again changes nothing. A time set before the first `update` applies to a Start held from power-on.

The blind recovery does not change, with the Start hold on or off.

What the app must do:

- Read the resolved `input.menu_hold_start_seconds`, multiply it by 1 000 000 000, and pass it to `Hotkeys::set_start_hold_ns`. It can pass it every frame, or when the setting changes.
- The core sees Start down for the whole hold, so the game can act on it before the menu opens (for example, it pauses). The recogniser only reports and changes no port state. When the hold fires, the app opens the quick menu, which pauses the core. From then on, the app gives the core Start up on that port until Start is released, so the core sees one release and no new press when it resumes.
- The 2 s of an arcade panel is a choice of the first-boot setup, which writes it to the settings. It is not a hidden default in this crate or in the app: nothing tells a pad from a panel.

## Frame loop and latency

Poll every source once per frame, immediately before `retro_run`, into one buffer that the frame loop keeps. Then apply the buffer to `PortStates` and call `Hotkeys::update` with the frame time. The core reads `PortStates` in that frame.

- An event waits in the kernel buffer of its device (evdev) or in the gamepad state of SDL3 until the next poll, so at most one frame.
- A press and a release that both come between two polls: the core sees the state at the poll, so it sees neither. The event stream has both.
- `poll` never blocks. A keyboard gives at most 8 reads per poll; the rest waits for the next poll.
- No allocation in the steady state: the backends keep their buffers, the mapper and the port state are fixed arrays, and the caller owns the event buffer. Two exceptions, both rare: the SDL3 joystick list and the `/dev/input` scan allocate a short list once per scan interval, and a notice allocates its text.

## Checks

No tier compiles the `sdl3` or `evdev` code. After a change, run:

```sh
cargo xtask clippy -p neptron-input --features sdl3
cargo xtask test -p neptron-input --features sdl3
```

The `evdev` code compiles only on Linux. On a Linux host, or with a Linux target installed, run `cargo xtask clippy -p neptron-input --features evdev` (add `--target <target>` on another host).

`examples/gamepads.rs` prints the events and notices of a real gamepad for 10 seconds: `cargo run -p neptron-input --features sdl3 --example gamepads`.

## Hardware checks

These need a device and are not verified:

- A USB gamepad on the x86_64 image: found at boot and when plugged in later, buttons as in the gamepad map, a disconnect releases its buttons.
- A pad with no SDL3 mapping on the PC and on the Pi: the generic layout, its notice, and the notice of a silent pad (`docs/hardware-checklist.md`, "Input: every USB gamepad (M04b)"). Two pads of one unknown model keep their ports across a launch (their paths come from `SDL_GetJoystickPathForID`).
- The guided mapping of an unknown pad and of a known pad, and a gamepad read by the raw indices of its joystick (`docs/hardware-checklist.md`, "Input remapping (M05)").
- Two and four pads in a game, an unplug and a replug in a game, two pads of one model across a launch, and the event or hidraw node of a pad that stays plugged across a relaunch (`docs/hardware-checklist.md`, "Input (M04)").
- SDL3 without libudev finds a gamepad plugged in after start (inotify on `/dev/input`).
- A USB keyboard on the image: found, grabbed (no key reaches a console), mapped, and released when unplugged.
- Blind recovery from a gamepad and from a keyboard with no picture.
- The Start hold from an arcade panel encoder with no Select button.
- The combination held at boot on a keyboard encoder triggers recovery.

## Must never depend on

- `neptron-kms`, `neptron-ui`, `neptron-video`, `neptron-window`
- SDL3 or evdev outside their features

## Rules

- `unsafe` is denied in every module (`#![deny(unsafe_code)]` at the crate root) but `ffi.rs` (feature `sdl3`), which holds the one SDL3 call that the `sdl3` crate does not wrap, `SDL_GetJoystickPathForID`, behind a safe function that checks the pointer for null, copies the text, and keeps no pointer. Each `unsafe` block has a `// SAFETY:` comment. The crate is on the `unsafe` allow-list.
- Features: `sdl3`, `evdev`. No feature is on by default.

## Status

Complete for the MVP. The SDL3 path compiles and passes clippy and its tests on macOS. The evdev path compiles only on Linux. The `kms` build of the frontend, with both paths, runs on the i3-6100T and on a Pi 3 B, where games were played. No record names the input devices of those sessions (UNVERIFIED). The generic layout, the mapping file and the silent pad rule have run with scripted pads only; no unmapped pad has run on hardware (UNVERIFIED). The map of the player, the capture and the guided mapping have run with scripted pads only; the SDL3 path of them is compiled for Linux, and no pad has been mapped on hardware (UNVERIFIED).
