//! The arcade policy table: real arcade boards, vertical and horizontal,
//! through the policy as it is, on every preset of data/monitors, on
//! profiles built from the EDID range limits of typical PC tubes, on flat
//! panels and on the composite TV standards.
//!
//! The test checks that every cell has a plan and that every CRT winner
//! has a whole-kHz pixel clock, and prints the table. Appendix A of
//! docs/spec/arcade.md holds a copy. To make it again:
//!
//! ```text
//! cargo xtask test -p neptron-modeline --no-capture arcade_table
//! ```
//!
//! The table is a report of the policy as it is, not a golden: a cell can
//! show a result that the arcade rule of the spec does not want. The cells
//! that follow the rule are goldens in tests/golden/policy_arcade.toml.

use neptron_modeline::policy::{
    Considered, Plan, PolicySettings, Proposal, RefreshKind, Rung, choose, decide,
};
use neptron_safety::validate;
use neptron_types::{
    CrtRange, Filter, GpuCaps, Modeline, MonitorKind, MonitorProfile, Rotation, SourceGeometry,
    TvStandard,
};

use crate::presets::{gpu_table, open_caps, panel_1080, pc_crt_30_71, preset};

/// An arcade board as FinalBurn Neo gives it: the frame as drawn, the rate
/// cut to 0.01 Hz, and the turn it asks for. The display aspect of every
/// board is 4:3 of the frame as drawn: FBNeo reports the upright 3:4 of a
/// vertical board, and the session inverts it (docs/spec/arcade.md,
/// section 11).
pub struct Board {
    pub name: &'static str,
    pub width: u16,
    pub height: u16,
    pub refresh_mhz: u32,
    pub rotation: Rotation,
}

/// The reference boards, then the hard cases. Unless the comment names
/// MAME, the size is the one of the FBNeo driver entry and the rate is the
/// one FBNeo reports: its `BurnSetRefreshRate` value cut to 0.01 Hz, or
/// 60.00 Hz when the driver sets none (`nBurnFPS` defaults to 6000 in
/// `burn.cpp`). Read at libretro/FBNeo master on 2026-11-16.
pub const BOARDS: [Board; 18] = [
    // Cave 68000, DoDonPachi (d_dodonpachi.cpp): 240x320 upright,
    // CAVE_REFRESHRATE 15625 / 271.5 Hz.
    board("ddonpach", 320, 240, 57_550, Rotation::R90),
    // Toaplan 2, Batsugun (d_batsugun.cpp): 240x320, TOA_REFRESHRATE
    // 59.637405 Hz.
    board("batsugun", 320, 240, 59_630, Rotation::R90),
    // Psikyo, Gunbird (d_psikyo.cpp): 224x320, 15625 / 263.5 Hz.
    board("gunbird", 320, 224, 59_290, Rotation::R90),
    // Taito F3, Gunlock (d_taitof3.cpp): 224x320, vertical and flipped,
    // 58.943844 Hz.
    board("gunlock", 320, 224, 58_940, Rotation::R270),
    // Pac-Man (d_pacman.cpp): 224x288, vertical and flipped. FBNeo sets
    // no rate and reports 60.00 Hz; the board runs at 60.606 Hz.
    board("pacman", 288, 224, 60_000, Rotation::R270),
    // The same board at its own rate, 6.144 MHz / 384 / 264 (the MAME
    // timing of tests/golden/systems.toml), as a core that reports it
    // gives it.
    board("pacman_60606", 288, 224, 60_606, Rotation::R270),
    // Donkey Kong (d_dkong.cpp): 224x256, vertical and flipped, 60.00 Hz
    // reported. Galaga (d_galaga.cpp, 224x288) gives the same cells as
    // Pac-Man.
    board("dkong", 256, 224, 60_000, Rotation::R270),
    // Sega System 24, Gain Ground (MAME segas24.cpp, ROT270; no FBNeo
    // driver): 384x496 upright, 16 MHz / 656 / 424 = 57.524 Hz, 24.4 kHz.
    board("gground", 496, 384, 57_524, Rotation::R270),
    // NAOMI class, Ikaruga (MAME naomi.cpp, ROT270; no FBNeo driver):
    // 480x640 upright. 60.00 Hz is the nominal VGA rate. UNVERIFIED: the
    // MAME raw screen (26.917 MHz, 820 x 532) gives 61.70 Hz.
    board("ikaruga", 640, 480, 60_000, Rotation::R270),
    // Neo Geo: 304 wide in FBNeo (docs/spec/arcade.md, OP-099), 59.18 Hz.
    board("neogeo", 304, 224, 59_180, Rotation::R0),
    // CPS-1 and CPS-2: 384x224 at 59.63 Hz.
    board("cps2", 384, 224, 59_630, Rotation::R0),
    // Cave horizontal, Power Instinct 2 (d_pwrinst2.cpp): 320x240.
    board("pwrinst2", 320, 240, 57_550, Rotation::R0),
    // Taito F3, Darius Gaiden (d_taitof3.cpp): 320x232, 58.943844 Hz.
    board("dariusg", 320, 232, 58_940, Rotation::R0),
    // Irem M72, R-Type (d_m72.cpp): 384x256, 55.00 Hz.
    board("rtype", 384, 256, 55_000, Rotation::R0),
    // Midway Y Unit, Mortal Kombat (d_yunit.cpp): 400x256, 54.70 Hz.
    board("mk", 400, 256, 54_700, Rotation::R0),
    // CPS-3 wide mode, SFIII 3rd Strike widescreen (cps3run.cpp):
    // 496x224, 59.59949 Hz.
    board("sfiii3ws", 496, 224, 59_590, Rotation::R0),
    // Sega System 24, Bonanza Bros (MAME segas24.cpp, ROT0): 496x384.
    board("bnzabros", 496, 384, 57_524, Rotation::R0),
    // Little Robin (d_littlerb.cpp): 338x288 at 30.00 Hz.
    board("littlerb", 338, 288, 30_000, Rotation::R0),
];

const fn board(
    name: &'static str,
    width: u16,
    height: u16,
    refresh_mhz: u32,
    rotation: Rotation,
) -> Board {
    Board {
        name,
        width,
        height,
        refresh_mhz,
        rotation,
    }
}

impl Board {
    pub fn source(&self) -> SourceGeometry {
        SourceGeometry {
            width: self.width,
            height: self.height,
            refresh_mhz: self.refresh_mhz,
            aspect_milli: 1_333,
            interlaced: false,
            rotation: self.rotation,
        }
    }
}

/// A display of the table: its profile and the GPU it is driven by.
pub struct Display {
    pub profile: MonitorProfile,
    pub caps: GpuCaps,
}

/// Every display of the table, by class: the 15 kHz presets, the
/// multi-range presets, the 31 kHz presets, the EDID tubes, the panels and
/// the TV standards. The CRT presets use a GPU with no pixel clock floor
/// that sends interlace, as the policy goldens do; a floor changes the
/// width of a mode, not its lines.
pub fn displays() -> Vec<Display> {
    let presets = [
        "generic_15",
        "arcade_15",
        "arcade_15ex",
        "h9110",
        "polo",
        "k7131",
        "k7000",
        "ntsc",
        "pal",
        "arcade_15_25",
        "arcade_15_25_31",
        "arcade_15_31",
        "m3129",
        "d9200",
        "d9400",
        "d9800",
        "ms929",
        "ms2930",
        "r666b",
        "pstar",
        "arcade_25",
        "arcade_31",
        "pc_31_120",
        "pc_70_120",
        "m2929",
    ];
    let mut displays: Vec<Display> = presets
        .iter()
        .map(|id| Display {
            profile: preset(id),
            caps: open_caps(),
        })
        .collect();
    displays.extend(edid_tubes());
    displays.extend(panels());
    let composite = gpu_table()
        .into_iter()
        .find(|(id, _)| id == "pi_vc4_composite")
        .map(|(_, caps)| caps)
        .expect("pi_vc4_composite is in data/hardware/matrix.toml");
    for standard in [TvStandard::Ntsc, TvStandard::Pal] {
        displays.push(Display {
            profile: standard.profile(),
            caps: composite,
        });
    }
    displays
}

/// Profiles that `neptron_platform::profile_from_edid` builds for typical
/// PC tubes, from their EDID range limits: line rates in kHz, field rates
/// in Hz, the pixel clock limit in MHz. The GPU's pixel clock is limited
/// to the EDID's, as `EdidProfile::limit_caps` does.
pub fn edid_tubes() -> Vec<Display> {
    let tubes = [
        // A 14 inch VGA tube that holds only 31 kHz.
        ("edid_31", (31, 32), (50, 75), 40),
        // A 15 inch tube, 1024x768 at 60 Hz.
        ("edid_30_54", (30, 54), (50, 120), 80),
        // A 17 inch tube, 1280x1024 at 60 Hz.
        ("edid_30_70", (30, 70), (50, 160), 110),
        // A 21 inch tube, 1600x1200 at 75 Hz.
        ("edid_30_96", (30, 96), (50, 160), 230),
    ];
    tubes
        .iter()
        .map(|&(id, h, v, pclock_mhz)| Display {
            profile: edid_tube(id, h, v),
            caps: GpuCaps {
                pclock_max_hz: pclock_mhz * 1_000_000,
                ..open_caps()
            },
        })
        .collect()
}

/// The CRT profile of `profile_from_edid` for range limits of `h_khz`
/// and `v_hz`. A copy of `crt_range` and the `gtf` module of
/// neptron-platform, which this crate may not depend on; the test
/// `the_edid_copy_gives_the_platform_profile` holds it to the platform's
/// own result for a 30 to 71 kHz tube.
pub fn edid_tube(id: &str, h_khz: (u64, u64), v_hz: (u64, u64)) -> MonitorProfile {
    const NS_MHZ: u128 = 1_000_000_000_000;
    let (lo, hi) = (
        u128::from(h_khz.0) * 1_000_000,
        u128::from(h_khz.1) * 1_000_000,
    );
    let (v_min, v_max) = (u128::from(v_hz.0) * 1_000, u128::from(v_hz.1) * 1_000);
    // GTF defaults: C' = 30 %, M' = 300, sync 8 % of the line.
    let back_a = 30 * NS_MHZ;
    let front_a = (30 - 16) * NS_MHZ;
    let b = 300 * 1_000_000_000_000_000_000_u128;
    let half_blank = |a: u128| {
        if a * lo <= 2 * b && 2 * b <= a * hi {
            (a * a).div_ceil(4 * b * 200)
        } else {
            let at = |h: u128| (a * h).saturating_sub(b).div_ceil(200 * h * h);
            at(lo).max(at(hi))
        }
    };
    let v_front = NS_MHZ.div_ceil(lo);
    let v_sync = (3 * NS_MHZ).div_ceil(lo);
    let v_back = 550_000_u128.saturating_sub(3 * NS_MHZ / hi);
    let blank = v_front + v_sync + v_back;
    let active = |v: u128, h: u128, up: bool| {
        let numerator = (NS_MHZ - blank * v) * h;
        let denominator = v * NS_MHZ;
        if up {
            numerator.div_ceil(denominator)
        } else {
            numerator / denominator
        }
    };
    let narrow = |x: u128| u32::try_from(x).unwrap();
    let lines = |x: u128| u16::try_from(x).unwrap();
    MonitorProfile {
        id: id.into(),
        kind: MonitorKind::Crt {
            ranges: vec![CrtRange {
                h_min_mhz: u64::try_from(lo).unwrap(),
                h_max_mhz: u64::try_from(hi).unwrap(),
                v_min_mhz: u64::try_from(v_min).unwrap(),
                v_max_mhz: u64::try_from(v_max).unwrap(),
                h_front_porch_ns: narrow(half_blank(front_a)),
                h_sync_ns: narrow((8 * NS_MHZ).div_ceil(100 * lo)),
                h_back_porch_ns: narrow(half_blank(back_a)),
                v_front_porch_ns: narrow(v_front),
                v_sync_ns: narrow(v_sync),
                v_back_porch_ns: narrow(v_back),
                hsync_positive: false,
                vsync_positive: true,
                progressive_lines_min: lines(active(v_max, lo, true).max(1)),
                progressive_lines_max: lines(active(v_min, hi, false)),
                interlaced_lines_min: 0,
                interlaced_lines_max: 0,
            }],
        },
    }
}

/// Flat panels with their preferred mode at 60 Hz (CEA-861 or CVT reduced
/// blanking timings), and the 1080p panel of the policy goldens.
pub fn panels() -> Vec<Display> {
    let mode = |pclock_hz, h: [u16; 4], v: [u16; 4]| Modeline {
        pclock_hz,
        hactive: h[0],
        hsync_start: h[1],
        hsync_end: h[2],
        htotal: h[3],
        vactive: v[0],
        vsync_start: v[1],
        vsync_end: v[2],
        vtotal: v[3],
        interlace: false,
        doublescan: false,
        hsync_positive: true,
        vsync_positive: true,
    };
    let panel = |id: &str, modes: Vec<Modeline>| Display {
        profile: MonitorProfile {
            id: id.into(),
            kind: MonitorKind::FlatPanel { modes },
        },
        caps: open_caps(),
    };
    vec![
        panel(
            "panel_720",
            vec![mode(
                74_250_000,
                [1280, 1390, 1430, 1650],
                [720, 725, 730, 750],
            )],
        ),
        panel(
            "panel_768",
            vec![mode(
                85_500_000,
                [1366, 1436, 1579, 1792],
                [768, 771, 774, 798],
            )],
        ),
        Display {
            profile: panel_1080(),
            caps: open_caps(),
        },
        panel(
            "panel_1440",
            vec![mode(
                241_500_000,
                [2560, 2608, 2640, 2720],
                [1440, 1443, 1448, 1481],
            )],
        ),
        panel(
            "panel_2160",
            vec![mode(
                594_000_000,
                [3840, 4016, 4104, 4400],
                [2160, 2168, 2178, 2250],
            )],
        ),
    ]
}

/// The largest shape error that a cell may have and still keep the game's
/// shape, in per mille: about one pixel of a 240-pixel side, and the
/// rounding of a mode width to whole pixels.
pub const SHAPE_ERROR_MAX_PERMILLE: i64 = 20;

/// One cell of the table.
pub struct Cell {
    /// The winner's rung, or "none".
    pub rung: String,
    /// Active size and scan, "240x480i".
    pub mode: String,
    /// Total lines of the mode (a frame of both fields when interlaced).
    pub total: u16,
    pub line_rate_mhz: u64,
    pub field_rate_mhz: u64,
    /// The game rate is kept: within 5 mHz on a CRT, or the speed change
    /// in percent on a panel or a TV.
    pub rate: String,
    /// The vertical scale: a whole factor, or a fraction for a fit.
    pub scale: String,
    /// The filter of the game picture: "nearest" or "sharp".
    pub filter: &'static str,
    /// The game's share of the active lines of the mode, in per mille.
    pub height_permille: u32,
    /// The black lines above and below the game, in lines of the mode.
    pub border_lines: u32,
    /// The shape on the screen against the game's own, in per mille: 0 is
    /// the right shape, above 0 is too wide.
    pub shape_error_permille: i64,
    /// The largest whole vertical scale of a proposal in the plan that
    /// keeps the game rate and that the validator accepts, as "3x 960p".
    pub best_whole: String,
    /// "ok" when the winner follows the arcade rule of the spec, else the
    /// first rule it breaks.
    pub rule: &'static str,
}

/// The cell of one board on one display, with the default settings.
pub fn cell(board: &Board, display: &Display) -> Cell {
    let (profile, caps) = (&display.profile, &display.caps);
    let plan = decide(&board.source(), &PolicySettings::default(), profile, caps)
        .unwrap_or_else(|e| panic!("{} on {}: {e}", board.name, profile.id));
    let best = best_whole(&plan, profile, caps);
    let best_whole = best.map_or_else(
        || if is_crt(profile) { "none" } else { "-" }.to_owned(),
        |(k, progressive, h)| format!("{k}x {h}{}", if progressive { "p" } else { "i" }),
    );
    let choice = choose(&plan, |m| validate(m, profile, caps));
    let Some(winner) = choice.winner else {
        return Cell {
            rung: "none".into(),
            mode: "-".into(),
            total: 0,
            line_rate_mhz: 0,
            field_rate_mhz: 0,
            rate: "-".into(),
            scale: "-".into(),
            filter: "-",
            height_permille: 0,
            border_lines: 0,
            shape_error_permille: 0,
            best_whole,
            rule: "no mode",
        };
    };
    let p = winner.proposal;
    let m = &p.mode;
    if is_crt(profile) {
        assert!(
            m.pclock_is_whole_khz(),
            "{} on {}: pixel clock {} Hz",
            board.name,
            profile.id,
            m.pclock_hz
        );
    }
    let d = p.placement.dest;
    let game_lines = u32::from(plan.source.height) - 2 * crop_lines(p);
    let scale = match p.placement.scale.y {
        Some(k) => format!("{k}"),
        None => format!("{:.2}", f64::from(d.h) / f64::from(game_lines)),
    };
    let shape_error_permille = shape_error(&plan, profile, p);
    Cell {
        rung: winner.considered.rung.to_string(),
        mode: format!(
            "{}x{}{}",
            m.hactive,
            m.vactive,
            if m.interlace { "i" } else { "p" }
        ),
        total: m.vtotal,
        line_rate_mhz: p.timing.line_rate_mhz,
        field_rate_mhz: p.timing.field_rate_mhz,
        rate: rate(winner.considered, p),
        scale,
        filter: match p.placement.filter {
            Filter::Nearest => "nearest",
            Filter::SharpBilinear => "sharp",
        },
        height_permille: d.h * 1_000 / u32::from(m.vactive),
        border_lines: u32::from(m.vactive).saturating_sub(d.h),
        shape_error_permille,
        best_whole,
        rule: rule(
            board,
            profile,
            winner.considered,
            p,
            shape_error_permille,
            best,
        ),
    }
}

/// The first rule of the arcade rule set that the winner breaks, or "ok".
/// The rules, in order: a mode exists; no soft fit; the game keeps its
/// shape; on a CRT, the game's rate or twice it when a range covers one of
/// them; on a CRT, no smaller whole scale than the largest one that keeps
/// the rate. A panel and a TV run at their own rate, and their whole scale
/// is the largest that fits by construction.
fn rule(
    board: &Board,
    profile: &MonitorProfile,
    considered: &Considered,
    p: &Proposal,
    shape_error_permille: i64,
    best: Option<(u16, bool, u32)>,
) -> &'static str {
    if considered.rung.is_fitted() {
        return "soft fit";
    }
    if shape_error_permille.abs() > SHAPE_ERROR_MAX_PERMILLE {
        return "shape";
    }
    let MonitorKind::Crt { ranges } = &profile.kind else {
        return "ok";
    };
    if p.timing.changes_speed() {
        let game = u64::from(board.refresh_mhz);
        let covered = |rate: u64| {
            ranges
                .iter()
                .any(|r| r.v_min_mhz <= rate && rate <= r.v_max_mhz)
        };
        return if covered(game) || covered(2 * game) {
            "rate"
        } else {
            "ok"
        };
    }
    match (p.placement.scale.y, best) {
        (Some(k), Some((largest, _, _))) if k < largest => "smaller scale",
        _ => "ok",
    }
}

fn is_crt(profile: &MonitorProfile) -> bool {
    matches!(profile.kind, MonitorKind::Crt { .. })
}

fn crop_lines(p: &Proposal) -> u32 {
    p.overscan_crop.map_or(0, |c| u32::from(c.lines_per_edge))
}

fn rate(considered: &Considered, p: &Proposal) -> String {
    let kind = considered.target.kind;
    let shown = p.timing.field_rate_mhz as f64;
    let game = (u64::from(p.timing.game_mhz) * u64::from(p.timing.repeat())) as f64;
    match kind {
        RefreshKind::Panel | RefreshKind::Tv => {
            format!("{:+.2} %", (shown / game - 1.0) * 100.0)
        }
        _ if !p.timing.changes_speed() && p.timing.repeat() == 2 => "exact x2".into(),
        _ if !p.timing.changes_speed() => "exact".into(),
        _ => format!("{kind} {:+.2} %", (shown / game - 1.0) * 100.0),
    }
}

/// The shape on the screen over the game's own shape, less 1, in per
/// mille. A CRT shows its active area on a 4:3 face; a TV shows 704 of
/// its 720 samples as 4:3; a panel has square pixels.
fn shape_error(plan: &Plan, profile: &MonitorProfile, p: &Proposal) -> i64 {
    let d = p.placement.dest;
    let m = &p.mode;
    let height_share = f64::from(d.h) / f64::from(m.vactive);
    let shown = match profile.kind {
        MonitorKind::FlatPanel { .. } => f64::from(d.w) / f64::from(d.h),
        MonitorKind::Tv { .. } => f64::from(d.w) / 704.0 * 4.0 / 3.0 / height_share,
        MonitorKind::Crt { .. } => f64::from(d.w) / f64::from(m.hactive) * 4.0 / 3.0 / height_share,
    };
    let s = plan.source;
    let own = if s.aspect_milli == 0 {
        f64::from(s.width) / f64::from(s.height)
    } else {
        f64::from(s.aspect_milli) / 1_000.0
    };
    // The crop hides lines, so the game's own shape is that of the rest.
    let own = own * f64::from(s.height) / f64::from(u32::from(s.height) - 2 * crop_lines(p));
    ((shown / own - 1.0) * 1_000.0).round() as i64
}

/// The largest whole vertical scale among the exact proposals of a CRT
/// plan that keep the game rate and that the validator accepts: the
/// factor, whether the mode is progressive, and the game's lines.
fn best_whole(plan: &Plan, profile: &MonitorProfile, caps: &GpuCaps) -> Option<(u16, bool, u32)> {
    if !is_crt(profile) {
        return None;
    }
    plan.proposals()
        .filter(|(_, c, p)| {
            matches!(c.rung, Rung::Native | Rung::Multiple(_) | Rung::Interlaced)
                && !p.timing.changes_speed()
                && validate(&p.mode, profile, caps).is_ok()
        })
        .filter_map(|(_, _, p)| {
            p.placement
                .scale
                .y
                .map(|k| (k, !p.mode.interlace, p.placement.dest.h))
        })
        .max()
}

fn khz(mhz: u64) -> String {
    format!("{}.{:03}", mhz / 1_000_000, mhz % 1_000_000 / 1_000)
}

fn hz(mhz: u64) -> String {
    format!("{}.{:03}", mhz / 1_000, mhz % 1_000)
}

fn rotation(rotation: Rotation) -> &'static str {
    match rotation {
        Rotation::R0 => "R0",
        Rotation::R90 => "R90",
        Rotation::R180 => "R180",
        Rotation::R270 => "R270",
    }
}

/// The table of one board, as Markdown.
pub fn board_table(board: &Board, displays: &[Display]) -> String {
    let mut out = format!(
        "### `{}`: {}x{} frame, {}, {} Hz\n\n",
        board.name,
        board.width,
        board.height,
        rotation(board.rotation),
        hz(u64::from(board.refresh_mhz))
    );
    out.push_str(
        "| Display | Rung | Mode | Total | kHz | Hz | Rate | Scale | Filter | Height | Border | Shape | Best whole | Rule |\n",
    );
    out.push_str(
        "| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |\n",
    );
    for display in displays {
        let c = cell(board, display);
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {}.{} % | {} | {}{}.{} % | {} | {} |\n",
            display.profile.id,
            c.rung,
            c.mode,
            c.total,
            khz(c.line_rate_mhz),
            hz(c.field_rate_mhz),
            c.rate,
            c.scale,
            c.filter,
            c.height_permille / 10,
            c.height_permille % 10,
            c.border_lines,
            if c.shape_error_permille < 0 { "-" } else { "+" },
            c.shape_error_permille.abs() / 10,
            c.shape_error_permille.abs() % 10,
            c.best_whole,
            c.rule,
        ));
    }
    out
}

#[test]
fn the_edid_copy_gives_the_platform_profile() {
    let mut built = edid_tube("edid_npt_0071", (30, 71), (50, 160));
    built.id = pc_crt_30_71().id;
    assert_eq!(built, pc_crt_30_71());
}

/// Every cell has a plan, and every CRT winner a whole-kHz pixel clock
/// (asserted in [`cell`]). The output is the table of docs/spec/arcade.md.
#[test]
fn arcade_table() {
    let displays = displays();
    let mut out = String::new();
    for board in &BOARDS {
        out.push_str(&board_table(board, &displays));
        out.push('\n');
    }
    println!("{out}");
}
