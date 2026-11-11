## Flat panels, shaders and vertical games

The monitor profile selects one of two display modes: CRT mode changes the signal to fit the game, flat-panel mode keeps the panel's native signal and fits the game with scaling and shaders. Both use the same frame loop, settings and UI.

**Flat-panel mode (HDMI, DisplayPort, DVI-D)**

- Starts in the panel's preferred EDID mode, up to 4K. Another listed mode can be chosen in the Display panel, with confirm-or-revert.
- Scaling, per game, system or global:

| Option | Result |
| --- | --- |
| Integer (default) | Largest whole multiple that fits; perfectly even pixels, black borders |
| Integer, overscan | One multiple larger, cropping a few lines the original TV would have hidden |
| Fit, sharp | Fills the height at a fractional scale with sharp-bilinear filtering, no shimmer |
| Stretch | Fills the screen; aspect ratio not preserved |

- Aspect ratio: as reported by the core (default), square pixels, or forced 4:3.
- Refresh: the panel mode nearest the game is chosen (50 Hz for PAL, 60 Hz for NTSC). Audio rate control absorbs small differences. Driving variable-refresh displays at the game's exact rate is planned for after the alpha.
- The UI's 320x240 canvas is integer-scaled. High-resolution skin assets are a later skin-format addition.

**HD upscaling**

- 3D systems: cores that can render above native resolution (PlayStation, N64, Dreamcast class) get an "internal resolution" setting in the normal settings tree, with a sensible default per platform and panel resolution.
- 2D systems: upscaling shader presets (xBR and ScaleFX class) through the slang preset tier.
- HD upscaling is offered in flat-panel mode only. In CRT mode cores stay at native resolution so the modeline matches the game.

**Shaders**

| Tier | Contents | Runs on |
| --- | --- | --- |
| Built-in | Scanlines with adjustable strength, aperture grille and shadow mask, sharp-bilinear, one combined "CRT" preset with light curvature and bloom, LCD grid for handhelds, and an analog filter (composite or S-Video blend that turns dither patterns into transparency; also usable in CRT mode) | Every target, written for OpenGL ES 2, including Pi 3 and Zero 2 W |
| Presets | The RetroArch slang preset library through [librashader](https://github.com/SnowflakePowered/librashader), a Rust runtime (MPL-2.0 or GPL-3.0) that also builds the C++ glslang and SPIRV-Cross libraries | Desktop OpenGL 3.3+ or Vulkan: x86 first. Pi 4 and 5 need the Vulkan path, or librashader's undocumented OpenGL ES 3 path if tests show it works. Never Pi 3 or Zero 2 W |

- Defaults by profile: flat panel gets built-in scanlines for consoles and arcade and the LCD grid for handhelds. A 15 kHz CRT gets no shader, because its scanlines are real. A 31 kHz CRT gets line doubling with optional scanlines.
- Scanlines look best at a whole-number vertical scale, so choosing a scanline shader suggests integer scaling. A brightness-adaptive variant with moiré filtering covers fractional scales, as RePlayOS's display filter does.
- Shaders run as a pass after scaling in `neptron-gfx`; preset loading lives in a new `neptron-shader` crate.
- Budget: every built-in shader holds full speed at 1080p on a Pi 4. This is a release test.

**Vertical games on horizontal screens**

Each game carries an orientation, from the arcade database or the core's rotation callback. Two settings control the result: `rotation` (auto, 0, 90, 180, 270) and `screen_orientation` (horizontal or vertical).

| Screen | What a vertical game gets |
| --- | --- |
| Flat panel | Rotated upright, integer-scaled, pillarboxed. Shaders rotate with it, so scanlines stay horizontal on the glass |
| VGA CRT, 31 kHz or multisync | Rotated upright. The solver picks a mode whose line count is a whole multiple of the game's long side when the profile allows, for example 576 lines for a 288-line game. Otherwise 480p with sharp fit |
| 15 kHz CRT | The long side (256 to 320 lines) does not fit 240 progressive lines. Default is an interlaced mode showing the game 1:1, every pixel exact but with interlace flicker. A one-time warning explains that a vertical game cannot be sharp on a horizontal 15 kHz screen and offers the alternatives: fitted interlaced, a soft scale into 240p, a cropped 1:1 view, or TATE mode |
| Any screen turned on its side (TATE mode, system-wide) | `screen_orientation = vertical`: the UI rotates too, vertical games use exact native modes, and horizontal games get the fitted treatment instead |

**Pixel-perfect by default.** Every game gets its own mode, in either orientation, and Auto always takes the highest rung the monitor can display:

1. The game's native mode: 240p, 288p, 480i or 480p, at its exact refresh.
2. A whole multiple of it (2x, 3x, 4x), still at the exact refresh.
3. Interlaced at 1:1, when the lines only fit that way.
4. A fitted, filtered scale. This is the only rung that is not pixel-exact, and reaching it always shows a notice saying why and what would avoid it.

A 480p game, such as an arcade board built for a 31 kHz monitor, runs native on a VGA monitor and falls to rung 3 on a 15 kHz set.

The mode solver takes a `rotated` flag and swaps the source axes before solving, so the safety validator sees the real signal in every case.
