# 0006: The scene carries the scanlines of the game picture

- Status: accepted
- Date: 2026-11-11
- Task: C04

## Context

On a 31 kHz PC CRT, a 240-line game is shown line-doubled as 480p. A 15 kHz set leaves a dark gap below each line, and the line-doubled picture has no gap, so it looks soft and flat. The Definition offers "line-doubled to 480p, optional black scanlines" for this case, and gives the picture style two companion settings, one of them the scanline strength. The owner decided (OP-050) that black scanlines are in the MVP, and that the type must leave room for other kinds later: shaped scanlines at 3x and 4x, and kinds that depend on the resolution, with their own settings.

`Scene` had no field for an effect over the game. The renderer cannot choose one: only the video side knows the monitor, the mode and the settings (as for `ui_dest`, ADR 0005).

We looked at what hardware line doublers and FPGA scalers are documented to do. No device was checked on the bench for this decision.

- OSSC: in its line-doubling modes, a scanline strength in steps, applied to the digital RGB, which is gamma encoded. The dark line is one line of each doubled pair, with an option for its position. It also has a hybrid mode that depends on the brightness.
- RetroTINK 2X and 5X: scanlines on the second line of each pair. The 5X has a strength from 0 to 100 %, a thickness and brightness options.
- Framemeister (XRGB-mini): scanlines with a strength, in its upscaled modes.
- MiSTer: the scandoubler option "Scandoubler FX" for VGA monitors, with CRT 25 %, 50 % and 75 % and no 100 %. It darkens every second output line by a fixed fraction, with shifts and adds on the gamma-encoded value.
- Defaults: OSSC and RetroTINK ship with scanlines off. Users on PC CRTs mostly prefer 75 %. Black lines on every second line halve the light, and older tubes are dim.

The common case is the same everywhere: one dark line per doubled pair, the dark line under the bright one, a strength that multiplies the gamma-encoded signal, and full strength gives black. Hardware does not draw scanlines at a fractional scale.

## Decision

1. `Scene` gets the field `scanlines: Scanlines`. `Scanlines` is `{ kind: ScanlineKind, strength_percent: u8 }`, with serde. `ScanlineKind` is `none` or `black`, snake_case on the wire, append-only. `Scanlines::NONE` is kind `none`, strength 0.
2. The pixel rule, stated in the doc comment of `Scanlines`:
   - Only at a whole vertical scale `k = dest.h / crop.h` of at least 2 (`dest.h` exactly `k * crop.h`). Otherwise nothing changes: a fractional scale or a scale of 1 has no scanlines, so no pattern beats with the scale.
   - Destination line `j`, counted from `dest.y`, is line `j mod k` of its group. It is dark when `j mod k >= k - d`, with `d = ScanlineKind::dark_lines(k)`. For `black`, `d = floor(k / 2)`: 2x darkens line 2 of 2, 3x line 3 of 3 (one dark line in three, as the Definition says), 4x lines 3 and 4 of 4. The pattern follows the source lines, whatever the border above the picture, and a crop moves it by whole source lines only.
   - A channel on a dark line is the exact value times `(100 - strength_percent) / 100`, rounded to the nearest integer with a half rounded up, once. For nearest that is `(c * (100 - s) + 50) / 100` on the 8-bit channel. For sharp bilinear the factor applies to the exact blend before its one rounding.
   - The values stay gamma encoded, as in every hardware doubler we looked at. A linear-light darkening would need a transfer curve in both renderers and would not match the look that users know from that hardware.
   - Borders, samples outside the frame and the UI canvas do not change. The UI is drawn after the scanlines.
   - A strength above 100 is refused with `RenderError::ScanlineStrengthTooHigh` when the scene has a game frame.
3. The GLES renderer gets the pattern (period, first dark line, kept percent) from the reference code as one uniform. A nearest pixel uses exact integer steps and equals the reference. A blended pixel scales the float blend before its rounding and stays within 1 per channel. `gles-compare` checks both, and every strength from 0 to 100 on every channel value.
4. `neptron-types` gives the one rule that turns the settings into the value: `scanlines_for(mode, strength_percent, vertical_multiple, screen, interlaced)`, with `ScanlineMode` (`auto`, `off`, `on`) and `ScreenClass` (`crt15khz`, `crt31khz`, `flat_panel`, from `ScreenClass::of(monitor_kind, mode)` by the line rate of the mode, with the limit `CRT_15KHZ_CLASS_MAX_LINE_RATE_MHZ` of 20 kHz). `auto` draws black lines only on a CRT in the 31 kHz class. No mode draws on an interlaced mode, and a strength of 0 gives `NONE`. The engine and the preview of the display panel call this function, so both take one code path.
5. Two settings: `display.scanlines` (`auto`, `off`, `on`, default `auto`) and `display.scanline_strength` (0 to 100, default 75), at the game, system and global levels, safety class none. The default is 75 and not 100 for the reasons in the context: MiSTer stops at 75, users prefer it, and black lines halve the light of a tube that can be old and dim. The UI can offer presets (off 0, light 25, medium 50, strong 75, black 100) over the whole percent.

## Consequences

- A line-doubled game on a PC CRT gets the dark gaps of a 15 kHz set, at a strength the user picks.
- Every code that builds a `Scene` sets `scanlines`. `Scene` has no `Default`, so a missing value is a build error. All callers in the workspace set `Scanlines::NONE`, and every existing golden is unchanged.
- The scene hash of the fake display in `neptron-testkit` includes the kind and the strength. Hash values from before this change are not comparable. No test pins such a value.
- `ScanlineKind` is exhaustive on purpose. A new kind is a new variant: the build of each renderer, the testkit hash and the policy then fails until it handles the kind. A wildcard arm would hide that.
- The video side must pass the whole vertical multiple. The policy already has it in `Placement::scale.y`.
- Open for later, with no second change of the `Scene` shape:
  - Shaped scanlines (a soft profile per line at 3x and 4x, or a brightness-dependent width, as the OSSC hybrid mode): new `ScanlineKind` variants with their own `dark_lines` table or profile. A kind that needs data (a profile, a position of the dark line) can be a struct variant.
  - Kinds that depend on the resolution: the policy picks the kind per mode in `scanlines_for`.
  - More scanline settings (kind, position of the dark line, thickness): new keys and new choices at the end of `display.scanlines`.
  - Scanlines on a flat panel: the Definition gives built-in scanlines to a flat panel after the MVP. `auto` gives none there now.
  - A 15 kHz CRT with a game of fewer than half its lines: `auto` gives none. `on` gives black lines.

## Alternatives

- A strength only (no kind) in `Scene`: rejected, because shaped and per-multiple kinds would then need a second change of the same size.
- `#[non_exhaustive]` on `ScanlineKind`: rejected. All users are in the workspace, and a wildcard arm in a renderer would draw nothing for a new kind with no build error.
- A floating-point strength: rejected. Contract values are integers, and two renderers must give the same bytes.
- Darkening in linear light: rejected for the MVP, see decision 2. A later kind can add it.
- Scanlines at a fractional scale with a brightness-adaptive, moiré-filtered pattern (as the Definition names for shaders after the MVP): rejected for this kind, because it cannot be bit exact and it is not what a line doubler does. A later kind can add it.
- Dark lines from the output line parity: rejected, because a border of an odd number of lines would put the dark line on the wrong half of each source line.
- The renderer picks the scanlines from the scale: rejected, because only the video side knows the monitor, the settings and interlace.
