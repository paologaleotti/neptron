//! The internal resolution of a GL core: the whole scale `k` at which it
//! draws its picture (`docs/spec/hw-render.md`, section 4). Pure code.

use neptron_types::{MonitorKind, ScanlineMode};

/// The largest internal resolution that the session sets: 4x of 240 lines
/// is 960, the largest whole multiple below 1080.
pub const INTERNAL_SCALE_LIMIT: u8 = 4;

/// The machine the frontend runs on, as the rule of the internal
/// resolution sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Machine {
    /// A PC: its GPU draws a GL core above its native size on a flat
    /// panel.
    Pc,
    /// A Raspberry Pi. The cost of a larger picture is not measured on its
    /// GPU, so a GL core draws at its native size.
    RaspberryPi,
}

/// The core option that sets the internal resolution of a GL core, from
/// the core data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InternalScale<'a> {
    /// The key of the core option.
    pub option: &'a str,
    /// The lines of the native picture of the core's systems.
    pub native_lines: u16,
    /// The value of the option for each scale `k` that the core offers,
    /// sorted by `k`, with `k = 1`.
    pub values: &'a [(u8, String)],
}

/// The internal resolution for a game on `monitor`:
///
/// - 1 on a CRT and a TV: the mode carries the native lines.
/// - 1 on a Raspberry Pi.
/// - 1 on a flat panel with the scanlines on: the scanlines follow the
///   native lines only at 1x.
/// - Else, on a flat panel, the largest `k` of `offered` with
///   `k * native_lines` at most the lines of the largest mode of the
///   panel, and at most [`INTERNAL_SCALE_LIMIT`]; 1 when none fits.
pub fn internal_scale(
    monitor: &MonitorKind,
    machine: Machine,
    scanlines: ScanlineMode,
    native_lines: u16,
    offered: &[(u8, String)],
) -> u8 {
    let MonitorKind::FlatPanel { modes } = monitor else {
        return 1;
    };
    if machine == Machine::RaspberryPi || scanlines == ScanlineMode::On || native_lines == 0 {
        return 1;
    }
    let panel_lines = modes.iter().map(|m| m.vactive).max().unwrap_or(0);
    let fits = panel_lines / native_lines;
    offered
        .iter()
        .map(|(k, _)| *k)
        .filter(|&k| k <= INTERNAL_SCALE_LIMIT && u16::from(k) <= fits)
        .max()
        .unwrap_or(1)
}

/// The option value of `scale` for scale `k`, or `None` when the core
/// does not offer `k`.
pub fn scale_value<'a>(scale: &InternalScale<'a>, k: u8) -> Option<&'a str> {
    scale
        .values
        .iter()
        .find(|(offered, _)| *offered == k)
        .map(|(_, value)| value.as_str())
}

/// The scale of one frame of `width` by `height` from a core set to the
/// internal resolution `k`, whose largest frame is `max` and whose native
/// picture has `native_lines`: `k` when the core draws at `k` (its largest
/// frame has at least `k` times the native lines), both sides are
/// multiples of `k`, and the native size fits the native maximum (`max`
/// over `k`); else 1, and the session takes the frame as native. A core
/// that did not take the option reports its native maximum, so its frames
/// stay native.
pub(crate) fn frame_scale(
    (width, height): (u16, u16),
    k: u8,
    max: (u16, u16),
    native_lines: u16,
) -> u16 {
    let k = u16::from(k);
    let scaled = k > 1
        && u32::from(max.1) >= u32::from(k) * u32::from(native_lines)
        && width.is_multiple_of(k)
        && height.is_multiple_of(k)
        && width / k <= max.0 / k
        && height / k <= max.1 / k;
    if scaled { k } else { 1 }
}

#[cfg(test)]
mod tests {
    use neptron_types::{CrtRange, Modeline, TvStandard};

    use super::*;

    fn panel(lines: &[u16]) -> MonitorKind {
        MonitorKind::FlatPanel {
            modes: lines
                .iter()
                .map(|&vactive| Modeline {
                    pclock_hz: 148_500_000,
                    hactive: 1_920,
                    hsync_start: 2_008,
                    hsync_end: 2_052,
                    htotal: 2_200,
                    vactive,
                    vsync_start: vactive + 4,
                    vsync_end: vactive + 9,
                    vtotal: vactive + 45,
                    interlace: false,
                    doublescan: false,
                    hsync_positive: true,
                    vsync_positive: true,
                })
                .collect(),
        }
    }

    fn values(ks: &[u8]) -> Vec<(u8, String)> {
        ks.iter().map(|k| (*k, format!("{k}x"))).collect()
    }

    const ALL: [u8; 5] = [1, 2, 3, 4, 8];

    #[test]
    fn a_crt_and_a_tv_always_take_1x() {
        let crt = MonitorKind::Crt {
            ranges: Vec::<CrtRange>::new(),
        };
        let tv = MonitorKind::Tv {
            standard: TvStandard::Ntsc,
        };
        for monitor in [crt, tv] {
            for scanlines in [ScanlineMode::Auto, ScanlineMode::Off, ScanlineMode::On] {
                assert_eq!(
                    internal_scale(&monitor, Machine::Pc, scanlines, 240, &values(&ALL)),
                    1
                );
            }
        }
    }

    #[test]
    fn a_pc_panel_takes_the_largest_whole_multiple_that_fits_up_to_4x() {
        let offered = values(&ALL);
        let pc = |lines: &[u16], native| {
            internal_scale(
                &panel(lines),
                Machine::Pc,
                ScanlineMode::Auto,
                native,
                &offered,
            )
        };
        assert_eq!(pc(&[720, 1080], 240), 4, "960 of 1080");
        assert_eq!(pc(&[720], 240), 3, "720 of 720");
        assert_eq!(pc(&[1080], 288), 3, "864 of 1080");
        assert_eq!(pc(&[2160], 240), 4, "9x fits; the limit is 4");
        assert_eq!(pc(&[480], 480), 1);
        assert_eq!(pc(&[200], 240), 1, "a panel below the native lines");
        assert_eq!(pc(&[], 240), 1, "a panel with no mode");
    }

    #[test]
    fn a_pc_panel_takes_only_a_scale_that_the_core_offers() {
        let offered = values(&[1, 2, 4]);
        let at = |lines| {
            internal_scale(
                &panel(&[lines]),
                Machine::Pc,
                ScanlineMode::Off,
                240,
                &offered,
            )
        };
        assert_eq!(at(720), 2, "3x fits, and the core has no 3x");
        assert_eq!(at(1080), 4);
        assert_eq!(
            internal_scale(
                &panel(&[1080]),
                Machine::Pc,
                ScanlineMode::Off,
                240,
                &values(&[1])
            ),
            1
        );
    }

    #[test]
    fn a_pi_and_scanlines_on_a_panel_take_1x() {
        let offered = values(&ALL);
        assert_eq!(
            internal_scale(
                &panel(&[1080]),
                Machine::RaspberryPi,
                ScanlineMode::Off,
                240,
                &offered
            ),
            1
        );
        assert_eq!(
            internal_scale(
                &panel(&[1080]),
                Machine::Pc,
                ScanlineMode::On,
                240,
                &offered
            ),
            1
        );
        assert_eq!(
            internal_scale(
                &panel(&[1080]),
                Machine::Pc,
                ScanlineMode::Auto,
                240,
                &offered
            ),
            4,
            "auto draws no scanlines on a panel"
        );
    }

    #[test]
    fn the_value_of_a_scale_comes_from_the_core_data() {
        let offered = values(&[1, 2, 4]);
        let scale = InternalScale {
            option: "res",
            native_lines: 240,
            values: &offered,
        };
        assert_eq!(scale_value(&scale, 2), Some("2x"));
        assert_eq!(scale_value(&scale, 3), None);
    }

    #[test]
    fn a_frame_is_scaled_only_when_k_divides_it_and_its_native_size_fits() {
        let max = (2_560, 1_920);
        assert_eq!(frame_scale((1_280, 960), 4, max, 240), 4);
        assert_eq!(frame_scale((320, 240), 1, max, 240), 1);
        assert_eq!(frame_scale((1_282, 960), 4, max, 240), 1, "not a multiple");
        assert_eq!(frame_scale((1_280, 962), 4, max, 240), 1, "not a multiple");
        assert_eq!(
            frame_scale((1_284, 960), 4, (1_280, 960), 240),
            1,
            "above the maximum"
        );
    }

    #[test]
    fn a_core_that_did_not_take_the_option_keeps_native_frames() {
        assert_eq!(
            frame_scale((320, 240), 4, (640, 480), 240),
            1,
            "a native maximum: 320x240 is not 4x of 80x60"
        );
        assert_eq!(frame_scale((640, 480), 2, (640, 480), 240), 2);
    }
}
