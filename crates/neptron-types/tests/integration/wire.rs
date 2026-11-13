use neptron_types::{
    Control, CrtRange, Filter, InputEvent, Modeline, MonitorKind, MonitorProfile, PixelFormat,
    Rotation, ScanlineKind, ScanlineMode, Scanlines, ScreenClass, SourceGeometry, TvStandard,
};
use serde::Serialize;
use serde::de::DeserializeOwned;

const CRT_TOML: &str = r#"
id = "test_15"

[[kind.crt.ranges]]
h_min_mhz = 15625000
h_max_mhz = 15750000
v_min_mhz = 49500
v_max_mhz = 65000
h_front_porch_ns = 2000
h_sync_ns = 4700
h_back_porch_ns = 8000
v_front_porch_ns = 64000
v_sync_ns = 192000
v_back_porch_ns = 1024000
hsync_positive = false
vsync_positive = true
progressive_lines_min = 192
progressive_lines_max = 288
interlaced_lines_min = 448
interlaced_lines_max = 576
"#;

fn test_range() -> CrtRange {
    CrtRange {
        h_min_mhz: 15_625_000,
        h_max_mhz: 15_750_000,
        v_min_mhz: 49_500,
        v_max_mhz: 65_000,
        h_front_porch_ns: 2_000,
        h_sync_ns: 4_700,
        h_back_porch_ns: 8_000,
        v_front_porch_ns: 64_000,
        v_sync_ns: 192_000,
        v_back_porch_ns: 1_024_000,
        hsync_positive: false,
        vsync_positive: true,
        progressive_lines_min: 192,
        progressive_lines_max: 288,
        interlaced_lines_min: 448,
        interlaced_lines_max: 576,
    }
}

#[test]
fn crt_profile_reads_from_toml() {
    let profile: MonitorProfile = toml::from_str(CRT_TOML).unwrap();
    assert_eq!(
        profile,
        MonitorProfile {
            id: "test_15".to_owned(),
            kind: MonitorKind::Crt {
                ranges: vec![test_range()],
            },
        }
    );
}

#[test]
fn crt_profile_with_a_missing_field_is_an_error() {
    let broken = CRT_TOML.replace("h_sync_ns = 4700\n", "");
    let err = toml::from_str::<MonitorProfile>(&broken).unwrap_err();
    assert!(err.to_string().contains("h_sync_ns"), "{err}");
}

#[test]
fn flat_panel_profile_round_trips_through_json() {
    let profile = MonitorProfile {
        id: "panel".to_owned(),
        kind: MonitorKind::FlatPanel {
            modes: vec![Modeline {
                pclock_hz: 25_175_000,
                hactive: 640,
                hsync_start: 656,
                hsync_end: 752,
                htotal: 800,
                vactive: 480,
                vsync_start: 490,
                vsync_end: 492,
                vtotal: 525,
                interlace: false,
                doublescan: false,
                hsync_positive: false,
                vsync_positive: false,
            }],
        },
    };
    let json = serde_json::to_string(&profile).unwrap();
    assert!(
        json.starts_with(r#"{"id":"panel","kind":{"flat_panel":{"modes":[{"pclock_hz":25175000,"#)
    );
    let back: MonitorProfile = serde_json::from_str(&json).unwrap();
    assert_eq!(back, profile);
}

#[test]
fn input_event_json_shape() {
    let event = InputEvent {
        device: 2,
        control: Control::LeftX,
        value: -32768,
    };
    let json = serde_json::to_string(&event).unwrap();
    assert_eq!(json, r#"{"device":2,"control":"left_x","value":-32768}"#);
    assert_eq!(serde_json::from_str::<InputEvent>(&json).unwrap(), event);
}

#[test]
fn source_geometry_round_trips_through_json() {
    let source = SourceGeometry {
        width: 256,
        height: 224,
        refresh_mhz: 60_098,
        aspect_milli: 1_333,
        interlaced: false,
        rotation: Rotation::R90,
    };
    let json = serde_json::to_string(&source).unwrap();
    assert_eq!(
        serde_json::from_str::<SourceGeometry>(&json).unwrap(),
        source
    );
}

#[test]
fn a_negative_rate_is_an_error() {
    let broken = CRT_TOML.replace("v_min_mhz = 49500", "v_min_mhz = -1");
    assert!(toml::from_str::<MonitorProfile>(&broken).is_err());
}

/// Serialises each value, compares it with its pinned JSON string, and reads
/// it back. A renamed variant fails here, because the wire format is
/// append-only.
fn pin<T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug>(cases: &[(T, &str)]) {
    for (value, name) in cases {
        let json = serde_json::to_string(value).unwrap();
        assert_eq!(json, format!("\"{name}\""), "{value:?}");
        assert_eq!(&serde_json::from_str::<T>(&json).unwrap(), value);
    }
}

#[test]
fn scanlines_json_shape() {
    let black = Scanlines {
        kind: ScanlineKind::Black,
        strength_percent: 75,
    };
    let json = serde_json::to_string(&black).unwrap();
    assert_eq!(json, r#"{"kind":"black","strength_percent":75}"#);
    assert_eq!(serde_json::from_str::<Scanlines>(&json).unwrap(), black);
    let none = serde_json::from_str::<Scanlines>(r#"{"kind":"none","strength_percent":0}"#);
    assert_eq!(none.unwrap(), Scanlines::NONE);
}

#[test]
fn enum_wire_names_are_pinned() {
    pin(&[
        (Rotation::R0, "r0"),
        (Rotation::R90, "r90"),
        (Rotation::R180, "r180"),
        (Rotation::R270, "r270"),
    ]);
    pin(&[
        (PixelFormat::Xrgb8888, "xrgb8888"),
        (PixelFormat::Rgb565, "rgb565"),
        (PixelFormat::Xrgb1555, "xrgb1555"),
    ]);
    pin(&[
        (Filter::Nearest, "nearest"),
        (Filter::SharpBilinear, "sharp_bilinear"),
    ]);
    pin(&[(ScanlineKind::None, "none"), (ScanlineKind::Black, "black")]);
    pin(&[
        (ScanlineMode::Auto, "auto"),
        (ScanlineMode::Off, "off"),
        (ScanlineMode::On, "on"),
    ]);
    pin(&[
        (ScreenClass::Crt15khz, "crt15khz"),
        (ScreenClass::Crt31khz, "crt31khz"),
        (ScreenClass::FlatPanel, "flat_panel"),
    ]);
    pin(&[
        (Control::B, "b"),
        (Control::Y, "y"),
        (Control::Select, "select"),
        (Control::Start, "start"),
        (Control::Up, "up"),
        (Control::Down, "down"),
        (Control::Left, "left"),
        (Control::Right, "right"),
        (Control::A, "a"),
        (Control::X, "x"),
        (Control::L, "l"),
        (Control::R, "r"),
        (Control::L2, "l2"),
        (Control::R2, "r2"),
        (Control::L3, "l3"),
        (Control::R3, "r3"),
        (Control::LeftX, "left_x"),
        (Control::LeftY, "left_y"),
        (Control::RightX, "right_x"),
        (Control::RightY, "right_y"),
    ]);
}

#[test]
fn monitor_kind_is_externally_tagged_in_snake_case() {
    let crt = MonitorKind::Crt { ranges: vec![] };
    let panel = MonitorKind::FlatPanel { modes: vec![] };
    let tv = MonitorKind::Tv {
        standard: TvStandard::Ntsc443,
    };
    assert_eq!(
        serde_json::to_string(&tv).unwrap(),
        r#"{"tv":{"standard":"ntsc_443"}}"#
    );
    assert_eq!(
        serde_json::to_string(&crt).unwrap(),
        r#"{"crt":{"ranges":[]}}"#
    );
    assert_eq!(
        serde_json::to_string(&panel).unwrap(),
        r#"{"flat_panel":{"modes":[]}}"#
    );
}

#[test]
fn tv_standards_serialise_as_their_keys() {
    let cases: Vec<(TvStandard, &str)> = TvStandard::ALL.iter().map(|s| (*s, s.key())).collect();
    pin(&cases);
    pin(&[
        (TvStandard::Ntsc, "ntsc"),
        (TvStandard::NtscJ, "ntsc_j"),
        (TvStandard::Ntsc443, "ntsc_443"),
        (TvStandard::Pal60, "pal_60"),
        (TvStandard::PalM, "pal_m"),
        (TvStandard::Pal, "pal"),
        (TvStandard::PalN, "pal_n"),
        (TvStandard::Secam, "secam"),
        (TvStandard::Mono60, "mono_60"),
        (TvStandard::Mono50, "mono_50"),
    ]);
}

#[test]
fn tv_profile_reads_from_toml_and_round_trips_through_json() {
    let text = "id = \"tv_pal\"\n\n[kind.tv]\nstandard = \"pal\"\n";
    let profile: MonitorProfile = toml::from_str(text).unwrap();
    assert_eq!(profile, TvStandard::Pal.profile());
    let json = serde_json::to_string(&profile).unwrap();
    assert_eq!(json, r#"{"id":"tv_pal","kind":{"tv":{"standard":"pal"}}}"#);
    assert_eq!(
        serde_json::from_str::<MonitorProfile>(&json).unwrap(),
        profile
    );
    let unknown = text.replace("\"pal\"", "\"pal_b\"");
    assert!(toml::from_str::<MonitorProfile>(&unknown).is_err());
}
