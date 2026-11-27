# neptron-session

Game lifecycle: launch, the display-locked loop, pause, quick menu, exit and save RAM flush. It mediates between UI, core host and video.

`docs/spec/pacing-audio.md` defines the loop and the audio. `docs/spec/libretro-host.md` defines the core host and the order of its calls.

## Must never depend on

- `neptron-ui` (only the app binary depends on it)
- `neptron-kms`, `neptron-window`

## Public API

- `Session<D: Display, S: AudioSink, I: InputSource, C: Clock>`: one game, from launch to exit. The same type runs the real cores and the test core, on KMS, the window or the fakes.
  - `launch(&GameLaunch, VideoSetup<D>, Devices<S, I, C>, SessionOptions) -> Result<Session, LaunchError>`.
  - `run_frame(ui: Option<Frame>) -> Result<FrameReport, SessionError>`: one frame of the loop.
  - `resume()`, `reset()`, `hide_picture(hide)` (while paused, frames show the UI with no game picture, for the screensaver), `set_start_hold_ns(ns)`, `set_volume_percent(percent)`, `write_cue(samples)` (a tone mixed into the next core frames), `quit() -> ExitReport`.
  - `slow_speed_percent()`: the speed of a game that the machine cannot keep at full speed, in percent of the mode's run rate, after 2 s of late pacing below 98% (`SLOW_NOTICE_PERMILLE`), else `None`. `sound_too_slow()`: the game runs below half speed and its sound is off.
  - Start failures: `start_errors()`, `start_dropped_logs()`, `last_message()`, `last_error()`.
  - Reads for the UI snapshot and the control API: `paused()`, `ended()`, `video_status(&mut VideoStatusText)`, `video()`, `last_frame()` (a frame in memory), `read_picture()` (the last picture, read back from the image of a GL core on a request), `picture_scale()`, `hw_request()`, `source()`, `input_events()`, `ports()`, `input()`, `message()`, `host()`, `core_logs()`, `audio_stats()`, `audio_sink()`, `volume_percent()`, `run_rate_mhz()`, `save_stats()`, `counters()`.
  - Discs (`docs/spec/disc-games.md`, section 9): `discs()` (`DiscState`: the count, the disc in the drive, the tray; `None` for a core with no disc control interface), `change_disc(index)` (opens the tray, sets the disc, and closes the tray after `DISC_TRAY_OPEN_FRAMES` (60) core frames of play), `disc_issues()` (an added disc that the core refused, a tray that did not close: lines for the log).
  - Live changes: `video_mut()` (`set_settings`, `set_monitor`, `retry` of the engine), `set_scanlines(ScanlineSettings)` (the engine's scanlines, and the internal resolution option of a GL core that they decide), `host_mut()` (`set_option`).
- `GameLaunch`: the core, the content, the extensions, the BIOS and save folders, the core option values, `always_progressive`, and `internal_scale` (`InternalScale`: the option of a GL core that sets its internal resolution, from the core data). For a disc game: `more_discs` (the discs after the first of a group of files, added with the disc control interface after the load; a core with no interface plays the first disc), `save_name` (the save file name in place of the content stem, so every disc of a game shares one save and its memory card), and `core_saves_in_game_folder` (the core's save directory is the save folder of the game). Launch gives the option values to `Host::hold_options` before the load, so a core that declares its options in `retro_load_game` (FinalBurn Neo) gets them too (ADR 0016). `Counters::refused_options` counts the held values that did not apply, and `host().held_option_report()` names them.
- `VideoSetup<D>`: the display, the monitor, the policy settings and the scanline settings. The scanline settings are part of the launch because the engine draws none until it gets them.
- `Devices<S, I, C>`, `SessionOptions` (`Resampling`, `RunRateSource`, the Start hold, the volume in percent, the `Machine`), `RunRateSource::{ReadBack, Measured}`.
- `FrameReport`, `PacingChange`, `EndReason`, `Counters`, `ExitReport`, `Flush`, `SaveStats`, `SaveError`, `LaunchError`, `SessionError`, `StartFacts`, `end_reason`.
- `zip_entries`, `ZipDirectoryEntry`, `ZipError`: the zip listing of the core host, re-exported for the arcade set check of the app (`docs/spec/arcade.md`, section 10).
- `source_geometry(width, height, &GameGeometry, &Timing, FrameRules) -> SourceGeometry` with `FrameRules { rotation, always_progressive }` (`FrameRules::CONSOLE` for no turn).
- `internal_scale(&MonitorKind, Machine, ScanlineMode, native_lines, values) -> u8` and `scale_value`: the rule of the internal resolution `k` of a GL core (`docs/spec/hw-render.md`, section 4).
- Constants: `INTERNAL_SCALE_LIMIT`, `CONTEXT_RECOVERIES_MAX`, `LAUNCH_STABLE_FRAMES`, `LAUNCH_SETTLE_MAX_FRAMES`, `INTERLACE_MIN_LINES`, `FLIPLESS_FRAMES_LIMIT`, `SAVE_RAM_FLUSH_INTERVAL_NS`, `CORE_PORTS`, `START_ERRORS_MAX`, `START_WINDOW_NS`, `LATE_PRESS_MIN_FRAMES`, `LATE_PRESS_MAX_FRAMES`, and the pacing and speed constants of `docs/spec/pacing-audio.md`.

## Rules

- `unsafe` is forbidden: the crate root and the test root carry `#![forbid(unsafe_code)]`. The tests set the test core's script variable on a child process of the test binary, because setting it in the running process is `unsafe`.
- No features.
- No allocation and no file I/O in a steady frame. The buffers (input events, hotkeys, the last frame, the message, the error line, the save RAM copies) are made at launch. A core timing change makes a new audio pipeline.

## One frame

```text
poll input once -> ports -> hotkeys      (Select+Start held 0.5 s or the Start hold opens the quick menu)
set_pad for each port, run_frame          (not while the quick menu is open)
engine.begin_frame(source)
mute the audio if the engine, the menu or a refused mode asks for it
present                                   (the GPU starts to draw)
end_frame of the audio                    (only when the core ran; else idle_frame)
wait_flip, then present + wait_flip again for double refresh
unmute after the flip in the new mode
run rate = read-back field rate (or measured flip rate) / flips per core frame
save RAM to the writer when the timer is due
```

- The display is the master clock. The core runs one frame per flip, or per two flips for double refresh, where the same frame is presented twice.
- Late pacing, the measured speed of a slow core, the sound mute below half speed, `Counters::missed_fields`, `Counters::input_to_flip_ns`, `Counters::flips` and the drained flips are in `docs/spec/pacing-audio.md`, "A core slower than the field". `pacing.rs` and `speed.rs` decide; both are pure.
- The audio of a frame goes out after the first present and before the first flip wait, so the resampler runs while the GPU draws. The mute comes before the present (`docs/spec/pacing-audio.md`, "Frame order").
- While the quick menu is open, the session presents the last frame under the UI and does not run the core. The audio is muted and `end_frame` is not called (the pipeline primes again on resume, OP-042).
- While `current_refused()` of the engine is on, the session mutes the audio and presents no game picture.
- The run rate is the field rate of the mode read back over the flips per core frame (`RunRateSource::ReadBack`, KMS), or the flip rate from `FlipRateMeter` (`RunRateSource::Measured`, the desktop window).
- The user volume (`audio.volume`, `docs/spec/audio-settings.md`) is a gain in the pipeline, apart from the mute. The pipeline starts at `SessionOptions::volume_percent` with no ramp; `set_volume_percent` changes it live with a 20 ms ramp; a new pipeline for a core timing change keeps it. A sink that stops taking data (a lost device) only costs the frames it does not take: the loop never waits on it.

## A GL core

`docs/spec/hw-render.md` defines it. `session/hw.rs` holds the code.

- Launch: the offer of `VideoSetup::core_offer` (the app keeps it per boot), or else of `Display::core_render`, goes to `Host::set_hw_offer` before the load, with the option value of `k` held with the core options. After a load whose core asked for a context: `CoreRender::create` with an image of the core's largest frame, `Host::set_hw_target`, then `context_reset` between `begin` and `end`, then the settle frames. A refused type gives `LaunchError::Refused` with `hw_refusal`; a failed `create` gives `LaunchError::Graphics`, and the core unloads.
- Each call into the core that can draw (`retro_run`, `retro_reset`, `context_reset`) runs between `CoreRender::begin` and `CoreRender::end`, before the present of its frame. A larger maximum of the core resizes the image before the next call. While the quick menu is open the core does not run, and the image keeps the last picture under the UI.
- `k`: 1 on a CRT and a TV, on a Raspberry Pi, and with the scanlines on; on a flat panel of a PC the largest `k` of the core data with `k * native_lines` at most the panel lines, at most 4. The policy sees the native size, the frame over `k`. A frame is taken at `k` only when the core's largest frame has at least `k` times the native lines and both sides are multiples of `k`; else it counts as native and `Counters::unscaled_frames` counts it. A live change of the scanlines, and a change of the monitor or of its class in a game (a guard revert, the blind recovery; one compare of the monitor id per frame), set the option again; the session also takes frames at the `k` of the launch, for a core that takes the option only at its next launch.
- A lost context, reported by `end` or found by `begin` (`CoreRenderError::NoContext` after the context was made): the session makes it again once (`CONTEXT_RECOVERIES_MAX`) and calls `context_reset` with no `context_destroy`; the next loss ends the game with `SessionError::ContextLost`. A failed `begin`, `end` or `resize` ends it with `SessionError::Graphics`.
- No picture: a GL core that sends no hardware frame in the core frames of `START_WINDOW_NS` (10 s) ends the game.
- End: `context_destroy` runs inside the context, and the context stays current for the unload and the deinit of the host, which drops first; the display destroys the context after.

## Launch: the first mode

Genesis Plus GX reports 256x192 at load and sends 320x224 from frame 4 (`tests/traces`, OP-055). The engine sets its first mode with no debounce, so the load geometry would give a wrong mode and a second modeset.

So the session runs the core with no display until the source has been the same for `LAUNCH_STABLE_FRAMES` (6) frames, at most `LAUNCH_SETTLE_MAX_FRAMES` (30). The audio of these frames is dropped and the core reads no input. The first `run_frame` then sets the game's mode, once. The cost is at most 100 ms more before the first picture.

## A game that does not start

- The session keeps the error-level log lines of the core from the load and the settle frames, at most `START_ERRORS_MAX` (8): `start_errors()`. It reads them from `Host::error_logs`, which keeps them also when a noisy call fills the log queue; `start_dropped_logs()` counts the records of the launch that the queue dropped. Only error lines of the core count.
- A `retro_load_game` that returns false gives `LaunchError::Refused`, and a `SHUTDOWN` during the load or the settle gives `LaunchError::CoreStopped`. Both carry those lines, and no mode was set.
- A `SHUTDOWN` in the loop ends the session with the reason of `end_reason` (`start.rs`, pure): `EndReason::FailedStart` or `EndReason::CoreShutdown`, by the rule of `docs/spec/arcade.md` section 10.1 (`START_WINDOW_NS`, 10 s). A picture that a refused mode hides, or the output off, counts as started.
- `last_message()` gives the last `SET_MESSAGE` text and `last_error()` the last error line of the core in the frames.

## The source

- The size comes from the last frame, because some cores send a new size without telling the frontend. The rate and the aspect come from the core's AV info and geometry events.
- A frame of more than 288 lines (`INTERLACE_MIN_LINES`) is interlaced, unless the system data sets `always_progressive` (`docs/spec/arcade.md` section 9).
- The rotation is the turn of `SET_ROTATION` (`Host::rotation`), read for each frame, so a turn is a new source that the engine debounces. For `R90` and `R270` the session inverts an aspect that the core reported (`round(1_000_000 / aspect)`). The host's fallback aspect (`GameGeometry::aspect_from_core` false) is already the shape of the frame as drawn and stays. `R180` keeps the aspect (ADR 0015, `docs/spec/arcade.md` section 11).

## The quick menu

- Select+Start held together for 0.5 s (`neptron_input::QUICK_MENU_HOLD_NS`), and the Start hold (`input.menu_hold_start_seconds`), open it. The hotkey pauses the core. A second hotkey while it is open does nothing: the menu is never a toggle (OP-051). The app closes it with `resume()`.
- On `resume()`, every button held at that moment reaches the core only after its release (OP-051).
- The combination mask (`PadMask`) holds back the button that completes Select and Start, and gives it to the core late when no menu opens (`docs/spec/arcade.md`, "13.1 The quick menu hold").
- `FrameReport::blind_recovery` reports the 5 s hold of the combination, also while the menu is open. The safety layer acts on it.
- `resume()` calls the engine's `retry()` when the engine has a source blocked after a device error of its `set_mode` (`VideoEngine::source_blocked`).
- Opening the menu flushes the save RAM.

## Display errors

- A present or wait error ends the frame's flips and is in `FrameReport::display_error`. The engine keeps a timed-out flip queued and drains it at the next present. Each frame makes at most one bounded wait per flip, so a failing display never hangs or spins the loop.
- After `FLIPLESS_FRAMES_LIMIT` (10) frames in a row with no completed flip, `run_frame` returns `SessionError::DisplayStalled`. With the 500 ms bounded wait of KMS this takes at most about 5 s. When the display fails at once, nothing paces the loop: the session ends after 10 unpaced frames.
- When no mode is valid, the engine turns the output off (`Display::blank`). The game is frozen, the audio muted, and `FrameReport::blanked` set. These frames are not a stall: `run_frame` returns at once, and the caller paces the loop until the engine sets a mode again.

## Save RAM

- Launch reads `<save_dir>/<save_folders>/<content stem>.srm` (`GameLaunch::save_folders`), and makes the folders with no link followed. With `GameLaunch::adopt_flat_save`, a game in a folder with no save of its own gets a copy of the flat save `<save_dir>/<content stem>.srm` first, once (`<save>.srm.adopted` records it); the flat save stays. The core's save directory stays `<save_dir>`. A save file that exists but cannot be read stops the launch, so the session never writes over a file it did not load.
- A save file longer than the core's save RAM: every write keeps the rest after the save RAM (`SaveStats::kept_tail_bytes`), so the file never gets shorter. A shorter file grows to the save RAM size.
- A writer thread writes the file with `write_save_file` of the host: temp file, fsync, rename. The session owns it; `quit()` and the drop stop and join it.
- One `.bak` per game session: before its first write, the writer copies the save file to `<name>.bak` (`back_up_save_file`). Later writes of the session leave it, so a bad save written in play never reaches the backup within the session. A failed copy writes nothing, and the next flush tries again.
- Two buffers travel between the frame thread and the writer over two bounded one-direction channels. The frame thread sends a copy only when it changed. With no free buffer the flush waits for the next one. The frame thread never blocks.
- The flush runs every `SAVE_RAM_FLUSH_INTERVAL_NS` (30 s) while the game runs, when the quick menu opens, and at exit. A failed write makes the next flush write again.
- The writer starts only for a core with save RAM at launch. When such a core later gives zero bytes (at a reset or a disk change), the flush writes nothing and returns `NoSaveRam` (`SaveStats::no_save_ram`): an empty file never replaces a save. At exit, `NoSaveRam` after a failed write is `WriteFailed`.
- At exit, `quit()` waits for the writes in flight, sends the save RAM when it changed, joins the writer, and reports the last write in `ExitReport::save`: `Written`, `WriteFailed` (reason in `SaveStats::last_error`), `Unchanged`, `Failed` (the copy failed) or `NoSaveRam`.

## What the app must do

1. Resolve the settings: `policy_settings` and `scanline_settings` of `neptron-video`, `input.menu_hold_start_seconds` times 10^9, the core option values. Pick `RunRateSource::ReadBack` for KMS and `Measured` for the window.
2. `Session::launch`. On `LaunchError`, go back to the game list with a notice. For `Refused` and `CoreStopped`, the notice quotes the core.
3. Each frame: build the UI snapshot from `paused()`, `video_status`, `message()`, `host().options()`, then let the UI update with the navigation of `input_events()`, draw the canvas, and call `run_frame(Some(canvas))` (or `None` when nothing is drawn).
4. `FrameReport::menu_opened`: show the quick menu. The UI intent `Resume` calls `resume()`, and `QuitGame` calls `quit()`. The UI has no reset intent, so the app does not call `reset()`. A second quick menu hotkey while it is open is ignored by the session.
5. `FrameReport::blind_recovery`, `FrameReport::blanked`, `video().current_refused()`: pass to the safety layer (the `Guard` of `neptron-video`). While `blanked`, pace the loop with the clock.
6. `FrameReport::ended`, `SessionError`: call `quit()` and re-execute for the game list. For `EndReason::FailedStart`, the game list shows a notice with `start_errors()` or `last_message()`; for `CoreShutdown`, a notice with `last_message()` or `last_error()`.
7. Make the save folder writable. The session makes it when it is missing.

## Tests

- Unit tests: the source and the launch settle, the button mask, the frame copy, the save RAM writer, the message cut.
- `tests/integration/`: `session.rs` (the scripted session with the test core through the real host), `launch.rs` (the first mode, and the two modesets without the settle), `start.rs` (every start failure), `arcade.rs` (held options, rotation, `always_progressive`, no save file without save RAM, the zip listing), `pacing.rs` and `pacing_edges.rs`, `speed.rs` and `slow_notice.rs`, `volume.rs`, `faults.rs`, `timing.rs` (timing check 6 at 50.00, 59.94 and 60.10 Hz, 30 simulated seconds each).
- Two 100,000-frame runs (about 28 simulated minutes, about 67 s each in the dev profile) are ignored tests: with jitter only, and with one late flip in a thousand.

```sh
cargo xtask test -p neptron-session --run-ignored only -E 'test(hundred_thousand)'
```

A late flip takes one frame of audio (16.7 ms) from the 32 ms queue, and the law gives it back with a time constant of 6.4 s. So two late flips a few seconds apart run the queue dry. The long run with late flips asserts that every underrun follows two late flips in the 10 s before it, so the session adds no underrun of its own.

## Status

It runs the real cores on the i3-6100T and on a Pi 3 B. On composite NTSC on the Pi, NES, SNES and Mega Drive games hold 60.054 frames per second with no missed field (OP-103).
