//! At most one modeset in a second for the game's own changes
//! (`MODESET_MIN_INTERVAL_NS`, docs/spec/interlaced-systems.md section 4).

use neptron_types::{Orientation, Rotation, SourceGeometry};
use neptron_video::{Content, Directive, MODESET_MIN_INTERVAL_NS, Outcome, Trigger};

use crate::harness::{
    MD_H32, MD_H40, Rig, SNES, SNES_239, fixed_width, hold_frames, native_width, source,
};

/// The frames of the geometry modesets of the trace.
fn geometry_modesets(rig: &Rig) -> Vec<u64> {
    rig.engine
        .trace()
        .records()
        .filter(|r| r.outcome == Outcome::Modeset && r.trigger == Trigger::Geometry)
        .map(|r| r.decided_frame)
        .collect()
}

fn nanos(frames: u64, refresh_mhz: u32) -> u64 {
    frames * 1_000_000_000_000 / u64::from(refresh_mhz)
}

/// Launch on H40, then the first change to H32: no hold, as no change of
/// the game came before it.
fn after_one_change() -> Rig {
    let mut rig = Rig::generic_15(native_width());
    rig.frame(MD_H40);
    for _ in 0..3 {
        rig.frame(MD_H32);
    }
    assert_eq!(geometry_modesets(&rig), [4]);
    rig
}

#[test]
fn the_first_change_after_the_launch_is_not_held() {
    let rig = after_one_change();
    assert_eq!(rig.modes().len(), 2);
}

#[test]
fn a_change_within_a_second_waits_and_comes_at_the_second() {
    let mut rig = after_one_change();
    let mut modeset_at = None;
    for _ in 0..2 * hold_frames(&MD_H32) {
        let step = rig.frame(MD_H40);
        if step.directive == Directive::Modeset {
            modeset_at = Some(rig.engine.odren().frame());
            break;
        }
        assert!(matches!(step.directive, Directive::Waiting { .. }));
    }
    let at = modeset_at.expect("the change comes after the hold");
    let since = at - 4;
    assert!(nanos(since, MD_H32.refresh_mhz) >= MODESET_MIN_INTERVAL_NS);
    assert!(
        nanos(since - 1, MD_H32.refresh_mhz) < MODESET_MIN_INTERVAL_NS,
        "the first frame after the second"
    );
    assert_eq!(rig.modes().len(), 3);
}

#[test]
fn a_game_that_goes_back_within_the_second_keeps_its_mode() {
    let mut rig = after_one_change();
    for _ in 0..20 {
        let step = rig.frame(MD_H40);
        assert!(matches!(step.directive, Directive::Waiting { .. }));
    }
    for _ in 0..2 * hold_frames(&MD_H32) {
        assert_eq!(rig.frame(MD_H32).directive, Directive::Steady);
    }
    assert_eq!(rig.modes().len(), 2);
}

#[test]
fn every_modeset_of_a_restless_game_is_a_second_apart() {
    let mut rig = after_one_change();
    for stretch in 0..60 {
        let source = if stretch % 2 == 0 { MD_H40 } else { MD_H32 };
        for _ in 0..10 {
            rig.frame(source);
        }
    }
    let frames = geometry_modesets(&rig);
    assert!(frames.len() >= 3, "{frames:?}");
    for pair in frames.windows(2) {
        let gap = nanos(pair[1] - pair[0], MD_H32.refresh_mhz);
        assert!(gap >= MODESET_MIN_INTERVAL_NS, "{frames:?}");
    }
}

#[test]
fn a_change_of_the_settings_is_not_held() {
    let mut rig = after_one_change();
    rig.engine.set_settings(fixed_width());
    let step = rig.frame(MD_H32);
    assert_eq!(step.directive, Directive::Modeset);
    assert_eq!(
        rig.engine.trace().last().unwrap().trigger,
        Trigger::Settings
    );
}

#[test]
fn a_scale_only_change_is_never_held() {
    let mut rig = Rig::generic_15(fixed_width());
    rig.frame(SNES);
    for _ in 0..neptron_video::DEBOUNCE_FRAMES {
        rig.frame(SNES_239);
    }
    assert_eq!(geometry_modesets(&rig).len(), 1, "the hold starts");
    // The same lines and rate at another width share the mode.
    let hires_239 = source(512, 239, SNES_239.refresh_mhz);
    assert_eq!(rig.frame(hires_239).directive, Directive::Rescale);
    assert_eq!(rig.modes().len(), 2);
}

/// The first frames after an input that the player changed, until one is a
/// modeset or the debounce is over.
fn modeset_within_the_debounce(rig: &mut Rig, source: SourceGeometry) -> bool {
    (0..neptron_video::DEBOUNCE_FRAMES).any(|_| rig.frame(source).directive == Directive::Modeset)
}

#[test]
fn a_change_of_the_orientation_ends_the_hold() {
    let mut rig = after_one_change();
    rig.engine.set_orientation(Orientation {
        screen: Rotation::R90,
        game_override: None,
    });
    assert!(
        modeset_within_the_debounce(&mut rig, MD_H32),
        "the new turn comes after the debounce alone"
    );
}

#[test]
fn a_change_of_the_content_ends_the_hold() {
    let mut rig = after_one_change();
    rig.engine.set_content(Content::Menu);
    assert!(
        modeset_within_the_debounce(&mut rig, MD_H32),
        "the menu comes after the debounce alone"
    );
}

/// The control of the two tests above: a new source of the game itself
/// within the second waits.
#[test]
fn a_change_of_the_game_within_the_second_waits_past_the_debounce() {
    let mut rig = after_one_change();
    assert!(!modeset_within_the_debounce(&mut rig, MD_H40));
}
