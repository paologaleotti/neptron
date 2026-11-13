# neptron-session

Game lifecycle: launch, the display-locked loop, pause, quick menu, exit and save RAM flush. It mediates between UI, core host and video.

`docs/spec/pacing-audio.md` defines the loop and the audio. `docs/spec/libretro-host.md` defines the core host and the order of its calls.

## Public API

- `Session<D: Display, S: AudioSink, I: InputSource, C: Clock>`: one game, from launch to exit. The same type runs the real cores and the test core, on KMS, the window or the fakes.
  - `launch(&GameLaunch, VideoSetup<D>, Devices<S, I, C>, SessionOptions) -> Result<Session, LaunchError>`.
  - `run_frame(ui: Option<Frame>) -> Result<FrameReport, SessionError>`: one frame of the loop.
  - `resume()`, `reset()`, `set_start_hold_ns(ns)`, `set_volume_percent(percent)`, `quit() -> ExitReport`.
  - Reads for the UI snapshot and the control API: `paused()`, `ended()`, `video_status(&mut VideoStatusText)`, `video()`, `last_frame()`, `source()`, `input_events()`, `ports()`, `message()`, `host()` (system info, core options), `core_logs()`, `audio_stats()`, `audio_sink()`, `volume_percent()`, `run_rate_mhz()`, `save_stats()`, `counters()`.
  - Live changes: `video_mut()` (`set_settings`, `set_scanlines`, `set_monitor`, `retry` of the engine), `host_mut()` (`set_option`).
- `GameLaunch`: the core, the content, the extensions, the BIOS and save folders, the core option values.
- `VideoSetup<D>`: the display, the monitor, the policy settings and the scanline settings. The scanline settings are part of the launch because the engine draws none until it gets them.
- `Devices<S, I, C>`, `SessionOptions` (`Resampling`, `RunRateSource`, the Start hold, the volume in percent), `RunRateSource::{ReadBack, Measured}`.
- `FrameReport`, `EndReason`, `Counters`, `ExitReport`, `Flush`, `SaveStats`, `SaveError`, `LaunchError`, `SessionError`.
- `source_geometry(width, height, &GameGeometry, &Timing) -> SourceGeometry`, and the constants `LAUNCH_STABLE_FRAMES`, `LAUNCH_SETTLE_MAX_FRAMES`, `INTERLACE_MIN_LINES`, `FLIPLESS_FRAMES_LIMIT`, `SAVE_RAM_FLUSH_INTERVAL_NS`, `CORE_PORTS`.

## One frame

```text
poll input once -> ports -> hotkeys      (Select+Start or the Start hold opens the quick menu)
set_pad for each port, run_frame          (not while the quick menu is open)
engine.begin_frame(source)
mute the audio if the engine, the menu or a refused mode asks for it
end_frame of the audio                    (only when the core ran)
present + wait_flip, once or twice        (twice for double refresh)
unmute after the flip in the new mode
run rate = read-back field rate (or measured flip rate) / flips per core frame
save RAM to the writer when the timer is due
```

- The display is the master clock. The core runs one frame per flip, or per two flips for double refresh, where the same frame is presented twice.
- The audio of a frame goes out before the flip wait, so the queue is read at the same point of every frame. A modeset frame writes silence, which covers the wait for the first flip of the new mode.
- While the quick menu is open, the session presents the last frame under the UI and does not run the core. The audio is muted and `end_frame` is not called (the pipeline primes again on resume, OP-042).
- The run rate for audio rate control is the field rate of the mode read back over the flips per core frame (`RunRateSource::ReadBack`, KMS), or the flip rate from `FlipRateMeter` (`RunRateSource::Measured`, the desktop window).
- While `current_refused()` of the engine is on, the session mutes the audio and presents no game picture.
- The user volume (`audio.volume`, `docs/spec/audio-settings.md`) is a gain in the pipeline, apart from the mute. The pipeline starts at `SessionOptions::volume_percent` with no ramp; `set_volume_percent` changes it live with a 20 ms ramp; a new pipeline for a core timing change keeps it. A sink that stops taking data (a lost device) only costs the frames it does not take: the loop never waits on it.
- No allocation and no file I/O in a steady frame. The buffers (input events, hotkeys, the last frame, the message, the save RAM copies) are made at launch. A core timing change makes a new audio pipeline, as the spec says.

## Launch: the first mode

Genesis Plus GX reports 256x192 at load and sends 320x224 from frame 4 (`tests/traces`, OP-055). The engine sets its first mode with no debounce, so starting from the load geometry gives a wrong mode and a second modeset four frames later (`launch::without_the_settle_the_engine_sets_two_modes`).

The session runs the core with no display until the source has been the same for `LAUNCH_STABLE_FRAMES` (6) frames, at most `LAUNCH_SETTLE_MAX_FRAMES` (30). The audio of these frames is dropped and the core reads no input. The first `run_frame` then sets the game's mode, once (`launch::the_first_mode_is_the_mode_of_the_settled_source`). The cost is at most 100 ms more before the first picture, 10 frames for Genesis Plus GX. The data has no default geometry per system, so the session cannot start from one.

## The source

The size comes from the last frame, because some cores send a new size without telling the frontend. The rate and the aspect come from the core's AV info and geometry events. A frame of more than 288 lines (`INTERLACE_MIN_LINES`) is interlaced: the cores do not say it, and 288 is the tallest progressive console picture. Rotation is `R0` (not in the MVP).

## The quick menu

- Select+Start (and the Start hold, `input.menu_hold_start_seconds`) opens it. The hotkey pauses the core. A second quick menu hotkey while it is open does nothing: the menu is never a toggle (OP-051). The app closes it with `resume()`.
- On `resume()`, every button held at that moment reaches the core only after its release. The button that chose "resume" does not reach the game, and after a Start hold the game sees Start go up and no new press (OP-051).
- `FrameReport::blind_recovery` reports the 5 s hold of the combination, also while the menu is open. The safety layer acts on it.
- `resume()` calls the engine's `retry()` when the engine has a source blocked after a device error of its `set_mode` (`VideoEngine::source_blocked`), whatever the last switch record.
- Opening the menu flushes the save RAM.

## Display errors

The session follows what the engine reports. A present or wait error ends the frame's flips and is in `FrameReport::display_error`; the engine keeps a timed-out flip queued and drains it at the next present. Each frame makes at most one bounded wait per flip, so a failing display never hangs or spins the loop. After `FLIPLESS_FRAMES_LIMIT` (10) frames in a row with no completed flip, `run_frame` returns `SessionError::DisplayStalled` and the session ends. When each wait times out, the 500 ms bounded wait of KMS makes this at most about 5 s. When the display fails at once (an immediate error of `present` or of the wait, or `NoMode`), nothing paces the loop: the session ends after 10 unpaced frames, in which the core runs with no display.

When the inputs refuse the mode on the display and no mode is valid, the engine turns the output off (`Display::blank`). The core then waits with the game frozen, the audio is muted, and `FrameReport::blanked` is set. These frames are not a stall, and no flip paces them: `run_frame` returns at once, and the caller paces the loop until the engine sets a mode again (an input change from the safety layer, or `retry`).

## Save RAM

- Launch reads `<save_dir>/<content stem>.srm` into the core. A save file that exists but cannot be read stops the launch, so the session never writes over a file it did not load.
- A save file longer than the core's save RAM: the host loads only its first bytes, as RetroArch does. The rest is user data the core does not see, so every write keeps it after the save RAM (`SaveStats::kept_tail_bytes`). The file never gets shorter, and no write rotates the full file out of the `.bak`. A file shorter than the save RAM grows to the save RAM size, which loses nothing.
- A writer thread writes the file with `write_save_file` of the host: temp file, fsync, rename, one `.bak`. The session owns it; `quit()` and the drop stop and join it.
- Two buffers travel between the frame thread and the writer over two bounded one-direction channels. The frame thread copies the save RAM into a free buffer, compares it with the last copy and sends it only when it changed. With no free buffer the flush waits for the next one. The frame thread never blocks.
- The flush runs every `SAVE_RAM_FLUSH_INTERVAL_NS` (30 s) while the game runs, when the quick menu opens, and at exit. A failed write makes the next flush write again.
- At exit, `quit()` waits for the writes in flight, sends the save RAM when it changed, joins the writer, and reports the result of that last write in `ExitReport::save`: `Written`, `WriteFailed` (the reason is in `SaveStats::last_error`), `Unchanged`, `Failed` (the copy failed) or `NoSaveRam`.

## What the app (T27) must do

1. Resolve the settings: `policy_settings` and `scanline_settings` of `neptron-video`, `input.menu_hold_start_seconds` times 10^9, the core option values. Pick `RunRateSource::ReadBack` for KMS and `Measured` for the window.
2. `Session::launch`. On `LaunchError`, go back to the game list with a notice.
3. Each frame: build the UI snapshot from `paused()`, `video_status`, `message()`, `host().options()`, then let the UI update with the navigation of `input_events()`, draw the canvas, and call `run_frame(Some(canvas))` (or `None` when nothing is drawn).
4. `FrameReport::menu_opened`: show the quick menu. The UI intent "resume" calls `resume()`, "reset" calls `reset()`, "quit" calls `quit()`. A second quick menu hotkey while it is open is ignored by the session.
5. `FrameReport::blind_recovery`, `FrameReport::blanked`, `video().current_refused()`: pass to the safety layer (T26). While `blanked`, pace the loop with the clock.
6. `FrameReport::ended`, `SessionError`: call `quit()` and re-execute for the game list.
7. Make the save folder writable. The session makes it when it is missing.

## Must never depend on

- `neptron-ui` (only the app binary depends on it)
- `neptron-kms`, `neptron-window`

## Rules

- `unsafe` is forbidden: the crate root and the test root carry `#![forbid(unsafe_code)]`. The tests set the test core's script variable on a child process of the test binary, because setting it in the running process is `unsafe`.
- No features.

## Tests

- Unit tests: the source and the launch settle, the button mask, the frame copy, the save RAM writer (backup, unchanged, many flushes, failed write, failed copy), the message cut.
- `tests/integration/session.rs`, the scripted session with the test core through the real host: one frame per flip, the input of each frame, the quick menu (pause, no toggle, UI over the frozen frame, the mask on resume), the Start hold, the blind recovery under the menu, save RAM at launch, on the timer, at the menu and at exit, a core shutdown, double refresh, scanline settings and video status, core options, a timing change.
- `tests/integration/launch.rs`: the first mode for the Genesis Plus GX geometry, and the two modesets without the settle.
- `tests/integration/faults.rs`: a slow flip, a failed present and wait, a display that stops flipping, the output turned off and on again.
- `tests/integration/timing.rs`: timing check 6 at 50.00, 59.94 and 60.10 Hz with jitter, with late flips, and with a display rate that differs from the mode (measured run rate). 30 simulated seconds each in the tier. Two 100,000-frame runs (about 28 simulated minutes, about 67 s each in the dev profile) are ignored tests: with jitter only, and with one late flip in a thousand.

```sh
cargo xtask test -p neptron-session --run-ignored only -E 'test(hundred_thousand)'
```

A late flip takes one frame of audio (16.7 ms) from the 32 ms queue, and the law gives it back with a time constant of 6.4 s. Two late flips a few seconds apart run the queue dry. The long run with late flips asserts that every underrun follows two late flips in the 10 s before it, so the session adds no underrun of its own. A larger target level would trade latency for this margin (see the open points in the task report).

## Status

Complete for the MVP on the fakes. No real core and no hardware has run it.
