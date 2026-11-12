//! The class of a display, the modes its EDID lists, and the flat-panel
//! profile, on blobs built from the specification.

use neptron_platform::{
    DisplayClass, EdidError, ProfileError, UNKNOWN_GPU_CAPS, display_class, listed_modes,
    panel_profile_from_edid, profile_for_display,
};
use neptron_types::{GpuCaps, Modeline, MonitorKind};

use crate::blobs::{self, fix_checksum};

/// `(width, height, field rate in mHz)` of each listed mode.
fn sizes(modes: &[Modeline]) -> Vec<(u16, u16, u64)> {
    modes
        .iter()
        .map(|m| (m.hactive, m.vactive, m.field_rate_mhz()))
        .collect()
}

fn broken() -> Vec<u8> {
    let mut b = blobs::panel_1080p();
    b[0x40] ^= 0x01;
    b
}

/// What the row is, the connector type, the subconnector, the EDID and
/// the class.
type Row<'a> = (&'a str, &'a str, Option<&'a str>, &'a [u8], DisplayClass);

fn edid_class(kind: &str, sub: Option<&str>, edid: &[u8]) -> Result<DisplayClass, EdidError> {
    display_class(kind, sub, edid)
}

#[test]
fn the_class_of_each_display() {
    use DisplayClass::{Crt, FlatPanel};
    let crt = blobs::pc_crt_30_71();
    let fhd = blobs::panel_1080p();
    let uhd = blobs::panel_2160p();
    let tv = blobs::tv_720p();
    let rows: [Row<'_>; 11] = [
        ("1080p HDMI panel", "HDMI-A", None, &fhd, FlatPanel),
        (
            "4K DisplayPort panel",
            "DP",
            Some("Native"),
            &uhd,
            FlatPanel,
        ),
        (
            "720p TV, CTA modes, no range limits",
            "HDMI-A",
            None,
            &tv,
            FlatPanel,
        ),
        ("laptop panel", "eDP", None, &fhd, FlatPanel),
        ("DVI-D panel", "DVI-D", None, &fhd, FlatPanel),
        ("analog VGA CRT", "VGA", None, &crt, Crt),
        (
            "DP-to-VGA bridge, the CRT's EDID",
            "DP",
            Some("VGA"),
            &crt,
            Crt,
        ),
        (
            "DP with an analog EDID, no subconnector",
            "DP",
            None,
            &crt,
            Crt,
        ),
        (
            "HDMI-to-VGA adapter, the CRT's EDID",
            "HDMI-A",
            None,
            &crt,
            Crt,
        ),
        (
            "DP-to-VGA adapter with its own digital EDID",
            "DP",
            Some("VGA"),
            &fhd,
            Crt,
        ),
        ("VGA connector with a digital EDID", "VGA", None, &fhd, Crt),
    ];
    for (what, kind, sub, edid, class) in rows {
        assert_eq!(edid_class(kind, sub, edid), Ok(class), "{what}");
    }
    assert!(
        matches!(
            edid_class("HDMI-A", None, &broken()),
            Err(EdidError::Checksum { block: 0, .. })
        ),
        "a broken EDID gives no class"
    );
    assert!(edid_class("HDMI-A", None, &[]).is_err(), "no EDID");
}

#[test]
fn the_profile_of_each_display() {
    let fhd = profile_for_display("HDMI-A", None, &blobs::panel_1080p()).unwrap();
    assert_eq!(fhd.profile.id, "panel_npt_1080");
    assert_eq!(fhd.name.as_deref(), Some("NPT FHD 24"));
    assert_eq!(fhd.formula, None);
    let crt = profile_for_display("DP", Some("VGA"), &blobs::pc_crt_30_71()).unwrap();
    assert_eq!(crt.profile.id, "edid_npt_0071");
    assert!(matches!(crt.profile.kind, MonitorKind::Crt { .. }));
    // A DAC with its own digital EDID: the CRT path refuses the EDID, so
    // the caller uses its safe start.
    assert_eq!(
        profile_for_display("DP", Some("VGA"), &blobs::panel_1080p()),
        Err(ProfileError::NotAnalog {
            id: "edid_npt_1080".to_owned()
        })
    );
    assert!(matches!(
        profile_for_display("HDMI-A", None, &broken()),
        Err(ProfileError::Edid(_))
    ));
}

#[test]
fn a_1080p_panel_lists_its_native_mode_first_and_its_50_hz_modes() {
    let listed = listed_modes(&blobs::panel_1080p()).unwrap();
    assert!(!listed.analog);
    assert_eq!(listed.name.as_deref(), Some("NPT FHD 24"));
    assert_eq!(
        sizes(&listed.modes),
        [
            (1920, 1080, 60_000),
            (1920, 1080, 50_000),
            (1280, 720, 60_000),
            (1280, 720, 50_000),
            (720, 576, 50_000),
        ]
    );
    // The detailed timing is the CTA-861 timing of code 16, field for
    // field, so the code and the standard timing add nothing.
    assert_eq!(
        listed.modes[0],
        Modeline {
            pclock_hz: 148_500_000,
            hactive: 1920,
            hsync_start: 2008,
            hsync_end: 2052,
            htotal: 2200,
            vactive: 1080,
            vsync_start: 1084,
            vsync_end: 1089,
            vtotal: 1125,
            interlace: false,
            doublescan: false,
            hsync_positive: true,
            vsync_positive: true,
        }
    );
}

#[test]
fn a_4k_panel_and_a_720p_tv() {
    let uhd = listed_modes(&blobs::panel_2160p()).unwrap();
    assert_eq!(
        sizes(&uhd.modes),
        [
            (3840, 2160, 60_000),
            (3840, 2160, 50_000),
            (3840, 2160, 30_000),
            (1920, 1080, 60_000),
            (1920, 1080, 50_000),
            // The standard timing 1280x720 at 60 Hz.
            (1280, 720, 60_000),
        ]
    );
    let tv = listed_modes(&blobs::tv_720p()).unwrap();
    // 1080i, as a detailed timing and as code 5, is left out.
    assert_eq!(
        sizes(&tv.modes),
        [(1280, 720, 60_000), (1280, 720, 50_000), (720, 576, 50_000)]
    );
}

#[test]
fn established_and_standard_timings_of_a_dmt_monitor() {
    // The CRT blob lists 640x480, 800x600 and 1024x768 at 60 Hz as
    // established timings, after its 1024x768 at 75 Hz detailed timing.
    let listed = listed_modes(&blobs::pc_crt_30_71()).unwrap();
    assert!(listed.analog);
    assert_eq!(
        sizes(&listed.modes),
        [
            (1024, 768, 75_029),
            (640, 480, 59_940),
            (800, 600, 60_317),
            (1024, 768, 60_004),
        ]
    );
}

#[test]
fn a_panel_with_no_progressive_mode_has_no_profile() {
    let mut b = blobs::digital_base(0x0001, blobs::dtd_1080i60(), "NPT I", false);
    b[0x26..0x36].fill(0x01);
    fix_checksum(&mut b, 0);
    assert_eq!(
        panel_profile_from_edid(&b),
        Err(ProfileError::NoListedModes {
            id: "panel_npt_0001".to_owned()
        })
    );
}

#[test]
fn a_mode_above_the_limits_leaves_the_largest_mode_that_fits_first() {
    let uhd = panel_profile_from_edid(&blobs::panel_2160p()).unwrap();
    assert_eq!(uhd.pclock_max_hz, 594_000_000);
    let fit = uhd.fit_panel(&UNKNOWN_GPU_CAPS, 3840, 2160).unwrap();
    let native = uhd.panel_mode().copied();
    assert_eq!(fit.native_left_out, native);
    let MonitorKind::FlatPanel { modes } = &fit.profile.profile.kind else {
        panic!("a panel");
    };
    assert_eq!(
        sizes(modes),
        [
            (1920, 1080, 60_000),
            (1920, 1080, 50_000),
            (1280, 720, 60_000)
        ]
    );
    assert_eq!(fit.profile.pclock_max_hz, 148_500_000);

    let wide = GpuCaps {
        pclock_max_hz: 600_000_000,
        max_width: 4096,
        max_height: 4096,
        ..UNKNOWN_GPU_CAPS
    };
    let all = uhd.fit_panel(&wide, 3840, 2160).unwrap();
    assert_eq!(all.native_left_out, None);
    assert_eq!(all.profile, uhd);
    let buffer = uhd.fit_panel(&wide, 2560, 1440).unwrap();
    assert_eq!(buffer.native_left_out, native);
    let narrow = GpuCaps {
        max_width: 1024,
        ..UNKNOWN_GPU_CAPS
    };
    assert!(uhd.fit_panel(&narrow, 3840, 2160).is_none(), "no mode fits");
}

#[test]
fn a_1080p_panel_fits_the_unknown_gpu() {
    let fhd = panel_profile_from_edid(&blobs::panel_1080p()).unwrap();
    let fit = fhd.fit_panel(&UNKNOWN_GPU_CAPS, 3840, 2160).unwrap();
    assert_eq!(fit.native_left_out, None);
    assert_eq!(fit.profile, fhd);
}

#[test]
fn every_listed_mode_has_a_whole_khz_clock() {
    // A detailed timing counts its clock in 10 kHz and the timing tables
    // in kHz, so every listed mode is whole kHz, as the backends ask.
    for edid in [
        blobs::panel_1080p(),
        blobs::panel_2160p(),
        blobs::tv_720p(),
        blobs::pc_crt_30_71(),
    ] {
        for mode in listed_modes(&edid).unwrap().modes {
            assert!(mode.pclock_is_whole_khz(), "{mode:?}");
        }
    }
}

#[test]
fn a_fallback_mode_keeps_a_game_rate_before_a_larger_size() {
    // HD 530 on DisplayPort: 533.25 MHz. The CTA-861 2160p60 and 2160p50
    // modes (594 MHz) are out; 2160p30 fits, but it would halve the speed
    // of a 60 Hz game, so 1080p60 comes first.
    let dp = GpuCaps {
        pclock_max_hz: 533_250_000,
        max_width: 4096,
        max_height: 2160,
        ..UNKNOWN_GPU_CAPS
    };
    let uhd = panel_profile_from_edid(&blobs::panel_2160p()).unwrap();
    let fit = uhd.fit_panel(&dp, 3840, 2160).unwrap();
    let MonitorKind::FlatPanel { modes } = &fit.profile.profile.kind else {
        panic!("a panel");
    };
    assert_eq!(sizes(modes)[0], (1920, 1080, 60_000));
    assert!(sizes(modes).contains(&(3840, 2160, 30_000)));
}
