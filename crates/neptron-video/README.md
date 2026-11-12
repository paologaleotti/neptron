# neptron-video

The ODREN engine: core geometry to policy to safety to display. Debounce, commit, mute and the switch trace. The `guard` module holds confirm-or-revert, last known good, safe mode and blind recovery.

## Public API

- `Odren`: the state machine. It does no I/O and holds no display. `on_frame(SourceGeometry) -> Directive` takes the source of one core frame. `set_monitor`, `set_settings` and `set_veto` change its inputs; `set_scanlines` changes the scene only; `retry()` lifts a blocked source and starts new counts of modeset and blank attempts. `scene(game, ui)` and `preview_scene(&Proposal, &SourceGeometry, game, ui)` build the scene of the mode on screen and of a Display panel row through the same function. `current()`, `current_refused()`, `blanked()`, `trace()`, `muted()`, `style_rows(&SourceGeometry)` read it.
- `VideoEngine<D: Display>`: `Odren` on a display. `new(display, Monitor, PolicySettings)`, `begin_frame(SourceGeometry) -> Directive`, `present(game, ui) -> Result<Presented, VideoError>`, `wait_flip() -> Result<FlipEvent, VideoError>`, `muted()`, `current()`, `current_refused()`, `blanked()`, `trace()`, `style_rows`, `set_monitor`, `set_settings`, `set_veto`, `set_scanlines`, `preview_scene`, `retry()`, `display()`.
- `Guard`: confirm-or-revert, last known good, safe mode and blind recovery over the engine's inputs. `DisplayConfig`, `Health`, `GuardRecord`, `GuardStep`, `GuardEvent`, `Phase`, `Prompt`, `PromptId`, `SafeReason`, `RevertReason`, `ConfirmError`. See "The guard".
- `Monitor`: `Profile(MonitorProfile)` or `Edid(EdidProfile)`. `caps_for(&GpuCaps)` applies the EDID pixel clock limit (`EdidProfile::limit_caps`).
- `policy_settings(style, refresh_policy, horizontal) -> Result<PolicySettings, SettingsError>`: from the values of `display.style` and `display.refresh_policy`.
- `scanline_settings(mode, strength_percent) -> Result<ScanlineSettings, SettingsError>`: from the values of `display.scanlines` and `display.scanline_strength`. Until the session calls `set_scanlines`, the engine draws none (`ScanlineSettings::OFF`).
- `build_scene(&SceneInputs, game, ui) -> Scene`: the one function that builds a `Scene`. The placement's crop is kept when the frame has the size of the placement's source and the crop lies inside it; otherwise the whole frame goes to the destination. The scanlines come from `neptron_types::scanlines_for` with the settings, the whole vertical multiple of the scene's crop and destination, `ScreenClass::of` the monitor and the mode, and the interlace flag of the mode.
- `Current`: the winner on screen (`Chosen`: index, rung, target, proposal, token, notice), the mode read back, and the source.
- `SwitchTrace` of `SwitchRecord`s, `Outcome`, `Trigger`, `Refusal`, `Unshowable`, `VideoError`.
- Re-exports of the policy types that callers name (`PolicySettings`, `PictureStyle`, `RefreshPolicy`, `HorizontalStrategy`, `Notice`, `StyleRow`, `Timing`, `Placement`, and others), because only this crate may depend on `neptron-modeline`.

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

`repeat` is `Current::chosen.proposal.timing.repeat()`. The real field rate for frame pacing and audio rate control is `Current::read_back.field_rate_mhz()`.

## The video status

`write_video_status(Option<&StatusInputs>, &mut VideoStatusText) -> Option<&VideoStatusText>` writes the video block of the quick menu and the display panel as text. `Odren::video_status` and `VideoEngine::video_status` call it for the mode on screen; a preview builds `StatusInputs` from a proposal and calls the same function. The app copies the strings into the UI snapshot; the UI parses nothing. The function clears and rewrites caller-owned strings, so a steady frame allocates nothing. Integers only.

| Field | Content | Example |
| --- | --- | --- |
| `game` | The source size padded to 10, 10 spaces, the field rate with 3 decimals | `256x224             60.099 Hz` |
| `mode` | The read back: size and scan suffix padded to 10, line rate in kHz with 2 decimals padded to 10, field rate, two spaces, the rung (`native` for native and interlaced 1:1, `2x`, `3x`, `4x`, `fit`, `panel`) | `768x448p  30.05 kHz 60.099 Hz  2x` |
| `result` | `exact`, `double refresh`, `cropped N lines`, `small picture N%`, `filtered`, `speed (X to Y Hz)`, in that order, joined with `, `. An alert result starts with `ALERT_MARK`, `[!] ` | `[!] cropped 2 lines, speed (60.099 to 59.940 Hz)` |
| `alert` | `filtered` or `speed` | |

- `exact`: not fitted, no crop, not padded, and the speed does not change. It combines only with `double refresh`.
- A fractional horizontal scale under a fixed super width counts as `exact`: the lines are exact, and the horizontal axis of a CRT is analog, so it has no pixel grid to miss.
- `StatusInputs` built from a proposal show a requested mode. That is for the preview of the Display panel only; the running game's status always comes from `Odren::video_status`, from the read back.
- `double refresh`: the winner shows each frame twice (`Timing::repeat() == 2`), from `exact_x2` or from `native` on its own.
- `small picture N%`: the fill of a padded winner, rounded to whole percent.
- `filtered`: the fitted rung.
- `speed`: the speed of the game changes. X is the game's rate, Y the read-back field rate per game frame. It shows when they differ by `REFRESH_MATCH_MHZ` (5 mHz) or more, never on a flat panel (audio rate control absorbs the difference there, as in the policy's notice).
- Before the first mode is on screen the function returns `None`, and the UI shows no block.
- `[!] ` starts the result when `alert` is true, so the alert shows also where its colour is hard to tell apart. The mark is not a result word and has no `, ` after it.
- The worst mode line, `1920x238p 15.73 kHz 59.9xx Hz  native`, has 37 characters. The longest real results, with the mark, wrap into two lines of 37: `[!] double refresh, small picture 100%, speed (57.500 to 50.082 Hz)` gives 33 and 33 characters, and `[!] double refresh, cropped 16 lines, speed (100.000 to 60.099 Hz)` gives 37 and 28. The word is `speed` and not `speed changed`, because with `speed changed` and the mark the first of these needs three lines.

## The state machine

Each core frame gives one `Directive`:

| Directive | When | What the display does |
| --- | --- | --- |
| `Steady` | The source equals the source on screen | One flip with the stored placement |
| `Rescale` | The winner equals the mode on screen (step 7) | One flip with the new crop, destination and filter. No debounce, no mute |
| `Waiting` | A new source needs a new mode, and it has been the same for fewer than `DEBOUNCE_FRAMES` (3) core frames | One flip in the old mode; the frame is shown whole in the old destination |
| `Modeset` | The new source held for 3 frames, or at launch, or after a monitor, settings or driver retry (no debounce for these) | Mute; `set_mode` with the winner's token; one flip; unmute at that flip |
| `Kept` | The source is turned, the policy gives no plan, or every proposal is refused | The mode on screen stays; the trace says why. Before the first mode, `present` returns `NoMode` |
| `Blank` | The inputs refuse the mode on the display (`current_refused()`), and the decision gives no mode to set: no winner, or a source blocked after device errors | `present` calls `Display::blank`, records `Blanked` or `BlankFailed`, and returns `NoMode` |

Rules:

- The engine sets `token.mode()` of the winner, the `ValidatedMode` that `neptron_safety::validate` returned. `proposal.mode` never reaches the display.
- The validator is `neptron_safety::validate` with the monitor's caps, after an optional veto that can only refuse more (`set_veto`). The same check runs for the Display panel rows (`style_rows`), so the preview and the run give one answer.
- No winner: the mode on screen stays, and the trace records `Kept` with every refusal. The engine never guesses a mode. When the inputs also refuse the mode on screen, the engine turns the output off (`Blank`).
- A turned game (`Rotation` other than `R0`) is `Kept(Rotated)`: `Scene` has no rotation in the MVP.
- Decisions are cached by source (`CACHE_ENTRIES`, 8). A monitor, settings or veto change clears the cache. The steady state does no decision and no allocation.
- A token reaches `set_mode` only through `VideoEngine::set_mode`, and only while the inputs that validated it are current. Each change of the monitor, the settings, the veto or the driver refusals raises a generation and drops the pending modeset; a token travels with its generation (`ModeOrder`), and `Odren::authorize` refuses an older one.
- A modeset that a monitor, settings or retry change asks for is decided again on the next frame when its present does not run or fails, and after a device error, up to `MODESET_ATTEMPTS` (3) attempts in all.
- Consecutive scale-only decisions share one trace record (`SwitchRecord::count`), so a flicker under a fixed super width does not push the modesets out of the 64-record trace.

## Display errors

| Error | What the engine does |
| --- | --- |
| `set_mode` returns `ModeRefused` | The mode on screen stays (the `Display` contract). The engine stores the mode as refused by the driver, and the next frame tries the next proposal with no debounce (`Trigger::Retry`). A monitor change forgets the refused modes |
| `set_mode` returns `Device` or another error, for a geometry or launch modeset | The mode on screen stays. The engine blocks the source until the source on screen comes back, the monitor or the settings change, or the session calls `retry()`. No retry storm |
| The same, for a monitor, settings or retry modeset | The engine tries again on the next frame, up to `MODESET_ATTEMPTS` (3) attempts in all, then keeps the mode on screen; `current_refused()` tells whether the inputs refuse it |
| `set_mode` reads back a mode of another size or scan, or one the validator refuses | The engine stores the mode as refused by the driver, validates the last good mode again with the current inputs and restores it only with that new token. The read back of the restore must hold like a modeset. When there is no last good mode, or the inputs refuse it, or the restore fails or reads back wrong, the engine sets nothing more, forgets its mode, turns the output off with `blank` in the same present, and the next frame decides again; a failed `set_mode` of the restore gives `RestoreFailed` |
| `blank` fails | The trace records `BlankFailed`, and `current_refused()` stays on. The next frame tries again, up to `MODESET_ATTEMPTS` (3) attempts in all; an input change or `retry()` starts a new count |
| A flip is queued before a present or a modeset | The engine waits for it first, because the display refuses `set_mode` and `present` while a flip is pending (OP-032). `Presented::drained_flip` gives the event |
| A wait fails (`wait_flip` or that drain) | The flip stays queued until a wait returns its event, because the `Display` contract bounds the wait and never drops a queued flip. While it is queued, `present` sets and queues nothing and returns `PendingFlip`; a modeset happens on a later frame. A flip that never completes gives `PendingFlip` on every frame. The mute stays until a flip in the new mode completes |
| `present` fails | `VideoError::Present`; no flip is queued |
| The read back has another pixel clock | Kept when it has the same size and scan and passes the validator. When its field rate differs by `READ_BACK_RATE_NOTICE_MHZ` (5 mHz, the policy's two-decimal match) or more, the modeset record holds `rate_drift_mhz` |

## A mode the inputs refuse

The engine detects the case, turns the output off when no valid mode exists, and exposes both states:

- `set_monitor`, `set_settings` and `set_veto` judge the mode on the display with the new inputs. A refusal is recorded as `Kept(CurrentRefused)` with the reason, and `current_refused()` turns on.
- A refused read back with no restore, or a failed or refused restore, leaves a mode the engine does not know on the display, with no current mode. `current_refused()` turns on too, and the same present blanks.
- When the next decision gives no mode to set (`Kept`, or `Steady` on a blocked source), the frame gets `Directive::Blank`, and its `present` calls `Display::blank` once. A refused read back with no restore, and a failed restore, blank in the same present. `blank` carries no mode: it is the one display change without a `ValidatedMode`.
- A blank that succeeds forgets the mode, records `Blanked`, turns `current_refused()` off and `blanked()` on. `present` returns `NoMode` until a mode is set. A later valid decision (a new source, an input change, `retry()`) sets a mode with a normal modeset, which is a real commit on every backend, and `blanked()` turns off.
- `current_refused()` stays on until a valid mode is on screen or the output is off: a modeset or restore whose read back holds, a scale-only change to a winner the inputs accept, or a blank. It stays on after `MODESET_ATTEMPTS` failed blanks.

What the callers must do:

- The guard reads `current_refused()` and `blanked()` after each frame. When either stays on for `NO_PICTURE_LIMIT_NS`, it reverts to the last confirmed inputs, or to the safe default. `current_refused()` on after failed blanks means a refused mode is still on the wire. See "The guard".
- T25 (session) shows no game picture as confirmed while either is on, keeps the audio muted, and passes the state to the guard and to the control protocol.

## The guard

`Guard` decides and does no I/O. It changes the display only through the inputs of the engine: `retry`, `set_monitor` and `set_settings`. Every mode it causes is therefore a winner that the validator accepted for the current monitor, and a revert target that the current inputs refuse is never set: the engine blanks or picks another valid mode. It reads `current()`, `current_refused()`, `blanked()` and `muted()` through `Health::of`. It never calls the display.

The guard owns a `DisplayConfig`: the monitor and the policy settings. Only the monitor is last known good, because the policy settings resolve per game.

| Constant | Value | Meaning |
| --- | --- | --- |
| `CONFIRM_WINDOW_NS` | 10 s | A change the user asks for reverts unless confirmed in this time |
| `SAFE_MODE_FAILED_BOOTS` | 2 | Failed boots in a row before the next boot starts in safe mode |
| `GOOD_BOOT_PICTURE_NS` | 10 s | A picture of the confirmed configuration this long makes the boot good |
| `NO_PICTURE_LIMIT_NS` | 1 s | No valid picture this long makes the guard leave the configuration |

The hold time of the blind recovery (5 s, `BLIND_RECOVERY_HOLD_NS`) belongs to `neptron-input`. It sends one `Hotkey::BlindRecovery`, and the app calls `Guard::recover`.

### States

- **Confirmed.** The engine has the confirmed configuration.
- **Pending.** A change waits for a confirmation, with a `Prompt` (id and deadline). It keeps what a revert restores: the configuration confirmed before it, or the safe default when the guard came from safe mode.
- **Safe.** The engine has the safe default, with a `SafeReason`: `FailedBoots`, `Recovery`, `NoValidMode` or `NothingConfirmed`.

| From | Call or condition | To | Event |
| --- | --- | --- | --- |
| Boot | `failed_boots` at least 2 | Safe(FailedBoots) | `SafeMode` |
| Boot | the record is in safe mode | Safe(Recovery) | `SafeMode` |
| Boot | the settings' monitor is the last known good | Confirmed | none |
| Boot | otherwise | Pending, reverts to the last known good or to the safe default | `PromptOpened` |
| Confirmed or Safe | `propose` of a new configuration | Pending | `PromptOpened` |
| Pending | `propose` of another configuration | Pending with a new id and deadline, same revert target | `PromptOpened` |
| Pending | `propose` of the confirmed configuration, or `follow_settings` | Confirmed | `Reverted(Superseded)` |
| Pending | `confirm` with the open id, before the deadline, with a picture | Confirmed; the monitor is last known good; `failed_boots` 0; safe mode off | `Confirmed` |
| Pending | `decline` | the revert target | `Reverted(Declined)` |
| Pending | `tick` at or after the deadline | the revert target | `Reverted(Timeout)` |
| Pending | `tick`: no valid picture for 1 s | the revert target | `Reverted(NoPicture)` |
| Confirmed | `tick`: no valid picture for 1 s | Safe(NoValidMode) | `SafeMode` |
| Confirmed | `tick`: a picture for 10 s, first time in the boot | Confirmed; `failed_boots` 0 | `GoodBoot` |
| Any | `recover` | Safe(Recovery); the record is in safe mode | `SafeMode` |

- No valid picture (`Health::no_picture`): the output is off, a refused mode is on the display, or the engine has set no mode. A mode that waits for its first flip is not counted, because a flip can be late.
- `confirm` is refused with `ConfirmError` when no prompt is open, the id is another one, the time is at or after the deadline, or the engine shows no valid picture. Nothing changes; the deadline still reverts.
- A revert to the safe default (from a pending change that began in safe mode) goes back to the same `SafeReason`.
- In Safe, `follow_settings` changes nothing: the safe default keeps its settings. The guard does nothing more when the safe default gives no picture: no signal is safe, and the engine bounds its own attempts.
- Every change to the engine starts with `retry`, so a blocked source or exhausted blank attempts get a new set. A recovery in any state therefore reaches the display also while it is blank, while a refused mode stays after failed blanks, and before the first frame.

### Last known good and failed boots

`GuardRecord` is plain data: `last_known_good: Option<Monitor>`, `failed_boots`, `safe_mode`. It is kept apart from the user settings.

- Recorded only when the user confirms. A long stable picture makes the boot good, but records nothing.
- `failed_boots` goes up by one at each `Guard::boot`, before the first mode, and goes to 0 at a confirmation or a good boot. A boot that crashes or hangs before 10 s of picture is a failed boot. A boot in safe mode never counts as good, so safe mode stays until the user confirms a configuration.
- `safe_mode` is set by a recovery and by safe mode after failed boots. Only a confirmation clears it.
- The guard never clears `last_known_good`. The app gives `None` when it cannot build the stored profile again.

### Engine states

| Engine | What the guard does | Left to the session |
| --- | --- | --- |
| `current_refused()` on | Counts it as no picture. After 1 s: reverts a pending change, or goes to the safe default | Muting the audio and showing no picture as confirmed |
| `blanked()` on | The same | The same |
| Failed blanks (`current_refused()` stays on) | The same; the new inputs and `retry` give a new set of blank attempts | |
| No mode set (no winner at launch, a blocked source) | The same | |
| `retry()` | Calls it on every change it makes | Calls it when the player asks, for example when the quick menu closes |
| A flip that never completes (`PendingFlip`, OP-062 point 1) | Nothing: the mode on the wire is valid. `muted()` stays on after a modeset, so a confirmation is refused and the deadline reverts. The revert waits for the flip like any modeset | Opening the display again after a number of `PendingFlip` errors |

### What the app must do

1. At boot, before the first frame: load the record (a record that cannot be read is `GuardRecord::default()`), build `wanted` from the settings and `safe` for the detected output, call `Guard::boot`, and write the record it returns before the first modeset.
2. Write every `GuardStep::save` with temp file, fsync, rename, off the frame thread.
3. Call `tick` once per frame, after `present`, also when it failed. The time is the clock of the frame loop.
4. Send every display change the user asks for (monitor profile, picture style, refresh policy, also through the control protocol) to `propose`, and every settings change that needs no confirmation (the resolved settings of a game) to `follow_settings`. Never call `set_monitor` or `set_settings` of the engine directly.
5. `PromptOpened`: put the confirm dialog in the snapshot with `remaining_ns` from `Prompt::deadline_ns`, map `PromptId` to the UI's `ConfirmId`, stop every key repeat and send nothing for a key held now (OP-048).
6. `KeepMode`: call `confirm` with the open id and the frame time. A `ConfirmError` keeps the dialog; a `KeepMode` after the deadline is dropped. `RevertMode`: call `decline`.
7. `Confirmed`: write the changed settings now. The settings files hold a display change only once it is confirmed, so a crash during a pending change leaves the old value.
8. `Reverted` and `SafeMode`: close the dialog and show a notice. Write no setting.
9. `Hotkey::BlindRecovery`: call `recover`, in every screen and state, also while a dialog is open. On `SafeMode(Recovery)` play the audio cue.
10. Show safe mode in the display panel while `phase()` is `Safe`.

## Must never depend on

- `neptron-kms` (it uses the `Display` trait only)
- `neptron-ui`, `neptron-audio`
- Any hardware crate

## Rules

- `unsafe` is forbidden: the crate root carries `#![forbid(unsafe_code)]`.
- No features.

## Tests

- Unit tests: the cache, the scene builder, the monitor caps, the settings mapping.
- Unit test of `Odren`: an order from older inputs is not authorized.
- `tests/integration/engine.rs`: launch, steady state, flicker, debounce, scale only, mute order, double refresh, the queued flip before a modeset, the trace, merged scale-only records, a turned game, the Display panel rows.
- `tests/integration/faults.rs`: a veto that refuses, no winner, each `DisplayError` of `set_mode`, `present` and `wait_flip`, a read back that differs, a restore validated again, the read back of a restore, a failed restore and its blank, `retry()`, the read-back rate note, calls out of order.
- `tests/integration/inputs.rs`: a monitor change and a settings change in a game, a stale token after an input change, an unapplied or failed monitor modeset, the attempt bound, `current_refused()`, the blank when no valid mode exists and the modeset after it, a failed blank and its bound, a blank after a queued flip, the EDID pixel clock limit.
- `tests/integration/stall.rs`: waits that time out with the flip still queued (`FlipFault::Slow` and `FlipFault::Lost` of the fake display).
- `tests/integration/guard_confirm.rs`: timeout revert, confirmation, refused confirmations, decline, a new change while one is pending, the confirmed configuration chosen again, a settings change during a pending change, a blank during a pending change, a revert target the inputs refuse, device errors during a revert.
- `tests/integration/guard_boot.rs`: a boot on the last known good, a good boot, two failed boots, safe mode until a confirmation, a revert in safe mode, settings that differ from the last known good, a first boot with nothing confirmed, a last known good with no valid mode, settings that follow.
- The guard rig checks after every frame that a mode on the wire passes the validator for the current monitor, unless the engine reports it as refused.
- `tests/integration/check5.rs`: timing check 5. Two scripted cores (SNES under a fixed super width, Mega Drive under the native width) change width, lines, scan and rate at given frames. The list of switches must equal `tests/golden/switch_trace.txt`, and every frame checks the mute and the debounce. The golden holds the modes the policy picks on `generic_15`; a policy change shows as a diff to check line by line.
