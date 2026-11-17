# Input for four players: ports, order, hotplug and Settings > Input

This spec settles task M04. It decides how a gamepad gets its port, what happens when a gamepad goes away and comes back, what the menus read, and what `Settings > Input` shows. It does not change the gamepad map, the keyboard map or the hotkeys (`crates/neptron-input/README.md`).

## Facts that drive the design

- `docs/definition/09-input.md` names SDL3 for gamepads, evdev for keyboards, hotplug with no udev daemon, and "per-player assignment" for arcade encoders. It says nothing about the order of gamepads, a gamepad that goes away in a game, more pads than players, or a settings panel. Those answers are defaults of this spec (see "Open questions").
- **One run of the frontend is one process: the menus, or one game.** A launch writes the relaunch file (`/run`, RAM) and exits; the launcher starts the next run, which opens the gamepads again. A return to the menus does the same. So the order of the pads must cross the relaunch file, or each run gets the order in which SDL3 lists the devices.
- The libretro host answers `input_state` for 4 ports (`neptron_libretro::MAX_PORTS`). The session gives the core ports 0 to 3 each frame. After the load the host sets `RETRO_DEVICE_JOYPAD` on ports 0 and 1 only.
- The mapper of `neptron-input` has 8 gamepad slots (`MAX_PORTS` = 8). A gamepad device id holds its port (`0x100 + port`). The port state (`PortStates`) and the hotkeys follow all 8 ports. Every keyboard plays on port 0.
- The menus take a UI action from any device (`Navigator`), and the quick menu hotkey and the blind recovery fire on any port.
- The identity of a gamepad is a hash of its SDL GUID (vendor, product, version). Two pads of one model have the same identity. The source does not read the serial number: the `sdl3` crate panics on a serial that is not UTF-8 (OP-039).
- SDL3 gives the path of a gamepad (`SDL_GetGamepadPath`). On Linux it is the event node, `/dev/input/eventN`, or, with the HIDAPI drivers of SDL3, the hidraw node, `/dev/hidrawN`. The kernel keeps the node while the device stays plugged, so the path tells two pads of one model apart for as long as they stay plugged, also across a relaunch. A replug can give a different node.

## Words

- **Player port**: ports 0 to 3. The core reads them. The UI names them `Player 1` to `Player 4`.
- **Extra port**: ports 4 to 7. The core never reads them. A pad on an extra port drives the menus and the hotkeys only.
- **Held port**: in a game, the port of a pad that went away. It waits for that pad.
- **Pad key**: what the port rules compare: the identity (GUID hash) and the path (hash of the SDL path).

## The order: plug order

A pad gets its port in the order in which it comes: the first pad found is player 1, the next one player 2, and so on. At boot, the order is the order in which SDL3 lists the pads.

Why not the first input ("press a button to join"): a pad that nobody touches would have no port, the core would see no player 1 until a press, and the panel would need a join screen. Plug order needs no screen, and the player can change it (see "Changing the order").

Why not a fixed rule (for example by USB socket): the USB topology is not known to SDL3 on every target, and a socket rule fails for a hub or a 2.4 GHz dongle with two pads.

## The rules

### In the menus (a run with no game)

1. A new pad takes the lowest free port.
2. A pad that goes away frees its port at once. Every pad on a higher port moves down by one, in its order. So the ports in use are always 0 to n-1, and the first pad is always player 1.
3. A pad that comes back is a new pad: it takes the end of the list.
4. A pad that moves gets the release of every control that it held on its old port. The next poll gives its controls again on the new port. A move happens only in the menus, so a running core never sees a pad change its port.
5. A hotkey held across a move starts again on the new port: the blind recovery hold counts its 5 s from the move. The release on the old port is the end of a short Select and Start press there, so the quick menu hotkey of that port fires; in the menus that hotkey does nothing.

### In a game (a run with a game)

1. A pad that goes away keeps its port: the port is held for its pad key. The release of every control that it held comes out at once, so no button stays down.
2. A pad that comes takes, in this order: a held port with the same identity and the same path; else a held port with the same identity; else the lowest free port that is not held. Among held ports that match, the lowest wins.
3. Nothing moves in a game. A held port stays held until the run ends, also while the quick menu is open.
4. So the game never loses player 1 to another pad: a new pad of another model takes a free port above the held one, or an extra port.

### Across runs

- The run that ends puts its port list in the relaunch file: for each port, the identity, the path and the name of the pad, and whether it is present.
- The next run starts with every entry of the list as a held port. Its first scan connects the pads, which take their held ports by the rule above. A pad of one model on two ports keeps its port when its path is the same, which it is while it stays plugged.
- A scan connects the pads it found as one batch: the pads whose identity and path match a held port connect first, then the others in the order of SDL3. So a pad of the same model at a new path cannot take the port held for a pad that comes later in the same scan.
- A menu run then drops every held port that no pad took, and the pads move down (rule 2 of the menus). A pad of the first scan that found every port in use or held then tries again, so the held ports of the run before never refuse a pad of this run. A game run keeps them held: a pad that was there in the menu and is not found at the start of the game keeps its port.
- A run with no relaunch file (a boot) starts with no list.

### More than four pads

- Pads 5 to 8 take the extra ports. They drive the menus, the quick menu and the blind recovery. The core does not read them.
- In the menus, an extra pad moves down to a player port when a player pad goes away (rule 2).
- A ninth pad gets no port. The source ignores it with a notice until it is unplugged and plugged again (OP-039, unchanged).
- With held ports, the 8 ports can all be in use or held. A pad that comes then is a ninth pad.

### Two pads of the same model

- In one run, and across a relaunch with both pads plugged, each keeps its port: the path tells them apart.
- When both go away in a game and come back, the first to come back takes the lower held port. The two can swap. This is a limit (OP-039, changed text): only a serial number would tell them apart.

## Who controls what

- **The menus**: every device, on every port: each pad, extra pads too, every keyboard, the window keyboard, the control API. This is the behaviour today, and it stays. A couch with four pads needs no "pad 1 only" rule, and an arcade panel can wire its player 2 controls to any port.
- **The quick menu**: the quick menu hotkey on any port opens it, and every device then drives it. The rule "the menu is never a toggle" stays.
- **The blind recovery**: any port, as today.
- **The keyboard**: every keyboard plays as player 1 (port 0), together with the pad on port 0. A button of the port is down while any device holds it. A keyboard never takes a pad port and never moves a pad. Per-player keyboard encoders (`09-input.md`, "Keyboard-as-joystick profiles, per-player assignment") come later.

## The core

- The host sets `RETRO_DEVICE_JOYPAD` on all 4 ports after the load, as RetroArch does for each player it has. Today it sets ports 0 and 1 only, and some cores read a port only after the frontend has set its device.
- A multitap (SNES, PC Engine) or a four-player adapter (NES Four Score, Mega Drive 4-Way Play) is a device or an option of the core. This task sets no such device and no core option (see "Open questions").

## Changing the order

The design asks for no order screen. The order changes with the cable: in the menus, unplug a pad and plug it in again, and it goes to the end of the list. With two pads, unplugging player 1 for a moment swaps players 1 and 2. `Settings > Input` shows the result at once.

A screen that picks the pad for each player is a later step (see "Open questions").

## Settings > Input

`Settings` gets a panel `Input` (`INPUT_PAGE`), after `Audio`: Display, Audio, Input, Library, Network, System. It shows in the generic page screen, at the global level, from home and from the quick menu. Screen id: `input`.

| Row | Label | Kind | Text or value |
| --- | --- | --- | --- |
| Player | `Player 1` to `Player 4` | information | the name of the pad, or `<name> (away)` for a held port, or `None` |
| Extra pad | `Extra pad` | information | the name of the pad. One row for each pad on an extra port, in port order; none when there is none |
| Keyboard | `Keyboard` | information | `Player 1` |
| Hold Start | `Hold Start for menu` | value | the setting `input.menu_hold_start_seconds`, as today |

- The four player rows are always there, so the panel has the same shape with no pad, and the crawler finds it on every rig.
- `Hold Start for menu` moves from the Display panel to this panel. It is an input setting; the Display panel held it only because no input panel existed. The key, the schema entry and the levels do not change.
- The panel reads the port list of the input source when it opens, and again when the list changes while it is on screen. A pad plugged in while the panel is open shows in the next frames.
- A pad with the generic layout shows ` (generic layout)` after its name (`docs/spec/input-generic-pads.md`).
- The name is the device name that SDL3 gives the pad, read through the joystick API: the gamepad API of the `sdl3` crate panics on a name that is not UTF-8, and such bytes become the replacement character. A character that the font has no glyph for (it has ASCII and the Latin-1 Supplement) shows as `?`, so `é` stays `é`. A name that does not fit is cut by the app with `...` so that the row fits: `Player 1`, one space and the text hold 44 characters, with a scroll bar. So the name has at most 35 characters, and 28 before ` (away)`. The tree row is then not clipped, and the crawler does not fail on a long name.
- Every player row is a row of its own key, one for each port. The row of a present pad opens the page of that pad (M05, `docs/spec/input-remapping.md`), with no change of the layout.

Example, in a game, with player 2 unplugged:

```
Player 1                      Xbox 360 Controller
Player 2                 8BitDo SN30 Pro (away)
Player 3                                     None
Player 4                                     None
Keyboard                                 Player 1
Hold Start for menu         0 s          default
```

## The input source

- The port rules are pure code in `neptron-input`: one table of ports (free, playing, held) and the rules above. The mapper uses it. The device backends decide nothing: they connect, disconnect and read.
- The SDL3 source passes the pad key and the name on connect. It reads the path with `Gamepad::path` (every path that SDL3 makes is ASCII, so the UTF-8 unwrap of the crate cannot fail there) and the name with `Joystick::name`, which converts lossy. SDL types stay in `sdl.rs`.
- A source of pads gives the frontend its port list (the "roster") and a change counter, through a trait of `neptron-input`. Sources with no pads (the window keyboard) give an empty list.
- The source takes, when it opens, the rule of the run (menus or game) and the list of the run before.
- A scripted pad source in `neptron-input` runs the same mapper and port table with no hardware. The tier 2 tests use it to plug, unplug and press pads.
- The relaunch file gets one optional field with the port list. The version stays 1: an old file with no list reads as "no list", and the file lives in RAM, so no file of an older build survives a reboot.

## Tests

- Pure, `neptron-input`: plug order; unplug and compaction in the menus, with the release of the moved pads; unplug and hold in a game; a replug takes its held port; a pad of another model does not take a held port; five pads (the fifth on an extra port, and in the menus it moves down); nine pads (the ninth refused); two pads of one model: kept apart by the path, and the swap when both replug with a new path; the list of an earlier run restored in a menu run (missing pads dropped, the rest moved down) and in a game run (missing pads held); a scan that gives each pad of one model the port held for its path, whatever the order of SDL3; a first scan of the menus that frees held ports before it refuses a pad; the hotkeys from ports 4 to 7 and from a keyboard; a recovery hold that starts again after a move; a name that is not UTF-8 and a name that is not ASCII.
- Libretro host: the fake core sees `RETRO_DEVICE_JOYPAD` on ports 0 to 3.
- App: the Settings > Input rows for no pad, four pads, a held pad, extra pads and a long name; the relaunch file with and without the list.
- Tier 2, with scripted pads: two pads in the menus, then a launch: the relaunch file holds both in order, and the game run gives each its port. In the game, player 1 unplugs, a third pad comes and gets port 2, and player 1 comes back on port 0; the core reads each press on its port. The panel shows the pads and an away pad. The crawler finds the panel.
- Scenes and goldens: `screens/input` (four players, one away, an extra pad) and `screens/input-empty` (no pad). The Settings page and the Display panel scenes change: the Settings page has one more row, and the Display panel has no `Hold Start for menu` row.

## Hardware checks

Added to `docs/hardware-checklist.md`, section "Input (M04)": two pads and four pads in a game, an unplug and a replug in a game, two pads of the same model across a launch, a fifth pad, and the order after an unplug in the menus. No hardware result is claimed.

## Open questions

Each one has a default that this task builds. OP-110 records the decisions of the owner on them: plug order stays, every pad controls the menus, more than four pads stay as built, a picker for the pad of each player is task M05b, and four-player adapters stay open.

| Question | Default | Where | How to change |
| --- | --- | --- | --- |
| How does a pad get its port? | Plug order; at boot, the order of SDL3 | `neptron-input`, port table | A first-input rule in the port table |
| Can the player pick the pad of each player in the UI? | No; the order changes by a replug in the menus | `Settings > Input` | Make each player row a choice of the pads, or a page (M05) |
| Can a new pad take a held port in a game? | No, never in that run | port table | An action in the quick menu that frees the held ports |
| Who controls the menus? | Every device | `nav.rs` of the app | Filter the events by port in the navigator |
| Multitap and four-player adapters | Not set; `RETRO_DEVICE_JOYPAD` on ports 0 to 3 | libretro host, per-core data | A device per system in the core data, set after the load |
| Keyboard per player | Every keyboard is player 1 | `neptron-input`, keyboard map | Per-player keyboard profiles |
| Two pads of one model that both replug in a game | They can swap | port table, SDL source | Read the serial number when the crate stops panicking on it |

## Not built

- A UI to pick the pad of each player, to remap a pad (M05), or to name a pad.
- Pads with no SDL3 mapping: M04b opens them as joysticks with a generic layout (`docs/spec/input-generic-pads.md`). The port table takes them with no change, by their pad key.
- Light guns, mice, wheels and per-player keyboards.
- Rumble.
