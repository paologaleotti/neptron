# neptron-ui

The sealed UI: snapshot in, scene and intents out. A 320x240 XRGB8888 canvas, a bitmap font, five widget templates, the scene registry and the screens.

## Public API sketch

- `Snapshot` (module `snapshot`): what the UI shows. All systems; a `GameWindow` of one game list; a `SettingsPage` of one page (`MenuItem` with a typed `SettingValue` and its source `Level`); monitor profiles, the display, a pending confirm-or-revert, the session, notices. The app fills it. The snapshot and its entry structs are `#[non_exhaustive]`: start from `Snapshot::default()` and the `new` constructors.
- `ChoiceOption`: one option of a `SettingValue::Choice`. A label, a detail text (for a picture style: the line rate and the field rate it gives), and `unavailable: Option<String>`, the reason why the player cannot choose it now. `ChoiceOption::new(label).with_detail(..).unavailable(..)`; `From<&str>` makes a plain option.
- `Notice` carries a `NoticeId`. Two notices in one snapshot have different ids.
- `UiAction`: up, down, left, right, accept, back, menu.
- `Intent` and `Value`: what the app must do (load a game window or a settings page, launch, set or reset a value at a level, run an action, select a profile, keep or revert a mode, resume, quit, dismiss a notice). `Intent` is exhaustive on purpose: a new intent breaks the build of the app until the app handles it.
- `UiFrame`: the output of one frame. `scene: UiScene` (draw commands), `tree: UiTree` (rows for `ui tree` in the control API), `intents: Vec<Intent>`. Make one at launch and reuse it; `clear()` keeps the memory.
- `UiScene`: the four primitives. `rect`, `sprite`, `text_run` (or `text_fmt`), `clip`. `render(&mut impl Canvas)` replays them.
- `trait Canvas` and `CpuCanvas`: `CpuCanvas::as_bytes()` is XRGB8888, little-endian, pitch 1280, ready for the `ui` frame of a `Scene` in `neptron-types`.
- `font`: Spleen 6x12, printable ASCII and Latin-1, 6x12 glyph box, 6 px advance, capitals 8 px tall. Other characters draw `?`.
- `text`: `fit`, `draw_in` (cut with `...`, aligned, reports clipping), `wrap`, `StackText` (formatting with no allocation), `value_text`.
- `skin`: `parse(text) -> Result<Theme, SkinError>` reads a skin file (see "Skin" below).
- `theme`: `Theme` and `SAFE_AREA` (5% overscan margin: 16 px left and right, 12 px top and bottom). `Theme::new(palette, row_height_px, padding_px)` refuses sizes outside `ROW_HEIGHT_MIN_PX..=ROW_HEIGHT_MAX_PX` (12 to 24) and above `PADDING_MAX_PX` (16) with a `ThemeError`, so a bad skin fails once at load. Inside these limits no layout sum can overflow, and the templates use saturating maths on every count and rectangle that comes from the caller. A test with a fixed seed draws all five templates with random themes, rectangles, counts and values.
- `widgets`: `List`, `SettingsList`, `Dialog`, `Grid`, `Slider`. Each holds only its cursor or focus. `handle(action, ...)` returns an event (`SettingsList` returns ready intents); `draw(frame, theme, data)` adds commands and tree rows. A `List` row that is not available is dim, and grey instead of the focus colour when focused. `SettingsList::with_pickers()` makes accept on a choice row give `SettingsEvent::Pick` instead of a change.
- `registry`: `SceneDef { name, build }`, `scenes()`, `find()`, `render()`. The template scenes are `widgets/*` and `font/ascii`.
- `golden`: the golden image format and a bit-exact comparison with a readable difference.
- `screens`: `Screens`, the router that the app calls once per frame; the page ids and the reserved menu actions; `screens::SCENES` adds the scenes of the screens to the registry. See "Screens" below.

### Data for each view

The UI chooses the view; the app owns the data. A screen asks for the data of its view with an intent, and the app answers in a later snapshot:

- `Intent::LoadGames { source, first, count }` asks for rows of a game list. `GameSource` is `System(id)`, `Favourites` or `Recents`. The app puts a window of rows around them in `Snapshot::games`: a `GameWindow` with its `source`, the index `first` of its first row, the `total` of the list, and the rows. A list of 10,000 games costs a window of rows, not 10,000 copies, and the snapshot is passed by reference, never copied per frame.
- `Intent::LoadSettings { page }` asks for the rows of a settings page. The UI names two pages, `screens::DISPLAY_PAGE` and `screens::QUICK_MENU_PAGE`; the app numbers other pages below `screens::RESERVED_ID_MIN`. The app puts the rows in `Snapshot::settings`, a `SettingsPage` with its `page`.
- Systems, profiles, display, confirm and session are small and always complete.

`GameWindow::is_for`, `covers` and `SettingsPage::is_for` let a screen tell old data from current data. A screen asks again in each frame until the snapshot answers, at most one request per frame, and never while the snapshot holds what it needs. The app must drop a request that is already in flight, so the repeats cost nothing. This keeps the UI free of I/O and the app free of screen logic. The other option, a snapshot with every list complete, copies or holds the whole library for each frame, so it was not taken.

## Screens

### The router and what the app does each frame

`screens::Screens` holds the state of the UI: the stack of open screens with their cursors, the focus of the confirm dialog, the notices the player dismissed, and the changes made in the quick menu. Everything else comes from the snapshot. A screen is therefore a function of that state, the snapshot and the actions of the frame.

The app makes one `Screens::new(theme)` at launch, keeps it, and each frame:

1. Fills the snapshot from the library, the settings, the session and the video engine. A notice stays until the player dismisses it or the app removes it.
2. Calls `screens.update(&snapshot, &actions, &mut frame)`. `update` clears the frame, follows the snapshot (a session that starts or ends), applies the actions in order, asks for missing data, and draws.
3. Carries out `frame.intents` in order. It drops a `LoadGames` or `LoadSettings` that is already in flight. It ignores a `Launch` while a session starts or runs. When it puts a new `PendingConfirm` in the snapshot, it stops every key repeat, and it sends no action for a key that is held at that moment until the key is released.
4. Reads `screens.shows_ui()`. False: a game runs with no menu over it, so the app shows the game alone, keeps the core running, and sends the UI only the menu button. True: the core is paused and the app draws `frame.scene` over it.
5. Renders `frame.scene` on the canvas and gives `frame.tree` to the control API (`ui tree`).

The UI has no clock and takes no time step. Key repeat comes from the app: while a direction is held, the app sends the action again (for example after 400 ms, then every 66 ms). A held key must not reach the confirm dialog: a right held on a number row sends a change, the confirmation appears, and the repeat would move the focus to Keep, so a later blind accept would keep a picture the player cannot see. Two guards exist, because each one alone has a gap. The app knows when a key goes down and up, so it stops the repeat and ignores keys held when the confirmation appears; the UI cannot see that, because `UiAction` has no press or repeat flag. The UI guards itself all the same, so an app with a fault cannot keep a mode by accident: the dialog ignores left and right until it has seen `QUIET_FRAMES_BEFORE_MOVE` (60) frames in a row with no direction action. One quiet frame is not enough, because key repeat leaves gaps; 60 frames is longer than a first repeat delay of 400 ms at every refresh up to 120 Hz. The countdown of the confirm dialog comes from `PendingConfirm::remaining_ns`. No screen needs a timer: a long title is cut with `...` and shown in full under the list, and the UI never expires a notice.

### Navigation

- Everything works with the D-pad, accept and back, with two exceptions that need the menu button: it opens the quick menu during a game (the app sends it for the button combination), and it resets a settings row to the inherited value. A pad with two buttons can choose any value, but cannot reset a row to inherited yet.
- Accept opens, plays or chooses. Back closes the screen on top; on home it does nothing.
- Up and down move one row; left and right move one page in a list. Lists stop at their ends and do not wrap, so a held direction stops at the last row.
- Menu in a game list adds the focused game to the favourites or removes it. Menu on a settings row removes a value set at the level of the screen, so the row inherits again.
- The stack: home, then a game list, then the game. Over the game: the quick menu, from it the display panel or the grid pattern, from a choice row the list of its options. From home: the display panel, then options or the grid pattern. `MAX_DEPTH` is 8; the deepest path is 6.
- On top of every screen: the confirm dialog while the snapshot holds a pending confirmation (also over a running game), else the oldest notice that the player has not dismissed (not while a game runs alone: the canvas is opaque and would hide the game).

The screen ids in the tree are `home`, `games`, `game` (nothing drawn), `quick-menu`, `quick-menu/keep`, `display`, `options`, `pattern/grid`, `confirm` and `notice`. Under a dialog the tree holds the rows of the dialog only, and the hint band shows the hints of the dialog: "Accept: revert" and "Right, then Accept: keep" for the confirm dialog, "Accept: OK" for a notice, "Accept: choose" and "Back: cancel" for the prompt of the quick menu.

### Ids that the UI and the app share

| Id | Value | Use |
| --- | --- | --- |
| `DISPLAY_PAGE` | `PageId(RESERVED_ID_MIN + 1)` | The rows of the display panel: output, monitor, picture style, refresh policy, and a `GRID_PATTERN` row. Written at the global level |
| `QUICK_MENU_PAGE` | `PageId(RESERVED_ID_MIN + 2)` | The rows of the quick menu: the settings of the game and the rows below. Written at the game level |
| `RESUME` | `ActionId(RESERVED_ID_MIN + 1)` | Close the quick menu: `Intent::Resume` |
| `QUIT_GAME` | `ActionId(RESERVED_ID_MIN + 2)` | Stop the game: `Intent::QuitGame` |
| `OPEN_DISPLAY` | `ActionId(RESERVED_ID_MIN + 3)` | Open the display panel |
| `GRID_PATTERN` | `ActionId(RESERVED_ID_MIN + 4)` | Show the grid test pattern |

`RESERVED_ID_MIN` is `0xFFFF_0000`. The UI carries out the reserved actions itself and never sends them as `Intent::Action`. The app numbers its own pages and actions below it.

### Home and the game list

Home lists the systems with their game counts, then Favourites, Recent games and Display. With no systems it says where the games go (`roms/nes`, `roms/snes`, `roms/megadrive` on the share partition) and keeps the Display row, so a new user can still fix the picture.

The game list reads a window of rows. It needs the visible rows and `PREFETCH_ROWS` (32) rows on each side of them. While the snapshot holds that range it asks for nothing. When it does not, it asks for `WINDOW_ROWS` (128) rows centred on the visible rows, once per frame, until the snapshot holds them. The margin is larger than a page, so a step or a page jump shows rows of the old window while the new one loads. A jump past the window shows `...` rows until the data comes. While the snapshot holds a window of a different list, the cursor stays where it is and the list draws no rows. An empty list says why it is empty. The focused title shows in full on two lines under the list.

### The display panel and the confirm dialog

The display panel shows the output, the mode and the monitor on screen now, then the rows of `DISPLAY_PAGE`. It never changes the display itself. A choice row does not change with left and right: accept opens the list of its options, each with its detail, for example "31.5 kHz  59.94 Hz". An option the monitor cannot show is grey, with its reason in the row and in full under the list ("Not available. Needs a 15 kHz monitor"), and accept on it does nothing. Accept on a different option sends one `SetValue`. The app validates the change, switches, and puts a `PendingConfirm` in the snapshot.

The confirm dialog has the buttons Revert and Keep, in that order. The focus starts on Revert for each new confirmation: one whose `PendingConfirm::id` differs from the last frame, or that the last frame did not have. The app gives each signal change a new `ConfirmId`, also a change that replaces another at once; more time left under the same id is the same confirmation. Accept on Revert and back send `RevertMode`. Only right, then accept, sends `KeepMode`, and right moves the focus only after 60 frames in a row with no direction action (see key repeat above). Accept and back work at once. So a player who cannot see the picture and presses accept gets the old picture back. After one answer the dialog ignores input until the snapshot drops the confirmation or holds one with a new id. The dialog shows the time left, rounded up to whole seconds. When it reaches zero the app reverts; the UI never does.

### The quick menu

Menu during a game opens the quick menu over the paused game. Its rows write at the game level and apply at once. When the player leaves after a change (back, Resume or Quit game), one prompt asks where to keep the changes: This game (the focus), System or All games. For System or All games the UI sends `SetValue` at that level and `ResetValue` at the game level, for each changed row that allows the level. Back in the prompt returns to the quick menu. The UI notes at most `MAX_CHANGES` (16) settings per visit; a further one stays at the game level. After Quit game the quick menu ignores input until the session ends.

### Notices

A notice shows as a dialog with one button, oldest first. Accept or back sends `DismissNotice(id)`, and the UI hides that notice until the app removes it from the snapshot. A failed launch is a notice of kind `Error`.

### The grid pattern

The grid pattern draws fixed pixels that do not follow the skin. `src/screens/pattern.rs` lists every pixel: a white border on the exact edge of the canvas, a grey grid of 16-pixel squares with lines two pixels wide and centred on the canvas, a white centre cross, amber corner marks at the safe area, and "Back: return" at the bottom of the safe area. Accept or back closes it.

### Look

The screens use the templates on a dark background: a title band with a two-pixel accent line under it, framed boxes, a hint band at the bottom, and an amber bar on the focused row. The frames that the screens draw are two pixels high, so an interlaced mode shows them in both fields. The dialog template keeps its one-pixel frame in the border colour, which has medium contrast. No text is pure red or pure blue. All content is inside `SAFE_AREA`.

### Scenes

Every screen state that matters is a scene with a golden image: `screens/home`, `screens/home-empty`, `screens/games-top`, `screens/games-middle` (a title too long for its row), `screens/games-end`, `screens/games-one`, `screens/games-empty`, `screens/display`, `screens/display-style` (a picture style the monitor cannot show), `screens/confirm-10s`, `screens/confirm-3s`, `screens/notice` (a game that did not start), `screens/quick-menu`, `screens/quick-menu-keep` and `pattern/grid`. Each scene drives a `Screens` with fixture data and actions, as the app does. A new scene needs a golden image in `crates/neptron-ui/tests/golden/<name>.ppm`: look at it with `cargo xtask shot <name>` first, then write it with `cargo xtask golden --bless <name>`.

## Skin

`data/skins/default/skin.toml` holds the colours and sizes of the built-in skin. It is a small subset of TOML: a top-level `version`, a `[colors]` table with the eight palette colours as `"#RRGGBB"`, and a `[sizes]` table with `row_height_px` and `padding_px`. The app reads the file and passes the text to `skin::parse`, which makes the theme through `Theme::new`. A missing or unknown key, a bad colour or number, a size outside the theme limits, or a version other than `SKIN_VERSION` is refused with a `SkinError` that names the line or the key. A later format adds keys under a new version. A test checks that the committed file gives `Theme::default()`. Images, fonts and sounds of a skin come after the MVP.

## Golden images

The goldens of the scenes live in this crate, in `crates/neptron-ui/tests/golden/` (`golden::GOLDEN_DIR` is relative to the crate). The root `tests/golden/` is for application-level screenshots, such as the end-to-end runs of the app.

The format is binary PPM (`P6`, maxval 255): the header `P6\n320 240\n255\n`, then RGB rows. The unused byte of XRGB is not stored. The test `tests/golden.rs` renders every scene and compares the bytes. Every value in the pixel path is an integer (positions, sizes, slider fill, scroll bar), so macOS and Linux give the same bytes.

## No allocation per frame

`UiScene`, `UiTree` and the intent list keep their memory on `clear()`. Text goes into the store of the scene; numbers are formatted with `StackText` on the stack. `CpuCanvas` makes its buffer once in `new()`; drawing writes into it. A unit test draws every registered scene twice and checks that no buffer grew or moved. That test is a proxy: the crate forbids `unsafe`, so it cannot install a counting allocator. Fixture data of the scenes (the `Vec<MenuItem>` of a scene) is made when the scene is built and is not part of the frame.

## Why no `embedded-graphics`

The brief asks to try `embedded-graphics` first. The comparison:

| | `embedded-graphics` 0.8 | Own code |
| --- | --- | --- |
| Compile time | The crate and its dependencies (`embedded-graphics-core`, `az`, `byteorder`, `micromath`, `float-cmp`), none in the warm cache of the workspace | No new crate. The whole crate checks in under a second |
| What it gives at 320x240 | Many primitives (lines, circles, triangles, styled shapes), mono fonts, a `DrawTarget` trait | Only the four primitives the Definition allows: rectangle, sprite, text run, clip |
| Pixel control | Its own rules for strokes, text baselines and alignment; the GLES renderer must copy them | Every pixel rule is in about 300 lines here, all integer maths, easy to match in GLES |
| Font | Mono fonts from the X11 `misc-fixed` set | Spleen 6x12 from BDF, through a small generator |

The decision is the own implementation. Compile time has priority, the UI uses only rectangles, sprites and text, and the GLES renderer must reproduce the same pixels from a small and plain rule set. The comparison was made from the documented design and dependency tree of `embedded-graphics` 0.8. It was not built here, because the crate is not in the local cache and the task adds no download.

## Font

The font is Spleen by Frederic Cambus, release 2.2.0, size 6x12, licence BSD-2-Clause. `assets/fonts/spleen/` holds the unchanged BDF file, the upstream `LICENSE`, and a README with the source, the commit, the SHA-256 of each file and how to refresh them.

`cargo xtask font` generates `src/font/spleen_6x12.rs` from the BDF file: printable ASCII and the Latin-1 Supplement, so accented game names show correctly. Do not edit the generated file. A test in `xtask` fails when the committed table differs from the generator output. Other characters draw `?`.

At 320x240, 6x12 gives 48 characters on a line of the safe area and 18 rows of 12 pixels. Capitals are 8 pixels tall, the minimum for text on the canvas. The layout reads the size from the `font` constants, so a different Spleen size needs a change of the generator paths and of the module name in `src/font.rs`, and new goldens.

The BSD-2-Clause licence asks that a binary distribution reproduces the notice. The crate therefore declares `license = "(MIT OR Apache-2.0) AND BSD-2-Clause"`, and the licence manifest of the image must list `crates/neptron-ui/assets/fonts/spleen/LICENSE`. `docs/adr/0004-font-licence.md` records the decision.

## Must never depend on

- Any other NEPTRON crate
- Any hardware crate

Only the app and `xtask` (for `shot`, `gallery` and `golden`) depend on this crate.

## Rules

- `unsafe` is forbidden: the crate root carries `#![forbid(unsafe_code)]`.
- No I/O, no clock, no threads. The golden module works on bytes; the caller reads and writes files.
- No features.
- Dependency: `thiserror` only. The skin file is read by a small parser in `skin.rs`, not by the `toml` crate.
