# neptron-video

The ODREN engine: core geometry to policy to safety to display. Debounce, commit, mute and the switch trace. The `guard` module (task T26) will hold confirm-or-revert, last known good and safe mode.

## Public API

- `Odren`: the state machine. It does no I/O and holds no display. `on_frame(SourceGeometry) -> Directive` takes the source of one core frame. `set_monitor`, `set_settings` and `set_veto` change its inputs. `current()`, `trace()`, `muted()`, `style_rows(&SourceGeometry)` read it.
- `VideoEngine<D: Display>`: `Odren` on a display. `new(display, Monitor, PolicySettings)`, `begin_frame(SourceGeometry) -> Directive`, `present(game, ui) -> Result<Presented, VideoError>`, `wait_flip() -> Result<FlipEvent, VideoError>`, `muted()`, `current()`, `trace()`, `style_rows`, `set_monitor`, `set_settings`, `set_veto`, `display()`.
- `Monitor`: `Profile(MonitorProfile)` or `Edid(EdidProfile)`. `caps_for(&GpuCaps)` applies the EDID pixel clock limit (`EdidProfile::limit_caps`).
- `policy_settings(style, refresh_policy, horizontal) -> Result<PolicySettings, SettingsError>`: from the values of `display.style` and `display.refresh_policy`.
- `build_scene(&Placement, game, ui) -> Scene`: the one function that builds a `Scene`.
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

## The state machine

Each core frame gives one `Directive`:

| Directive | When | What the display does |
| --- | --- | --- |
| `Steady` | The source equals the source on screen | One flip with the stored placement |
| `Rescale` | The winner equals the mode on screen (step 7) | One flip with the new crop, destination and filter. No debounce, no mute |
| `Waiting` | A new source needs a new mode, and it has been the same for fewer than `DEBOUNCE_FRAMES` (3) core frames | One flip in the old mode; the frame is shown whole in the old destination |
| `Modeset` | The new source held for 3 frames, or at launch, or after a monitor, settings or driver retry (no debounce for these) | Mute; `set_mode` with the winner's token; one flip; unmute at that flip |
| `Kept` | The source is turned, the policy gives no plan, or every proposal is refused | The mode on screen stays; the trace says why. Before the first mode, `present` returns `NoMode` |

Rules:

- The engine sets `token.mode()` of the winner, the `ValidatedMode` that `neptron_safety::validate` returned. `proposal.mode` never reaches the display.
- The validator is `neptron_safety::validate` with the monitor's caps, after an optional veto that can only refuse more (`set_veto`). The same check runs for the Display panel rows (`style_rows`), so the preview and the run give one answer.
- No winner: the mode on screen stays, and the trace records `Kept` with every refusal. The engine never guesses a mode.
- A turned game (`Rotation` other than `R0`) is `Kept(Rotated)`: `Scene` has no rotation in the MVP.
- Decisions are cached by source (`CACHE_ENTRIES`, 8). A monitor, settings or veto change clears the cache. The steady state does no decision and no allocation.

## Display errors

| Error | What the engine does |
| --- | --- |
| `set_mode` returns `ModeRefused` | The mode on screen stays (the `Display` contract). The engine stores the mode as refused by the driver, and the next frame tries the next proposal with no debounce (`Trigger::Retry`). A monitor change forgets the refused modes |
| `set_mode` returns `Device` or another error | The mode on screen stays. The engine does not try the same source again until the source on screen comes back, or the monitor or the settings change. No retry storm |
| `set_mode` reads back a mode of another size or scan, or one the validator refuses | The engine stores it as refused by the driver and sets the last good mode again. If that fails, `present` returns `RestoreFailed`, the engine forgets its mode, and the next frame decides again |
| A flip is queued before a modeset | The engine calls `wait_flip` first, because the display refuses `set_mode` while a flip is pending (OP-032). `Presented::drained_flip` gives the event |
| That wait fails | `present` returns `FlipBeforeModeset`; the modeset happens on the next frame |
| `present` fails | `VideoError::Present`; no flip is queued |
| `wait_flip` fails | `VideoError::WaitFlip`; the flip is lost. The mute stays until a flip in the new mode completes |

## Seam for `guard` (T26)

The guard reverts by giving the engine its old inputs again: `set_monitor` or `set_settings` with the last confirmed values decide again on the next frame, with no debounce. It reads `current()` and `trace()` to see which mode is on screen and why. Blind recovery sets the safe monitor the same way.

## Must never depend on

- `neptron-kms` (it uses the `Display` trait only)
- `neptron-ui`, `neptron-audio`
- Any hardware crate

## Rules

- `unsafe` is forbidden: the crate root carries `#![forbid(unsafe_code)]`.
- No features.

## Tests

- Unit tests: the cache, the scene builder, the monitor caps, the settings mapping.
- `tests/integration/engine.rs`: launch, steady state, flicker, debounce, scale only, mute order, double refresh, the queued flip before a modeset, the trace, a turned game, the Display panel rows.
- `tests/integration/faults.rs`: a veto that refuses, no winner, each `DisplayError` of `set_mode`, `present` and `wait_flip`, a read back that differs, a failed restore, calls out of order.
- `tests/integration/inputs.rs`: a monitor change and a settings change in a game, the EDID pixel clock limit.
- `tests/integration/check5.rs`: timing check 5. Two scripted cores (SNES under a fixed super width, Mega Drive under the native width) change width, lines, scan and rate at given frames. The list of switches must equal `tests/golden/switch_trace.txt`, and every frame checks the mute and the debounce. The golden holds the modes the policy picks on `generic_15`; a policy change shows as a diff to check line by line.
