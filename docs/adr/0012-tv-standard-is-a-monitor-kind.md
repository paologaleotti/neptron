# 0012: A TV standard is a monitor kind with a fixed mode table

- Status: proposed
- Date: 2026-11-13
- Task: P02

## Context

The composite encoder of the Raspberry Pi 3 (the VEC of `vc4`) has a fixed pixel clock of 13.5 MHz, a fixed horizontal total per line system (858 for 525 lines, 864 for 625 lines), and ten standards. Each standard is a value of the connector property `TV mode` together with a line system. `docs/spec/raspberry-pi.md`, section 3, describes the design. Switchres cannot calculate for this encoder, and the policy gives no mode for `pi_vc4_composite` today (OP-047).

The video model has two monitor kinds: a CRT with ranges, and a flat panel with listed modes. Neither carries a standard, and no contract type can carry one to the backend.

## Decision

1. `neptron-types` gets `TvStandard`: `Ntsc`, `NtscJ`, `Ntsc443`, `Pal60`, `PalM`, `Pal`, `PalN`, `Secam`, `Mono60`, `Mono50`. Each standard has `lines()` (525 or 625), `kms_name()` (the `TV mode` value), `key()` (the settings and file value) and `modes()` (its progressive and its interlaced mode). `TvStandard::ALL` lists the ten. The wire names are the keys: `ntsc`, `ntsc_j`, `ntsc_443`, `pal_60`, `pal_m`, `pal`, `pal_n`, `secam`, `mono_60`, `mono_50`.
2. Four constants hold the table: `TV_525_240P`, `TV_525_480I`, `TV_625_288P` and `TV_625_576I`, with `TV_PCLOCK_HZ` (13 500 000). The values are in "The four modes" below.
3. `MonitorKind::Tv { standard }` is a third variant. In TOML it is `[kind.tv]` with `standard = "ntsc"`. This is an addition to the profile format: no field is renamed or removed.
4. A TV profile is built in, not a file in `data/monitors`. `TvStandard::profile()` gives `MonitorProfile { id: "tv_<key>", kind: Tv { standard } }`, for example `tv_ntsc`, and `TvStandard::from_profile_id` reads the id back. The id carries the standard because the guard record stores a monitor by its id only (`MonitorRef` in `apps/neptron`), and the boot builds the profile again from it. `data/monitors` stays the Switchres presets: it is GPL-2.0-or-later as a Switchres derivative, every reader of it expects CRT ranges, and the safety build script makes the hard rails from it. The setting `display.tv` (a later task) names a standard by its `key()`, plus `auto`.
5. `neptron_safety::validate` gets a TV arm. The structure check and the GPU check run first, as for every kind. Then the mode must equal one of `standard.modes()` field for field, or `validate` returns the new `Rejection::NotATvMode { mode, standard }`. Then the mode must pass the hard rails of its line-rate class, as a second opinion against a mistake in the table. This is stricter than the CRT path, never wider.
6. `ValidatedMode` gets a private field `tv: Option<TvStandard>` and the reader `tv_standard()`. `validate` sets it from a TV profile and to `None` for every other kind. Nothing else can set it, so the standard on the wire is the standard that was validated with the mode.
7. The `Display::set_mode` contract of `neptron-backend` gets one rule. For a mode with a TV standard, a backend that drives a connector sets the standard in the same atomic commit as the mode. A backend that cannot set it (no `TV mode` property, or no value of that name) refuses with `DisplayError::ModeRefused` and changes nothing. A request whose mode and standard both equal what is on the wire changes nothing; a change of either one is a modeset. A window backend has no signal and ignores the standard.
8. `FakeDisplay` follows the KMS rule. `DisplayOptions::tv_standards` lists the `TV mode` values of the fake connector; the default is none, as on HDMI or vkms. It refuses a mode with a standard that is not in the list, and a mode that is not a table mode of its standard. It records the standard of each call.

## The four modes

All four have a pixel clock of 13 500 kHz, negative sync pulses and no doublescan. 13 500 000 Hz is 13 500 kHz exactly, so ADR 0009 holds with no rounding.

| Mode | h: active, sync start, sync end, total | v: active, sync start, sync end, total | Field rate, mHz | Source |
| --- | --- | --- | --- | --- |
| 525, 240p | 720 734 798 858 | 240 243 246 262 | 60 054 | `drm_mode_240p` in `drivers/gpu/drm/vc4/vc4_vec.c` |
| 625, 288p | 720 740 804 864 | 288 290 293 312 | 50 080 | `drm_mode_288p` in the same file |
| 525, 480i | 720 736 800 858 | 480 486 492 525 | 59 940 | `drm_analog_tv_mode()` in `drivers/gpu/drm/drm_modes.c`, for NTSC, 13.5 MHz, 720x480, interlaced |
| 625, 576i | 720 732 796 864 | 576 581 587 625 | 50 000 | `drm_analog_tv_mode()`, for PAL, 13.5 MHz, 720x576, interlaced |

The source text is the Raspberry Pi fork, branch `rpi-6.18.y`, commit `ea17ae02ca28`, read on 2026-11-13. `drm_modes.c` of mainline `v6.18` has the same `fill_analog_mode()` and the same parameter table.

How `drm_analog_tv_mode()` gives the 480i and 576i values:

- The pixel time is `1 000 000 000 / 13 500 000` = 74 ns in integer division. The line total is `line_duration_ns * 13 500 000 / 10^9`: 63 556 ns gives 858, 64 000 ns gives 864.
- 720 active samples are more than the longest active time allows (714 at 525, 707 at 625), so the function takes the BT.601 path: the front porch is `bt601_hfp`, 16 at 525 and 12 at 625.
- The sync is `DIV_ROUND_UP(4 700, 74)` = 64 samples. The back porch is the rest: 858 - 720 - 16 - 64 = 58, and 864 - 720 - 12 - 64 = 68.
- Vertically, an interlaced mode adds the values of both fields. At 525 lines: front porch 3 + 3 = 6, sync 3 + 3 = 6, back porch at least 16 + 17 = 33. The 39 blank lines leave 0 to share, so the porches are 6 and 33. At 625 lines: front porch 3 + 2 = 5, sync 3 + 3 = 6, back porch at least 19 + 20 = 39. The 43 blank lines leave -1; C integer division makes `-1 / 2` zero, so the front porch stays 5 and the back porch is 38.
- The kernel's own tests (`drm_test_modes_analog_tv_ntsc_480i` and `drm_test_modes_analog_tv_pal_576i` in `drivers/gpu/drm/tests/drm_modes_test.c`) pin the sync start (736 and 732) and the totals.

The VEC check (`vc4_vec_encoder_atomic_check`) uses the per-field values, which DRM halves for an interlaced mode. 480i gives 240, 243, 246, 262, the same field as 240p; 576i gives 288, 290, 293, 312, the same field as 288p. Each passes: 3 lines of sync after at least 1 line of front porch, at least 4 (525) or 2 (625) lines after the sync, and a field total of at most 262 or 312.

The field rates are exact fractions, rounded to the nearest millihertz by `Modeline::field_rate_mhz`:

- 240p: 13 500 000 000 / (858 x 262) = 13 500 000 000 / 224 796 = 60 054.45, so 60 054 mHz.
- 288p: 13 500 000 000 / (864 x 312) = 13 500 000 000 / 269 568 = 50 080.13, so 50 080 mHz.
- 480i: 2 x 13 500 000 000 / (858 x 525) = 27 000 000 000 / 450 450 = 59 940.06, so 59 940 mHz.
- 576i: 2 x 13 500 000 000 / (864 x 625) = 27 000 000 000 / 540 000 = 50 000 mHz exactly.

The line rates are 15 734 266 mHz (525) and 15 625 000 mHz (625).

The sync polarity is negative on both axes. `drm_analog_tv_mode()` sets `NHSYNC` and `NVSYNC`; `drm_mode_240p` and `drm_mode_288p` set no polarity flag, and the VEC does not read the flags.

## What does not change

- `OutputKind` (ADR 0010). Composite stays `Analog`: the GPU limits of `pi_vc4_composite` apply, and the output kind picks them as before.
- The guard state machine in `neptron-video`. A change of standard is a change of monitor: confirm or revert, last known good and blind recovery work on it as they are.
- ODREN, the policy in `neptron-modeline`. In this decision `decide` returns `EmptyProfile` for a TV profile, as it gives no mode for composite today. The TV planner is a later task.
- The CRT and flat-panel arms of `validate`, the hard rails and their values, and the presets in `data/monitors`.

## Consequences

- A TV profile can reach the wire only as one of four broadcast modes, with the standard it was validated with.
- Every exhaustive `match` on `MonitorKind` gets an arm. A TV is a 15 kHz CRT for the screen class and the display class, never a flat panel, and the Switchres solver refuses it as a profile with no CRT range.
- The KMS backend does not set `TV mode` yet. Until it does, a `ValidatedMode` with a standard must not reach it; no code makes a TV profile before the app task, which comes after the KMS task.
- `neptron-safety` grows by about 40 lines.

## Unverified

- The four modes on a Pi: the 480i and 576i values are what the kernel source generates, not what `modetest` prints on a Pi 3. The hardware checklist line compares them.
- What the TV shows for each standard, and the flip cadence of 480i and 576i on `vc4` (OP-030).

## Alternatives

- A TV as an output kind: rejected. `OutputKind` picks the GPU limits; the set of modes a display takes belongs to the monitor profile, and the guard already works on the monitor.
- A preset file per standard in `data/monitors`: rejected. The directory is the Switchres presets under their own licence, its readers expect CRT ranges, and a file would let a user edit a broadcast table that the encoder fixes.
- A TV as a flat panel with four listed modes: rejected. A panel profile skips the hard rails and carries no standard, so the standard could not reach the backend with the validated mode.
- One profile id `tv` for every standard: rejected. The guard record stores the id only, and the boot could not tell which standard was confirmed.
