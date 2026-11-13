//! The start of a run, before the first frame.

use neptron_video::Phase;

use crate::rig::Rig;

#[test]
fn a_guard_record_that_cannot_be_written_starts_the_safe_default() {
    let dir = std::env::temp_dir().join(format!("neptron-app-record-{}", std::process::id()));
    // A leftover of an earlier run with the same process id; absence is fine.
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("state")).expect("the folder");
    let mut rig = Rig::new(&dir);
    std::fs::write(
        rig.paths.state.join("video"),
        b"a file where the folder must be",
    )
    .expect("block the record folder");
    rig.boot(None);
    assert!(
        matches!(rig.shell().guard().phase(), Phase::Safe(_)),
        "{:?}",
        rig.shell().guard().phase()
    );
    let notices: Vec<String> = rig
        .shell()
        .snapshot()
        .notices
        .iter()
        .map(|n| n.text.clone())
        .collect();
    assert!(
        notices.iter().any(|n| n.contains("guard record")),
        "{notices:?}"
    );
    rig.frames(2);
    assert!(matches!(rig.shell().guard().phase(), Phase::Safe(_)));
    let shell = rig.shell.take().expect("a run");
    // The later writes of the record fail too; they are reported, and the
    // run ends all the same.
    let _late_failures = shell.finish();
}

#[test]
fn a_stop_signal_ends_the_run_with_a_quit() {
    let dir = std::env::temp_dir().join(format!("neptron-app-stop-{}", std::process::id()));
    // A leftover of an earlier run with the same process id; absence is fine.
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("the folder");
    let mut rig = Rig::new(&dir);
    rig.boot(None);
    rig.frames(2);
    rig.stop.store(true, std::sync::atomic::Ordering::Relaxed);
    assert_eq!(rig.until_end(1), neptron::Outcome::Quit);
}

#[test]
fn every_mode_on_the_wire_is_in_the_log() {
    let dir = std::env::temp_dir().join(format!("neptron-app-modelog-{}", std::process::id()));
    // A leftover of an earlier run with the same process id; absence is fine.
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("the folder");
    let mut rig = Rig::new(&dir);
    rig.boot(None);
    rig.frames(2);
    let lines = rig.shell().log_lines(100);
    assert!(
        lines
            .iter()
            .any(|l| l.starts_with("mode set: ") && l.contains("; read back Modeline")),
        "{lines:?}"
    );
}

#[test]
fn a_missing_device_is_a_notice_and_the_run_goes_on() {
    let dir = std::env::temp_dir().join(format!("neptron-app-devices-{}", std::process::id()));
    // A leftover of an earlier run with the same process id; absence is fine.
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("the folder");
    let mut rig = Rig::new(&dir);
    rig.notices = vec!["No sound: the audio device did not open.".to_owned()];
    rig.boot(None);
    let shown = rig
        .shell()
        .snapshot()
        .notices
        .iter()
        .any(|n| n.text.starts_with("No sound"));
    assert!(shown);
    assert!(
        rig.state_file("video/guard.toml")
            .contains("failed_boots = 1")
    );
    rig.finish();
}

#[test]
fn a_state_partition_in_ram_is_a_notice_and_the_safe_default() {
    let dir = std::env::temp_dir().join(format!("neptron-app-storage-{}", std::process::id()));
    // A leftover of an earlier run with the same process id; absence is fine.
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("the folder");
    let mut rig = Rig::new(&dir);
    std::fs::create_dir_all(&rig.paths.run).expect("the run folder");
    std::fs::write(
        rig.paths.run.join("storage.toml"),
        "state = \"ram\"\nstate_reason = \"/dev/sda3 does not mount\"\nshare = \"partition\"\n",
    )
    .expect("the status");
    rig.boot(None);
    assert!(
        matches!(rig.shell().guard().phase(), Phase::Safe(_)),
        "an unrecorded boot: {:?}",
        rig.shell().guard().phase()
    );
    let notices: Vec<String> = rig
        .shell()
        .snapshot()
        .notices
        .iter()
        .map(|n| n.text.clone())
        .collect();
    assert!(
        notices
            .iter()
            .any(|n| n.contains("state partition is not in use (/dev/sda3 does not mount)")),
        "{notices:?}"
    );
    let lines = rig.shell().log_lines(100);
    assert!(
        lines.iter().any(|l| l.contains("/dev/sda3 does not mount")),
        "{lines:?}"
    );
    rig.frames(2);
    rig.finish();
}
