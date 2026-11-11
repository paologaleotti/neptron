# 0001: Corrections to the contracts block when it became code

- Status: accepted
- Date: 2026-11-11
- Task: T02

## Context

The contracts block in the MVP taskboard is a sketch that was never compiled. When it became code in `neptron-types`, `neptron-safety` and `neptron-backend`, some parts needed a decision that changes what a contract means. Other corrections are mechanical (derives, field types of the rejection payloads, doc comments) and are listed in the T02 task report only.

## Decision

1. `Rejection` gets a tenth variant, `Malformed { field, value, limit }`. It reports a modeline whose timings are out of order or zero (for example `hsync_start` below `hactive`, or `htotal` 0). The validator can then refuse such a mode before any subtraction or division, and the other variants keep their meaning.
2. `FlipEvent`, `InputEvent` and a new `Control` enum live in `neptron-types`, not in `neptron-backend`. They are plain data with serde, because the control protocol sends them as JSON. `neptron-backend` keeps the traits and `DisplayError`.
3. `InputEvent` is `{ device: u32, control: Control, value: i16 }`. `Control` is the set of RetroPad buttons and analog axes, and `value` uses the libretro ranges. An `InputSource` gives events after the mapping to the RetroPad.
4. `Modeline::line_rate_mhz` and `field_rate_mhz` return 0 for a modeline with `htotal` or `vtotal` 0, so that they cannot panic. They round to the nearest millihertz, with a half rounded up. The field rate follows the DRM refresh rule: times 2 for interlace, divided by 2 for doublescan.
5. `Rotation` counts steps of 90 degrees counter-clockwise, as libretro does.
6. Every serialised enum uses snake_case names. `MonitorKind` is externally tagged: the variant is a key, `{"crt":{"ranges":[...]}}` in JSON and `[[kind.crt.ranges]]` in TOML. The T04 build script, the EDID profiles and the control protocol depend on this shape.

## Consequences

- `validate` reports a broken modeline with a value and a limit, like every other check.
- `neptron-ipc` can serialise input and flip events with no dependency on the backend traits.
- A device that is not a RetroPad (a mouse, a light gun) needs new `Control` variants or a new event type later. That is an addition, not a change.
- A rate of 0 is never in range, so the validator refuses a modeline with a zero total.

## Alternatives

- Report a zero or out-of-order timing as `Porch` or `LineCount`: rejected, because the value and the limit of those variants would not describe the fault.
- Keep the events in `neptron-backend` and add serde there: rejected, because the control protocol would then depend on the backend crate for two data types.
- Raw device codes in `InputEvent`: rejected, because every consumer would then repeat the mapping.
- `Option<u64>` from the rate functions: rejected, because the block gives `u64`, and 0 is out of range for every check.
