//! A game of a system with widths on a CRT: one wide mode for every width
//! of the PlayStation and the SNES, and a modeset at each change between
//! 240p and 480i (docs/spec/interlaced-systems.md sections 2 and 3).

use neptron_testkit::preset::monitor_preset;
use neptron_types::SourceGeometry;
use neptron_video::{Directive, Monitor, PolicySettings, wide_strategy};

use crate::harness::{Rig, hold_frames, source};

/// The widths of a PlayStation game, the most used first. Its system file
/// comes with its core (task S10); until then the list lives here.
const WIDTHS: [u16; 5] = [320, 256, 640, 512, 368];
const PSX_480I: SourceGeometry = SourceGeometry {
    interlaced: true,
    ..source(640, 480, 59_940)
};

fn playstation(monitor: &str) -> Rig {
    let settings = PolicySettings {
        horizontal: wide_strategy(&WIDTHS),
        ..PolicySettings::default()
    };
    Rig::new(Monitor::Profile(monitor_preset(monitor).unwrap()), settings)
}

#[test]
fn a_width_change_of_a_playstation_game_is_no_modeset_on_a_crt() {
    for monitor in ["generic_15", "pc_31_120"] {
        let mut rig = playstation(monitor);
        rig.frame(source(320, 240, 59_826));
        for round in 0..3 {
            for &width in &WIDTHS {
                let step = rig.frame(source(width, 240, 59_826));
                assert!(
                    matches!(step.directive, Directive::Steady | Directive::Rescale),
                    "{monitor} round {round} width {width}: {:?}",
                    step.directive
                );
                assert!(!step.muted_before_present, "{monitor}: no mute");
            }
        }
        assert_eq!(rig.modes().len(), 1, "{monitor}: one mode for every width");
    }
}

#[test]
fn a_playstation_game_gets_a_modeset_at_each_change_between_240p_and_480i() {
    let mut rig = playstation("generic_15");
    let progressive = source(320, 240, 59_826);
    rig.frame(progressive);
    let mut switches = vec![rig.current_mode()];
    for next in [PSX_480I, progressive, PSX_480I] {
        for _ in 0..hold_frames(&next) {
            rig.frame(next);
        }
        let mode = rig.current_mode();
        assert_eq!(mode.interlace, next.interlaced, "{next:?}");
        switches.push(mode);
    }
    assert_eq!(rig.modes().len(), 4);
    assert_eq!(switches[0], switches[2], "the same 240p mode comes back");
    assert_eq!(switches[1], switches[3], "the same 480i mode comes back");
}

#[test]
fn a_width_change_of_a_snes_game_is_no_modeset_on_a_crt() {
    let settings = PolicySettings {
        horizontal: wide_strategy(&[256, 512]),
        ..PolicySettings::default()
    };
    let monitor = Monitor::Profile(monitor_preset("generic_15").unwrap());
    let mut rig = Rig::new(monitor, settings);
    rig.frame(source(256, 224, 60_099));
    for _ in 0..10 {
        for width in [512, 256] {
            let step = rig.frame(source(width, 224, 60_099));
            assert_eq!(step.directive, Directive::Rescale, "{width}");
        }
    }
    assert_eq!(rig.modes().len(), 1);
}
