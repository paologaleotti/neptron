# 0009: A mode for a display has a pixel clock in whole kHz

- Status: proposed
- Date: 2026-11-12
- Task: C09

## Context

The kernel's mode record (`struct drm_mode_modeinfo`) holds the pixel clock in kHz. The KMS backend (`ModeRecord::from_validated` in `neptron-kms`) refuses a clock that is not a whole number of kHz, and it never rounds: a backend decides nothing, and a rounded mode is not the mode that passed validation.

Switchres gives the clock in whole hertz, for example 25 466 400 Hz. The policy (`neptron-modeline`) passed it on. On real KMS every proposal was then refused with "the pixel clock ... Hz is not a whole number of kHz", and the frontend showed no picture. The hardware self-test on vkms found it. No test found it before: `FakeDisplay` accepted any clock, and the `kms-check` example uses modes written by hand.

The `Display::set_mode` contract did not state the rule.

A step of 1 kHz moves the field rate by `1000 / (htotal * vtotal)` Hz. For a 15 kHz mode of 256 to 320 pixels that is 9 to 12 mHz, more than `REFRESH_MATCH_MHZ` (5 mHz), the bound under which a rate matches the game's rate. Rounding alone also moves a mode that Switchres puts at the edge of a range over the edge: Neo Geo on `generic_15` sits at 15.625 kHz, and a rate of 65.000 Hz sits at `v_max`. With the nearest kHz and nothing more, 14 of 180 policy goldens lost their rung or their mode, and 5 of them got no mode at all.

## Decision

1. The contract: the pixel clock of a mode for `Display::set_mode` is a whole number of kHz. A backend refuses any other mode with `DisplayError::ModeRefused`, changes nothing, and never rounds. The `set_mode` doc states this.
2. One pure check holds the rule: `Modeline::pclock_is_whole_khz` and `Modeline::PCLOCK_STEP_HZ` (1 000) in `neptron-types`. The KMS backend and the fake display use it.
3. `FakeDisplay` refuses such a mode the same way, after its busy check, so code that passes on the fake also passes on KMS. Its README lists the rule.
4. The policy makes every CRT proposal whole kHz before any check of its own and before validation (`whole_khz` in `src/policy/clock.rs`). The rates of the proposal are the rates of the final clock and totals, so the mode that is validated is the mode that is set and read back. To keep the rate, the policy tries Switchres's horizontal total and up to 8 pixels more or less (the back porch takes the change), each with the whole-kHz clock below and above the one that keeps Switchres's line time, and takes the first mode with Switchres's field rate to the millihertz, else the nearest. A mode must keep each rate inside the range where Switchres has it inside, and each horizontal porch and the sync pulse at least 90 % of the nominal time (the share of Switchres) and the sync pulse at most 110 %, or no worse than Switchres's mode. When no mode passes, the nearest kHz at Switchres's total is the proposal, and the validator judges it.
5. `neptron_safety::validate` does not check the step. It judges a mode against the monitor and the GPU limits. The step is a property of the kernel interface, which the backend owns. The safety crate does not change.

## Consequences

- Every policy golden keeps its rung and its mode size. The field rate of 35 of 180 profile goldens and 4 of 160 GPU goldens moves by at most 3 mHz, and stays less than 5 mHz from the game's rate, so no new speed notice fires. The `ntsc` preset (one field rate, 59.940 Hz) still gets its exact mode, through a changed total.
- A changed total makes the active area up to 8 pixel times shorter or longer, about 2.5 % on a 320-pixel 15 kHz mode, and moves the picture by the same amount. The porch and sync guards keep each time within the rules of Switchres. This needs a check on a CRT.
- The engine's read back check gets the requested mode back on KMS, because the requested clock is already whole kHz.
- A test mode written by hand for a display must be whole kHz. A mode with another clock still validates, and the fake refuses it at `set_mode`.
- A `ValidatedMode` can still hold a clock that a display refuses. The engine treats `ModeRefused` as a refusal and walks to the next proposal. A policy mistake would show as a refused mode in the switch trace, not as damage. The property test of the policy checks every proposal.

## Alternatives

- Round in the backend: rejected. The backend decides nothing, and a rounded mode is not the mode that passed validation. A rounding up can cross a range limit that the validator checked on the old clock.
- Change the kernel record to hold hertz: not possible. `drm_mode_modeinfo.clock` is a kHz field of the kernel ABI.
- Round to the nearest kHz in the policy and nothing more: rejected. It drops the exact rate of low-clock modes (Atari 2600 and Game Boy on 15 kHz fell to the fitted rung) and pushes modes at a range edge out of the range (Neo Geo on 15 kHz got no mode).
- Refuse the step in `validate`: not taken. It makes a `ValidatedMode` always settable, but it puts a kernel interface rule in the monitor safety check, it grows the safety-critical crate, and the second-opinion test of the solver goldens (raw Switchres output) would then refuse every golden. The owner can still take it as one named check in `check_gpu`.
- A Switchres setting for the clock step: the vendored calculation sources (`vendor/switchres`) have none. The clock is `htotal * hfreq` in `modeline.cpp`.
