# neptron-video

The ODREN engine: core geometry to policy to safety to display. Debounce, commit, mute and the switch trace. The `guard` module (task T26) will hold confirm-or-revert, last known good and safe mode.

## Public API

- `Odren`: the state machine. It does no I/O and holds no display. `on_frame(SourceGeometry) -> Directive` takes the source of one core frame. `set_monitor`, `set_settings` and `set_veto` change its inputs; `set_scanlines` changes the scene only; `retry()` lifts a blocked source. `scene(game, ui)` and `preview_scene(&Proposal, &SourceGeometry, game, ui)` build the scene of the mode on screen and of a Display panel row through the same function. `current()`, `current_refused()`, `blanked()`, `trace()`, `muted()`, `style_rows(&SourceGeometry)` read it.
- `VideoEngine<D: Display>`: `Odren` on a display. `new(display, Monitor, PolicySettings)`, `begin_frame(SourceGeometry) -> Directive`, `present(game, ui) -> Result<Presented, VideoError>`, `wait_flip() -> Result<FlipEvent, VideoError>`, `muted()`, `current()`, `current_refused()`, `blanked()`, `trace()`, `style_rows`, `set_monitor`, `set_settings`, `set_veto`, `set_scanlines`, `preview_scene`, `retry()`, `display()`.
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
| `set_mode` reads back a mode of another size or scan, or one the validator refuses | The engine stores the mode as refused by the driver, validates the last good mode again with the current inputs and restores it only with that new token. The read back of the restore must hold like a modeset. When the last good mode is refused, the engine sets nothing more, forgets its mode, sets `current_refused()`, and the next frame decides again. When the restore fails or reads back wrong, the same present also turns the output off with `blank`; a failed `set_mode` there gives `RestoreFailed` |
| `blank` fails | The trace records `BlankFailed`, and `current_refused()` stays on. The next frame tries again, up to `MODESET_ATTEMPTS` (3) attempts in all; an input change starts a new count |
| A flip is queued before a present or a modeset | The engine waits for it first, because the display refuses `set_mode` and `present` while a flip is pending (OP-032). `Presented::drained_flip` gives the event |
| A wait fails (`wait_flip` or that drain) | The flip stays queued until a wait returns its event, because the `Display` contract bounds the wait and never drops a queued flip. While it is queued, `present` sets and queues nothing and returns `PendingFlip`; a modeset happens on a later frame. A flip that never completes gives `PendingFlip` on every frame. The mute stays until a flip in the new mode completes |
| `present` fails | `VideoError::Present`; no flip is queued |
| The read back has another pixel clock | Kept when it has the same size and scan and passes the validator. When its field rate differs by `READ_BACK_RATE_NOTICE_MHZ` (5 mHz, the policy's two-decimal match) or more, the modeset record holds `rate_drift_mhz` |

## A mode the inputs refuse

The engine detects the case, turns the output off when no valid mode exists, and exposes both states:

- `set_monitor`, `set_settings` and `set_veto` judge the mode on the display with the new inputs. A refusal is recorded as `Kept(CurrentRefused)` with the reason, and `current_refused()` turns on.
- A failed or refused restore leaves a mode the engine does not know on the display, with no current mode. `current_refused()` turns on too.
- When the next decision gives no mode to set (`Kept`, or `Steady` on a blocked source), the frame gets `Directive::Blank`, and its `present` calls `Display::blank` once. A failed restore blanks in the same present. `blank` carries no mode: it is the one display change without a `ValidatedMode`.
- A blank that succeeds forgets the mode, records `Blanked`, turns `current_refused()` off and `blanked()` on. `present` returns `NoMode` until a mode is set. A later valid decision (a new source, an input change, `retry()`) sets a mode with a normal modeset, which is a real commit on every backend, and `blanked()` turns off.
- `current_refused()` stays on until a valid mode is on screen or the output is off: a modeset or restore whose read back holds, a scale-only change to a winner the inputs accept, or a blank. It stays on after `MODESET_ATTEMPTS` failed blanks.

What the callers must do:

- T26 (`guard`) reads `current_refused()` and `blanked()` after each input change and each frame. When either is on and no modeset follows, it reverts to the last confirmed inputs, or to safe mode, with `set_monitor` and `set_settings`. `current_refused()` on after failed blanks means a refused mode is still on the wire.
- T25 (session) shows no game picture as confirmed while either is on, keeps the audio muted, and passes the state to the guard and to the control protocol.

## Seam for `guard` (T26)

The guard reverts by giving the engine its old inputs again: `set_monitor` or `set_settings` with the last confirmed values decide again on the next frame, with no debounce. It reads `current()`, `current_refused()`, `blanked()` and `trace()` to see which mode is on screen, or that the output is off, and why. Blind recovery sets the safe monitor the same way.

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
- `tests/integration/check5.rs`: timing check 5. Two scripted cores (SNES under a fixed super width, Mega Drive under the native width) change width, lines, scan and rate at given frames. The list of switches must equal `tests/golden/switch_trace.txt`, and every frame checks the mute and the debounce. The golden holds the modes the policy picks on `generic_15`; a policy change shows as a diff to check line by line.
