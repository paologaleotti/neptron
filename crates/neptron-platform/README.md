# neptron-platform

GPU and board detection from sysfs, the capability table, and a monitor profile built from the range limits in an EDID.

## Public API

- `fn detect(&Path) -> Result<Platform, SysfsError>`: reads `class/drm` under a root path (`/sys` on a running system). `Platform` lists each `Gpu` (driver, PCI id, device tree compatible strings) and each `Connector` (name, type, status, EDID).
- `Matrix::builtin()`, `Matrix::parse(&str)`: the capability table of `data/hardware/matrix.toml`, checked. `Matrix::lookup(&Gpu, connector_kind)` gives the entry; `Matrix::caps_for` gives its `GpuCaps`, or `UNKNOWN_GPU_CAPS`.
- `fn parse_edid(&[u8]) -> Result<Edid, EdidError>`: the strict EDID parser.
- `fn profile_from_edid(&[u8]) -> Result<EdidProfile, ProfileError>`: a one-range CRT `MonitorProfile`, with the highest pixel clock beside it. `EdidProfile::limit_caps` lowers the GPU's highest pixel clock to it.
- The pure parsers of sysfs text: `parse_uevent`, `parse_card_name`, `parse_connector_name`, `parse_status`.

Only `detect` does I/O. Everything else is a pure function over bytes and strings.

## EDID rules

- The parser reads EDID 1.3 and 1.4. It checks the length against the extension count, the header, the checksum of every block, the version, the manufacturer id, and the layout of the four descriptors: the first is a detailed timing, display descriptors have reserved bytes at 0 and no reserved tag, and there is at most one range limits descriptor (tag `0xFD`).
- The range limits descriptor is read with the EDID 1.4 offsets for rates above 255, and its padding and timing formula byte are checked. Reserved offset flags, offsets in EDID 1.3, a rate of 0, a minimum above a maximum and an unknown formula byte are errors.
- Only the rates and the highest pixel clock are used. The secondary GTF and CVT parameters are not used. A CVT descriptor that limits the active pixels per line is refused, because the profile cannot carry that limit.
- The highest pixel clock is one 10 MHz unit below byte 9, because the display rounds byte 9 up. For CVT in EDID 1.4 it is the exact value: byte 9 minus the 250 kHz reduction.
- A profile needs an analog input, bit 0 of byte 0x18 set, and a range limits descriptor. EDID 1.3 names that bit `default GTF supported` and EDID 1.4 names it `continuous frequency`; the GTF porches of the profile rest on it. Without any of them there is an error, not a guess.

## The profile from EDID

- Rates: the EDID limits, converted exactly to millihertz.
- Porches and syncs: EDID has none. They come from the default parameters of the VESA GTF 1.1 (C = 40, M = 600, K = 128, J = 20): the horizontal blanking duty cycle `30 - 300 * period_us / 1000` %, an 8 % horizontal sync, the blanking split in halves around the end of the sync, a vertical front porch of 1 line, a vertical sync of 3 lines, and at least 550 us of vertical sync and back porch. Each porch is a minimum, so it is the largest GTF value anywhere in the line-rate range, rounded up. Each sync is the GTF value at the lowest line rate, rounded up.
- Polarities: GTF, horizontal negative and vertical positive.
- Progressive line limits: the active lines that the rates leave after the vertical blanking. The maximum is `(1 / v_min - blanking) * h_max`, rounded down; the minimum is `(1 / v_max - blanking) * h_min`, rounded up.
- Interlace: none.
- Id: `edid_<manufacturer>_<product code>`, for example `edid_npt_0071`.

For a 30 to 71 kHz, 50 to 160 Hz monitor the range is: porches 817, 3334 ns horizontal and 33334, 507747 ns vertical; syncs 2667 ns and 100000 ns; 169 to 1374 lines.

## Unknown hardware

A GPU that matches no entry gets `UNKNOWN_GPU_CAPS`: 25 MHz lowest pixel clock, no interlace, no flip per field, 165 MHz highest pixel clock, 1920 by 1200. These are constants in the code, so a data edit cannot loosen them. A wrong driver name or PCI id in the table makes a GPU unknown, which is the safe direction.

## Tests

- `tests/integration/edid.rs`: the parser on blobs built byte by byte from the specification (`blobs.rs`), and every refusal.
- `tests/integration/profile.rs` and the unit tests in `src/gtf.rs` and `src/profile.rs`: the profile, with the GTF values worked out by hand.
- `tests/integration/matrix.rs`: the shipped table, the lookup, every format rule.
- `tests/integration/sysfs.rs`: detection on the fixture trees in `tests/fixtures/sysfs`.

## Must never depend on

- `neptron-kms`, `neptron-ui`, `neptron-video`

## Rules

- `unsafe` is forbidden: the crate root carries `#![forbid(unsafe_code)]`.
- No features.
