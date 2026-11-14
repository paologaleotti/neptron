use neptron_platform::{Connector, Gpu, UNKNOWN_GPU_CAPS};
use neptron_types::{MonitorKind, MonitorProfile, TvStandard};

use super::*;

/// EDID blobs of `tests/fixtures/edid`, made from the VESA E-EDID and
/// CTA-861 specifications. No blob is the EDID of a real display.
const PANEL_1080P: &[u8] = include_bytes!("../../tests/fixtures/edid/panel_1080p.bin");
const PANEL_2160P: &[u8] = include_bytes!("../../tests/fixtures/edid/panel_2160p.bin");
const TV_720P: &[u8] = include_bytes!("../../tests/fixtures/edid/tv_720p.bin");
const CRT_ANALOG: &[u8] = include_bytes!("../../tests/fixtures/edid/crt_analog.bin");

fn presets() -> Presets {
    let profile = |id: &str| MonitorProfile {
        id: id.to_owned(),
        kind: MonitorKind::Crt { ranges: Vec::new() },
    };
    Presets::from_profiles(vec![profile(SAFE_START_PROFILE), profile("pc_31_120")])
}

fn connector(name: &str, kind: &str, edid: Option<&[u8]>) -> Connector {
    Connector {
        card: "card0".to_owned(),
        name: name.to_owned(),
        kind: kind.to_owned(),
        status: ConnectorStatus::Connected,
        edid: edid.map(<[u8]>::to_vec),
    }
}

fn platform(connectors: Vec<Connector>) -> Platform {
    Platform {
        gpus: vec![Gpu {
            card: "card0".to_owned(),
            driver: Some("i915".to_owned()),
            pci_id: None,
            of_compatible: Vec::new(),
        }],
        connectors,
    }
}

fn broken() -> Vec<u8> {
    let mut b = PANEL_1080P.to_vec();
    b[0x40] ^= 0x01;
    b
}

#[test]
fn with_no_edid_the_safe_default_is_the_15_khz_start() {
    let safe = Detected::default().safe(&presets(), None).expect("safe");
    let detected = Detected::default();
    assert_eq!(detected.monitor_setting(&safe.monitor), SAFE_START_PROFILE);
    assert_eq!(safe.settings.style.key(), "auto");
    assert_eq!(safe.settings.refresh.key(), "exact");
    assert_eq!(
        Detected::default().safe(&Presets::default(), None),
        Err(MonitorError::NoSafeProfile)
    );
}

/// What `auto` drives on each display: `(what, connector type,
/// subconnector, EDID, monitor id)`.
type Row<'a> = (&'a str, &'a str, Option<&'a str>, Option<&'a [u8]>, &'a str);

#[test]
fn auto_and_the_safe_start_of_each_display() {
    let broken = broken();
    let rows: [Row<'_>; 9] = [
        (
            "1080p HDMI panel",
            "HDMI-A",
            None,
            Some(PANEL_1080P),
            "panel_npt_1080",
        ),
        (
            "4K DisplayPort panel",
            "DP",
            Some("Native"),
            Some(PANEL_2160P),
            "panel_npt_2160",
        ),
        ("720p TV", "HDMI-A", None, Some(TV_720P), "panel_npt_0720"),
        (
            "DP with an analog EDID",
            "DP",
            None,
            Some(CRT_ANALOG),
            "edid_npt_0071",
        ),
        (
            "HDMI with an analog EDID",
            "HDMI-A",
            None,
            Some(CRT_ANALOG),
            "edid_npt_0071",
        ),
        (
            "DP to VGA with a digital EDID",
            "DP",
            Some("VGA"),
            Some(PANEL_1080P),
            SAFE_START_PROFILE,
        ),
        ("VGA CRT", "VGA", None, Some(CRT_ANALOG), "edid_npt_0071"),
        (
            "broken EDID",
            "HDMI-A",
            None,
            Some(&broken),
            SAFE_START_PROFILE,
        ),
        ("no EDID", "HDMI-A", None, None, SAFE_START_PROFILE),
    ];
    for (what, kind, sub, edid, id) in rows {
        let name = format!("{kind}-1");
        let p = platform(vec![connector(&name, kind, edid)]);
        let mut detected = Detected::of(&p, &name, sub);
        detected.fit_to(&UNKNOWN_GPU_CAPS);
        let auto = detected.monitor(AUTO, &presets()).expect("auto");
        assert_eq!(auto.profile().id, id, "{what}");
        // A panel is the safe default only once it is kept on this
        // output; until then the safe start is the 15 kHz one.
        let safe = detected.safe(&presets(), None).expect("safe");
        let unconfirmed = if id.starts_with("panel_") {
            SAFE_START_PROFILE
        } else {
            id
        };
        assert_eq!(safe.monitor.profile().id, unconfirmed, "{what}");
        let kept = detected.safe(&presets(), Some(&auto)).expect("safe");
        assert_eq!(kept.monitor.profile().id, id, "{what} kept");
        assert_eq!(
            detected.notices.is_empty(),
            !matches!(
                what,
                "broken EDID" | "DP to VGA with a digital EDID" | "4K DisplayPort panel"
            ),
            "{what}: {:?}",
            detected.notices
        );
    }
}

#[test]
fn a_named_output_never_takes_the_edid_of_another_display() {
    let p = platform(vec![
        connector("HDMI-A-1", "HDMI-A", Some(PANEL_1080P)),
        connector("VGA-1", "VGA", None),
    ]);
    let vga = Detected::of(&p, "VGA-1", None);
    assert_eq!(vga.edid, None);
    assert_eq!(vga.panel, None);
    let auto = Detected::of(&p, AUTO, None);
    assert_eq!(
        auto.edid.map(|e| e.profile.id).as_deref(),
        Some("panel_npt_1080")
    );
    assert_eq!(vga.outputs, ["HDMI-A-1", "VGA-1"]);
    // A VGA output with no EDID is often `unknown` (a 15 kHz CRT), and a
    // named output can be `disconnected`: neither takes the panel's EDID.
    for status in [ConnectorStatus::Unknown, ConnectorStatus::Disconnected] {
        let p = platform(vec![
            connector("HDMI-A-1", "HDMI-A", Some(PANEL_1080P)),
            Connector {
                status,
                ..connector("VGA-1", "VGA", None)
            },
        ]);
        let vga = Detected::of(&p, "VGA-1", None);
        assert_eq!(vga.edid, None, "{status:?}");
        assert_eq!(vga.panel, None, "{status:?}");
        let safe = vga.safe(&presets(), None).expect("safe");
        assert_eq!(safe.monitor.profile().id, SAFE_START_PROFILE, "{status:?}");
    }
    // A name that no connector has (a window, a typo) takes no EDID
    // either; only `auto` looks at the other displays.
    let window = Detected::of(&p, "window", None);
    assert_eq!(window.edid, None);
}

#[test]
fn a_1080p_panel_starts_in_its_native_mode() {
    let p = platform(vec![connector("HDMI-A-1", "HDMI-A", Some(PANEL_1080P))]);
    let mut detected = Detected::of(&p, "HDMI-A-1", None);
    detected.fit_to(&UNKNOWN_GPU_CAPS);
    assert_eq!(detected.auto_text(), "NPT FHD 24  1920x1080 60 Hz");
    assert_eq!(
        detected.flat_text().as_deref(),
        Some("NPT FHD 24  1920x1080 60 Hz")
    );
    assert!(!detected.is_crt(AUTO, &presets()));
    assert!(
        detected.is_crt("pc_31_120", &presets()),
        "a CRT preset wins"
    );
    assert!(!detected.is_crt(FLAT, &presets()));
    assert_eq!(detected.panel_size(), Some((1920, 1080)));
    let auto = detected.monitor(AUTO, &presets()).expect("auto");
    assert_eq!(detected.monitor_setting(&auto), AUTO);
    assert!(detected.auto_is_panel());
    let safe = detected.safe(&presets(), None).expect("safe");
    assert_eq!(safe.monitor.profile().id, SAFE_START_PROFILE);
    assert_eq!(detected.monitor_setting(&safe.monitor), SAFE_START_PROFILE);
}

#[test]
fn a_4k_panel_above_the_limits_starts_in_its_largest_mode_that_fits() {
    let p = platform(vec![connector("DP-1", "DP", Some(PANEL_2160P))]);
    let mut detected = Detected::of(&p, "DP-1", None);
    assert_eq!(detected.panel_size(), Some((3840, 2160)), "before the fit");
    detected.fit_to(&UNKNOWN_GPU_CAPS);
    assert_eq!(detected.auto_text(), "NPT UHD 27  1920x1080 60 Hz");
    assert_eq!(
        detected.notices,
        [
            "The panel's native mode 3840x2160 60 Hz is above what this system sends (1920x1200 at 165 MHz). It runs at 1920x1080 60 Hz."
        ]
    );
    let wide = GpuCaps {
        pclock_max_hz: 600_000_000,
        max_width: 4096,
        max_height: 4096,
        ..UNKNOWN_GPU_CAPS
    };
    let mut native = Detected::of(&p, "DP-1", None);
    native.fit_to(&wide);
    assert_eq!(native.auto_text(), "NPT UHD 27  3840x2160 60 Hz");
    assert!(native.notices.is_empty());
}

#[test]
fn flat_drives_an_lcd_with_a_vga_input_in_its_listed_modes() {
    let p = platform(vec![connector("DP-1", "DP", Some(CRT_ANALOG))]);
    let mut detected = Detected::of(&p, "DP-1", Some("VGA"));
    detected.fit_to(&UNKNOWN_GPU_CAPS);
    let safe = detected.safe(&presets(), None).expect("safe");
    assert_eq!(
        safe.monitor.profile().id,
        "edid_npt_0071",
        "auto is the CRT path"
    );
    assert_eq!(detected.monitor_setting(&safe.monitor), AUTO);
    assert!(detected.is_crt(AUTO, &presets()));
    let flat = detected.monitor(FLAT, &presets()).expect("flat");
    assert_eq!(flat.profile().id, "panel_npt_0071");
    assert_eq!(detected.monitor_setting(&flat), FLAT);
    let entry = MonitorRef::of(&flat);
    assert_eq!(detected.rebuild(&presets(), &entry), Some(flat));
    assert_eq!(
        Detected::default().monitor(FLAT, &presets()),
        Err(MonitorError::NoPanel)
    );
}

#[test]
fn a_monitor_setting_names_a_preset_or_auto() {
    let detected = Detected::default();
    let safe = detected.safe(&presets(), None).expect("safe");
    let auto = detected.monitor(AUTO, &presets()).expect("auto");
    assert_eq!(auto, safe.monitor);
    let pc = detected.monitor("pc_31_120", &presets()).expect("pc");
    assert_eq!(detected.monitor_setting(&pc), "pc_31_120");
    assert_eq!(
        detected.monitor("gone", &presets()),
        Err(MonitorError::Unknown("gone".to_owned()))
    );
    assert_eq!(detected.auto_text(), SAFE_START_PROFILE);
    assert_eq!(detected.flat_text(), None);
    assert!(detected.is_crt(AUTO, &presets()));
}

#[test]
fn a_record_entry_is_built_again() {
    let detected = Detected::default();
    let id = |id: &str| MonitorRef::Profile { id: id.to_owned() };
    assert!(detected.rebuild(&presets(), &id("pc_31_120")).is_some());
    assert!(detected.rebuild(&presets(), &id("gone")).is_none());
    let edid = MonitorRef::Edid {
        id: "edid_npt_0071".to_owned(),
    };
    assert!(detected.rebuild(&presets(), &edid).is_none(), "no EDID now");
    let p = platform(vec![connector("HDMI-A-1", "HDMI-A", Some(PANEL_1080P))]);
    let panel = Detected::of(&p, "HDMI-A-1", None);
    assert!(
        panel.rebuild(&presets(), &edid).is_none(),
        "another display"
    );
    let entry = MonitorRef::Edid {
        id: "panel_npt_1080".to_owned(),
    };
    assert!(panel.rebuild(&presets(), &entry).is_some());
}

#[test]
fn an_unreadable_sysfs_detects_nothing() {
    assert_eq!(
        Detected::read(Path::new("/nonexistent/sys"), AUTO, None),
        Detected::default()
    );
}

#[test]
fn every_mode_of_the_fixtures_has_a_whole_khz_clock() {
    for edid in [PANEL_1080P, PANEL_2160P, TV_720P, CRT_ANALOG] {
        let panel = panel_profile_from_edid(edid).expect("a panel profile");
        let MonitorKind::FlatPanel { modes } = panel.profile.kind else {
            panic!("a panel");
        };
        for mode in modes {
            assert!(mode.pclock_is_whole_khz(), "{mode:?}");
        }
    }
}

const PANEL_2160P_RB: &[u8] = include_bytes!("../../tests/fixtures/edid/panel_2160p_rb.bin");

/// An HD 530, the GPU of the bench.
fn hd_530(connectors: Vec<Connector>) -> Platform {
    let mut p = platform(connectors);
    p.gpus[0].pci_id = Some(neptron_platform::PciId {
        vendor: 0x8086,
        device: 0x1912,
    });
    p
}

#[test]
fn a_4k_displayport_panel_runs_in_its_native_mode_on_an_hd_530() {
    let p = hd_530(vec![connector("DP-1", "DP", Some(PANEL_2160P_RB))]);
    let mut detected = Detected::of(&p, "DP-1", None);
    let caps = detected.backend_caps(AUTO);
    assert_eq!(
        (caps.pclock_max_hz, caps.max_width, caps.max_height),
        (533_250_000, 3840, 2160),
        "the buffers hold the native mode"
    );
    detected.fit_to(&caps);
    assert_eq!(detected.auto_text(), "NPT UHD DP  3840x2160 59.99 Hz");
    assert!(detected.notices.is_empty(), "{:?}", detected.notices);
}

#[test]
fn a_4k_hdmi_panel_falls_to_1080p60_not_2160p30() {
    // HDMI 1.4 on the HD 530: 300 MHz. 2160p60 and 2160p50 (594 MHz) are
    // out, 2160p30 would halve a 60 Hz game.
    let p = hd_530(vec![connector("HDMI-A-1", "HDMI-A", Some(PANEL_2160P))]);
    let mut detected = Detected::of(&p, "HDMI-A-1", None);
    let caps = detected.backend_caps(AUTO);
    assert_eq!(caps.pclock_max_hz, 300_000_000);
    detected.fit_to(&caps);
    assert_eq!(detected.auto_text(), "NPT UHD 27  1920x1080 60 Hz");
    assert_eq!(detected.notices.len(), 1, "{:?}", detected.notices);
}

#[test]
fn the_crt_path_keeps_its_caps_and_buffers() {
    let analog = |kind: &str, sub| {
        let p = hd_530(vec![connector(
            &format!("{kind}-1"),
            kind,
            Some(CRT_ANALOG),
        )]);
        Detected::of(&p, &format!("{kind}-1"), sub).backend_caps(AUTO)
    };
    let bench = analog("DP", None);
    assert_eq!(
        (bench.pclock_max_hz, bench.max_width, bench.max_height),
        (165_000_000, 1920, 1200),
        "the DP-to-VGA bench"
    );
    assert_eq!(analog("VGA", None), bench);
    // A CRT preset on a panel output keeps the analog caps.
    let p = hd_530(vec![connector("DP-1", "DP", Some(PANEL_2160P_RB))]);
    let panel = Detected::of(&p, "DP-1", None);
    let preset = panel.backend_caps("pc_31_120");
    assert_eq!(preset.pclock_max_hz, 165_000_000);
}

const PANEL_1440P: &[u8] = include_bytes!("../../tests/fixtures/edid/panel_1440p.bin");

/// An Intel GPU of gen12 or newer on i915: no digital limits in the table.
fn gen12(connectors: Vec<Connector>) -> Platform {
    let mut p = hd_530(connectors);
    p.gpus[0].pci_id = Some(neptron_platform::PciId {
        vendor: 0x8086,
        device: 0x9a49,
    });
    p
}

/// What `auto` starts in for `edid` on connector `kind` of `platform`.
fn start(platform: fn(Vec<Connector>) -> Platform, kind: &str, edid: &[u8]) -> (String, GpuCaps) {
    let name = format!("{kind}-1");
    let p = platform(vec![connector(&name, kind, Some(edid))]);
    let mut detected = Detected::of(&p, &name, None);
    let caps = detected.backend_caps(AUTO);
    detected.fit_to(&caps);
    (detected.auto_text(), caps)
}

/// What a start shows: (what, GPU, connector type, EDID, `auto` text).
type StartRow<'a> = (
    &'a str,
    fn(Vec<Connector>) -> Platform,
    &'a str,
    &'a [u8],
    &'a str,
);

#[test]
fn a_digital_link_takes_the_panel_native_mode_when_the_link_allows_it() {
    let rows: [StartRow<'_>; 6] = [
        (
            "1440p on DP",
            hd_530,
            "DP",
            PANEL_1440P,
            "NPT QHD 27  2560x1440 59.95 Hz",
        ),
        (
            "1440p on HDMI",
            hd_530,
            "HDMI-A",
            PANEL_1440P,
            "NPT QHD 27  2560x1440 59.95 Hz",
        ),
        (
            "2160p on DP",
            hd_530,
            "DP",
            PANEL_2160P_RB,
            "NPT UHD DP  3840x2160 59.99 Hz",
        ),
        (
            "2160p on HDMI 1.4",
            hd_530,
            "HDMI-A",
            PANEL_2160P_RB,
            "NPT UHD DP  1920x1080 60 Hz",
        ),
        (
            "1440p, no digital limits",
            gen12,
            "DP",
            PANEL_1440P,
            "NPT QHD 27  1920x1080 60 Hz",
        ),
        (
            "1080p on DP",
            hd_530,
            "DP",
            PANEL_1080P,
            "NPT FHD 24  1920x1080 60 Hz",
        ),
    ];
    for (what, platform, kind, edid, text) in rows {
        let (shown, caps) = start(platform, kind, edid);
        assert_eq!(shown, text, "{what}");
        assert!(caps.pclock_max_hz >= 165_000_000, "{what}");
    }
}

#[test]
fn the_output_kind_picks_the_limits() {
    // (what, connector, subconnector, EDID, monitor setting, output kind,
    // highest pixel clock, buffer size)
    let analog = (OutputKind::Analog, 165_000_000, (1920, 1200));
    type Row<'a> = (
        &'a str,
        &'a str,
        Option<&'a str>,
        &'a [u8],
        &'a str,
        (OutputKind, u32, (u16, u16)),
    );
    let rows: [Row<'_>; 7] = [
        (
            "panel on DP",
            "DP",
            None,
            PANEL_2160P_RB,
            AUTO,
            (OutputKind::Digital, 533_250_000, (3840, 2160)),
        ),
        (
            "panel on HDMI",
            "HDMI-A",
            None,
            PANEL_1440P,
            AUTO,
            (OutputKind::Digital, 300_000_000, (2560, 1440)),
        ),
        (
            "flat on the same panel",
            "DP",
            None,
            PANEL_1440P,
            FLAT,
            (OutputKind::Digital, 533_250_000, (2560, 1440)),
        ),
        (
            "the bench: analog EDID over DP",
            "DP",
            None,
            CRT_ANALOG,
            AUTO,
            analog,
        ),
        (
            "flat on the bench keeps the analog limits",
            "DP",
            None,
            CRT_ANALOG,
            FLAT,
            analog,
        ),
        (
            "a CRT preset on a panel output",
            "DP",
            None,
            PANEL_1440P,
            "pc_31_120",
            analog,
        ),
        (
            "a DAC with a digital EDID",
            "DP",
            Some("VGA"),
            PANEL_1440P,
            AUTO,
            analog,
        ),
    ];
    for (what, kind, sub, edid, name, (output, clock, size)) in rows {
        let connector_name = format!("{kind}-1");
        let p = hd_530(vec![connector(&connector_name, kind, Some(edid))]);
        let detected = Detected::of(&p, &connector_name, sub);
        assert_eq!(detected.output_for(name), output, "{what}");
        let caps = detected.backend_caps(name);
        assert_eq!(
            (caps.pclock_max_hz, (caps.max_width, caps.max_height)),
            (clock, size),
            "{what}"
        );
    }
    assert_eq!(Detected::default().output, OutputKind::Analog, "no EDID");
}

#[test]
fn before_the_display_opens_the_caps_follow_the_profile() {
    // auto with a VGA output first: the caps come from the HDMI panel that
    // gave the profile, not from the first connected output.
    let p = hd_530(vec![
        connector("HDMI-A-1", "HDMI-A", Some(PANEL_1440P)),
        connector("VGA-1", "VGA", None),
    ]);
    let mut p2 = p.clone();
    p2.connectors.reverse();
    let auto = Detected::of(&p2, AUTO, None);
    assert_eq!(
        auto.digital_caps.map(|c| c.pclock_max_hz),
        Some(300_000_000)
    );
    assert_eq!(auto.backend_caps(AUTO).pclock_max_hz, 300_000_000);
    let named = Detected::of(&p2, "VGA-1", None);
    assert_eq!(named.backend_caps(AUTO).pclock_max_hz, 165_000_000);
}

#[test]
fn an_analog_output_never_runs_with_the_digital_limits() {
    // The display opened with the digital caps of a DP panel EDID, then
    // the kernel reports a VGA subconnector: a DAC. The policy gets the
    // analog caps.
    let p = hd_530(vec![connector("DP-1", "DP", Some(PANEL_1440P))]);
    let before = Detected::of(&p, AUTO, None);
    let opened = before.backend_caps(AUTO);
    assert_eq!(opened.pclock_max_hz, 533_250_000);
    let after = Detected::of(&p, "DP-1", Some("VGA"));
    assert_eq!(after.output, OutputKind::Analog);
    let policy = after.policy_caps(&opened);
    assert_eq!(
        (policy.pclock_max_hz, policy.max_width, policy.max_height),
        (165_000_000, 1920, 1200)
    );
    assert_eq!(policy.pclock_min_hz, opened.pclock_min_hz);
    // A digital adapter with no EDID, opened ahead of a panel: analog.
    let bare = hd_530(vec![
        connector("DP-1", "DP", None),
        connector("HDMI-A-1", "HDMI-A", Some(PANEL_1440P)),
    ]);
    let adapter = Detected::of(&bare, "DP-1", None);
    assert_eq!(adapter.output, OutputKind::Analog);
    assert_eq!(adapter.policy_caps(&opened).pclock_max_hz, 165_000_000);
    // A digital panel keeps the caps the display opened with.
    let panel = Detected::of(&p, "DP-1", None);
    assert_eq!(panel.policy_caps(&opened), opened);
}

#[test]
fn the_identity_names_the_monitor_on_the_driven_connector() {
    assert_eq!(Detected::default().identity(), NO_EDID_IDENTITY);
    let one = platform(vec![
        connector("VGA-1", "VGA", Some(CRT_ANALOG)),
        connector("HDMI-A-1", "HDMI-A", Some(PANEL_1080P)),
    ]);
    let vga = Detected::of(&one, "VGA-1", None).identity();
    let hdmi = Detected::of(&one, "HDMI-A-1", None).identity();
    assert!(vga.starts_with("edid-") && vga.len() == 21, "{vga}");
    assert_ne!(vga, hdmi);
    assert_eq!(vga, Detected::of(&one, "VGA-1", None).identity(), "stable");
    let blind = platform(vec![connector("VGA-1", "VGA", None)]);
    assert_eq!(Detected::of(&blind, "VGA-1", None).identity(), "none");
    assert_eq!(
        Detected::of(&one, AUTO, None).identity(),
        "none",
        "only the driven connector counts"
    );
}

/// A Pi 3: HDMI with no display and the composite connector, whose status
/// is always `unknown`.
fn pi3() -> Platform {
    Platform {
        gpus: vec![Gpu {
            card: "card1".to_owned(),
            driver: Some("vc4-drm".to_owned()),
            pci_id: None,
            of_compatible: vec!["brcm,bcm2835-vc4".to_owned()],
        }],
        connectors: vec![
            Connector {
                card: "card1".to_owned(),
                status: ConnectorStatus::Disconnected,
                ..connector("HDMI-A-1", "HDMI-A", None)
            },
            Connector {
                card: "card1".to_owned(),
                status: ConnectorStatus::Unknown,
                ..connector("Composite-1", "Composite", None)
            },
        ],
    }
}

fn composite(offered: &[TvStandard]) -> Detected {
    Detected::of(&pi3(), "Composite-1", None).with_tv(Some(offered.to_vec()))
}

#[test]
fn a_tv_output_drives_the_profile_of_its_standard_whatever_the_monitor() {
    let detected = composite(&TvStandard::ALL);
    assert!(detected.is_tv());
    assert_eq!(
        detected.caps.map(|c| c.max_height),
        Some(576),
        "the composite entry"
    );
    for (monitor, tv, expected) in [
        (AUTO, AUTO, TvStandard::Ntsc),
        (AUTO, "pal", TvStandard::Pal),
        ("pc_31_120", "pal_60", TvStandard::Pal60),
        (FLAT, "mono_50", TvStandard::Mono50),
    ] {
        let wanted = detected
            .wanted(monitor, tv, &presets())
            .expect("a TV profile");
        assert_eq!(
            wanted,
            Monitor::Profile(expected.profile()),
            "{monitor} {tv}"
        );
        assert_eq!(detected.monitor_setting(&wanted), AUTO, "never a TV id");
        assert_eq!(tv_setting(&wanted), Some(expected.key()));
    }
    let pc = detected.monitor("pc_31_120", &presets()).expect("pc");
    assert_eq!(tv_setting(&pc), None);
}

#[test]
fn a_standard_the_tv_output_does_not_offer_is_an_error() {
    let detected = composite(&[TvStandard::Ntsc, TvStandard::Pal]);
    assert_eq!(
        detected.wanted(AUTO, "secam", &presets()),
        Err(MonitorError::TvStandard("secam".to_owned()))
    );
    assert_eq!(
        detected.wanted(AUTO, "pal_70", &presets()),
        Err(MonitorError::TvStandard("pal_70".to_owned()))
    );
    assert!(detected.wanted(AUTO, "pal", &presets()).is_ok());
}

#[test]
fn the_safe_default_of_a_tv_output_is_ntsc() {
    for confirmed in [None, Some(Monitor::Profile(TvStandard::Pal.profile()))] {
        let safe = composite(&TvStandard::ALL)
            .safe(&Presets::default(), confirmed.as_ref())
            .expect("no preset is needed");
        assert_eq!(safe.monitor, Monitor::Profile(TvStandard::Ntsc.profile()));
        assert_eq!(safe.settings.style.key(), "auto");
    }
    assert_eq!(
        composite(&[]).safe(&presets(), None),
        Err(MonitorError::NoTvStandard),
        "never a profile the connector refuses"
    );
    assert!(!usable_tv_output(Some(&[])));
    assert!(usable_tv_output(Some(&[TvStandard::Pal])));
    assert!(usable_tv_output(None));
    let pal_only = composite(&[TvStandard::Pal, TvStandard::Secam]);
    let safe = pal_only.safe(&presets(), None).expect("safe");
    assert_eq!(
        safe.monitor,
        Monitor::Profile(TvStandard::Pal.profile()),
        "the first standard offered"
    );
}

#[test]
fn a_record_gives_a_tv_profile_only_on_a_tv_output_that_offers_it() {
    let id = |id: &str| MonitorRef::Profile { id: id.to_owned() };
    let tv = composite(&[TvStandard::Ntsc, TvStandard::Pal]);
    assert_eq!(
        tv.rebuild(&presets(), &id("tv_pal")),
        Some(Monitor::Profile(TvStandard::Pal.profile()))
    );
    assert_eq!(tv.rebuild(&presets(), &id("tv_secam")), None);
    assert_eq!(
        tv.rebuild(&presets(), &id("pc_31_120")),
        None,
        "no CRT preset"
    );
    assert_eq!(
        tv.rebuild(
            &presets(),
            &MonitorRef::Edid {
                id: "edid_x".to_owned()
            }
        ),
        None
    );
    let crt = Detected::default();
    assert_eq!(crt.rebuild(&presets(), &id("tv_ntsc")), None);
    assert!(crt.rebuild(&presets(), &id("pc_31_120")).is_some());
}

#[test]
fn the_tv_setting_names_a_standard_and_auto_is_ntsc() {
    assert_eq!(tv_standard(AUTO), Some(TvStandard::Ntsc));
    for standard in TvStandard::ALL {
        assert_eq!(tv_standard(standard.key()), Some(standard));
    }
    assert_eq!(tv_standard("pal_70"), None);
    for value in neptron_settings::TV_STANDARDS {
        assert!(tv_standard(value).is_some(), "{value}");
    }
}

#[test]
fn a_tv_output_with_no_property_is_no_tv() {
    let detected = Detected::of(&pi3(), "Composite-1", None);
    assert!(!detected.is_tv(), "the backend decides with the property");
    assert_eq!(
        detected.wanted(AUTO, "pal", &presets()),
        detected.monitor(AUTO, &presets())
    );
}
