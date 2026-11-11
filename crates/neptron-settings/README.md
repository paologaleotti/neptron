# neptron-settings

Typed settings schema, three-level resolution (game, system, global, default) and TOML persistence.

## Public API sketch

- `Schema`: the typed schema. `Schema::from_entries(entries)` checks a list of entries. `Schema::builtin()` holds every NEPTRON setting. `Schema::entries()` lists them in menu order for the UI crawler. Each `Entry` has a key, a label, a `Kind` (type, allowed values, default), the `Levels` where it may be set, a `live` flag and a `Safety` class.
- `Layer`: the valid values of one file. `Layer::set` checks key, level and value.
- `Layers::try_new(global, system, game)` checks that each layer has the level of its slot.
- `resolve(schema, layers)` and `resolve_key(schema, layers, key)`: pure. Each `Resolved` has the value and its `Source` (game, system, global or default).
- `parse_layer(schema, level, text) -> Parsed`: pure, never fails. It returns the valid values and a list of `Issue`s with line and key.
- `edit(schema, level, text, changes) -> Result<String, EditError>`: pure. It sets or unsets keys and keeps comments, order and unknown keys.
- `Storage::new(config_dir)`, `Storage::load(schema, target)`, `Storage::apply(schema, target, changes)`: the only file I/O.

## Built-in keys

| Key | Type | Levels | Default | Safety |
| --- | --- | --- | --- | --- |
| `display.output` | name (`auto` or a connector, such as `VGA-1`) | global | `auto` | confirm or revert |
| `display.monitor` | name (`auto` or a profile id, such as `generic_15`) | global | `auto` | confirm or revert |
| `display.style` | `auto`, `native`, `2x`, `3x`, `4x` | game, system, global | `auto` | validated |
| `display.refresh_policy` | `exact`, `exact_x2`, `force_50`, `force_60` | game, system, global | `exact` | validated |
| `input.menu_hold_start_seconds` | whole number of seconds, 0 to 10 (`MENU_HOLD_START_MAX_SECONDS`), 0 is off | global | `0` | none |
| `display.scanlines` | `auto`, `off`, `on` | game, system, global | `auto` | none |
| `display.scanline_strength` | integer, 0 to 100 (percent) | game, system, global | `75` | none |

Picture style and refresh policy are independent settings. The "Native, double refresh" row of the picture style table is the pair `style = native` with `refresh_policy = exact_x2`. The UI can show the pair as one row.

`input.menu_hold_start_seconds`: hold Start alone on a controller for this number of seconds to open the quick menu. 0 turns it off. Select and Start together still open the quick menu at once, and their 5 s hold is still the blind recovery. It is for an arcade panel with no Select button. `neptron-input` describes the exact rule. The schema has no help text field, so the menu help text is in the doc comment of `INPUT_MENU_HOLD_START_SECONDS`.

- The app reads the resolved value, converts it to nanoseconds, and passes it to `Hotkeys::set_start_hold_ns` of `neptron-input`. The value applies live.
- The setting is global only. A controller belongs to the machine, not to a game or a system.
- The built-in default stays 0. The 2 s of an arcade panel is a choice that the first-boot setup writes to `global.toml` (or that the user writes in `neptron.toml`). It is not a hidden default: nothing detects a panel.

The two scanline settings sit beside the picture style. `auto` draws black scanlines where the tube cannot show real ones: a whole vertical multiple of at least 2 on a CRT that is not in a 15 kHz mode. It draws none on a 15 kHz CRT, whose scanlines are real, and none on a flat panel. `on` draws them at any whole multiple of at least 2, `off` never. No mode draws them on an interlaced mode. The strength is how much a dark line loses: 100 is black, 0 is no effect. The default is 75. MiSTer's scandoubler effect for VGA monitors offers 25, 50 and 75 % and no 100, and users on PC CRTs mostly prefer 75. Black lines on every second line halve the light, and older tubes are dim. OSSC and RetroTINK ship with scanlines off. The UI can offer the strength as presets on top of the value: off (0), light (25), medium (50), strong (75) and black (100), as the "off, light, medium, strong" of the Definition asks. The stored value stays a whole percent, so a user can also set any value from 0 to 100 in a file. These settings do not change the video signal, so their safety class is none. `neptron_types::scanlines_for` turns the resolved values into the scanlines of a scene; this crate does not depend on it, and `ScanlineMode` in `neptron-types` uses the same choice names. Later scanline kinds are new choices at the end of `SCANLINE_MODES`. The keys come after `input.menu_hold_start_seconds` in the schema because the key list is append-only; a menu can show them in any order.

The schema checks only the form of a name. The consumer checks that the connector or the profile exists, and every mode still passes the safety validator.

## Files

The files are under `config/` on the state partition: `global.toml`, `systems/<system>.toml`, `games/<system>/<game>.toml`. A key `display.style` is the key `style` in the table `[display]`. Dotted keys and inline tables are read the same way.

- **Unknown key**: reported with its line, ignored, and kept in the file. A newer build may have written it.
- **Wrong type, out of range, or a level the key does not allow**: reported with its line. The value is left out, so the level below applies. The file keeps it.
- **File that does not parse, is not UTF-8, or cannot be read**: reported once and read as empty. `Storage::apply` refuses to write it, so the user's file is never lost.
- **Version that is not an integer**: reported at load. `Storage::apply` refuses to write the file, so the content under `schema_version` is never lost.
- **Write**: `Storage::apply` reads the file again, so a hand edit made since the last load stays. It edits the document in place and writes it atomically:
  1. Make the missing directories. Sync each new directory into its parent.
  2. Remove a stale `<name>.tmp`. For a symlink, this removes the link, not its target.
  3. Create `<name>.tmp` with `create_new`, so an entry of any kind there makes the write fail. Nothing is written through a link.
  4. Give it the permission bits of the old file, or `NEW_FILE_MODE` (`0o644`, before the umask) for a new file.
  5. Write it, sync it, rename it over the file, sync the directory.

  `File::sync_all` is `fsync` on Linux and `fcntl(F_FULLFSYNC)` on macOS. A crash leaves the old file or the new file. When the settings file is a symlink, the read follows it, and the rename replaces the link with a regular file. The target of the link is never written.

## Format versions

The format is append-only. A key or a choice is never renamed or removed. `tests/fixtures/keys.txt` lists every key with its levels and choices, and a test compares it with the schema. To add a key, append its line. To add a choice, append it to the end of its line.

Each write records `SCHEMA_VERSION` in the top-level key `schema_version`. A file with no version is version 1. A write raises an older version and keeps a newer one.

The version is the version of the file layout: how a key maps to TOML tables, and which TOML type each kind uses. It never changes for an added key or an added choice. An older build reports a new key as unknown, keeps it, and uses the level below. The version changes only for a layout change that an older build would misread. The append-only rule forbids such a change for the existing keys, so the version stays 1 while the layout stays as it is.

## First-boot file

`neptron.toml` on the boot partition uses the same keys as `global.toml`. To import it, the caller parses it with `parse_layer(schema, Level::Global, text)` and passes its values as `Change::Set` to `Storage::apply` for `Target::Global`. Keys outside the schema, such as the Wi-Fi keys, are not copied. The caller decides when the file has changed since the last boot.

## Must never depend on

- Any other NEPTRON crate
- Any hardware crate

## Rules

- `unsafe` is forbidden: the crate root carries `#![forbid(unsafe_code)]`.
- No features.
- Only `storage.rs` does file I/O. Schema, resolution, parsing and editing are pure.
