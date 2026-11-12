# neptron

The frontend binary. It holds only wiring, no logic: it joins the library, the settings, the UI, the session, the ODREN engine and its guard, the input and the control protocol on one frame loop.

## Run it on a desktop

The window backend needs SDL3 (`brew install sdl3` on macOS, `libsdl3-dev` on Debian). From the root of the checkout, with a folder of ROMs and a state folder of your own:

```sh
mkdir -p /tmp/neptron/share/roms/nes /tmp/neptron/state /tmp/neptron/cores /tmp/neptron/run
cargo run -p neptron --features window -- --backend window \
  --share /tmp/neptron/share --state /tmp/neptron/state --data data \
  --cores /tmp/neptron/cores --run /tmp/neptron/run --no-control
```

- Put games in `share/roms/nes`, `share/roms/snes` or `share/roms/megadrive`, and the cores (`fceumm_libretro.dylib` named `fceumm_libretro.so`, and so on, as `data/cores` names them) in the cores folder.
- Keys: arrows, X accepts (RetroPad A), Z goes back (B), S is the menu button (X), Right Shift and Return together open the quick menu, and held for 5 s they are the blind recovery.
- A launch, and the end of a game, end the process with exit code 75: one process holds one core. Run it under the launcher to get the next run at once: `cargo run -p neptron-launch -- --no-device -- target/debug/neptron --backend window ...`.

`cargo xtask clippy -p neptron --features window` checks the window build. The `kms` build compiles only on Linux.

## A run

One run is one process. It shows the game list, or it runs one game, because `neptron-libretro` allows one core per process and the display descriptor closes with the display. A run ends with an outcome:

| Outcome | Exit code | What the launcher does |
| --- | --- | --- |
| Relaunch: a launch from the list, the end of a game, a game that did not start | 75 (`RELAUNCH_EXIT_CODE`) | Starts the next run at once |
| Quit: the window was closed | 0 | Stops |
| The run cannot start (`BootError`) | 1 | Starts it again after its backoff |

Before a relaunch the run writes `run/next.toml` (the relaunch file, `relaunch.rs`): the game to launch, or none for the list, and a notice for the player. The next run takes and removes it. The run folder is a RAM file system (`/run/neptron` on the image), so a power cut leaves no file.

**A relaunch is not a new boot.** `Guard::boot` counts one failed boot for each start. A run that starts from a relaunch file gives back the count of the run before (`record::for_continued_boot`), so quick game changes never reach safe mode. A crash, a power cut or a restart of the launcher leaves no relaunch file, and that start counts.

## Boot, before the first frame

1. The settings: `global.toml`, and for a game its system and game files. A file that cannot be read is an empty layer and a notice.
2. The monitor presets from `data/monitors` at run time (OP-044): the binary embeds no GPL data. A bad file is left out with a notice.
3. The detected output from sysfs: the connected connectors, the EDID profile, the GPU caps of the capability table.
4. The safe default: the EDID profile when a monitor gives one, else `generic_15`, with `auto` and `exact` (`SAFE_START_PROFILE`). Without both the run cannot start.
5. `wanted`: the monitor of `display.monitor` (`auto` is the safe default, an unknown id is the safe default with a notice) and the global picture style and refresh policy.
6. The guard record from `state/video/guard.toml`. A record that cannot be read is the default record. The file also holds `output`, the connector its last known good was confirmed on (an optional field, written at each confirmation). When the output of this start differs, or the file names none, the guard gets no last known good: it opens the boot prompt and reverts to the safe default for the new output, so a profile confirmed on one connector never drives another display without a prompt.
7. The library cache. In the list, a scan starts on its own thread and replaces the index when it ends.
8. The list: a `VideoEngine` for the menu source (320x240, 60 Hz, 4:3, `MENU_SOURCE`). A game: `Session::launch` with the resolved settings, the core options of `data/cores` (OP-055), `RunRateSource::ReadBack` on KMS and `Measured` in the window. A launch that fails ends the run with a relaunch to the list and the reason as a notice.
9. `Guard::boot`, and the record it returns is written at once (temp file, fsync, rename) before the first modeset. In a game, `follow_settings` with the game's policy follows.

## One frame

Each frame reads the clock once and gives that time to every guard call, the hotkeys and the key repeat of the frame.

| Game list | Game |
| --- | --- |
| Poll the input once, ports, hotkeys. Blind recovery calls `Guard::recover` | The session polled the input in its last frame |
| Navigation from the events, with key repeat | Navigation from `Session::input_events`, only while the UI shows; else only the menu button after `menu_opened` |
| Snapshot: confirm, display mode | Snapshot: confirm, session, `video_status` (the read back, never a proposal), display mode |
| `Screens::update`, then the intents | The same |
| `begin_frame(MENU_SOURCE)`, `present(None, canvas)`, `wait_flip` | `run_frame(canvas or None)` |
| `Guard::tick`, also after a failed present | `Guard::tick` |
| No flip: the loop waits for itself one 60 Hz frame | `blanked`: the same |

- Key repeat (`nav.rs`): a direction repeats after 400 ms, then every 66 ms. On `PromptOpened` every repeat stops and every held key is ignored until its release (OP-048). A, B and X are accept, back and menu; Start and Select are left to the hotkeys.
- The quick menu is never a toggle: the session ignores a second hotkey, and the app only opens it (OP-051). Resume, reset and quit go to the session.
- A notice at the first picture of a game that is filtered, cropped or small, in the words of the video status.
- `FrameReport::ended` or any `SessionError` (`DisplayStalled` included): the game ends with `quit()`, and the run relaunches to the list, which opens the display again in a new process. This is the one rule for a display that stops.

## Settings and the guard

| Setting | What a change does |
| --- | --- |
| Safety none: scanlines, scanline strength, the Start hold | Written at once (writer thread) and applied live |
| Picture style, refresh policy, monitor | Changed in memory and given to `Guard::propose`. Written only on `Confirmed`. Taken back on `Reverted` and `SafeMode` |
| Output (`display.output`) | Shown, not changed from the menu or the protocol; a new connector at the next start opens the boot prompt |

- `PromptOpened` puts the confirm dialog in the snapshot with the guard's deadline. `KeepMode` calls `confirm` with the open prompt; a confirmation after the deadline is dropped. `RevertMode` calls `decline`.
- `Reverted { rewrite: Some(config) }`: the monitor, picture style and refresh policy of that configuration go to the global settings, never `Guard::active()`.
- `SafeMode(Recovery)` plays a short tone in the game list. The display panel shows "(safe mode)" after the mode while the guard is in safe mode.
- The display panel writes the global level; the quick menu writes the game level, and its keep prompt moves a change to the system or global level.
- Every write (settings, the guard record, the library cache) goes to the writer thread (`outbox.rs`) over a bounded channel. A full channel keeps the job in a backlog on the frame thread, in order; nothing is dropped. The frame thread does no file I/O.

## Control protocol

`control.rs`: the socket thread accepts one connection at a time (read and write timeouts of 2 s, socket file mode 0600) and sends each command to the frame thread over a bounded channel. The frame thread answers between two frames. `wait` stays on the frame thread until its condition holds or its time ends; a `timeout_ms` above 60 s (`WAIT_TIMEOUT_MAX_MS`) is cut to it. At the end of a run `ControlServer::stop` answers every open wait and queued command with `unavailable`, closes the frame thread's side, and stops the socket thread, which ends a connection between two request lines, so a waiting or busy client never delays the end of a run.

| Command | Served |
| --- | --- |
| `hello`, `ui_tree`, `session_state`, `video_state`, `stats`, `settings_get`, `log_tail` | Always |
| `input_send`, `input_script`, `session_launch`, `settings_set` | With the `control` feature. A display setting goes through the guard, as a menu choice does |
| `wait` | Always, on the frame thread |
| `clock_step` | `unavailable`: the binary runs in real time. The tier 2 tests step the simulated clock themselves |
| `screenshot` | `unavailable`: no PNG encoder in the frontend yet |

## Tests

- Unit tests of each module: the record file, the relaunch file, the presets, the settings view, the pages, the navigation, the writer thread, the library numbers, the command line.
- `tests/app/control.rs` (tier 1): the control socket with a real client.
- Tier 2, ignored in tier 1 and run by `cargo xtask verify --tier 2`, on the fakes of `neptron-testkit` with the built test core:
  - `end_to_end.rs`: a new boot confirms its settings, the game list, a launch, the mode switch of the core, the quick menu (not a toggle), a display change that reverts with no answer, the same change kept and written to the game's file, the quit with the keep prompt, the save RAM at the quit, and the list again with one boot counted.
  - `recovery.rs`: the 5 s hold of Select and Start from `neptron-input` to `Guard::recover`, in the list and in a game, with the record in safe mode and the tone.
  - `faults.rs`: a display that stops flipping ends the game and relaunches the list with the reason.
  - `crawler.rs`: the UI crawler. It walks every menu path from home and from the quick menu with the D-pad, accept and back only, through the UI tree of the control protocol. It fails on a dead end, on a clipped settings row, on a row outside the overscan-safe area, and on a schema setting that no screen shows.

## Must never depend on

- Logic of its own: logic belongs in the crates

## Rules

- `unsafe` is forbidden: the crate root carries `#![forbid(unsafe_code)]`.
- Features: `kms` (Linux: KMS, SDL3 gamepads and audio, evdev keyboards), `window` (SDL3 window and audio), `control` (the commands that change the frontend). No feature is on by default.

## Open points

- The output connector is not an input of the guard, and a new connector needs the display opened again, so the display panel shows it read-only and `settings_set display.output` is refused. It changes in `global.toml` or `neptron.toml` and takes effect at the next start, where the guard record's output check below opens the boot prompt.
- The audio cue of the blind recovery plays only in the game list. In a game the session owns the sink.
- The picture style options do not show the line rate or grey out a style the monitor cannot show (`style_rows`). A style with no picture reverts after 1 s.
- `screenshot` is not served: the frontend has no PNG encoder.
- Favourites and recent games are not kept; their lists are empty.
- The KMS build: the buffers are at most 2560x1200, the connector rule treats the monitor as a CRT, and the GPU caps come from the first connected output. It is not compiled on macOS and has not run.
- The menu source is 60 Hz also on a 50 Hz profile; the policy picks the mode.
- `video_state` gives no validator verdicts yet.

## Status

Complete for the MVP on the fakes and the window. The KMS build has not run.
