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
    /// SDL3 has a gamepad mapping for it. The counts are those of its
    /// joystick, which a map of the player reads.
    Gamepad(JoystickCounts),
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

impl PlayerMaps {
    /// The bindings of the map of `identity` that the pad with `layout`
    /// does not have: an index past its counts. [`PadMaps::map`] drops
    /// them. 0 when the pad does not use the map.
    pub fn dropped(&self, identity: DeviceIdentity, layout: PadLayout) -> u32 {
        self.fitted(identity, layout)
            .map_or(0, |(_, dropped)| dropped)
    }

    /// The map of `identity` with only the bindings that the pad has, and
    /// the count of the others. `None` with no map, or when the fitted
    /// map has a menu gap: a damaged file or a pad with fewer inputs can
    /// leave one, and then the pad could not drive the menus.
    fn fitted(&self, identity: DeviceIdentity, layout: PadLayout) -> Option<(PlayerMap, u32)> {
        self.get(identity)
            .map(|map| map.fitted(layout.counts()))
            .filter(|(map, _)| map.menu_gaps().next().is_none())
    }
}

impl PadLayout {
    /// The counts of the joystick of the pad.
    pub fn counts(&self) -> JoystickCounts {
        match *self {
            Self::Gamepad(counts) | Self::Joystick(counts) => counts,
        }
    }
}

impl PadMaps for PlayerMaps {
    /// The map of the player of `identity`, with only the bindings that
    /// the pad has, or the built-in map. A map that would leave a menu
    /// gap is not used: the pad gets the built-in map.
    fn map(&self, identity: DeviceIdentity, layout: PadLayout) -> Result<PadMap, NotAGamepad> {
        match self.fitted(identity, layout) {
            Some((map, _)) => Ok(PadMap::Player(map)),
            None => BuiltinMaps.map(identity, layout),
        }
    }
}

impl PadMaps for BuiltinMaps {
    fn map(&self, _identity: DeviceIdentity, layout: PadLayout) -> Result<PadMap, NotAGamepad> {
        match layout {
            PadLayout::Gamepad(_) => Ok(PadMap::Known),
            PadLayout::Joystick(counts) => GenericLayout::for_counts(counts).map(PadMap::Generic),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAD: JoystickCounts = JoystickCounts {
        buttons: 16,
        hats: 2,
        axes: 6,
    };

    #[test]
    fn the_builtin_maps_give_known_or_generic() {
        let id = DeviceIdentity(7);
        assert_eq!(
            BuiltinMaps.map(id, PadLayout::Gamepad(PAD)),
            Ok(PadMap::Known)
        );
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

    /// A map that drives the menus with one button and three axes: the
    /// directions on axes 0 and 1, A on button 0, B on axis 2.
    fn menu_map() -> PlayerMap {
        use crate::Binding;
        use neptron_types::Control;
        let half = |index, positive| {
            Some(Binding::HalfAxis {
                index,
                positive,
                full: false,
            })
        };
        let mut map = PlayerMap::new();
        map.set(Control::Left, half(0, false)).unwrap();
        map.set(Control::Right, half(0, true)).unwrap();
        map.set(Control::Up, half(1, false)).unwrap();
        map.set(Control::Down, half(1, true)).unwrap();
        map.set(Control::A, Some(Binding::Button(0))).unwrap();
        map.set(Control::B, half(2, true)).unwrap();
        map
    }

    #[test]
    fn the_map_of_the_player_comes_first_for_any_layout() {
        let mine = DeviceIdentity(7);
        let other = DeviceIdentity(8);
        let map = menu_map();
        let mut maps = PlayerMaps::new();
        maps.insert(mine, map);
        let few = JoystickCounts {
            buttons: 1,
            hats: 0,
            axes: 3,
        };
        assert_eq!(
            maps.map(mine, PadLayout::Gamepad(PAD)),
            Ok(PadMap::Player(map))
        );
        assert_eq!(
            maps.map(mine, PadLayout::Joystick(few)),
            Ok(PadMap::Player(map)),
            "the player's map reads a pad that the generic layout refuses"
        );
        assert_eq!(maps.map(other, PadLayout::Gamepad(PAD)), Ok(PadMap::Known));
        assert!(maps.map(other, PadLayout::Joystick(few)).is_err());
        assert_eq!(PadMap::Player(map).kind(), MapKind::Player);
        assert_eq!(
            PadMap::Player(map).raw_reads().map(|r| (r.buttons, r.axes)),
            Some((1, 3))
        );
        assert_eq!(PadMap::Known.raw_reads(), None);
        let mut other_map = map;
        other_map.set(neptron_types::Control::A, None).unwrap();
        maps.insert(mine, other_map);
        assert_eq!(maps.iter().count(), 1, "insert replaces");
        assert_eq!(maps.get(mine), Some(&other_map));
        assert!(maps.remove(mine));
        assert!(!maps.remove(mine));
        assert_eq!(maps.map(mine, PadLayout::Gamepad(PAD)), Ok(PadMap::Known));
    }

    #[test]
    fn a_map_drops_the_bindings_that_the_pad_does_not_have() {
        use crate::{Binding, HatDirection};
        use neptron_types::Control;
        let mine = DeviceIdentity(7);
        let mut map = menu_map();
        map.set(Control::X, Some(Binding::Button(12))).unwrap();
        let start = Binding::Hat {
            index: 1,
            direction: HatDirection::Up,
        };
        map.set(Control::Start, Some(start)).unwrap();
        let l2 = Binding::HalfAxis {
            index: 5,
            positive: true,
            full: true,
        };
        map.set(Control::L2, Some(l2)).unwrap();
        let mut maps = PlayerMaps::new();
        maps.insert(mine, map);
        let small = PadLayout::Joystick(JoystickCounts {
            buttons: 12,
            hats: 1,
            axes: 4,
        });
        let Ok(PadMap::Player(fitted)) = maps.map(mine, small) else {
            panic!("a map of the player");
        };
        assert_eq!(fitted.get(Control::A), Some(Binding::Button(0)));
        assert_eq!(fitted.get(Control::X), None);
        assert_eq!(fitted.get(Control::Start), None);
        assert_eq!(fitted.get(Control::L2), None);
        assert_eq!(maps.dropped(mine, small), 3);
        assert_eq!(maps.dropped(mine, PadLayout::Gamepad(PAD)), 0);
        assert_eq!(
            maps.map(mine, PadLayout::Gamepad(PAD)),
            Ok(PadMap::Player(map))
        );
    }

    #[test]
    fn a_map_that_cannot_drive_the_menus_gives_the_builtin_map() {
        use neptron_types::Control;
        let mine = DeviceIdentity(7);
        let mut no_b = menu_map();
        no_b.set(Control::B, None).unwrap();
        let mut maps = PlayerMaps::new();
        maps.insert(mine, no_b);
        assert_eq!(maps.map(mine, PadLayout::Gamepad(PAD)), Ok(PadMap::Known));
        assert_eq!(maps.dropped(mine, PadLayout::Gamepad(PAD)), 0);
        let joystick = PadLayout::Joystick(PAD);
        assert_eq!(
            maps.map(mine, joystick),
            BuiltinMaps.map(mine, joystick),
            "a plain joystick gets the generic layout"
        );

        // A pad with no axis 2 loses B in the fit: the same rule.
        maps.insert(mine, menu_map());
        let two_axes = PadLayout::Gamepad(JoystickCounts {
            buttons: 4,
            hats: 0,
            axes: 2,
        });
        assert_eq!(maps.map(mine, two_axes), Ok(PadMap::Known));
        assert_eq!(maps.dropped(mine, two_axes), 0, "no trimmed notice");
    }
}
