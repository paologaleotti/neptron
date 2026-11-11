## Input

NEPTRON uses SDL3 for gamepads, reads keyboards, mice and guns through evdev, and maps every device onto libretro's device types; the kernel ships with the same HID and joystick driver set as RetroPie and Batocera, plus light-gun modules.

| Device class | Kernel side | NEPTRON side | Milestone |
| --- | --- | --- | --- |
| USB gamepads (Bluetooth after alpha) | `xpad`, `hid-sony`, `hid-playstation`, `hid-nintendo`, `hid-microsoft`, `hid-logitech`, `hid-generic`, 8BitDo and clones | SDL3's built-in mappings, guided mapping for unknown pads, mapping to RetroPad | M2 |
| Arcade encoders | Keyboard encoders (I-PAC class), zero-delay USB encoders, GPIO/JAMMA joystick modules on Pi | Keyboard-as-joystick profiles, per-player assignment | M2 |
| Steering wheels and pedals | `hid-logitech`, `hid-logitech-hidpp`, `hid-thrustmaster`; out-of-tree `new-lg4ff`, `hid-tmff2`, `hid-fanatec`ff where licences allow | Axis mapping with range, dead zone and sensitivity per game; rumble forwarded to the wheel's force-feedback motor | M4 |
| GunCon 2 | Out-of-tree [guncon2](https://github.com/beardypig/guncon2) module | Calibration, timing-aware coordinate mapping, white flash on trigger | M4 |
| GunCon 3 | [guncon3](https://www.github.com/beardypig/guncon3) module, or a Rust userspace USB driver | Calibration, mapped to `RETRO_DEVICE_LIGHTGUN` | M4 |
| Gun4IR, OpenFIRE, AimTrak, Blamcon | Standard HID absolute pointers | Works through the generic pointer path, per-gun calibration | M4 |
| Sinden | Camera gun; needs a border on screen and Sinden's own driver software | Border rendering in the frontend; driver bundling is an open question | Later |
| Spinners, trackballs, mice | `hid-generic` | Each device is read separately, so two players can have two mice | M3 |

**Design points**

- **One read per frame.** All devices are drained immediately before `retro_run`, which keeps input lag at the minimum the core allows. The kernel parameter usbhid.jspoll=1 sets 1000 Hz polling for HID pads; xpad devices use the rate the pad itself reports.
- **Hotplug without a udev daemon.** SDL3 is built without libudev and finds devices through inotify. BusyBox `mdev` loads kernel modules from each device's modalias, as the stock Buildroot `mdev.conf` does.
- **GunCon 2 and the video engine belong together.** The gun reports raw values that need calibration per display, as its [driver README](https://github.com/beardypig/guncon2) shows. Calibration is stored per mode family, as other systems do. Converting one calibration to every mode from the known modeline is an experiment for later, not a promise. GunCon 2 only works on a 15 kHz CRT. Both GunCon kernel modules are unmaintained (last changed in 2022 and 2019), so expect to carry patches for the current kernel.
- **Wheels are limited by libretro, not by the kernel.** The libretro API offers a rumble interface but no steering force-feedback effects. NEPTRON can map axes precisely and send rumble to the wheel motor; true force feedback depends on future core support.
- **Bluetooth** is deferred until after the alpha: a pairing UI over BlueZ is a project of its own. USB pads and 2.4 GHz dongles cover the alpha.
