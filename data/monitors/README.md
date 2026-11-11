# Monitor presets

One TOML file per monitor preset. Each file is a `MonitorProfile` of `neptron-types`: an `id` equal to the file name, and one `[[kind.crt.ranges]]` table per Switchres `crt_range`.

## Origin

The values come from the preset table of Switchres (function `monitor_set_preset` in `monitor.cpp`):

- Project: https://github.com/antonioginer/switchres
- Commit: `09272ac24640daee45f5be15b1b856ecd220dda2`
- Files: `monitor.cpp` (the preset table) and `switchres.ini` (the documentation of the `crt_range` format)
- Source URL of the table: https://github.com/antonioginer/switchres/blob/09272ac24640daee45f5be15b1b856ecd220dda2/monitor.cpp

Switchres is licensed GPL-2.0-or-later. Its copyright holders are Chris Kennedy, Antonio Giner, Alexandre Wodarczyk and Gil Delescluse. Each file keeps the original `crt_range` lines in a comment. Whether data derived from this table can stay in the `MIT OR Apache-2.0` tree is an open licence question.

## Units

A Switchres `crt_range` line is:

```text
HfreqMin-HfreqMax, VfreqMin-VfreqMax, HFrontPorch, HSyncPulse, HBackPorch,
VFrontPorch, VSyncPulse, VBackPorch, HSyncPol, VSyncPol,
ProgressiveLinesMin, ProgressiveLinesMax, InterlacedLinesMin, InterlacedLinesMax
```

| Switchres field | Switchres unit | Field here | Unit here | Factor |
| --- | --- | --- | --- | --- |
| Hfreq | Hz | `h_min_mhz`, `h_max_mhz` | mHz | 1,000 |
| Vfreq | Hz | `v_min_mhz`, `v_max_mhz` | mHz | 1,000 |
| HFrontPorch, HSyncPulse, HBackPorch | µs | `h_front_porch_ns`, `h_sync_ns`, `h_back_porch_ns` | ns | 1,000 |
| VFrontPorch, VSyncPulse, VBackPorch | ms | `v_front_porch_ns`, `v_sync_ns`, `v_back_porch_ns` | ns | 1,000,000 |
| HSyncPol, VSyncPol | 0 or 1 | `hsync_positive`, `vsync_positive` | bool | 1 is positive |
| Line limits | lines | `progressive_lines_*`, `interlaced_lines_*` | lines | 1 |

Interlaced limits of 0 and 0 mean that the range accepts no interlaced mode. The line rates in the table are all in Hz; Switchres reads them as Hz.

## Conversion

`cargo xtask monitors <path to monitor.cpp>` writes the files again. The converter reads each value as decimal text and scales it with integer maths, so no value goes through a float. A value that does not convert exactly is an error. Get `monitor.cpp` at the commit above, run the command, and review the diff.

The test `crates/neptron-types/tests/integration/monitors.rs` parses every file, checks that each range is sane and that the ids are unique, and compares `generic_15`, `arcade_15`, `ntsc`, `pc_31_120` and `m3129` with values computed by hand from the source.

## Presets

25 files: `pal`, `ntsc`, `generic_15`, `arcade_15`, `arcade_15ex`, `arcade_25`, `arcade_31`, `arcade_15_25`, `arcade_15_31`, `arcade_15_25_31`, `m2929`, `d9800`, `d9400`, `d9200`, `k7000`, `k7131`, `m3129`, `h9110`, `polo`, `pstar`, `ms2930`, `ms929`, `r666b`, `pc_31_120`, `pc_70_120`. An alias in Switchres (`d9800` and `d9400`, `h9110` and `polo`) gets one file per name.

Not converted:

- `vesa_480`, `vesa_600`, `vesa_768`, `vesa_1024`: Switchres computes these ranges at run time from the VESA GTF formula, with floating point maths. The table holds no `crt_range` for them.
- `custom` and `lcd`: these are modes of Switchres, not presets.
