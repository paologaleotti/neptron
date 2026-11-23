# Screensaver

This spec settles task M15. After a time with no input, the menus go to a dark picture with a small moving wordmark. It protects a CRT from burn-in. Any input brings the menus back.

## Facts that drive the design

- The UI draws into one 320x240 XRGB8888 canvas (`neptron-ui`, `UI_WIDTH` and `UI_HEIGHT`). The video engine scales that canvas to the mode on screen: the full active area on a CRT, a whole scale centred on a flat panel. Every screen of the UI goes through this one path.
- The UI crate has no clock. The app gives it a `Snapshot` and the actions of each frame. The app reads the clock once per frame.
- The navigator of the app (`apps/neptron/src/nav.rs`) turns input events into UI actions. `Navigator::hold_back` ignores each control held now until its release. A release gives no action.
- An `InputEvent` has the value 0 for a released button and for a stick inside its dead zone (`neptron-input` applies the dead zone before the event). A value that is not 0 is a press, or a stick out of its dead zone.
- In a paused game the session presents the last game picture under the UI canvas. On a CRT the canvas covers the whole active area. On a flat panel the game picture can show around the canvas.
- The menus play no sound. A paused game is silent. The only sound of the game list is the tone of a blind recovery.

## What shows

- A black screen. The word `NEPTRON` in the UI font (Spleen 6x12, one size), 42x12 pixels.
- The word moves on a diagonal. At each wall it bounces and takes the next colour of a fixed cycle of 8 colours.
- The colours are saturated, in the manner of the 1990s. No channel is above 0xBF (75 %), and no colour is white, to keep the load on a tube low. In cycle order: red `BF0000`, orange `BF5F00`, yellow `BFBF00`, green `00BF00`, cyan `00BFBF`, blue `005FBF`, violet `5F00BF`, magenta `BF00BF`. The cycle never gives the same colour twice in a row.
- The first colour is red. The word starts at the centre of its free area and moves right and down. No random source is used.

## Movement

The position is a pure function of the time since the screensaver started and the size of the screen. The UI screen is always 320x240; the function takes any size, and its tests use other sizes too.

- `free_x = screen_width - word_width` and `free_y = screen_height - word_height`. A free side of 0 keeps the word at 0 on that axis.
- The speed on each axis is 1/8 of the screen side per second (`SPEED_NUM / SPEED_DEN` screens per second): 40 px/s across and 30 px/s down at 320x240. So the picture looks the same at every resolution and every refresh rate.
- The distance on an axis is `elapsed_ns * side * SPEED_NUM / (SPEED_DEN * 1e9)`, in whole pixels, in 128-bit integers. It comes from the whole elapsed time each frame, so no remainder adds up and nothing drifts.
- The unfolded coordinate is `u = free / 2 + distance`. The position is a triangle wave of `u` with the period `2 * free`. The count of walls hit on that axis is `u / free`.
- The colour is entry `(walls_x + walls_y) mod 8` of the cycle. So the colour changes exactly in the frame in which the word reaches or passes a wall. A corner hits two walls in one frame and moves two steps; the colour still changes.
- The position is in whole pixels, so the text stays sharp at 240p on a CRT. At 320x240 the word moves 0 or 1 pixel in a frame.

## When it starts

The app keeps an idle timer (`Idle` in `neptron-ui`, pure). Each frame it gets the time, whether a wake input came, whether the screensaver may show, and the time setting.

The screensaver may show:

- in the game list and every menu screen of it,
- in a paused game: the quick menu and every screen opened from it.

It never shows while a game runs (the core runs, with no menu over it). It never starts while one of these shows:

- the confirm dialog of a signal change ("Keep the new picture?"), which reverts at its timeout,
- the guided mapping of a controller, which has a timeout and waits for presses.

While the screensaver may not show, the timer restarts. So the time counts from the last input, or from the moment the screensaver may show again (a game paused, a dialog closed). With the setting at Off the timer never fires.

The screensaver starts when the time since the timer restarted reaches the time setting: at it, not before.

If a dialog with a timeout opens while the screensaver shows (the app opens none by itself), the screensaver ends at once with no input to consume, and the dialog shows.

A notice with no timeout stays under the screensaver, and shows again after the wake. A new notice that comes under the screensaver waits there too.

## Wake

- A wake input is any input event with a value that is not 0: a button press or a key press on any controller or the keyboard, a d-pad press, or a stick out of its dead zone. A release is not a wake input.
- The wake input is consumed: the app sends the UI no action in that frame, and the screen under the screensaver shows as it was, with the same focus. A press of accept on a list row does not activate the row.
- At the wake the app calls `Navigator::hold_back`. A control that is still held gives no repeat and no action until its release. The release gives no action, so nothing leaks.
- The router of the UI also ignores every action while the snapshot says the screensaver shows. The rule then holds in the UI alone.
- The hotkeys of the input layer are not part of the UI. The blind recovery (Select and Start held for 5 s) still works under the screensaver: it is a safety rail. In a paused game, the menu hotkey of the wake frame is dropped with the other actions.

## Drawing

- The app puts the time since the screensaver started into the snapshot (`Snapshot::screensaver`). The router then draws the screensaver in place of every screen and overlay, with the screen id `screensaver` in the UI tree, and no rows.
- The screensaver changes no display mode. The canvas goes through the same present as every menu.
- In a paused game the app asks the session to show no game picture while the screensaver shows (`Session::hide_picture`). On a flat panel the frozen game picture then does not stay around the canvas.
- A frame allocates nothing: the scene buffers keep their memory, and the word is one text command.

## Background work

Work of the app goes on under the screensaver: the game scan, the writes of the outbox, the network reader, the picture answers of the page under it. Their notices wait for the wake.

## Sound

The menus play no sound, and a paused game is silent, so the screensaver stops no sound. The tone of a blind recovery plays as before.

## Settings

Two global settings on Settings > Display, after the scanline rows:

| Key | Label | Kind | Default | Levels | Safety |
| --- | --- | --- | --- | --- | --- |
| `display.screensaver` | `Screensaver` | bool, `On` or `Off` | `true` (On) | global | none |
| `display.screensaver_minutes` | `Screensaver time` | choice `1`, `5`, `10`, `30` | `5` | global | none |

- The options of `Screensaver time` read `1 minute`, `5 minutes`, `10 minutes`, `30 minutes`. Each option has the help line "The time with no input before the screensaver starts."
- The help text of `Screensaver` is in the doc comment of its key, as for the other toggle-like settings: "A dark picture with a moving NEPTRON after a time with no input, in the menus and in a paused game. It protects a tube from burn-in."
- Both apply live. A change of the time restarts nothing: the timer keeps its start, and the new time applies from the next frame.
- The keys go to the end of the schema: the key list is append-only.

## Tests

- The idle timer, with the simulated clock: it starts at the set time and not before; not in a running game; not with the setting Off; not while a confirm or a mapping shows; it restarts after input.
- The wake consumes the input: an accept on a row of the game list does not open the row.
- The bounce at 320x240, 640x480, 1920x1080 and 1920x240: the word stays inside the screen, the positions are whole pixels, the colour changes only in a frame that reaches or passes a wall, and it follows the cycle.
- The settings: the schema entries, a round trip through a file, the rows on the Display panel, and the crawler finds both.
- Scenes and goldens at 320x240 at three times: the start, and two later times with other positions and colours. The UI has one canvas size, so a larger screen is a test of the movement, not a golden.
- A tier 2 app test: idle in the menus for the set time, the screensaver shows; one press, the menu is back and unchanged; the press did nothing else.

## Hardware

Two checks go to `docs/hardware-checklist.md`: no missed field while the screensaver runs on the Raspberry Pi 3, and a look at the screensaver on a CRT.
