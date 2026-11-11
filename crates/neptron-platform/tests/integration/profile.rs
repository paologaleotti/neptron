//! The CRT profile built from an EDID.

use neptron_platform::{EdidError, ProfileError, TimingFormula, profile_from_edid};
use neptron_types::{CrtRange, GpuCaps, MonitorKind};

use crate::blobs::{self, at, fix_checksum, with};

#[test]
fn pc_crt_30_to_71_khz_profile() {
    let built = profile_from_edid(&blobs::pc_crt_30_71()).unwrap();
    assert_eq!(built.profile.id, "edid_npt_0071");
    assert_eq!(built.pclock_max_hz, 100_000_000);
    assert_eq!(built.formula, TimingFormula::DefaultGtf);
    let MonitorKind::Crt { ranges } = built.profile.kind else {
        panic!("not a CRT profile");
    };
    // The porches and syncs are worked out by hand in the unit tests of
    // the gtf module; the line limits in those of the profile module.
    assert_eq!(
        ranges,
        vec![CrtRange {
            h_min_mhz: 30_000_000,
            h_max_mhz: 71_000_000,
            v_min_mhz: 50_000,
            v_max_mhz: 160_000,
            h_front_porch_ns: 817,
            h_sync_ns: 2_667,
            h_back_porch_ns: 3_334,
            v_front_porch_ns: 33_334,
            v_sync_ns: 100_000,
            v_back_porch_ns: 507_747,
            hsync_positive: false,
            vsync_positive: true,
            progressive_lines_min: 169,
            progressive_lines_max: 1_374,
            interlaced_lines_min: 0,
            interlaced_lines_max: 0,
        }]
    );
}

#[test]
fn the_profile_holds_480p_and_240p_at_120_hz() {
    let built = profile_from_edid(&blobs::pc_crt_30_71()).unwrap();
    let MonitorKind::Crt { ranges } = built.profile.kind else {
        panic!("not a CRT profile");
    };
    let range = ranges[0];
    for lines in [224, 240, 448, 480] {
        assert!((range.progressive_lines_min..=range.progressive_lines_max).contains(&lines));
    }
    // 31.5 kHz and both 60 Hz and 120 Hz are inside the rates.
    assert!((range.h_min_mhz..=range.h_max_mhz).contains(&31_500_000));
    assert!((range.v_min_mhz..=range.v_max_mhz).contains(&60_000));
    assert!((range.v_min_mhz..=range.v_max_mhz).contains(&120_000));
}

#[test]
fn rates_above_255_are_kept_exactly() {
    let built = profile_from_edid(&blobs::edid_14_high_rate()).unwrap();
    let MonitorKind::Crt { ranges } = built.profile.kind else {
        panic!("not a CRT profile");
    };
    assert_eq!(ranges[0].h_max_mhz, 300_000_000);
    assert_eq!(built.pclock_max_hz, 190_000_000);
}

#[test]
fn limit_caps_lowers_only_the_highest_pixel_clock() {
    let built = profile_from_edid(&blobs::pc_crt_30_71()).unwrap();
    let caps = GpuCaps {
        pclock_min_hz: 25_000_000,
        pclock_max_hz: 165_000_000,
        interlace: false,
        flip_per_field: false,
        max_width: 1920,
        max_height: 1200,
    };
    let limited = built.limit_caps(&caps);
    assert_eq!(limited.pclock_max_hz, 100_000_000);
    assert_eq!(
        GpuCaps {
            pclock_max_hz: 165_000_000,
            ..limited
        },
        caps
    );
    let slow = GpuCaps {
        pclock_max_hz: 80_000_000,
        ..caps
    };
    assert_eq!(built.limit_caps(&slow), slow);
}

#[test]
fn an_edid_that_does_not_parse_gives_no_profile() {
    let b = with(blobs::pc_crt_30_71(), 0, 0x01);
    assert!(matches!(
        profile_from_edid(&b),
        Err(ProfileError::Edid(EdidError::Header { .. }))
    ));
}

#[test]
fn a_digital_input_gives_no_profile() {
    let b = with(blobs::pc_crt_30_71(), at::INPUT, 0x80);
    assert_eq!(
        profile_from_edid(&b),
        Err(ProfileError::NotAnalog {
            id: "edid_npt_0071".into()
        })
    );
}

#[test]
fn edid_1_4_without_continuous_frequency_gives_no_profile() {
    let b = with(blobs::edid_14_high_rate(), at::FEATURES, 0xea);
    assert_eq!(
        profile_from_edid(&b),
        Err(ProfileError::NotContinuous {
            id: "edid_npt_0071".into(),
            revision: 4
        })
    );
}

#[test]
fn edid_1_3_without_default_gtf_gives_no_profile() {
    let b = with(blobs::pc_crt_30_71(), at::FEATURES, 0xea);
    assert_eq!(
        profile_from_edid(&b),
        Err(ProfileError::NotContinuous {
            id: "edid_npt_0071".into(),
            revision: 3
        })
    );
}

#[test]
fn no_range_limits_descriptor_gives_no_profile() {
    let mut b = blobs::pc_crt_30_71();
    let d = at::DESCRIPTOR_2;
    b[d..d + 18].fill(0);
    b[d + 3] = 0x10;
    fix_checksum(&mut b, 0);
    assert_eq!(
        profile_from_edid(&b),
        Err(ProfileError::NoRangeLimits {
            id: "edid_npt_0071".into()
        })
    );
}

#[test]
fn rates_that_leave_no_lines_give_no_profile() {
    // 1 to 1 kHz at 255 Hz: the GTF vertical blanking is longer than a
    // field.
    let mut b = blobs::pc_crt_30_71();
    let d = at::DESCRIPTOR_2;
    b[d + 5..d + 9].copy_from_slice(&[255, 255, 1, 1]);
    fix_checksum(&mut b, 0);
    assert!(matches!(
        profile_from_edid(&b),
        Err(ProfileError::NoLines { min: 1, max: 0, .. })
    ));
}
