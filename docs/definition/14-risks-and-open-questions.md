## Risks and open questions

The largest risk is scope: NEPTRON replaces RetroArch, EmulationStation and a CRT script layer at once, so the milestone order and the five-core limit in M2 matter more than any single technical choice.

| Risk | Why it matters | Mitigation |
| --- | --- | --- |
| Onboard DP-to-VGA bridges differ between motherboards | Some may reject low pixel clocks or resync slowly. The author of the Intel work reported multi-second switch delays on one setup ([PR #17 discussion](https://github.com/D0023R/linux_kernel_15khz/pull/17)) | Super resolutions make switches rare; measure on the i3-6100T in M1; publish a list of known-good external DACs |
| x86 firmware shows POST at 31 kHz | A fixed-frequency 15 kHz arcade monitor receives an out-of-range signal before NEPTRON runs | Document it; recommend Pi or a sync-protecting adapter for such monitors |
| Libretro host breadth | Cores rely on dozens of environment callbacks; each core exposes new gaps | Five reference cores first; a per-core compatibility file; add callbacks only when a core needs them |
| Out-of-tree kernel patches | The 15 kHz set and Pi patches must follow kernel updates | Pin an LTS kernel; track the upstream patch repository; keep our own patch count minimal |
| Core licences | Snes9x, Genesis Plus GX, PicoDrive and FBNeo are non-commercial. The GPL alternatives (bsnes, BlastEm, current MAME) are too heavy for Pi 3 and Zero 2 W | Record the licence in each core's metadata; the build can produce an image without them; never ship BIOS or ROM files |
| Switchres output changes between versions or hosts | Floating-point results can differ between macOS and Linux builds, and an upgrade can change modes | Pin the Switchres commit; golden modelines on both hosts; the validator judges the final integer timings |
| Pi 5 interlace needs GPIO 1 to 3 and the PIO block | Some DAC boards may use those pins differently | Verify each supported board's pinout; overlay per board |
| NVIDIA | Closed driver, weak low-resolution support in nouveau | Stay tier 3; revisit after alpha |
| Cores run in-process | A crashing core takes the frontend down, and some cores do not unload cleanly | Supervisor restarts into the game list with the last validated mode; save RAM flushed periodically; the frontend re-executes itself between games for a clean address space |
| Pi 3 and Zero 2 W limits | OpenGL ES 2 only and 512 MB on the Zero 2 W; the documentation says every Pi model supports tryboot, but this is untested here | Per-board core list; a 512 MB memory-budget test in CI; test tryboot on a Pi 3 before WP 4.1, with U-Boot as the fallback |
| Binary firmware | GPUs and Wi-Fi chips need non-free firmware blobs, so "all open" covers our code but not every byte in the image | Ship only the needed linux-firmware subsets and list them in the licence manifest |
| Limits of headless testing | Emulation cannot show what a real DAC, GPU driver or tube does with a signal | Self-test reports from real machines and an optional sync probe; see the Implementation plan tab |
| Switchres licence and build | Its source headers say GPL-2.0-or-later, so the NEPTRON binary is GPL, and GPL-3.0-or-later fits. It has no LICENSE file, and its makefile has no macOS branch, so NEPTRON compiles the calculation sources itself | Compile them with the cc crate in WP 1.2; if that fails on either host, fall back to a Rust port tested against the Switchres command-line tool |
