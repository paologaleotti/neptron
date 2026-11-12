# 0010: The output kind picks the GPU limits

- Status: proposed
- Date: 2026-11-12
- Task: C08

## Context

The capability table holds one set of GPU limits per entry: 165 MHz and 1920x1200 for every Intel, AMD and NVIDIA entry. These are careful values for the analog path to a CRT. A flat panel on HDMI or DisplayPort got the same values, so a 1440p or 4K panel ran at 1080p and the panel scaled the picture. The owner's rule: the CRT limits are for an analog output; when the display is surely a flat panel on a digital link, those limits make no sense, and the software and the type system must tell digital from analog.

The detection already has a class rule (`display_class` in `neptron-platform`): an analog EDID, a `VGA` or `DVI-A` subconnector or an analog connector is a CRT; a digital EDID on a digital connector is a flat panel. A DAC on a DisplayPort connector, the bench of the owner, is a CRT by that rule.

## Decision

- `neptron-types` gets `OutputKind { Analog, Digital }`. `Analog` is the default: with no certainty, an output is analog.
- The class rule gives the output kind once at start: `DisplayClass::Crt` is `Analog`, `DisplayClass::FlatPanel` is `Digital`. No EDID, a broken EDID and a name with no connector are `Analog`.
- The capability table gets optional digital limits per connector type (`[[gpu.digital]]`: highest pixel clock, largest mode), an append-only addition to version 1. A digital limit never names an analog connector type.
- `Matrix::caps_for_output(gpu, connector_kind, OutputKind)` gives the caps: for `Analog`, the entry's values, exactly as `caps_for`; for `Digital`, the entry's values with its digital limits for that connector, when it has them.
- The app asks for `Digital` caps only when the output is digital and `display.monitor` gives that panel (`auto` or `flat`). A CRT preset on a digital output, and `flat` on an analog output, get the analog caps.
- `validate` does not change. A flat-panel mode must still be one of the panel's own listed EDID modes, field for field, inside the GPU's pixel clock range; only the caps it gets differ. The kernel's test-only commit is the last word on what the link takes, and the engine tries the next listed mode when the driver refuses one.

## Consequences

- No CRT limit changes: every analog caps value is the one of `caps_for`, and every golden of the CRT path stays.
- A 4K DisplayPort panel on an HD 530 runs at its native 3840x2160; on HDMI 1.4 (300 MHz) it runs at 1080p60.
- Only `intel_gen9` has digital limits today, from i915 and Intel ARK. Every other GPU gives a digital output its analog values until a source gives its limits.
- The kernel does not report a connector's link limit as a property; it prunes the connector's mode list. The DRM framebuffer limit (`mode_config` max width and height) is larger than the scanout of a plane on gen9, so the table values are used, and the test-only commit catches the rest.
- Before the display opens the subconnector is not known, so a DisplayPort connector with a digital EDID opens with the digital caps and buffers. Right after the open the app reads the class of the driven connector again, with its subconnector; when the output is analog (a `VGA` subconnector, or a digital adapter with no EDID), it lowers the caps that the policy and the validator get to the analog ones (`KmsDisplay::lower_caps`). So no CRT path runs with the digital limits; only the buffers stay larger.

## Alternatives

- A flat-panel special case in the app with no type: the same values, but nothing in the types says why a limit applies, and the next reader can apply the panel limits to a CRT by mistake.
- An output kind field in `GpuCaps` or a second caps set in the validator: a change of the contract type and of `validate` for no gain, because the validator already treats a panel profile as a list of exact modes.
- Raising the table limits for every connector: it would change the CRT path, which the owner forbids.
