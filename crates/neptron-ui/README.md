# neptron-ui

The sealed UI: snapshot in, scene and intents out. A 320x240 XRGB8888 canvas, a bitmap font, five widget templates, the scene registry and the screens.

## Public API sketch

- `Snapshot` (module `snapshot`): what the UI shows. All systems; a `GameWindow` of one game list; a `SettingsPage` of one page (`MenuItem` with a typed `SettingValue` and its source `Level`); monitor profiles, the display, a pending confirm-or-revert, the session, notices. The app fills it. The snapshot and its entry structs are `#[non_exhaustive]`: start from `Snapshot::default()` and the `new` constructors.
- `UiAction`: up, down, left, right, accept, back, menu.
- `Intent` and `Value`: what the app must do (load a game window or a settings page, launch, set or reset a value at a level, run an action, select a profile, keep or revert a mode, resume, quit). `Intent` is exhaustive on purpose: a new intent breaks the build of the app until the app handles it.
- `UiFrame`: the output of one frame. `scene: UiScene` (draw commands), `tree: UiTree` (rows for `ui tree` in the control API), `intents: Vec<Intent>`. Make one at launch and reuse it; `clear()` keeps the memory.
- `UiScene`: the four primitives. `rect`, `sprite`, `text_run` (or `text_fmt`), `clip`. `render(&mut impl Canvas)` replays them.
- `trait Canvas` and `CpuCanvas`: `CpuCanvas::as_bytes()` is XRGB8888, little-endian, pitch 1280, ready for the `ui` frame of a `Scene` in `neptron-types`.
- `font`: Spleen 6x12, printable ASCII and Latin-1, 6x12 glyph box, 6 px advance, capitals 8 px tall. Other characters draw `?`.
- `text`: `fit`, `draw_in` (cut with `...`, aligned, reports clipping), `wrap`, `StackText` (formatting with no allocation), `value_text`.
- `theme`: `Theme` and `SAFE_AREA` (5% overscan margin: 16 px left and right, 12 px top and bottom). `Theme::new(palette, row_height_px, padding_px)` refuses sizes outside `ROW_HEIGHT_MIN_PX..=ROW_HEIGHT_MAX_PX` (12 to 24) and above `PADDING_MAX_PX` (16) with a `ThemeError`, so a bad skin fails once at load. Inside these limits no layout sum can overflow, and the templates use saturating maths on every count and rectangle that comes from the caller. A test with a fixed seed draws all five templates with random themes, rectangles, counts and values.
- `widgets`: `List`, `SettingsList`, `Dialog`, `Grid`, `Slider`. Each holds only its cursor or focus. `handle(action, ...)` returns an event (`SettingsList` returns ready intents); `draw(frame, theme, data)` adds commands and tree rows.
- `registry`: `SceneDef { name, build }`, `scenes()`, `find()`, `render()`. The template scenes are `widgets/*` and `font/ascii`.
- `golden`: the golden image format and a bit-exact comparison with a readable difference.
- `screens`: the screens and the screen stack. `screens::SCENES` adds their scenes to the registry.

### Data for each view

The UI chooses the view; the app owns the data. A screen asks for the data of its view with an intent, and the app answers in a later snapshot:

- `Intent::LoadGames { source, first, count }` asks for rows of a game list. `GameSource` is `System(id)`, `Favourites` or `Recents`. The app puts a window of rows around them in `Snapshot::games`: a `GameWindow` with its `source`, the index `first` of its first row, the `total` of the list, and the rows. A list of 10,000 games costs a window of rows, not 10,000 copies, and the snapshot is passed by reference, never copied per frame.
- `Intent::LoadSettings { page }` asks for the rows of a settings page (`PageId`, defined by the app: the display panel, the quick menu, a settings page). The app puts them in `Snapshot::settings`, a `SettingsPage` with its `page`.
- Systems, profiles, display, confirm and session are small and always complete.

`GameWindow::is_for`, `covers` and `SettingsPage::is_for` let a screen tell old data from current data. Until the answer comes, the screen shows the list as empty and asks again in each frame; the app ignores a request that the snapshot already answers. This keeps the UI free of I/O and the app free of screen logic. The other option, a snapshot with every list complete, copies or holds the whole library for each frame, so it was not taken.

### Writing a screen

A screen keeps its widget states, turns each `UiAction` into widget events and intents, asks for its data, and draws from the snapshot:

```rust
use neptron_ui::snapshot::GameSource;
use neptron_ui::widgets::{List, ListEvent, ListItem};
use neptron_ui::{Intent, Rect, Snapshot, Theme, UiAction, UiFrame};

const WINDOW_ROWS: usize = 64;

struct GameList { source: GameSource, list: List }

impl GameList {
    fn total(&self, snap: &Snapshot) -> usize {
        if snap.games.is_for(self.source) { snap.games.total } else { 0 }
    }

    fn update(&mut self, snap: &Snapshot, action: UiAction, out: &mut UiFrame) {
        if let ListEvent::Activated(i) = self.list.handle(action, self.total(snap)) {
            if let Some(game) = snap.games.get(i) {
                out.intents.push(Intent::Launch { game: game.id });
            }
        }
        let (top, rows) = (self.list.top(), self.list.visible_rows());
        if !snap.games.covers(self.source, top, rows) {
            let first = top.saturating_sub(WINDOW_ROWS / 2);
            out.intents.push(Intent::LoadGames { source: self.source, first, count: WINDOW_ROWS });
        }
    }

    fn draw(&self, snap: &Snapshot, theme: &Theme, out: &mut UiFrame) {
        out.tree.set_screen("games");
        out.scene.rect(Rect::new(0, 0, 320, 240), theme.colors.background);
        self.list.draw(out, theme, self.total(snap), |i| {
            ListItem::new(snap.games.get(i).map_or("", |g| g.title.as_str()))
        });
    }
}
```

Every scene in the registry needs a golden image in `crates/neptron-ui/tests/golden/<name>.ppm`. `cargo xtask golden --bless <name>` writes it; look at it with `cargo xtask shot <name>` first.

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
- Dependency: `thiserror` only.
