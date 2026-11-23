# neptron-platform

GPU and board detection from sysfs, the capability table, a monitor profile from an EDID (a CRT profile from its range limits, or a flat-panel profile from its listed modes), and the parse of the network status file of the image.

## Must never depend on

- `neptron-kms`, `neptron-ui`, `neptron-video`

## Rules

- `#![forbid(unsafe_code)]` at the crate root. No features.
- Only `detect` does I/O. Everything else is a pure function over bytes and strings.

## Public API

- `detect(&Path) -> Result<Platform, SysfsError>` reads `class/drm` under a root path (`/sys` on a running system). `Platform` lists each `Gpu` (driver, PCI id, device tree compatible strings) and each `Connector` (name, type, status, EDID). The pure sysfs text parsers: `parse_uevent`, `parse_card_name`, `parse_connector_name`, `parse_status`.
- `Matrix::builtin()`, `Matrix::parse(&str)`: the checked capability table of `data/hardware/matrix.toml`. `lookup(&Gpu, connector_kind)` gives the entry. `caps_for` gives its `GpuCaps`, or `UNKNOWN_GPU_CAPS`. `caps_for_output(&Gpu, connector_kind, OutputKind)`: an analog output gets `caps_for`; a digital output gets the entry's caps with its `DigitalLimits` for that connector, when it has them. `DisplayClass::output_kind`: a flat panel is digital, a CRT analog.
- `parse_edid(&[u8]) -> Result<Edid, EdidError>`: the strict EDID parser.
- `profile_from_edid(&[u8]) -> Result<EdidProfile, ProfileError>`: a one-range CRT `MonitorProfile` with its highest pixel clock. `EdidProfile::limit_caps` lowers the GPU's highest pixel clock to it.
- `listed_modes(&[u8]) -> Result<ListedModes, EdidError>`: the identity, product name and progressive modes an EDID lists.
- `display_class(connector_kind, subconnector, &[u8]) -> Result<DisplayClass, EdidError>`: CRT or flat panel ("CRT or flat panel").
- `profile_for_display(connector_kind, subconnector, &[u8]) -> Result<EdidProfile, ProfileError>`: the CRT or the flat-panel profile, by the class.
- `panel_profile_from_edid(&[u8]) -> Result<EdidProfile, ProfileError>`: a flat-panel profile of the listed modes, preferred mode first, whatever the input type.
- `EdidProfile::fit_panel(&GpuCaps, max_width, max_height) -> Option<PanelFit>`: the listed modes the GPU and the buffers can send. When the preferred mode does not fit, the largest mode of at least `GAME_RATE_MIN_MHZ` (50 Hz) that fits comes first (2160p30 would halve the speed of a 60 Hz game, so 1080p60 wins), then a slower one. `PanelFit::native_left_out` names the preferred mode for a notice.
- `parse_network(&str) -> Result<NetworkStatus, NetworkError>`: the text of `/run/neptron/network.toml` ("The network file").

## EDID rules

- The parser reads EDID 1.3 and 1.4. It checks the length against the extension count, the header, the checksum of every block, the version, the manufacturer id, and the layout of the four descriptors: the first is a detailed timing, display descriptors have reserved bytes at 0 and no reserved tag, and there is at most one range limits descriptor (tag `0xFD`).
- The range limits descriptor is read with the EDID 1.4 offsets for rates above 255, and its padding and timing formula byte are checked. Reserved offset flags, offsets in EDID 1.3, a rate of 0, a minimum above a maximum and an unknown formula byte are errors.
- Only the rates and the highest pixel clock are used. The secondary GTF and CVT parameters are not used. A CVT descriptor that limits the active pixels per line is refused, because the profile cannot carry that limit.
- The highest pixel clock is one 10 MHz unit below byte 9, because the display rounds byte 9 up. For CVT in EDID 1.4 it is the exact value: byte 9 minus the 250 kHz reduction.
- A profile needs an analog input, bit 0 of byte 0x18 set, and a range limits descriptor. EDID 1.3 names that bit `default GTF supported` and EDID 1.4 names it `continuous frequency`; the GTF porches of the profile rest on it. Without any of them there is an error, not a guess.

## CRT or flat panel

`display_class` decides, in this order:

1. An EDID that fails the structural checks (length, header, the checksum of the base block or of any extension block, version, manufacturer, descriptor layout) gives an error. EDID 1.0 to 1.2 is refused as a version: only 1.3 and 1.4 are read. The caller has no EDID and uses its safe start (`generic_15`), as with no EDID at all.
2. An analog input (byte 0x14, bit 7 clear) is a CRT, on any connector. An active DP-to-VGA or HDMI-to-VGA adapter passes the EDID of the monitor behind it, so a `DP-n` or `HDMI-A-n` with an analog EDID is a CRT behind a DAC. An LCD with a VGA input also has an analog EDID and takes the CRT path; that is correct, it accepts the modes in its ranges.
3. A downstream port type (the `subconnector` property of a DP or DVI-I connector) of `VGA` or `DVI-A` is a CRT, even with a digital EDID: the adapter is a DAC.
4. An analog-only connector (`VGA`, `DVI-A`, `Composite`, `SVIDEO`, `Component`, `DIN`, `TV`, `DPI`) is a CRT, even with a digital EDID.
5. Every other case is a digital input on a digital connector (`HDMI-A`, `DP`, `eDP`, `DVI-D`, `DVI-I`, `LVDS`, `DSI`, `Virtual`): a flat panel. The parser has proved that the first descriptor is a detailed timing, the preferred mode.

A CRT with a digital EDID builds no CRT profile (`ProfileError::NotAnalog`), so the caller uses its safe start.

**The limit of the rule.** An adapter that presents its own digital EDID, with a CRT behind it, and that reports no analog downstream port, looks like a flat panel. The rule then proposes the adapter's preferred mode (often 1080p) to the CRT. No signal in the EDID tells such an adapter apart from a real panel without making some real panels fail: a missing product name, an unknown manufacturer id and an odd preferred mode are all common on cheap panels and TVs. So the rule adds no guess. The protections are in the app: a monitor preset in `display.monitor` always wins over detection, and a panel mode is never the safe default until the user keeps it on that output, so an unanswered boot prompt leaves the 15 kHz safe start on the wire after one confirm window.

## The flat-panel profile

- Id: `panel_<manufacturer>_<product code>`, for example `panel_npt_1080`. The prefix keeps a CRT profile and a panel profile of the same EDID apart in the guard record.
- Modes, each once, progressive only, in this order: the detailed timings of the base block (the first is the preferred mode), the detailed timings of the CTA-861 extension blocks, the CTA-861 video codes of `CTA_MODES` (640x480 and 720x480 at their 59.94 Hz clocks of 25.175 and 27 MHz; 720p60, 1080p60, 576p50, 720p50, 1080p50, 2160p30, 2160p50 and 2160p60 at the integer rate), the standard timings that name a 60 Hz mode of `DMT_MODES`, and the established timings 640x480, 800x600 and 1024x768 at 60 Hz.
- Not read: interlaced timings, standard timings at other rates or sizes, other video codes (59.94 Hz variants included), the other established timings, DisplayID and HDMI Forum blocks. A CTA-861 block whose offsets point outside the block gives no modes.
- The content of the range limits descriptor is not read: a panel is driven only in its listed modes. So a panel EDID whose range limits descriptor the CRT parser refuses (a CVT limit on the active pixels, a padding byte out of the specification) still gives a panel profile.
- The highest pixel clock is the clock of the fastest listed mode, so `limit_caps` never removes a listed mode.
- The product name of descriptor tag `0xFC` goes in `EdidProfile::name`, for both kinds.

## The profile from EDID

- Rates: the EDID limits, converted exactly to millihertz.
- Porches and syncs: EDID has none. They come from the default parameters of the VESA GTF 1.1 (C = 40, M = 600, K = 128, J = 20): the horizontal blanking duty cycle `30 - 300 * period_us / 1000` %, an 8 % horizontal sync, the blanking split in halves around the end of the sync, a vertical front porch of 1 line, a vertical sync of 3 lines, and at least 550 us of vertical sync and back porch. Each porch is a minimum, so it is the largest GTF value anywhere in the line-rate range, rounded up. Each sync is the GTF value at the lowest line rate, rounded up.
- Polarities: GTF, horizontal negative and vertical positive.
- Progressive line limits: the active lines that the rates leave after the vertical blanking. The maximum is `(1 / v_min - blanking) * h_max`, rounded down; the minimum is `(1 / v_max - blanking) * h_min`, rounded up.
- Interlace: none.
- Id: `edid_<manufacturer>_<product code>`, for example `edid_npt_0071`.

For a 30 to 71 kHz, 50 to 160 Hz monitor the range is: porches 817, 3334 ns horizontal and 33334, 507747 ns vertical; syncs 2667 ns and 100000 ns; 169 to 1374 lines.

## The network file

`/run/neptron/network.toml` holds the host name and the IPv4 address of each wired port with a lease. The address script of the image writes it, and the Network panel of the app shows it. `docs/spec/network-panel.md`, "The reader", has every rule of the parse: the version first (`NETWORK_FILE_VERSION`, 1), unknown fields ignored, the bounds (`NETWORK_FILE_MAX_BYTES`, `ADDRESSES_MAX`), the port, address and host name rules, and where the reader is stricter than the script. An empty or invalid host name gives `host_name: None`, not an error. The parse does no I/O. The app reads the file off the frame thread.

## Unknown hardware

A GPU that matches no entry gets `UNKNOWN_GPU_CAPS`: 25 MHz lowest pixel clock, no interlace, no flip per field, 165 MHz highest pixel clock, 1920 by 1200. These are constants in the code, so a data edit cannot loosen them. A wrong driver name or PCI id in the table makes a GPU unknown, which is the safe direction.

## Tests

`cargo xtask test -p neptron-platform`. In `tests/integration/`:

- `edid.rs`: the parser on blobs built byte by byte from the specification (`blobs.rs`), and every refusal.
- `panel.rs`: the class table (HDMI, DisplayPort, eDP, DVI-D and TV panels, a VGA CRT, a DP-to-VGA bridge, analog EDIDs on digital connectors, a DAC with its own digital EDID and a VGA subconnector, a VGA connector with a digital EDID, a broken EDID, no EDID), the listed modes of each blob, and the fit to the limits.
- `profile.rs`, with the unit tests in `src/gtf.rs` and `src/profile.rs`: the profile, with GTF values worked out by hand.
- `matrix.rs`: the shipped table, the lookup, every format rule.
- `sysfs.rs`: detection on the fixture trees in `tests/fixtures/sysfs`.
- `network.rs`: the network file, every error and every bound.
