# neptron-input

Input: SDL3 gamepads, evdev keyboards, RetroPad mapping and hotplug.

## Layers

1. **Device backends** read devices and decide nothing. `SdlGamepads` (feature `sdl3`) and `EvdevKeyboards` (feature `evdev`, Linux only) turn what they read into `RawInput` values (button, axis, hat, key) for a device id.
2. **The mapper** (`Mapper`, pure) gives each device its port and device id, turns raw input into `InputEvent`s with the mapping tables, and releases every held control on a disconnect.
3. **The port state** (`PortStates`, pure) merges the events of every source into the RetroPad state of each port.
4. **The hotkeys** (`Hotkeys`, pure) read the port state and a time that the caller passes in.

The tests drive layers 2 to 4 with scripted devices and scripted input, with no hardware and no sleeps.

## Public API

- `Mapper`: `connect_gamepad(identity)`, `connect_keyboard()`, `disconnect(device, out)`, `apply(device, raw, out)`. `MapperError`.
- `PortStates`: `apply(events)`, `value(port, control)`, `is_held(port, control)`.
- `Hotkeys`: `update(ports, now_ns, out)` gives `Hotkey::QuickMenu { port }` and `Hotkey::BlindRecovery { port }`.
- `port_of(device)`, `DeviceIdentity`, the id ranges and limits.
- The mapping tables and rules: `PAD_BUTTON_MAP`, `pad_control`, `axis_target`, `stick_value`, `trigger_value`, `hat_values`, `KEY_MAP`, `key_control`, `looks_like_keyboard`.
- `InputNotice`: what a backend tells the frontend to log.
- Feature `sdl3`: `SdlGamepads: InputSource`, with `open()` and `take_notices(out)`. `SdlInputError`.
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
- A joystick that SDL3 has no gamepad mapping for is ignored, with a notice that gives its GUID. Guided mapping comes after the MVP.

## Keyboard map

The defaults of the RetroArch keyboard driver for player one, by physical key (Linux key codes). Every keyboard plays on port 0. A key repeat gives no event.

| Keys | RetroPad |
| --- | --- |
| Arrows | D-pad |
| X, Z, S, A | A, B, X, Y |
| Q, W | L, R |
| Enter | Start |
| Right Shift | Select |

`neptron-window` has the same defaults by SDL scancode. The shared fixture `tests/fixtures/retroarch-keyboard.txt` lists the pairs by RetroArch key name, and a test holds `KEY_MAP` to it. The window table is not tested against the fixture yet (see open points).

The frontend actions of a keyboard come from the hotkeys: Right Shift and Enter together open the quick menu.

A key held before a keyboard opens gives no kernel event. So when the source opens a keyboard, it reads the key state and sends a press for each mapped key that is down (`mapped_keys_down`). A combination held from power-on, or held while a keyboard opens again after a read error, reaches the hotkeys.

## Device ids and ports

| Ids | Devices | Port |
| --- | --- | --- |
| 0 to 7 | Direct ids: the window keyboard (id 0), scripted input, the control API | the id |
| `0x100` to `0x107` | SDL3 gamepads | the id minus `0x100` |
| `0x200` to `0x207` | evdev keyboards | 0 |

`port_of(device)` gives the port. Ids outside the ranges have no port and `PortStates` ignores them. The window keyboard keeps its id 0 and needs no change.

## Ports and hotplug

- A new gamepad takes the port that a device with the same identity had last, when that port is free, and the lowest free port otherwise. So a new pad becomes player one when player one's pad is gone.
- A disconnect frees the port at once. The mapper gives the release of every control that the device held, so no button stays down.
- The identity of a gamepad is its SDL GUID (vendor, product, version). Two pads of one model are then the same identity: when both reconnect, they can swap ports. The source does not read the serial number, because the `sdl3` crate panics on a serial that is not UTF-8.
- At most 8 gamepads and 8 keyboards. A ninth gamepad is ignored, with a notice, until it reconnects.
- A port merges its devices: a button is down while any device holds it, an axis takes the value with the largest magnitude.

**What hotplug relies on.** `SdlGamepads` calls `SDL_UpdateGamepads` in each poll, which runs the device detection of SDL3. The source asks SDL3 for the joystick list every `SCAN_INTERVAL_POLLS` (30) polls and checks each open gamepad for a disconnect in every poll. With no libudev, SDL3 on Linux finds devices through inotify on `/dev/input`, or by a scan of that folder when inotify is not there. `EvdevKeyboards` scans `/dev/input` every `KEYBOARD_SCAN_INTERVAL_POLLS` (120) polls. Both rely on `mdev` making the nodes in `/dev/input` with read access for the frontend. This is unverified on the target until the image runs it.

**A line on stderr when a keyboard is lost.** The drop of an `evdev::Device` ungrabs it, and evdev 0.13.2 prints the error of that ungrab with `eprintln!`. The drop of a device that was never grabbed is silent: the crate then makes no call. So the source grabs a keyboard as the last step before it keeps it, and the scan of nodes that are not keyboards prints nothing. A grabbed keyboard that goes away (unplugged, read error) prints one line, `Failed to ungrab device: ...`, because the ungrab of a removed device fails and the crate gives no safe way to skip it. The source also gives an `InputNotice::Lost` for it.

## The SDL event pump

SDL allows one event pump for each process. In a window build the window keyboard owns it. `SdlGamepads` needs none:

- It turns off SDL's joystick and gamepad events, so SDL queues none of them, and the window's pump does not read the devices.
- Each `poll` calls `SDL_UpdateGamepads`, which reads the devices, and then reads the state of each button and axis of each open gamepad.

The same code runs on the device, where no video subsystem and no pump exist, and in a window build. With the `sdl3` crate, every `sdl3::init` of the process must run on the thread of the first one, so make the window, the audio sink and this source on one thread, and poll this source there.

## Hotkeys

| Hotkey | Combination on one port | Fires |
| --- | --- | --- |
| Quick menu | Select + Start (`QUICK_MENU_COMBO`) | when the combination goes down |
| Blind recovery | Select + Start (`BLIND_RECOVERY_COMBO`) | when it has stayed down for `BLIND_RECOVERY_HOLD_NS` (5 s) |

- Each fires once per hold. A release starts the hold time again.
- The hold counts from the first `update` that sees the combination, so call `update` every frame, also with no event.
- A recovery hold opens the quick menu first. The frontend acts on the recovery even when the menu is open.
- The recogniser reads the port state only. It needs no picture and no UI, and the control API and scripted input can trigger it.

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
- SDL3 without libudev finds a gamepad plugged in after start (inotify on `/dev/input`).
- A USB keyboard on the image: found, grabbed (no key reaches a console), mapped, and released when unplugged.
- Blind recovery from a gamepad and from a keyboard with no picture.
- The combination held at boot on a keyboard encoder triggers recovery.

## Must never depend on

- `neptron-kms`, `neptron-ui`, `neptron-video`, `neptron-window`
- SDL3 or evdev outside their features

## Rules

- `unsafe` is forbidden: the crate root carries `#![forbid(unsafe_code)]`. The crate is on the `unsafe` allow-list, but the safe APIs of `sdl3` and `evdev` are enough.
- Features: `sdl3`, `evdev`. No feature is on by default.

## Status

Complete for the MVP. The SDL3 path compiles and passes clippy and its tests on macOS; no gamepad has run it. The evdev path compiles only on Linux: the review built it on aarch64 Linux in a container (check, clippy with warnings denied, tests with `--features evdev`). No keyboard has run it.
