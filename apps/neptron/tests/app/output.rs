//! A profile confirmed on one output does not drive another output with
//! no prompt.

use neptron_video::{Phase, SafeReason};

use crate::rig::Rig;

#[test]
#[ignore = "tier 2"]
fn a_new_output_asks_again_and_reverts_to_the_safe_default() {
    let dir = std::env::temp_dir().join(format!("neptron-app-output-{}", std::process::id()));
    // A leftover of an earlier run with the same process id; absence is fine.
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("the folder");
    let mut rig = Rig::new(&dir);
    rig.boot(None);
    rig.frames(2);
    rig.keep();
    assert_eq!(rig.shell().guard().phase(), Phase::Confirmed);
    rig.finish();
    assert!(
        rig.state_file("video/guard.toml")
            .contains("output = \"fake-1\"")
    );

    rig.boot(None);
    rig.frames(2);
    assert_eq!(
        rig.shell().guard().phase(),
        Phase::Confirmed,
        "the same output"
    );
    rig.finish();

    rig.output = "fake-2".to_owned();
    rig.boot(None);
    rig.frames(2);
    assert!(
        matches!(rig.shell().guard().phase(), Phase::Pending(_)),
        "a new output opens the boot prompt"
    );
    rig.frames(660);
    assert_eq!(
        rig.shell().guard().phase(),
        Phase::Safe(SafeReason::NothingConfirmed)
    );
    rig.finish();
}

/// The EDID of an analog CRT, made from the specification.
const CRT_ANALOG: &[u8] = include_bytes!("../fixtures/edid/crt_analog.bin");

/// `edid` with another serial number (bytes 0x0C to 0x0F) and its checksum
/// fixed: the same model, another monitor.
fn other_serial(edid: &[u8]) -> Vec<u8> {
    let mut other = edid.to_vec();
    other[0x0C] ^= 0x5A;
    let sum = other[..127].iter().fold(0_u8, |s, b| s.wrapping_add(*b));
    other[127] = 0_u8.wrapping_sub(sum);
    other
}

/// A sysfs tree under `root` with one GPU and a VGA connector that reports
/// `edid`.
fn sysfs_with_vga(root: &std::path::Path, edid: &[u8]) {
    let drm = root.join("class/drm");
    let card = drm.join("card0/device");
    std::fs::create_dir_all(&card).expect("the card");
    std::fs::write(card.join("uevent"), "DRIVER=fake\n").expect("the uevent");
    let connector = drm.join("card0-VGA-1");
    std::fs::create_dir_all(&connector).expect("the connector");
    std::fs::write(connector.join("status"), "connected\n").expect("the status");
    std::fs::write(connector.join("edid"), edid).expect("the EDID");
}

#[test]
#[ignore = "tier 2"]
fn another_monitor_on_the_same_output_asks_again_with_a_preset() {
    let dir = std::env::temp_dir().join(format!("neptron-app-swap-{}", std::process::id()));
    // A leftover of an earlier run with the same process id; absence is fine.
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("the folder");
    let mut rig = Rig::new(&dir);
    rig.output = "VGA-1".to_owned();
    sysfs_with_vga(&rig.paths.sysfs, CRT_ANALOG);
    let config = rig.paths.config();
    std::fs::create_dir_all(&config).expect("the config folder");
    std::fs::write(
        config.join("global.toml"),
        "schema_version = 1\n\n[display]\nmonitor = \"generic_15\"\n",
    )
    .expect("the settings");
    rig.boot(None);
    rig.frames(2);
    rig.keep();
    assert_eq!(rig.shell().guard().phase(), Phase::Confirmed);
    rig.finish();
    let record = rig.state_file("video/guard.toml");
    assert!(record.contains("monitor = \"edid-"), "{record}");

    rig.boot(None);
    rig.frames(2);
    assert_eq!(
        rig.shell().guard().phase(),
        Phase::Confirmed,
        "the same monitor"
    );
    rig.finish();

    sysfs_with_vga(&rig.paths.sysfs, &other_serial(CRT_ANALOG));
    rig.boot(None);
    rig.frames(2);
    assert!(
        matches!(rig.shell().guard().phase(), Phase::Pending(_)),
        "another monitor opens the boot prompt: {:?}",
        rig.shell().guard().phase()
    );
    rig.finish();
}
