//! The map of a pad, and the sources that give it by the identity of the
//! controller.

use crate::device::DeviceIdentity;
use crate::generic_layout::{GenericLayout, JoystickCounts, NotAGamepad};
use crate::player_map::PlayerMap;
use crate::raw_state::RawReads;

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
    /// The map of the player, from the settings folder. It reads raw
    /// joystick indices, for a gamepad and a plain joystick alike.
    Player(PlayerMap),
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
    /// The map that the player made for this controller model.
    Player,
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

/// The maps of the player, by the identity of the controller, in front of
/// [`BuiltinMaps`]. Two pads of one model share a map.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PlayerMaps {
    maps: Vec<(DeviceIdentity, PlayerMap)>,
}

impl PadMap {
    /// The kind of this map.
    pub fn kind(&self) -> MapKind {
        match self {
            Self::Known => MapKind::Known,
            Self::Generic(_) => MapKind::Generic,
            Self::Player(_) => MapKind::Player,
        }
    }

    /// The raw indices that a backend reads for this map, or `None` for
    /// the standard layout, which the backend reads by name.
    pub fn raw_reads(&self) -> Option<RawReads> {
        match self {
            Self::Known => None,
            Self::Generic(layout) => Some(layout.reads()),
            Self::Player(map) => Some(map.reads()),
        }
    }
}

impl PlayerMaps {
    /// No map.
    pub fn new() -> Self {
        Self::default()
    }

    /// The map of the controller model `identity`.
    pub fn get(&self, identity: DeviceIdentity) -> Option<&PlayerMap> {
        self.maps
            .iter()
            .find(|(id, _)| *id == identity)
            .map(|(_, map)| map)
    }

    /// Sets the map of `identity`, or replaces it.
    pub fn insert(&mut self, identity: DeviceIdentity, map: PlayerMap) {
        match self.maps.iter_mut().find(|(id, _)| *id == identity) {
            Some(slot) => slot.1 = map,
            None => self.maps.push((identity, map)),
        }
    }

    /// Removes the map of `identity`. True when it had one.
    pub fn remove(&mut self, identity: DeviceIdentity) -> bool {
        let before = self.maps.len();
        self.maps.retain(|(id, _)| *id != identity);
        self.maps.len() != before
    }

    /// Each identity and its map, in the order they were added.
    pub fn iter(&self) -> impl Iterator<Item = (DeviceIdentity, &PlayerMap)> {
        self.maps.iter().map(|(id, map)| (*id, map))
    }
}

impl PadMaps for PlayerMaps {
    fn map(&self, identity: DeviceIdentity, layout: PadLayout) -> Result<PadMap, NotAGamepad> {
        match self.get(identity) {
            Some(map) => Ok(PadMap::Player(*map)),
            None => BuiltinMaps.map(identity, layout),
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

    #[test]
    fn the_map_of_the_player_comes_first_for_any_layout() {
        let mine = DeviceIdentity(7);
        let other = DeviceIdentity(8);
        let mut map = PlayerMap::new();
        map.set(neptron_types::Control::A, Some(crate::Binding::Button(3)))
            .unwrap();
        let mut maps = PlayerMaps::new();
        maps.insert(mine, map);
        let few = JoystickCounts {
            buttons: 1,
            hats: 0,
            axes: 0,
        };
        assert_eq!(maps.map(mine, PadLayout::Gamepad), Ok(PadMap::Player(map)));
        assert_eq!(
            maps.map(mine, PadLayout::Joystick(few)),
            Ok(PadMap::Player(map)),
            "the player's map reads a pad that the generic layout refuses"
        );
        assert_eq!(maps.map(other, PadLayout::Gamepad), Ok(PadMap::Known));
        assert!(maps.map(other, PadLayout::Joystick(few)).is_err());
        assert_eq!(PadMap::Player(map).kind(), MapKind::Player);
        assert_eq!(PadMap::Player(map).raw_reads().map(|r| r.buttons), Some(4));
        assert_eq!(PadMap::Known.raw_reads(), None);
        let mut other_map = map;
        other_map.set(neptron_types::Control::A, None).unwrap();
        maps.insert(mine, other_map);
        assert_eq!(maps.iter().count(), 1, "insert replaces");
        assert_eq!(maps.get(mine), Some(&other_map));
        assert!(maps.remove(mine));
        assert!(!maps.remove(mine));
        assert_eq!(maps.map(mine, PadLayout::Gamepad), Ok(PadMap::Known));
    }
}
