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
