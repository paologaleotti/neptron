//! The good path: launch, debounce, scale only, mute, trace.

use neptron_types::{Filter, Rect, Rotation, SourceGeometry};
use neptron_video::{
    DEBOUNCE_FRAMES, Directive, Outcome, PictureStyle, PolicySettings, Trigger, Unshowable,
    VideoError,
};

use crate::harness::{
    MD_H32, MD_H40, Rig, SNES, SNES_239, fixed_width, frame, generic_15, mode_for, native_width,
};

#[test]
fn the_first_frame_sets_a_mode_with_no_debounce() {
    let mut rig = Rig::generic_15(native_width());
    let step = rig.frame(MD_H40);
    assert_eq!(step.directive, Directive::Modeset);
    assert_eq!(step.mode_calls, 1);
    assert_eq!(rig.modes().len(), 1);
    let record = rig.engine.trace().last().unwrap();
    assert_eq!(record.trigger, Trigger::Launch);
    assert_eq!(record.outcome, Outcome::Modeset);
    assert_eq!(record.asked_frame, 1);
    assert_eq!(record.shown_frame, Some(1));
    assert_eq!(record.took_frames(), Some(0));
    assert_eq!(record.gap_ns, None, "no picture before launch");
    assert_eq!(record.previous, None);
}

#[test]
fn the_engine_sets_the_token_of_the_winner() {
    let mut rig = Rig::generic_15(native_width());
    rig.frame(MD_H40);
    let current = rig.engine.current().unwrap();
    assert_eq!(
        rig.fake.mode_calls()[0].requested,
        *current.chosen.token.mode()
    );
    assert_eq!(current.chosen.token.mode(), &current.chosen.proposal.mode);
    assert_eq!(current.read_back, *current.chosen.token.mode());
    assert_eq!(current.source, MD_H40);
}

#[test]
fn a_steady_source_makes_no_display_call_and_no_record() {
    let mut rig = Rig::generic_15(native_width());
    rig.frame(SNES);
    let records = rig.engine.trace().total();
    for _ in 0..1_000 {
        assert_eq!(rig.frame(SNES).directive, Directive::Steady);
    }
    assert_eq!(rig.fake.mode_calls().len(), 1);
    assert_eq!(rig.engine.trace().total(), records);
    assert!(rig.fake.dropped_frames().is_empty());
}

#[test]
fn a_flicker_between_two_sizes_causes_no_switch() {
    assert_ne!(
        mode_for(generic_15(), native_width(), MD_H32),
        mode_for(generic_15(), native_width(), MD_H40),
        "the two sizes need two modes, so only the debounce holds the switch"
    );
    let mut rig = Rig::generic_15(native_width());
    rig.frame(MD_H40);
    for _ in 0..50 {
        assert_eq!(
            rig.frame(MD_H32).directive,
            Directive::Waiting { stable_frames: 1 }
        );
        assert_eq!(rig.frame(MD_H40).directive, Directive::Steady);
    }
    assert_eq!(rig.modes().len(), 1);
    assert_eq!(rig.engine.trace().total(), 1);
}

#[test]
fn a_change_that_holds_for_the_debounce_causes_exactly_one_switch() {
    let mut rig = Rig::generic_15(native_width());
    rig.frame(MD_H40);
    let old = rig.current_mode();
    let mut directives = Vec::new();
    for _ in 0..10 {
        directives.push(rig.frame(MD_H32).directive);
    }
    assert_eq!(DEBOUNCE_FRAMES, 3);
    assert_eq!(
        directives[..4],
        [
            Directive::Waiting { stable_frames: 1 },
            Directive::Waiting { stable_frames: 2 },
            Directive::Modeset,
            Directive::Steady,
        ]
    );
    assert!(directives[4..].iter().all(|d| *d == Directive::Steady));
    assert_eq!(rig.modes().len(), 2);
    let record = rig.engine.trace().last().unwrap();
    assert_eq!(record.trigger, Trigger::Geometry);
    assert_eq!(record.asked_frame, 2);
    assert_eq!(record.decided_frame, 4);
    assert_eq!(record.shown_frame, Some(4));
    assert_eq!(record.took_frames(), Some(2));
    assert_eq!(record.previous, Some(old));
    assert!(record.gap_ns.is_some());
}

#[test]
fn a_change_that_breaks_before_the_debounce_starts_again() {
    let mut rig = Rig::generic_15(native_width());
    rig.frame(MD_H40);
    rig.frame(MD_H32);
    rig.frame(MD_H32);
    assert_eq!(rig.frame(MD_H40).directive, Directive::Steady);
    assert_eq!(
        rig.frame(MD_H32).directive,
        Directive::Waiting { stable_frames: 1 }
    );
    assert_eq!(rig.modes().len(), 1);
}

#[test]
fn the_frames_during_the_debounce_show_whole_in_the_old_mode() {
    let mut rig = Rig::generic_15(native_width());
    rig.frame(MD_H40);
    let dest = rig.engine.current().unwrap().chosen.proposal.placement.dest;
    rig.frame(MD_H32);
    let scene = rig.fake.frames().last().unwrap().scene;
    assert_eq!(
        scene.crop,
        Rect {
            x: 0,
            y: 0,
            w: 256,
            h: 224
        }
    );
    assert_eq!(scene.dest, dest);
}

#[test]
fn an_equal_mode_changes_only_scale_and_crop() {
    assert_eq!(
        mode_for(generic_15(), fixed_width(), MD_H32),
        mode_for(generic_15(), fixed_width(), MD_H40),
    );
    let mut rig = Rig::generic_15(fixed_width());
    rig.frame(MD_H40);
    let before = rig.fake.frames().last().unwrap().scene;
    let step = rig.frame(MD_H32);
    assert_eq!(step.directive, Directive::Rescale);
    assert_eq!(step.mode_calls, 0);
    assert!(!step.muted_before_present, "no modeset, no mute");
    let after = rig.fake.frames().last().unwrap().scene;
    assert_eq!(after.crop.w, 256);
    assert_eq!(before.crop.w, 320);
    assert_eq!(after.ui_dest, before.ui_dest);
    assert_eq!(rig.modes().len(), 1);
    let record = rig.engine.trace().last().unwrap();
    assert_eq!(record.outcome, Outcome::ScaleOnly);
    assert_eq!(record.decided_frame, 2, "a scale change has no debounce");
    assert!(record.dropped.is_empty() && record.refused.is_empty());
    assert_eq!(rig.engine.current().unwrap().source, MD_H32);
}

#[test]
fn a_flicker_under_a_fixed_width_rescales_each_frame_with_no_modeset() {
    let mut rig = Rig::generic_15(fixed_width());
    rig.frame(MD_H40);
    for _ in 0..20 {
        assert_eq!(rig.frame(MD_H32).directive, Directive::Rescale);
        assert_eq!(rig.frame(MD_H40).directive, Directive::Rescale);
    }
    assert_eq!(rig.modes().len(), 1);
    assert!(rig.fake.frames().iter().all(|f| f.present.is_ok()));
}

#[test]
fn mute_comes_before_the_modeset_and_unmute_after_the_first_flip() {
    let mut rig = Rig::generic_15(native_width());
    let launch = rig.frame(MD_H40);
    assert!(launch.muted_before_present);
    assert!(!launch.muted_after_flip);
    rig.frame(MD_H32);
    rig.frame(MD_H32);
    let calls = rig.fake.mode_calls().len();
    assert_eq!(rig.engine.begin_frame(MD_H32), Directive::Modeset);
    assert!(rig.engine.muted());
    assert_eq!(rig.fake.mode_calls().len(), calls, "muted before set_mode");
    rig.engine.present(Some(frame(&MD_H32)), None).unwrap();
    assert_eq!(rig.fake.mode_calls().len(), calls + 1);
    assert!(rig.engine.muted(), "still muted until the new mode flips");
    let switch_ns = rig.fake.mode_switches().last().unwrap().time_ns;
    let flip = rig.engine.wait_flip().unwrap();
    assert!(flip.time_ns > switch_ns);
    assert!(!rig.engine.muted());
}

#[test]
fn double_refresh_presents_twice_per_core_frame_and_counts_core_frames() {
    let mut rig = Rig::generic_15(native_width());
    rig.engine.begin_frame(MD_H40);
    for _ in 0..2 {
        rig.engine.present(Some(frame(&MD_H40)), None).unwrap();
        rig.engine.wait_flip().unwrap();
    }
    assert_eq!(rig.engine.odren().frame(), 1);
    assert_eq!(rig.fake.mode_calls().len(), 1);
    assert_eq!(rig.fake.flips().len(), 2);
}

#[test]
fn present_drains_a_queued_flip_before_a_modeset() {
    let mut rig = Rig::generic_15(native_width());
    rig.frame(MD_H40);
    rig.frame(MD_H32);
    rig.frame(MD_H32);
    // The caller presents and does not wait: the next modeset must wait
    // for that flip, or the display refuses it as busy.
    rig.engine.begin_frame(MD_H32);
    rig.engine.present(Some(frame(&MD_H32)), None).unwrap();
    rig.engine.wait_flip().unwrap();
    rig.engine.begin_frame(MD_H40);
    rig.engine.present(Some(frame(&MD_H40)), None).unwrap();
    rig.engine.begin_frame(MD_H40);
    rig.engine.begin_frame(MD_H40);
    let presented = rig.engine.present(Some(frame(&MD_H40)), None).unwrap();
    assert!(presented.drained_flip.is_some());
    assert_eq!(presented.modeset, Some(Outcome::Modeset));
    rig.engine.wait_flip().unwrap();
    assert!(rig.fake.mode_calls().iter().all(|c| c.result.is_ok()));
}

#[test]
fn the_trace_holds_the_reasons_of_a_modeset() {
    let mut rig = Rig::generic_15(native_width());
    rig.frame(SNES);
    rig.frame(SNES_239);
    rig.frame(SNES_239);
    rig.frame(SNES_239);
    let record = rig.engine.trace().last().unwrap();
    assert_eq!(record.source, SNES_239);
    let winner = record.winner.as_ref().unwrap();
    assert_eq!(record.read_back, Some(*winner.token.mode()));
    assert!(record.dropped.iter().all(|(i, _, _)| *i < winner.index));
    assert!(record.refused.iter().all(|r| r.index < winner.index));
    let line = record.to_string();
    assert!(
        line.contains("geometry 256x239p at 60099 mHz: modeset"),
        "{line}"
    );
    assert!(line.contains("shown after 2 frames"), "{line}");
}

#[test]
fn a_turned_game_is_not_shown() {
    let turned = SourceGeometry {
        rotation: Rotation::R90,
        ..SNES
    };
    let mut rig = Rig::generic_15(native_width());
    assert_eq!(rig.engine.begin_frame(turned), Directive::Kept);
    assert_eq!(
        rig.engine.present(Some(frame(&turned)), None),
        Err(VideoError::NoMode)
    );
    assert!(rig.fake.mode_calls().is_empty());
    assert_eq!(
        rig.engine.trace().last().unwrap().outcome,
        Outcome::Kept(Unshowable::Rotated(Rotation::R90))
    );
    rig.frame(SNES);
    let mode = rig.current_mode();
    for rotation in [Rotation::R90, Rotation::R180, Rotation::R270] {
        let turned = SourceGeometry { rotation, ..SNES };
        for _ in 0..5 {
            assert_eq!(rig.frame(turned).directive, Directive::Kept);
        }
    }
    assert_eq!(rig.current_mode(), mode);
    assert_eq!(rig.modes().len(), 1);
}

#[test]
fn the_style_rows_come_from_the_same_decision() {
    let mut rig = Rig::generic_15(native_width());
    rig.frame(SNES);
    let rows = rig.engine.style_rows(&SNES).unwrap();
    assert_eq!(rows.len(), PictureStyle::ALL.len());
    let auto = &rows[0];
    assert_eq!(auto.style, PictureStyle::Auto);
    let current = rig.engine.current().unwrap();
    assert_eq!(auto.winner, Some(current.chosen.index));
    let turned = SourceGeometry {
        rotation: Rotation::R270,
        ..SNES
    };
    assert_eq!(
        rig.engine.style_rows(&turned).unwrap_err(),
        Unshowable::Rotated(Rotation::R270)
    );
}

#[test]
fn the_ui_alone_needs_a_mode() {
    let mut rig = Rig::generic_15(native_width());
    rig.frame(SNES);
    rig.engine.begin_frame(SNES);
    rig.engine.present(None, Some(frame(&SNES))).unwrap();
    rig.engine.wait_flip().unwrap();
    let scene = rig.fake.frames().last().unwrap().scene;
    assert_eq!(scene.game, None);
    assert_eq!(scene.ui, Some((256, 224)));
    let placement = rig.engine.current().unwrap().chosen.proposal.placement;
    assert_eq!(scene.ui_dest, placement.ui_dest);
}

#[test]
fn a_long_flicker_under_a_fixed_width_keeps_the_modeset_in_the_trace() {
    let mut rig = Rig::generic_15(fixed_width());
    rig.frame(MD_H40);
    for _ in 0..200 {
        rig.frame(MD_H32);
        rig.frame(MD_H40);
    }
    let trace = rig.engine.trace();
    assert_eq!(trace.modesets().count(), 1, "the launch modeset is kept");
    assert_eq!(trace.records().len(), 2);
    let merged = trace.last().unwrap();
    assert_eq!(merged.outcome, Outcome::ScaleOnly);
    assert_eq!(merged.count, 400);
    assert_eq!(merged.decided_frame, 401);
    assert_eq!(merged.source, MD_H40);
}

#[test]
fn the_overscan_crop_of_the_policy_reaches_the_scene() {
    // An NES on arcade_31: 240 lines at 2x miss the 478 lines the range
    // holds, so the policy hides 8 lines per edge.
    let monitor = neptron_video::Monitor::Profile(
        neptron_testkit::preset::monitor_preset("arcade_31").unwrap(),
    );
    let nes = crate::harness::source(256, 240, 60_099);
    let mut rig = Rig::new(monitor, PolicySettings::default());
    rig.frame(nes);
    let proposal = rig.engine.current().unwrap().chosen.proposal;
    let crop = proposal.overscan_crop.expect("the policy crops this game");
    let scene = rig.fake.frames().last().unwrap().scene;
    assert_eq!(scene.crop, proposal.placement.crop);
    assert_eq!(scene.crop.h, 240 - u32::from(crop.lines()));
    assert_eq!(scene.crop.y, i32::from(crop.lines_per_edge));
    assert_eq!(scene.filter, Filter::Nearest);
    assert_eq!(scene.dest.h % scene.crop.h, 0);
    assert!(scene.dest.h / scene.crop.h >= 2);
}
