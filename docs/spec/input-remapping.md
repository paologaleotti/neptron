# Input remapping: the map of the player, per controller

The player can fix the layout of any controller: one that SDL3 does not know (the generic layout), and one that SDL3 or the mapping file knows but maps wrongly. One flow serves both. The map is kept per controller model in the settings folder, so it holds on every port and after a reboot. The port rules are in `docs/spec/input-players.md`. The map sources are in `docs/spec/input-generic-pads.md`; the map of the player is the first source.

## Facts that drive the design

- A pad has an identity (`DeviceIdentity`, the hash of the SDL GUID) and a path. Two pads of one model have the same identity.
- SDL3 reads every pad as a joystick: buttons, hats and axes by index. A gamepad mapping is a layer over these indices. The indices of a gamepad are stable for one driver: the guided mapping and the play read the same joystick.
- One run is one process. A game session owns the input source, and the app reads it only through a shared reference in a game. In the game list the app owns the source.
- The UI has no clock. The app sends the time left of a countdown in the snapshot, as for the confirm dialog of a picture.
- The frame thread writes no file. The writer thread writes the files of the frame loop.

## Words

- **Map of the player** ("your layout"): a map that the player made with `Map buttons`. Kind `MapKind::Player`.
- **Raw input**: a button index, a hat direction or an axis index of the joystick of a pad.
- **Binding**: the raw input that drives one RetroPad control.
- **Rest value**: the value of an axis when nobody touches it. A stick rests at the centre (0). Many triggers rest at one end (-32768).

## The sources of a map

The map of a pad comes from the first source that has one:

1. **The map of the player**, from `config/input/pads.toml`, by the identity of the pad.
2. The NEPTRON mapping file.
3. The community database (`SDL_GameControllerDB`, `/usr/share/sdl-gamecontrollerdb/gamecontrollerdb.txt`, Linux lines only).
4. The built-in database of SDL3.
5. The generic layout.

Source 1 is `PlayerMaps` in `neptron-input`. It implements `PadMaps` and falls back to `BuiltinMaps`. It gives `PadMap::Player(PlayerMap)` for any layout, a gamepad or a plain joystick. Sources 2 to 4 all make SDL3 report a gamepad mapping, so they give `Known`.

A pad gets only the bindings that its joystick has: `PadLayout` carries the counts of buttons, hats and axes of the joystick, a gamepad too, and `PlayerMaps` drops a binding whose index is past a count (`PlayerMap::fitted`). The source logs one `InputNotice::LayoutTrimmed` with the number dropped. So a map made on a pad of the model with more inputs never holds a control down on a smaller one: an index that the device does not have is not read.

## How a pad with a map of the player is read

A map of the player always reads raw indices (`RawInput::JoyButton`, `JoyHat`, `JoyAxis`), for a plain joystick and for a gamepad alike. So a map means the same thing for both kinds of pad, and a map made for a pad stays right if SDL3 later learns a gamepad mapping for it.

- `PlayerMap` holds one optional binding per RetroPad control. It is a fixed value (`Copy`), so the mapper keeps one per pad with no allocation.
- `PlayerMap::reads()` gives the counts of buttons, hats and axes that the backend reads (`RawReads`): one past the highest index in use. The backend reads only these indices.
- In the SDL3 source, `Device::with_map` pairs the open device with the map, with no wildcard:
  - a gamepad and `Known`: read the standard layout;
  - a plain joystick and `Generic`: read the indices of the generic table;
  - a gamepad and `Player`: read the indices of the map from the joystick of the gamepad. The source keeps the gamepad open too, so a reset to the default goes back to the standard layout with no reconnect;
  - a plain joystick and `Player`: read the indices of the map;
  - a gamepad and `Generic`, or a plain joystick and `Known`: refused.
- The source keeps the joystick of every pad open, a gamepad too. The guided mapping reads it (see below).

## A binding

| Binding | Drives | Rule |
| --- | --- | --- |
| Button `i` | a button control | down while button `i` is down |
| Hat `i`, direction `d` | a button control | down while hat `i` points in `d` (a diagonal sets two directions) |
| Half axis `i`, `+` or `-`, from the centre | a button control | down when the axis is at `MAP_AXIS_THRESHOLD` (16384, half travel) or past it, on that side |
| Half axis `i`, `+` or `-`, from an end | a button control | for an axis that rests at one end (a trigger): down when the axis has moved past the middle (0) towards the other end |
| Stick axis `i`, normal or inverted | an axis control (`LeftX`, `LeftY`, `RightX`, `RightY`) | the value through `STICK_DEAD_ZONE`, negated when inverted |

A raw input can drive more than one control: an axis can be the left stick and, through two half axis bindings, the D-pad.

## The guided mapping

`Map buttons` starts a guided mapping. Its rules are pure code (`GuidedMapping` in `neptron-input`). Its input is the raw state of one pad in each frame (`RawState`: every button, hat and axis of the joystick) and the time of the frame. Its output is the prompt and, at the end, the map.

### The order of the controls

1. Up, Down, Left, Right. The D-pad comes first, so the player can drive the menus with it soon.
2. A, then B: accept and back.
3. Start, Select.
4. Y, X, L, R, L2, R2, L3, R3.
5. Left stick right, left stick down, right stick right, right stick down.

So 20 steps. The prompt names the RetroPad control and its place: `Press Up`, `Press A (right face button)`, `Press L2 (left trigger)`, `Move the left stick right`.

### What counts

- At the start the source gives the first raw state of the pad. Its axis values are the rest values. An axis at -16384 or less rests at the low end, at 16384 or more at the high end, else at the centre. So a trigger that rests at full travel counts from its rest, and it never looks pressed.
- A **press** is a raw input that becomes active: a button down, a hat direction, an axis that moves `MAP_AXIS_THRESHOLD` (16384) from a centre rest, or past the middle from an end rest. The direction of the axis is recorded. An axis press ends only past `MAP_AXIS_RELEASE` (8192 from the centre), or back in the quarter of travel next to an end rest: an axis that shakes at the threshold gives one press.
- A button step takes a button, a hat direction or a half axis. A stick step takes only an axis that rests at the centre; the side of the move gives "inverted" (left or up for a prompt of right or down).
- When more than one input becomes active in one frame, the first one counts, in this order: buttons, hats, axes, each by index.
- The input counts when it is released before `MAP_SKIP_HOLD_NS` (2 s). The step then records it.
- **Skip**: an input of the pad held for `MAP_SKIP_HOLD_NS` or more skips the step. The control then has no binding. So a pad with few buttons skips what it does not have with any button. The prompt says it: `Hold a button of this controller to skip it.`, and the hint band `Hold: skip`. The prompt names no time: the skip time is a constant of the input crate, which the UI does not know.
- **Between steps** the pad must be neutral first: no button down, no hat direction, every axis back near its rest. So one press never fills two steps, and the press of accept that started the mapping does not count. When the pad stays held for `MAP_RELEASE_HINT_NS` (1 s) there, the screen says `Release every control.`
- **A control pressed twice**: the input is refused. The step stays, and the screen says `Up uses it already. Try another.` A swap would change a step that the player has passed without telling. A stick step and a button step do not conflict: an axis can be the stick and the D-pad.
- A button pressed in a stick step does not count. The screen says `Move a stick, or hold a button to skip.`

### Cancel

The player can cancel with no working button on the pad:

- **Back** from any other device: a keyboard, another pad, the control API. The pad of the mapping sends no UI action while the mapping runs, so every back comes from another device.
- **The timeout**: when the pad gives no press and no release for `MAP_IDLE_TIMEOUT_NS` (15 s), the mapping ends with no change. The screen always shows the countdown: `No input: cancel in 12 s.` Each press or release of the pad starts it again. The noise of an analog axis inside its threshold is no press, so it never keeps the mapping open.
- **The total time**: a mapping ends with no change after `MAP_TOTAL_TIMEOUT_NS` (300 s), also while the pad gives input. The countdown shows the nearer of the two times, so it shows the end of the total time near it.
- **The pad goes away**: the mapping ends with no change and a notice.

### The end: keep or cancel

After the last step the guided mapping gives the map. The app checks it:

- **The minimum**: the four directions, A (accept) and B (back). A map without one of them cannot drive the menus. The screen names what is missing and shows `Cancel` only: `This layout has no Left and no B. A controller needs the four directions, A and B.`
- **Start and Select**: a map without both can be kept, and the review says it: `Without Start and Select it cannot open the quick menu.` The keyboard or another pad opens the quick menu (the hotkeys fire on any port).
- The review says how many controls are set, and names the skipped ones (more than 6 are counted, not named): `18 of 20 controls are set. Skipped: Right stick X, Right stick Y.`

A map with the minimum goes to a trial, as a change of picture does:

- The dialog shows at once. The app gives the new map to the pad when the pad is neutral: until then the source still captures the pad, and the pad sends no UI action. A last step skipped by a hold ends with that input down. With the new map, that input could be A or B, and it would answer the dialog before the player sees it.
- The player tests the new map on the dialog.
- The dialog has `Cancel` (the focus) and `Keep`. Only right, then accept, keeps. So the trial checks the D-pad and accept of the new map.
- Back, accept on `Cancel`, or `MAP_REVIEW_TIMEOUT_NS` (15 s) with no answer, gives the old map back. The dialog shows the time left. A review that cannot keep shows `Cancel` alone and closes at the same timeout.
- `Keep` stores the map.

### The screen

The mapping shows in a box over the screen, like the confirm dialog, at 320x240. Every text fits a line of the box, or wraps in it.

```
Map buttons                        Step 3 of 20
+--------------------------------------------+
| USB Joystick                               |
|                                            |
|##############  Press Left  ################|
|                                            |
| [=====                                   ] |
| Up uses it already. Try another.           |
| Hold a button of this controller to skip it.|
| Back on a keyboard or another pad: cancel. |
| No input: cancel in 12 s.                  |
+--------------------------------------------+
Back: cancel                         Hold: skip
```

- The UI draws what the snapshot gives (`Snapshot::pad_mapping`): the name of the pad, the prompt, the step and the number of steps, the note, the time left, and whether it is the trial. It decides nothing about the content.
- Screen ids in the tree: `pad-mapping` and `pad-mapping/review`.
- While it shows, every action goes to it: back sends `Intent::CancelPadMapping`; the other actions do nothing. In the trial, the dialog takes the actions as the confirm dialog does, and sends `Intent::KeepPadMapping` or `Intent::CancelPadMapping`.

## Where the map is stored

The file is `config/input/pads.toml` under the settings folder (`/state/config` on the image). It is not a key of the schema: its keys depend on the controllers, so they cannot be a fixed list. It is TOML, written whole by the writer thread, as temp file, fsync, rename, fsync of the folder.

```toml
version = 1

[[pad]]
identity = "8f3a2b1c0d4e5f60"
name = "USB Joystick"
up = "h0up"
down = "h0down"
left = "h0left"
right = "h0right"
a = "b2"
b = "b1"
start = "b9"
select = "b8"
l2 = "a4+full"
left_x = "a0"
left_y = "-a1"
```

- `version` is the version of the layout of the file. A file with no version is version 1. A file with a higher version is not used: this build cannot know what its bindings mean. Every pad gets its default map, and a boot notice says so: `The file of the controller layouts has version 2, newer than 1. Its controller layouts are not used. A new layout starts a new file and keeps the old one as pads.toml.bak.` The player can still map and reset a pad. The first write of the run copies the newer file to `pads.toml.bak` (temp file, fsync, rename), then writes a new file of version 1. When the copy fails, the file is not written. So the newer file is never lost, and the player is never stuck with no way to fix a pad.
- `identity` is the identity of the pad, 16 hex digits. `name` is for the reader of the file.
- A control key holds its binding. A control with no key has no binding (it was skipped).
- The control keys: `b`, `y`, `select`, `start`, `up`, `down`, `left`, `right`, `a`, `x`, `l`, `r`, `l2`, `r2`, `l3`, `r3`, `left_x`, `left_y`, `right_x`, `right_y`.
- The binding texts: `b3` (button 3), `h0up`, `h0right`, `h0down`, `h0left` (hat 0), `a2+` and `a2-` (a half of axis 2 from the centre), `a4+full` and `a4-full` (axis 4 from its end), `a0` and `-a0` (a stick axis, normal or inverted). A stick text is valid only for an axis control, and the other texts only for a button control.
- **Append-only.** A key is never renamed or removed. A new key is added, and an older build ignores it. An entry with a damaged binding loses that binding only, with a log line. An entry with no valid identity is skipped. A file that does not parse, whose `pad` key is not a list, or whose version is not a number, reads as no map, with a notice, and the app refuses to write it, so the file of the player is never lost. While the file cannot be written, `Map buttons` and `Reset to default` are disabled with the reason `File cannot be written`, and `Set up this controller` opens the Input panel.
- An identity is compared as a value: an entry written by hand in capitals is the same identity. When the file has more than one entry of an identity, the last one is the map in memory; `Keep` replaces that entry, with its unknown keys, and drops the others; `Reset to default` removes them all.
- The text of the file is made before anything changes. When it cannot be made, the layout is not kept, nothing is written, and the player gets a notice.
- **Room for per-game and per-system maps.** A later build adds the keys `system` and `game` to a `[[pad]]` table. This build skips an entry with either key, so it never applies a per-game map to every game.
- A write starts from the file as the app read it at the start of the run. It replaces or removes only the `[[pad]]` table of the one identity with no `system` and no `game` key, and keeps every other table and every unknown key as they are. In the table that it replaces, it keeps the unknown keys too. So a newer key survives a write of an older build. Comments are not kept.

### The flow of the stored maps

- The app reads the file once, at the start of each run, before the first poll of the input source, and gives the maps to the source (`PadRoster::set_player_maps`). So the maps hold in the game list and in every game.
- `Keep` adds or replaces the map of that identity, gives the maps to the source, and queues the write. `Reset to default` removes it, gives the maps to the source, and queues the write.
- The source gives each open pad of that identity its new map at the next poll: the mapper releases every control the pad holds, and the next poll reads it with the new map. So a change takes effect at once, in the menus, and the next game (a new run) reads the file.
- The trait `PadRoster` of `neptron-input` carries it: `set_player_maps(maps)`, `capture(Some(key))` and `capture(None)`, `captured()` (`Capture::Waiting`, `Capture::Pad(RawState)` or `Capture::Gone`), and `pad_map(key)`, the map that the source reads a pad with, for the page. A source with no pad ignores them.
- Two pads of one model share the map.

## The Input panel and the page of a controller

- A player row and an extra pad row of a present pad open the page of that pad. The row keeps its text (`USB Joystick (generic layout)`) and gets the arrow of a row that opens a page. Its row value is `SettingValue::Link { page, text }`: an information text and a page. A row with no pad, or of an away pad, stays an information row. The arrow takes the room of one character, so the text of such a row is cut to 43 characters with its label (`LINK_ROW_MAX_CHARS`).
- The suffix of a row: none for a known pad, ` (generic layout)` for the generic layout, ` (your layout)` for a map of the player.
- The page of a controller is a page of the app, one page id per port (`pad_page(port)`). Its title is the label of the row (`Player 1`). Its rows:

| Row | Kind | Text |
| --- | --- | --- |
| `Controller` | information | the name of the pad |
| `Layout` | information | `Known`, `Generic` or `Yours` |
| `Map buttons` | action | starts the guided mapping |
| `Reset to default` | action, asks first | removes the map of the player |
| one row per RetroPad control | information | the binding: `Hat 0 up`, `Button 2`, `Axis 4 + (from its end)`, `Axis 1 inverted`, `South button` for a known pad, `None` for a control with no binding |

- `Reset to default` is disabled with the reason `Already default` when the pad has no map of the player. It asks first: `Reset this controller?`, `Your layout of this model is removed. The default layout comes back.`
- When the pad of the port goes away while the page is open, the page shows `Controller` `None`, and the actions are disabled with the reason `No controller`.

## In a game

The simplest correct rule: **the mapping runs only in the game list.** In a game the source belongs to the session, and the core reads the pad: a change of map under a paused core, and a pad that sends nothing to the menu, are both risks for no gain.

- In a game the Input panel and the page of a controller show as in the game list, read only: `Map buttons` and `Reset to default` are disabled with the reason `Not during a game`.
- The notice of a generic pad in a game: `Set up this controller` opens the Input panel on the row of the pad (`docs/spec/input-generic-pads.md`).

## Set up this controller

In the game list, the button `Set up this controller` of the notice of a generic pad starts the guided mapping of that pad at once: the app finds the pad by its identity and path in the roster. When the pad has no port any more, the Input panel opens on its first row. In a game, see above.

## The notices of a pad with a map of the player

A pad with a map of the player gets no notice of the generic layout: its map is the choice of the player. A pad in a guided mapping counts as heard (`Mapper::mark_heard`), so it never gets the notice of a pad that sends no input. The other notices do not change.

## The minimum of a map and the quick menu

The rule of the minimum holds for every map. The guided mapping never keeps a map without it. A map from the file can lack one: a hand edit, a damaged binding, or a pad with fewer inputs than the pad of the mapping. The source then gives that pad its default map (`PlayerMaps` gives `BuiltinMaps`), so a pad alone can always drive the menus. The entry stays in the file. A boot notice says it: `A controller layout has no direction, A or B. Its controller uses its default layout.`

## Tests

- Pure, `neptron-input`: the binding texts, both ways, and the damaged ones; each kind of binding in the mapper (button, hat, half axis from the centre and from an end, stick normal and inverted); `PlayerMap::reads`; `PlayerMaps` before `BuiltinMaps`; the guided mapping: the order, a hat, an axis with its direction, a trigger that rests at full travel, a skip by hold, a refused second press, a button in a stick step, the neutral wait between steps, the timeout and its countdown, the end with the map; the minimum and the Start and Select warning.
- The scripted pads and the mapper: a new map takes effect at once, with the release of the held controls; the capture of a pad: its controls go up, it sends no event, and the raw state reads all its indices; a gamepad with a map of the player reads raw indices.
- App: the file, both ways, with an unknown key and a skipped entry kept; a damaged file is never written; the Input rows and the page of a controller; the actions disabled in a game.
- UI: the overlay takes every action; back cancels; the trial dialog; scenes and goldens.
- Tier 2, with scripted pads: a generic pad mapped through the guided flow, then it plays with the new map; a known pad remapped; skip by hold; cancel from the keyboard; the timeout; reset; the map survives a relaunch and a new start of the app (the file); two pads of one model share the map; `Set up this controller` starts the mapping.
- Scenes and goldens: `screens/pad-mapping` (the prompt with progress and countdown), `screens/pad-mapping-note` (a refused press), `screens/pad-mapping-review` (keep or cancel), `screens/pad-mapping-incomplete` (cancel only), `screens/pad-page` (the page of a controller), and `screens/input` gets a row with ` (your layout)`.

## Hardware checks

In `docs/hardware-checklist.md`, section "Input remapping (M05)", labelled `needs-hardware`: the flow on the PC and on the Pi with a real unknown pad and with a known one, on a 240-line CRT picture; a trigger that rests at full travel; cancel from the keyboard; the map after a reboot. No hardware result is claimed.

## Open questions

| Question | Default | Where | How to change |
| --- | --- | --- | --- |
| Remap during a game | No: the page is read only in a game | `apps/neptron`, `pages.rs` | Let the session give the app the source mutably, then allow it with the core paused |
| A press used twice | Refused | `guided.rs` | A swap rule in `GuidedMapping` |
| The times | 2 s to skip, 15 s with no input, 300 s in total, 15 s for the trial | `guided.rs`, `shell/mapping.rs` | The constants |
| A map per game or per system | Not built; such an entry is skipped and kept | `config/input/pads.toml` | Add the keys `system` and `game` and a source per level |
| The map is a schema key | No: a file of its own in the settings folder | `apps/neptron`, `pad_maps.rs` | None needed |

## Known limits

- Two pads of one model whose paths are both empty (SDL3 gives no path) have one key. The capture, the page and the button of a notice then take the first of them.
- The UI crawler reaches the prompt of the guided mapping and leaves it with back; it does not reach the review dialog. The tier 2 tests of `remapping.rs` cover the review.

## Not built

- The picker of the pad of each player.
- Maps per game or per system, turbo, macros, pressure-sensitive buttons, multitap.
- A swap of two bindings in the guided mapping.
