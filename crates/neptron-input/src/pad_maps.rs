//! The map of a pad, and the sources that give it by the identity of the
//! controller.

use crate::device::DeviceIdentity;
use crate::generic_layout::{GenericLayout, JoystickCounts, NotAGamepad};

/// What turns the raw input of one pad into RetroPad controls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum PadMap {
    /// The standard layout of SDL3: the pad has a gamepad mapping, from
    /// the NEPTRON mapping file or the database of SDL3.
    #[default]
    Known,
    /// The generic layout by position, for a joystick with no mapping.
    Generic(GenericLayout),
}

/// Which kind of map a pad uses, as the roster and the Input panel show
/// it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum MapKind {
    /// A gamepad mapping: the buttons are where their labels say.
    #[default]
    Known,
    /// The generic layout: some buttons can be in other places.
    Generic,
}

/// What a backend knows of a pad before it has a map.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PadLayout {
    /// SDL3 has a gamepad mapping for it.
    Gamepad,
    /// A plain joystick, with its counts.
    Joystick(JoystickCounts),
}

/// A source of the map of a pad. The backend asks it once, when the pad
/// connects. A source of the maps of the player, kept in the settings by
/// the identity of the controller, goes in front of [`BuiltinMaps`].
pub trait PadMaps {
    /// The map of the pad with `identity` and `layout`.
    ///
    /// # Errors
    ///
    /// [`NotAGamepad`] when the pad is not read at all.
    fn map(&self, identity: DeviceIdentity, layout: PadLayout) -> Result<PadMap, NotAGamepad>;
}

/// The maps that need no settings: the standard layout for a pad with a
/// gamepad mapping, the generic layout for any other joystick.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BuiltinMaps;

impl PadMap {
    /// The kind of this map.
    pub fn kind(&self) -> MapKind {
        match self {
            Self::Known => MapKind::Known,
            Self::Generic(_) => MapKind::Generic,
        }
    }
}

impl PadMaps for BuiltinMaps {
    fn map(&self, _identity: DeviceIdentity, layout: PadLayout) -> Result<PadMap, NotAGamepad> {
        match layout {
            PadLayout::Gamepad => Ok(PadMap::Known),
            PadLayout::Joystick(counts) => GenericLayout::for_counts(counts).map(PadMap::Generic),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_builtin_maps_give_known_or_generic() {
        let id = DeviceIdentity(7);
        assert_eq!(BuiltinMaps.map(id, PadLayout::Gamepad), Ok(PadMap::Known));
        let counts = JoystickCounts {
            buttons: 13,
            hats: 1,
            axes: 4,
        };
        let map = BuiltinMaps.map(id, PadLayout::Joystick(counts));
        assert_eq!(
            map,
            Ok(PadMap::Generic(GenericLayout::for_counts(counts).unwrap()))
        );
        assert_eq!(map.map(|m| m.kind()), Ok(MapKind::Generic));
        assert_eq!(PadMap::Known.kind(), MapKind::Known);
        let none = JoystickCounts {
            buttons: 1,
            hats: 1,
            axes: 0,
        };
        assert!(BuiltinMaps.map(id, PadLayout::Joystick(none)).is_err());
    }
}
