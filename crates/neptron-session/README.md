# neptron-session

Game lifecycle: launch, the display-locked loop, pause, quick menu, exit and save RAM flush. It mediates between UI, core host and video.

`docs/spec/pacing-audio.md` defines the loop and the audio. `docs/spec/libretro-host.md` defines the core host and the order of its calls.

## Public API

- `Session<D: Display, S: AudioSink, I: InputSource, C: Clock>`: one game, from launch to exit. The same type runs the real cores and the test core, on KMS, the window or the fakes.
  - `launch(&GameLaunch, VideoSetup<D>, Devices<S, I, C>, SessionOptions) -> Result<Session, LaunchError>`.
  - `run_frame(ui: Option<Frame>) -> Result<FrameReport, SessionError>`: one frame of the loop.
  - `resume()`, `reset()`, `hide_picture(hide)` (while paused, frames show the UI with no game picture, for the screensaver), `set_start_hold_ns(ns)`, `set_volume_percent(percent)`, `write_cue(samples)` (a tone mixed into the next core frames), `quit() -> ExitReport`.
  - `slow_speed_percent()`: the speed of a game that the machine cannot keep at full speed, in percent of the mode's run rate, after 2 s of late pacing below 98% (`SLOW_NOTICE_PERMILLE`), else `None`. `sound_too_slow()`: the game runs below half speed and its sound is off. The app shows a notice from them.
  - Reads for the UI snapshot and the control API: `paused()`, `ended()`, `video_status(&mut VideoStatusText)`, `video()`, `last_frame()`, `source()`, `input_events()`, `ports()`, `input()` (the source, for the port of each pad), `message()`, `host()` (system info, core options), `core_logs()`, `audio_stats()`, `audio_sink()`, `volume_percent()`, `run_rate_mhz()`, `save_stats()`, `counters()`.
  - Live changes: `video_mut()` (`set_settings`, `set_scanlines`, `set_monitor`, `retry` of the engine), `host_mut()` (`set_option`).
- `GameLaunch`: the core, the content, the extensions, the BIOS and save folders, the core option values. Launch gives the values to `Host::hold_options` before the load, so a core that declares its options in `retro_load_game` (FinalBurn Neo) gets them too (ADR 0016). After the load, `Counters::refused_options` counts the held values that did not apply, and `host().held_option_report()` names them.
- `VideoSetup<D>`: the display, the monitor, the policy settings and the scanline settings. The scanline settings are part of the launch because the engine draws none until it gets them.
- `Devices<S, I, C>`, `SessionOptions` (`Resampling`, `RunRateSource`, the Start hold, the volume in percent), `RunRateSource::{ReadBack, Measured}`.
- `FrameReport`, `EndReason`, `Counters`, `ExitReport`, `Flush`, `SaveStats`, `SaveError`, `LaunchError`, `SessionError`.
- `zip_entries`, `ZipDirectoryEntry`, `ZipError`: the zip listing of the core host, re-exported for the arcade set check of the app, which reaches the host only through the session (`docs/spec/arcade.md`, section 10).
- `source_geometry(width, height, &GameGeometry, &Timing, FrameRules) -> SourceGeometry` with `FrameRules { rotation, always_progressive }` (`FrameRules::CONSOLE` for no turn), and `GameLaunch::always_progressive`, and the constants `LAUNCH_STABLE_FRAMES`, `LAUNCH_SETTLE_MAX_FRAMES`, `INTERLACE_MIN_LINES`, `FLIPLESS_FRAMES_LIMIT`, `SAVE_RAM_FLUSH_INTERVAL_NS`, `CORE_PORTS`.

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
- Late pacing (`docs/spec/pacing-audio.md`, "A core slower than the field"). A frame in late pacing does not wait for its last flip. The next frame reads the input and runs the core at once, then waits for that flip (`VideoEngine::wait_flip`), then presents. So a core that is 4% too slow loses 4% of the frames, not half. `pacing.rs` decides; it is pure.
  - The loop enters late pacing at a core frame whose work (from the flip it follows to the end of its first present) is at least one flip interval, and after `LATE_FLIPS_TO_ENTER` (2) flips in a row that land after their blank although the work fit: the GPU was not done in time (`Counters::late_flips`). It learns the work of that frame. A driver that does not advance the vertical blank count gets its gaps from the flip times.
  - It goes back to wait pacing after a hold of frames whose work fits, whose flip lands on its blank and, when it learned a late work, whose work is `TRY_MARGIN_PERMILLE` (3%) below it. Each stay doubles the next hold, from `LATE_HOLD_FRAMES` (30) up to `LATE_HOLD_MAX_FRAMES` (1920); each `HOLD_DECAY_FRAMES` (600) of wait pacing with no late frame halve it. So long frames that come again and again lead to a steady late pacing, and a rare one costs 0.5 s of it.
  - It goes back to wait pacing at once, with the shortest hold and nothing learned, for a frame with no core, a modeset, a display error, a drained flip or the end of the session.
  - `FrameReport::deferred`, `FrameReport::pacing` (a change) and `Counters::late_frames` show it. A frame whose flip is queued is not flipless. A flip that the engine drains after a failed wait counts in `FrameReport::flips` and `drained_flips`, and is no progress for the stall check.
- The audio of a frame goes out after the first present and before the first flip wait. The present only queues the GPU's work, so the resampler (1.3 to 1.4 ms on the Raspberry Pi 3) runs while the GPU draws, off the path from the flip event to the GPU submit. The queue is still read at the same point of every frame. The audio runs after a failed present too. The mute comes before the present and leaves the queue at the target (about 32 ms), which covers a modeset of up to about that length; the frame's audio, written after the present, covers the field until the first flip of the new mode. A longer modeset empties the muted queue and counts an underrun (`docs/spec/pacing-audio.md`, "Frame order").
- While the quick menu is open, the session presents the last frame under the UI and does not run the core. The audio is muted and `end_frame` is not called (the pipeline primes again on resume, OP-042).
- The run rate for audio rate control is the field rate of the mode read back over the flips per core frame (`RunRateSource::ReadBack`, KMS), or the flip rate from `FlipRateMeter` (`RunRateSource::Measured`, the desktop window).
- In late pacing the run rate is the lower of that rate and the measured rate of a slow core (`speed.rs`, pure): the mean cycle of the last `SPEED_WINDOW_FRAMES` (120) late frames. The core is slow when the window holds `SPEED_MIN_FRAMES` (8) cycles and its mean without its largest cycle is more than 0.5% above the interval, or after 2 cycles in a row of at least 1.2 intervals. The window keeps only its last `SPEED_RECENT_FRAMES` (30) cycles when their mean differs from the older ones by 10%, and a clearly slow run that turns clearly fast clears it at once. One long frame of a fast core changes nothing. While a measure is in use and until it settles, the queue corrects the rate outside its band (`QUEUE_BAND_PERMILLE`, `QUEUE_TRIM_PERMILLE`, `QUEUE_SMOOTH_FRAMES`), so it comes back to its target within about 3 s. The run rate moves by at most `RUN_RATE_SLEW_PERMILLE_PER_S` (4000 permille of the mode's rate per second). So a game at 96% speed plays its sound at 96% of its tempo and pitch, with no underrun. A measured rate below `SOUND_MUTE_PERMILLE` (half the core's frame rate) mutes the audio until it is back above `SOUND_RESUME_PERMILLE` (52%), and the core's frames feed no audio and are idle frames of the pipeline, as under the quick menu: no underrun counts while the device runs out of silence.
- While `current_refused()` of the engine is on, the session mutes the audio and presents no game picture.
- `Counters::missed_fields` counts the fields in which no new frame reached the display while the core ran: the vertical blank count (`FlipEvent::sequence`) of each flip of a frame with the core, less the count of the flip before, less the blanks one flip takes (1, or 2 for an interlaced mode on a driver that flips once per frame). A flip late by one field adds 1. A modeset starts the count again from its first flip, and a flip under the quick menu adds nothing but marks the place the next gap counts from. `flipless_frames` (frames with no completed flip) does not change.
- `Counters::input_to_flip_ns` is the last delay from an input poll that returned an event to the first flip of the frame that ran the core with it, on the session clock. The flip event can come from another clock with another origin, so the session keeps the lowest offset between the two clocks that a wait showed (a wait that returns at the flip), and moves the flip time to its own clock with it. In late pacing the frame after waits for a flip that completed earlier, and the offset still gives the time of that flip.
- `Counters::flips` counts every flip that completed: the flips the loop waits for, the flip that the next frame waits for in late pacing, and the flip that the engine drains after a failed wait.
- The user volume (`audio.volume`, `docs/spec/audio-settings.md`) is a gain in the pipeline, apart from the mute. The pipeline starts at `SessionOptions::volume_percent` with no ramp; `set_volume_percent` changes it live with a 20 ms ramp; a new pipeline for a core timing change keeps it. A sink that stops taking data (a lost device) only costs the frames it does not take: the loop never waits on it.
- No allocation and no file I/O in a steady frame. The buffers (input events, hotkeys, the last frame, the message, the save RAM copies) are made at launch. A core timing change makes a new audio pipeline, as the spec says.

## Launch: the first mode

Genesis Plus GX reports 256x192 at load and sends 320x224 from frame 4 (`tests/traces`, OP-055). The engine sets its first mode with no debounce, so starting from the load geometry gives a wrong mode and a second modeset four frames later (`launch::without_the_settle_the_engine_sets_two_modes`).

The session runs the core with no display until the source has been the same for `LAUNCH_STABLE_FRAMES` (6) frames, at most `LAUNCH_SETTLE_MAX_FRAMES` (30). The audio of these frames is dropped and the core reads no input. The first `run_frame` then sets the game's mode, once (`launch::the_first_mode_is_the_mode_of_the_settled_source`). The cost is at most 100 ms more before the first picture, 10 frames for Genesis Plus GX. The data has no default geometry per system, so the session cannot start from one.

## A game that does not start

The session keeps the error-level log lines of the core from the load and the settle frames, at most `START_ERRORS_MAX` (8): `Session::start_errors`. It reads them from `Host::error_logs`, which keeps them also when a noisy call fills the log queue; `Session::start_dropped_logs` counts the records of the launch that the queue dropped. Only error lines of the core count: info, warn, debug and host lines are left out (`start.rs`, `collect_errors`).

A `retro_load_game` that returns false gives `LaunchError::Refused`, and a `SHUTDOWN` during the load or the settle gives `LaunchError::CoreStopped`; both carry those lines, and no mode was set.

A `SHUTDOWN` in the loop ends the session with the reason of `end_reason` (`start.rs`, pure): `EndReason::FailedStart` when no loop frame ran the core with a picture yet, or when the core logged an error line or sent a message at the launch and the stop comes within `START_WINDOW_NS` (10 s of the session clock) of the end of the launch; else `EndReason::CoreShutdown`. A picture that a refused mode hides, or the output off, still counts: the game has started. FinalBurn Neo loads a set whose driver does not start, logs why, shows its own error screen and sends `SHUTDOWN` at a button press (`docs/spec/arcade.md`, section 10.1).

For the notice of a stop, `Session::last_message` gives the last `SET_MESSAGE` text and `Session::last_error` the last error line of the core in the frames. The copy of that line goes into a buffer made at launch, so the frame loop does not allocate.

## The source

The size comes from the last frame, because some cores send a new size without telling the frontend. The rate and the aspect come from the core's AV info and geometry events. A frame of more than 288 lines (`INTERLACE_MIN_LINES`) is interlaced: the cores do not say it, and 288 is the tallest progressive console picture. A system whose data sets `always_progressive` (`arcade`, `neogeo`) has no interlaced frame: the launch request carries the key (`GameLaunch::always_progressive`).

The rotation is the turn that the core asks for with `SET_ROTATION` (`Host::rotation`), read for each frame, so a turn in a frame is a new source that the engine debounces. The aspect is the one of the frame as the core draws it, which the policy reads before it turns the source: for `R90` and `R270` the session inverts an aspect that the core reported (`round(1_000_000 / aspect)`: FinalBurn Neo reports 750, the 3:4 of the upright picture, for a 320x240 frame on its side, and the source gets 1,333). The host's fallback aspect (`GameGeometry::aspect_from_core` false), the base width over the base height, is already the shape of the frame as drawn and stays. `R180` keeps the aspect.

## The quick menu

- Select+Start held together for 0.5 s (`QUICK_MENU_HOLD_NS`), and the Start hold (`input.menu_hold_start_seconds`), open it. The hotkey pauses the core. A second quick menu hotkey while it is open does nothing: the menu is never a toggle (OP-051). The app closes it with `resume()`.
- On `resume()`, every button held at that moment reaches the core only after its release. The button that chose "resume" does not reach the game, and after a Start hold the game sees Start go up and no new press (OP-051).
- Select and Start down together on a port can be the quick menu hold, and on an arcade stick a coin and a start that overlap (`PadMask`). The button that completes the combination is held back while the combination is down, so the core does not see both down during a hold. When the combination ends before the menu opens, the held-back press reaches the core late: until its release, or, when it is up already, for as many core frames as it was held back, at least `LATE_PRESS_MIN_FRAMES` (3) and at most `LATE_PRESS_MAX_FRAMES` (30). Only frames in which the core runs count (`PadMask::core_ran`), so a late press does not run out while the core is paused or the output is off. So the game gets the coin and the start. A short tap of Select and Start reaches the game, as on the original hardware, so a game's own Select and Start combination (for example a soft reset with other buttons) works with a tap; only a hold of 0.5 s opens the menu. When the menu opens, the held-back press is dropped, and at `resume()` every button still down waits for its release: the menu leaves no stuck or phantom press.
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

## What the app must do

1. Resolve the settings: `policy_settings` and `scanline_settings` of `neptron-video`, `input.menu_hold_start_seconds` times 10^9, the core option values. Pick `RunRateSource::ReadBack` for KMS and `Measured` for the window.
2. `Session::launch`. On `LaunchError`, go back to the game list with a notice. For `Refused` and `CoreStopped`, the notice quotes the core.
3. Each frame: build the UI snapshot from `paused()`, `video_status`, `message()`, `host().options()`, then let the UI update with the navigation of `input_events()`, draw the canvas, and call `run_frame(Some(canvas))` (or `None` when nothing is drawn).
4. `FrameReport::menu_opened`: show the quick menu. The UI intent `Resume` calls `resume()`, and `QuitGame` calls `quit()`. The UI has no reset intent, so the app does not call `reset()`. A second quick menu hotkey while it is open is ignored by the session.
5. `FrameReport::blind_recovery`, `FrameReport::blanked`, `video().current_refused()`: pass to the safety layer (the `Guard` of `neptron-video`). While `blanked`, pace the loop with the clock.
6. `FrameReport::ended`, `SessionError`: call `quit()` and re-execute for the game list. For `EndReason::FailedStart`, the game list shows a notice with `start_errors()` or `last_message()`; for `CoreShutdown`, a notice with `last_message()` or `last_error()`.
7. Make the save folder writable. The session makes it when it is missing.

## Must never depend on

- `neptron-ui` (only the app binary depends on it)
- `neptron-kms`, `neptron-window`

## Rules

- `unsafe` is forbidden: the crate root and the test root carry `#![forbid(unsafe_code)]`. The tests set the test core's script variable on a child process of the test binary, because setting it in the running process is `unsafe`.
- No features.

## Tests

- Unit tests: the source and the launch settle, the button mask, the frame copy, the save RAM writer (backup, unchanged, many flushes, failed write, failed copy), the message cut.
- `tests/integration/session.rs`, the scripted session with the test core through the real host: one frame per flip, the input of each frame, the quick menu (pause, no toggle, UI over the frozen frame, the mask on resume), the Start hold, a 5 s recovery hold that never reaches the core, save RAM at launch, on the timer, at the menu and at exit, a core shutdown, double refresh, scanline settings and video status, core options, a timing change.
- `tests/integration/launch.rs`: the first mode for the Genesis Plus GX geometry, and the two modesets without the settle.
- `tests/integration/start.rs`: a refused load with the error line of the core, a `SHUTDOWN` in the load and in a settle frame (no mode, no sound), a `SHUTDOWN` in the first loop frame, a `SHUTDOWN` within the start window after an error line or a message, one after the window, info and warn lines that do not count, error lines of the settle frames up to the cap, a noisy load that keeps its error line and counts what it lost, and a long run in a refused mode.
- `tests/integration/arcade.rs`: held option values for a core that declares its options at load, the rotation of a board drawn on its side in the host of the session, the turned source (rotation and inverted aspect) and its scene on a 15 kHz CRT (1:1 interlaced, turned by 90 then 270 degrees after the debounce), a half turn, the fallback aspect, `always_progressive` against a console system, no save file for a core with no save RAM, and the zip listing through the session.
- `tests/integration/pacing.rs` and `pacing_edges.rs`: the input delay of a fast core, a core that needs more than one flip interval, late flips from the GPU, the return to wait pacing, repeated long frames, flips with no blank count, and the safety paths in late pacing.
- `tests/integration/speed.rs` and `slow_notice.rs`: the run rate at the measured speed of a late core, its bounded change, the mute below half speed, and the speed for the notice of a slow game.
- `tests/integration/volume.rs`: the user volume, and a sound device lost during a game.
- `tests/integration/faults.rs`: a slow flip, a failed present and wait, a display that stops flipping, the output turned off and on again.
- `tests/integration/timing.rs`: timing check 6 at 50.00, 59.94 and 60.10 Hz with jitter, with late flips, and with a display rate that differs from the mode (measured run rate). 30 simulated seconds each in the tier. Two 100,000-frame runs (about 28 simulated minutes, about 67 s each in the dev profile) are ignored tests: with jitter only, and with one late flip in a thousand.

```sh
cargo xtask test -p neptron-session --run-ignored only -E 'test(hundred_thousand)'
```

A late flip takes one frame of audio (16.7 ms) from the 32 ms queue, and the law gives it back with a time constant of 6.4 s. Two late flips a few seconds apart run the queue dry. The long run with late flips asserts that every underrun follows two late flips in the 10 s before it, so the session adds no underrun of its own. A larger target level would trade latency for this margin.

## Status

It runs the real cores on the i3-6100T and on a Pi 3 B. On composite NTSC on the Pi, NES, SNES and Mega Drive games hold 60.054 frames per second with no missed field (OP-103).
