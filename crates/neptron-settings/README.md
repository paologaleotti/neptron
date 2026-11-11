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

Picture style and refresh policy are independent settings. The "Native, double refresh" row of the picture style table is the pair `style = native` with `refresh_policy = exact_x2`. The UI can show the pair as one row.

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
