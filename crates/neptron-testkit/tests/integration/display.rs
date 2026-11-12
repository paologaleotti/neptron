//! The fake display against the `Display` contract, its timing and its
//! faults.

use std::collections::BTreeMap;

use neptron_backend::{Clock, Display, DisplayError};
use neptron_gfx::{OutputBuffer, ReferenceRenderer};
use neptron_safety::ValidatedMode;
use neptron_testkit::preset::{CAPS_LOW_CLOCK, MODE_240P, MODE_288P, MODE_480I, validated_mode};
use neptron_testkit::{
    DEFAULT_FLIP_TIMEOUT_NS, DisplayOptions, FakeDisplay, FlipFault, Jitter, LateFlips, ModeFault,
    Render, SimClock, frame_hash,
};
use neptron_types::{
    Filter, FlipEvent, Frame, GpuCaps, Modeline, PixelFormat, Rect, ScanlineKind, Scanlines, Scene,
};

/// One period of 240p at 60.029 Hz, in nanoseconds: 10^12 / 60 029,
/// rounded down.
const PERIOD_240P_NS: u64 = 16_658_615;

/// The nominal time of blank `k` of 240p after a mode set at 0.
fn blank_240p(k: u64) -> u64 {
    k * 1_000_000_000_000 / 60_029
}

fn mode(m: Modeline) -> ValidatedMode {
    validated_mode(&m, "generic_15", &CAPS_LOW_CLOCK).unwrap()
}

/// A 4x2 XRGB8888 frame: red, green, blue and white on top, black below.
const PIXELS: [u32; 8] = [
    0x00FF_0000,
    0x0000_FF00,
    0x0000_00FF,
    0x00FF_FFFF,
    0,
    0,
    0,
    0,
];

fn bytes(pixels: &[u32]) -> Vec<u8> {
    pixels.iter().flat_map(|p| p.to_ne_bytes()).collect()
}

fn scene(data: &[u8]) -> Scene<'_> {
    Scene {
        game: Some(Frame {
            data,
            width: 4,
            height: 2,
            pitch: 16,
            format: PixelFormat::Xrgb8888,
        }),
        crop: Rect {
            x: 0,
            y: 0,
            w: 4,
            h: 2,
        },
        dest: Rect {
            x: 0,
            y: 0,
            w: 320,
            h: 240,
        },
        filter: Filter::Nearest,
        scanlines: Scanlines::NONE,
        ui: None,
        ui_dest: Rect {
            x: 0,
            y: 0,
            w: 0,
            h: 0,
        },
    }
}

fn empty() -> Scene<'static> {
    Scene {
        game: None,
        crop: Rect {
            x: 0,
            y: 0,
            w: 0,
            h: 0,
        },
        dest: Rect {
            x: 0,
            y: 0,
            w: 0,
            h: 0,
        },
        filter: Filter::Nearest,
        scanlines: Scanlines::NONE,
        ui: None,
        ui_dest: Rect {
            x: 0,
            y: 0,
            w: 0,
            h: 0,
        },
    }
}

fn timing_only(clock: &SimClock, options: DisplayOptions) -> FakeDisplay {
    FakeDisplay::with_options(
        clock.clone(),
        CAPS_LOW_CLOCK,
        DisplayOptions {
            render: Render::Off,
            ..options
        },
    )
}

fn frame(display: &mut FakeDisplay) -> Result<FlipEvent, DisplayError> {
    display.present(&empty())?;
    display.wait_flip()
}

#[test]
fn nothing_works_before_the_first_mode() {
    let clock = SimClock::new();
    let mut display = FakeDisplay::new(clock, CAPS_LOW_CLOCK);
    assert_eq!(display.caps(), &CAPS_LOW_CLOCK);
    assert_eq!(display.present(&empty()), Err(DisplayError::NoMode));
    assert_eq!(display.wait_flip(), Err(DisplayError::NoMode));
    assert_eq!(display.active_mode(), None);
    assert_eq!(display.frames().len(), 1);
    assert_eq!(display.frames()[0].present, Err(DisplayError::NoMode));
}

#[test]
fn flips_come_at_the_field_rate_of_the_mode() {
    let clock = SimClock::starting_at(1_000);
    let mut display = timing_only(&clock, DisplayOptions::default());
    assert_eq!(display.set_mode(&mode(MODE_240P)), Ok(MODE_240P));
    for k in 1..=3 {
        let flip = frame(&mut display).unwrap();
        assert_eq!(flip.sequence, k);
        assert_eq!(flip.time_ns, 1_000 + blank_240p(k));
        assert_eq!(clock.now_ns(), flip.time_ns);
    }
    assert_eq!(
        display.flip_times(),
        [1, 2, 3].map(|k| 1_000 + blank_240p(k))
    );
    assert_eq!(display.repeated_vblanks(), 0);
    assert!(display.late_frames().is_empty());
    assert!(display.dropped_frames().is_empty());
    assert_eq!(display.active_mode(), Some(MODE_240P));
}

/// 480i flips on caps with and without one flip per field.
fn interlaced_flips(flip_per_field: bool) -> (FakeDisplay, Vec<FlipEvent>) {
    let clock = SimClock::new();
    let caps = GpuCaps {
        flip_per_field,
        ..CAPS_LOW_CLOCK
    };
    let options = DisplayOptions {
        render: Render::Off,
        ..DisplayOptions::default()
    };
    let mut display = FakeDisplay::with_options(clock, caps, options);
    display.set_mode(&mode(MODE_480I)).unwrap();
    let flips = (0..4).map(|_| frame(&mut display).unwrap()).collect();
    (display, flips)
}

/// The nominal time of field `k` of 480i after a mode set at 0.
fn field_480i(k: u64) -> u64 {
    k * 1_000_000_000_000 / 59_915
}

#[test]
fn an_interlaced_mode_flips_every_field_with_flip_per_field() {
    let (display, flips) = interlaced_flips(true);
    let sequences: Vec<u64> = flips.iter().map(|f| f.sequence).collect();
    assert_eq!(sequences, [1, 2, 3, 4]);
    assert_eq!(flips[3].time_ns, field_480i(4));
    assert_eq!(display.repeated_vblanks(), 0);
}

#[test]
fn an_interlaced_mode_flips_once_per_frame_without_flip_per_field() {
    let (mut display, flips) = interlaced_flips(false);
    let sequences: Vec<u64> = flips.iter().map(|f| f.sequence).collect();
    assert_eq!(sequences, [2, 4, 6, 8]);
    assert_eq!(flips[3].time_ns, field_480i(8));
    assert_eq!(display.repeated_vblanks(), 0);
    assert!(display.late_frames().is_empty());
    assert_eq!(display.frames()[0].flip_interval, Some(2));

    // A late flip of one blank waits for the next frame start.
    display.fault_flip(4, FlipFault::Late { vblanks: 1 });
    assert_eq!(frame(&mut display).unwrap().sequence, 12);
    assert_eq!(display.late_frames()[0].late_by(), Some(2));
    assert_eq!(display.repeated_vblanks(), 2);

    // A programmed refresh takes a flip on every blank.
    display.set_refresh_mhz(Some(59_915));
    display.set_mode(&mode(MODE_480I)).unwrap();
    let a = frame(&mut display).unwrap();
    let b = frame(&mut display).unwrap();
    assert_eq!(b.sequence, a.sequence + 1);
}

#[test]
fn a_slow_frame_lands_on_a_later_blank() {
    let clock = SimClock::new();
    let mut display = timing_only(&clock, DisplayOptions::default());
    display.set_mode(&mode(MODE_240P)).unwrap();
    assert_eq!(frame(&mut display).unwrap().sequence, 1);
    clock.advance(PERIOD_240P_NS + 1_000);
    let flip = frame(&mut display).unwrap();
    assert_eq!(flip.sequence, 3);
    assert_eq!(display.repeated_vblanks(), 1);
    // The frame was presented after blank 2, so it is not a late flip.
    assert!(display.late_frames().is_empty());
    assert_eq!(display.frames()[1].target_sequence, Some(3));
}

#[test]
fn the_display_takes_one_flip_at_a_time() {
    let clock = SimClock::new();
    let mut display = timing_only(&clock, DisplayOptions::default());
    display.set_mode(&mode(MODE_240P)).unwrap();
    let busy = |operation| DisplayError::Device {
        operation,
        reason: "a flip is already queued".to_owned(),
    };
    assert_eq!(
        display.wait_flip(),
        Err(DisplayError::Device {
            operation: "wait_flip",
            reason: "no flip is queued".to_owned()
        })
    );
    display.present(&empty()).unwrap();
    assert_eq!(display.present(&empty()), Err(busy("present")));
    assert_eq!(display.set_mode(&mode(MODE_288P)), Err(busy("set_mode")));
    assert_eq!(display.active_mode(), Some(MODE_240P));
    assert!(display.wait_flip().is_ok());
    assert_eq!(display.set_mode(&mode(MODE_288P)), Ok(MODE_288P));
}

#[test]
fn sequence_numbers_continue_across_modes() {
    let clock = SimClock::new();
    let mut display = timing_only(
        &clock,
        DisplayOptions {
            modeset_ns: 5_000_000,
            ..DisplayOptions::default()
        },
    );
    display.set_mode(&mode(MODE_240P)).unwrap();
    frame(&mut display).unwrap();
    frame(&mut display).unwrap();
    // Two more blanks of 240p pass with no frame.
    clock.advance(2 * PERIOD_240P_NS + 10);
    let switch_ns = clock.now_ns();
    display.set_mode(&mode(MODE_288P)).unwrap();
    assert_eq!(clock.now_ns(), switch_ns + 5_000_000);
    let flip = frame(&mut display).unwrap();
    assert_eq!(flip.sequence, 5);
    // 288p runs at 50.409 Hz: 10^12 / 50 409 ns per field.
    assert_eq!(flip.time_ns, switch_ns + 5_000_000 + 19_837_727);
    let switches = display.mode_switches();
    assert_eq!(switches.len(), 2);
    assert_eq!(switches[1].time_ns, switch_ns);
    assert_eq!(switches[1].active, MODE_288P);
}

#[test]
fn mode_faults_keep_the_active_mode() {
    let clock = SimClock::new();
    let mut display = timing_only(&clock, DisplayOptions::default());
    display.fault_mode(
        1,
        ModeFault::Refuse {
            reason: "EINVAL".to_owned(),
        },
    );
    display.fault_mode(
        2,
        ModeFault::Device {
            reason: "ENODEV".to_owned(),
        },
    );
    display.set_mode(&mode(MODE_240P)).unwrap();
    assert_eq!(display.next_mode_call(), 1);
    assert_eq!(
        display.set_mode(&mode(MODE_288P)),
        Err(DisplayError::ModeRefused {
            mode: MODE_288P,
            reason: "EINVAL".to_owned()
        })
    );
    assert_eq!(
        display.set_mode(&mode(MODE_288P)),
        Err(DisplayError::Device {
            operation: "set_mode",
            reason: "ENODEV".to_owned()
        })
    );
    assert_eq!(display.active_mode(), Some(MODE_240P));
    assert_eq!(frame(&mut display).unwrap().time_ns, PERIOD_240P_NS);
    let calls = display.mode_calls();
    assert_eq!(calls.len(), 3);
    assert_eq!(calls[1].requested, MODE_288P);
    assert!(calls[2].result.is_err());
    assert_eq!(display.mode_switches().len(), 1);
}

#[test]
fn mode_faults_can_match_the_requested_mode() {
    let clock = SimClock::new();
    let mut display = timing_only(&clock, DisplayOptions::default());
    let refused = |reason: &str| ModeFault::Refuse {
        reason: reason.to_owned(),
    };
    display.fault_requested_mode(MODE_288P, refused("first"));
    display.fault_requested_mode(MODE_288P, refused("ERANGE"));
    let rounded = Modeline {
        pclock_hz: 6_000_000,
        ..MODE_240P
    };
    display.fault_requested_mode(MODE_240P, ModeFault::ReadBack(rounded));
    let refusal = Err(DisplayError::ModeRefused {
        mode: MODE_288P,
        reason: "ERANGE".to_owned(),
    });
    for _ in 0..3 {
        assert_eq!(display.set_mode(&mode(MODE_288P)), refusal);
        assert_eq!(display.set_mode(&mode(MODE_240P)), Ok(rounded));
    }
    // A fault by call number comes first.
    display.fault_mode(display.next_mode_call(), ModeFault::ReadBack(MODE_288P));
    assert_eq!(display.set_mode(&mode(MODE_288P)), Ok(MODE_288P));
    assert_eq!(display.active_mode(), Some(MODE_288P));
}

#[test]
fn a_clock_that_is_not_whole_khz_is_refused_and_the_mode_stays() {
    let clock = SimClock::new();
    let mut display = timing_only(&clock, DisplayOptions::default());
    display.set_mode(&mode(MODE_240P)).unwrap();
    let odd = Modeline {
        pclock_hz: MODE_240P.pclock_hz + 400,
        ..MODE_240P
    };
    // The validator judges the monitor and the GPU, not the step of the
    // kernel's clock field, so the token exists.
    let token = mode(odd);
    // A read back fault for the mode does not apply: the refusal comes
    // first, as in the KMS backend.
    display.fault_requested_mode(odd, ModeFault::ReadBack(MODE_240P));
    assert_eq!(
        display.set_mode(&token),
        Err(DisplayError::ModeRefused {
            mode: odd,
            reason: "the pixel clock 6700400 Hz is not a whole number of kHz".to_owned(),
        })
    );
    assert_eq!(display.active_mode(), Some(MODE_240P));
    assert!(frame(&mut display).is_ok());
    let whole = Modeline {
        pclock_hz: MODE_240P.pclock_hz + 1_000,
        ..MODE_240P
    };
    assert_eq!(display.set_mode(&mode(whole)), Ok(whole));
}

#[test]
fn the_mode_read_back_sets_the_flip_times() {
    let clock = SimClock::new();
    let mut display = timing_only(&clock, DisplayOptions::default());
    let rounded = Modeline {
        pclock_hz: 6_000_000,
        ..MODE_240P
    };
    display.fault_mode(0, ModeFault::ReadBack(rounded));
    assert_eq!(display.set_mode(&mode(MODE_240P)), Ok(rounded));
    assert_eq!(display.active_mode(), Some(rounded));
    let period = 1_000_000_000_000 / rounded.field_rate_mhz();
    assert_eq!(frame(&mut display).unwrap().time_ns, period);
    let switch = display.mode_switches()[0];
    assert_eq!((switch.requested, switch.active), (MODE_240P, rounded));
}

#[test]
fn a_programmed_refresh_replaces_the_field_rate() {
    let clock = SimClock::new();
    let mut display = timing_only(
        &clock,
        DisplayOptions {
            refresh_mhz: Some(50_000),
            ..DisplayOptions::default()
        },
    );
    display.set_mode(&mode(MODE_240P)).unwrap();
    assert_eq!(frame(&mut display).unwrap().time_ns, 20_000_000);
    display.set_refresh_mhz(Some(60_100));
    // The new rate applies from the next mode.
    assert_eq!(frame(&mut display).unwrap().time_ns, 40_000_000);
    display.set_mode(&mode(MODE_240P)).unwrap();
    assert_eq!(
        frame(&mut display).unwrap().time_ns,
        40_000_000 + 16_638_935
    );
    display.set_refresh_mhz(None);
    display.set_mode(&mode(MODE_240P)).unwrap();
    let start = clock.now_ns();
    assert_eq!(frame(&mut display).unwrap().time_ns, start + PERIOD_240P_NS);
}

#[test]
fn flip_faults() {
    let clock = SimClock::new();
    let mut display = timing_only(&clock, DisplayOptions::default());
    display.set_mode(&mode(MODE_240P)).unwrap();
    display.fault_flip(0, FlipFault::Late { vblanks: 2 });
    display.fault_flip(
        1,
        FlipFault::PresentFails {
            reason: "EIO".to_owned(),
        },
    );
    display.fault_flip(
        2,
        FlipFault::WaitFails {
            reason: "EIO".to_owned(),
        },
    );

    let late = frame(&mut display).unwrap();
    assert_eq!(late.sequence, 3);
    assert_eq!(display.late_frames()[0].late_by(), Some(2));

    assert_eq!(
        display.present(&empty()),
        Err(DisplayError::Device {
            operation: "present",
            reason: "EIO".to_owned()
        })
    );
    assert_eq!(display.next_present_index(), 2);
    let before = clock.now_ns();
    let failed = Err(DisplayError::Device {
        operation: "wait_flip",
        reason: "EIO".to_owned(),
    });
    assert_eq!(frame(&mut display), failed);
    assert_eq!(clock.now_ns(), before, "a device failure takes no time");
    // The flip stays queued, and the next wait returns its event.
    assert!(display.present(&empty()).is_err());
    let landed = display.wait_flip().unwrap();
    assert_eq!(landed.sequence, late.sequence + 1);
    assert_eq!(display.frames()[2].flip, Some(landed));
    assert_eq!(display.frames()[2].flip_error, failed.err());
    let after = frame(&mut display).unwrap();
    assert!(after.sequence > landed.sequence);

    let dropped: Vec<u64> = display.dropped_frames().iter().map(|f| f.index).collect();
    assert!(dropped.is_empty(), "{dropped:?}");
    assert_eq!(display.flips().len(), 3);
}

fn timed_out() -> Result<FlipEvent, DisplayError> {
    Err(DisplayError::Device {
        operation: "wait_flip",
        reason: format!("no flip event after {DEFAULT_FLIP_TIMEOUT_NS} ns"),
    })
}

#[test]
fn a_lost_flip_stays_queued_and_every_wait_times_out() {
    let clock = SimClock::new();
    let mut display = timing_only(&clock, DisplayOptions::default());
    display.set_mode(&mode(MODE_240P)).unwrap();
    display.fault_flip(0, FlipFault::Lost);
    display.present(&empty()).unwrap();
    for wait in 1..=3 {
        let before = clock.now_ns();
        assert_eq!(display.wait_flip(), timed_out(), "wait {wait}");
        assert_eq!(clock.now_ns(), before + DEFAULT_FLIP_TIMEOUT_NS);
        let busy = |operation| DisplayError::Device {
            operation,
            reason: "a flip is already queued".to_owned(),
        };
        assert_eq!(display.present(&empty()), Err(busy("present")));
        assert_eq!(display.set_mode(&mode(MODE_288P)), Err(busy("set_mode")));
    }
    assert_eq!(display.active_mode(), Some(MODE_240P));
    assert!(display.flips().is_empty());
    assert_eq!(display.frames()[0].flip_error, timed_out().err());
    assert_eq!(display.dropped_frames().len(), 1);
}

#[test]
fn a_slow_flip_lands_after_its_timeouts() {
    let clock = SimClock::new();
    let mut display = timing_only(&clock, DisplayOptions::default());
    display.set_mode(&mode(MODE_240P)).unwrap();
    display.fault_flip(0, FlipFault::Slow { timeouts: 2 });
    display.present(&empty()).unwrap();
    assert_eq!(display.wait_flip(), timed_out());
    assert!(display.present(&empty()).is_err(), "still busy");
    assert_eq!(display.wait_flip(), timed_out());
    assert_eq!(clock.now_ns(), 2 * DEFAULT_FLIP_TIMEOUT_NS);
    let landed = display.wait_flip().unwrap();
    // The first blank of 240p after 200 ms is blank 13, at 216.6 ms.
    assert_eq!(landed.sequence, 13);
    assert_eq!(landed.time_ns, blank_240p(13));
    assert_eq!(clock.now_ns(), landed.time_ns);
    assert_eq!(display.frames()[0].flip, Some(landed));
    let next = frame(&mut display).unwrap();
    assert_eq!(next.sequence, 14);
}

#[test]
fn a_slow_flip_whose_target_comes_after_the_timeouts_lands_on_time() {
    let clock = SimClock::new();
    let mut display = timing_only(
        &clock,
        DisplayOptions {
            flip_timeout_ns: 1_000_000,
            ..DisplayOptions::default()
        },
    );
    display.set_mode(&mode(MODE_240P)).unwrap();
    display.fault_flip(0, FlipFault::Slow { timeouts: 1 });
    display.present(&empty()).unwrap();
    assert!(display.wait_flip().is_err());
    assert_eq!(clock.now_ns(), 1_000_000);
    let landed = display.wait_flip().unwrap();
    assert_eq!((landed.sequence, landed.time_ns), (1, blank_240p(1)));
}

#[test]
fn seeded_jitter_moves_each_blank_inside_its_bound() {
    let clock = SimClock::new();
    let jitter = Jitter::Seeded {
        seed: 7,
        max_ns: 500_000,
    };
    let mut display = timing_only(
        &clock,
        DisplayOptions {
            jitter: jitter.clone(),
            ..DisplayOptions::default()
        },
    );
    display.set_mode(&mode(MODE_240P)).unwrap();
    let mut moved = 0;
    for k in 1..=200_u64 {
        let flip = frame(&mut display).unwrap();
        let nominal = blank_240p(k);
        let offset = flip.time_ns as i64 - nominal as i64;
        assert!(offset.abs() <= 500_000, "blank {k}: {offset}");
        moved += u64::from(offset != 0);
    }
    assert!(moved > 190);

    let script = Jitter::Script(BTreeMap::from([(2, 1_500_000)]));
    let clock = SimClock::new();
    let mut display = timing_only(
        &clock,
        DisplayOptions {
            jitter: script,
            ..DisplayOptions::default()
        },
    );
    display.set_mode(&mode(MODE_240P)).unwrap();
    frame(&mut display).unwrap();
    let late_blank = frame(&mut display).unwrap();
    assert_eq!(late_blank.time_ns, blank_240p(2) + 1_500_000);
}

#[test]
fn jitter_of_half_a_period_is_refused() {
    let clock = SimClock::new();
    let mut display = timing_only(
        &clock,
        DisplayOptions {
            jitter: Jitter::Seeded {
                seed: 1,
                max_ns: PERIOD_240P_NS / 2 + 1,
            },
            ..DisplayOptions::default()
        },
    );
    let err = display.set_mode(&mode(MODE_240P)).unwrap_err();
    let DisplayError::Device { operation, reason } = err else {
        panic!("expected a device error, got {err:?}");
    };
    assert_eq!(operation, "set_mode");
    assert!(reason.starts_with(
        "a jitter of 8329308 ns needs a period above 16658616 ns; 60029 mHz gives 16658615 ns"
    ));
    assert_eq!(display.active_mode(), None);
}

#[test]
fn seeded_late_flips_follow_the_seed() {
    let run = || {
        let clock = SimClock::new();
        let mut display = timing_only(
            &clock,
            DisplayOptions {
                late_flips: Some(LateFlips {
                    seed: 99,
                    per_million: 50_000,
                    max_vblanks: 3,
                }),
                ..DisplayOptions::default()
            },
        );
        display.set_mode(&mode(MODE_240P)).unwrap();
        for _ in 0..1_000 {
            frame(&mut display).unwrap();
        }
        display.late_frames()
    };
    let late = run();
    assert!((20..=90).contains(&late.len()), "{} late flips", late.len());
    assert!(late.iter().all(|f| (1..=3).contains(&f.late_by().unwrap())));
    assert_eq!(late, run());
}

#[test]
fn present_renders_the_scene_with_the_reference_renderer() {
    let clock = SimClock::new();
    let options = DisplayOptions {
        render: Render::Pixels,
        ..DisplayOptions::default()
    };
    let mut display = FakeDisplay::with_options(clock, CAPS_LOW_CLOCK, options);
    display.set_mode(&mode(MODE_240P)).unwrap();
    let data = bytes(&PIXELS);
    display.capture(0);
    display.present(&scene(&data)).unwrap();
    assert_eq!(display.screen(), None);
    display.wait_flip().unwrap();

    let mut expected = vec![0_u8; 320 * 240 * 4];
    ReferenceRenderer::new()
        .render(
            &scene(&data),
            &mut OutputBuffer {
                data: &mut expected,
                width: 320,
                height: 240,
                pitch: 1280,
            },
        )
        .unwrap();
    assert_eq!(display.frame_hashes(), [frame_hash(&expected)]);

    let shown = display.screen().unwrap();
    assert_eq!((shown.width(), shown.height()), (320, 240));
    assert_eq!(shown.pixel(0, 0), Some([0xFF, 0, 0]));
    assert_eq!(shown.pixel(319, 0), Some([0xFF, 0xFF, 0xFF]));
    assert_eq!(shown.pixel(0, 239), Some([0, 0, 0]));
    assert_eq!(display.captured(0), Some(shown));
    assert_eq!(display.captured(1), None);
    let summary = display.frames()[0].scene;
    assert_eq!(summary.game, Some((4, 2, PixelFormat::Xrgb8888)));
    assert_eq!(summary.ui, None);
}

#[test]
fn a_broken_scene_is_refused_with_or_without_rendering() {
    for render in [Render::Pixels, Render::Scene, Render::Off] {
        let clock = SimClock::new();
        let options = DisplayOptions {
            render,
            ..DisplayOptions::default()
        };
        let mut display = FakeDisplay::with_options(clock, CAPS_LOW_CLOCK, options);
        display.set_mode(&mode(MODE_240P)).unwrap();
        let short = vec![0_u8; 8];
        let err = display.present(&scene(&short)).unwrap_err();
        assert!(
            matches!(&err, DisplayError::SceneRefused { reason } if reason.contains("holds 8 bytes")),
            "{err:?}"
        );
        // Nothing is queued after a refused scene.
        assert!(display.wait_flip().is_err());
        assert_eq!(display.frames()[0].hash, None);
    }
}

#[test]
fn scene_mode_shows_the_same_pixels_and_hashes_the_scene() {
    let shown = |render| {
        let clock = SimClock::new();
        let options = DisplayOptions {
            render,
            ..DisplayOptions::default()
        };
        let mut display = FakeDisplay::with_options(clock, CAPS_LOW_CLOCK, options);
        display.set_mode(&mode(MODE_240P)).unwrap();
        let data = bytes(&PIXELS);
        let mut other = PIXELS;
        other[5] = 0x0012_3456;
        let other = bytes(&other);
        display.capture(1);
        for frame in [&data, &other, &data] {
            display.present(&scene(frame)).unwrap();
            display.wait_flip().unwrap();
        }
        (
            display.screen(),
            display.captured(1),
            display.frame_hashes(),
        )
    };
    let (pixels_screen, pixels_capture, _) = shown(Render::Pixels);
    let (scene_screen, scene_capture, hashes) = shown(Render::Scene);
    assert!(pixels_screen.is_some());
    assert_eq!(scene_screen, pixels_screen);
    assert!(scene_capture.is_some());
    assert_eq!(scene_capture, pixels_capture);
    assert_ne!(scene_capture, scene_screen);
    assert_eq!(hashes.len(), 3);
    assert_eq!(hashes[0], hashes[2]);
    assert_ne!(hashes[0], hashes[1]);
}

#[test]
fn the_scene_hash_ignores_the_row_padding() {
    let hash = |data: &[u8], pitch| {
        let clock = SimClock::new();
        let options = DisplayOptions {
            render: Render::Scene,
            ..DisplayOptions::default()
        };
        let mut display = FakeDisplay::with_options(clock, CAPS_LOW_CLOCK, options);
        display.set_mode(&mode(MODE_240P)).unwrap();
        let mut s = scene(data);
        if let Some(game) = &mut s.game {
            game.pitch = pitch;
        }
        display.present(&s).unwrap();
        display.frames()[0].hash.unwrap()
    };
    let packed = bytes(&PIXELS);
    let mut padded = packed[..16].to_vec();
    padded.extend_from_slice(&[0xAB; 4]);
    padded.extend_from_slice(&packed[16..]);
    assert_eq!(hash(&packed, 16), hash(&padded, 20));
}

#[test]
fn the_scene_hash_includes_the_ui_destination() {
    let hash = |ui_dest| {
        let clock = SimClock::new();
        let mut display =
            FakeDisplay::with_options(clock, CAPS_LOW_CLOCK, DisplayOptions::default());
        display.set_mode(&mode(MODE_240P)).unwrap();
        let data = bytes(&PIXELS);
        let s = Scene {
            ui: Some(Frame {
                data: &data,
                width: 4,
                height: 2,
                pitch: 16,
                format: PixelFormat::Xrgb8888,
            }),
            ui_dest,
            ..scene(&data)
        };
        display.present(&s).unwrap();
        display.frames()[0].hash.unwrap()
    };
    let wide = Rect {
        x: 0,
        y: 0,
        w: 320,
        h: 240,
    };
    let narrow = Rect {
        x: 158,
        y: 119,
        w: 4,
        h: 2,
    };
    assert_eq!(hash(wide), hash(wide));
    assert_ne!(hash(wide), hash(narrow));
}

#[test]
fn the_scene_hash_includes_the_scanlines() {
    let hash = |scanlines| {
        let clock = SimClock::new();
        let mut display =
            FakeDisplay::with_options(clock, CAPS_LOW_CLOCK, DisplayOptions::default());
        display.set_mode(&mode(MODE_240P)).unwrap();
        let data = bytes(&PIXELS);
        let s = Scene {
            scanlines,
            ..scene(&data)
        };
        display.present(&s).unwrap();
        display.frames()[0].hash.unwrap()
    };
    let black = |strength_percent| Scanlines {
        kind: ScanlineKind::Black,
        strength_percent,
    };
    assert_eq!(hash(black(50)), hash(black(50)));
    assert_ne!(hash(Scanlines::NONE), hash(black(50)));
    assert_ne!(hash(black(50)), hash(black(100)));
}

#[test]
fn rendering_off_keeps_no_pixels() {
    let clock = SimClock::new();
    let mut display = timing_only(&clock, DisplayOptions::default());
    display.set_mode(&mode(MODE_240P)).unwrap();
    let data = bytes(&PIXELS);
    display.capture(0);
    display.present(&scene(&data)).unwrap();
    display.wait_flip().unwrap();
    assert!(display.frame_hashes().is_empty());
    assert_eq!(display.screen(), None);
    assert_eq!(display.captured(0), None);
}

#[test]
fn clones_share_the_records() {
    let clock = SimClock::new();
    let probe = FakeDisplay::new(clock.clone(), CAPS_LOW_CLOCK);
    let mut boxed: Box<dyn Display + Send> = Box::new(probe.clone());
    boxed.set_mode(&mode(MODE_240P)).unwrap();
    assert_eq!(probe.active_mode(), Some(MODE_240P));
}

#[test]
fn blank_turns_the_output_off_until_the_next_mode() {
    let clock = SimClock::new();
    let mut display = FakeDisplay::new(clock.clone(), CAPS_LOW_CLOCK);
    assert_eq!(display.blank(), Ok(()), "a blank with no mode succeeds");
    display.set_mode(&mode(MODE_240P)).unwrap();
    frame(&mut display).unwrap();
    let last = frame(&mut display).unwrap();
    assert!(display.screen().is_some());
    clock.advance(PERIOD_240P_NS);
    let blanked_ns = clock.now_ns();
    assert_eq!(display.blank(), Ok(()));
    assert_eq!(display.active_mode(), None);
    assert_eq!(display.screen(), None);
    assert_eq!(display.present(&empty()), Err(DisplayError::NoMode));
    assert_eq!(display.wait_flip(), Err(DisplayError::NoMode));
    assert_eq!(display.blank(), Ok(()), "a second blank succeeds");
    let calls = display.blank_calls();
    assert_eq!(calls.len(), 3);
    assert_eq!((calls[1].call, calls[1].time_ns), (1, blanked_ns));
    assert!(calls.iter().all(|c| c.result.is_ok()));

    clock.advance(10 * PERIOD_240P_NS);
    assert_eq!(display.set_mode(&mode(MODE_240P)), Ok(MODE_240P));
    assert_eq!(display.mode_calls().len(), 2, "the same mode sets again");
    let next = frame(&mut display).unwrap();
    // No blank comes while the output is off.
    assert_eq!(next.sequence, last.sequence + 2);
}

#[test]
fn blank_with_a_flip_queued_is_busy() {
    let clock = SimClock::new();
    let mut display = timing_only(&clock, DisplayOptions::default());
    display.set_mode(&mode(MODE_240P)).unwrap();
    display.present(&empty()).unwrap();
    assert_eq!(
        display.blank(),
        Err(DisplayError::Device {
            operation: "blank",
            reason: "a flip is already queued".to_owned(),
        })
    );
    assert_eq!(display.active_mode(), Some(MODE_240P));
    display.wait_flip().unwrap();
    assert_eq!(display.blank(), Ok(()));
    assert_eq!(display.active_mode(), None);
}

#[test]
fn a_failed_blank_keeps_the_mode() {
    let clock = SimClock::new();
    let mut display = timing_only(&clock, DisplayOptions::default());
    display.set_mode(&mode(MODE_240P)).unwrap();
    display.fault_blank(0, "EIO".to_owned());
    let failed = Err(DisplayError::Device {
        operation: "blank",
        reason: "EIO".to_owned(),
    });
    assert_eq!(display.blank(), failed);
    assert_eq!(display.active_mode(), Some(MODE_240P));
    frame(&mut display).unwrap();
    assert_eq!(display.blank_calls()[0].result, failed);
    assert_eq!(display.blank(), Ok(()));
}
