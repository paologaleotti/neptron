use neptron_platform::{Connector, Gpu, UNKNOWN_GPU_CAPS};
use neptron_types::{MonitorKind, MonitorProfile};

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
