# neptron-ui

The sealed UI: snapshot in, scene and intents out. An XRGB8888 canvas of one form (landscape 320x240 or portrait 240x320), a bitmap font, five widget templates, the scene registry and the screens.

## Must never depend on

- Any other NEPTRON crate
- Any hardware crate

Only the app and `xtask` (for `shot`, `gallery` and `golden`) depend on this crate.

## Rules

- `unsafe` is forbidden: the crate root carries `#![forbid(unsafe_code)]`.
- No I/O, no clock, no threads. The golden module works on bytes; the caller reads and writes files.
- No features.
- Dependency: `thiserror` only. A small parser in `skin.rs` reads the skin file, not the `toml` crate.

## Public API sketch

- `Snapshot` (module `snapshot`): what the UI shows. The systems, a `GameWindow` of one game list, a `SettingsPage` of one page (`MenuItem` with a typed `SettingValue` and its source `Level`), the monitor profiles, the display, a `PendingConfirm`, the session, the notices, the `VideoStatus` of the running game, a `PadMapping` and the screensaver frame count. The app fills it. The snapshot and its entry structs are `#[non_exhaustive]`: start from `Snapshot::default()` and the `new` constructors.
- `SettingValue`: `Toggle`, `Number`, `Choice` (of `ChoiceOption`), `Action(ActionId)`, `Page(PageId)` (a row that opens a page), `Text(String)` (an information row) and `Link { page, text }` (an information text on a row that opens a page). `MenuItem::with_confirm(question, message, working)` gives an action row a confirm question; `MenuItem::unavailable(reason)` disables a row and shows the reason in place of its value.
- `ChoiceOption::new(label).with_detail(..).with_short(..).with_help(..).unavailable(..)`; `From<&str>` makes a plain option. `short` is the text of the value column of a settings row; the option list always shows the label. `help` replaces the detail in the help line under the option list.
- `UiAction`: up, down, left, right, accept, back, menu.
- `Intent` and `Value`: what the app must do. `Intent` is exhaustive on purpose: a new intent breaks the build of the app until the app handles it.
- `UiFrame`: the output of one frame. `scene: UiScene`, `tree: UiTree` (rows for `ui tree` of the control API, with a `RowKind` on each row of a list) and `intents: Vec<Intent>`. Make one at launch and reuse it; `clear()` keeps the memory.
- `UiScene`: the four primitives `rect`, `sprite`, `text_run` (or `text_fmt`) and `clip`. `render(&mut impl Canvas)` replays them.
- `metrics`: `Form` (`Landscape` 320x240, `Portrait` 240x320), `Size` (only the size of a form can be built; `Size::new` refuses any other with `SizeError`) and `Metrics`. `Metrics::of(form, theme)` computes once, with no I/O, the numbers of a layout: the canvas, the safe area (5 % of each axis on each side), the title band, the hint band, the content area, the row height and the padding, the characters on a line of a framed box (`text_chars`) and of a dialog (`dialog_chars`), the dialog column, and the line counts of the game title box, the option help box and the video block. `Metrics` is a small `Copy` value. The router holds one and passes it to every screen and to the frame helpers, and no rectangle of the layout comes from a fixed canvas size. The line counts of the boxes and the caps of `List` and `Grid` still come from constants of their modules; package L2 reads them from `Metrics`. Only the landscape form is in use; the portrait value is ready for the portrait layout (`docs/spec/ui-layout.md`).
- `trait Canvas` and `CpuCanvas`: `CpuCanvas::new()` is landscape, `CpuCanvas::with_size(size)` takes the size of a form, and `size()` returns it. `CpuCanvas::as_bytes()` is XRGB8888, little-endian, pitch 4 bytes times the width (1280 in landscape), ready for the `ui` frame of a `Scene` in `neptron-types`. A fill writes one row and copies it to the other rows: a loop over the pixels took 5.5 ms a frame for the quick menu on a Raspberry Pi 3.
- `SceneCanvas`: a `CpuCanvas` and the scene on it. `new()` is landscape; `with_size(size)` takes the size of a form. `paint(&UiScene) -> bool` draws only a scene that differs from the scene on the canvas, so a static UI costs one comparison per frame.
- `font`: Spleen 6x12, printable ASCII and Latin-1, 6 px advance, capitals 8 px tall. Other characters draw `?`.
- `text`: `fit`, `draw_in` (cut with `...`, aligned, reports clipping), `wrap`, `StackText` (formatting with no allocation), `value_text`.
- `skin`: `parse(text) -> Result<Theme, SkinError>` (see "Skin").
- `theme`: `Palette` (ten colours: `background`, `panel`, `border`, `text`, `text_dim`, `focus`, `focus_text`, `accent`, `warning`, `error`), `Theme` and `SAFE_AREA`, the safe area of the landscape form (16 px left and right, 12 px top and bottom). The layout reads the safe area from `Metrics`; `SAFE_AREA` stays only for the crawler of the app, and package L5 removes it. `UI_WIDTH` and `UI_HEIGHT` are the landscape size and stay only for the app until package L4. `Theme::new(palette, row_height_px, padding_px)` refuses a row height outside `ROW_HEIGHT_MIN_PX..=ROW_HEIGHT_MAX_PX` (12 to 24) or a padding above `PADDING_MAX_PX` (16) with a `ThemeError`, so a bad skin fails once at load. Inside these limits no layout sum can overflow, and the templates use saturating maths on every count and rectangle from the caller.
- `widgets`: `List`, `SettingsList`, `Dialog` (`DialogTone` sets the colour of its title bar), `Grid`, `Slider`. Each holds only its cursor or focus. `handle(action, ...)` returns an event (`SettingsList` returns ready intents); `draw(frame, theme, data)` adds commands and tree rows. `SettingsList::with_pickers()` makes accept on a choice row give `SettingsEvent::Pick`; accept on a `Page` row gives `SettingsEvent::Open`.
- `registry`: `SceneDef { name, build }`, `scenes()`, `find()`, `render()`.
- `golden`: the golden image format and a bit-exact comparison with a readable difference.
- `screens`: `Screens`, the router that the app calls once per frame; the page ids and the reserved actions; the scenes of the screens.
- `screensaver`: `Idle` and `pose`, pure (`docs/spec/screensaver.md`). While `Snapshot::screensaver` holds a frame count, the router draws the screensaver over every screen (screen id `screensaver`) and ignores every action.

### Data for each view

The UI chooses the view; the app owns the data. A screen asks for the data of its view with an intent, and the app answers in a later snapshot:

- `Intent::LoadGames { source, first, count }` asks for rows of a game list (`GameSource`: `System(id)`, `Favourites`, `Recents` or `Versions(id)`). The app puts a `GameWindow` with its `source`, `first`, `total` and rows in `Snapshot::games`. A list of 10,000 games costs one window of rows.
- `Intent::LoadSettings { page }` asks for the rows of a settings page. The app puts a `SettingsPage` in `Snapshot::settings`.
- Systems, profiles, display, confirm and session are small and always complete.

`GameWindow::is_for`, `covers` and `SettingsPage::is_for` tell old data from current data. A screen asks again in each frame until the snapshot answers, at most one request per frame. The app drops a request that is already in flight. So the UI does no I/O, and the app has no screen logic.

## Screens

`docs/spec/ui-navigation.md` is the spec of the Settings page, the row kinds, the arrow, the breadcrumb, Back and the focus, the footer hints, the generic pages, the confirm question of an action row and the keep prompt. `docs/flows/07-screens.md` describes the screens as built. This section gives the contract between the UI and the app.

### The router and what the app does each frame

`screens::Screens` holds the state of the UI: the stack of open screens with their cursors, the focus of the confirm dialog, the dismissed notices, and the changes made in the quick menu. Everything else comes from the snapshot.

The app makes one `Screens::new(theme)` at launch and, each frame:

1. Fills the snapshot. A notice stays until the player dismisses it or the app removes it.
2. Calls `screens.update(&snapshot, &actions, &mut frame)`. `update` clears the frame, follows the snapshot (a session that starts or ends), applies the actions in order, asks for missing data, and draws.
3. Carries out `frame.intents` in order. It drops a `LoadGames` or `LoadSettings` that is already in flight, and ignores a `Launch` while a session starts or runs. When it puts a new `PendingConfirm` in the snapshot, it stops every key repeat and ignores a key held at that moment until its release (`Navigator::hold_back` in the app).
4. Reads `screens.shows_ui()`. False: a game runs with no menu over it; the app shows the game alone, keeps the core running, and sends the UI only the menu button. True: the core is paused and the app draws `frame.scene` over it.
5. Renders `frame.scene` and gives `frame.tree` to the control API.

The UI has no clock. Key repeat comes from the app (`REPEAT_DELAY_NS` 400 ms, then `REPEAT_INTERVAL_NS` 66 ms in `apps/neptron/src/nav.rs`). `UiAction` has no press or repeat flag, so step 3 is what keeps a held direction from moving the confirm focus to Keep. The countdown of the confirm dialog comes from `PendingConfirm::remaining_ns`. The UI never expires a notice.

### Navigation

- Everything works with the D-pad, accept and back, except two uses of the menu button: it opens the quick menu during a game (the app sends it for the button combination), and on a settings row it removes the value set at the level of the screen, so the row inherits again. A pad with two buttons cannot reset a row to inherited (OP-049).
- Up and down move one row; left and right move one page in a list. Lists do not wrap.
- Menu in a game list adds the focused game to the favourites or removes it.
- `MAX_DEPTH` is 8: home, game list, the versions of a game, game, quick menu, Settings, display panel, option list.
- On top of every screen: the confirm dialog while the snapshot holds a pending confirmation (also over a running game), else the guided mapping of a pad (`docs/spec/input-remapping.md`, also over a running game), else the oldest notice not dismissed. A notice waits while a game runs alone, because the opaque canvas would hide the game.

Screen ids in the tree: `home`, `games`, `game` (nothing drawn), `quick-menu`, `quick-menu/keep`, `quick-menu/video`, `settings`, `display`, `audio`, `input`, `library`, `network`, `system`, `page` (any other page), `options`, `pattern/grid`, `confirm`, `confirm-action`, `confirm-action/working`, `notice`, `pad-mapping`, `pad-mapping/review` and `screensaver`. Under a dialog the tree holds the rows of the dialog only, and the hint band shows its hints: "Accept: revert" and "Right, then Accept: keep" for the confirm dialog, "Accept: OK" for a notice.

### Ids that the UI and the app share

The page ids are in `docs/spec/ui-navigation.md`, "Ids". `RESERVED_ID_MIN` is `0xFFFF_0000`; the app numbers its own pages and actions below it. The app answers `LoadSettings` for `DISPLAY_PAGE`, `QUICK_MENU_PAGE`, `AUDIO_PAGE`, `VIDEO_PAGE`, `LIBRARY_PAGE`, `SYSTEM_PAGE`, `NETWORK_PAGE` and `INPUT_PAGE`, and never loads `GRID_PAGE` or `SETTINGS_PAGE`. The UI carries out `RESUME` (`Intent::Resume`) and `QUIT_GAME` (`Intent::QuitGame`) itself and never sends them as `Intent::Action`. The app reads `NETWORK_SCREEN` and `INPUT_SCREEN` to know whether that panel is on screen.

`SETTINGS_PANELS` is the static table of the Settings page. Any page other than the display panel shows in the generic page screen, so a new panel is one table entry and a page that the app answers for.

### Home and the game list

Home lists the systems with their game counts, then Favourites, Recent games and Settings (OP-049). With no systems it says where the games go (the `roms` folder of the share partition, with `roms/snes` and `roms/arcade` as examples) and keeps one Settings row.

The game list needs the visible rows and `PREFETCH_ROWS` (32) rows on each side. When the snapshot lacks that range, it asks for `WINDOW_ROWS` (128) rows centred on the visible rows. A jump past the window shows `...` rows until the data comes. While the snapshot holds a window of a different list, the list draws no rows. An empty list says why it is empty. The focused title shows in full on two lines under the list.

The detail column shows the marks of a game: `*` favourite, `V` vertical screen, `!` not working, `?` not a set of the arcade index. The last line under the list gives them in words, with `Other versions (N)` for a game with folded clones (`GameEntry::versions`). Accept on such a game opens `GameSource::Versions(id)`: the game first, then its other versions.

### The display panel and the confirm dialog

The display panel shows the output, the mode and the monitor on screen now, then the rows of `DISPLAY_PAGE`. While a game runs with a video status, it shows the video block under the info box, with the mode as the kernel reports it, when at least 2 rows of the page fit under the block; the rows scroll. Else the panel has no block and the mode line stays in the info box. The panel always writes the global level (OP-049) and never changes the display itself.

A choice row opens the list of its options on accept; left and right do not change it. Each option shows its detail, or its help line under the list. An option the display cannot give is grey, with its reason in the row and in full under the list, and accept on it does nothing. A reason wider than half the row shows as "Not available" in the row. Accept on a different option sends one `SetValue`. The app validates the change, switches, and puts a `PendingConfirm` in the snapshot.

The confirm dialog has the buttons Revert and Keep, in that order. The focus starts on Revert for each new confirmation: a `PendingConfirm::id` that differs from the last frame, or that the last frame did not have. The app gives each signal change a new `ConfirmId`, also a change that replaces another at once. Accept on Revert and back send `RevertMode`. Only right, then accept, sends `KeepMode`. So a player who cannot see the picture and presses accept gets the old picture back. After one answer the dialog ignores the other actions of that frame. When the next snapshot still holds the same confirmation, the app refused the keep, and the dialog takes input again. The dialog shows the time left, rounded up to whole seconds. At 0 s it shows Revert alone, because the app refuses a keep at the deadline. When the time ends the app reverts; the UI never does.

### The quick menu

`docs/spec/ui-navigation.md`, "The quick menu", gives the rows and the keep prompt. For System or All games the prompt sends `SetValue` at that level and `ResetValue` at the game level, for each noted change whose row scopes allow the level. "All games" writes the global level only, so a value set for the system still wins (OP-049). The app checks the scopes of every `SetValue` again. The UI notes at most `MAX_CHANGES` (16) settings per visit; a further one stays at the game level (OP-049). After Quit game the quick menu waits for the session to end and takes only back, which gives the quick menu back.

### The video block

`Snapshot::video` holds the video status of the running game as text. A formatter in `neptron-video` makes the text, and the app copies it into the snapshot. The UI draws the text as it gets it: it does not parse, compare or round a value, and it does not choose which result is an alert (OP-058).

| Field | Meaning | Example |
| --- | --- | --- |
| `game` | What the core asks for: the source size, then the field rate | `256x224             60.099 Hz` |
| `mode` | The mode on screen as the kernel reports it: the size with its scan suffix, the line rate, the field rate, two spaces, the rung of the picture | `768x448p  30.05 kHz 60.099 Hz  2x` |
| `result` | The result in words, joined with `, `. An alert result starts with the mark `[!] ` | `exact`, `[!] cropped 2 lines, speed (60.099 to 59.940 Hz)` |
| `alert` | True for `filtered` and for `speed` | |

The formatter writes fixed columns, so the field rates of the two lines are one under the other:

- Columns 1 to 10: the size, padded with spaces. The mode size ends with `p` for progressive or `i` for interlaced.
- Columns 11 to 20: the line rate, padded. kHz with two decimals (`30.05 kHz`), with one decimal from 100 kHz up (`133.3 kHz`). Spaces on the game line.
- Then the field rate in Hz with three decimals. The mode line uses the rate read back from the kernel, never the requested one.
- On the mode line only: two spaces, then the rung (`2x`, `native`, `fit`).
- When `alert` is true: the mark `[!] ` first. It is not a result word, and no `, ` follows it.
- The results in this order: `exact`, `double refresh`, `cropped N lines`, `small picture N%`, `filtered`, `speed (X to Y Hz)`. X is the rate of the game and Y the rate on screen, with three decimals.

The app sends `None` before the video engine reads the mode back from the kernel, and when no game runs, so the block never shows a requested mode as if it were on screen. It keeps one `VideoStatus` and writes new text into its strings, so the frame allocates nothing. The screens show the block only while `Snapshot::session` holds a game as well.

The block is a framed box with the heading "Video" and four lines of 12 px:

```
Video
Game     256x224             60.099 Hz
Mode     768x448p  30.05 kHz 60.099 Hz  2x
         exact
         (second line of the result, empty here)
```

The result always starts on its own line and has two lines, so a changed digit or result never moves a line or the menu rows. An alert result is drawn in the `warning` colour, which differs from the amber focus colour. The block takes no focus and no input. A screen shows it only when at least 2 rows of its page fit under it.

With the default skin the value column holds 37 characters. A longer `game` or `mode` is cut with `...`, never wrapped. The worst mode line, `1920x238p 15.73 kHz 59.940 Hz  native`, is 37 characters; a field rate of 100 Hz or more makes it 38, and it is cut. `result` wraps at spaces onto two lines; text after the second line is cut with `...`. A test in `neptron-video` checks the longest real results. A skin with a larger padding gives fewer characters.

The tree gives each line a row with a stable id as the label and the whole text as the value, also when the drawing is cut: `video/game` (`screens::VIDEO_GAME_ROW`), `video/mode` (`VIDEO_MODE_ROW`) and `video/result` (`VIDEO_RESULT_ROW`).

### Notices

A notice shows as a dialog with one button, oldest first. Accept or back sends `DismissNotice(id)`, and the UI hides that notice until the app removes it. Two notices in one snapshot have different `NoticeId`s, and the app adds new notices at the end. The UI shows the notice after the newest dismissed one, and remembers 8 dismissed ids. The title bar has the colour of the kind: `accent` for `Info`, `warning` for `Warning`, `error` for `Error`.

`Notice::with_choice(label)` shows `Cancel` and the label, with the focus on `Cancel`. Accept on the label sends `AcceptNotice(id)`; back, or accept on `Cancel`, sends `DismissNotice(id)`.

### Opening a panel from the app

`Screens::open_panel(page, focus)` opens a panel of the Settings page in the next `update`, with the focus on the row `focus` once the rows are in the snapshot. When a Settings page is open, the screens above it close and the panel opens from it, so there is never a second Settings page. Else, when the stack has no room, the screens above the quick menu or home close first. When the panel cannot open, `Screens::take_open_failure()` gives the page once, and the app shows a notice. Nothing opens over a game with no menu.

### The grid pattern

The grid pattern draws fixed pixels that do not follow the skin; `src/screens/pattern.rs` lists them. Accept or back closes it.

### Look

Frames that the screens draw are two pixels high, so an interlaced mode shows them in both fields. No text is pure red or pure blue. All content is inside `SAFE_AREA`.

## Skin

`data/skins/default/skin.toml` holds the colours and sizes of the built-in skin, in a small subset of TOML: a top-level `version`, a `[colors]` table with `"#RRGGBB"` values, and a `[sizes]` table with `row_height_px` and `padding_px`. The app passes the text to `skin::parse`, which makes the theme through `Theme::new`. A missing or unknown key (OP-049), a bad colour or number, a size outside the theme limits, or a version outside `SKIN_VERSION_OLDEST..=SKIN_VERSION` gives a `SkinError` that names the line or the key. The parser checks the version first, so a newer file gets the version error.

The format is append-only: a new version adds keys and never renames or removes one. Every key of the version of the file is required.

| Version | Colours | Sizes |
| --- | --- | --- |
| 1 | `background`, `panel`, `border`, `text`, `text_dim`, `focus`, `focus_text`, `accent` | `row_height_px`, `padding_px` |
| 2 | Version 1, plus `warning` and `error` | As version 1 |

A version 1 file gets `warning` and `error` from `Palette::default()`, and a `warning` or `error` key in it is an unknown key. The committed file is version 2, and a test checks that it gives `Theme::default()`.

Default colours of version 2: `warning` is orange `#FF8020`, 19 degrees of hue from the amber focus bar (`#F0C030`) with two thirds of its luminance, so warning text does not read as a focused row. `error` is light red `#FF6060`: pure red text smears on a CRT and has low luminance. Both read on the panel at 5.7:1 and 4.9:1, and dark text on them at 7.3:1 and 6.2:1.

## Font

Spleen by Frederic Cambus, release 2.2.0, size 6x12, licence BSD-2-Clause. `assets/fonts/spleen/` holds the unchanged BDF file, the upstream `LICENSE`, and a README with the source, the commit and the SHA-256 of each file.

`cargo xtask font` generates `src/font/spleen_6x12.rs` from the BDF file. Do not edit the generated file: a test in `xtask` fails when it differs from the generator output.

At 320x240, 6x12 gives 48 characters on a line of the safe area and 18 rows of 12 px. Capitals are 8 px tall, the minimum for text on the canvas. Another Spleen size needs a change of the generator paths, of the module name in `src/font.rs`, and new goldens.

BSD-2-Clause asks that a binary distribution reproduces the notice. So the crate declares `license = "(MIT OR Apache-2.0) AND BSD-2-Clause"`, and the licence manifest of the image lists `crates/neptron-ui/assets/fonts/spleen/LICENSE` (`docs/adr/0004-font-licence.md`).

## Why no `embedded-graphics`

Compile time has priority. `embedded-graphics` 0.8 brings `embedded-graphics-core`, `az`, `byteorder`, `micromath` and `float-cmp`; this crate needs none and checks in under a second. The UI uses only the four primitives of the Definition, and every pixel rule is integer maths here that the GLES renderer can reproduce. `embedded-graphics` has its own rules for strokes, text baselines and alignment. This comparison comes from its documented design and dependency tree, not from a build.

## How to test

`cargo xtask test -p neptron-ui`.

### Golden images

Every screen state that matters is a scene with a golden image (`src/screens/scenes.rs`, `src/registry/demos.rs`). Each screen scene drives a `Screens` with fixture data and actions, as the app does. The goldens are in `crates/neptron-ui/tests/golden/` (`golden::GOLDEN_DIR`); the root `tests/golden/` is for application-level screenshots. A new scene needs a golden: look at it with `cargo xtask shot <name>` first, then write it with `cargo xtask golden --bless <name>`.

The format is binary PPM: the header `P6\n320 240\n255\n`, then RGB rows, without the unused byte of XRGB. `tests/golden.rs` renders every scene and compares the bytes. Every value in the pixel path is an integer, so macOS and Linux give the same bytes.

A test with a fixed seed draws all five templates with random themes, rectangles, counts and values.

### No allocation per frame

`UiScene`, `UiTree` and the intent list keep their memory on `clear()`. Text goes into the store of the scene; numbers are formatted with `StackText`. `CpuCanvas` makes its buffer once in `new()`. `SceneCanvas` copies the scene with `UiScene::copy_from`, which keeps the memory.

A unit test draws every registered scene twice and checks that no buffer grew or moved. It is a proxy: the crate forbids `unsafe`, so it cannot install a counting allocator. Router tests prove that the scene, the tree, the intents and the screen stack keep their capacity: for a static screen after its first frame; for a round trip over a 10,000-game list after one warm-up round trip; for a confirmation, a notice and the quick menu over a game, with and without the video block, after one warm-up round. What can still grow: a frame with more or longer text than any frame before grows the scene and the tree once; opening the quick menu allocates one box for its state, freed when it closes. The snapshot belongs to the app and is not counted.
