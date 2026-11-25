use neptron_modeline::Aspect;
use neptron_types::{Form, MonitorKind, Rotation, SourceGeometry};

/// What an engine shows, which sets the source that the policy plans.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Content {
    /// Games: the policy plans the display source of each core source,
    /// with the screen orientation and the game override.
    #[default]
    Game,
    /// The menu: the policy plans the raster of the canvas after its turn
    /// ([`menu_source`]), and the engine reads no game override.
    Menu,
}

/// The form of the UI on a display of `kind` at the screen orientation
/// `screen`, by [`Form::of`]. The face in scan-out is `crt_face` on a CRT,
/// 4:3 on a TV, and the first (preferred) mode on a flat panel; a panel
/// with no mode counts as 4:3.
pub fn form_of(kind: &MonitorKind, crt_face: Aspect, screen: Rotation) -> Form {
    let face = match kind {
        MonitorKind::Crt { .. } => (u32::from(crt_face.width), u32::from(crt_face.height)),
        MonitorKind::Tv { .. } => standard_face(),
        MonitorKind::FlatPanel { modes } => modes.first().map_or_else(standard_face, |m| {
            (u32::from(m.hactive), u32::from(m.vactive))
        }),
    };
    Form::of(face.0, face.1, screen)
}

/// The canvas of `form` after the turn `ui_turn`, in scan-out pixels:
/// width, then lines. This is `T`, the size that the UI placement takes.
pub fn turned_canvas(form: Form, ui_turn: Rotation) -> (u16, u16) {
    let (width, height) = form.canvas();
    if ui_turn.across() {
        (height, width)
    } else {
        (width, height)
    }
}

/// The source that the policy plans for the menu: the raster of `T`, the
/// canvas of `form` after the turn `ui_turn`, progressive, not turned, at
/// the rate of `menu`, with square canvas pixels. On a CRT, a TV and a
/// landscape panel `T` is 320x240 at every orientation, so the menu has
/// the same mode at every orientation.
pub fn menu_source(form: Form, ui_turn: Rotation, menu: &SourceGeometry) -> SourceGeometry {
    let (width, height) = turned_canvas(form, ui_turn);
    let aspect = u64::from(width) * 1_000 / u64::from(height.max(1));
    SourceGeometry {
        width,
        height,
        aspect_milli: u32::try_from(aspect).unwrap_or(u32::MAX),
        interlaced: false,
        rotation: Rotation::R0,
        ..*menu
    }
}

fn standard_face() -> (u32, u32) {
    let face = Aspect::STANDARD_CRT;
    (u32::from(face.width), u32::from(face.height))
}

#[cfg(test)]
mod tests {
    use neptron_types::{Modeline, Orientation, TvStandard};

    use super::*;

    const MENU: SourceGeometry = SourceGeometry {
        width: 320,
        height: 240,
        refresh_mhz: 60_000,
        aspect_milli: 1_333,
        interlaced: false,
        rotation: Rotation::R0,
    };

    fn panel(hactive: u16, vactive: u16) -> MonitorKind {
        MonitorKind::FlatPanel {
            modes: vec![Modeline {
                hactive,
                vactive,
                ..neptron_types::TV_525_240P
            }],
        }
    }

    fn crt() -> MonitorKind {
        MonitorKind::Crt { ranges: Vec::new() }
    }

    fn form(kind: &MonitorKind, screen: Rotation) -> Form {
        form_of(kind, Aspect::STANDARD_CRT, screen)
    }

    #[test]
    fn a_crt_a_tv_and_a_landscape_panel_are_portrait_only_when_turned_across() {
        let tv = TvStandard::Pal.profile().kind;
        for kind in [crt(), tv, panel(1_920, 1_080)] {
            for screen in Rotation::ALL {
                let want = if screen.across() {
                    Form::Portrait
                } else {
                    Form::Landscape
                };
                assert_eq!(form(&kind, screen), want, "{kind:?} {screen:?}");
            }
        }
    }

    #[test]
    fn a_portrait_panel_is_portrait_at_0_and_180() {
        let kind = panel(1_080, 1_920);
        assert_eq!(form(&kind, Rotation::R0), Form::Portrait);
        assert_eq!(form(&kind, Rotation::R180), Form::Portrait);
        assert_eq!(form(&kind, Rotation::R90), Form::Landscape);
        assert_eq!(form(&kind, Rotation::R270), Form::Landscape);
    }

    #[test]
    fn a_panel_with_no_mode_counts_as_4_3() {
        let kind = MonitorKind::FlatPanel { modes: Vec::new() };
        assert_eq!(form(&kind, Rotation::R0), Form::Landscape);
        assert_eq!(form(&kind, Rotation::R90), Form::Portrait);
    }

    /// The claim of `docs/spec/ui-layout.md`, section 6.3: on a CRT, a TV
    /// and a landscape panel the menu source is the menu source of a
    /// screen at 0 at every orientation.
    #[test]
    fn the_menu_source_is_the_same_at_every_orientation_of_a_crt() {
        for screen in Rotation::ALL {
            let orientation = Orientation {
                screen,
                game_override: None,
            };
            let form = form(&crt(), screen);
            assert_eq!(
                menu_source(form, orientation.ui_turn(), &MENU),
                MENU,
                "{screen:?}"
            );
        }
    }

    #[test]
    fn a_portrait_panel_plans_the_portrait_raster() {
        let source = menu_source(Form::Portrait, Rotation::R0, &MENU);
        assert_eq!((source.width, source.height), (240, 320));
        assert_eq!(source.aspect_milli, 750);
        assert_eq!(source.rotation, Rotation::R0);
        assert_eq!(source.refresh_mhz, MENU.refresh_mhz);
    }

    #[test]
    fn the_turned_canvas_swaps_its_sides_across() {
        assert_eq!(turned_canvas(Form::Portrait, Rotation::R270), (320, 240));
        assert_eq!(turned_canvas(Form::Portrait, Rotation::R0), (240, 320));
        assert_eq!(turned_canvas(Form::Landscape, Rotation::R180), (320, 240));
        assert_eq!(turned_canvas(Form::Landscape, Rotation::R90), (240, 320));
    }
}
