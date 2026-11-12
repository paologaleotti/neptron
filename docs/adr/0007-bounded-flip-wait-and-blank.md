# 0007: The flip wait is bounded, and a display can turn its output off

- Status: accepted
- Date: 2026-11-12
- Task: C07

## Context

`Display::wait_flip` says that it blocks until the next flip completes. The KMS backend (`neptron-kms`) waits at most `FLIP_WAIT_LIMIT_MS` (500 ms), so that the frame thread never hangs on a flip. On a timeout it returns `DisplayError::Device` and keeps the flip pending: the event can still come, and until it comes `present` and `set_mode` return `Device` as busy, because the kernel answers `EBUSY`. A failed event read also keeps the flip pending. The contract does not say this (OP-059).

The fake display of `neptron-testkit` does something else. `FlipFault::Lost` and `FlipFault::WaitFails` drop the pending flip, so the next `present` works. Code that passes on the fake can then fail on KMS.

The engine (`neptron-video`) keeps a flip queued after a failed wait, and counts it as lost after `FLIP_WAIT_ATTEMPTS` (2) failed waits in a row. After that it waits no more. On KMS the flip is still pending, so every later `present` and `set_mode` is busy, and the engine never waits again: the output stays frozen even when the event comes on the third wait.

When no valid mode exists for the inputs, the engine reports `current_refused()` but cannot turn the output off, because the contract has no call for it (OP-061, point 2). A mode that the inputs refuse then stays on the wire. The Definition calls no signal safe for every display.

## Decision

1. The `wait_flip` doc states the bounded wait: "Blocks until the next flip completes, but no longer than a limit of the backend. When the limit passes first, it returns `DisplayError::Device` and the flip stays queued: it can still complete, a later `wait_flip` returns its event, and until then `present`, `set_mode` and `blank` return `DisplayError::Device` as busy." Any other error of the wait also keeps the flip queued. A backend never drops a queued flip.
2. The fake display follows it:
   - `FlipFault::Lost`: the flip never lands. Each wait moves the simulated clock by the flip timeout and returns `Device`. The flip stays queued, so `present` and `set_mode` stay busy.
   - `FlipFault::Slow { timeouts }` (new): the first `timeouts` waits time out as for `Lost`. The next wait returns the event. The flip lands on its target blank, or on the first blank where a flip can land after the last timeout, whichever is later.
   - `FlipFault::WaitFails`: the first wait fails at once with `Device`. The flip stays queued, and the next wait returns its event.
3. The engine keeps a flip queued until a wait returns its event. `FLIP_WAIT_ATTEMPTS` goes. While the flip is queued, each `present` waits for it first; when that wait fails, the present sets and queues nothing and returns `PendingFlip`.
4. The contract gets `Display::blank(&mut self) -> Result<(), DisplayError>`. It turns the output off (no sync, no picture) until the next `set_mode`. It is the one call that changes the display without a `ValidatedMode`, and it carries no mode.
   - After a blank, `present` and `wait_flip` return `NoMode`, as before the first `set_mode`. No mode is read back.
   - With a flip queued, `blank` returns `Device` as busy, as `set_mode` does. The caller waits for the flip first. This is the simplest rule: one busy rule for every call that changes the output.
   - The next `set_mode` is always a real modeset with its read back, also for the mode that was set before the blank.
   - A blank of an output that is off, or that never had a mode, turns it off again and succeeds. A failed blank changes nothing.
5. The KMS backend blanks with one atomic commit, test-only first, with `ALLOW_MODESET`: the connector and the writeback connector detached (`CRTC_ID` 0), the CRTC with `MODE_ID` 0 and `ACTIVE` 0, the plane with `FB_ID` 0 and `CRTC_ID` 0. The drop restore uses the same request when the driver refuses the plane-off commit. `MODE_ID` is only ever a blob from `ModeRecord::from_validated`, or 0. After a blank, `WireMode` has no mode on the wire, so the next `set_mode` commits, and drop makes no commit.
6. The engine blanks when no valid mode exists for the inputs:
   - The mode on the display is refused (`current_refused()`), and the decision ends with no winner: the frame gets `Directive::Blank`, and the next `present` calls `blank()`.
   - The driver reads back a mode the engine refuses, and no restore follows (there is no last good mode, or the inputs now refuse it), or the restore fails or its read back does not hold: the same `present` calls `blank()`, so the refused mode does not stay on the wire.
   - A blank that succeeds forgets the mode, turns `current_refused()` off and `blanked()` on, and adds an `Outcome::Blanked` record to the trace. A failed blank adds `Outcome::BlankFailed` and is tried again on the next frame, up to `MODESET_ATTEMPTS` in all, then `current_refused()` stays on for the guard. An input change or `retry()` starts a new count.
   - The next valid decision sets a mode again with a normal modeset, and `blanked()` turns off.

## Consequences

- Code that passes on the fake now meets the same busy rule as on KMS after a timed-out or failed wait.
- A flip that never completes stops the output: every frame waits once (500 ms on KMS) and returns `PendingFlip`. The session and the guard see the errors; the engine does not hide them by guessing that the flip is gone.
- Every `Display` implementation adds `blank`: `KmsDisplay`, `FakeDisplay`, `WindowDisplay` and the test displays. The window shows black and goes back to its state with no mode.
- The guard (T26) and the session (T25) read `blanked()`, `current_refused()` and the trace. A blank is not a mode change of the guard's inputs: the guard still reverts to the last confirmed inputs.
- The fake records each `blank` call (`blank_calls()`), so tests assert it.
- A blank on `vkms` is checked by `kms-check`. A blank on a real GPU and a real CRT is a hardware check.

## Alternatives

- A blank that waits for a queued flip within the bound: rejected. It adds a second wait path with its own timeout to a safety-critical call; the busy rule is already the rule of `set_mode`, and the engine already waits first.
- Keep `FLIP_WAIT_ATTEMPTS` and let the engine count a flip as lost: rejected. With the bounded contract a display never drops a flip, so the engine and the display disagree and the output freezes.
- A blank through a "null" `ValidatedMode`: rejected. A `ValidatedMode` means a mode that passed the validator; a blank carries no mode.
- Turn the output off only at drop: rejected. The engine needs it while it runs, when the inputs refuse every mode.
