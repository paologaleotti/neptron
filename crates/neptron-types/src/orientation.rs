use crate::{Rotation, SourceGeometry};

/// How the display stands and how the game picture turns: the screen
/// orientation and the game override.
///
/// The screen orientation `S` is the turn of the physical display,
/// counter-clockwise, as the player faces it: [`Rotation::R90`] is a
/// display turned a quarter turn to the left, its top edge on the left.
/// This is the direction of the turn that a core asks for, so one type
/// holds both.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Orientation {
    /// The screen orientation `S`.
    pub screen: Rotation,
    /// The game override `O`: a fixed game orientation that replaces the
    /// turn that the core asks for, or `None` for Auto.
    pub game_override: Option<Rotation>,
}

/// The turns of one case: the game picture and the UI canvas relative to
/// the scan-out, and how the game picture is sized.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Turns {
    /// The game orientation in use `G'`: the override when it is set,
    /// else the turn that the core asks for.
    pub game_orientation: Rotation,
    /// The turn of the game picture relative to the scan-out,
    /// `R = (G' - S) mod 360`: the value of `Scene::rotation`.
    pub game: Rotation,
    /// The turn of the UI canvas relative to the scan-out,
    /// `U = (0 - S) mod 360`, so that the canvas is upright for the
    /// player.
    pub ui: Rotation,
    /// How the game picture is sized, from `game`.
    pub size: PictureSize,
    /// A horizontal game stands across a turned screen at its own
    /// orientation: the screen is at 90 or 270, the picture is across,
    /// and the override is Auto or equal to the core's turn. The app adds
    /// its sentence to the picture notice.
    pub horizontal_on_turned_screen: bool,
}

/// How the game picture is sized on the display, from its turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PictureSize {
    /// The turn is [`Rotation::R0`] or [`Rotation::R180`]: the display
    /// scans the frame's own lines. The policy sees the raster of a
    /// horizontal source of that size, and the picture fills the screen.
    OwnLinesFull,
    /// The turn is [`Rotation::R90`] or [`Rotation::R270`]: the lines of
    /// the display are columns of the frame. The policy sees the swapped
    /// raster, and the picture is upright and small, with borders.
    AcrossSmall,
}

impl Orientation {
    /// A screen that stands as usual and a game at Auto: every turn is the
    /// turn of a display with no orientation.
    pub const NORMAL: Orientation = Orientation {
        screen: Rotation::R0,
        game_override: None,
    };

    /// The game orientation in use `G'`: the override when it is set, else
    /// `core`, the turn that the core asks for.
    pub const fn game_orientation(self, core: Rotation) -> Rotation {
        match self.game_override {
            Some(fixed) => fixed,
            None => core,
        }
    }

    /// The turn of the game picture relative to the scan-out,
    /// `R = (G' - S) mod 360`. A frame turned by `G'` is upright on a
    /// screen at 0, and the display adds its own turn `S` as the player
    /// sees it, so the renderer turns by the difference.
    pub const fn game_turn(self, core: Rotation) -> Rotation {
        self.game_orientation(core).minus(self.screen)
    }

    /// The turn of the UI canvas relative to the scan-out,
    /// `U = (0 - S) mod 360`. The canvas is always drawn upright.
    pub const fn ui_turn(self) -> Rotation {
        self.screen.inverse()
    }

    /// Every turn of the case of a game whose core asks for `core`.
    pub fn turns(self, core: Rotation) -> Turns {
        let game = self.game_turn(core);
        let size = if game.across() {
            PictureSize::AcrossSmall
        } else {
            PictureSize::OwnLinesFull
        };
        let at_own_orientation = self.game_override.is_none_or(|fixed| fixed == core);
        Turns {
            game_orientation: self.game_orientation(core),
            game,
            ui: self.ui_turn(),
            size,
            horizontal_on_turned_screen: self.screen.across()
                && game.across()
                && at_own_orientation,
        }
    }

    /// The display source: `source` with its rotation set to the game
    /// turn `R`. The size, the rate, the aspect and the interlace flag
    /// stay: they describe the frame as drawn, and `R` turns that frame.
    /// The policy plans with this source, so it places the lines that the
    /// display scans.
    pub const fn display_source(self, source: &SourceGeometry) -> SourceGeometry {
        SourceGeometry {
            rotation: self.game_turn(source.rotation),
            ..*source
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(rotation: Rotation, aspect_milli: u32, interlaced: bool) -> SourceGeometry {
        SourceGeometry {
            width: 320,
            height: 240,
            refresh_mhz: 57_550,
            aspect_milli,
            interlaced,
            rotation,
        }
    }

    fn at(screen: Rotation, game_override: Option<Rotation>) -> Orientation {
        Orientation {
            screen,
            game_override,
        }
    }

    #[test]
    fn normal_keeps_the_turn_of_the_core_and_an_upright_canvas() {
        for core in Rotation::ALL {
            let turns = Orientation::NORMAL.turns(core);
            assert_eq!(turns.game, core);
            assert_eq!(turns.game_orientation, core);
            assert_eq!(turns.ui, Rotation::R0);
            assert!(!turns.horizontal_on_turned_screen);
        }
    }

    #[test]
    fn the_ui_turn_is_the_inverse_of_the_screen() {
        let want = [Rotation::R0, Rotation::R270, Rotation::R180, Rotation::R90];
        for (screen, ui) in Rotation::ALL.into_iter().zip(want) {
            assert_eq!(at(screen, None).ui_turn(), ui, "{screen:?}");
            assert_eq!(at(screen, Some(Rotation::R90)).turns(Rotation::R0).ui, ui);
        }
    }

    #[test]
    fn the_game_turn_is_the_game_orientation_less_the_screen() {
        for screen in Rotation::ALL {
            for core in Rotation::ALL {
                for game_override in [None].into_iter().chain(Rotation::ALL.map(Some)) {
                    let o = at(screen, game_override);
                    let g = game_override.unwrap_or(core);
                    let turns = o.turns(core);
                    assert_eq!(turns.game_orientation, g);
                    assert_eq!(turns.game.plus(screen), g, "{o:?} {core:?}");
                    assert_eq!(turns.game, o.game_turn(core));
                    assert_eq!(
                        turns.size == PictureSize::AcrossSmall,
                        turns.game.across(),
                        "{o:?} {core:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn a_vertical_game_on_a_turned_screen_scans_its_own_lines() {
        let turns = at(Rotation::R90, None).turns(Rotation::R90);
        assert_eq!(turns.game, Rotation::R0);
        assert_eq!(turns.size, PictureSize::OwnLinesFull);
        let turns = at(Rotation::R270, None).turns(Rotation::R90);
        assert_eq!(turns.game, Rotation::R180);
        assert_eq!(turns.size, PictureSize::OwnLinesFull);
    }

    #[test]
    fn the_notice_is_for_a_horizontal_game_upright_across_a_turned_screen() {
        for screen in [Rotation::R90, Rotation::R270] {
            for core in [Rotation::R0, Rotation::R180] {
                assert!(at(screen, None).turns(core).horizontal_on_turned_screen);
                assert!(
                    at(screen, Some(core))
                        .turns(core)
                        .horizontal_on_turned_screen
                );
                // An override that does not match the game turns it on its
                // side or upside down: that picture is the user's choice.
                let other = core.plus(Rotation::R180);
                assert!(
                    !at(screen, Some(other))
                        .turns(core)
                        .horizontal_on_turned_screen
                );
            }
            for core in [Rotation::R90, Rotation::R270] {
                assert!(!at(screen, None).turns(core).horizontal_on_turned_screen);
            }
        }
        for screen in [Rotation::R0, Rotation::R180] {
            for core in Rotation::ALL {
                assert!(!at(screen, None).turns(core).horizontal_on_turned_screen);
            }
        }
    }

    #[test]
    fn the_display_source_keeps_every_field_but_the_rotation() {
        let cases = [
            source(Rotation::R90, 1_333, false),
            source(Rotation::R0, 1_333, true),
            source(Rotation::R270, 0, false),
        ];
        for original in cases {
            for screen in Rotation::ALL {
                let o = at(screen, None);
                let display = o.display_source(&original);
                assert_eq!(display.rotation, o.game_turn(original.rotation));
                assert_eq!(
                    SourceGeometry {
                        rotation: original.rotation,
                        ..display
                    },
                    original
                );
            }
        }
    }

    #[test]
    fn the_display_source_of_dodonpachi_on_a_screen_at_90_is_not_turned() {
        let dodonpachi = source(Rotation::R90, 1_333, false);
        let display = at(Rotation::R90, None).display_source(&dodonpachi);
        assert_eq!(display.rotation, Rotation::R0);
        assert_eq!(Orientation::NORMAL.display_source(&dodonpachi), dodonpachi);
    }
}
