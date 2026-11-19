//! The pacing of the loop: the input delay of a fast core, and a core that
//! needs more than one flip interval.

use neptron_types::Control;

use crate::harness::{Rig, Setup};
use crate::with_script;

const SCRIPT: &str = "\
neptron-testcore 1
pixel_format rgb565
geometry 128 112 1333
timing 60000 48000
";

/// The core of a fast game runs at the flip event and its frame flips at
/// the next blank: the delay from the poll to the flip is one interval.
#[test]
fn the_input_delay_of_a_fast_core_is_one_flip_interval() {
    with_script!(
        "the_input_delay_of_a_fast_core_is_one_flip_interval",
        SCRIPT,
        |dir| {
            let mut rig = Rig::launch(dir, Setup::default());
            rig.run(30);
            assert_eq!(rig.session.counters().input_to_flip_ns, 0, "no input yet");
            for change in [Rig::press, Rig::release] {
                change(&rig, &[Control::A]);
                rig.run(1);
                let flips = rig.display.flip_times();
                let interval = flips[flips.len() - 1] - flips[flips.len() - 2];
                assert_eq!(rig.session.counters().input_to_flip_ns, interval);
                rig.run(10);
                assert_eq!(
                    rig.session.counters().input_to_flip_ns,
                    interval,
                    "frames with no input keep the last delay"
                );
            }
        }
    );
}
