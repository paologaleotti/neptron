# neptron-input

Input: SDL3 gamepads, evdev keyboards, RetroPad mapping and hotplug.

Four layers. The device backends read devices and decide nothing: `SdlGamepads` (feature `sdl3`) and `EvdevKeyboards` (feature `evdev`, Linux only) make `RawInput` values for a device id. The pure `Mapper` gives each device its port and turns raw input into `InputEvent`s. The pure `PortStates` merges the events into the RetroPad state of each port. The pure `Hotkeys` reads the port state and a time from the caller. The tests drive the pure layers with scripted devices, with no hardware and no sleeps.

## Must never depend on

- `neptron-kms`, `neptron-ui`, `neptron-video`, `neptron-window`
- SDL3 or evdev outside their features

## Rules

- `#![deny(unsafe_code)]` at the crate root. Only `ffi.rs` (feature `sdl3`) allows `unsafe`. It holds the one SDL3 call that the `sdl3` crate does not wrap, `SDL_GetJoystickPathForID`, behind a safe function that checks the pointer for null, copies the text and keeps no pointer. Each `unsafe` block has a `// SAFETY:` comment. The crate is on the `unsafe` allow-list.
- Features: `sdl3`, `evdev`. No feature is on by default. No SDL3 or evdev type is in the public API.

## Public API

| Area | Items |
| --- | --- |
| Mapper | `Mapper` (`new()` uses the menus rule, `with_rule(rule, earlier)`, `connect_scan`, `connect_scan_with`, `connect_gamepad`, `connect_gamepad_with`, `set_map`, `map_of`, `map_by_key`, `release`, `activity`, `mark_heard`, `disconnect_gamepad`, `apply_gamepad`, `device_of`, `settle`, `roster`, `roster_changes`; by device id `connect_keyboard`, `disconnect`, `apply`, `is_connected`), `MapperError`, `PadId` |
| Roster | `PadInfo` (`from_device` reads a name that is not UTF-8 with a lossy conversion, `key()`), `PadKey`, `PortRule` (`Menus`, `Game`), `Roster`, `RosterEntry`, `RosterError`, `PLAYER_PORTS` (4), `Capture` (`Waiting`, `Pad`, `Gone`), trait `PadRoster` |
| Maps | `PadMap` (`Known`, `Generic`, `Player`; `raw_reads()`), `MapKind`, `PadLayout` (`Gamepad(counts)`, `Joystick(counts)`), `JoystickCounts`, trait `PadMaps`, `BuiltinMaps`, `PlayerMaps` |
| Map of the player | `PlayerMap`, `Binding` (`Button`, `Hat`, `HalfAxis`, `Stick`; text form `b3`, `h0up`, `a2+`, `a4+full`, `a0`, `-a0`), `HatDirection`, `BindingError`, `CONTROL_KEYS`, `control_key`, `control_of_key`, `is_axis`, `MENU_CONTROLS`, `MAP_AXIS_THRESHOLD` |
| Raw state | `RawState`, `RawReads`, `MAX_RAW_BUTTONS` (32), `MAX_RAW_HATS` (4), `MAX_RAW_AXES` (8) |
| Guided mapping | `GuidedMapping` (`update(state, now_ns) -> Guided`, `neutral`), `Guided` (`Prompt`, `Done`, `TimedOut`), `Prompt`, `GuidedNote`, `GUIDED_STEPS`, `MAP_SKIP_HOLD_NS` (2 s), `MAP_IDLE_TIMEOUT_NS` (15 s), `MAP_TOTAL_TIMEOUT_NS` (300 s), `MAP_AXIS_RELEASE` (8192), `MAP_RELEASE_HINT_NS` (1 s) |
| Generic layout | `GenericLayout::for_counts`, `NotAGamepad`, `AxisRole`, `GENERIC_BUTTON_ORDER`, `GENERIC_DPAD_THRESHOLD`, `MIN_GENERIC_BUTTONS`, `GENERIC_AXES`, `dpad_axis_values` |
| Mapping files | `MAPPING_FILE`, `COMMUNITY_MAPPING_FILE`, `read_mapping_file`, `parse_mappings`, `Mappings`, `MappingLineError` |
| Silent pad | `SilentPads` (`new(checked)`, `update`, `checked`, `checked_count`), `PadActivity`, `PadSignal`, `SilentPad`, `SILENT_PAD_AFTER_NS`, `SILENT_PAD_OTHER_PRESSES` |
| Ports and hotkeys | `PortStates` (`apply`, `value`, `is_held`), `Hotkeys` (`update`, `set_start_hold_ns`, `start_hold_ns`, `cancel_quick_menu`), `Hotkey` (`QuickMenu { port }`, `BlindRecovery { port }`), `port_of`, `DeviceIdentity`, the id bases and limits |
| Tables | `PAD_BUTTON_MAP`, `pad_control`, `axis_target`, `stick_value`, `trigger_value`, `hat_values`, `KEY_MAP`, `key_control`, `looks_like_keyboard`, `mapped_keys_down` |
| Sources | `InputSource`, `InputNotice` (log text by `Display`); `ScriptedPads` and `ScriptedPad` for frontend tests, through the same mapper, maps and notices as SDL3; feature `sdl3`: `SdlGamepads: InputSource + PadRoster` (`open(rule, earlier, mappings)`), `SdlInputError`; feature `evdev` on Linux: `EvdevKeyboards: InputSource` (`new()`, `in_dir(dir)`) |

`PadRoster` is what a source of pads gives the frontend. `take_notices`, `activity`, `captured` and `pad_map` take a shared reference, because a game session owns the source, and allocate nothing when nothing is new. `set_player_maps` and `capture` take effect at the next poll. The frontend shows the notices `GenericLayout`, `NotAGamepad` and `NoFreePort` to the player.

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

- A stick value with a magnitude up to `STICK_DEAD_ZONE` (4096) gives 0. Above it the value is scaled from 0 to 32767. -32768 gives -32767. Negative Y is up, in SDL3 and in libretro.
- A hat sets the D-pad.
- A joystick with no SDL3 gamepad mapping opens as a plain joystick with the generic layout.

## The sources of a map

`docs/spec/input-generic-pads.md` (the mapping file, the community database, the generic layout, the silent pad) and `docs/spec/input-remapping.md` (the map of the player, the guided mapping, the capture) are the specs. A pad takes the map of the first source that has one: the map of the player (`PlayerMaps`), the NEPTRON mapping file, the community database, the SDL3 database, the generic layout. The mapper keeps the map per pad.

- **Mapping files.** `MAPPING_FILE` is `input/gamecontrollerdb.txt` under the data folder (`data/input/` in the repository, `/usr/share/neptron/input/` on the image). It ships with no entry. `COMMUNITY_MAPPING_FILE` is the pinned `SDL_GameControllerDB` (zlib, ADR 0017). `SdlGamepads::open` takes the files in priority order and gives them to SDL3 in reverse order, because the later line wins for a GUID. A missing file or a bad line gives one `InputNotice::MappingFile`, and input always opens.
- **The map of the player** reads raw joystick indices, for a plain joystick and a gamepad alike. `PlayerMaps` drops the bindings past the counts of the joystick (`InputNotice::LayoutTrimmed`). A fitted map that lacks a direction, A or B falls back to `BuiltinMaps`, so the pad can always drive the menus. `Binding::value` reads an index past the counts of a raw state as not down. A scripted gamepad has the buttons of `PAD_BUTTON_MAP` as raw buttons 0 to 13.
- **New maps** (`set_player_maps`) reach each open pad at the next poll, with the release of its held controls.
- **The capture** (`capture(Some(key))`) gives the whole raw state of one pad and sends no event for it, from the next poll until `capture(None)`. A captured pad counts as heard for the silent pad rule.
- **The guided mapping** is pure and runs on the captured raw states.
- **The generic layout** is for a joystick with no mapping. The backend reads only the indices of its table (`RawInput::JoyButton`, `JoyHat`, `JoyAxis`). Raw buttons 0 to 11 are Y, B, A, X, L, R, L2, R2, Select, Start, L3, R3 (PS3 order). Hat 0 is the D-pad. Axes 0 to 3 are the sticks. From 10 buttons a pad has Select and Start, so the quick menu and the blind recovery work on it. A joystick with fewer than 2 buttons, or with no hat and no axis, is ignored with `InputNotice::NotAGamepad`.

## The silent pad

A pad can enumerate and send only neutral reports. `SilentPads` reports a pad that has sent no value that is not neutral `SILENT_PAD_AFTER_NS` (20 s) after its connect, if the other devices pressed `SILENT_PAD_OTHER_PRESSES` (8) buttons in that time. It reports once per plug, and only when the caller allows it (the frontend: in the game list).

## Keyboard map

The RetroArch keyboard defaults for player one, by physical key (Linux key codes). Every keyboard plays on port 0. A key repeat gives no event.

| Keys | RetroPad |
| --- | --- |
| Arrows | D-pad |
| X, Z, S, A | A, B, X, Y |
| Q, W | L, R |
| Enter | Start |
| Right Shift | Select |

`neptron-window` has the same defaults by SDL scancode. `tests/fixtures/retroarch-keyboard.txt` lists the pairs by RetroArch key name, and a test holds `KEY_MAP` to it. No test checks the window table against the fixture (OP-039).

Right Shift and Enter held together for 0.5 s open the quick menu (the hotkeys below).

A key held before a keyboard opens gives no kernel event. So when the source opens a keyboard, it sends a press for each mapped key that is down (`mapped_keys_down`). A combination held from power-on, or while a keyboard opens again after a read error, reaches the hotkeys.

## Device ids and ports

| Ids | Devices | Port |
| --- | --- | --- |
| 0 to 7 | Direct ids: the window keyboard (id 0), scripted input, the control API | the id |
| `0x100` to `0x107` | SDL3 gamepads and scripted pads | the id minus `0x100` |
| `0x200` to `0x207` | evdev keyboards | 0 |

`port_of(device)` gives the port. Ids outside the ranges have no port, and `PortStates` ignores them. A gamepad id holds its port, so it changes when the pad moves down in the menus. A backend keeps the `PadId` of each pad: it stays the same while the pad is connected, and `device_of` gives its device id now.

## Ports and hotplug

`docs/spec/input-players.md` is the spec. In short:

- A pad gets the lowest free port, in plug order. Ports 0 to 3 (`PLAYER_PORTS`) are players 1 to 4. Ports 4 to 7 drive only the menus and the hotkeys.
- `PortRule::Menus` (no game): a disconnect frees the port, and every pad on a higher port moves down by one. A pad that moves gets the release of every held control on its old device id. `PortRule::Game`: the port is held for the pad, and nothing moves.
- `Mapper::with_rule(rule, earlier)` starts with the pads of the run before held on their ports. Each scan of a source goes to `connect_scan` as one batch, and the first scan settles the held ports. A scan whose joystick list fails still goes to the mapper as an empty scan.
- A disconnect releases every control the device held.
- The identity of a gamepad is its SDL GUID. Two pads of one model have the same identity. The path (`/dev/input/eventN`, `/dev/input/jsN` with the `SDL_JOYSTICK_LINUX_CLASSIC` hint, or `/dev/hidrawN` with the HIDAPI drivers) tells them apart while they stay plugged. When both go away in a game and come back at new paths, they can swap ports (OP-039).
- The `sdl3` crate unwraps the serial number, the gamepad name and the path as UTF-8, and panics on other bytes. So the source does not read the serial number, reads the name through the joystick API with a lossy conversion, and reads the path through `ffi.rs` with a lossy conversion.
- At most 8 gamepads and 8 keyboards. A ninth gamepad, or a pad that finds every port in use or held, is ignored with `NoFreePort` until it reconnects (OP-039).
- A port merges its devices: a button is down while any device holds it, and an axis takes the value with the largest magnitude.

**What hotplug relies on.** Each `SdlGamepads` poll calls `SDL_UpdateGamepads`, which runs SDL3 device detection. The source asks for the joystick list every `SCAN_INTERVAL_POLLS` (30) polls and checks each open gamepad for a disconnect in every poll. `EvdevKeyboards` scans `/dev/input` every `KEYBOARD_SCAN_INTERVAL_POLLS` (120) polls. Both scans run on the frame thread. With no libudev, SDL3 on Linux finds devices through inotify on `/dev/input`, or by a scan of that folder. Both sources rely on `mdev` making the nodes in `/dev/input` readable by the frontend. This is not verified on the target (OP-039).

**A line on stderr when a keyboard is lost.** evdev 0.13.2 ungrabs a grabbed device on drop and prints a failed ungrab with `eprintln!`. A device never grabbed drops silently. So the source grabs a keyboard as the last step before it keeps it, and the scan of other nodes prints nothing. A grabbed keyboard that goes away prints one line, `Failed to ungrab device: ...`, and the crate gives no safe way to skip it (OP-039). The source also gives `InputNotice::Lost`.

## The SDL event pump

SDL allows one event pump per process. In a window build the window keyboard owns it. `SdlGamepads` needs none: it turns off SDL joystick and gamepad events, so SDL queues none of them, and each `poll` calls `SDL_UpdateGamepads` and then reads each button and axis of each open gamepad. The same code runs on the device, where no video subsystem and no pump exist.

Every `sdl3::init` of the process must run on the thread of the first one. So make the window, the audio sink and this source on one thread, and poll this source there.

## Hotkeys

| Hotkey | Combination on one port | Fires |
| --- | --- | --- |
| Quick menu | Select + Start (`QUICK_MENU_COMBO`) | after `QUICK_MENU_HOLD_NS` (0.5 s) |
| Blind recovery | Select + Start (`BLIND_RECOVERY_COMBO`) | after `BLIND_RECOVERY_HOLD_NS` (5 s) |
| Quick menu | Start alone (`START_HOLD_BUTTON`), off by default | after the time of `set_start_hold_ns` |

- Each fires once per hold, at the first `update` at or after its hold time, not at the release. A release of one of the buttons ends the hold.
- A shorter hold of Select and Start fires nothing. On an arcade stick Select is the coin, and a coin and a Start in quick sequence overlap for a moment.
- The hold counts from the first `update` that sees the combination, so call `update` every frame, also with no event.
- A recovery hold opens the quick menu at 0.5 s and fires the recovery at 5 s. The frontend acts on the recovery also when the menu is open.
- `cancel_quick_menu()` ends every quick menu hold in progress with no menu. The session calls it when the menu closes, so a combination held across the close does not open the menu again. The recovery hold goes on.
- The rule is the same for every pad on every port, a generic pad with Select and Start, and the keyboard pair Right Shift and Enter.
- The quick menu hold is a constant, not a setting.
- The recogniser reads only the port state, so the control API and scripted input can trigger it with no picture.

### The Start hold

For an arcade panel with no Select button, Start held alone can open the quick menu. The setting `input.menu_hold_start_seconds` of `neptron-settings` holds the time in seconds. 0 is off and is the default.

The rule on each port:

- The hold starts at the first `update` that sees Start down and Select (`START_HOLD_BLOCKER`) up.
- It fires one `Hotkey::QuickMenu` at exactly the hold time, not one nanosecond before.
- A release of Start ends the hold.
- Select down on the same port ends the hold, and no new hold starts until Start is released. So Select and Start give only their own 0.5 s quick menu, also when Select goes up again while Start stays down.
- After a Start hold fires, Select that joins later gives no second quick menu until Start is released. The recovery hold on the same press still fires at 5 s.
- Other buttons, and every button on other ports, do not matter.
- A disconnect releases Start, so it ends the hold.
- A new hold time ends every hold in progress. The same time again changes nothing. A time set before the first `update` applies to a Start held from power-on.
- The blind recovery does not change.

What the app must do (OP-051):

- Read the resolved `input.menu_hold_start_seconds`, multiply it by 1 000 000 000, and pass it to `Hotkeys::set_start_hold_ns`, every frame or when the setting changes.
- The core sees Start down for the whole hold, so the game can act on it first (for example, it pauses). The recogniser changes no port state. When the hold fires, the app opens the quick menu, which pauses the core. Then the app gives the core Start up on that port until Start is released, so the core sees one release and no new press when it resumes.
- The 2 s of an arcade panel is a choice of the first-boot setup, which writes it to the settings. It is not a hidden default here or in the app: nothing tells a pad from a panel.

## Frame loop and latency

Poll every source once per frame, immediately before `retro_run`, into one buffer that the frame loop keeps. Apply the buffer to `PortStates` and call `Hotkeys::update` with the frame time. The core reads `PortStates` in that frame.

- An event waits in the evdev kernel buffer or in the SDL3 gamepad state until the next poll: at most one frame.
- A press and a release that both come between two polls are not seen by the core, which reads the state at the poll. The event stream has both.
- `poll` never blocks. A keyboard gives at most 8 reads per poll. The rest waits for the next poll.
- No allocation in the steady state: the backends keep their buffers, the mapper and the port state are fixed arrays, and the caller owns the event buffer. Two rare exceptions: the SDL3 joystick list and the `/dev/input` scan allocate a short list once per scan interval, and a notice allocates its text.

## Checks

No tier compiles the `sdl3` or `evdev` code. After a change, run:

```sh
cargo xtask clippy -p neptron-input --features sdl3
cargo xtask test -p neptron-input --features sdl3
```

The `evdev` code compiles only on Linux: `cargo xtask clippy -p neptron-input --features evdev` (add `--target <target>` on another host).

`examples/gamepads.rs` prints the events and notices of a real gamepad for 10 seconds: `cargo run -p neptron-input --features sdl3 --example gamepads`.

## Hardware checks

The `kms` build of the frontend runs on the i3-6100T and on a Pi 3 B, where games were played. No record names the input devices of those sessions. The generic layout, the mapping file, the silent pad rule, the map of the player, the capture and the guided mapping have run with scripted pads only. Not verified:

- A USB gamepad on the x86_64 image: found at boot and when plugged in later, buttons as in the gamepad map, a disconnect releases its buttons.
- A pad with no SDL3 mapping on the PC and on the Pi: the generic layout, its notice, and the silent pad notice (`docs/hardware-checklist.md`, "Input: every USB gamepad (M04b)"). Two pads of one unknown model keep their ports across a launch.
- The guided mapping of an unknown and of a known pad, and a gamepad read by the raw indices of its joystick (`docs/hardware-checklist.md`, "Input remapping (M05)").
- Two and four pads in a game, an unplug and a replug in a game, two pads of one model across a launch, and the node of a pad that stays plugged across a relaunch (`docs/hardware-checklist.md`, "Input (M04)").
- SDL3 without libudev finds a gamepad plugged in after start.
- A USB keyboard on the image: found, grabbed (no key reaches a console), mapped, and released when unplugged.
- Blind recovery from a gamepad and from a keyboard with no picture.
- The Start hold from an arcade panel encoder with no Select button.
- The combination held at boot on a keyboard encoder triggers recovery.
