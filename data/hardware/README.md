# Hardware capability table

`matrix.toml` lists, per GPU and output, what NEPTRON may drive: the pixel clock range, interlace, page flips per field, the largest mode, the support tier and the known quirks. `neptron-platform` compiles the file in, checks it, and looks up a detected GPU in it.

## Format

The format is versioned and append-only. `version = 1` is the only version. A new field or a new quirk is an addition. A field is never renamed or removed: a change of meaning needs a new version.

Each `[[gpu]]` table is one entry:

| Field | Type | Meaning |
| --- | --- | --- |
| `id` | string | Stable id: lower-case letters, digits and `_` |
| `name` | string | Name for people |
| `tier` | 1, 2 or 3 | Support tier of the hardware support matrix |
| `driver` | string | Kernel driver, as `DRIVER=` in the device's uevent gives it |
| `pci_vendor`, `pci_devices` | integers | Optional, together: the PCI ids the entry matches |
| `of_compatible` | strings | Optional: device tree compatible strings, any one matches |
| `connectors` | strings | Optional: connector types, such as `DPI` or `Composite` |
| `pclock_min_hz`, `pclock_max_hz` | integers | Pixel clock range, in hertz |
| `interlace` | bool | The output can send an interlaced mode |
| `flip_per_field` | bool | The driver signals a page flip per field |
| `max_width`, `max_height` | integers | Largest mode, in pixels and lines |
| `quirks` | strings | Known limits and fixes; see `Quirk` in `neptron-platform` |
| `[[gpu.digital]]` | tables | Optional, added after version 1: the limits of a digital link to a flat panel. Each has `connectors` (never an analog type), `pclock_max_hz`, `max_width` and `max_height`. They apply only to an output of kind `Digital` (`Matrix::caps_for_output`, ADR 0010); an analog output, also a DAC on the same connector, keeps the values of the entry. The other values of the entry stay |

## Match rules

- An entry matches a GPU when the driver is equal and each optional field it names matches.
- An entry that names PCI ids or compatible strings beats one that names only a driver. At the same level, an entry that names connectors beats one that does not.
- Two entries at the same level that can match the same GPU and connector are an error, so the order of the file never decides.
- A GPU that matches no entry gets the unknown-GPU values, which are constants in the code, not data: 25 MHz lowest pixel clock, no interlace, no flip per field, 165 MHz highest pixel clock, 1920 by 1200.

## Rules for values

- Values come from the hardware support matrix of the Definition.
- Where the matrix says a capability is not proven, the entry leaves it off and names it in a quirk. The parser refuses an entry with `interlace_unproven` or `interlace_needs_kernel_patch` and interlace on, and an entry with `low_pixel_clock_unproven` and a lowest pixel clock below 25 MHz.
- `flip_per_field` is false everywhere: no entry has a proof that the driver signals a flip per field.
- The highest pixel clock (165 MHz) and the largest mode (1920 by 1200) are the unknown-GPU values everywhere except composite. The matrix gives no value for them. They are a conservative default, not a measured limit. They stay the limits of the CRT path on every connector.
- Digital limits are given only where a source states them. A GPU with none gives a digital output its entry's values, so a 1440p or 4K panel there runs in its largest listed mode that fits.

## Sources

| Entry | Source of the values |
| --- | --- |
| `intel_gen9` digital limits | HDMI-A and DVI-D: 300 MHz, the TMDS clock limit that i915 sets below display version 10 (`intel_hdmi_source_max_tmds_clock`). DP: 533.25 MHz, 3840x2160 at 60 Hz with CVT reduced blanking, within DisplayPort 1.2 (HBR2, 4 lanes). Size 4096x2160: Intel ARK for HD Graphics 530 gives 4096x2304 on both (24 Hz on HDMI, 60 Hz on DP); the DCI 4K size is the cap here. Not checked on the bench |
| `intel_gen9` | Matrix: super resolution default, native low clock to be measured, interlace only with linear or X-tiled scanout and not yet proven. PCI ids of Skylake, Kaby Lake, Amber Lake and Coffee Lake from the Linux file `include/drm/intel/pciids.h`. Whiskey Lake and Comet Lake ids are not listed: those GPUs match `intel_i915` and get its conservative values |
| `intel_i915` | Matrix: older Intel with native VGA, super resolution only, interlace varies, tier 2. Gen12 and newer on i915 also match it; its values are as conservative as the gen12 row |
| `intel_xe` | Matrix: gen12 and newer, super resolution, no interlace, tier 3 |
| `amd_amdgpu`, `amd_radeon` | Matrix: low pixel clock and interlace with the 15 kHz patch set, tier 2 until a card is tested |
| `nvidia_nouveau` | Matrix: super resolution, interlace unreliable, 25 MHz floor for older cards |
| `pi_vc4_dpi` | Matrix: GPIO DPI with any timing, interlace needs a vc4 DPI patch |
| `pi_vc4_composite` | Matrix: 240p, 288p, 480i and 576i at a fixed pixel clock. The 13.5 MHz clock and the 720 by 576 size are the modes of the Linux `vc4_vec.c` driver |
| `pi5_rp1_dpi` | Matrix: GPIO DPI with any timing, interlace through the RP1 PIO block |

The driver names and compatible strings of the Raspberry Pi entries are not yet checked on a board. A wrong one makes the GPU unknown, which gives the conservative values.

## Licence

This directory is `MIT OR Apache-2.0`, as the rest of the source.
