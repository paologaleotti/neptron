# neptron-settings

Typed settings schema, resolution over the levels game, system, global and default, and TOML persistence.

## Must never depend on

- Any other NEPTRON crate
- Any hardware crate

## Rules

- `unsafe` is forbidden: the crate root carries `#![forbid(unsafe_code)]`.
- No features.
- Only `storage.rs` does file I/O. Schema, resolution, parsing and editing are pure.

## Public API sketch

- `Schema`: `Schema::from_entries(entries)` checks a list of entries. `Schema::builtin()` holds every NEPTRON setting. `Schema::entries()` lists them in menu order for the UI crawler. Each `Entry` has a key, a label, a `Kind` (type, allowed values, default), the `Levels` where it may be set, a `live` flag, a `Safety` class, `applies_to` and `expert`.
- `Layer`: the valid values of one file. `Layer::set` checks key, level and value.
- `Layers::try_new(global, system, game)` checks that each layer has the level of its slot.
- `resolve(schema, layers)` and `resolve_key(schema, layers, key)`: pure. Each `Resolved` has the value and its `Source` (game, system, global or default).
- `parse_layer(schema, level, text) -> Parsed`: pure, never fails. It returns the valid values and a list of `Issue`s with line and key.
- `edit(schema, level, text, changes) -> Result<String, EditError>`: pure. It sets or unsets keys and keeps comments, order and unknown keys.
- `Storage::new(config_dir)`, `Storage::load(schema, target)`, `Storage::apply(schema, target, changes)`, `Storage::copy_game_file(system, from, to)`: the only file I/O.

## Built-in keys

| Key | Type | Levels | Default | Safety | Applies to | Live |
| --- | --- | --- | --- | --- | --- | --- |
| `display.output` | name (`auto` or a connector, such as `VGA-1`) | global | `auto` | confirm or revert | every | yes |
| `display.monitor` | name (`auto` or a profile id, such as `generic_15`) | global | `auto` | confirm or revert | not TV | yes |
| `display.style` (Picture mode) | `auto`, `native`, `2x`, `3x`, `4x`, `sharp`, `speed`, `fit` | game, system, global | `auto` | validated | every | yes |
| `display.refresh_policy` | `exact`, `exact_x2`, `force_50`, `force_60` | game, system, global | `exact` | validated | CRT, not TV | yes |
| `input.menu_hold_start_seconds` | whole seconds, 0 to 10 (`MENU_HOLD_START_MAX_SECONDS`), 0 is off | global | `0` | none | every | yes |
| `display.scanlines` | `auto`, `off`, `on` | game, system, global | `auto` | none | every | yes |
| `display.scanline_strength` | integer, 0 to 100 (percent) | game, system, global | `75` | none | every | yes |
| `audio.volume` | integer, 0 to 100 (percent; 100 is the core's own level and, with the mixer at 0 dB, the full output) | game, system, global | `80` | none | every | yes |
| `audio.output` | text (`auto` or a playback device as the audio backend lists it, on ALSA a PCM name such as `hw:CARD=PCH,DEV=0`), 1 to 128 bytes | global | `auto` | none | every | no |
| `display.tv` | `auto` (NTSC), `ntsc`, `ntsc_j`, `ntsc_443`, `pal_60`, `pal_m`, `pal`, `pal_n`, `secam`, `mono_60`, `mono_50` | global | `auto` | confirm or revert | TV | yes |
| `display.tv_width` | `aspect`, `integer`, `fill` | game, system, global | `aspect` | validated | TV | yes |
| `display.screensaver` | bool | global | `true` | none | every | yes |
| `display.screensaver_minutes` | `1`, `5`, `10`, `30` (minutes) | global | `5` | none | every | yes |
| `display.vertical` (expert) | `exact`, `fit` | game, system, global | `exact` | validated | every | yes |
| `display.panel_scale` (expert) | `auto`, `integer` | game, system, global | `auto` | validated | flat panel | yes |

The help texts of the menus are in the doc comments of the key constants: the schema has no help text field.

### Applies to

`applies_to` (`AppliesTo::Every`, `Crt`, `Tv`, `NotTv`, `CrtNotTv` or `FlatPanel`) names the displays on which a setting has an effect, for a `DisplayClass` of `Crt`, `FlatPanel` or `Tv` (a TV on a composite encoder). The settings screen, the quick menu and the UI crawler read this one field, so a row with no effect on the display shows as disabled with its reason; `AppliesTo::shown` hides the TV rows on every other display. It is not part of the file format. The reasons:

- `display.refresh_policy`: the flat-panel path scales the game into a mode that the panel lists, and the TV standard fixes the rate.
- `display.monitor`: a TV takes its monitor profile from `display.tv`.
- The scanline settings: `on` draws scanlines on a panel at a whole vertical multiple.

`expert`: the menus never show the row of an expert key (`Entry::shown`), and the crawler does not look for it. Only a settings file sets it, and its value keeps its effect. The picture modes `fit` and `sharp` cover `display.vertical` and `display.panel_scale`.

### Notes on the keys

- **Picture mode and refresh rate** are two settings. The "Native, double refresh" picture mode is `style = native` with `refresh_policy = exact_x2`. The menus judge each option of one row with the stored value of the other row; a pick that the display gives only with the other row at Auto sets both (`docs/spec/video-policy-cases.md`, section 8.7).
- **`input.menu_hold_start_seconds`**: hold Start alone on a controller for this time to open the quick menu, for an arcade panel with no Select button. Select and Start held together for 0.5 s still open the quick menu, and their 5 s hold is still the blind recovery (`neptron-input` gives the exact rule). The app converts the value to nanoseconds for `Hotkeys::set_start_hold_ns`. It is global only: a controller belongs to the machine. The built-in default stays 0, because nothing detects a panel; the first-boot setup or the user writes 2 s for an arcade panel.
- **Scanlines**: `auto` draws black scanlines at a whole vertical multiple of at least 2 on a CRT that is not in a 15 kHz mode, and none on a 15 kHz CRT (its scanlines are real) or a flat panel. `on` draws them at any whole multiple of at least 2, `off` never. No mode draws them on an interlaced mode. The strength is how much a dark line loses: 100 is black, 0 is no effect. The default is 75: black lines on every second line halve the light, and older tubes are dim. MiSTer's VGA scandoubler offers 25, 50 and 75 and no 100, and users on PC CRTs mostly prefer 75. The UI can offer presets (off 0, light 25, medium 50, strong 75, black 100); the stored value stays a whole percent. The safety class is none, because the video signal does not change. `neptron_types::scanlines_for` turns the resolved values into the scanlines of a scene, and `ScanlineMode` in `neptron-types` uses the same choice names. A new scanline kind is a new choice at the end of `SCANLINE_MODES`.
- **`display.tv`** is the broadcast standard of a composite output (`docs/spec/raspberry-pi.md`). Its choices are the keys of `TvStandard` in `neptron-types`, in the same order. It is global only, as `display.monitor` is: the standard is the monitor of a TV, and the guard keeps the monitor of the boot for every game. **`display.tv_width`** sets the width of the game on a TV: `aspect` the console's own shape, `integer` a whole factor of the game width, `fill` the full 4:3 width. It changes only the placement, so every level may set it.
- **Screensaver** (`docs/spec/screensaver.md`): the minutes are a choice, so a file holds them as a string (`screensaver_minutes = "10"`). The safety class is none, because the screensaver changes no mode.
- **Audio** (`docs/spec/audio-settings.md`): `audio.volume` applies with a short ramp in the audio pipeline. `audio.output` applies at the next start of the frontend; an absent named device falls back to the default device with a notice, and the file keeps the name.
- **`Kind::Text`** holds a name that the `name` kind cannot: 1 to `max_len` bytes of UTF-8 with no control character, such as an ALSA device name with spaces and a colon. In a file it is a TOML string.

The schema checks only the form of a name. The consumer checks that the connector or the profile exists, and every mode still passes the safety validator.

## Files

The files are under `config/` on the state partition: `global.toml`, `systems/<system>.toml`, `games/<system>/<game>.toml`. A game key may hold folders (`hacks/Chrono Trigger` gives `games/<system>/hacks/Chrono Trigger.toml`); each part is one plain file name. A symbolic link on the way to a file is refused (`StorageError::Link`), so no read or write leaves `config/`. `Storage::copy_game_file` copies the file of one game key to another that has none, with the atomic write; the app uses it for a game in a folder that used the flat file before. The key `display.style` is the key `style` in the table `[display]`. Dotted keys and inline tables are read the same way.

- **Unknown key**: reported with its line, ignored, and kept in the file. A newer build may have written it.
- **Wrong type, out of range, or a level the key does not allow**: reported with its line. The value is left out, so the level below applies. The file keeps it.
- **File that does not parse, is not UTF-8, or cannot be read**: reported once and read as empty. `Storage::apply` refuses to write it, so the user's file is never lost.
- **Version that is not an integer**: reported at load. `Storage::apply` refuses to write the file.
- **Write**: `Storage::apply` reads the file again, so a hand edit made since the last load stays. It edits the document in place and writes it atomically:
  1. Make the missing directories. Sync each new directory into its parent.
  2. Remove a stale `<name>.tmp`. For a symlink, this removes the link, not its target.
  3. Create `<name>.tmp` with `create_new`, so an entry of any kind there makes the write fail. Nothing is written through a link.
  4. Give it the permission bits of the old file, or `NEW_FILE_MODE` (`0o644`, before the umask) for a new file.
  5. Write it, sync it, rename it over the file, sync the directory.

  `File::sync_all` is `fsync` on Linux and `fcntl(F_FULLFSYNC)` on macOS. A crash leaves the old file or the new file. When the settings file is a symlink, the read follows it, and the rename replaces the link with a regular file. The target of the link is never written.
- **No value left**: an unset also removes each table that it leaves empty, unless the table or its key has a comment. A comment above a key goes with the key. `Storage::apply` removes the file (and syncs the directory) only when all of these hold: the old file held no `#` at all, the new text holds only `schema_version` and empty tables, and that version is not newer than `SCHEMA_VERSION`. Else the file is written and stays. When the file is not there, it writes no file and makes no directory.

## Format versions

The format is append-only. A key or a choice is never renamed or removed. `tests/fixtures/keys.txt` lists every key with its levels and choices, and a test compares it with the schema. To add a key, append its line. To add a choice, append it to the end of its line. A menu can show the keys in any order.

Each write records `SCHEMA_VERSION` in the top-level key `schema_version`. A file with no version is version 1. A write raises an older version and keeps a newer one.

The version is the version of the file layout: how a key maps to TOML tables, and which TOML type each kind uses. An added key or choice does not change it: an older build reports a new key as unknown, keeps it, and uses the level below. The version changes only for a layout change that an older build would misread, which the append-only rule forbids for the existing keys. So the version stays 1.

## First-boot file

`neptron.toml` on the boot partition uses the same keys as `global.toml`. To import it, the caller parses it with `parse_layer(schema, Level::Global, text)` and passes its values as `Change::Set` to `Storage::apply` for `Target::Global`. Keys outside the schema, such as the Wi-Fi keys, are not copied. The caller decides when the file has changed since the last boot.

## How to test

`cargo xtask test -p neptron-settings`. The pure modules have unit tests with no mocks; the storage tests use a temporary directory.
