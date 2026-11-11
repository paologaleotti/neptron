//! The screens: which template shows which snapshot data, and the screen
//! stack. Each screen registers its scenes in [`SCENES`].

use crate::registry::SceneDef;

/// The scenes of the screens, added to the registry after the template
/// scenes.
pub static SCENES: &[SceneDef] = &[];
