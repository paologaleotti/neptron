//! The first mode at launch. Genesis Plus GX reports 256x192 at load and
//! sends 320x224 from its fifth frame (frame 4, `tests/traces`). The
//! session lets the source settle before it sets the first mode, so the
//! first mode is the game's mode and the monitor resyncs once.

use neptron_session::{LAUNCH_STABLE_FRAMES, source_geometry};
use neptron_testkit::preset::{CAPS_LOW_CLOCK, monitor_preset};
use neptron_testkit::{FakeDisplay, SimClock};
use neptron_types::{Frame, PixelFormat};
use neptron_video::{Monitor, Outcome, PolicySettings, Trigger, VideoEngine};

use crate::harness::{Rig, Setup};
use crate::with_script;

/// The video of the Genesis Plus GX trace, with its rate and aspect.
const GENESIS_SCRIPT: &str = "\
neptron-testcore 1
pixel_format rgb565
geometry 256 192 1306
max_size 348 240
timing 59923 44100
at 4 geometry 320 224 1306
";

#[test]
fn the_first_mode_is_the_mode_of_the_settled_source() {
    with_script!(
        "the_first_mode_is_the_mode_of_the_settled_source",
        GENESIS_SCRIPT,
        |dir| {
            let mut rig = Rig::launch(dir, Setup::default());
            assert_eq!(
                rig.session.counters().settle_frames,
                4 + u64::from(LAUNCH_STABLE_FRAMES)
            );
            assert!(
                rig.display.mode_calls().is_empty(),
                "no mode before the loop"
            );
            rig.run(120);
            assert_eq!(rig.display.mode_calls().len(), 1, "one modeset");
            let trace: Vec<_> = rig.session.video().trace().records().cloned().collect();
            assert_eq!(trace.len(), 1);
            assert_eq!(trace[0].trigger, Trigger::Launch);
            assert_eq!(trace[0].outcome, Outcome::Modeset);
            assert_eq!((trace[0].source.width, trace[0].source.height), (320, 224));
            assert_eq!(rig.session.counters().flips, 120);
        }
    );
}

/// Why the session settles: the engine sets the first mode on its first
/// frame with no debounce, so the load geometry of Genesis Plus GX would
/// give a second modeset four frames later.
#[test]
fn without_the_settle_the_engine_sets_two_modes() {
    static PIXELS: [u8; 348 * 240 * 2] = [0; 348 * 240 * 2];
    let geometry = neptron_libretro::GameGeometry {
        base_width: 256,
        base_height: 192,
        max_width: 348,
        max_height: 240,
        aspect_milli: 1_306,
    };
    let timing = neptron_libretro::Timing {
        fps_mhz: 59_923,
        sample_rate_mhz: 44_100_000,
    };
    let clock = SimClock::new();
    let display = FakeDisplay::new(clock, CAPS_LOW_CLOCK);
    let monitor = Monitor::Profile(monitor_preset("generic_15").unwrap());
    let mut engine = VideoEngine::new(display.clone(), monitor, PolicySettings::default());
    for n in 0..12 {
        let (width, height) = if n < 4 { (256, 192) } else { (320, 224) };
        let frame = Frame {
            data: &PIXELS,
            width,
            height,
            pitch: usize::from(width) * 2,
            format: PixelFormat::Rgb565,
        };
        engine.begin_frame(source_geometry(width, height, &geometry, &timing));
        engine.present(Some(frame), None).unwrap();
        engine.wait_flip().unwrap();
    }
    assert_eq!(display.mode_calls().len(), 2);
}
