# neptron-ui

The sealed UI: snapshot in, scene and intents out. A 320x240 XRGB8888 canvas, a bitmap font, five widget templates, the scene registry and the screens.

## Public API sketch

- `Snapshot` (module `snapshot`): what the UI shows. All systems; a `GameWindow` of one game list; a `SettingsPage` of one page (`MenuItem` with a typed `SettingValue` and its source `Level`); monitor profiles, the display, a pending confirm-or-revert, the session, notices, and the video status of the running game. The app fills it. The snapshot and its entry structs are `#[non_exhaustive]`: start from `Snapshot::default()` and the `new` constructors.
- `ChoiceOption`: one option of a `SettingValue::Choice`. A label, a detail text (for a picture style: the line rate and the field rate it gives), `unavailable: Option<String>`, the reason why the player cannot choose it now, and `short: Option<String>`, a short form of the label for the value column of a settings row (`HDMI/DP 1` for `HDMI/DP 1 (HDA Intel PCH)`). The option list always shows the label. `ChoiceOption::new(label).with_detail(..).with_short(..).unavailable(..)`; `From<&str>` makes a plain option.
- `SettingValue::Page(PageId)`: a row that opens a page. A reserved page id opens a screen of the UI: `SETTINGS_PAGE` the Settings page, `GRID_PAGE` the grid pattern.
- `MenuItem::unavailable(reason)`: a row whose setting has no effect now, for example the picture style on a flat panel. The row shows the reason in place of its value, source and arrows, in the dim colour, and takes no change, no reset and no picker. With the focus it has a one-pixel outline in the focus colour and dim text, not the focus fill, and the footer shows no accept hint. The UI tree gives the reason as the row's value and the kind `Disabled`.
- `VideoStatus`: the video status of the running game as three texts and a flag: `game`, `mode`, `result` and `alert`. `VideoStatus::new(game, mode, result).with_alert(..)`. See "The video block" below.
- `Notice` carries a `NoticeId`. Two notices in one snapshot have different ids.
- `UiAction`: up, down, left, right, accept, back, menu.
- `Intent` and `Value`: what the app must do (load a game window or a settings page, launch, set or reset a value at a level, run an action, select a profile, keep or revert a mode, resume, quit, dismiss a notice). `Intent` is exhaustive on purpose: a new intent breaks the build of the app until the app handles it.
- `UiFrame`: the output of one frame. `scene: UiScene` (draw commands), `tree: UiTree` (rows for `ui tree` in the control API, each with its `RowKind` when it is a row of a list), `intents: Vec<Intent>`. Make one at launch and reuse it; `clear()` keeps the memory.
- `UiScene`: the four primitives. `rect`, `sprite`, `text_run` (or `text_fmt`), `clip`. `render(&mut impl Canvas)` replays them.
- `trait Canvas` and `CpuCanvas`: `CpuCanvas::as_bytes()` is XRGB8888, little-endian, pitch 1280, ready for the `ui` frame of a `Scene` in `neptron-types`.
- `font`: Spleen 6x12, printable ASCII and Latin-1, 6x12 glyph box, 6 px advance, capitals 8 px tall. Other characters draw `?`.
- `text`: `fit`, `draw_in` (cut with `...`, aligned, reports clipping), `wrap`, `StackText` (formatting with no allocation), `value_text`.
- `skin`: `parse(text) -> Result<Theme, SkinError>` reads a skin file (see "Skin" below).
- `theme`: `Palette` (ten colours: `background`, `panel`, `border`, `text`, `text_dim`, `focus`, `focus_text`, `accent`, `warning` for a result the player must notice, `error` for something that failed), `Theme` and `SAFE_AREA` (5% overscan margin: 16 px left and right, 12 px top and bottom). `Theme::new(palette, row_height_px, padding_px)` refuses sizes outside `ROW_HEIGHT_MIN_PX..=ROW_HEIGHT_MAX_PX` (12 to 24) and above `PADDING_MAX_PX` (16) with a `ThemeError`, so a bad skin fails once at load. Inside these limits no layout sum can overflow, and the templates use saturating maths on every count and rectangle that comes from the caller. A test with a fixed seed draws all five templates with random themes, rectangles, counts and values.
- `widgets`: `List`, `SettingsList`, `Dialog`, `Grid`, `Slider`. `DialogContent::tone` (`DialogTone::Normal`, `Warning` or `Error`) sets the colour of the title bar of a dialog. Each holds only its cursor or focus. `handle(action, ...)` returns an event (`SettingsList` returns ready intents); `draw(frame, theme, data)` adds commands and tree rows. A `List` row that is not available is dim, and grey instead of the focus colour when focused. `SettingsList::with_pickers()` makes accept on a choice row give `SettingsEvent::Pick` instead of a change; accept on a `SettingValue::Page` row gives `SettingsEvent::Open`. See "Rows" below.
- `registry`: `SceneDef { name, build }`, `scenes()`, `find()`, `render()`. The template scenes are `widgets/*` and `font/ascii`.
- `golden`: the golden image format and a bit-exact comparison with a readable difference.
- `screens`: `Screens`, the router that the app calls once per frame; the page ids and the reserved menu actions; `screens::SCENES` adds the scenes of the screens to the registry. See "Screens" below.

### Data for each view

The UI chooses the view; the app owns the data. A screen asks for the data of its view with an intent, and the app answers in a later snapshot:

- `Intent::LoadGames { source, first, count }` asks for rows of a game list. `GameSource` is `System(id)`, `Favourites` or `Recents`. The app puts a window of rows around them in `Snapshot::games`: a `GameWindow` with its `source`, the index `first` of its first row, the `total` of the list, and the rows. A list of 10,000 games costs a window of rows, not 10,000 copies, and the snapshot is passed by reference, never copied per frame.
- `Intent::LoadSettings { page }` asks for the rows of a settings page. The UI names four pages that the app loads, `screens::DISPLAY_PAGE`, `screens::AUDIO_PAGE`, `screens::QUICK_MENU_PAGE` and `screens::VIDEO_PAGE`; the app numbers other pages below `screens::RESERVED_ID_MIN`. The app puts the rows in `Snapshot::settings`, a `SettingsPage` with its `page`.
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
- Accept opens, plays or chooses. Back closes the screen on top and nothing else; on home it does nothing. The screen under it keeps its cursor, so the focus is on the row that opened the closed screen. A page whose rows are not in the snapshot yet takes only back, so an early action cannot move its cursor.
- Up and down move one row; left and right move one page in a list. Lists stop at their ends and do not wrap, so a held direction stops at the last row.
- Menu in a game list adds the focused game to the favourites or removes it. Menu on a settings row removes a value set at the level of the screen, so the row inherits again.
- The stack: home, then a game list, then the game. Over the game: the quick menu, from it the video page or the Settings page, then a panel, then options or the grid pattern. From home: the Settings page, then a panel, then options or the grid pattern. `MAX_DEPTH` is 8; the deepest path is 7 (home, game list, game, quick menu, Settings, display panel, options).
- On top of every screen: the confirm dialog while the snapshot holds a pending confirmation (also over a running game), else the oldest notice that the player has not dismissed (not while a game runs alone: the canvas is opaque and would hide the game).

The screen ids in the tree are `home`, `games`, `game` (nothing drawn), `quick-menu`, `quick-menu/keep`, `quick-menu/video`, `settings`, `display`, `audio`, `page` (any other page), `options`, `pattern/grid`, `confirm` and `notice`. The rows of the video block have the ids `video/game`, `video/mode` and `video/result` as their labels (see "The video block"). Under a dialog the tree holds the rows of the dialog only, and the hint band shows the hints of the dialog: "Accept: revert" and "Right, then Accept: keep" for the confirm dialog, "Accept: OK" for a notice, "Accept: choose" and "Back: cancel" for the prompt of the quick menu.

### Ids that the UI and the app share

| Id | Value | Use |
| --- | --- | --- |
| `DISPLAY_PAGE` | `PageId(RESERVED_ID_MIN + 1)` | The rows of the display panel: output, monitor, picture style, refresh policy, scanlines, the Start hold, and a row that opens `GRID_PAGE`. Written at the global level |
| `QUICK_MENU_PAGE` | `PageId(RESERVED_ID_MIN + 2)` | The first page of the quick menu: Resume, the volume of the game, a row for `VIDEO_PAGE`, a row for `SETTINGS_PAGE`, Quit game. Written at the game level |
| `AUDIO_PAGE` | `PageId(RESERVED_ID_MIN + 3)` | The rows of the audio panel: volume and output device. Written at the global level |
| `GRID_PAGE` | `PageId(RESERVED_ID_MIN + 4)` | The grid test pattern, as the target of a `Page` row. The app never loads it |
| `SETTINGS_PAGE` | `PageId(RESERVED_ID_MIN + 5)` | The Settings page, as the target of a `Page` row. The UI shows it from `SETTINGS_PANELS`; the app never loads it |
| `VIDEO_PAGE` | `PageId(RESERVED_ID_MIN + 6)` | The video page of the quick menu: the video block and the display settings of the game. Written at the game level |
| `RESUME` | `ActionId(RESERVED_ID_MIN + 1)` | Close the quick menu: `Intent::Resume` |
| `QUIT_GAME` | `ActionId(RESERVED_ID_MIN + 2)` | Stop the game: `Intent::QuitGame` |

`RESERVED_ID_MIN` is `0xFFFF_0000`. The UI carries out the reserved actions itself and never sends them as `Intent::Action`. The app numbers its own pages and actions below it.

`SETTINGS_PANELS` is the list of the Settings page: a static table of `Panel { label, page }`, `Display` then `Audio` today. A new panel is one entry and a page that the app answers for. Any page other than the display panel shows in the generic page screen (`PagePanel`), so a new panel needs no screen code.

### Home and the game list

Home lists the systems with their game counts, then Favourites, Recent games and Settings. With no systems it says where the games go (`roms/nes`, `roms/snes`, `roms/megadrive` on the share partition) and keeps one Settings row only, so a new user can still fix the picture and the sound.

### Pages and the audio panel

The Settings page lists the panels of `SETTINGS_PANELS` and writes nothing; its panels write at the global level, also in a game. A page opened from a row writes at the level of the screen that opened it. The generic page screen shows the rows of its page in one framed box, and the video block above them for `VIDEO_PAGE`. The audio panel is such a page: the volume as a number row (left and right step it) and the output device as a choice row, whose options open in a list (`docs/spec/audio-settings.md`).

### Rows

`docs/spec/ui-navigation.md` sets one rule for the rows of every list. A row has a `RowKind`: `Page` (accept opens a page, an option list or the grid), `Value` (it changes in place), `Action` (accept does something) or `Disabled`. A `Page` row shows a right-pointing triangle at its right edge, 4 px wide and 7 px high, drawn with rectangles in `text_dim` (`focus_text` on the focused row). A `>` glyph would read as the end of `< value >`. Every settings row keeps a column of `ARROW_COLUMN_PX` (8) free at its right edge, so the value and source columns line up. The footer hint follows the kind of the focused row (`SettingsList::hint`): `Accept: open`, `Accept: change` for a switch, `Left/Right: change` for a number, `Accept: choose` for an action, none for a disabled row.

### The breadcrumb

The title band of every page below home and of every quick menu page shows the path to it: `Settings > Audio > Audio output`, or the title of the game first in a game (`Astro Racer > Video`). The separator is ` > `. A path too long for the band keeps its last two parts after `... > `; then the second last part is cut at its end with `...`; then only the last part stays, cut the same way (`breadcrumb::fit`). The tree row holds the text as drawn, and the whole path as its value when the rule shortened it. Each screen keeps the label of the row that opened it in a `StackText`, so opening a screen allocates nothing for it.

The game list reads a window of rows. It needs the visible rows and `PREFETCH_ROWS` (32) rows on each side of them. While the snapshot holds that range it asks for nothing. When it does not, it asks for `WINDOW_ROWS` (128) rows centred on the visible rows, once per frame, until the snapshot holds them. The margin is larger than a page, so a step or a page jump shows rows of the old window while the new one loads. A jump past the window shows `...` rows until the data comes. While the snapshot holds a window of a different list, the cursor stays where it is and the list draws no rows. An empty list says why it is empty. The focused title shows in full on two lines under the list.

### The display panel and the confirm dialog

The display panel shows the output, the mode and the monitor on screen now, then the rows of `DISPLAY_PAGE`. While a game runs with a video status, the panel shows the output and the monitor, then the video block of the quick menu (see "The video block"), which shows the mode as the kernel reports it, then the rows. With the default skin all five rows of the display page still fit. A skin with rows too tall for the block (24 px rows leave no row under it) keeps the panel without the block, with the mode line in the info box. It never changes the display itself. A choice row does not change with left and right: accept opens the list of its options, each with its detail, for example "31.5 kHz  59.94 Hz". An option the monitor cannot show is grey, with its reason in the row and in full under the list ("Not available. Needs a 15 kHz monitor"), and accept on it does nothing. Accept on a different option sends one `SetValue`. The app validates the change, switches, and puts a `PendingConfirm` in the snapshot.

The confirm dialog has the buttons Revert and Keep, in that order. The focus starts on Revert for each new confirmation: one whose `PendingConfirm::id` differs from the last frame, or that the last frame did not have. The app gives each signal change a new `ConfirmId`, also a change that replaces another at once; more time left under the same id is the same confirmation. Accept on Revert and back send `RevertMode`. Only right, then accept, sends `KeepMode`, and right moves the focus only after 60 frames in a row with no direction action (see key repeat above). Accept and back work at once. So a player who cannot see the picture and presses accept gets the old picture back. After one answer the dialog ignores the other actions of that frame. The app carries out the answer in the same frame. When the next snapshot still holds the same confirmation, the app refused the keep: the dialog takes input again, with the focus where it was. The dialog shows the time left, rounded up to whole seconds. When it reaches zero the app reverts; the UI never does.

### The quick menu

Menu during a game opens the quick menu over the paused game: Resume, Volume, Video, Settings and Quit game. Its rows and the rows of the video page write at the game level and apply at once. When the player leaves after a change (back, Resume or Quit game), one prompt asks where to keep the changes: This game (the focus), System or All games. For System or All games the UI sends `SetValue` at that level and `ResetValue` at the game level, for each changed row that allows the level; the quick menu reads the scopes of the row when the change is made, from the page of that frame, because the prompt opens on the first page, which does not hold the rows of the video page. A change whose row was not in the snapshot stays at the game level. The app checks the scopes of every `SetValue` again, as for any other change. Back in the prompt returns to the quick menu. When the player leaves a sub page with back, before it asks, and again before it moves a change, the quick menu forgets a noted change whose row in the snapshot no longer shows that value at the game level: the app took it back (the guard reverted a display change that was not kept), so there is nothing to keep. The UI notes at most `MAX_CHANGES` (16) settings per visit; a further one stays at the game level. After Quit game the quick menu waits for the session to end and ignores input, except back: back gives the quick menu back, so a player is never stuck when the app does not stop the game.

### The video block

`Snapshot::video` holds the video status of the running game as text. A formatter in `neptron-video` makes the text from the state of the video engine, and the app copies it into the snapshot each frame. The UI draws the text as it gets it. It decides nothing about the content: it does not parse, compare or round a value, and it does not choose which result is an alert.

| Field | Meaning | Example |
| --- | --- | --- |
| `game` | What the core asks for: the source size, then the field rate | `256x224             60.099 Hz` |
| `mode` | The mode on screen as the kernel reports it: the size with its scan suffix, the line rate, the field rate, then two spaces and the picture style | `768x448p  30.05 kHz 60.099 Hz  2x` |
| `result` | The result in words. The formatter joins two or more results with `, `. An alert result starts with the mark `[!] ` | `exact`, `[!] cropped 2 lines, speed (60.099 to 59.940 Hz)` |
| `alert` | True when the player must notice the result. The formatter sets it for `filtered` and for `speed` | |

The formatter writes fixed columns, so that the field rates of the two lines are one under the other:

- Columns 1 to 10: the size, padded with spaces to 10 characters. The mode size ends with the scan suffix: `p` for progressive, `i` for interlaced (`768x448p`, `640x480i`).
- Columns 11 to 20: the line rate, padded with spaces to 10 characters. It is in kHz with two decimals (`30.05 kHz`), and with one decimal from 100 kHz up (`133.3 kHz`, a 2160p panel), so it stays 9 characters. On the game line these columns are spaces.
- Then the field rate in Hz with three decimals (`60.099 Hz`). The mode line uses the rate read back from the kernel, never the requested one.
- On the mode line only: two spaces, then the picture style (`2x`, `native`).
- When `alert` is true: the mark `[!] ` first. It is not a result word, and no `, ` follows it.
- The results, in this order when there are more than one: `exact`, `double refresh`, `cropped N lines`, `small picture N%`, `filtered`, `speed (X to Y Hz)`. X is the rate of the game and Y the rate on screen, both with three decimals.

The worst case of the mode line is `1920x238p 15.73 kHz 59.940 Hz  native`: 37 characters, which is the width of the value column. A field rate of 100 Hz or more adds one character: with the style `native` the line then has 38 characters and is cut with `...`.

Before the video engine reads the mode back from the kernel, the app sends no status (`None`), and the block does not show. So the block never shows a requested mode as if it were on screen.

What the app supplies each frame: `Some(status)` while a game runs, `None` when no game runs. It keeps one `VideoStatus` and writes new text into its strings (`clear`, then `write!`) when the status changes, so the frame allocates nothing. The screens show the block only while `Snapshot::session` holds a game as well.

The video page of the quick menu shows the block above its rows, and the display panel shows it under its info box while a game runs: a framed box with the heading "Video" and four lines of 12 pixels (one glyph box, for every row height of the skin).

```
Video
Game     256x224             60.099 Hz
Mode     768x448p  30.05 kHz 60.099 Hz  2x
         exact
         (second line of the result, empty here)
```

The layout is fixed: the result always starts on its own line under the mode text, and the block always has two lines for it. A digit or a result that changes never moves a line or the menu rows, which reads better on a 15 kHz CRT than a result that jumps between line 2 and line 3. An alert result carries the mark `[!] ` from the formatter and is drawn in the `warning` colour of the skin (orange in the default skin), which differs from the amber focus colour, so it never reads as a focused row. The block is not focusable and takes no input: the cursor starts on the first menu row, and Up from it stays there. Without a status the block is absent and the video page is the same as without this feature. A screen shows the block only when at least 2 rows of its page fit under it. With the default skin the video page shows all its rows; with 24 px rows it shows 3 of its 4 rows and scrolls.

Lengths at 320x240 with the default skin: the value column starts after a name column of 9 characters and holds 37 characters (222 px). `game` and `mode` fit when they have 37 characters or fewer; a longer one is cut and ends with `...`. `result` wraps at spaces onto two lines of 37 characters; text left over after the second line is cut with `...`. The longest realistic texts fit: `1920x238p 15.73 kHz 59.940 Hz  native` (37), and results with the mark such as `[!] double refresh, small picture 100%, speed (57.500 to 50.082 Hz)` (67, 33 and 33 characters on two lines). A test in `neptron-video` checks the longest real results. A skin with a larger padding gives fewer characters. The UI never wraps `game` or `mode`, and never draws outside the block.

The tree gives each line one label row with a stable id as the label and the whole text as the value, also when the drawing is cut: `video/game` (`screens::VIDEO_GAME_ROW`), `video/mode` (`VIDEO_MODE_ROW`) and `video/result` (`VIDEO_RESULT_ROW`). The heading is a label row "Video".

### Notices

A notice shows as a dialog with one button, oldest first. Accept or back sends `DismissNotice(id)`, and the UI hides that notice until the app removes it from the snapshot. The UI shows the notice after the newest one the player dismissed: the notices are oldest first, so the ones before it were dismissed too. It remembers 8 dismissed ids; when all 8 are still in the snapshot, it forgets the oldest one, which stays hidden behind a newer dismissed one. The app adds new notices at the end. A failed launch is a notice of kind `Error`. The title bar of a notice has the colour of its kind: `accent` for `Info` ("Note"), `warning` for `Warning` and `error` for `Error`, so the player sees how serious it is before reading it.

### The grid pattern

The grid pattern draws fixed pixels that do not follow the skin. `src/screens/pattern.rs` lists every pixel: a white border on the exact edge of the canvas, a grey grid of 16-pixel squares with lines two pixels wide and centred on the canvas, a white centre cross, amber corner marks at the safe area, and "Back: return" at the bottom of the safe area. Accept or back closes it.

### Look

The screens use the templates on a dark background: a title band with the breadcrumb and a two-pixel accent line under it, framed boxes, a hint band at the bottom, an amber bar on the focused row, and an arrow at the right edge of a row that opens a page. The frames that the screens draw are two pixels high, so an interlaced mode shows them in both fields. The dialog template keeps its one-pixel frame in the border colour, which has medium contrast. No text is pure red or pure blue. All content is inside `SAFE_AREA`.

### Scenes

Every screen state that matters is a scene with a golden image: `screens/home`, `screens/home-empty`, `screens/settings` (the Settings page), `screens/breadcrumb-long` (a game title cut in the breadcrumb), `screens/games-top`, `screens/games-middle` (a title too long for its row), `screens/games-end`, `screens/games-one`, `screens/games-empty`, `screens/display`, `screens/display-style` (a picture style the monitor cannot show), `screens/confirm-10s`, `screens/confirm-3s`, `screens/notice` (a game that did not start, an error), `screens/notice-warning` (a warning), `screens/quick-menu` (the first page), `screens/quick-menu-panel` (the video page on a flat panel), `screens/quick-menu-keep`, `screens/quick-menu-video` (the video page with the video block, exact), `screens/quick-menu-video-filtered` (an alert, in the warning colour), `screens/quick-menu-video-long` (a full mode line and a result on two lines), `screens/display-video` (the display panel during a game, with the video block of a 31 kHz mode), `pattern/grid`, `screens/audio` (the audio panel, with the short value of the output), `screens/audio-output` (the options of the output device, a three-part breadcrumb) and `screens/quick-menu-volume` (the volume row of a game, set for its system). Each scene drives a `Screens` with fixture data and actions, as the app does. A new scene needs a golden image in `crates/neptron-ui/tests/golden/<name>.ppm`: look at it with `cargo xtask shot <name>` first, then write it with `cargo xtask golden --bless <name>`.

## Skin

`data/skins/default/skin.toml` holds the colours and sizes of the built-in skin. It is a small subset of TOML: a top-level `version`, a `[colors]` table with the palette colours as `"#RRGGBB"`, and a `[sizes]` table with `row_height_px` and `padding_px`. The app reads the file and passes the text to `skin::parse`, which makes the theme through `Theme::new`. A missing or unknown key, a bad colour or number, a size outside the theme limits, or a version outside `SKIN_VERSION_OLDEST..=SKIN_VERSION` is refused with a `SkinError` that names the line or the key. The parser checks the version when it reads it, so a newer file gets the version error and not an error for one of its new keys.

The format is append-only: a new version adds keys and never renames or removes one. Every key of the version of the file is required.

| Version | Colours | Sizes |
| --- | --- | --- |
| 1 | `background`, `panel`, `border`, `text`, `text_dim`, `focus`, `focus_text`, `accent` | `row_height_px`, `padding_px` |
| 2 | Version 1, plus `warning` and `error` | As version 1 |

A version 1 file still loads. It gets `warning` and `error` from `Palette::default()`, and a `warning` or `error` key in it is an unknown key. The committed file is version 2, and a test checks that it gives `Theme::default()`. Images, fonts and sounds of a skin come after the MVP.

The default colours of version 2: `warning` is orange `#FF8020` and `error` is a light red `#FF6060`. `warning` is 19 degrees of hue away from the amber focus bar (`#F0C030`) and has two thirds of its luminance, so warning text does not read as a focused row. Both stay readable on the panel (contrast 5.7:1 and 4.9:1), and dark text on them reads at 7.3:1 and 6.2:1. The red has green and blue in it: pure red text smears on a CRT and has low luminance.

## Golden images

The goldens of the scenes live in this crate, in `crates/neptron-ui/tests/golden/` (`golden::GOLDEN_DIR` is relative to the crate). The root `tests/golden/` is for application-level screenshots, such as the end-to-end runs of the app.

The format is binary PPM (`P6`, maxval 255): the header `P6\n320 240\n255\n`, then RGB rows. The unused byte of XRGB is not stored. The test `tests/golden.rs` renders every scene and compares the bytes. Every value in the pixel path is an integer (positions, sizes, slider fill, scroll bar), so macOS and Linux give the same bytes.

## No allocation per frame

`UiScene`, `UiTree` and the intent list keep their memory on `clear()`. Text goes into the store of the scene; numbers are formatted with `StackText` on the stack. `CpuCanvas` makes its buffer once in `new()`; drawing writes into it. A unit test draws every registered scene twice and checks that no buffer grew or moved. That test is a proxy: the crate forbids `unsafe`, so it cannot install a counting allocator. Fixture data of the scenes (the `Vec<MenuItem>` of a scene) is made when the scene is built and is not part of the frame.

For the router, the tests prove three things about the scene, the tree, the intent list and the screen stack: a static screen or overlay keeps their capacity after its first frame; a round trip from the top of a 10,000-game list to its end and back keeps it after one warm-up round trip; and a confirmation, a notice and the quick menu that open and close over a game keep it after one warm-up round, with and without the video block. What can still grow: a frame with more or longer text than any frame before it grows the scene and the tree once, to the new size; opening the quick menu allocates one box for its state, which is freed when it closes; the snapshot belongs to the app and is not counted.

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
