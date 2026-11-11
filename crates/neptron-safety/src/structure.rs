use neptron_types::Modeline;

use crate::Rejection;

/// Refuses a modeline with a zero pixel clock, an empty active area, or
/// timings out of order. The later checks subtract and divide these values,
/// so they rely on this order.
pub(crate) fn check_structure(mode: &Modeline) -> Result<(), Rejection> {
    if mode.pclock_hz == 0 {
        return Err(Rejection::Malformed {
            field: "pclock_hz",
            value: 0,
            limit: 1,
        });
    }
    let order = [
        ("hactive", mode.hactive, 1),
        ("hsync_start", mode.hsync_start, mode.hactive),
        ("hsync_end", mode.hsync_end, mode.hsync_start),
        ("htotal", mode.htotal, mode.hsync_end),
        ("vactive", mode.vactive, 1),
        ("vsync_start", mode.vsync_start, mode.vactive),
        ("vsync_end", mode.vsync_end, mode.vsync_start),
        ("vtotal", mode.vtotal, mode.vsync_end),
    ];
    for (field, value, limit) in order {
        if value < limit {
            return Err(Rejection::Malformed {
                field,
                value,
                limit,
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ntsc_480i() -> Modeline {
        Modeline {
            pclock_hz: 13_500_000,
            hactive: 720,
            hsync_start: 739,
            hsync_end: 801,
            htotal: 858,
            vactive: 480,
            vsync_start: 488,
            vsync_end: 494,
            vtotal: 525,
            interlace: true,
            doublescan: false,
            hsync_positive: false,
            vsync_positive: false,
        }
    }

    #[test]
    fn a_well_formed_mode_passes() {
        assert_eq!(check_structure(&ntsc_480i()), Ok(()));
    }

    #[test]
    fn zero_pixel_clock_is_malformed() {
        let mode = Modeline {
            pclock_hz: 0,
            ..ntsc_480i()
        };
        assert_eq!(
            check_structure(&mode),
            Err(Rejection::Malformed {
                field: "pclock_hz",
                value: 0,
                limit: 1
            })
        );
        let one = Modeline {
            pclock_hz: 1,
            ..ntsc_480i()
        };
        assert_eq!(check_structure(&one), Ok(()));
    }

    /// Sets one field and returns the result of the check.
    fn with(field: &str, value: u16) -> Result<(), Rejection> {
        let mut m = ntsc_480i();
        match field {
            "hactive" => m.hactive = value,
            "hsync_start" => m.hsync_start = value,
            "hsync_end" => m.hsync_end = value,
            "htotal" => m.htotal = value,
            "vactive" => m.vactive = value,
            "vsync_start" => m.vsync_start = value,
            "vsync_end" => m.vsync_end = value,
            "vtotal" => m.vtotal = value,
            _ => unreachable!("unknown field {field}"),
        }
        check_structure(&m)
    }

    #[test]
    fn each_field_passes_at_its_limit_and_fails_one_below() {
        // (field, the smallest value it can have in ntsc_480i)
        let limits: [(&'static str, u16); 8] = [
            ("hactive", 1),
            ("hsync_start", 720),
            ("hsync_end", 739),
            ("htotal", 801),
            ("vactive", 1),
            ("vsync_start", 480),
            ("vsync_end", 488),
            ("vtotal", 494),
        ];
        for (field, limit) in limits {
            assert_eq!(with(field, limit), Ok(()), "{field} at {limit}");
            assert_eq!(with(field, limit + 1), Ok(()), "{field} at {limit} + 1");
            assert_eq!(
                with(field, limit - 1),
                Err(Rejection::Malformed {
                    field,
                    value: limit - 1,
                    limit
                }),
                "{field} at {limit} - 1"
            );
        }
    }

    #[test]
    fn all_zero_reports_the_first_field() {
        let zero = Modeline {
            pclock_hz: 1,
            hactive: 0,
            hsync_start: 0,
            hsync_end: 0,
            htotal: 0,
            vactive: 0,
            vsync_start: 0,
            vsync_end: 0,
            vtotal: 0,
            ..ntsc_480i()
        };
        assert_eq!(
            check_structure(&zero),
            Err(Rejection::Malformed {
                field: "hactive",
                value: 0,
                limit: 1
            })
        );
    }
}
