//! Detection on the fixture sysfs trees.

use std::path::{Path, PathBuf};

use neptron_platform::{ConnectorStatus, ParseError, PciId, SysfsError, detect};

use crate::blobs;

fn tree(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sysfs")
        .join(name)
}

#[test]
fn intel_hd_530_with_a_crt_on_vga() {
    let platform = detect(&tree("i3_6100t_vga")).unwrap();
    assert_eq!(platform.gpus.len(), 1);
    let gpu = &platform.gpus[0];
    assert_eq!(gpu.card, "card0");
    assert_eq!(gpu.driver.as_deref(), Some("i915"));
    assert_eq!(
        gpu.pci_id,
        Some(PciId {
            vendor: 0x8086,
            device: 0x1912
        })
    );
    let names: Vec<_> = platform
        .connectors
        .iter()
        .map(|c| c.name.as_str())
        .collect();
    assert_eq!(names, ["DP-1", "HDMI-A-1", "VGA-1"]);
    let vga = &platform.connectors[2];
    assert_eq!(vga.kind, "VGA");
    assert_eq!(vga.status, ConnectorStatus::Connected);
    assert_eq!(vga.edid.as_deref(), Some(blobs::pc_crt_30_71().as_slice()));
    assert_eq!(platform.gpu_of(vga), Some(gpu));
    // An empty edid file and a missing one both mean no EDID.
    assert_eq!(platform.connectors[0].edid, None);
    assert_eq!(platform.connectors[1].edid, None);
    assert_eq!(platform.connectors[0].status, ConnectorStatus::Disconnected);
}

#[test]
fn raspberry_pi_3_with_two_cards() {
    let platform = detect(&tree("pi3_composite")).unwrap();
    let cards: Vec<_> = platform.gpus.iter().map(|g| g.card.as_str()).collect();
    assert_eq!(cards, ["card0", "card1"]);
    let vc4 = &platform.gpus[1];
    assert_eq!(vc4.driver.as_deref(), Some("vc4-drm"));
    assert_eq!(vc4.of_compatible, ["brcm,bcm2835-vc4"]);
    assert_eq!(vc4.pci_id, None);
    let composite = &platform.connectors[0];
    assert_eq!(composite.kind, "Composite");
    assert_eq!(composite.status, ConnectorStatus::Unknown);
    assert_eq!(platform.gpu_of(composite), Some(vc4));
}

#[test]
fn cards_sort_by_number_and_may_have_no_uevent() {
    let platform = detect(&tree("qemu_bochs")).unwrap();
    let cards: Vec<_> = platform.gpus.iter().map(|g| g.card.as_str()).collect();
    assert_eq!(cards, ["card0", "card2", "card10"]);
    assert_eq!(platform.gpus[1].driver, None);
    assert_eq!(platform.gpus[1].pci_id, None);
    assert_eq!(platform.connectors[0].kind, "Virtual");
}

#[test]
fn no_drm_class_means_no_gpu() {
    let platform = detect(&tree("no_drm")).unwrap();
    assert!(platform.gpus.is_empty());
    assert!(platform.connectors.is_empty());
}

#[test]
fn unreadable_drm_class_is_an_error_with_the_path() {
    let root = tree("drm_is_file");
    let Err(SysfsError::Read { path, .. }) = detect(&root) else {
        panic!("expected a read error");
    };
    assert_eq!(path, root.join("class/drm"));
}

#[test]
fn bad_uevent_is_an_error_with_the_path() {
    let root = tree("bad_uevent");
    let Err(SysfsError::Parse { path, source }) = detect(&root) else {
        panic!("expected a parse error");
    };
    assert_eq!(path, root.join("class/drm/card0/device/uevent"));
    assert_eq!(
        source,
        ParseError::PciId {
            value: "zzzz:1912".into()
        }
    );
}

#[test]
fn bad_status_is_an_error_with_the_path() {
    let root = tree("bad_status");
    let Err(SysfsError::Parse { path, .. }) = detect(&root) else {
        panic!("expected a parse error");
    };
    assert_eq!(path, root.join("class/drm/card0-VGA-1/status"));
}

#[test]
fn missing_status_is_an_error_with_the_path() {
    let root = tree("missing_status");
    let Err(SysfsError::Missing { path }) = detect(&root) else {
        panic!("expected a missing file error");
    };
    assert_eq!(path, root.join("class/drm/card0-VGA-1/status"));
}
