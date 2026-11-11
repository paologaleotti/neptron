## Work packages

Thirty-nine work packages in five milestones, in build order; each is sized for one focused agent session and closes on an automated gate. The first six exist only to make the rest verifiable, and they need no hardware, no Buildroot and no QEMU.

### M0 Foundation: the agent can build, run and see

| WP | Task | Gate |
| --- | --- | --- |
| 0.1 | Workspace with the crates of the monorepo layout as stubs, lints, pinned toolchain, `CLAUDE.md`, ADR folder, dependency-rule checker | T0 green on macOS and Linux |
| 0.2 | `xtask`: `verify`, `shot`, `gallery`, `golden`; JSON report format | `cargo xtask verify --tier 1` runs on both hosts |
| 0.3 | Backend traits (display, audio sink, input source, clock) and their fakes in `neptron-testkit` | A fake session runs 1,000 frames with identical output twice |
| 0.4 | CPU UI renderer (evaluate the `embedded-graphics` crate before writing a blitter), scene registry, first scene and golden | Golden is bit-identical on macOS and Linux |
| 0.5 | `neptron-ipc` and `neptronctl`: screenshot, UI tree, input, clock, wait | A scripted T2 test drives the app only through the socket |
| 0.6 | CI: macOS and Linux for T0 to T2 | Pipeline green |

### M1 Signal: a correct, safe mode on the wire

| WP | Task | Gate |
| --- | --- | --- |
| 1.1 | Hardware spike on a stock Linux install, on the i3-6100T and a Pi 4: set custom 240p and 480i modes through atomic KMS with our own buffers; confirm Intel interlace with non-Y-tiled buffers; confirm the kernel stays silent with fbdev emulation off | Findings written as an ADR and the KMS spec; later tasks adjusted to them |
| 1.2 | Switchres licence and build check; `neptron-modeline` wrapper built on macOS and Linux; profile presets; EDID-derived profiles | Wrapper returns modes for the golden list on both hosts. If the check fails: Rust port, tested against the Switchres command-line tool |
| 1.3 | `neptron-safety`: `ValidatedMode`, range checks, hard rails | Timing check 2 |
| 1.4 | ODREN policy: horizontal strategies, picture styles, pixel-perfect ladder, sources that do not fit, rotation | Timing check 3 |
| 1.5 | Dev container; Buildroot tree with the two QEMU configs; BusyBox init; `cargo xtask qemu`, `image`, `deploy`; the `vkms` modeline and interlace check | T3: boots to the test screen; `vkms` writeback screenshot equals the golden |
| 1.6 | `neptron-kms` with the GPU capability table: atomic modeset, buffer modifiers, buffers allocated once, flip events | Timing check 7 |
| 1.7 | Confirm-or-revert, last-known-good, safe mode, restore on restart | Scripted safety scenarios in T2 and T3 |
| 1.8 | Real target images (`rpi-arm64`, `x86_64`); test patterns; `neptronctl selftest video` and its report | Pattern goldens; then the hardware run on the i3-6100T and a Pi 4 with each GPIO DAC |
| 1.9 | CI adds T3, a cached base image, boot-time and memory trends | Pipeline green; numbers stored per run |

### M2 Play: games run with exact timing

| WP | Task | Gate |
| --- | --- | --- |
| 2.1 | `neptron-libretro-sys`, the core host for software-rendered cores, `neptron-testcore` | Test-core frames hash to the expected values |
| 2.2 | Audio: SDL3 output, rubato resampler, rate control | Timing check 6; T3 capture of a test tone |
| 2.3 | Input: SDL3 gamepads, hotplug, guided mapping, keyboard encoders | T3 with `uinput` pads |
| 2.4 | ODREN engine in `neptron-video` | Timing check 5 |
| 2.5 | Game-path GLES renderer; hardware-rendered cores | Goldens under software GL, with a small tolerance |
| 2.6 | Flat-panel mode: scaling options, aspect, refresh choice | Goldens on virtual 720p, 1080p and 4K displays |
| 2.7 | Five reference cores as Buildroot packages; free test ROM set; core traces | T4 frame hashes; traces committed |
| 2.8 | Re-exec between games; crash recovery | A crashing test core returns the user to the game list with the right mode |

### M3 Console: a complete couch experience

| WP | Task | Gate |
| --- | --- | --- |
| 3.1 | Settings schema, three levels, generated menus | Unit tests; the crawler reaches every key |
| 3.2 | Widgets, screens, skin loader, default skin | Scene goldens; UI crawler |
| 3.3 | Library: scan, index, `gamelist.xml` import, zipped ROMs | Fixture trees; a 10,000-game list scrolls within the frame budget |
| 3.4 | Quick menu, save states, fast-forward | Scripted sessions |
| 3.5 | Built-in shaders; rotation for vertical games | Goldens under software GL with tolerance; solver goldens with the rotation flag |
| 3.6 | First-boot wizard and calibration screens | Scripted run from blank state to game list |
| 3.7 | Network setup, SMB share, safe shutdown | T3: a ROM copied over SMB appears in the library; a power-button event leaves saves intact |
| 3.8 | Mice, spinners, trackballs per player | T3 with two `uinput` mice |

### M4 Alpha: ready for other people

| WP | Task | Gate |
| --- | --- | --- |
| 4.1 | A/B updates with RAUC | T3: good update boots and confirms; corrupt image is refused; power cut mid-write boots the old slot; bad new slot rolls back |
| 4.2 | x86 installer | T3 with two virtual disks |
| 4.3 | Light guns: driver packages, calibration maths | Property tests on the maths; recorded gun traces replayed |
| 4.4 | Steering wheels | Recorded wheel traces replayed; rumble commands observed on the virtual device |
| 4.5 | Composite output; one device-tree overlay per GPIO DAC | Overlays compile and pass pin-conflict checks; hardware self-test reports |
| 4.6 | Slang presets through librashader; HD upscaling options | Goldens for a small preset set under software GL |
| 4.7 | Box-art scraping | Tests against a local mock server; no network in any test |
| 4.8 | Hardware validation campaign, user documentation, release images | Self-test reports for every tier-1 row of the matrix |
