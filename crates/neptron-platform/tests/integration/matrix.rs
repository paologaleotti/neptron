//! The capability table: the shipped file, the lookup, and every rule.

use std::path::Path;

use neptron_platform::{Gpu, Matrix, MatrixError, PciId, Quirk, UNKNOWN_GPU_CAPS, detect};
use neptron_types::OutputKind;

fn pci_gpu(driver: &str, vendor: u16, device: u16) -> Gpu {
    Gpu {
        card: "card0".into(),
        driver: Some(driver.into()),
        pci_id: Some(PciId { vendor, device }),
        of_compatible: vec![],
    }
}

fn of_gpu(driver: &str, compatible: &str) -> Gpu {
    Gpu {
        card: "card0".into(),
        driver: Some(driver.into()),
        pci_id: None,
        of_compatible: vec![compatible.into()],
    }
}

const ENTRY: &str = r#"
id = "one"
name = "One"
tier = 2
driver = "drv"
pclock_min_hz = 25_000_000
pclock_max_hz = 165_000_000
interlace = false
flip_per_field = false
max_width = 1920
max_height = 1200
quirks = []
"#;

/// A version 1 table with one entry: `ENTRY` with `extra` lines added.
fn table(extra: &str) -> String {
    format!("version = 1\n[[gpu]]\n{extra}\n{ENTRY}")
}

fn entry_rule(text: &str) -> &'static str {
    match Matrix::parse(text) {
        Err(MatrixError::Entry { rule, .. }) => rule,
        other => panic!("expected an entry error, got {other:?}"),
    }
}

#[test]
fn the_shipped_table_is_valid() {
    let matrix = Matrix::builtin().unwrap();
    assert!(matrix.entries().len() >= 9);
}

#[test]
fn hd_530_is_intel_gen9_with_conservative_values() {
    let matrix = Matrix::builtin().unwrap();
    let gpu = pci_gpu("i915", 0x8086, 0x1912);
    let entry = matrix.lookup(&gpu, "VGA").unwrap();
    assert_eq!(entry.id, "intel_gen9");
    assert_eq!(entry.tier, 1);
    // A 6.5 MHz clock is shown on a DP panel only, and 480i is not proven
    // on this machine: both stay off.
    assert_eq!(entry.caps.pclock_min_hz, 25_000_000);
    assert!(!entry.caps.interlace);
    assert!(entry.quirks.contains(&Quirk::LowPixelClockUnproven));
    assert!(entry.quirks.contains(&Quirk::InterlaceUnproven));
    assert!(entry.quirks.contains(&Quirk::InterlaceNeedsLinearScanout));
}

#[test]
fn other_intel_falls_back_to_the_driver_entry() {
    let matrix = Matrix::builtin().unwrap();
    let gen12 = pci_gpu("i915", 0x8086, 0x9a49);
    assert_eq!(matrix.lookup(&gen12, "DP").unwrap().id, "intel_i915");
    assert_eq!(matrix.lookup(&gen12, "DP").unwrap().tier, 2);
    let amber = pci_gpu("i915", 0x8086, 0x87c0);
    assert_eq!(matrix.lookup(&amber, "DP").unwrap().id, "intel_gen9");
}

#[test]
fn unknown_gpu_gets_the_unknown_caps() {
    let matrix = Matrix::builtin().unwrap();
    let bochs = pci_gpu("bochs-drm", 0x1234, 0x1111);
    assert_eq!(matrix.lookup(&bochs, "Virtual"), None);
    assert_eq!(matrix.caps_for(&bochs, "Virtual"), UNKNOWN_GPU_CAPS);
    assert_eq!(UNKNOWN_GPU_CAPS.pclock_min_hz, 25_000_000);
    const { assert!(!UNKNOWN_GPU_CAPS.interlace) };
    let no_driver = Gpu {
        driver: None,
        ..pci_gpu("", 0x8086, 0x1912)
    };
    assert_eq!(matrix.caps_for(&no_driver, "VGA"), UNKNOWN_GPU_CAPS);
}

#[test]
fn pi_entries_depend_on_the_connector() {
    let matrix = Matrix::builtin().unwrap();
    let vc4 = of_gpu("vc4-drm", "brcm,bcm2835-vc4");
    assert_eq!(matrix.lookup(&vc4, "DPI").unwrap().id, "pi_vc4_dpi");
    let composite = matrix.lookup(&vc4, "Composite").unwrap();
    assert_eq!(composite.id, "pi_vc4_composite");
    assert!(composite.caps.interlace);
    assert_eq!(composite.caps.pclock_min_hz, composite.caps.pclock_max_hz);
    assert_eq!(matrix.caps_for(&vc4, "HDMI-A"), UNKNOWN_GPU_CAPS);
    let pi4 = of_gpu("vc4-drm", "brcm,bcm2711-vc5");
    assert_eq!(matrix.lookup(&pi4, "DPI").unwrap().id, "pi_vc4_dpi");
    let dpi = matrix.lookup(&vc4, "DPI").unwrap();
    assert!(!dpi.caps.interlace);
}

#[test]
fn detection_and_lookup_together_on_fixture_trees() {
    let matrix = Matrix::builtin().unwrap();
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sysfs");
    let platform = detect(&fixtures.join("i3_6100t_vga")).unwrap();
    let vga = platform
        .connectors
        .iter()
        .find(|c| c.kind == "VGA")
        .unwrap();
    let gpu = platform.gpu_of(vga).unwrap();
    assert_eq!(matrix.lookup(gpu, &vga.kind).unwrap().id, "intel_gen9");
    let platform = detect(&fixtures.join("pi3_composite")).unwrap();
    let composite = &platform.connectors[0];
    let gpu = platform.gpu_of(composite).unwrap();
    assert_eq!(
        matrix.lookup(gpu, &composite.kind).unwrap().id,
        "pi_vc4_composite"
    );
    let platform = detect(&fixtures.join("qemu_bochs")).unwrap();
    let gpu = &platform.gpus[0];
    assert_eq!(matrix.caps_for(gpu, "Virtual"), UNKNOWN_GPU_CAPS);
}

#[test]
fn every_entry_that_marks_interlace_unproven_has_it_off() {
    for entry in Matrix::builtin().unwrap().entries() {
        if entry.quirks.contains(&Quirk::InterlaceUnproven) {
            assert!(!entry.caps.interlace, "{}", entry.id);
        }
        if entry.quirks.contains(&Quirk::LowPixelClockUnproven) {
            assert!(entry.caps.pclock_min_hz >= 25_000_000, "{}", entry.id);
        }
    }
}

#[test]
fn a_minimal_table_parses() {
    let matrix = Matrix::parse(&table("")).unwrap();
    let gpu = pci_gpu("drv", 1, 2);
    assert_eq!(matrix.lookup(&gpu, "VGA").unwrap().id, "one");
    assert!(Matrix::parse("version = 1\n").unwrap().entries().is_empty());
}

#[test]
fn other_versions_are_refused() {
    assert!(matches!(
        Matrix::parse("version = 2\n[[gpu]]\nnew_field = 1\n"),
        Err(MatrixError::Version {
            found: 2,
            supported: 1
        })
    ));
}

#[test]
fn bad_toml_and_unknown_fields_are_refused() {
    assert!(matches!(
        Matrix::parse("version = "),
        Err(MatrixError::Parse { .. })
    ));
    assert!(matches!(
        Matrix::parse(&table("colour = \"red\"")),
        Err(MatrixError::Parse { .. })
    ));
    assert!(matches!(
        Matrix::parse(&table("").replace("quirks = []", "quirks = [\"fast\"]")),
        Err(MatrixError::Parse { .. })
    ));
}

#[test]
fn entry_rules() {
    let replace = |from: &str, to: &str| table("").replace(from, to);
    assert_eq!(
        entry_rule(&replace("id = \"one\"", "id = \"One\"")),
        "id is not lower-case letters, digits and '_'"
    );
    assert_eq!(
        entry_rule(&replace("tier = 2", "tier = 4")),
        "tier is not 1, 2 or 3"
    );
    assert_eq!(
        entry_rule(&replace("driver = \"drv\"", "driver = \"\"")),
        "driver is empty"
    );
    assert_eq!(
        entry_rule(&table("pci_vendor = 0x8086")),
        "pci_vendor and pci_devices must come together"
    );
    assert_eq!(
        entry_rule(&table("pci_devices = [1]")),
        "pci_vendor and pci_devices must come together"
    );
    assert_eq!(
        entry_rule(&replace(
            "pclock_min_hz = 25_000_000",
            "pclock_min_hz = 200_000_000"
        )),
        "pclock_min_hz is above pclock_max_hz, or pclock_max_hz is 0"
    );
    assert_eq!(
        entry_rule(&replace("max_width = 1920", "max_width = 0")),
        "max_width or max_height is 0"
    );
    assert_eq!(
        entry_rule(
            &replace("interlace = false", "interlace = true")
                .replace("quirks = []", "quirks = [\"interlace_unproven\"]")
        ),
        "interlace_unproven with interlace on"
    );
    assert_eq!(
        entry_rule(
            &replace("interlace = false", "interlace = true")
                .replace("quirks = []", "quirks = [\"interlace_needs_kernel_patch\"]")
        ),
        "interlace_needs_kernel_patch with interlace on"
    );
    assert_eq!(
        entry_rule(
            &replace("pclock_min_hz = 25_000_000", "pclock_min_hz = 6_500_000")
                .replace("quirks = []", "quirks = [\"low_pixel_clock_unproven\"]")
        ),
        "low_pixel_clock_unproven with a lowest pixel clock below 25 MHz"
    );
    assert_eq!(
        entry_rule(&replace("quirks = []", "quirks = [\"fixed_pixel_clock\"]")),
        "fixed_pixel_clock with two pixel clock limits"
    );
}

#[test]
fn duplicate_ids_are_refused() {
    let text = format!("{}\n[[gpu]]\nconnectors = [\"DPI\"]\n{ENTRY}", table(""));
    assert!(matches!(
        Matrix::parse(&text),
        Err(MatrixError::DuplicateId { id }) if id == "one"
    ));
}

#[test]
fn overlapping_entries_of_the_same_precision_are_refused() {
    let second = ENTRY.replace("id = \"one\"", "id = \"two\"");
    let text = format!("{}\n[[gpu]]\n{second}", table(""));
    assert!(matches!(
        Matrix::parse(&text),
        Err(MatrixError::Ambiguous { first, second }) if first == "one" && second == "two"
    ));
    let text = format!(
        "{}\n[[gpu]]\npci_vendor = 1\npci_devices = [2, 3]\n{second}",
        table("pci_vendor = 1\npci_devices = [3, 4]")
    );
    assert!(matches!(
        Matrix::parse(&text),
        Err(MatrixError::Ambiguous { .. })
    ));
}

#[test]
fn disjoint_entries_of_the_same_precision_are_accepted() {
    let second = ENTRY.replace("id = \"one\"", "id = \"two\"");
    for (a, b) in [
        ("connectors = [\"DPI\"]", "connectors = [\"Composite\"]"),
        (
            "pci_vendor = 1\npci_devices = [2]",
            "pci_vendor = 1\npci_devices = [3]",
        ),
        ("of_compatible = [\"a\"]", "of_compatible = [\"b\"]"),
    ] {
        let text = format!("{}\n[[gpu]]\n{b}\n{second}", table(a));
        assert!(Matrix::parse(&text).is_ok(), "{a} / {b}");
    }
    let other_driver = second.replace("driver = \"drv\"", "driver = \"other\"");
    assert!(Matrix::parse(&format!("{}\n[[gpu]]\n{other_driver}", table(""))).is_ok());
}

#[test]
fn the_more_precise_entry_wins() {
    let second = ENTRY.replace("id = \"one\"", "id = \"two\"");
    let text = format!("{}\n[[gpu]]\nconnectors = [\"VGA\"]\n{second}", table(""));
    let matrix = Matrix::parse(&text).unwrap();
    let gpu = pci_gpu("drv", 1, 2);
    assert_eq!(matrix.lookup(&gpu, "VGA").unwrap().id, "two");
    assert_eq!(matrix.lookup(&gpu, "DP").unwrap().id, "one");
}

#[test]
fn hd_530_has_digital_limits_for_a_digital_output_only() {
    let matrix = Matrix::builtin().unwrap();
    let gpu = pci_gpu("i915", 0x8086, 0x1912);
    let analog = matrix.caps_for(&gpu, "DP");
    assert_eq!(
        (analog.pclock_max_hz, analog.max_width, analog.max_height),
        (165_000_000, 1920, 1200),
        "the CRT path keeps its limits on every connector"
    );
    for (kind, clock) in [
        ("HDMI-A", 300_000_000),
        ("DVI-D", 165_000_000),
        ("DP", 533_250_000),
    ] {
        let panel = matrix.caps_for_output(&gpu, kind, OutputKind::Digital);
        assert_eq!(
            (panel.pclock_max_hz, panel.max_width, panel.max_height),
            (clock, 4096, 2160),
            "{kind}"
        );
        assert_eq!(panel.pclock_min_hz, analog.pclock_min_hz, "{kind}");
        assert_eq!(panel.interlace, analog.interlace, "{kind}");
    }
    assert_eq!(
        matrix.caps_for_output(&gpu, "VGA", OutputKind::Digital),
        matrix.caps_for(&gpu, "VGA")
    );
    // The table of output kind against limits on the bench GPU.
    for (kind, output, limits) in [
        ("DP", OutputKind::Analog, (165_000_000, 1920, 1200)),
        ("HDMI-A", OutputKind::Analog, (165_000_000, 1920, 1200)),
        ("VGA", OutputKind::Analog, (165_000_000, 1920, 1200)),
        ("DP", OutputKind::Digital, (533_250_000, 4096, 2160)),
        ("HDMI-A", OutputKind::Digital, (300_000_000, 4096, 2160)),
        ("DVI-D", OutputKind::Digital, (165_000_000, 4096, 2160)),
    ] {
        let caps = matrix.caps_for_output(&gpu, kind, output);
        assert_eq!(
            (caps.pclock_max_hz, caps.max_width, caps.max_height),
            limits,
            "{kind} {output:?}"
        );
        if output == OutputKind::Analog {
            assert_eq!(caps, matrix.caps_for(&gpu, kind), "{kind}");
        }
    }
    let gen12 = pci_gpu("i915", 0x8086, 0x9a49);
    assert_eq!(
        matrix.caps_for_output(&gen12, "HDMI-A", OutputKind::Digital),
        matrix.caps_for(&gen12, "HDMI-A"),
        "no cited limit, no change"
    );
    let bochs = pci_gpu("bochs-drm", 0x1234, 0x1111);
    assert_eq!(
        matrix.caps_for_output(&bochs, "Virtual", OutputKind::Digital),
        UNKNOWN_GPU_CAPS
    );
}

#[test]
fn digital_limit_rules() {
    let panel = |body: &str| format!("{}\n[[gpu.digital]]\n{body}\n", table(""));
    let ok = panel(
        "connectors = [\"HDMI-A\"]\npclock_max_hz = 300_000_000\nmax_width = 4096\nmax_height = 2160",
    );
    assert!(Matrix::parse(&ok).is_ok());
    assert_eq!(
        entry_rule(&ok.replace("[\"HDMI-A\"]", "[\"VGA\"]")),
        "a digital limit names no connector, or an analog connector"
    );
    assert_eq!(
        entry_rule(&ok.replace("[\"HDMI-A\"]", "[]")),
        "a digital limit names no connector, or an analog connector"
    );
    assert_eq!(
        entry_rule(&ok.replace("pclock_max_hz = 300_000_000", "pclock_max_hz = 1_000")),
        "a digital limit has a pixel clock below pclock_min_hz, or a size of 0"
    );
    assert_eq!(
        entry_rule(&ok.replace("max_width = 4096", "max_width = 0")),
        "a digital limit has a pixel clock below pclock_min_hz, or a size of 0"
    );
    assert!(matches!(
        Matrix::parse(&ok.replace("max_height = 2160", "max_height = 2160\ncolour = 1")),
        Err(MatrixError::Parse { .. })
    ));
}
