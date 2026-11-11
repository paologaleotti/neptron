## Hardware support matrix

Flat panels on any KMS driver are tier 1, and two tier-1 CRT targets carry the project: Raspberry Pi on GPIO DPI, and Intel gen9 onboard VGA, which is your i3-6100T test machine. Tier 1 is tested on real hardware every release, tier 2 is expected to work, tier 3 is best effort.

| Platform | Output | Kernel driver | 15 kHz progressive | Interlace | Tier |
| --- | --- | --- | --- | --- | --- |
| Raspberry Pi 4, 3, Zero 2 W | GPIO DPI: VGA666, RGB-Pi, Pi2SCART (all tier 1 boards) | vc4 KMS | Native, any timing | Needs a small vc4 DPI patch | 1 |
| Raspberry Pi 5 | GPIO DPI | RP1 DPI | Native, any timing | Yes, through PIO; GPIO 1 to 3 reserved | 1 |
| Raspberry Pi 3, 4, 5, Zero 2 W | Composite | vc4 VEC / RP1 VEC | 240p and 288p | 480i and 576i | 2 |
| x86 AMD Radeon, analog connector (VGA, DVI-I) | Native analog | radeon / amdgpu + 15 kHz patch set | Native low pixel clock | Yes with patches | 2 |
| x86 AMD, digital only, with DP-to-VGA DAC | DAC | amdgpu + patch set | Depends on DAC; super resolution default | Yes with patches | 2 |
| x86 Intel gen9 (Skylake to Coffee Lake), onboard VGA or DP + DAC | Internal or external DP-to-VGA bridge | i915 | Super resolution default; native to be measured | Yes, if scanout buffers are not Y-tiled | 1 |
| x86 Intel, older, native VGA | Native analog | i915 | Super resolution only | Varies by generation | 2 |
| x86 Intel gen12 and newer | DP + DAC | i915 / xe | Super resolution | No | 3 |
| x86 NVIDIA | Analog on older cards | nouveau | Super resolution | Unreliable | 3 |
| Any | HDMI / DP flat panel | any KMS driver | Not applicable: integer scaling and shaders | Not applicable | 1 |

**Notes behind the table**

- **Intel gen9 is better than its reputation, but the evidence is thin.** [Measurements on an i3-6100T / HD 530](https://github.com/amxcs/batocera-crt-15khz-intel/blob/master/docs/dp-dotclock-floor.md) show stock i915 on kernel 6.18 accepts a user-supplied 6.5 MHz, 320x240 modeline; the 10 MHz floor only prunes the probed mode list. That test ran on a DisplayPort LCD, from one author, and the [Batocera CRT Script](https://github.com/ZFEbHVUE/Batocera-CRT-Script) still lists Intel graphics as unsupported. Whether the motherboard's DP-to-VGA bridge passes such a low clock is untested, so super resolutions stay the default and WP 1.1 measures it.
- **Intel interlace.** i915 accepts only linear or X-tiled framebuffers in interlaced modes, and Mesa picks Y-tiled ones. [linux\_kernel\_15khz PR #17](https://github.com/D0023R/linux_kernel_15khz/pull/17), now merged as an optional patch, works around that and was verified on an HD 530 VGA port. NEPTRON allocates its own linear buffers, so 480i should work with no patch. It is the first thing to prove on your machine.
- **AMD** is the community's reference for x86, but stays tier 2 here until someone can test a card. The [15 kHz kernel patch set](https://github.com/D0023R/linux_kernel_15khz) maintained by D0023R (used by Batocera, current for 6.18) is applied as Buildroot kernel patches. amdgpu's display code gained analog outputs (VGA, DVI-I) only in kernel 6.19; on 6.18 those cards need `radeon`, or amdgpu with its legacy display path.
- **NVIDIA** is deliberately low. Old Intel chipsets [refuse pixel clocks under 20 to 25 MHz](https://web.archive.org/web/20240321205558/http://forum.arcadecontrols.com/index.php/topic,159804.0.html), the [Batocera CRT guide](https://github.com/ZFEbHVUE/Batocera-CRT-Script/wiki/How-to-CRT-Script-Configuration) gives 25 MHz for older NVIDIA cards, and [Batocera's CRT notes](https://github.com/ZFEbHVUE/Batocera-CRT-Script) describe NVIDIA support as limited and fragile. The proprietary driver is not shipped in the first releases.
- **Raspberry Pi 5** gained DPI interlace and composite sync [through the RP1 PIO block](https://www.raspberrypi.com/news/how-we-added-interlaced-video-to-raspberry-pi-5/). The PIO block then owns GPIO 1 (DE, or composite sync), GPIO 2 (VSYNC) and GPIO 3 (HSYNC) and is not free for other use. Interlace gives composite sync or VSYNC, not both. DPI and composite are not meant to be active together on Pi 5 because they [share clock resources](https://github.com/raspberrypi/linux/issues/5901).
- **Raspberry Pi composite** needs [`enable_tvout=1`](https://www.raspberrypi.com/documentation/computers/config_txt.html#enable_tvout) on Pi 4, Pi 5 and Zero boards, and that setting disables HDMI. The encoder fixes the pixel clock, so ODREN chooses among 240p, 288p, 480i and 576i and scales horizontally. Progressive 240p and 288p modes came with [raspberrypi/linux PR #4406](https://github.com/raspberrypi/linux/pull/4406).
- **31 kHz profiles** show 240-line content line-doubled to 480p, with optional black scanlines. Every output above can do this without special drivers.
- The matrix lives in the repo as a data file (`hardware/matrix.toml`) that also drives runtime GPU capability detection, so the document and the code cannot drift.

* **Pi clock stability.** RePlayOS's [changelog](https://www.replayos.com/changelog/) records that `core_freq_fixed=1` in place of `force_turbo=1` cured out-of-sync CRT modes, and that a bootloader EEPROM older than 2025-11-05 caused unstable interlace on GPIO DPI. NEPTRON sets the first in `config.txt` and warns about the second on the diagnostics page.
* **NVIDIA with the proprietary driver.** The [Batocera CRT guide](https://github.com/ZFEbHVUE/Batocera-CRT-Script/wiki/How-to-CRT-Script-Configuration) reports that Maxwell-generation and newer cards accept very low pixel clocks with it. NEPTRON does not ship that driver, so NVIDIA stays tier 3.
* **Driver quirks are data.** The capability table records, per GPU: minimum pixel clock, whether interlace works, whether page flips arrive per field, and known fixes such as forcing even fields on some AMD chips.

- **Pi 3, 4 and Zero 2 W interlace is the least proven tier-1 path.** The current Raspberry Pi kernel still rejects interlaced DPI modes in [`vc4_dpi.c`](https://github.com/raspberrypi/linux/blob/rpi-6.18.y/drivers/gpu/drm/vc4/vc4_dpi.c); the patch removes that check. RePlayOS supports GPIO DPI on Pi 5 only, so there is no reference implementation for the older boards. Test 480i on a Pi 4 in the M1 spike before these boards stay in tier 1.
