## What cannot be automated, and how to shrink it

Emulation proves NEPTRON asks for the right signal; it cannot prove a given GPU driver, DAC and tube deliver it. Three tools reduce the remaining human work to a few minutes per hardware setup.

| Gap | Why emulation cannot cover it | What reduces it |
| --- | --- | --- |
| Does the real driver program what was asked? | i915, vc4, RP1 and amdgpu are not emulated | `neptronctl selftest video` on the device cycles every mode family, reads back what the driver actually programmed, and writes a report file |
| Does the signal on the wire match? | Needs a measurement | **Sync probe**: a microcontroller board wired to HSYNC and VSYNC that measures line rate, field rate, pulse widths and polarity and reports them over USB |
| Is the picture stable on the tube? | Needs eyes | During the self-test the user answers yes or no per mode family with the gamepad; the answers go into the same report |
| Flat-panel output | Real HDMI path | A USB HDMI capture dongle on a runner; captured frames compared with goldens |
| Light guns, wheels, odd controllers | Physical devices | Record raw events once from the real device; replay them through `uinput` in every T3 run |
| Speed on Pi 3 and Zero 2 W | QEMU timing means nothing | `neptronctl bench` on self-hosted Pi runners: frame-time percentiles per core against thresholds |
| BIOS POST signal, tube geometry, convergence | Outside software | Documented; calibration screens for the human part |

**The self-test report is the unit of community testing.** A contributor with hardware you do not own runs one command, answers a few yes/no prompts and attaches one file. The matrix is then updated by hand in the same pull request. Automatic merging can come later if reports become frequent.

**A hardware runner, when you want one.** Your i3-6100T and one Pi 4, each with a sync probe on its VGA output, registered as self-hosted CI runners. That turns the M1 exit test into a nightly automated job. It is optional and can come after M1.
