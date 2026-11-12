//! The engine on flat panels of every common shape: the mode, where the
//! game and the menu sit, the filter, and the pixels the renderer makes.
//!
//! Every rectangle is worked out by hand from the rule of the policy: the
//! largest whole vertical factor `k` at which the game's 4:3 width fits,
//! the width `round(k * height * 1333 / 1000)`, centred (the margin
//! rounded down); the 320x240 menu at the largest whole factor that fits
//! both axes, centred.

use neptron_platform::EdidProfile;
use neptron_testkit::{FakeDisplay, SimClock};
use neptron_types::{
    Filter, Frame, GpuCaps, Modeline, MonitorKind, MonitorProfile, PixelFormat, Rect,
    SourceGeometry,
};
use neptron_video::{Monitor, PolicySettings, VideoEngine, VideoStatusText};

use crate::harness::{SNES, SNES_PAL, frame, source};

/// A GPU that sends 4K at 60 Hz.
const CAPS_4K: GpuCaps = GpuCaps {
    pclock_min_hz: 25_000_000,
    pclock_max_hz: 600_000_000,
    interlace: false,
    flip_per_field: false,
    max_width: 4_096,
    max_height: 4_096,
};

/// Mega Drive H40: 320x224 at 59.922 Hz, 4:3.
const MD_H40: SourceGeometry = source(320, 224, 59_922);
/// A 320x240 game at 60 Hz, 4:3.
const GAME_240: SourceGeometry = source(320, 240, 60_000);

/// A progressive mode near `rate_hz`, with the blanking of CTA-861 1080p
/// scaled to the size; only the size and the rate matter to a panel. The
/// clock is rounded down to whole kHz, as every mode on the wire has it.
fn mode(width: u16, height: u16, rate_hz: u32) -> Modeline {
    let htotal = width + width / 8;
    let vtotal = height + height / 24;
    let pclock_hz = u32::from(htotal) * u32::from(vtotal) * rate_hz;
    Modeline {
        pclock_hz: pclock_hz / 1_000 * 1_000,
        hactive: width,
        hsync_start: width + 8,
        hsync_end: width + 16,
        htotal,
        vactive: height,
        vsync_start: height + 2,
        vsync_end: height + 4,
        vtotal,
        interlace: false,
        doublescan: false,
        hsync_positive: true,
        vsync_positive: true,
    }
}

fn panel(modes: Vec<Modeline>) -> Monitor {
    let pclock_max_hz = modes.iter().map(|m| m.pclock_hz).max().unwrap();
    Monitor::Edid(EdidProfile {
        profile: MonitorProfile {
            id: "panel_npt_0001".to_owned(),
            kind: MonitorKind::FlatPanel { modes },
        },
        pclock_max_hz,
        formula: None,
        name: None,
    })
}

fn engine(monitor: Monitor) -> (FakeDisplay, VideoEngine<FakeDisplay>) {
    let fake = FakeDisplay::new(SimClock::new(), CAPS_4K);
    let engine = VideoEngine::new(fake.clone(), monitor, PolicySettings::default());
    (fake, engine)
}

/// The mode, the game's rectangle, the menu's rectangle and the filter
/// that the engine shows for `source` on `monitor`.
fn shown(monitor: Monitor, source: SourceGeometry) -> (Modeline, Rect, Rect, Filter) {
    let (_, mut engine) = engine(monitor);
    engine.begin_frame(source);
    engine.present(Some(frame(&source)), None).unwrap();
    engine.wait_flip().unwrap();
    let current = engine.current().unwrap();
    let placement = current.chosen.proposal.placement;
    (
        current.read_back,
        placement.dest,
        placement.ui_dest,
        placement.filter,
    )
}

fn rect(x: i32, y: i32, w: u32, h: u32) -> Rect {
    Rect { x, y, w, h }
}

/// One shape: the panel size, then for SNES 256x224, Mega Drive 320x224
/// and a 320x240 game the game's rectangle, and the menu's rectangle.
type Shape = ((u16, u16), Rect, Rect, Rect);

#[test]
fn a_4_3_game_keeps_its_aspect_on_every_panel_shape() {
    let shapes: [Shape; 8] = [
        // 16:9 1080p: k = 4, 896 lines, 1194 wide, (363, 92); 240 lines:
        // 960, 1280 wide, (320, 60). Menu: min(6, 4) = 4.
        (
            (1920, 1080),
            rect(363, 92, 1194, 896),
            rect(320, 60, 1280, 960),
            rect(320, 60, 1280, 960),
        ),
        // 16:9 1440p: k = 6, 1344 lines, 1792 wide, (384, 48); 240 lines:
        // 1440, 1920 wide, (320, 0). Menu: min(8, 6) = 6.
        (
            (2560, 1440),
            rect(384, 48, 1792, 1344),
            rect(320, 0, 1920, 1440),
            rect(320, 0, 1920, 1440),
        ),
        // 16:9 2160p: k = 9, 2016 lines, 2687 wide, (576, 72); 240 lines:
        // 2160, 2879 wide, (480, 0). Menu: min(12, 9) = 9.
        (
            (3840, 2160),
            rect(576, 72, 2687, 2016),
            rect(480, 0, 2879, 2160),
            rect(480, 0, 2880, 2160),
        ),
        // 16:9 720p: k = 3, 672 lines, 896 wide, (192, 24); 240 lines: 720,
        // 960 wide, (160, 0). Menu: 3.
        (
            (1280, 720),
            rect(192, 24, 896, 672),
            rect(160, 0, 960, 720),
            rect(160, 0, 960, 720),
        ),
        // 16:10 1920x1200: k = 5, 1120 lines, 1493 wide, (213, 40); 240
        // lines: 1200, 1600 wide, (160, 0). Menu: min(6, 5) = 5.
        (
            (1920, 1200),
            rect(213, 40, 1493, 1120),
            rect(160, 0, 1600, 1200),
            rect(160, 0, 1600, 1200),
        ),
        // 21:9 2560x1080: k = 4, as 1080p, centred in 2560. Menu: 4.
        (
            (2560, 1080),
            rect(683, 92, 1194, 896),
            rect(640, 60, 1280, 960),
            rect(640, 60, 1280, 960),
        ),
        // 5:4 1280x1024: k = 4, 896 lines, 1194 wide, (43, 64); 240 lines:
        // 960, 1280 wide, (0, 32). Menu: 4.
        (
            (1280, 1024),
            rect(43, 64, 1194, 896),
            rect(0, 32, 1280, 960),
            rect(0, 32, 1280, 960),
        ),
        // 4:3 1024x768: k = 3, 672 lines, 896 wide, (64, 48); 240 lines:
        // 720, 960 wide, (32, 24). Menu: 3.
        (
            (1024, 768),
            rect(64, 48, 896, 672),
            rect(32, 24, 960, 720),
            rect(32, 24, 960, 720),
        ),
    ];
    for ((w, h), dest_224, dest_240, ui) in shapes {
        let monitor = panel(vec![mode(w, h, 60)]);
        for (source, dest) in [(SNES, dest_224), (MD_H40, dest_224), (GAME_240, dest_240)] {
            let (shown_mode, shown_dest, shown_ui, filter) = shown(monitor.clone(), source);
            let what = format!("{}x{} on {w}x{h}", source.width, source.height);
            assert_eq!((shown_mode.hactive, shown_mode.vactive), (w, h), "{what}");
            assert_eq!(shown_dest, dest, "{what}");
            assert_eq!(shown_ui, ui, "{what}");
            let whole_x = dest.w % u32::from(source.width) == 0;
            let expected = if whole_x {
                Filter::Nearest
            } else {
                Filter::SharpBilinear
            };
            assert_eq!(filter, expected, "{what}");
            assert!(
                dest.w >= u32::from(source.width) && dest.h >= u32::from(source.height),
                "{what}: a panel never scales a game down, so sharp bilinear never blurs"
            );
        }
    }
}

#[test]
fn a_50_hz_game_takes_the_50_hz_mode_of_the_native_size() {
    let monitor = panel(vec![
        mode(1920, 1080, 60),
        mode(1920, 1080, 50),
        mode(1280, 720, 60),
        mode(1280, 720, 50),
    ]);
    let (pal, _, _, _) = shown(monitor.clone(), SNES_PAL);
    assert_eq!((pal.hactive, pal.field_rate_mhz()), (1920, 50_000));
    let (ntsc, _, _, _) = shown(monitor.clone(), SNES);
    assert_eq!((ntsc.hactive, ntsc.field_rate_mhz()), (1920, 60_000));
    // 720p would fill all 720 lines with a 240-line game at 3x, more than
    // 960 of 1080 at 4x, but the native size comes first: the panel does
    // no scaling of its own.
    let (native, dest, _, _) = shown(monitor, GAME_240);
    assert_eq!((native.hactive, native.vactive), (1920, 1080));
    assert_eq!(dest, rect(320, 60, 1280, 960));
}

#[test]
fn snes_on_1080p_is_sharp_with_no_alert() {
    let (_, mut engine) = engine(panel(vec![mode(1920, 1080, 60)]));
    engine.begin_frame(SNES);
    engine.present(Some(frame(&SNES)), None).unwrap();
    engine.wait_flip().unwrap();
    let mut text = VideoStatusText::default();
    let status = engine.video_status(&mut text).unwrap();
    assert_eq!(status.game, "256x224             60.099 Hz");
    assert_eq!(status.mode, "1920x1080p 67.50 kHz 60.000 Hz  panel");
    assert_eq!(status.result, "sharp");
    assert!(!status.alert);
}

#[test]
fn sharp_bilinear_blends_only_a_narrow_band_at_each_source_edge() {
    // SNES columns black and white in turn: 256 columns, 255 edges, over
    // 1194 pixels (4.66 per column). The prescale is floor(4.66) = 4, so
    // a pixel blends only when its centre is within 1 / (2 * 4) of a
    // source pixel of an edge: a band of 1 / 4 source pixel, 1.17 screen
    // pixels. It touches one pixel, or two with small weights when the
    // edge falls between two pixel centres; the rest are pure.
    let pixels: Vec<u8> = (0..224)
        .flat_map(|_| (0..256u16).flat_map(|x| if x % 2 == 0 { [0, 0] } else { [0xff, 0xff] }))
        .collect();
    let game = Frame {
        data: &pixels,
        width: 256,
        height: 224,
        pitch: 512,
        format: PixelFormat::Rgb565,
    };
    let (fake, mut engine) = engine(panel(vec![mode(1920, 1080, 60)]));
    engine.begin_frame(SNES);
    engine.present(Some(game), None).unwrap();
    engine.wait_flip().unwrap();
    let screen = fake.screen().unwrap();
    let dest = rect(363, 92, 1194, 896);
    let y = 92 + 448;
    let mut blended = 0;
    let mut run = 0;
    for x in 0..1920u32 {
        let p = screen.pixel(x, y).unwrap();
        let inside = (363..363 + 1194).contains(&x);
        if !inside {
            assert_eq!(p, [0, 0, 0], "border at {x}");
            continue;
        }
        let pure = p == [0, 0, 0] || p == [255, 255, 255];
        if pure {
            run = 0;
        } else {
            blended += 1;
            run += 1;
            assert!(run <= 2, "three blended pixels in a row at {x}");
        }
    }
    // 255 bands of 1194 / 1024 screen pixels each: at most 298.
    assert!(blended <= 298, "{blended} blended pixels for 255 edges");
    assert!(blended > 0, "a fractional scale blends at some edges");
    // Each line of the game is 4 lines of the screen: no blend between
    // lines.
    for row in 0..dest.h {
        let a = screen.pixel(400, 92 + row).unwrap();
        let b = screen.pixel(400, 92 + (row / 4) * 4).unwrap();
        assert_eq!(a, b, "line {row}");
    }
}
