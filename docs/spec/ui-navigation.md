# UI navigation: the Settings page, the row arrow and the breadcrumb

This spec settles OP-090. It changes how the menus are put together. It does not change a setting, a level, the guard or the confirm dialog. `docs/flows/07-screens.md` describes the screens as built.

## Facts that drive the design

- The snapshot holds the rows of one settings page at a time (`Snapshot::settings`). A screen asks for its page with `Intent::LoadSettings`, and the app answers in a later frame.
- Each screen on the stack of the router keeps its own cursor. A screen that closes gives the screen under it back with its cursor.
- The quick menu notes each change at the game level, so that the keep prompt can move it to the system or the global level. Today it reads the scopes of a noted row from the snapshot when the prompt opens. With sub pages that row is often not in the snapshot then.
- The tier 3 boot golden `os/tests/golden/boot-game-list.ppm` shows the empty home screen under a notice: the empty-state text and one row `Display`, with the focus, across the full width of its box. Tier 3 needs QEMU. The orchestrator rebuilds the images and blesses that golden for this change.
- The control protocol (`neptron-ipc`, version 1) is append-only. A new reply field needs a new protocol version, and the handlers do not see the version of a request today.
- The session ignores the quick menu hotkey while the core is paused (`Hotkey::QuickMenu` only acts in `State::Running`). The end-to-end test checks that a second hotkey does not close the menu.

## Home

Home lists the systems that have games, then `Favourites`, `Recent games` and one row `Settings`. The rows `Display` and `Audio` are no longer on home.

Home with no games follows the same rule: the text that says where the games go, and one row `Settings` at the bottom, with its arrow. So Settings and every panel are reachable on a new stick with no games, when a user needs them most. The display panel is two presses away: `Settings > Display`. No notice or hint text names the row. The boot golden of tier 3 changes for this reason; the orchestrator blesses it.

## The Settings page

`Settings` opens a page that lists the settings panels. The list is data: `screens::SETTINGS_PANELS`, a static table of `Panel { label, page }` in `neptron-ui`. Today it holds `Display` (`DISPLAY_PAGE`), `Audio` (`AUDIO_PAGE`), `Library` (`LIBRARY_PAGE`) and `System` (`SYSTEM_PAGE`), in that order. A new panel is one line in that table and one page id; the app then answers `LoadSettings` for that page. A panel page other than the display page shows in the generic page screen (see "Pages"), so a new panel needs no new screen code.

The Settings page writes nothing. Its panels write at the global level, also when the page opens from the quick menu.

Screen id: `settings`.

## Rows: one rule for every list

A row has one of four kinds. The kind decides how the row looks, what accept does and what the footer says.

| Kind | What the row does | How it looks | Footer, left |
| --- | --- | --- | --- |
| `page` | Accept opens a page, an option list or the grid pattern | A right arrow at the right edge of the row | `Accept: open` |
| `value` | Changes its value in place | `< value >` while focused (a number), the value alone (a switch) | `Left/Right: change` for a number, `Accept: change` for a switch |
| `action` | Accept does something: resume, quit, play a game, use an option. An action row with a confirm question opens a dialog first (see "The confirm question of an action row") | Neither an arrow nor `< >` | `Accept: choose`, or the hint of its screen (`Accept: play`, `Accept: use`) |
| `disabled` | Nothing: the row is unavailable now | Dim, with its reason, no arrow. With the focus: a solid bar in the `border` colour with the text in `text`, the look of a focused option that cannot be chosen (a one-pixel edge can vanish under scanlines) | none |

The kind of a settings row comes from its value:

- `SettingValue::Page(id)` (new): `page`. Accept opens page `id`.
- `SettingValue::Choice` in a list with pickers (every screen): `page`. Accept opens the option list.
- `SettingValue::Toggle` and `SettingValue::Number`: `value`.
- `SettingValue::Action(id)`: `action`.
- Any row with a reason (`MenuItem::unavailable`): `disabled`.

The kind of a `List` row is in `ListItem::kind`. Home rows and Settings rows are `page`; game rows and option rows are `action`.

`SettingValue::Page` replaces the reserved actions `OPEN_DISPLAY` and `GRID_PATTERN`. The grid pattern gets the reserved page id `GRID_PAGE`: it is not a settings page, but a row that opens it is a `page` row like the others.

### The arrow

The arrow is a small shape drawn with rectangles, not a glyph. The font has `>`, but a `>` at the right edge reads as the right half of `< value >`. The arrow is a right-pointing triangle, 4 px wide and 7 px high: four columns of 7, 5, 3 and 1 px, centred on the row. It sits at the right edge of the row inside the padding, in a column of `ARROW_COLUMN_PX` = 8 px that every row of a settings list keeps free, so the value and source columns of all rows stay aligned. Colour: `focus_text` on the focused row, `text_dim` on the others. The arrow uses colours of the skin and fixed sizes, so the skin format does not change.

With the default skin the label column of a settings row is 118 px (19 characters) wide, 114 px with a scroll bar. `Hold Start for menu` (19 characters) still fits.

### The value column

A settings row shows a short form of a long value. `ChoiceOption` gets an optional `short` text (`ChoiceOption::with_short`). The row shows `short` when the option has one, else the label. The option list always shows the full label. The app sets `short` for an audio output: the head of the label before ` (`, so `HDMI/DP 1 (HDA Intel PCH)` shows as `HDMI/DP 1` and `Analog (HDA Intel PCH)` as `Analog`. Any other option can use it.

### The tree

`TreeRow` gets `kind: Option<RowKind>`: `Some` for a row of a list or a settings list, `None` for a label, a dialog text, a button and a video line. The control protocol does not carry the kind in this task, because that needs protocol version 2 (see "Not in this task"). The UI crawler reads the kind from the tree of the frame (`Shell::ui`), not from the protocol.

## The breadcrumb

The title band of every page below home and of every quick menu page shows where the player is. The parts are, from the root:

- Below home: `Settings`, then the label of each row that opened a page.
- In a game: the title of the game, then the label of each row that opened a page.
- An option list adds the label of its row.

Examples: `Settings`, `Settings > Display`, `Settings > Audio > Audio output`, `Astro Racer`, `Astro Racer > Video`, `Astro Racer > Video > Picture style`, `Astro Racer > Settings > Display`. The game list keeps its title (the system name and the count). Home keeps `NEPTRON`.

The separator is ` > `: space, `>`, space. The font has `>`.

### Truncation

The title box is 280 px (46 characters) with the default skin, or 232 px (38 characters) on the first page of the quick menu, which keeps `Paused` on the right. The rule, in characters of the font:

1. The whole path, when it fits.
2. Else, with three parts or more: `... > ` and the last two parts.
3. Else the second last part is cut at its end and ends with `...`. With two parts there is no `... > ` before it.
4. When that leaves fewer than 4 characters for the second last part (one character and `...`), it goes: `... > ` and the last part.
5. When the last part alone does not fit, it is cut at its end with `...`.

Examples at 38 characters: `Astro Racer > Settings > Display > Monitor profile` (50) gives `... > Display > Monitor profile` (31). `Super Mario Bros. 3 (USA) (Virtual Console) > Video` (51) gives `Super Mario Bros. 3 (USA) (... > Video` (38): the title keeps its first 27 characters and ends with `...`.

The text is formatted into a `StackText` and allocates nothing. The tree has one label row with the text as drawn. When the rule shortened the path, the row has the whole path as its value. The row is not marked as clipped: the cut is the rule, not a layout fault.

## Back and the focus

Back closes the screen on top and nothing else. The screen under it keeps its cursor, so the focus is on the row that opened the closed screen. This holds at every level: Settings, a panel, an option list, the quick menu and its sub pages. On the first page of the quick menu Back resumes the game, as today (through the keep prompt after a change).

A page whose rows are not in the snapshot yet takes only Back. An up, down or accept in that frame would clamp the cursor to an empty list and lose the row the player came from.

## Footer hints

The left hint follows the kind of the focused row (see the table above). A page with no rows loaded shows no left hint. The right hint: `Back: return` on every page below home and every quick menu sub page, `Back: resume` on the first page of the quick menu, `Back: cancel` in an option list. The game list, the dialogs, the notice and the keep prompt keep their hints.

## Pages

A page that is not the display panel, the Settings page, the grid pattern or the first page of the quick menu shows in one generic screen: a framed box with the rows of the page, under the breadcrumb. It writes at the level of the screen that opened it: the global level from the Settings page, the game level from a quick menu page. The display panel always writes the global level.

The generic screen shows the video block above its rows for `VIDEO_PAGE` while a game runs with a video status, with the rule of today (at least 2 rows under it).

Screen ids: `audio` for `AUDIO_PAGE`, `library` for `LIBRARY_PAGE`, `system` for `SYSTEM_PAGE`, `quick-menu/video` for `VIDEO_PAGE`, `page` for any other page.

### The Library panel

`Settings > Library` holds the `Rescan games` action row of the app (no arrow). While a scan runs the row keeps its label and is disabled with the reason `Scanning...`. In a game the panel is still listed, and the row is disabled with the reason `Not during a game`: one list of panels for the menus and the game is simpler than a list that changes. The row keys of the app are all different: the `Settings` row of the quick menu has its own key, and a test checks it.

### The System panel

`Settings > System` holds two action rows of the app, with no arrow: `Reboot` and `Shut down`, and a third row when a format of the share partition is on offer (see below). Each row has a confirm question, so accept opens a dialog first:

| Row | Dialog title | Dialog message | Buttons |
| --- | --- | --- | --- |
| `Reboot` | `Reboot NEPTRON?` | `Settings and saves are written first. Then the machine starts again.` | `Cancel`, `Reboot` |
| `Shut down` | `Shut down NEPTRON?` | `Settings and saves are written first. Then the machine turns off.` | `Cancel`, `Shut down` |

When the player confirms, the app ends its run as a quit (the settings and the guard record are written, the threads are joined, the display stays in the last validated mode) and exits with a code that asks the launcher for a reboot or a power-off. The launcher then asks init, and init runs the shutdown steps of the image (`docs/flows/01-power-on.md`, "Shutdown"). A confirm before the boot is good gives back the failed boot that the guard counted at the start, but only on a boot the guard does not hold to be failed (`Confirmed`, `Safe(NothingConfirmed)`). After `Safe(NoValidMode)` the count stays, so boots with no valid picture still reach `Safe(FailedBoots)`; `Safe(FailedBoots)` and `Safe(Recovery)` keep their own rules.

In a game the panel is still listed, and both rows are disabled with the reason `Not during a game`, the rule of the rescan row. A game has save RAM that only its own quit writes in full, so the player quits the game first and a save is never cut.

A run that no launcher with power control started (the desktop window, a bench run of the frontend alone) shows both rows disabled with the reason `Needs the launcher`. The rows stay in the list, so the panel looks the same on every rig and the crawler finds it everywhere. A hidden row would make the desktop and the image differ. The launcher gives power control only with a DRM device, never with `--no-device`, so a desktop run can never reboot the computer it runs on.

#### Format share

A third action row, `Format share`, comes after `Shut down`, but only when the storage script offers a format: `storage.toml` holds `share_fix = "format"` with the device, start, size and type of partition 4 (`os/README.md`, "States a user-confirmed format may fix"). With no offer the row is absent, not disabled: no other state has a format that can fix it. In a game the row is disabled with `Not during a game`, and with no power control with `Needs the launcher`, as the other two rows.

| Row | Dialog title | Dialog message | Buttons |
| --- | --- | --- | --- |
| `Format share` | `Format the share partition?` | `NEPTRON finds no file system it can read on the partition for games (<size>). Formatting it erases everything on it.` | `Cancel`, `Format share` |

`<size>` is the size that `storage.toml` names, in decimal units with one decimal: `56.6 GB`, `512.0 MB`. The focus starts on `Cancel`, as for every confirm question.

The frontend never formats. On the confirm it writes the request file `/state/share-format-request` (the device, start, size and type of the offer, one `key=value` per line) through the writer thread, with the write rule of user data (temp file, fsync, rename), shows the working box `Formatting at the next start...`, and ends the run with the reboot outcome (exit code 80), as `Reboot` does. At the next boot the storage script reads the request, removes it first, checks every condition again on the disk, and only then formats and mounts. A request that does not match the disk formats nothing.

The Warning notice of a share in a RAM folder gains one sentence when a format is on offer: `Settings > System can format it.` After a boot with a request, `storage.toml` holds `share_format` (`done`, `refused` or `failed`) and, when it is not `done`, `share_format_reason`. The frontend shows `The share partition is formatted.` as an Info notice, or `The share partition was not formatted (<reason>).` as a Warning.

Not in this panel yet: the version, the network address and a sleep timer.

### The confirm question of an action row

`MenuItem::with_confirm(question, message, working)` gives an action row a confirm question. Accept on such a row opens the dialog widget over its page, and sends nothing yet:

- The title is the question and the message says what happens. The buttons are `Cancel`, then the label of the row.
- The focus starts on `Cancel`, so a blind press of accept cancels. Only right, then accept, sends the action of the row.
- Back cancels. Accept on `Cancel` cancels. A cancel sends nothing, and the focus stays on the row.
- When the row is gone from the page, is disabled, or has no question any more, the dialog closes and sends nothing.
- The hints are `Accept: choose` and `Back: cancel`. The tree holds the rows of the dialog only, under the screen id `confirm-action`.
- After the confirm, a box with the label of the row as its title and the third text of the question (`Shutting down...`, `Restarting...`) replaces the dialog, with no buttons and no hints, under the screen id `confirm-action/working`. The app presents this frame once, through the normal present in the mode on screen, and ends the run; the launcher keeps it on the wire until init stops it. So a slow or failed shutdown never looks like a frozen dialog. The box takes only Back, which closes it, so a run that does not end leaves no box the player cannot close.

## The quick menu

The first page of the quick menu (`QUICK_MENU_PAGE`, written at the game level):

| Row | Kind | What it does |
| --- | --- | --- |
| `Resume` | action | Leaves the menu, through the keep prompt after a change |
| `Volume` | value | The volume of the game, Left and Right step 5 |
| `Video` | page | Opens `VIDEO_PAGE` |
| `Settings` | page | Opens the Settings page: Display, Audio, Library and System, at the global level |
| `Quit game` | action | Ends the game, through the keep prompt after a change |

`VIDEO_PAGE` (new, written at the game level): the video block, then `Picture style`, `Refresh policy`, `Scanlines` and `Scanline strength`. These are the display rows of the quick menu of today.

Every function of the quick menu of today stays reachable: the four display rows under `Video`, the volume on the first page, and the global display panel under `Settings > Display`. The global audio panel is now reachable in a game too.

### The keep prompt with sub pages

The quick menu notes a change at the game level from any screen above it, as today. It now also notes the scopes of the row when it notes the change, from the page in the snapshot of that frame (the page that sent the change). The prompt moves a change to the system or global level when those noted scopes allow the level. The app checks the scopes of each `SetValue` again.

A change that the app took back (the guard reverted it) must not be asked about. The quick menu checks this at two moments: when the player leaves a sub page with Back, and when the prompt would open. It checks a change only against the page that made it (the page in the snapshot of that frame). A global panel holds the same keys with a value that never comes from the game level, so its rows must not count. A change whose page is not in the snapshot at that moment stays noted. The UI takes no more actions in the frame of a change at the game level: the snapshot of that frame still shows the old value, and the confirm dialog of a display change opens in the next frame. So the player is still on the page of the change when the dialog opens, the dialog takes every action until it closes, and the player leaves the page later with Back, which checks the change.

### The hotkey

The quick menu hotkey does nothing while the menu is open, at any depth. The session ignores it while the core is paused, so the UI gets no `Menu` action from it. This is the rule of today, and the end-to-end test checks it. The menu closes only through Resume or Back on its first page, and Quit game. The Menu button (X) on a settings row resets the value at the level of the page, on every page.

## Ids

| Id | Value | Use |
| --- | --- | --- |
| `DISPLAY_PAGE` | `PageId(RESERVED_ID_MIN + 1)` | The display panel (unchanged) |
| `QUICK_MENU_PAGE` | `PageId(RESERVED_ID_MIN + 2)` | The first page of the quick menu |
| `AUDIO_PAGE` | `PageId(RESERVED_ID_MIN + 3)` | The audio panel (unchanged) |
| `GRID_PAGE` | `PageId(RESERVED_ID_MIN + 4)` | The grid pattern, as the target of a `page` row |
| `SETTINGS_PAGE` | `PageId(RESERVED_ID_MIN + 5)` | The Settings page, as the target of a `page` row. The app never loads it |
| `VIDEO_PAGE` | `PageId(RESERVED_ID_MIN + 6)` | The video page of the quick menu |
| `LIBRARY_PAGE` | `PageId(RESERVED_ID_MIN + 7)` | The Library panel: the rescan of the games folder |
| `SYSTEM_PAGE` | `PageId(RESERVED_ID_MIN + 8)` | The System panel: reboot, shut down, and the format of the share partition when it is on offer |
| `RESUME`, `QUIT_GAME` | unchanged | The actions of the quick menu |

`OPEN_DISPLAY` and `GRID_PATTERN` go: `SettingValue::Page(DISPLAY_PAGE)` and `SettingValue::Page(GRID_PAGE)` replace them. `MAX_DEPTH` stays 8. The deepest stack is home, game list, game, quick menu, Settings, display panel, option list: 7 screens.

## Scenes and goldens

New scenes: `screens/system` (the System panel), `screens/system-shutdown-confirm` (the confirm question of `Shut down`, with the focus on `Cancel`), `screens/system-shutting-down` (the last frame after the confirm), `screens/settings` (the Settings page), `screens/breadcrumb-long` (a long game title cut by the rule, on the video page) and `screens/library-scanning` (the Library panel during a scan, with a focused disabled row). Existing scenes show the other states: `screens/audio-output` an option list with a three-part breadcrumb (`Settings > Audio > Audio output`), `screens/quick-menu-video` the video page of the quick menu with the video block, `screens/home` the `Settings` row and `screens/quick-menu` the first page. Every golden that shows a settings row, a list row that opens a page, a title or a hint changes; the task report lists each one. `screens/home-empty` shows the `Settings` row with its arrow, and the tier 3 boot golden changes with it.

## Tests

- The widget: the arrow on a `page` row, none on a `value`, `action` or `disabled` row, and the tree kind of each; the aligned columns with the arrow column; the short value in the row and the full label in the option list.
- The breadcrumb: each step of the truncation rule, with long names, and the 38 and 46 character widths.
- The router: Back closes one level and gives the focus back to the row it came from, at every level, from home and from the quick menu; a page that is not loaded takes only Back; the footer hint of each kind.
- The quick menu: a change on the video page is moved by the keep prompt; a change that the guard took back on the video page is not asked about; the hotkey case is a tier 2 test that exists.
- The empty home: one `Settings` row of the kind `page`, which opens the Settings page; the scene `screens/home-empty` shows it.
- The crawler (tier 2): every row that opens a screen has the kind `page`, every enabled `page` row opens a screen, Back from every screen returns to the screen under it with the focus on the row it came from. It finds the System panel and never accepts `Reboot` or `Shut down`: accept opens their confirm question, and a confirm ends the run.
- The confirm question: accept opens it with the focus on `Cancel`; accept on `Cancel`, and back, send nothing; right, then accept, sends the action once; a row that goes away or is disabled closes it.
- The System panel (tier 2): confirm `Reboot` ends the run with `Outcome::Reboot`, confirm `Shut down` with `Outcome::PowerOff`, cancel changes nothing, and in a game both rows are disabled.
- The format row (tier 2): an offer in `storage.toml` shows the row, and no offer hides it; confirm writes the request file with the numbers of the offer and ends the run with `Outcome::Reboot`; cancel writes nothing; in a game and with no power control the row is disabled. The crawler, with an offer, finds the row and never accepts it. Scenes `screens/system-format` (the panel with the row) and `screens/system-format-confirm` (its question, focus on `Cancel`).

## Not in this task

- The kind in the control protocol (`ui_tree` of `neptronctl`). It needs protocol version 2 and the request version in the handlers: a contract change with its own branch and ADR.
- The `Network` panel. Its task adds a line to `SETTINGS_PANELS`, a page id and the page in the app.
