use neptron_types::{Orientation, Rotation, SourceGeometry};

/// What an engine shows, which sets the source that the policy plans.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Content {
    /// Games: the policy plans the display source of each core source,
    /// with the screen orientation and the game override.
    #[default]
    Game,
    /// The menu: the source is the source of the menu canvas, and the
    /// engine reads no game override. The mode of the menu on a turned
    /// screen follows the choice.
    Menu(TurnedMenuMode),
}

/// The mode of the menu on a screen turned by 90 or 270 degrees: 320
/// canvas columns across the lines of the display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TurnedMenuMode {
    /// The policy's answer for the turned menu source. On a 15 kHz CRT
    /// and on a TV it has 320 lines or more, so it is interlaced, and the
    /// canvas is 1:1, sharp, with flicker.
    Interlaced,
    /// The mode of the menu on a screen at 0. On a 15 kHz CRT and on a TV
    /// it has 240 or 288 progressive lines, and the canvas is squeezed
    /// into them with sharp bilinear, soft, with no flicker.
    Progressive,
}

/// The mode of the turned menu that the app uses. The owner has not
/// decided it yet (question T3 of `docs/spec/tate.md`); this is the
/// proposed default of the spec.
pub const TURNED_MENU_MODE: TurnedMenuMode = TurnedMenuMode::Interlaced;

/// The source that the policy plans for the menu source `menu` on a
/// screen at `screen`, with the mode of the turned menu `choice`. The
/// canvas turns with the screen in every case; only the mode changes.
pub const fn menu_plan_source(
    choice: TurnedMenuMode,
    screen: Rotation,
    menu: &SourceGeometry,
) -> SourceGeometry {
    let turned = Orientation {
        screen,
        game_override: None,
    }
    .display_source(menu);
    match choice {
        TurnedMenuMode::Interlaced => turned,
        TurnedMenuMode::Progressive if screen.across() => *menu,
        TurnedMenuMode::Progressive => turned,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MENU: SourceGeometry = SourceGeometry {
        width: 320,
        height: 240,
        refresh_mhz: 60_000,
        aspect_milli: 1_333,
        interlaced: false,
        rotation: Rotation::R0,
    };

    fn planned(choice: TurnedMenuMode, screen: Rotation) -> Rotation {
        let source = menu_plan_source(choice, screen, &MENU);
        assert_eq!(
            SourceGeometry {
                rotation: Rotation::R0,
                ..source
            },
            MENU,
            "only the turn changes"
        );
        source.rotation
    }

    #[test]
    fn interlaced_plans_the_menu_turned_with_the_screen() {
        let turns = Rotation::ALL.map(|s| planned(TurnedMenuMode::Interlaced, s));
        assert_eq!(
            turns,
            [Rotation::R0, Rotation::R270, Rotation::R180, Rotation::R90]
        );
    }

    #[test]
    fn progressive_plans_the_menu_of_a_screen_at_0_on_a_turned_screen() {
        let turns = Rotation::ALL.map(|s| planned(TurnedMenuMode::Progressive, s));
        assert_eq!(
            turns,
            [Rotation::R0, Rotation::R0, Rotation::R180, Rotation::R0]
        );
    }

    #[test]
    fn the_proposed_default_is_interlaced() {
        assert_eq!(TURNED_MENU_MODE, TurnedMenuMode::Interlaced);
    }
}
