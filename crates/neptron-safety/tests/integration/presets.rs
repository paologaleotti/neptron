//! A mode in the middle of each shipped range validates; modes that leave
//! a range on one value are rejected for that value.

use neptron_safety::{Rejection, validate};
use neptron_types::{CrtRange, MonitorKind, MonitorProfile};

use crate::synth::{GPUS, Request, build, presets, ranges};

/// The middle of the range, in whole hertz when the range is wider than
/// one hertz, so that the line rate of the built mode is exact.
fn middle_line_rate(r: &CrtRange) -> u64 {
    let middle = (r.h_min_mhz + r.h_max_mhz) / 2;
    let whole = middle / 1_000 * 1_000;
    if whole >= r.h_min_mhz { whole } else { middle }
}

/// A 640-pixel mode in the middle of the range. A range with a single
/// field rate (ntsc, pal) gets an interlaced mode: with a whole number of
/// lines per field, only interlace reaches 59.94 Hz or 50 Hz at the line
/// rate of the preset.
fn middle(r: &CrtRange) -> Request {
    let fixed_field = r.v_min_mhz == r.v_max_mhz;
    let interlace = fixed_field && r.interlaced_lines_max > 0;
    let (low, high) = if interlace {
        (r.interlaced_lines_min, r.interlaced_lines_max)
    } else {
        (r.progressive_lines_min, r.progressive_lines_max)
    };
    Request {
        width: 640,
        lines: (low + high) / 2,
        line_rate_mhz: middle_line_rate(r),
        field_rate_mhz: (r.v_min_mhz + r.v_max_mhz) / 2,
        interlace,
        doublescan: false,
    }
}

fn single(id: &str, range: CrtRange) -> MonitorProfile {
    MonitorProfile {
        id: id.to_owned(),
        kind: MonitorKind::Crt {
            ranges: vec![range],
        },
    }
}

#[test]
fn a_mode_in_the_middle_of_every_shipped_range_validates() {
    let caps = GPUS[1];
    let mut checked = 0;
    for profile in presets() {
        for (i, r) in ranges(&profile).iter().enumerate() {
            let mode = build(r, middle(r));
            let alone = single(&profile.id, *r);
            let at = format!("{} range {i}: {mode:?}", profile.id);
            assert_eq!(
                validate(&mode, &alone, &caps).map(|v| *v.mode()),
                Ok(mode),
                "{at}"
            );
            assert_eq!(
                validate(&mode, &profile, &caps).map(|v| *v.mode()),
                Ok(mode),
                "{at}"
            );
            checked += 1;
        }
    }
    assert_eq!(checked, 54);
}

#[test]
fn leaving_the_range_on_one_value_names_that_value() {
    let profiles = presets();
    let generic = profiles.iter().find(|p| p.id == "generic_15").unwrap();
    let r = ranges(generic)[0];
    let caps = GPUS[1];
    let base = middle(&r);
    assert!(validate(&build(&r, base), generic, &caps).is_ok());

    let fast = build(
        &r,
        Request {
            line_rate_mhz: 15_751_000,
            ..base
        },
    );
    assert!(matches!(
        validate(&fast, generic, &caps),
        Err(Rejection::LineRate {
            value_mhz: 15_751_000,
            min_mhz: 15_625_000,
            max_mhz: 15_750_000
        })
    ));

    let tall = build(&r, Request { lines: 289, ..base });
    assert!(matches!(
        validate(&tall, generic, &caps),
        Err(Rejection::LineCount {
            value: 289,
            min: 192,
            max: 288,
            interlaced: false
        })
    ));

    let doubled = build(
        &r,
        Request {
            lines: 145,
            doublescan: true,
            ..base
        },
    );
    assert!(matches!(
        validate(&doubled, generic, &caps),
        Err(Rejection::LineCount {
            value: 290,
            min: 192,
            max: 288,
            interlaced: false
        })
    ));

    let slow = build(
        &r,
        Request {
            field_rate_mhz: 30_000,
            ..base
        },
    );
    assert!(matches!(
        validate(&slow, generic, &caps),
        Err(Rejection::FieldRate {
            min_mhz: 49_500,
            max_mhz: 65_000,
            ..
        })
    ));

    let interlaced = build(
        &r,
        Request {
            lines: 480,
            interlace: true,
            ..base
        },
    );
    assert!(validate(&interlaced, generic, &caps).is_ok());
    assert!(matches!(
        validate(&interlaced, generic, &GPUS[0]),
        Err(Rejection::PixelClock { .. })
    ));
}
