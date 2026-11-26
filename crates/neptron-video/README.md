# neptron-video

The ODREN engine: core geometry to policy to safety to display. It debounces, commits, mutes and keeps the switch trace. The `guard` module holds confirm-or-revert, last known good, safe mode and blind recovery. [Flow 4](../../docs/flows/04-odren.md) and [flow 3](../../docs/flows/03-guard.md) give every transition in full.

## Must never depend on

- `neptron-kms` (it uses the `Display` trait only)
- `neptron-ui`, `neptron-audio`
- Any hardware crate

`unsafe` is forbidden (`#![forbid(unsafe_code)]`). No features.

## Public API

- `Odren`: the state machine. No I/O, no display. `on_frame(SourceGeometry) -> Directive`. Inputs: `set_monitor`, `set_settings`, `set_veto`; `set_scanlines` changes the scene only. `retry()` lifts a blocked source and starts new counts of modeset and blank attempts. `scene` and `preview_scene(&Proposal, &SourceGeometry, game, ui)` build the scene of the mode on screen and of a Display panel row through one function. Reads: `current()`, `current_refused()`, `blanked()`, `source_blocked()`, `decision_due()`, `settled()`, `trace()`, `muted()`, `picture_choices`. `answers`, `step_answers(budget)` and `answers_now(&AnswerRequest)` cache the picture answers of a request (sources, settings, scanlines of a level), keyed by the request and the input generation, and make new ones a few decisions at a time (`ChoicesWork`).
  - `decision_due()`: true from an input change or `retry()` until a frame decides on the new inputs and its modeset does not fail.
  - `settled()`: no decision is due, and the mode on screen comes from a decision on the current input generation (a modeset whose read back holds, or a scale-only change). A failed modeset, no winner, a blocked source and a restore keep the older generation. A blank leaves no mode.
- `VideoEngine<D: Display>`: `Odren` on a display. `new(display, Monitor, PolicySettings)`, `begin_frame(SourceGeometry) -> Directive`, `present(game, ui) -> Result<Presented, VideoError>`, `present_picture(Option<GamePicture>, picture_scale, ui)` for a GPU picture or a picture at an internal resolution, `core_render()` (the `CoreRender` of the display, if any), `wait_flip() -> Result<FlipEvent, VideoError>`, `video_status`, `odren()`, `display()`, and the `Odren` inputs and reads, plus `picture_choices_with`, `picture_choices_for_every_game`, `request_for`, `answers_in_work`.
- `Guard` and its types: `DisplayConfig`, `Health`, `GuardRecord`, `GuardStep`, `GuardEvent`, `Phase`, `Prompt`, `PromptId`, `SafeReason`, `RevertReason`, `ConfirmError`. See "The guard".
- `Monitor`: `Profile(MonitorProfile)` or `Edid(EdidProfile)`. `caps_for(&GpuCaps)` applies the EDID pixel clock limit (`EdidProfile::limit_caps`).
- `policy_settings(style, refresh_policy, horizontal)` and `scanline_settings(mode, strength_percent)` map the settings values to `PolicySettings` and `ScanlineSettings`, or `SettingsError`. Until the session calls `set_scanlines`, the engine draws none (`ScanlineSettings::OFF`).
- `build_scene(&SceneInputs, Option<GamePicture>, ui) -> Scene`: the one function that builds a `Scene`, for a picture in memory or on the GPU. A picture at the internal resolution `SceneInputs::picture_scale` (`k`) is `k` times its source: every rule runs on the native size, and the crop is `k` times the native crop, with sharp bilinear when the destination is not a whole multiple of it. It keeps the placement's crop when the frame has the size of the placement's source and the crop lies inside it; else the whole frame goes to the destination. Scanlines come from `neptron_types::scanlines_for`. The UI canvas turns by the orientation and goes where `ui_placement` places it after its turn, from the size of its frame (the canvas of `SceneInputs::form` when the scene has none), with the filter of `ui_placement` at every turn: `Nearest` when each axis scales by 1 or more, `SharpBilinear` below 1, as in a game mode of 224 lines or of fewer than 320 samples on a CRT.
- The form of the UI and the menu: `form()` on `Odren` and `VideoEngine` is `form_of(kind, crt_face, screen)`, the rule of `Form::of` with the face of the monitor (a CRT: the solver's face; a TV: 4:3; a flat panel: its first mode). The app draws the canvas of that form. `Content::Menu` makes the policy plan `menu_source(form, ui_turn, &menu)`: the raster of `T`, the canvas after its turn (`turned_canvas`), progressive and not turned. On a CRT, a TV and a landscape panel that is 320x240 at every orientation, so the menu keeps its mode when the screen turns (`docs/spec/ui-layout.md`, section 6.3).
- `Current` (the winner `Chosen`, the read back, the source), `SwitchTrace`, `SwitchRecord`, `Outcome`, `Trigger`, `Refusal`, `Unshowable`, `VideoError`.
- Re-exports of the policy types that callers name (`PolicySettings`, `PictureStyle`, `RefreshPolicy`, `Proposal`, `Timing`, `Placement`, and others), because only this crate may depend on `neptron-modeline`.

## One core frame

```text
let directive = engine.begin_frame(source);   // decides, no display call
audio.set_muted(engine.muted());              // silent before a modeset
for _ in 0..repeat {                           // 2 for double refresh
    engine.present(Some(game), ui)?;           // set_mode if due, then one flip
    engine.wait_flip()?;
    audio.set_muted(engine.muted());           // sound again after the new mode flips
}
```

`repeat` is `Current::chosen.proposal.timing.repeat()`. Frame pacing and audio rate control use `Current::read_back.field_rate_mhz()`.

## The video status

`write_video_status(Option<&StatusInputs>, &mut VideoStatusText)` writes the video block of the quick menu and the display panel as text. It returns `None` before the first mode, and the UI then shows no block. `Odren::video_status` and `VideoEngine::video_status` call it with the read back of the mode on screen. Inputs built from a proposal show a requested mode: only the Display panel preview may use them. The function rewrites caller-owned strings, so a steady frame allocates nothing. Integers only. The doc comments in `src/status.rs` give the exact format.

| Field | Content | Example |
| --- | --- | --- |
| `game` | Source size padded to 10, 10 spaces, field rate with 3 decimals | `256x224             60.099 Hz` |
| `mode` | Read back size and scan padded to 10, line rate padded to 10 (2 decimals, 1 from 100 kHz up), field rate, two spaces, rung (`native`, `2x`, `3x`, `4x`, `fit`, `panel`). A TV shows its standard in place of the width and no rung | `768x448p  30.05 kHz 60.099 Hz  2x` |
| `result` | `exact`, `sharp`, `double refresh`, `cropped N lines`, `small picture N%`, `filtered`, `speed (X to Y Hz)`, in that order, joined with `, `. An alert starts with `ALERT_MARK` (`[!] `), so it shows also where its colour is hard to see | `[!] cropped 2 lines, speed (57.550 to 59.940 Hz)` |
| `alert` | `filtered` (never on a flat panel), or `speed` above `SPEED_ALERT_MIN_PPM` | |

- `exact`: not fitted, no crop, not padded, no speed change. It combines only with `double refresh`. A fractional horizontal scale under a fixed super width counts as `exact`, because the horizontal axis of a CRT is analog.
- `sharp`: a flat panel picture that scales an axis by a fraction (sharp bilinear, or nearest at a scale from 1 to 2). The owner's rule: a panel picture must be sharp, not pixel exact, so it is never an alert. Whole factors on both axes give `exact`.
- `double refresh`: `Timing::repeat() == 2`. `small picture N%`: the fill of a padded winner, whole percent. `filtered`: the fitted rung on a CRT.
- `speed`: X is the game's rate, Y the read-back field rate per game frame. On a CRT it shows from `REFRESH_MATCH_MHZ` (5 mHz) of difference and is an alert only above `SPEED_ALERT_MIN_PPM` (5000 ppm of the shown rate, the one alert threshold of every display class). On a flat panel it shows only above `PANEL_SPEED_SILENT_PPM` (5000 ppm), the bound of the audio rate control (`neptron_audio::MAX_RATE_DEVIATION_PPM`): 60.099 Hz on 60 Hz is silent, a 50 Hz game on a 60 Hz mode gives `[!] speed (50.000 to 60.000 Hz)`. A TV uses `TV_SPEED_SILENT_PPM`. The bounds compare in parts per million of the shown rate (`neptron_modeline::policy::speed_beyond`, `speed_alert`).
- The mode line fits the 37 characters of the UI value column (`3840x2160p 133.3 kHz 59.997 Hz  panel` is 37). The longest real results wrap into two lines of 37. The word is `speed`, not `speed changed`, so that they need no third line.

## The state machine

| Directive | When | What the display does |
| --- | --- | --- |
| `Steady` | The source equals the source on screen | One flip with the stored placement |
| `Rescale` | The winner equals the mode on screen | One flip with the new crop, destination and filter. No debounce, no mute |
| `Waiting` | A new source needs a new mode and has held for fewer than `DEBOUNCE_FRAMES` (3) core frames | One flip in the old mode; the frame shows whole in the old destination |
| `Modeset` | The new source held for 3 frames, or launch, or a monitor, settings or retry change (no debounce) | Mute; `set_mode` with the winner's token; one flip; unmute at that flip |
| `Kept` | No plan, or every proposal refused | The mode on screen stays; the trace says why. Before the first mode, `present` returns `NoMode` |
| `Blank` | The inputs refuse the mode on the display (`current_refused()`), and the decision gives no mode to set | `present` calls `Display::blank`, records `Blanked` or `BlankFailed`, returns `NoMode` |

Rules:

- The display gets `token.mode()` of the winner, the `ValidatedMode` from `neptron_safety::validate` with the monitor's caps, after an optional veto that can only refuse more (`set_veto`). `proposal.mode` never reaches the display. `picture_choices` runs the same check, so the preview and the run give one answer.
- No winner: the mode on screen stays, and the trace records `Kept` with every refusal. The engine never guesses a mode.
- A turned game (`Rotation` other than `R0`) is planned like any game. The scene takes the rotation of the source the placement was planned for (ADR 0015). A new rotation is a new source and goes through the debounce.
- Decisions are cached by source (`CACHE_ENTRIES`, 8). A monitor, settings or veto change clears the cache. The steady state does no decision and no allocation.
- Each change of the monitor, the settings, the veto or the driver refusals raises the input generation and drops the pending modeset. A token travels with its generation (`ModeOrder`), and `Odren::authorize` refuses an older one, so a token reaches `set_mode` only while the inputs that validated it are current.
- A modeset that a monitor, settings or retry change asks for is decided again on the next frame when its present does not run or fails, up to `MODESET_ATTEMPTS` (3) attempts in all.
- Consecutive scale-only decisions share one trace record (`SwitchRecord::count`), so a flicker under a fixed super width does not push the modesets out of the 64-record trace.

## Display errors

| Error | What the engine does |
| --- | --- |
| `set_mode` returns `ModeRefused` | The mode on screen stays. The mode is stored as refused by the driver, and the next frame tries the next proposal with no debounce (`Trigger::Retry`). A monitor change forgets the refused modes |
| `set_mode` returns `Device` or another error, for a geometry or launch modeset | The mode on screen stays. The source is blocked until the source on screen comes back, the monitor or settings change, or `retry()`. No retry storm |
| The same, for a monitor, settings or retry modeset | Tried again on the next frame, up to `MODESET_ATTEMPTS` (3) in all; then the mode on screen stays, and `current_refused()` tells whether the inputs refuse it |
| `set_mode` reads back another size or scan, or a mode the validator refuses | The mode is stored as refused. The last good mode is validated again with the current inputs and restored only with that new token; its read back must hold. With no last good mode, a refused one, or a restore that fails or reads back wrong, the engine forgets its mode and blanks in the same present. A failed `set_mode` of the restore gives `RestoreFailed` |
| `blank` fails | Trace `BlankFailed`; `current_refused()` stays on. Tried again each frame, up to `MODESET_ATTEMPTS` (3); an input change or `retry()` starts a new count |
| A flip is queued before a present or a modeset | The engine waits for it first, because the display refuses `set_mode` and `present` while a flip is pending (OP-032). `Presented::drained_flip` gives the event |
| A wait fails (`wait_flip` or that drain) | The flip stays queued: the `Display` contract bounds the wait and never drops a queued flip. Until a wait returns its event, `present` sets and queues nothing and returns `PendingFlip`. The mute stays until a flip in the new mode completes |
| `present` fails | `VideoError::Present`; no flip is queued |
| The read back has another pixel clock | Kept when size and scan match and the validator accepts it. A field rate off by `READ_BACK_RATE_NOTICE_MHZ` (5 mHz) or more goes in `rate_drift_mhz` of the record |

## A mode the inputs refuse

- `set_monitor`, `set_settings` and `set_veto` judge the mode on the display with the new inputs. A refusal is traced as `Kept(CurrentRefused)`, and `current_refused()` turns on. A refused read back with no restore, or a failed restore, turns it on too.
- When the next decision gives no mode to set, the frame gets `Directive::Blank`. `blank` carries no mode: it is the one display change without a `ValidatedMode`.
- A blank that succeeds forgets the mode, records `Blanked`, turns `current_refused()` off and `blanked()` on. `present` returns `NoMode` until a later valid decision sets a mode with a normal modeset (a real commit on every backend).
- `current_refused()` stays on until a valid mode is on screen or the output is off, also after `MODESET_ATTEMPTS` failed blanks. Then a refused mode is still on the wire.

Callers: the guard counts either flag as no picture (see "The guard"). The session shows no game picture as confirmed while either is on, keeps the audio muted, and passes the state to the guard and to the control protocol.

## The guard

`Guard` decides and does no I/O. It changes the display only through the engine inputs `retry`, `set_monitor` and `set_settings`, so every mode it causes is a winner that the validator accepted for the current monitor. A revert target that the current inputs refuse is never set: the engine blanks or picks another valid mode. It reads the engine through `Health::of` and never calls the display. It owns a `DisplayConfig`: the monitor and the policy settings.

`docs/spec/display-guard.md` holds the rules, the trust classes, the record file and the notices. ADR 0013 holds the changes to the safety model.

| Constant | Value | Meaning |
| --- | --- | --- |
| `CONFIRM_WINDOW_NS` | 15 s | A change reverts unless confirmed in this time, counted from its first settled frame (until then, from the change) |
| `SAFE_MODE_FAILED_BOOTS` | 2 | Failed boots in a row before the next boot starts in safe mode |
| `GOOD_BOOT_PICTURE_NS` | 3 s | A settled picture of the confirmed configuration or of the safe default this long makes the run good |
| `NO_PICTURE_LIMIT_NS` | 1 s | No valid picture this long makes the guard leave the configuration |
| `PENDING_LIMIT_NS` | 30 s | A chain of unconfirmed changes reverts at the latest this long after the first |

The blind recovery hold (5 s, `BLIND_RECOVERY_HOLD_NS`) belongs to `neptron-input`. It sends one `Hotkey::BlindRecovery`, and the app calls `Guard::recover`.

### Narrower than the safety model

The CRT safety model (rows 4 to 6) asks more than the guard does, on purpose:

- Confirm or revert covers what the app sends to `propose`. The connector (`display.output`) is not an input of the engine: the menu shows it read only, and the settings file changes it. Signal geometry offsets do not exist in the MVP.
- Last known good holds the monitor only, because the policy settings resolve per game. A revert restores the settings that were active before the change, from memory.
- The guard starts at `Guard::boot`. The blind recovery before that belongs to the launcher and the app. Safe mode at boot comes from the record of an earlier boot.
- A boot needs no confirmation for a known monitor: the safe default's, a trusted one (a TV output, an EDID flat panel on a digital link with `auto` or `flat`), or the last known good (ADR 0013).

### States

- **Confirmed.** The engine has the confirmed configuration.
- **Pending.** A change waits for a confirmation, with a `Prompt` (id and deadline). It keeps the revert target: the configuration confirmed before it, or the safe default when it came from safe mode.
- **Safe.** The engine has the safe default, with a `SafeReason`: `FailedBoots`, `Recovery`, `NoValidMode` or `NothingConfirmed`.

[Flow 3, "Calls and conditions"](../../docs/flows/03-guard.md#calls-and-conditions) gives every transition with its condition, event and record change. In short: a boot goes to Safe after 2 failed boots or with the record in safe mode, to Confirmed for a known monitor, else to Pending (the boot prompt). `continue_boot` never opens Pending. `propose` opens Pending. Pending ends in Confirmed on a valid `confirm`, or in the revert target on `decline`, the deadline, 1 s with no valid picture, a clock that goes back, or `propose` of the confirmed configuration or `follow_settings` (`Superseded`). Confirmed goes to Safe(NoValidMode) after 1 s with no valid picture. In Safe, `keep_shown` confirms the safe default, and in Safe(FailedBoots) `retry_settings` leaves safe mode. `recover` goes to Safe(Recovery) from any state. `close_run`, and the first settled picture of 3 s in Confirmed or Safe (`GoodBoot`), clear the run marker.

- The guard never uses a time earlier than one it has seen, so a clock that goes back cannot stretch a deadline or restart a watch.
- `Reverted { changed, rewrite }`: `changed` is false when the restored configuration equals the change, and the app shows no notice. `rewrite` is `Some` when the pending chain began with the boot prompt: the settings files still hold that unconfirmed configuration. It holds the revert target, so a later `follow_settings` (game settings) never reaches the global settings. A menu change is never in the settings files before `Confirmed`, so its revert gives `None`.
- No valid picture (`Health::no_picture`): the output is off, a refused mode is on the display, or no mode is set. A mode that waits for its first flip does not count, because a flip can be late.
- The no-picture watch stops only at a valid picture or a revert. A new change, a `propose` while pending or a `follow_settings` does not restart it. After a revert the restored configuration gets its own 1 s.
- A picture counts only when the mode on the display comes from a decision on the guard's inputs (`Health::decided`, from `Odren::settled`). Every guard change raises the input generation or sets a decision due, so the old mode never counts, also when it passes the new inputs. `confirm` then gets `NotShown`, and the deadline reverts.
- `confirm` fails with `ConfirmError` for no open prompt, another id, a time at or after the deadline, no valid picture now, or no picture of the change seen (`NotShown`). Nothing changes.
- A revert to the safe default goes back to the same `SafeReason`. In Safe, `follow_settings` changes nothing. When the safe default gives no picture, the guard does nothing more: no signal is safe.
- Every change to the engine starts with `retry`, so a recovery reaches the display in any state: while blank, after failed blanks, with a blocked source, and before the first frame.

### Last known good and failed boots

`GuardRecord` is plain data: `last_known_good: Option<Monitor>`, `failed_boots`, `safe_mode`, `run_open`. The app keeps it apart from the user settings, one last known good per place (connector and monitor identity), and gives the guard the one of its place.

- Last known good changes only when the user keeps a configuration. A long stable picture makes the run good but records no monitor. The guard never clears it; the app gives `None` when it cannot build the stored profile again.
- `run_open` is the run marker. `Guard::boot` sets it before the first mode. A settled picture of 3 s in Confirmed or Safe clears it, and so does `close_run`. A boot that finds it set adds one to `failed_boots`. `continue_boot` neither sets it nor counts.
- `failed_boots` goes to 0 when the user keeps a configuration, and at a good picture outside safe mode. In safe mode it stays, so a start in `Safe(FailedBoots)` stays there until the user keeps a configuration.
- `safe_mode` is set by a recovery and by safe mode after failed boots. Only `confirm`, `keep_shown` or `retry_settings` clears it.

### Engine states

| Engine | What the guard does | Left to the session |
| --- | --- | --- |
| `current_refused()` or `blanked()` on, or no mode set | Counts it as no picture. After 1 s: reverts a pending change, or goes to the safe default | Muting the audio and showing no picture as confirmed |
| `retry()` | Calls it on every change it makes | Calls it when the player asks, for example when the quick menu closes |
| A flip that never completes (`PendingFlip`, OP-062 point 1) | Nothing: the mode on the wire is valid. `muted()` stays on, so a confirmation is refused and the deadline reverts. The revert waits for the flip like any modeset | Opening the display again after a number of `PendingFlip` errors |

### What the app must do

1. At boot, before the first frame: load the record (an unreadable one is `GuardRecord::default()`), build `wanted` from the settings and `safe` for the detected output, call `Guard::boot`, and write the record it returns before the first modeset. A run that continues a boot (a game after the game list) calls `Guard::continue_boot` instead.
2. Write every `GuardStep::save` with temp file, fsync, rename, off the frame thread.
3. Call `tick` once per frame, after `present`, also when it failed. Read the monotonic clock once per frame and give that value to every guard call of the frame. An earlier time makes the guard revert with `ClockWentBack`.
4. Send a change of the monitor profile or the TV standard (also through the control protocol) to `propose`. A picture mode, refresh rate or TV picture width change goes to `follow_settings` while `phase()` is `Confirmed`, else to `propose`. The resolved settings of a game go to `follow_settings`. Never call `set_monitor` or `set_settings` of the engine directly.
5. `PromptOpened`: put the confirm dialog in the snapshot with `remaining_ns` from `Prompt::deadline_ns`, map `PromptId` to the UI's `ConfirmId`, stop every key repeat and send nothing for a key held now (OP-048).
6. `KeepMode`: call `confirm` with the open id and the frame time. A `ConfirmError` keeps the dialog; a `KeepMode` after the deadline is dropped. `RevertMode`: call `decline`.
7. `Confirmed`: write the changed settings now. The settings files hold a display change only once it is confirmed, so a crash during a pending change leaves the old value.
8. `Reverted` and `SafeMode`: close the dialog and show a notice. Write no setting, except for `Reverted` with `rewrite: Some(config)`: write the monitor and the settings of that `config` to the global settings, never `Guard::active()`.
9. `Hotkey::BlindRecovery`: call `recover` in every screen and state, also with a dialog open, and play the audio cue for its `SafeMode(Recovery)`. A start in safe mode plays no cue. Before `recover`, read the display again and give its safe default to `set_safe`, because another monitor can be on the connector. After a kept change of the monitor, give the safe default of the kept setting to `set_safe` too.
10. Show safe mode in the display panel while `phase()` is `Safe`.

## Tests

- Unit tests: the cache, the scene builder, the monitor caps, the settings mapping, and an `Odren` order from older inputs.
- `tests/integration/`, one file per area:
  - `engine.rs`: launch, steady state, flicker, debounce, scale only, mute order, double refresh, a queued flip before a modeset, the trace, a turned game, the Display panel rows.
  - `faults.rs`: veto, no winner, each `DisplayError`, read backs that differ, restores, `retry()`, the read-back rate note, calls out of order.
  - `inputs.rs`: monitor and settings changes, stale tokens, the attempt bound, `current_refused()`, blanks and their bound, the EDID pixel clock limit.
  - `panel.rs`: flat panels from 4:3 to 21:9 and 720p to 2160p with 256x224, 320x224 and 320x240 games: native mode, rectangles worked out by hand, the filter, no downscale, a 50 Hz game, and the rendered pixels of SNES on 1080p with sharp bilinear.
  - `stall.rs`: waits that time out with the flip queued (`FlipFault::Slow`, `FlipFault::Lost`).
  - `guard_confirm.rs`, `guard_boot.rs`, `guard_recovery.rs`, `guard_tv.rs`: every row of the guard table and its bullets. The guard rig (`guard_rig.rs`) checks after every frame that a mode on the wire passes the validator for the current monitor, unless the engine reports it as refused.
  - `scanlines.rs`: the scanlines of the scene from the settings, the mode and the scale.
  - `check5.rs`: timing check 5. Two scripted cores (SNES under a fixed super width, Mega Drive under the native width) change width, lines, scan and rate. The switches must equal `tests/golden/switch_trace.txt` (modes on `generic_15`), and every frame checks the mute and the debounce. A policy change shows as a diff to check line by line.
