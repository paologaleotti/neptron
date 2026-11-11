## Frontend and UX

The UI is drawn on a 320x240 logical canvas with pixel fonts, so it is sharp on a 15 kHz tube and integer-scales on everything else. It emits a scene, a short list of draw commands. The GLES renderer draws it on hardware, and a CPU reference renderer draws the same scene in tests and in the desktop window.

**The UI is a sealed box.** It is the part that will change most, so `neptron-ui` has one narrow boundary and can be rewritten or reskinned without touching any other crate.

| Direction | What crosses the boundary |
| --- | --- |
| In | Navigation actions (up, down, accept, back, menu), a time step, and a read-only snapshot of what to show: systems, games, menu items with their values, notices |
| Out | A scene (rectangles, sprites, text runs and clips; never pixels), a list of intents (launch this game, set this value at this level, change this profile), and the widget tree for the control API |

- The UI never acts. It does not load cores, write settings, touch the display or read files other than its skin. It returns intents and the app carries them out.
- It depends on no other NEPTRON crate, and only the app binary depends on it. It has no threads and no I/O in its update path, so every screen is a pure function of snapshot plus input.
- The snapshot and intent types are defined inside `neptron-ui`. The app converts the settings schema and the library into them, so a change to settings or library internals cannot break a screen.

Inside the crate, four layers, each using only the one below:

1. **Scene**: the drawing vocabulary, held to four primitives (rectangle, sprite, text run, clip) so the GLES renderer and the CPU reference stay easy to keep identical. The renderers live in neptron-gfx, outside this crate.
2. **Widgets and templates**: list, settings list, dialog, grid, slider.
3. **Screens and navigation**: which template shows which snapshot data, and the screen stack.
4. **Skin**: colours, fonts, images and template parameters loaded from `skin.toml`.

**Screens**

| Screen | Purpose |
| --- | --- |
| First-boot wizard | Pick output and monitor profile (confirm-or-revert), centre the picture, map the controller, optional network |
| Home | Systems that actually contain games, plus Favourites, Recents, Settings |
| Game list | Per system: sort, filter, search, favourites; arcade filters by orientation and players |
| Game page | Box art, launch, resume last state, per-game settings, save-state slots |
| Display panel | One entry from Home: output, monitor profile, refresh policy, geometry, test patterns |
| Settings | Full tree: display, audio, input, systems and cores, network, storage, updates, diagnostics |
| Quick menu | In-game overlay on a button combination |

**Settings model**

- Resolution order is game, then system, then global, then built-in default. Each row shows where its current value comes from and offers "reset to inherited".
- Every setting is declared once in a typed schema: key, type, range, label, allowed scopes, whether it applies live. Menus are generated from the schema. A setting cannot exist without a UI entry, which is how "all settings via UI" stays true as the project grows.
- Storage is plain TOML: `global.toml`, `systems/<system>.toml`, `games/<system>/<game>.toml`. Files are written atomically and remain hand-editable, but never need to be.

**Quick menu ("everything" menu)**

- Default combination Select + Start, configurable. On arcade panels without a Select button the default is holding Start for 2 seconds.
- The core is paused and the overlay is drawn over the frozen frame in the current video mode. Opening it never causes a mode switch.
- Contents: resume, save and load state, video (refresh policy, interlace, position, scaling, shader, rotation), core options, controls (remap, turbo), disc swap, reset, quit.
- Changes apply immediately. On leaving, one prompt asks where to keep them: this game, this system, or everywhere.

**UX rules**

- Everything works with a D-pad and two buttons. Keyboard and mouse are optional.
- Content stays inside a 5% overscan-safe area. No text under 8 pixels tall on the logical canvas.
- The UI responds within one frame. List scrolling never blocks on disk.
- The game library is a local index rebuilt in the background. `gamelist.xml` files from EmulationStation setups are imported so RetroPie, Batocera and Recalbox users keep their names and favourites. *(added)*

**Skinning**

A skin is a folder: `skin.toml` (layout, colours, fonts, sounds) plus image assets. Screens are built from a handful of fixed templates (list, settings list, dialog, grid, slider). A skin changes colours, fonts, images and template parameters, not behaviour, and contains no scripts. There is no general layout engine. The first milestone ships one built-in skin and a stable skin file format version.
