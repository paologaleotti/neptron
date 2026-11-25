# UI layout: one layout code for a landscape and a portrait canvas

This spec defines how the menus of `neptron-ui` lay out on a canvas of more than one size: the landscape canvas of today (320x240) and a portrait canvas (240x320) for a screen turned on its side (TATE, `docs/spec/tate.md`). It gives the layout model, an inventory of every screen and widget, the change to the canvas contract (ADR 0021), the fate of the interim code of TATE package 3, the goldens and the tests, and the work packages. It is a design only: no product code comes with it.

## 1. The owner's decisions

The owner decided on 2026-11-24, in their words: "ofc second layout!! let the ui be somewhat responsive. ofc. that is hard requirement. no sense of squeezing anything. tate mode ui needs to be specific and well done. also in future ui will be much more complex with art box, themes, HD only ui etc."

And, on the same day, in answer to the first questions of this design:

1. A screen at 90 or 270 degrees gets the portrait menu. A screen at 180 keeps the landscape menu, turned.
2. A flat panel mounted in portrait also gets the portrait menu, scaled up sharp at a whole factor.
3. The quick menu over a horizontal game on a turned screen is the portrait menu at full size, with the small upright game behind it.
4. A text that does not fit a line of the portrait menu wraps to a second line. A shorter wording only where a wrap looks bad. Section 5.4 lists those cases with a proposed short text, for the owner's approval.

What follows from them:

- On a turned screen the menu is a portrait canvas, 240 wide and 320 high as the player sees it. It is never squeezed and never interlaced to fit. On a 15 kHz tube it uses the 240p menu mode of a normal screen: 240 canvas columns on 240 scan lines, 1:1.
- The layout follows the size of the canvas: one layout code that adapts, not a second copy of every screen.
- Box art, themes and a high-definition UI come later. This design does not design them, but it keeps the door open (section 11).

The owner answered every question of this design on 2026-11-24 ("Yes to all", to the recommendations). Section 14 lists the decisions; each one has the mark **Decision: Owner, 2026-11-24** where it applies in the text.

## 2. Terms

**Canvas.** The CPU image that `neptron-ui` draws: XRGB8888, upright for the player.

**Form factor** (`Form`). The shape of the canvas: `Landscape` (320x240) or `Portrait` (240x320). A closed set of named values, not an arbitrary size (section 3.6).

**`T`.** The canvas after its turn `U` (ADR 0020), in scan-out pixels. For a portrait canvas turned by `R90` or `R270`, `T` is 320x240: the size of the landscape canvas of today.

**Metrics.** The numbers of a layout (margins, safe area, bands, rows, columns, line widths in characters), computed once from the form factor and the theme.

**Character.** One cell of the Spleen 6x12 font: 6 pixels wide, 12 pixels high. "34 characters" means 204 pixels.

## 3. The layout model

### 3.1 The principle

The UI decides the layout from numbers that one pure function computes from the canvas size and the theme. A screen does not know the form factor by name. It reads its rectangles and its character counts from a `Metrics` value. Where a screen must choose a shape (one line or two for a settings row, two or three lines for a title box), the choice is a rule on a character count, not a test of the form factor. So a later form factor (a larger canvas for a high-definition UI) gets the right shape from the same rules.

Why not a general constraint solver: the screens are few and regular (a title band, one or two framed boxes, a hint band), and every rectangle of today already comes from `chrome::Layout` and `SAFE_AREA`. A solver adds code, tests and surprises for no case that the screens have. The smallest model that covers the real screens is one value, computed once, passed to every screen and widget.

### 3.2 The value

```rust
/// The shape of the canvas.
pub enum Form { Landscape, Portrait }

/// The numbers of a layout, computed once from the form and the theme.
pub struct Metrics {
    pub form: Form,
    pub canvas: Size,          // 320x240 or 240x320
    pub safe: Rect,            // the overscan-safe area
    pub title_band: Rect,      // one line; a second line when needed (3.4)
    pub content: Rect,         // between the bands, for a one-line hint band
    pub row_px: u32,           // from the theme
    pub padding_px: u32,       // from the theme
    pub text_chars: u32,       // characters on one line of a framed box
    pub dialog: Rect,          // the width and the horizontal place of a dialog
    pub dialog_chars: u32,     // characters on one line of a dialog message
    pub settings_row: RowShape,     // OneLine or Stacked (3.4)
    pub game_title_lines: u32,      // the box under the game list
    pub help_lines: u32,            // the box under an option list
    pub video_lines: u32,           // the video block
}

impl Metrics {
    /// The metrics of `form` with `theme`. Pure: no I/O, no clock.
    pub fn of(form: Form, theme: &Theme) -> Self;
}
```

`Metrics::of` replaces the constants `UI_WIDTH`, `UI_HEIGHT`, `SAFE_MARGIN_X_PX`, `SAFE_MARGIN_Y_PX`, `SAFE_AREA` and `DIALOG_WIDTH_PX`, and the fixed counts `TITLE_LINES`, `HELP_LINES`, `video::LINES`, `MAX_VISIBLE_ROWS` and `grid::MAX_COLUMNS`. `chrome::Layout` becomes a view of `Metrics` (title, content, footer). The router holds one `Metrics` and gives a reference to every screen and widget, as it gives `&Layout` and `&Theme` today.

### 3.3 The numbers

The rules, each with its reason:

- **Safe area: 5 % of each axis on each side.** It is the rule of today (`SAFE_MARGIN_X_PX = UI_WIDTH / 20`). A turned tube overscans as a normal tube does, and the margin follows the canvas axis that the player sees. Portrait: 12 px at the left and the right, 16 px at the top and the bottom. Decision: Owner, 2026-11-24 (D5).
- **Title band:** one row and 4 px, then a 2 px rule, then a 4 px gap. Unchanged.
- **Hint band:** one row and 2 px, at the bottom of the safe area; a second line when its two texts do not fit one line (3.4).
- **Framed box:** 1 px sides, 2 px top and bottom lines, 1 px gap (`BOX_EDGE_H_PX`). Unchanged.
- **Line width in characters:** the safe width less the box sides and the padding, divided by 6.

| Value | Landscape 320x240 | Portrait 240x320 | Note |
| --- | --- | --- | --- |
| Canvas in characters | 53 x 20 | 40 x 26 | The whole canvas; no text goes into the margins |
| Safe area | (16, 12) 288 x 216 | (12, 16) 216 x 288 | 5 % per side |
| Content (one-line hint band) | y 34, 288 x 176 | y 38, 216 x 248 | |
| Content (two-line hint band) | y 34, 288 x 164 | y 38, 216 x 236 | |
| Rows in one box filling the content | 14 | 20 | (h - 6) / 12 |
| `text_chars`: a line in a framed box | 46 | 34 | (safe w - 4 - 2 x 4) / 6 |
| Title band text | 46 | 34 | less the right text and 2 characters |
| Dialog | 224 wide, x 48 | 216 wide, x 12 | Portrait: the whole safe width |
| `dialog_chars` | 35 | 34 | (w - 2 - 2 x 4) / 6 |
| Dialog message lines, most | 14 | 20 | |
| Settings row | `OneLine`: label 21, value 14, source 7 | `Stacked`: label 32 on line 1; source 7 and value 22 on line 2 | Rule in 3.4 |
| Game title box | 2 lines (92 characters) | 3 lines (102 characters) | |
| Game list rows | 11 | 16 | |
| Option list rows and help box | 11 rows, 2 help lines | 15 rows, 4 help lines | |
| Video block | 5 lines | 7 lines | |
| Display page: facts box and rows | 3 facts, 10 one-line rows | 3 facts, 8 stacked rows | |
| Grid pattern | 20 x 15 cells of 16 px | 15 x 20 cells of 16 px | |
| Screensaver free area | 236 x 216, start (118, 108) | 156 x 296, start (78, 148) | The word is 84 x 24 |

The brief of this work names 53 and 40 characters: those are the canvas widths. The text of a framed box has 46 and 34 characters, because of the safe margins, the box and the padding. Every rule below uses `text_chars`.

The numbers with the largest skin values (`ROW_HEIGHT_MAX_PX` 24, `PADDING_MAX_PX` 16) hold in portrait too: a dialog keeps 30 characters (216 - 2 - 32 = 182 px) and more than three message lines. The theme limit tests run in both forms.

### 3.4 Rules for text that does not fit

`text.rs` has today: `fit` (cut, with `...` when the width has room), `draw_in` (fit, align, centre vertically, report a cut), `wrap` (break at spaces, inside a word when it must, `\n` ends a line), `StackText::push_cut`, and `chrome::paragraph` (wrap into a box, `...` on the last line when text is left). `breadcrumb::fit` shortens each part of a path. These stay, and every rule below uses them.

The rules, in order of preference:

1. **Wrap** (the owner's rule 4). A text that does not fit its line goes on to a second line. Where:
   - A list row label: the row takes two lines (3.5). The second line starts at the column of the first.
   - A band of two texts (the title band with its count, the hint band with two hints): when both do not fit one line with two characters between them, the left text takes line 1 and the right text takes line 2, right-aligned. The content area then starts or ends one row later. Decision: Owner, 2026-11-24 (D4).
   - A field (`chrome::field`, a name column of 9 characters and a value): the value wraps inside its column.
   - A paragraph, a dialog message, a help box, a notice: as today, with the line count of the form.
2. **Stack.** A settings row whose label, value and source do not fit one line in fixed columns (the label column under 18 characters) is `Stacked`: the label on line 1 with the arrow at the right; the source (indented by 2 characters) and the value (right-aligned) on line 2. A disabled row shows its reason as the value. The rule is on the character count, so a wide canvas keeps `OneLine`.
3. **Shorten with the mark.** Only where the whole text shows in another place on the same screen, or where the text has no bound: a game title in the game list (the title box under the list shows it whole), a part of the breadcrumb (`breadcrumb::fit`), a name from a device or a file that has no length limit. The new function `text::draw_short` shortens and does not report a fault; `draw_in` keeps its meaning: a cut is a fault.
4. **Scroll.** A list longer than its box scrolls, as today. A text that does not fit its box after the wrap is cut on its last line with `...` and reported as clipped: the fit check (section 9.4) fails on it.

Two rules are new in `text.rs`:

- **No break between a number and its unit**, or inside a number: `10 s`, `59.94 Hz`, `1920x1080`. `wrap` treats a space after a digit and before a unit word of a short fixed list (`s`, `Hz`, `kHz`, `%`, `px`, `ms`) as a non-breaking space. A test covers each unit.
- **No line of one short word.** When the last line of a wrapped text would be one word of 5 characters or fewer, and the text is a dialog message or a notice, the fit check reports it as a warning in the scene list (not a failure). Section 5.4 lists the texts where it happens, with a short text for approval.

Line 1 of a wrapped row never ends in a space, and a wrapped row keeps its whole height in the focus bar, so the focus covers both lines.

### 3.5 Rows of one or two lines

`List` today has one row height. It gets rows of one or two lines:

- A row has 1 line, or 2 when its label wraps (rule 1) or when the widget asks for a stacked row (rule 2). Never 3: a label longer than 2 lines is shortened on line 2 with the mark and reported.
- The window of visible rows keeps the focused row whole. Up and Down move by one row; Left and Right move by one page, where a page is the rows that fit the box.
- The scroll bar measures rows, not lines.
- The game list keeps rows of one line: its row pitch must be constant for a window of 10,000 rows, and the title box shows the whole title. Decision: Owner, 2026-11-24 (D3).

### 3.6 What is not supported

- **No arbitrary canvas size.** The form factor is one of the named values. `Metrics::of` takes a `Form`, not a size. A test runs every rule for both values. A later form factor is a new value of the enum, with its goldens and its fit check; the rules are written from sizes so that it needs no new layout code.
- **No font scaling inside the canvas.** The font stays 6x12. A larger picture comes from the scale of the canvas into the mode, as today.
- **No layout change in the middle of a frame.** The form changes only when the screen orientation or the display changes; the router lays out every screen again before the next frame (section 6.6).
- **No two-pane layout.** No screen of today has panes side by side, so the portrait form needs no rule to move a pane under another.

## 4. Inventory of the code

### 4.1 Where the size is assumed

Counted on `main` at `a979f9d3`, code outside `#[cfg(test)]`, `screens/tests.rs` and `screens/scenes.rs`:

| File | Uses of `UI_WIDTH`, `UI_HEIGHT`, `SAFE_AREA`, `SAFE_MARGIN_*`, `DIALOG_WIDTH_PX`, `Layout::new` | Other fixed sizes | What changes |
| --- | --- | --- | --- |
| `screens/pattern.rs` | 26 | `CENTRE_X`, `CENTRE_Y` from the canvas | The grid from `metrics.canvas`; a portrait grid of 15 x 20 cells |
| `screens/chrome.rs` | 15 | `FIELD_NAME_CHARS` 9, `GAP_PX`, `RULE_PX` | `Layout` from `Metrics`; two-line bands; the field wrap |
| `canvas.rs` | 11 | `CpuCanvas::new` is 320x240 | `CpuCanvas::with_size(Size)`; the rest is already generic (`width`, `height` fields) |
| `widgets/dialog.rs` | 11 | `DIALOG_WIDTH_PX` 224, `max_lines` from `SAFE_AREA` | Width, place and lines from `Metrics` |
| `registry/demos.rs` | 11 | `HEADER_HEIGHT_PX` 16 | The demos of each widget take `Metrics` |
| `theme.rs` | 9 | `SAFE_AREA`, the margins | The constants move into `Metrics::of` |
| `screensaver.rs` | 4 | none: `pose` already takes the size | `draw` passes the canvas size |
| `painted.rs` | 2 | `SceneCanvas` is 320x240 | `SceneCanvas::new(Size)` |
| `registry.rs` | 2 | | The render of a scene takes the form |
| `widgets/list.rs` | 1 | `MAX_VISIBLE_ROWS` = canvas height | Rows of 1 or 2 lines (3.5) |
| `widgets/grid.rs` | 1 | `MAX_COLUMNS` = canvas width | From `Metrics` |
| `screens/router.rs` | 1 | builds `Layout` once | Holds `Metrics`; lays out again on a form change |
| `screens/pad_mapping.rs` | 1 | `PROGRESS_H_PX` | `Layout` from the router |
| `widgets.rs` | 1 | `ARROW_COLUMN_PX` | none |
| `lib.rs` | 1 | re-exports `UI_WIDTH`, `UI_HEIGHT` | Re-exports `Form`, `Metrics`, `Size` |
| `widgets/settings_list.rs` | 0 | `VALUE_CHARS` 14, `SOURCE_CHARS` 7 | `RowShape`; the stacked row |
| `screens/game_list.rs` | 0 | `TITLE_LINES` 2 | `metrics.game_title_lines` |
| `screens/options.rs` | 0 | `HELP_LINES` 2, `HELP_BYTES` 192 | `metrics.help_lines` |
| `screens/video.rs` | 0 | `LINES` 5 | `metrics.video_lines`; one fact per line when it does not fit |
| `screens/display.rs` | 0 | `INFO_LINES` 3 | none |
| `screens/home.rs`, `page.rs`, `panel.rs`, `settings.rs`, `quick_menu.rs`, `overlays.rs`, `breadcrumb.rs` | 0 | none | They take `&Layout` today; they get it from `Metrics` |

Totals: 97 uses of the size constants in 15 files of `neptron-ui` (102 with `screens/tests.rs`), and 9 fixed counts of characters, lines or rows. The worst files are `pattern.rs`, `chrome.rs`, `canvas.rs` and `dialog.rs`. The good news: every menu screen already takes its rectangles from `chrome::Layout` or `SAFE_AREA`, never from literal coordinates. A literal coordinate shows only in the tests and the scene fixtures.

Outside `neptron-ui`:

| Place | What it assumes | What changes |
| --- | --- | --- |
| `neptron-types` `Scene::ui` | The doc says "The 320x240 UI canvas" | The doc: a canvas of the size of its `Frame` (ADR 0021) |
| `neptron-modeline` `UI_CANVAS` (`placement.rs`, `tv.rs`) | `ui_dest` of the canvas is 320x240 | `ui_dest` takes the size of `T` (section 6.3) |
| `neptron-modeline` `ui_dest_turned` | A turned 320x240 canvas (`T` 240x320) on a CRT or a TV | Removed (section 7) |
| `neptron-video` (package 3, branch `mvp/N01c`) | `TurnedMenuMode`, `Content::Menu(choice)`, `menu_plan_source` | Removed (section 7) |
| `apps/neptron/src/shell/frame.rs` `canvas_frame` | `width: UI_WIDTH`, `height: UI_HEIGHT` | The size of the canvas |
| `apps/neptron/src/shell.rs` `MENU_SOURCE` | 320x240 | Stays: it is the raster of `T` in both forms (section 6.2) |
| `apps/neptron/tests/app/crawler.rs` | `SAFE_AREA` of 320x240 | The safe area of the form of the tree |
| `neptron-gfx` GLES UI texture | Grows only; keeps the last canvas for the row compare | Compare only canvases of the same size; a new size sends the whole canvas |
| `neptron-gfx/src/gles/quad.rs` tests, `examples/gles_compare.rs` | Canvas of 320x240 | New cases with 240x320 |
| `neptron-ipc` `Stage::Ui` doc, `UiTree` | "The 320x240 logical UI canvas" | The doc; an appended field with the canvas size (section 6.8) |
| `docs/spec/screensaver.md`, `docs/spec/ui-navigation.md` | "The UI screen is always 320x240" | A line each, in package L5 |

### 4.2 The screens in portrait

The sketches are at character resolution: 40 columns by 26 lines, one cell for each 6x12 glyph box. They are close to the pixels, not exact. `[...]` is the focus bar, `>` the row arrow, `#` the scroll bar, `~` a dim text. The mock pictures of section 13 show three of them in pixels.

**Home (the system list).** Today: a list of systems with a count at the right, then `Favourites`, `Recent games`, `Settings`. Portrait: the same list, 20 rows. A long system name with its count does not fit one line ("Nintendo Entertainment System" with "12" and the arrow needs 35 of 32 characters): the row wraps (rule 1).

```
  Home
  ====================================
  +----------------------------------+
  |[Arcade                    312  >]|
  | Super Nintendo           5001  > |
  | Mega Drive                140  > |
  | Nintendo Entertainment     12  > |
  | System                           |
  | Favourites                  8  > |
  | Recent games               20  > |
  | Settings                       > |
  |                                  |
  +----------------------------------+
  Accept: open
```

Change: the list takes `Metrics`; rows of 1 or 2 lines.

**The game list of a system.** Today: 11 rows and a title box of 2 lines. Portrait: 16 rows and a title box of 3 lines. The focused row shortens a long title with `...`, and the title box shows it whole (rule 3). Mock: `portrait-game-list.png`.

```
  Super Nintendo            5001/10000
  ====================================
  +----------------------------------+
  | Kung-Fu Raiders                 #|
  | Kung-Fu Story                    |
  | Kung-Fu Quest II                 |
  |   ... 7 more rows ...            |
  | Kung-Fu Blaster II             * |
  |[The Legend of the Mystical Nin...]
  | Kung-Fu Strike II                |
  |   ... 4 more rows ...            |
  +----------------------------------+
  +----------------------------------+
  | The Legend of the Mystical Ninja:|
  | Ganbare Goemon, Special          |
  | Collector's Edition              |
  +----------------------------------+
  Accept: play           Back: systems
```

Change: `game_title_lines` from `Metrics`; the list rect from `Metrics`; the title band takes two lines when the system name and the count do not fit ("Nintendo Entertainment System" and "2/312" fit; with "5001/10000" they do not).

**A settings page with values (Display).** Today: a facts box (Output, Mode, Monitor), then one-line rows with the label, the value centred in a column of 14 characters, the source in a column of 7, and the arrow. In portrait the label column would have 9 characters: too few. Portrait: stacked rows (rule 2). 8 rows fit under the facts box. Mock: `portrait-display.png`.

```
  Settings > Display
  ====================================
  +----------------------------------+
  | Output   HDMI-A-1                |
  | Mode     1920x1080 60.00 Hz      |
  | Monitor  flat panel              |
  +----------------------------------+
  +----------------------------------+
  | Output                           |
  |   global               HDMI-A-1 >|
  | ~Monitor                         |
  |          ~Not used on a flat panel
  |[Screen orientation              ]|
  |[  global            Turned left >]|
  | Picture mode                     |
  |   default                  Auto >|
  | ~Refresh rate                    |
  |          ~Not used on a flat panel
  | Panel scale                      |
  |   default                  Auto >|
  |   ... 2 more rows ...            |
  +----------------------------------+
  Accept: open            Back: return
```

Decision: Owner, 2026-11-24 (D1): settings rows in portrait are stacked: the label on line 1, then the source and the value on line 2. The compact row (one line, the label wraps, no source column, `portrait-display-compact.png`) is not used.

**The options of a setting, with dim reasons.** Today: a mark column, the option label, the detail at the right in at most half the row, an option that cannot be chosen dim with its reason, or "Not available" when the reason does not fit half the row; the help box (2 lines) shows the help or the reason in full. Portrait: half a row is 16 characters. "Not all games" (13) fits; "Not available" (13) fits; every longer reason already goes to the help box. The help box has 4 lines (136 characters): the longest option help of today has 89 characters (3 lines).

```
  Settings > Display > Picture mode
  ====================================
  +----------------------------------+
  | * Auto                           |
  |  [Sharp lines                   ]|
  |   Game speed                     |
  |   Fit                            |
  |   Native          Not all games  |
  |  ~2x              ~Not available |
  |  ~3x              ~Not available |
  |  ~4x              ~Not available |
  |                                  |
  +----------------------------------+
  +----------------------------------+
  | Exact lines over the game's      |
  | speed. The picture can flicker or|
  | be small.                        |
  |                                  |
  +----------------------------------+
  Accept: use             Back: cancel
```

Change: `help_lines` from `Metrics`; the list rows of the option list stay one line (an option label has at most 14 characters today).

**A dialog with two buttons and a timeout (the keep prompt).** Today: 224 wide, centred, a title bar, the message wrapped at 35 characters, the buttons centred, and the hint band of the dialog ("Accept: revert", "Right, then Accept: keep"). Portrait: the whole safe width (216), 34 characters. The two hints need 40 of 34 characters: the hint band takes two lines (rule 1). "The old picture comes back in 10 s." has 35 characters: it wraps to "10 s." alone on a line, which looks bad; the owner approved "The old picture returns in 10 s." (32) in its place (D2). Mock: `portrait-keep-dialog.png`.

```
  Settings > Display
  ====================================
  +----------------------------------+
  | Output   HDMI-A-1                |
  | Mode     1920x1080 60.00 Hz      |
  | Monitor  flat panel              |
  +----------------------------------+
  +----------------------------------+
  | Output                           |
 +------------------------------------+
 |[Keep this picture?                ]|
 | Keep the new picture?              |
 | No answer brings the old one back. |
 |                                    |
 | The old picture returns in 10 s.   |
 |                                    |
 |       [ Revert ]  [  Keep  ]       |
 +------------------------------------+
  |   default                  Auto >|
  | Grid pattern                    >|
  +----------------------------------+
  Accept: revert
              Right, then Accept: keep
```

Change: `Dialog::layout` from `Metrics`; the buttons keep one row (every button set of today fits 34 characters: the widest, "Cancel" and "Set up this controller", needs 196 of 200 px).

**A notice.** Today: a dialog with a title in the tone colour, a message of up to 14 lines, and "OK". Portrait: 34 characters, up to 20 lines. The longest notice of today (the one-time notice of a vertical game interlaced, 269 characters) takes 9 lines. Nothing is cut.

```
 +------------------------------------+
 |[Warning                           ]|
 | Picture mode Game speed with       |
 | Refresh rate 50 Hz: the refresh    |
 | rate changes the game's speed.     |
 | Shown: 256x224p 15.65 kHz 50.000   |
 | Hz. The game runs at 50.000 Hz,    |
 | not 60.099 Hz: -16.80 % speed. No  |
 | scanlines: they need each game     |
 | line on two or more screen lines.  |
 |                                    |
 |              [  OK  ]              |
 +------------------------------------+
```

The rule "no break between a number and its unit" moves "50.000 Hz" whole to the next line in the real text.

**The quick menu over a game.** Today: the first page of the quick menu (Resume, Save state, ..., Video, Audio, Quit) in a box, written at the game level; its Video page shows the video block and the picture rows. Portrait: the same list of 20 rows; the Video page has a video block of 7 lines (each fact on its own line when the line does not fit) and stacked picture rows.

```
  Astro Racer > Video
  ====================================
  +----------------------------------+
  | Video                            |
  | Game   256x224                   |
  |        60.099 Hz                 |
  | Mode   768x448p                  |
  |        30.05 kHz 60.099 Hz       |
  |        2x exact                  |
  |                                  |
  +----------------------------------+
  +----------------------------------+
  |[Picture mode                    ]|
  |[  system                  Auto >]|
  | Refresh rate                     |
  |   default                  Auto >|
  +----------------------------------+
  Accept: open            Back: return
```

Change: `video_lines` and the split rule of `video.rs` (it has a two-line rule for one fact already); the keep prompt of the quick menu has three buttons, "This game", "System", "All games": 202 of 206 px, they fit.

**The guided mapping.** Today: the controller name, a band with the prompt ("Press Left"), a progress bar, three lines of help, and the countdown. Portrait: the band and the bar are full width; the help lines wrap: "Hold a button of this controller to skip it." (44) and "Back on a keyboard or another pad: cancel." (42) take two lines each. The longest prompt, "Press B (bottom face button)" (28), fits the band. The title band "Map buttons" with "Step 3 of 20" fits one line.

```
  Map buttons             Step 3 of 20
  ====================================
  +----------------------------------+
  | Arcade Stick                     |
  | +------------------------------+ |
  | |         Press Left           | |
  | +------------------------------+ |
  | ====------------------------     |
  | Hold a button of this controller |
  | to skip it.                      |
  | Back on a keyboard or another    |
  | pad: cancel.                     |
  | ~No input: cancel in 12 s.       |
  +----------------------------------+
  Back: cancel              Hold: skip
```

**The display page with its video block** (in a game, `Astro Racer > Settings > Display`). Today: a facts box, the video block with a warning line ("[!] speed (57.550 to 59.940 Hz)"), the rows. Portrait: the facts box, the video block of 7 lines (the warning line has 31 characters and fits), and the stacked rows: 4 rows fit, the list scrolls.

**The screensaver.** The word NEPTRON at twice the font size bounces in the canvas. `pose` already takes the size. Portrait: the free area is 156 x 296; the walls are the edges of the portrait canvas, as the player sees them.

**The grid pattern.** It shows how much of the canvas the monitor shows. Portrait: a grid of 15 x 20 cells of 16 px, the centre cross at (120, 160), the corner marks and the caption inside the safe area. It is the pattern that the player uses to adjust a turned tube, so it must be the portrait canvas.

### 4.3 The widgets

| Widget | Today | Portrait | Change |
| --- | --- | --- | --- |
| `List` | rows of one height; label, detail in at most half the row, arrow, scroll bar | the same; rows of 1 or 2 lines | Section 3.5 |
| `SettingsList` | label, value (14), source (7), arrows; info rows and link rows | `Stacked` rows (8 in a full box of the Display page) | `RowShape` from `Metrics` |
| `Dialog` | 224 px, 14 lines at most | 216 px, 20 lines at most | From `Metrics` |
| `Grid` | columns up to the canvas width | fewer columns; the same navigation | `MAX_COLUMNS` from `Metrics` |
| `Slider` | track in the row | the same, narrower | none |

## 5. Long texts

### 5.1 The method

Every string literal of `apps/neptron/src` and `crates/neptron-ui/src` (tests excluded) with 22 characters or more was checked: 147 texts. Each is drawn in one of two ways:

- **Wrapped** (paragraphs, dialog messages, notices, help boxes, the empty-state texts): they wrap at 34 characters in portrait. They need room in lines, not a shorter text. The line counts of 3.3 hold them.
- **On one line** (row labels, values, details and reasons, titles, buttons, hints): they need a place on one line, or rule 1 or rule 2 of 3.4.

### 5.2 One-line texts against 34 characters

| Text (characters) | Where | Portrait |
| --- | --- | --- |
| "Accept: revert" with "Right, then Accept: keep" (14 + 24) | Hint band of the keep prompt and the mapping review | Two-line hint band (rule 1) |
| "Nintendo Entertainment System" with a count (29 + 5 to 10) | Title band of the game list; home row with its count | Two-line title band; a home row of two lines |
| "Address" with "None (no cable or no network)" (9 + 29) | Information row of the Network page | Stacked row (rule 2) |
| Every setting label with its value and source, for example "Screen orientation" "Turned right" "global" (18 + 12 + 7) | Settings rows | Stacked row (rule 2) |
| "Not used on a flat panel" (24), "Not offered by this output" (26), "Not during a game" (17), "Needs the launcher" (18) | The reason of a disabled settings row | Line 2 of a stacked row: up to 30 characters, they fit |
| "Mode 768x448p 30.05 kHz 60.099 Hz 2x exact" (43) | Video block | One fact per line |
| "Astro Racer > Settings > Display > Picture mode" (47) | Breadcrumb | `breadcrumb::fit`, as today (rule 3) |
| A game title, for example 77 characters | Game list row | Shortened in the row, whole in the title box (rule 3) |
| Decline reasons of 25 to 46 characters ("Too large for this screen", "This screen cannot hold the game's speed", "Not possible with this screen and video output") | Detail of an option | "Not available" in the row, the reason in the help box, as today |
| "Press B (bottom face button)" (28) | Prompt of the mapping | Fits |
| "Cancel" with "Set up this controller" | Dialog buttons | Fit: 196 of 200 px |
| "This game", "System", "All games" | Keep prompt of the quick menu | Fit: 202 of 206 px |

No one-line text of today needs a shorter wording in portrait: each one has a place by rule 1, 2 or 3.

### 5.3 Wrapped texts with a bad last line

At 34 characters a few wrapped texts end in a line of one short word. These are the cases where the owner's rule 4 allows a shorter text:

| Text today | Where | At 34 characters | Proposed text |
| --- | --- | --- | --- |
| "The old picture comes back in 10 s." (35) | Keep prompt of a display change (`overlays.rs`) | "...comes back in" / "10 s." | "The old picture returns in 10 s." (32), approved |
| "The old orientation comes back in 15 s." (39, the text that TATE package 8 would write) | Keep prompt of the screen orientation | "...comes back in" / "15 s." | "The screen turns back in 15 s." (30), approved; not used, the owner removed this prompt on 2026-11-25 (TATE section 9.1) |
| "Keep the new settings for this game only, for all games of this system, or for all games?" (89) | Keep prompt of the quick menu | Last line "games?" | Keep: three full lines, the last one "system, or for all games?" with the real breaks; no change |
| "The old layout comes back in 12 s." (34) | Review of the mapping | Fits exactly | No change; it is at the limit, and a 3-digit count cannot occur (the window is 15 s) |

### 5.4 The approved short texts

Decision: Owner, 2026-11-24 (D2): two short texts, used in both forms (one text, one translation later):

1. "The old picture comes back in {s} s." becomes "The old picture returns in {s} s."
2. The keep prompt of the screen orientation (TATE package 8) says "The screen turns back in {s} s." Decision: Owner, 2026-11-25: the screen orientation has no keep prompt (TATE section 9.1), so this text is not used.

The text of the notices and of the help boxes needs no change. The automatic check of section 9.4 lists any new text that ends in a one-word line, so a later text gets the same review.

## 6. The canvas contract

### 6.1 What changes

The canvas size becomes a value end to end. ADR 0021 (accepted, Owner, 2026-11-24) records it. In short:

- `neptron-types`: `Scene::ui` is "the UI canvas, upright for the player, of the size of its `Frame`". No new field: `Frame` already carries `width` and `height`, and ADR 0020 already says that `ui_dest` holds `T`. The change is the meaning: a canvas of 240x320 is valid, and the renderers must draw it.
- Both renderers already turn and place any canvas size (ADR 0020 wrote them for it). The GLES UI texture grows only; the row compare of the upload must compare canvases of the same size, and a change of size sends the whole canvas once. Tests: a 240x320 canvas with each `ui_rotation`, against the reference renderer; a change of size between two scenes sends every row.
- The engine decides the form factor (6.2), plans the menu from `T` (6.3) and places `T` (6.4).
- The app makes the canvas of the form, gives the form to the UI, and sends the frame with its real size.

### 6.2 The form factor

The engine decides it, as it decides `ui_rotation`: it knows the screen orientation and the display. The rule is pure and lives in `neptron-types` next to `Orientation`:

> The form is `Portrait` when the face, as the player sees it, is taller than it is wide; else `Landscape`. The face is the shape of the display in scan-out (a CRT or a TV: the profile's face, 4:3; a flat panel: its native mode) turned by the screen orientation `S`.

| Display | `S` | Face for the player | Form | `U` | `T` |
| --- | --- | --- | --- | --- | --- |
| CRT, TV, landscape panel | 0 | landscape | Landscape | `R0` | 320x240 |
| CRT, TV, landscape panel | 180 | landscape | Landscape | `R180` | 320x240 |
| CRT, TV, landscape panel | 90 | portrait | Portrait | `R270` | 320x240 |
| CRT, TV, landscape panel | 270 | portrait | Portrait | `R90` | 320x240 |
| Panel with a portrait native mode (1080x1920) | 0 or 180 | portrait | Portrait | `R0`, `R180` | 240x320 |
| Panel with a portrait native mode | 90 or 270 | landscape | Landscape | `R270`, `R90` | 240x320 |

So on a turned screen the menu is always portrait (decisions 1 and 2), and `T` is 320x240 on every CRT and TV in every orientation.

### 6.3 The menu mode: the claim and its check

The claim: **at `S` of 90 or 270 the portrait canvas turned by `U` is `T` = 320x240 in scan-out, which is the raster of the landscape canvas. So the menu mode of a normal screen and `ui_dest` work unchanged.**

By construction: the engine plans the menu from the raster of `T`, which is `MENU_SOURCE` (320x240, 60 Hz, 4:3, `R0`) in every orientation of a CRT, a TV and a landscape panel. The policy gets the same source as at `S = 0`, so it gives the same mode, through the same validator. The canvas turn is set apart, in `ui_rotation`. Package 3 planned the menu source turned with the screen (`Orientation::display_source`), which gave 320 lines across and an interlaced mode on a 15 kHz tube; that path goes (section 7).

The menu source is planned at `R0`, not at `R180` for `S = 180`: the policy treats `R0` and `R180` the same (own lines), so the mode is the same, and one source for all orientations keeps one decision cache entry.

The check with numbers (the rule of `ui_dest` of today, `T` = 320x240):

| Display class | Menu mode (as at `S = 0`) | `ui_dest` of `T` | What the player sees on the turned face | At `S = 0` today |
| --- | --- | --- | --- | --- |
| 15 kHz CRT (`generic_15`) | 240p at 60 Hz | the full active area (`ui_dest_fill`): 240 lines, the active width | 240 canvas columns on 240 lines, 1:1, progressive; 320 canvas rows along the line. On a 20-inch tube turned (face 12 x 16 in): glyph 0.30 x 0.60 in | the same mode, the same glyph |
| 31 kHz CRT (`arcade_31`) | 480p, 2x | full active area: 480 lines | 2 lines for each canvas column, whole; glyph 0.24 x 0.48 in on a 17-inch tube | the same |
| Multi-sync CRT from 40 kHz | 2x or more | full active area | whole factor per line | the same |
| TV NTSC | 240p, 720 samples | 640x240 at (40, 0): `across` = 704 / 320 = 2, `down` = 240 / 240 = 1 | 240 canvas columns on 240 lines; 2 samples for each canvas row; 640 of the 704 safe samples, so each canvas pixel is 9 % short along the player's vertical axis, as it is 9 % narrow at `S = 0` | 640x240 at (40, 0) |
| TV PAL | 288p | 640x240 at (40, 24) | 240 of 288 lines; 24 lines of border at the player's left and right | 640x240 at (40, 24) |
| TV in 480i or 576i | as at `S = 0` | 640x480 at (40, 0) or (40, 48) | 2 lines for each canvas column | the same |
| Flat panel 1080p, turned | 1920x1080 native | `ui_dest_uniform(1920, 1080, 320, 240)` = 4x, 1280x960 at (320, 60) | 960 wide and 1280 high for the player; glyph 24 x 48 panel pixels | 4x, 1280x960 at (320, 60) |
| Flat panel 720p, turned | 1280x720 native | 3x, 960x720 at (160, 0) | glyph 18 x 36 | 3x |
| Flat panel 4K, turned | 3840x2160 native | 9x, 2880x2160 at (480, 0) | glyph 54 x 108 | 9x |
| Panel with a portrait native mode 1080x1920, `S = 0` | native | `ui_dest_uniform(1080, 1920, 240, 320)` = 4x, 960x1280 at (60, 320) | glyph 24 x 48 | (no landscape menu exists for it today: it would be 3x, 960x720) |

The aspect: on a 4:3 tube turned, 240 lines span the 3 units of the player's width and 320 canvas rows span the 4 units of the player's height. Each canvas pixel is 1/80 x 1/80 of the face: square, as today.

So:

- **No interlaced menu.** T3 of `docs/spec/tate.md` is settled by the layout: the turned menu on a 15 kHz CRT or a TV is 240p, 1:1, with no flicker.
- **No mode change at a change of orientation** on a CRT, a TV or a panel: the menu mode is the same at every `S`. The tube does not re-sync. The change applies at once, with no keep prompt (Decision: Owner, 2026-11-25, TATE section 9.1).
- **The flat panel gains:** 4x on 1080p, against 3x for the turned landscape canvas of package 3.
- `policy_tate.toml`: the "menu" cases record today the policy's answer for the turned menu source (interlaced on `generic_15`, 264 or 240 samples on a TV). Package L4 records them again as the source of `T`, equal to `S = 0`, with a test that asserts that equality. This is a designed change of what NEPTRON sends in the menu of a turned screen, not a golden blessed to get a green run.

### 6.4 The UI over a game

The game keeps its mode. `T` (320x240) goes where the placement of that mode puts the canvas today (ADR 0005, the TV rule of P03), and `ui_filter` follows the rule of ADR 0020 for every placement: `Nearest` when each axis of `T` scales by 1 or more, `SharpBilinear` below 1.

| Case | Mode of the game | `T` in it | Result |
| --- | --- | --- | --- |
| A vertical game on a turned 15 kHz tube (DoDonPachi, `S = 90`) | 240p at 57.55 Hz, own lines | full active area, 240 lines | 1:1, nearest, sharp. T4 of the TATE spec is settled: no mode change, and nothing is soft |
| A vertical game of 224 lines (`S = 90`) | 224p | 224 lines | 0.93 on the line axis: `SharpBilinear` (D6) |
| A horizontal game on a turned 15 kHz tube, Picture mode Auto (`S = 90`) | interlaced, for example 510x448i | full active area, 448 lines | The portrait menu at full size over the small upright game (decision 3). 240 canvas columns on 448 lines, nearest: some columns get 2 lines and some 1, as the landscape canvas over a vertical game at `S = 0` today. Each field has 224 lines, so each canvas column shows in at least one field |
| The same with Picture mode Fit | 240p | 240 lines | 1:1 |
| A turned 31 kHz tube | 480p, 2x | full active area | 2 lines per canvas column |
| A TV | 240p, 288p, 480i | whole factors, centred | as the menu |
| A turned flat panel | native | the whole scale of the menu | 4x on 1080p |

The canvas stays opaque (ADR 0005): the quick menu covers the game where it lies, and the game shows around it where the canvas does not reach.

#### 6.4.1 The change on a normal screen

Decision: Owner, 2026-11-24 (D6): the filter rule applies to every placement of `T`, also on a screen at 0 or 180. This changes a picture of a normal screen. It is a designed change, not a golden to bless by accident.

- **What changes.** Today the engine sets `Filter::Nearest` for every upright canvas (`ui_place` in `crates/neptron-video/src/scene.rs`, the arm `R0 | R180`). On a CRT the canvas fills the active area of the mode (`ui_dest_fill`). In a game mode with fewer than 240 active lines (a 224-line game: for example `ui_dest` 1280x224 in the placement test of `neptron-modeline`) or fewer than 320 active samples, the scale of an axis is below 1, and nearest drops canvas lines or columns: 16 of the 240 canvas lines at 224 lines. After the change the engine sets `Filter::SharpBilinear` there. The player sees the quick menu, its keep prompt, the dialogs and the notices over such a game soft, with every line of each glyph, in place of sharp with lines missing. The ADR 0020 sentence "every upright canvas of today has both scales at 1 or more" is not true for these modes; ADR 0021 corrects it.
- **What does not change.** The menu (its mode has 240 lines or more and 320 samples or more on every display class), a TV (whole factors, at least 1 on each axis), a flat panel (a whole scale, at least 1), and every game mode with 240 lines or more and 320 samples or more. No canvas golden of `neptron-ui`, no golden of `neptron-gfx` (its scenes set the filter themselves) and no policy golden records the filter of the engine at a screen at 0, so none of them changes.
- **What the change touches in tests.** The expectations that state `Filter::Nearest` for an upright canvas over a mode below the canvas size: in `crates/neptron-video` (the unit tests of `scene.rs`, `tests/integration/tate.rs`, `tests/integration/menu_turned.rs`), and any scene hash of the fake display (`neptron-testkit`) of a frame that shows the UI over such a mode in the tier 2 tests of `apps/neptron`. Before the change, the package runs these tests and lists every expectation that the rule changes; after it, each listed expectation changes in the same commit, and no other.
- **Where.** Package L4 (TATE package 7), in one commit of its own: `feat(video): soften the UI canvas below a scale of 1 on every screen`. Its body names this section and lists each changed expectation. The review checks that list against this section. A changed expectation that the list does not name stops the package (run rule 11).

### 6.5 The screensaver

It draws in the canvas of the form, with the walls of that canvas. In the menu of a turned tube it moves at 240p, one canvas pixel per step, as on a normal screen. In a paused game the session hides the game picture as today.

### 6.6 A change of form

A change of the screen orientation between 0 or 180 and 90 or 270, or a change of display, changes the form. The app reads the form from the engine after it sets the orientation and gives it to the UI before the next frame. The router computes the new `Metrics` and lays out every screen on its stack again: each screen keeps its cursor and its first visible row as an index, and the list windows are computed again for the new row count. Overlays (a dialog, a keep prompt) lay out again too. A change of the screen orientation has no keep prompt (Decision: Owner, 2026-11-25, TATE section 9.1): the Display page shows in the new form in the next frame, with no restart. A test changes the form under each screen kind and checks the cursor and the tree.

### 6.7 The canvas in the app

- `SceneCanvas::new(Size)` and `CpuCanvas::with_size(Size)`. The app keeps one canvas per form and makes the other at the first change (a buffer of 300 KB; no allocation in the steady-state frame loop).
- `canvas_frame` takes the size from the canvas, not from `UI_WIDTH`.
- The boot path: the orientation is read before the first frame (TATE section 7.5), so the first canvas already has its form.

### 6.8 Screenshots and the tree

- `neptronctl screenshot --stage ui` returns the canvas as it is: 240x320 in portrait. The `Screenshot` reply already carries `width` and `height`. The `output` and `scanout` stages are in scan-out pixels, as today.
- The UI tree: the row bounds are in canvas pixels. The tree reply gets the canvas size as an appended field in protocol version 4 (the protocol is at version 3 and is append-only; a reply holds only the fields of the version of its request), so the crawler and a tool know the safe area of the form.

### 6.9 ADR

A contract change: yes. `Scene::ui` changes its meaning (a canvas of either size), the placement takes the size of `T`, the menu source and the destination of a turned canvas change (they were written in ADR 0020 and the TATE spec for a 320x240 canvas turned), and the tree reply gets a field. ADR 0021 (accepted, Owner, 2026-11-24) records it. It amends ADR 0005 and ADR 0020.

## 7. The interim code of TATE package 3

Package 3 (merged on `main`) turns the 320x240 canvas on a turned screen and chooses its mode. With the portrait layout:

| Item | What happens | Why |
| --- | --- | --- |
| `TurnedMenuMode` (`Interlaced`, `Progressive`), `TURNED_MENU_MODE`, `menu_plan_source` | Removed | The menu always plans the source of `T`, which is `MENU_SOURCE` (6.3). No choice is left |
| `Content::Menu(TurnedMenuMode)` | Becomes `Content::Menu` with no payload | The same |
| `ui_dest_turned` in `neptron-modeline` | Removed, with its CRT and TV branches and its tests; `policy_tate.toml` keeps the board cases and records the menu cases again (6.3) | On a CRT and a TV, `T` is always 320x240, placed by the rule of today. On a panel `T` can be 240x320, and the uniform rule of today takes any size |
| The rule of the filter (`ui_filter` of `turned.rs`) | Stays, as `placement::ui_filter(dest, t)`, for every placement | A mode of fewer lines than `T` (a 224-line game) still needs it |
| `Scene::ui_rotation`, `Scene::ui_filter` (ADR 0020), `Orientation`, `game_turn`, `ui_turn`, `display_source`, the engine's display source for games | Stay | The turn of the game and of the canvas do not change |
| The placement's `UI_CANVAS` constant | Becomes the size of `T`, a parameter | A portrait native panel has `T` of 240x320 |
| Orientation 180 | Stays: the landscape canvas turned by `R180` | Decision 1 |
| A landscape canvas on a turned panel, large | Not used: a turned screen always gets the portrait menu | Decision 2; the portrait menu is 4x on 1080p, larger than the turned landscape canvas (3x) |
| The sharp-bilinear squeeze of the menu on 240 lines | Removed | Nothing squeezes the menu |

Until the settings package wires the orientation, the orientation is always `Orientation::NORMAL`, so the interim path never runs in the product. Package L4 removes it.

## 8. Navigation and input

- The directions do not change. The canvas is upright for the player in both forms, and the controls of a cabinet are mounted for the player (TATE section 8). `nav.rs` and `neptron-input` do not change.
- Lists scroll vertically in both forms. Up and Down move by one row; Left and Right move by one page, as today. A page in portrait holds more rows.
- A settings row changes its value with Left and Right, and opens its options with Accept, as today. A stacked row is one row for the navigation.
- A dialog keeps its buttons on one row, and Left and Right move between them, as today. Every button set of today fits (section 5.2). A set that does not fit is a failure of the fit check, never a column of buttons, because a column would change the input of the dialog.
- The grid widget has fewer columns in portrait. Left and Right move to the next cell and wrap to the next row as today, so the cell under Right can be another cell than in landscape. No screen of the menu uses the grid today; the scene of the widget shows it.
- Nothing of today uses Left or Right for a wide layout (no side panes).

## 9. Goldens and tests

### 9.1 The safety net

The landscape goldens stay byte-identical through packages L1 and L2. A package that changes a landscape pixel stops and reports (rule 11). The arithmetic of `Metrics::of(Landscape, theme)` must give exactly the rectangles of today, including the rounding of the centring (`(SAFE_AREA.w - DIALOG_WIDTH_PX) / 2`). A unit test asserts the landscape metrics against the constants of today before they are removed.

### 9.2 Which scenes get a portrait golden

Every scene of the registry (78) renders in both forms and passes the fit check (9.4). A chosen set gets a portrait golden: each widget and each screen kind once, plus the cases that wrap or stack.

| Kind | Scenes |
| --- | --- |
| Widgets | `widgets/list`, `widgets/settings-list`, `widgets/dialog`, `widgets/slider`, `widgets/grid` |
| Home and games | `home`, `home-empty`, `games-middle`, `games-arcade`, `games-empty`, `games-versions` |
| Settings pages | `settings`, `display`, `display-video`, `display-tv`, `display-safe-mode`, `audio`, `input`, `network`, `network-no-address` |
| Options | `picture-mode`, `picture-mode-judging`, `refresh-rate` |
| Dialogs and notices | `confirm-10s`, `notice-picture-long`, `notice-warning`, `system-format-confirm` |
| Quick menu | `quick-menu`, `quick-menu-video-long`, `quick-menu-keep` |
| Guided mapping | `pad-mapping`, `pad-mapping-review` |
| Others | `breadcrumb-long`, `library-scanning`, `screensaver-start`, `pattern/grid` |

36 portrait goldens. Decision: Owner, 2026-11-24 (D8). Why not all 78: a golden costs a look before it is blessed and a review at each visual change; the fit check covers every scene in both forms for the faults that matter (a cut, an overlap, a text outside the safe area), and the 36 cover each widget, each screen kind and each wrap rule once.

### 9.3 Names, tool and gallery

- A scene has a name and renders in a form. The landscape names stay as they are (`screens/display`), so the paths of the 78 goldens do not change. A portrait golden is `crates/neptron-ui/tests/golden/portrait/<name>.ppm`, and its id is `portrait/<name>`.
- The registry lists for each scene the forms with a golden: `Scene { name, forms: &[Form] }`.
- `cargo xtask shot portrait/screens/display`, `cargo xtask golden [--bless] [portrait/...]`, and `cargo xtask gallery` writes two sheets: `target/ui/gallery.png` and `target/ui/gallery-portrait.png`. `golden` reports a stale portrait golden as it does a landscape one.

### 9.4 The fit check

An automatic check that no text is cut, that no text overlaps another text, and that every text stays in the safe area. It is worth more than pixels, because it covers every scene in both forms and every later text.

- **Every text draw reports its box.** `UiScene` already holds each text command with its position and its text. The box of a text is its position, its character count times 6, and 12. The check reads the commands of a frame; no widget reports anything new.
- **Visible texts.** The check keeps a list of text boxes. A later filled rectangle that covers a text box hides it (a dialog over a list): the text is removed from the list, whole or in part. So text under a dialog is not an overlap.
- **The asserts, for each scene in each form:**
  1. Each visible text box lies inside the canvas, and inside the safe area, except on the screens that use the whole canvas (the screensaver and the grid pattern).
  2. Each visible text box lies inside the clip rectangle of its command.
  3. No two visible text boxes intersect.
  4. No tree row is marked clipped. A shortening by rule 3 (`text::draw_short`) does not mark the row clipped; `draw_in` marks a cut as a fault.
  5. A warning (not a failure) for a wrapped dialog or notice whose last line is one word of 5 characters or fewer (section 5.3).
- **Where it runs:** a tier 1 test in `neptron-ui` over every registry scene in both forms; the navigation tests of `screens/tests.rs` run the check on each frame they draw; the crawler (tier 2) runs the whole menu in both forms and checks on the tree that every row is inside the safe area of the form, not clipped, and that the rows of one widget do not overlap.
- **Its own tests:** a text across the edge of the safe area, two texts that overlap by one pixel, a text under a dialog (no fault), a text under a dialog rim that covers half of it (no fault), a cut text (fault), a shortened game title (no fault).

### 9.5 The crawler in both forms

The crawler of `apps/neptron/tests/app/crawler.rs` runs once per display class today. It runs again with the screen orientation at 90 on one CRT profile and on one flat panel: every screen and every schema setting is reachable, every enabled page row opens its screen, back returns, and the checks of 9.4 hold. Tier 2 has a budget of 5 minutes; the crawl takes about 10 s per run today, so two more runs fit.

## 10. Risks and what is hard

- **The refactor must keep every landscape pixel.** 97 uses of the constants in 15 files change, and the centring arithmetic must stay exact. The 78 goldens catch any slip, but a slip found late costs a rework of a package. Mitigation: package L1 moves the constants into `Metrics` with the landscape numbers and changes no widget.
- **Rows of two lines in `List`.** The window, the page step and the scroll bar get harder. The settings list and the home list use them; the game list does not (fixed pitch for 10,000 rows).
- **Stacked settings rows halve the rows on screen.** The Display page shows 8 rows in portrait against 10 in landscape (14 in an empty box). A page with safe-mode rows scrolls. This is the cost of no cut and no squeeze (D1).
- **A form change at run time** lays out every screen of the stack again. A bug there shows a screen in the wrong form for one frame or loses a cursor. Mitigation: the test of 6.6 for each screen kind.
- **The fit check can be noisy.** Intended overlaps (a focus bar, a dialog rim, a text in a framed band) must not count. The rule of covered texts handles the known cases; a new intended overlap needs a rule, not an exemption list.
- **Texts made at run time** (formatted values, reasons with numbers, names from devices and files) are only as long as the fixtures. The scenes use the longest real values; a name without a bound uses rule 3.
- **The quick menu over an interlaced game** (a horizontal game across a turned 15 kHz tube in Auto) puts 240 canvas columns on 448 lines with nearest: uneven column widths, as the landscape canvas over a vertical game at `S = 0` today. It is readable, not perfect. Picture mode Fit gives 240p and 1:1.
- **A 224-line game** gets a soft quick menu (D6), in both forms. Today at `S = 0` nearest drops 16 canvas rows there. This changes a picture of a normal screen (section 6.4.1).
- **Package order with N01.** Package 3 is in work with the interim code. Packages L1 and L2 touch only `neptron-ui` and can run beside TATE packages 3 to 5. L4 must come after package 3.
- **Hardware.** The claim of 6.3 is exact in emulation; the look of a 240p portrait menu on a turned tube is a hardware check (`docs/hardware-checklist.md`, needs-hardware).
- **Box art and an HD UI** will put more pressure on the canvas size (memory, upload, CPU drawing time on the Raspberry Pi 3). The portrait canvas has the same pixel count as the landscape one, so this design adds no cost.

## 11. Later: box art, themes, a high-definition UI

Not designed here. What they would build on, and what they would change:

- **Box art.** It builds on the sprites of `CpuCanvas` (`Sprite`, `add_sprite`) and on `Metrics`: a new rectangle `art` in the metrics of the game list. Portrait has room for it under the list (the title box area grows), landscape at the right of the list. The fit check treats a sprite box as a box that a text must not overlap. It must add: a cache of scaled images (a cache, deletable at any time), a decoder outside `neptron-ui` (the UI does no I/O), and a rule per form for the size of the art.
- **Themes.** They build on `skin.rs` (palette, row height, padding, with limits) and on `Metrics::of(form, theme)`. A theme could set values per form. It must add: new keys in the skin format (append-only, a new `SKIN_VERSION`), and limits that hold in every form (the theme limit test runs per form already).
- **A high-definition UI.** It builds on the canvas size as a value (ADR 0021), on `Metrics` derived from sizes and character counts (so the shapes of 3.4 follow), on the form factor rule of 6.2 (a new form, for example `Hd` at 640x480 or 1280x720, chosen for a flat panel), and on the fit check and goldens per form. It must change: the font, which is a set of constants today (`GLYPH_WIDTH`, `GLYPH_HEIGHT`, `ADVANCE`), into a value of the metrics with a second, larger font; the limits of the UI texture and the cost of the CPU canvas on the Pi; and the policy of which display class gets which form (an HD UI makes no sense on a 15 kHz tube).

## 12. Work packages

Each package merges alone, and the orientation stays `NORMAL` until the TATE settings package wires it. The rule of the safety net: **the 78 landscape goldens of `neptron-ui` stay byte-identical in L1 and L2.** A package of L1 or L2 that changes one landscape pixel stops and reports; it never blesses (run rule 11). L3 adds portrait goldens and changes no landscape golden except by the approved text of D2 (the scenes `confirm-3s` and `confirm-10s`, one line of text each, looked at before the bless). L4 changes the picture of a normal screen only as section 6.4.1 says. L5 changes no golden of today.

| # | Package | Crates and paths | Size (lines touched, rough) | Gate |
| --- | --- | --- | --- | --- |
| L1 | Metrics as values. `Form`, `Size`, `Metrics::of` with the landscape numbers and a portrait value that no screen uses yet; `chrome::Layout` from `Metrics`; the router holds `Metrics`; `SAFE_AREA`, the margins, `UI_WIDTH`, `UI_HEIGHT`, `DIALOG_WIDTH_PX` replaced at every use; `CpuCanvas::with_size`, `SceneCanvas::new(Size)`; the screensaver and the pattern take the size. No visual change | `crates/neptron-ui` | 700 | T1 (`cargo xtask verify --tier 1`); the 78 landscape goldens byte-identical; a test of the landscape metrics against the old constants |
| L2 | Widgets take the metrics. `List`, `SettingsList`, `Dialog`, `Grid`, the option help box, the game title box, the video block read their counts from `Metrics`; `RowShape`; rows of 1 or 2 lines in `List`; `text::draw_short`; the unit rule of `wrap`; the fit check as a test over every scene in landscape. Still no visual change | `crates/neptron-ui` | 1,000 | T1; the 78 landscape goldens byte-identical; the fit check passes on all 78 landscape scenes |
| L3 | The portrait layout. The portrait metrics in use; stacked rows; two-line bands; the registry with forms; `xtask shot`, `golden` and `gallery` with portrait; 36 portrait goldens, each one looked at; the fit check over every scene in both forms; the approved short texts (5.4), with the landscape goldens `confirm-3s` and `confirm-10s`, which show the new text | `crates/neptron-ui`, `xtask` (the UI tools), `apps/neptron` only for the approved texts | 1,500 and 36 goldens | T1; the 36 portrait goldens looked at before the bless; the fit check on all 78 scenes in both forms; no landscape golden changes except `confirm-3s` and `confirm-10s` |
| L4 | The canvas size in the contract and the engine (ADR 0021). `Scene::ui` doc; the form factor rule in `neptron-types`; the placement takes the size of `T`; the engine plans the menu from `T` and gives the form; removal of `TurnedMenuMode`, `menu_plan_source`, `ui_dest_turned`; `policy_tate.toml` menu cases recorded again with the equality test; the GLES upload on a size change; `canvas_frame` from the canvas; the tree reply with the canvas size (protocol version 4); the filter rule on every screen in its own commit (6.4.1) | `neptron-types`, `neptron-modeline`, `neptron-video`, `neptron-gfx`, `neptron-ipc`, `apps/neptron`, `apps/neptronctl` (tree print only) | 900 | T1; tier 3 `gles-compare` with a 240x320 canvas at each `ui_rotation`; only the expectations listed under 6.4.1 change |
| L5 | The wiring, inside the TATE settings package (TATE package 8). The app gives the form to the UI at boot and at each change of orientation; the router lays out again (6.6); no keep prompt of the orientation (TATE section 9.1); the crawler in both forms; the lines of `screensaver.md` and `ui-navigation.md` | `apps/neptron`, `crates/neptron-ui`, `docs/spec` | 400 | T2 (`cargo xtask verify --tier 2`); the crawler in both forms; no golden of today changes |

The TATE spec (section 12) holds the merged order of all packages.

## 13. The mock pictures

Three portrait mocks drawn with the real canvas, font and skin of `neptron-ui` by hand-placed calls, each at 1x and enlarged 3x, and the same screens in today's landscape layout from `cargo xtask shot` (1x and 4x). They are outside the repository, in `target/orchestrator/portrait-mocks/` of the main checkout:

- `portrait-game-list.png`: the game list of a system with the long title in the box.
- `portrait-display.png`: the Display page of a turned flat panel with stacked rows, two dim rows with their reason.
- `portrait-display-compact.png`: the same page with compact rows, the alternative that the owner did not take (D1).
- `portrait-keep-dialog.png`: the keep prompt over the Display page, with the two-line hint band and the approved short text.
- `landscape-games-middle.png`, `landscape-display.png`, `landscape-confirm-10s.png`, `landscape-picture-mode.png`: today's screens.

## 14. Decisions

The owner answered on 2026-11-24: "Yes to all", to the recommendations below. The decisions of section 1 stay. Each line is **Decision: Owner, 2026-11-24**.

| # | Question | Decision |
| --- | --- | --- |
| D1 | The settings rows in portrait | Stacked: the label on line 1, then the source and the value on line 2 (`portrait-display.png`). The compact row is not used |
| D2 | The texts where a wrap looks bad | "The old picture returns in {s} s." and "The screen turns back in {s} s.", in both forms (section 5.4). The second one is not used: the owner removed the keep prompt of the orientation on 2026-11-25 |
| D3 | A long game title in the game list | Shortened with `...` in its row, and shown whole in a title box under the list (3 lines in portrait, 2 in landscape) |
| D4 | A band whose two texts do not fit one line (hint band, title band) | Two lines: the left text on line 1, the right text on line 2, right-aligned |
| D5 | The safe margins in portrait | 5 % of each axis, as today: 12 px at the sides, 16 px at the top and the bottom (34 characters) |
| D6 | The UI over a game with fewer lines or samples than the canvas | The soft placement, `SharpBilinear`, in every orientation, also on a normal screen (section 6.4.1) |
| D7 | The first setup of a turned cabinet | The menu shows sideways once, until Screen orientation is set, or `orientation` in `neptron.toml` before the first boot. A first-boot screen that asks which way is up (four texts, one upright along each edge, each naming a direction to press; no input for 10 s keeps Normal) is later work: it needs text drawn in four turns and a new screen in the boot path |
| D8 | The portrait goldens | The 36 scenes of section 9.2, and the automatic fit check on all 78 scenes in both forms |

## 15. Tests that the design asks for (summary)

- `Metrics::of` in both forms: every rectangle in the canvas and in the safe area; the landscape values equal the constants of today; the theme limits hold in both forms.
- `text::wrap`: no break between a number and each unit; the rules of 3.4 for a band and a field.
- `List`: rows of 1 and 2 lines; the window keeps the focused row whole; page steps; the scroll bar.
- The fit check (9.4) with its own tests, over every scene in both forms.
- A form change under each screen kind: the cursor and the tree.
- The form factor rule (6.2): each row of its table.
- The engine: the menu source is `MENU_SOURCE` at every `S`; the mode of the menu at `S = 90` equals the mode at `S = 0` for each display class of 6.3; `ui_dest` and `ui_filter` of 6.3 and 6.4.
- The renderers: a 240x320 canvas at each `ui_rotation`; a change of canvas size sends every row.
- The crawler in both forms (9.5).
