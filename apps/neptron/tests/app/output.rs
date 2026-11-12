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
