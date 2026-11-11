use neptron_switchres_sys::{
    BlankingFault, CallError, HBLANK_MAX_FRACTION, MAX_RANGES, MONITOR_ASPECT_MAX,
    MONITOR_ASPECT_MIN, Mode, REFRESH_MAX_HZ, REFRESH_MIN_HZ, Range, Request, Settings,
    VBLANK_MAX_FRACTION, VBLANK_MIN_LINES, WEIGHT_OUT_OF_RANGE, modelines,
};

/// `generic_15`: 15625-15750, 49.50-65.00, 2.000, 4.700, 8.000, 0.064,
/// 0.192, 1.024, 0, 0, 192, 288, 448, 576.
fn generic_15() -> Range {
    Range {
        hfreq_min_hz: 15625.0,
        hfreq_max_hz: 15750.0,
        vfreq_min_hz: 49.5,
        vfreq_max_hz: 65.0,
        hfront_porch_us: 2.0,
        hsync_pulse_us: 4.7,
        hback_porch_us: 8.0,
        vfront_porch_ms: 0.064,
        vsync_pulse_ms: 0.192,
        vback_porch_ms: 1.024,
        hsync_positive: false,
        vsync_positive: false,
        progressive_lines_min: 192,
        progressive_lines_max: 288,
        interlaced_lines_min: 448,
        interlaced_lines_max: 576,
    }
}

/// `arcade_31`: 31400-31500, 49.50-65.00, 0.940, 3.770, 1.890, 0.349,
/// 0.064, 1.017, 0, 0, 400, 512, 0, 0.
fn arcade_31() -> Range {
    Range {
        hfreq_min_hz: 31400.0,
        hfreq_max_hz: 31500.0,
        vfreq_min_hz: 49.5,
        vfreq_max_hz: 65.0,
        hfront_porch_us: 0.94,
        hsync_pulse_us: 3.77,
        hback_porch_us: 1.89,
        vfront_porch_ms: 0.349,
        vsync_pulse_ms: 0.064,
        vback_porch_ms: 1.017,
        hsync_positive: false,
        vsync_positive: false,
        progressive_lines_min: 400,
        progressive_lines_max: 512,
        interlaced_lines_min: 0,
        interlaced_lines_max: 0,
    }
}

/// The defaults of `switchres_manager`.
fn defaults() -> Settings {
    Settings {
        pclock_min_hz: 0,
        monitor_aspect: 4.0 / 3.0,
        refresh_tolerance_hz: 2.0,
        interlace: true,
        doublescan: true,
        super_width: 2560,
        pixel_precision: true,
        interlace_force_even: false,
        scale_proportional: true,
    }
}

fn request(width: u16, height: u16, refresh_hz: f64) -> Request {
    Request {
        refresh_hz,
        width,
        height,
        interlaced: false,
        rotated: false,
    }
}

/// The timing part of a modeline, as the standalone program prints it.
fn timings(mode: &Mode) -> String {
    format!(
        "{}.{:06} {} {} {} {} {} {} {} {}{}{}",
        mode.pclock_hz / 1_000_000,
        mode.pclock_hz % 1_000_000,
        mode.hactive,
        mode.hbegin,
        mode.hend,
        mode.htotal,
        mode.vactive,
        mode.vbegin,
        mode.vend,
        mode.vtotal,
        if mode.interlace != 0 {
            " interlace"
        } else {
            ""
        },
        if mode.doublescan != 0 {
            " doublescan"
        } else {
            ""
        },
    )
}

fn only_mode(ranges: &[Range], settings: &Settings, request: &Request) -> Mode {
    let modes = modelines(ranges, settings, request).unwrap();
    assert_eq!(modes.len(), 1, "{modes:?}");
    modes[0]
}

#[test]
fn generic_15_320x240_at_60() {
    let mode = only_mode(&[generic_15()], &defaults(), &request(320, 240, 60.0));
    assert_eq!(timings(&mode), "6.514560 320 333 364 416 240 242 245 261");
    assert_eq!((mode.hsync_positive, mode.vsync_positive), (0, 0));
    assert_eq!(mode.range_index, 0);
    assert_eq!(mode.weight & WEIGHT_OUT_OF_RANGE, 0);
}

#[test]
fn generic_15_640x480_interlaced_at_59_94() {
    let interlaced = Request {
        interlaced: true,
        ..request(640, 480, 59.94)
    };
    let mode = only_mode(&[generic_15()], &defaults(), &interlaced);
    assert_eq!(
        timings(&mode),
        "13.025351 640 666 727 831 480 483 489 523 interlace"
    );
}

#[test]
fn arcade_31_doublescan_and_line_doubling() {
    let source = request(320, 240, 60.0);
    let mode = only_mode(&[arcade_31()], &defaults(), &source);
    assert_eq!(
        timings(&mode),
        "12.726000 320 332 380 404 240 245 246 262 doublescan"
    );
    let no_doublescan = Settings {
        doublescan: false,
        ..defaults()
    };
    let mode = only_mode(&[arcade_31()], &no_doublescan, &source);
    assert_eq!(timings(&mode), "25.452000 640 664 760 808 480 491 493 525");
}

#[test]
fn a_pixel_clock_floor_doubles_the_width() {
    let settings = Settings {
        pclock_min_hz: 25_000_000,
        interlace: false,
        doublescan: false,
        ..defaults()
    };
    let mode = only_mode(&[generic_15()], &settings, &request(256, 240, 60.099));
    assert_eq!(
        timings(&mode),
        "41.533216 2048 2131 2326 2658 240 241 244 260"
    );
}

#[test]
fn every_range_gives_a_mode_best_first() {
    let ranges = [arcade_31(), generic_15()];
    let modes = modelines(&ranges, &defaults(), &request(320, 240, 60.0)).unwrap();
    assert_eq!(modes.len(), 2);
    // A 240-line source fits the 15 kHz range without doublescan, so that
    // range ranks first although it comes second.
    assert_eq!(modes[0].range_index, 1);
    assert_eq!(modes[1].range_index, 0);
    assert_eq!(
        timings(&modes[0]),
        "6.514560 320 333 364 416 240 242 245 261"
    );
}

#[test]
fn no_range_gives_no_mode() {
    assert_eq!(
        modelines(&[], &defaults(), &request(320, 240, 60.0)),
        Ok(vec![])
    );
}

#[test]
fn too_many_ranges_is_an_error() {
    let ranges = vec![generic_15(); MAX_RANGES + 1];
    assert_eq!(
        modelines(&ranges, &defaults(), &request(320, 240, 60.0)),
        Err(CallError::TooManyRanges {
            count: MAX_RANGES + 1,
            max: MAX_RANGES
        })
    );
    let ranges = vec![generic_15(); MAX_RANGES];
    assert_eq!(
        modelines(&ranges, &defaults(), &request(320, 240, 60.0))
            .unwrap()
            .len(),
        MAX_RANGES
    );
}

#[test]
fn a_request_without_size_or_with_a_refresh_out_of_limits_is_an_error() {
    for bad in [
        request(0, 240, 60.0),
        request(320, 0, 60.0),
        request(320, 240, 0.0),
        request(320, 240, -60.0),
        request(320, 240, 0.000_999),
        request(320, 240, 10_000_001.0),
        request(320, 240, f64::NAN),
        request(320, 240, f64::INFINITY),
        request(320, 240, f64::MIN_POSITIVE),
    ] {
        match modelines(&[generic_15()], &defaults(), &bad) {
            Err(CallError::InvalidRequest {
                width,
                height,
                refresh_hz,
            }) => {
                // to_bits, because a NaN is not equal to itself.
                assert_eq!(
                    (width, height, refresh_hz.to_bits()),
                    (bad.width, bad.height, bad.refresh_hz.to_bits())
                );
            }
            other => panic!("{bad:?}: {other:?}"),
        }
    }
}

#[test]
fn the_refresh_limits_are_inclusive() {
    assert_eq!((REFRESH_MIN_HZ, REFRESH_MAX_HZ), (0.001, 10_000_000.0));
    for refresh_hz in [REFRESH_MIN_HZ, REFRESH_MAX_HZ] {
        assert!(modelines(&[generic_15()], &defaults(), &request(320, 240, refresh_hz)).is_ok());
    }
}

#[test]
fn a_monitor_aspect_out_of_limits_is_an_error() {
    assert_eq!((MONITOR_ASPECT_MIN, MONITOR_ASPECT_MAX), (0.25, 4.0));
    for monitor_aspect in [MONITOR_ASPECT_MIN, MONITOR_ASPECT_MAX] {
        let settings = Settings {
            monitor_aspect,
            ..defaults()
        };
        assert!(modelines(&[generic_15()], &settings, &request(320, 240, 60.0)).is_ok());
    }
    for monitor_aspect in [0.0, 0.249, 4.001, -1.0, f64::NAN, f64::INFINITY, 1e300] {
        let settings = Settings {
            monitor_aspect,
            ..defaults()
        };
        let result = modelines(&[generic_15()], &settings, &request(320, 240, 60.0));
        match result {
            Err(CallError::InvalidSettings {
                monitor_aspect: got,
            }) => {
                assert_eq!(got.to_bits(), monitor_aspect.to_bits());
            }
            other => panic!("{monitor_aspect}: {other:?}"),
        }
    }
}

#[test]
fn a_range_value_that_is_not_finite_is_refused() {
    let fields: [fn(&mut Range) -> &mut f64; 10] = [
        |r| &mut r.hfreq_min_hz,
        |r| &mut r.hfreq_max_hz,
        |r| &mut r.vfreq_min_hz,
        |r| &mut r.vfreq_max_hz,
        |r| &mut r.hfront_porch_us,
        |r| &mut r.hsync_pulse_us,
        |r| &mut r.hback_porch_us,
        |r| &mut r.vfront_porch_ms,
        |r| &mut r.vsync_pulse_ms,
        |r| &mut r.vback_porch_ms,
    ];
    for field in fields {
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let mut bad = generic_15();
            *field(&mut bad) = value;
            assert_eq!(
                modelines(&[generic_15(), bad], &defaults(), &request(320, 240, 60.0)),
                Err(CallError::InvalidRange { index: 1 }),
                "{bad:?}"
            );
        }
    }
}

/// The three inputs that reached undefined behaviour in Switchres before
/// the types and the shim closed them.
#[test]
fn the_inputs_that_overflowed_switchres_are_closed() {
    // 1. The vertical shift correction with a large interlaced range gave
    //    a negative border and a signed overflow. The settings no longer
    //    have the field, and the shim keeps the correction off. The same
    //    range and request now give a mode.
    let range = Range {
        hfreq_min_hz: 15_000.0,
        hfreq_max_hz: 60_000.0,
        vfreq_min_hz: 50.0,
        vfreq_max_hz: 60.0,
        hfront_porch_us: 1.0,
        hsync_pulse_us: 2.0,
        hback_porch_us: 2.0,
        vfront_porch_ms: 0.064,
        vsync_pulse_ms: 0.192,
        vback_porch_ms: 1.0,
        hsync_positive: false,
        vsync_positive: false,
        progressive_lines_min: 128,
        progressive_lines_max: 200,
        interlaced_lines_min: 400,
        interlaced_lines_max: 2_000,
    };
    let interlaced = Request {
        interlaced: true,
        ..request(640, 1_000, 50.0)
    };
    let result = modelines(&[range], &defaults(), &interlaced);
    assert!(
        matches!(&result, Ok(modes) if !modes.is_empty()),
        "{result:?}"
    );
    // 2. A pixel clock floor of 1e15 Hz overflowed the width doubling. The
    //    floor is a u32, so the largest floor is u32::MAX, about 4.3 GHz.
    let highest_floor = Settings {
        pclock_min_hz: u32::MAX,
        ..defaults()
    };
    let modes = modelines(&[generic_15()], &highest_floor, &request(320, 240, 60.0)).unwrap();
    assert!(modes[0].pclock_hz > u64::from(u32::MAX), "{modes:?}");
    // 3. A width of 2,000,000,000 overflowed get_line_params. The width is
    //    a u16, so the largest width is 65,535.
    let widest = modelines(&[generic_15()], &defaults(), &request(u16::MAX, 240, 60.0)).unwrap();
    assert!(widest[0].hactive >= i32::from(u16::MAX), "{widest:?}");
}

#[test]
fn switchres_refuses_a_bad_range_by_index() {
    let no_line_rate = Range {
        hfreq_min_hz: 0.0,
        ..generic_15()
    };
    assert_eq!(
        modelines(
            &[generic_15(), no_line_rate],
            &defaults(),
            &request(320, 240, 60.0)
        ),
        Err(CallError::InvalidRange { index: 1 })
    );
    let no_porch = Range {
        hback_porch_us: 0.0,
        ..arcade_31()
    };
    assert_eq!(
        modelines(&[no_porch], &defaults(), &request(320, 240, 60.0)),
        Err(CallError::InvalidRange { index: 0 })
    );
}

#[test]
fn parallel_calls_give_the_same_result() {
    let expected = modelines(&[generic_15()], &defaults(), &request(320, 240, 60.0)).unwrap();
    let results: Vec<_> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..8)
            .map(|_| {
                scope.spawn(|| {
                    (0..50)
                        .map(|_| {
                            modelines(&[generic_15()], &defaults(), &request(320, 240, 60.0))
                                .unwrap()
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    for run in results.iter().flatten() {
        assert_eq!(run, &expected);
    }
}

#[test]
fn errors_say_what_failed() {
    assert_eq!(
        CallError::InvalidRange { index: 3 }.to_string(),
        "Switchres refuses range 3"
    );
    assert_eq!(
        CallError::TooManyRanges { count: 11, max: 10 }.to_string(),
        "11 ranges, but Switchres holds at most 10"
    );
}

fn blanking(ranges: &[Range]) -> CallError {
    modelines(ranges, &defaults(), &request(320, 240, 60.0)).unwrap_err()
}

#[test]
fn a_horizontal_blanking_that_fills_the_line_is_refused() {
    // Switchres accepts this range and then loops forever in
    // get_line_params: 14.7 us of blanking in a line of 14.29 us.
    let fast = Range {
        hfreq_min_hz: 70_000.0,
        hfreq_max_hz: 70_000.0,
        vfreq_min_hz: 50.0,
        vfreq_max_hz: 60.0,
        ..generic_15()
    };
    match blanking(&[generic_15(), fast]) {
        CallError::Blanking {
            index,
            fault,
            blanking,
            limit,
        } => {
            assert_eq!(index, 1);
            assert_eq!(fault, BlankingFault::HorizontalTooLong);
            assert_eq!(blanking, 2.0 + 4.7 + 8.0);
            assert_eq!(limit, HBLANK_MAX_FRACTION * 1_000_000.0 / 70_000.0);
        }
        other => panic!("{other:?}"),
    }
}

/// A 20 kHz range: the line is 50 us, so the horizontal limit is 37.5 us.
fn twenty_khz(hback_porch_us: f64) -> Range {
    Range {
        hfreq_min_hz: 20_000.0,
        hfreq_max_hz: 20_000.0,
        hfront_porch_us: 2.0,
        hsync_pulse_us: 4.0,
        hback_porch_us,
        ..generic_15()
    }
}

#[test]
fn the_horizontal_limit_is_three_quarters_of_the_shortest_line() {
    assert_eq!(HBLANK_MAX_FRACTION, 0.75);
    let request = request(320, 240, 60.0);
    // 2 + 4 + 31.4999 us: below the limit.
    assert!(modelines(&[twenty_khz(31.4999)], &defaults(), &request).is_ok());
    // 2 + 4 + 31.5 us: at the limit.
    assert!(matches!(
        blanking(&[twenty_khz(31.5)]),
        CallError::Blanking {
            fault: BlankingFault::HorizontalTooLong,
            ..
        }
    ));
}

/// generic_15 with a vertical back porch of `vback_porch_ms`; the field at
/// 65 Hz is 15.38 ms, so the vertical limit is 7.69 ms.
fn long_vblank(vback_porch_ms: f64) -> Range {
    // Fewer lines, so that monitor_evaluate_range still fits the longer
    // blanking into a field at the lowest field rate.
    Range {
        vback_porch_ms,
        progressive_lines_max: 196,
        interlaced_lines_min: 0,
        interlaced_lines_max: 0,
        ..generic_15()
    }
}

#[test]
fn the_vertical_limit_is_half_of_the_shortest_field() {
    assert_eq!(VBLANK_MAX_FRACTION, 0.5);
    let request = request(320, 240, 60.0);
    // 0.064 + 0.192 + 7.43 = 7.686 ms: below 0.5 / 65 Hz = 7.6923 ms.
    assert!(modelines(&[long_vblank(7.43)], &defaults(), &request).is_ok());
    match blanking(&[long_vblank(7.44)]) {
        CallError::Blanking {
            index: 0,
            fault: BlankingFault::VerticalTooLong,
            blanking,
            limit,
        } => {
            assert!((blanking - 7.696).abs() < 1e-9, "{blanking}");
            assert!((limit - 500.0 / 65.0).abs() < 1e-9, "{limit}");
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_vertical_blanking_shorter_than_one_line_is_refused() {
    assert_eq!(VBLANK_MIN_LINES, 1.0);
    let short = |vback_porch_ms| Range {
        vfront_porch_ms: 0.01,
        vsync_pulse_ms: 0.02,
        vback_porch_ms,
        ..generic_15()
    };
    let request = request(320, 240, 60.0);
    // One line at 15 750 Hz is 0.0635 ms.
    assert!(modelines(&[short(0.034)], &defaults(), &request).is_ok());
    assert!(matches!(
        blanking(&[short(0.03)]),
        CallError::Blanking {
            fault: BlankingFault::VerticalTooShort,
            ..
        }
    ));
}

#[test]
fn every_blanking_error_says_what_failed() {
    let error = CallError::Blanking {
        index: 2,
        fault: BlankingFault::HorizontalTooLong,
        blanking: 14.7,
        limit: 10.5,
    };
    assert_eq!(
        error.to_string(),
        "range 2: horizontal blanking 14.7 us, must be below 10.5 us"
    );
    let error = CallError::Blanking {
        index: 0,
        fault: BlankingFault::VerticalTooShort,
        blanking: 0.03,
        limit: 0.0635,
    };
    assert_eq!(
        error.to_string(),
        "range 0: vertical blanking 0.03 ms, must be at least 0.0635 ms"
    );
}
