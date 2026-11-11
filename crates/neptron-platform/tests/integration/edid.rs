//! The EDID parser: what it reads from valid blobs, and every refusal.

use neptron_platform::{EdidError, RangeLimits, TimingFormula, parse_edid};

use crate::blobs::{self, at, fix_checksum, with};

#[test]
fn pc_crt_blob_parses() {
    let edid = parse_edid(&blobs::pc_crt_30_71()).unwrap();
    assert_eq!(edid.manufacturer, "NPT");
    assert_eq!(edid.product_code, 0x0071);
    assert_eq!(edid.revision, 3);
    assert!(edid.analog);
    assert!(edid.continuous_frequency);
    assert_eq!(
        edid.range_limits,
        Some(RangeLimits {
            v_min_hz: 50,
            v_max_hz: 160,
            h_min_khz: 30,
            h_max_khz: 71,
            // Byte 9 is 11: the display rounds up, so only 100 MHz is sure.
            pclock_max_hz: 100_000_000,
            formula: TimingFormula::DefaultGtf,
        })
    );
}

#[test]
fn rate_offset_adds_255_khz_to_the_maximum() {
    let limits = parse_edid(&blobs::edid_14_high_rate())
        .unwrap()
        .range_limits
        .unwrap();
    assert_eq!(limits.h_min_khz, 30);
    assert_eq!(limits.h_max_khz, 300);
    assert_eq!(limits.v_max_hz, 160);
    assert_eq!(limits.pclock_max_hz, 190_000_000);
    assert_eq!(limits.formula, TimingFormula::RangeLimitsOnly);
}

#[test]
fn rate_offset_on_both_vertical_limits() {
    let mut b = blobs::edid_14_high_rate();
    let d = at::DESCRIPTOR_2;
    // Bits 1 to 0 = 11: vertical minimum and maximum + 255 Hz.
    b[d + 4] = 0x08 | 0x03;
    b[d + 5] = 1;
    b[d + 6] = 10;
    fix_checksum(&mut b, 0);
    let limits = parse_edid(&b).unwrap().range_limits.unwrap();
    assert_eq!((limits.v_min_hz, limits.v_max_hz), (256, 265));
}

#[test]
fn cvt_gives_the_exact_pixel_clock() {
    let limits = parse_edid(&blobs::edid_14_cvt())
        .unwrap()
        .range_limits
        .unwrap();
    assert_eq!(limits.formula, TimingFormula::Cvt);
    assert_eq!(limits.pclock_max_hz, 107_750_000);
}

#[test]
fn secondary_gtf_is_accepted_and_its_parameters_ignored() {
    let mut b = blobs::pc_crt_30_71();
    let d = at::DESCRIPTOR_2;
    b[d + 10..d + 18].copy_from_slice(&[0x02, 0x00, 0x30, 0x50, 0x58, 0x02, 0x80, 0x28]);
    fix_checksum(&mut b, 0);
    let limits = parse_edid(&b).unwrap().range_limits.unwrap();
    assert_eq!(limits.formula, TimingFormula::SecondaryGtf);
    assert_eq!(limits.h_max_khz, 71);
}

#[test]
fn digital_input_and_continuous_bit_are_reported() {
    let b = with(blobs::edid_14_high_rate(), at::INPUT, 0xa5);
    let b = with(b, at::FEATURES, 0xea);
    let edid = parse_edid(&b).unwrap();
    assert!(!edid.analog);
    assert!(!edid.continuous_frequency);
}

#[test]
fn edid_1_3_without_default_gtf_bit_is_reported() {
    let b = with(blobs::pc_crt_30_71(), at::FEATURES, 0xea);
    let edid = parse_edid(&b).unwrap();
    assert_eq!(edid.revision, 3);
    assert!(!edid.continuous_frequency);
}

#[test]
fn cvt_maximum_active_pixels_is_refused() {
    // Byte 13 = 160: 160 * 8 = 1280 pixels.
    let b = with(blobs::edid_14_cvt(), at::DESCRIPTOR_2 + 13, 160);
    assert_eq!(
        parse_edid(&b),
        Err(EdidError::CvtMaxActivePixels { pixels: 1280 })
    );
    // Byte 12 bits 1 to 0 = 01 and byte 13 = 0: 256 * 8 = 2048 pixels.
    let b = with(blobs::edid_14_cvt(), at::DESCRIPTOR_2 + 12, (9 << 2) | 0x01);
    assert_eq!(
        parse_edid(&b),
        Err(EdidError::CvtMaxActivePixels { pixels: 2048 })
    );
}

#[test]
fn no_range_limits_descriptor_gives_none() {
    let mut b = blobs::pc_crt_30_71();
    let d = at::DESCRIPTOR_2;
    b[d..d + 18].fill(0);
    b[d + 3] = 0x10;
    fix_checksum(&mut b, 0);
    assert_eq!(parse_edid(&b).unwrap().range_limits, None);
}

#[test]
fn extension_blocks_are_accepted_with_valid_checksums() {
    let mut b = with(blobs::pc_crt_30_71(), at::EXTENSIONS, 1);
    let mut extension = vec![0u8; 128];
    extension[0] = 0x02;
    extension[1] = 0x03;
    b.extend_from_slice(&extension);
    fix_checksum(&mut b, 1);
    assert!(parse_edid(&b).is_ok());
}

#[test]
fn short_data_is_refused() {
    let b = blobs::pc_crt_30_71();
    assert_eq!(
        parse_edid(&b[..100]),
        Err(EdidError::Length {
            len: 100,
            expected: 128
        })
    );
}

#[test]
fn missing_extension_block_is_refused() {
    let b = with(blobs::pc_crt_30_71(), at::EXTENSIONS, 1);
    assert_eq!(
        parse_edid(&b),
        Err(EdidError::Length {
            len: 128,
            expected: 256
        })
    );
}

#[test]
fn bad_header_is_refused() {
    let b = with(blobs::pc_crt_30_71(), 1, 0x00);
    assert!(matches!(parse_edid(&b), Err(EdidError::Header { found }) if found[1] == 0));
}

#[test]
fn bad_checksum_is_refused() {
    let mut b = blobs::pc_crt_30_71();
    b[at::CHECKSUM] = b[at::CHECKSUM].wrapping_add(1);
    assert_eq!(
        parse_edid(&b),
        Err(EdidError::Checksum {
            block: 0,
            remainder: 1
        })
    );
}

#[test]
fn bad_extension_checksum_is_refused() {
    let mut b = with(blobs::pc_crt_30_71(), at::EXTENSIONS, 1);
    b.extend_from_slice(&[0x03; 128]);
    assert!(matches!(
        parse_edid(&b),
        Err(EdidError::Checksum { block: 1, .. })
    ));
}

#[test]
fn other_versions_are_refused() {
    for (version, revision) in [(1, 2), (1, 5), (2, 0)] {
        let b = with(blobs::pc_crt_30_71(), at::VERSION, version);
        let b = with(b, at::REVISION, revision);
        assert_eq!(
            parse_edid(&b),
            Err(EdidError::Version { version, revision })
        );
    }
}

#[test]
fn bad_manufacturer_is_refused() {
    // Reserved bit set.
    let b = with(blobs::pc_crt_30_71(), at::MANUFACTURER, 0xba);
    assert_eq!(parse_edid(&b), Err(EdidError::Manufacturer { raw: 0xba14 }));
    // A letter code of 0.
    let b = with(blobs::pc_crt_30_71(), at::MANUFACTURER, 0x02);
    assert_eq!(parse_edid(&b), Err(EdidError::Manufacturer { raw: 0x0214 }));
}

#[test]
fn first_descriptor_must_be_a_detailed_timing() {
    let mut b = blobs::pc_crt_30_71();
    let d = at::DESCRIPTOR_1;
    b[d..d + 18].fill(0);
    b[d + 3] = 0x10;
    fix_checksum(&mut b, 0);
    assert_eq!(parse_edid(&b), Err(EdidError::NoPreferredTiming));
}

#[test]
fn reserved_descriptor_bytes_are_refused() {
    let b = with(blobs::pc_crt_30_71(), at::DESCRIPTOR_3 + 2, 0x01);
    assert_eq!(
        parse_edid(&b),
        Err(EdidError::DescriptorReserved {
            offset: at::DESCRIPTOR_3,
            byte: 2,
            value: 1
        })
    );
    let b = with(blobs::pc_crt_30_71(), at::DESCRIPTOR_4 + 4, 0x01);
    assert_eq!(
        parse_edid(&b),
        Err(EdidError::DescriptorReserved {
            offset: at::DESCRIPTOR_4,
            byte: 4,
            value: 1
        })
    );
}

#[test]
fn reserved_tag_is_refused() {
    let b = with(blobs::pc_crt_30_71(), at::DESCRIPTOR_4 + 3, 0x80);
    assert_eq!(
        parse_edid(&b),
        Err(EdidError::ReservedTag {
            offset: at::DESCRIPTOR_4,
            tag: 0x80
        })
    );
}

#[test]
fn two_range_limits_descriptors_are_refused() {
    let mut b = blobs::pc_crt_30_71();
    let (from, to) = (at::DESCRIPTOR_2, at::DESCRIPTOR_4);
    b.copy_within(from..from + 18, to);
    fix_checksum(&mut b, 0);
    assert_eq!(
        parse_edid(&b),
        Err(EdidError::DuplicateRangeLimits {
            first: from,
            second: to
        })
    );
}

#[test]
fn offset_flags_are_refused_in_edid_1_3() {
    let b = with(blobs::pc_crt_30_71(), at::DESCRIPTOR_2 + 4, 0x08);
    assert_eq!(
        parse_edid(&b),
        Err(EdidError::DescriptorReserved {
            offset: at::DESCRIPTOR_2,
            byte: 4,
            value: 0x08
        })
    );
}

#[test]
fn reserved_offset_flags_are_refused_in_edid_1_4() {
    for flags in [0x01, 0x04, 0x10, 0x80] {
        let b = with(blobs::edid_14_high_rate(), at::DESCRIPTOR_2 + 4, flags);
        assert_eq!(
            parse_edid(&b),
            Err(EdidError::OffsetFlags { flags, revision: 4 })
        );
    }
}

#[test]
fn zero_limits_are_refused() {
    let fields = ["v_min", "v_max", "h_min", "h_max", "pclock_max"];
    for (byte, field) in (5..=9).zip(fields) {
        let b = with(blobs::pc_crt_30_71(), at::DESCRIPTOR_2 + byte, 0);
        assert_eq!(parse_edid(&b), Err(EdidError::ZeroLimit { field }));
    }
}

#[test]
fn inverted_limits_are_refused() {
    let b = with(blobs::pc_crt_30_71(), at::DESCRIPTOR_2 + 5, 200);
    assert_eq!(
        parse_edid(&b),
        Err(EdidError::Inverted {
            field: "vertical",
            min: 200,
            max: 160
        })
    );
    let b = with(blobs::pc_crt_30_71(), at::DESCRIPTOR_2 + 7, 72);
    assert_eq!(
        parse_edid(&b),
        Err(EdidError::Inverted {
            field: "horizontal",
            min: 72,
            max: 71
        })
    );
}

#[test]
fn unknown_or_newer_timing_formula_is_refused() {
    for value in [0x01, 0x04, 0x03, 0x08] {
        let b = with(blobs::pc_crt_30_71(), at::DESCRIPTOR_2 + 10, value);
        assert_eq!(
            parse_edid(&b),
            Err(EdidError::TimingSupport { value, revision: 3 })
        );
    }
}

#[test]
fn wrong_padding_is_refused() {
    let b = with(blobs::pc_crt_30_71(), at::DESCRIPTOR_2 + 11, 0x20);
    assert_eq!(
        parse_edid(&b),
        Err(EdidError::Padding {
            byte: 11,
            value: 0x20,
            expected: 0x0a
        })
    );
    let b = with(blobs::pc_crt_30_71(), at::DESCRIPTOR_2 + 17, 0x00);
    assert_eq!(
        parse_edid(&b),
        Err(EdidError::Padding {
            byte: 17,
            value: 0x00,
            expected: 0x20
        })
    );
    let b = with(blobs::edid_14_cvt(), at::DESCRIPTOR_2 + 11, 0x10);
    assert_eq!(
        parse_edid(&b),
        Err(EdidError::Padding {
            byte: 11,
            value: 0x10,
            expected: 0x11
        })
    );
}

#[test]
fn ten_mhz_limit_leaves_no_pixel_clock() {
    let b = with(blobs::pc_crt_30_71(), at::DESCRIPTOR_2 + 9, 1);
    assert_eq!(
        parse_edid(&b),
        Err(EdidError::NoPixelClock {
            coarse_10mhz: 1,
            reduction_250khz: 0
        })
    );
    let b = with(blobs::edid_14_cvt(), at::DESCRIPTOR_2 + 9, 1);
    let b = with(b, at::DESCRIPTOR_2 + 12, 40 << 2);
    assert_eq!(
        parse_edid(&b),
        Err(EdidError::NoPixelClock {
            coarse_10mhz: 1,
            reduction_250khz: 40
        })
    );
}
