//! One scan of a pad backend: the map of each pad found, its port, and
//! the notices. The SDL3 source and the scripted pads share it, so the
//! tests of the frontend see the notices of the device.

use neptron_types::InputEvent;

use crate::mapper::{Mapper, MapperError};
use crate::notice::{InputNotice, NoticeQueue};
use crate::pad_maps::{PadLayout, PadMap, PadMaps, PlayerMaps};
use crate::port_table::PadId;
use crate::roster::PadInfo;

/// A pad that a backend found in a scan, before it has a map and a port.
pub(crate) struct Found<T> {
    /// What the backend keeps of the pad: its open device.
    pub(crate) handle: T,
    pub(crate) info: PadInfo,
    pub(crate) layout: PadLayout,
    /// What the backend knows of it, for the log.
    pub(crate) what: String,
}

/// A pad of the scan that got a port.
pub(crate) struct Opened<T> {
    pub(crate) handle: T,
    pub(crate) id: PadId,
    pub(crate) map: PadMap,
}

/// The pads of the scan, opened or refused.
pub(crate) struct Scan<T> {
    pub(crate) opened: Vec<Opened<T>>,
    /// The pads that are not read until they are plugged again.
    pub(crate) refused: Vec<T>,
}

/// Gives each pad of one scan its map from `maps` and its port from the
/// mapper, and pushes the notices: a connect for each pad, the generic
/// layout, a joystick that is not a gamepad, a pad with no free port.
/// Every scan goes to the mapper, an empty one too: the first one settles
/// the ports.
pub(crate) fn connect_found<T>(
    mapper: &mut Mapper,
    maps: &PlayerMaps,
    found: Vec<Found<T>>,
    out: &mut Vec<InputEvent>,
    notices: &mut NoticeQueue,
) -> Scan<T> {
    let mut refused = Vec::new();
    let mut mapped = Vec::with_capacity(found.len());
    for pad in found {
        match maps.map(pad.info.identity, pad.layout) {
            Ok(map) => {
                let dropped = maps.dropped(pad.info.identity, pad.layout);
                if dropped > 0 {
                    notices.push(InputNotice::LayoutTrimmed {
                        pad: pad.info.clone(),
                        dropped,
                    });
                }
                mapped.push((pad, map));
            }
            Err(reason) => {
                notices.push(InputNotice::NotAGamepad {
                    pad: pad.info,
                    reason,
                });
                refused.push(pad.handle);
            }
        }
    }
    let pairs = mapped
        .iter()
        .map(|(pad, map)| (pad.info.clone(), *map))
        .collect();
    let results = mapper.connect_scan_with(pairs, out);
    let mut opened = Vec::with_capacity(mapped.len());
    for ((pad, map), result) in mapped.into_iter().zip(results) {
        match result {
            Ok(id) => {
                if let Some(device) = mapper.device_of(id) {
                    notices.push(InputNotice::Connected {
                        device,
                        what: pad.what,
                    });
                    if matches!(map, PadMap::Generic(_)) {
                        notices.push(InputNotice::GenericLayout {
                            device,
                            pad: pad.info,
                        });
                    }
                }
                opened.push(Opened {
                    handle: pad.handle,
                    id,
                    map,
                });
            }
            Err(MapperError::NoFreePort { .. }) => {
                notices.push(InputNotice::NoFreePort { pad: pad.info });
                refused.push(pad.handle);
            }
            Err(e) => {
                notices.push(InputNotice::Ignored {
                    what: pad.what,
                    reason: e.to_string(),
                });
                refused.push(pad.handle);
            }
        }
    }
    Scan { opened, refused }
}

/// A scan whose list of devices failed: a notice, and an empty scan for
/// the mapper, so the first scan of a run still settles the ports and the
/// held ports of the run before do not stay away in the menus.
#[cfg_attr(not(feature = "sdl3"), allow(dead_code))]
pub(crate) fn list_failed(
    mapper: &mut Mapper,
    what: &str,
    reason: String,
    out: &mut Vec<InputEvent>,
    notices: &mut NoticeQueue,
) {
    notices.push(InputNotice::Ignored {
        what: what.to_owned(),
        reason,
    });
    mapper.connect_scan_with(Vec::new(), out);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device::DeviceIdentity;
    use crate::generic_layout::{JoystickCounts, NotAGamepad};
    use crate::pad_maps::MapKind;
    use crate::roster::{PortRule, Roster, RosterEntry};

    fn info(n: u8) -> PadInfo {
        PadInfo {
            identity: DeviceIdentity(u64::from(n)),
            path: DeviceIdentity(100 + u64::from(n)),
            name: format!("Pad {n}"),
        }
    }

    fn found(n: u8, layout: PadLayout) -> Found<u8> {
        Found {
            handle: n,
            info: info(n),
            layout,
            what: format!("pad {n}"),
        }
    }

    fn drain(notices: &mut NoticeQueue) -> Vec<InputNotice> {
        let mut out = Vec::new();
        notices.drain_into(&mut out);
        out
    }

    #[test]
    fn a_failed_list_still_settles_the_first_scan_of_the_menus() {
        let mut earlier = Roster::new();
        let entry = RosterEntry {
            pad: info(1),
            present: true,
            map: MapKind::Known,
        };
        earlier.set(0, Some(entry)).unwrap();
        let mut mapper = Mapper::with_rule(PortRule::Menus, &earlier);
        let (mut out, mut notices) = (Vec::new(), NoticeQueue::default());
        list_failed(
            &mut mapper,
            "the joystick list",
            "no list".to_owned(),
            &mut out,
            &mut notices,
        );
        assert!(mapper.roster().is_empty(), "the held port is freed");
        assert_eq!(
            drain(&mut notices),
            [InputNotice::Ignored {
                what: "the joystick list".to_owned(),
                reason: "no list".to_owned()
            }]
        );
        // A failed list is the first scan of a game run too, and a game
        // keeps the held port.
        let mut game = Mapper::with_rule(PortRule::Game, &earlier);
        list_failed(&mut game, "x", String::new(), &mut out, &mut notices);
        assert_eq!(game.roster().get(0).map(|e| e.present), Some(false));
    }

    #[test]
    fn a_scan_gives_known_generic_and_refused_pads_their_notices() {
        let mut mapper = Mapper::new();
        let (mut out, mut notices) = (Vec::new(), NoticeQueue::default());
        let counts = |buttons, hats| JoystickCounts {
            buttons,
            hats,
            axes: 0,
        };
        let scan = connect_found(
            &mut mapper,
            &PlayerMaps::new(),
            vec![
                found(1, PadLayout::Gamepad(JoystickCounts::default())),
                found(2, PadLayout::Joystick(counts(12, 1))),
                found(3, PadLayout::Joystick(counts(1, 1))),
            ],
            &mut out,
            &mut notices,
        );
        let opened: Vec<_> = scan
            .opened
            .iter()
            .map(|o| (o.handle, o.map.kind()))
            .collect();
        assert_eq!(opened, [(1, MapKind::Known), (2, MapKind::Generic)]);
        assert_eq!(scan.refused, [3]);
        assert_eq!(
            drain(&mut notices),
            [
                InputNotice::NotAGamepad {
                    pad: info(3),
                    reason: NotAGamepad::TooFewButtons { buttons: 1, min: 2 }
                },
                InputNotice::Connected {
                    device: 0x100,
                    what: "pad 1".to_owned()
                },
                InputNotice::Connected {
                    device: 0x101,
                    what: "pad 2".to_owned()
                },
                InputNotice::GenericLayout {
                    device: 0x101,
                    pad: info(2)
                },
            ]
        );
        let kinds: Vec<_> = mapper.roster().ports()[..3]
            .iter()
            .map(|e| e.as_ref().map(|e| e.map))
            .collect();
        assert_eq!(kinds, [Some(MapKind::Known), Some(MapKind::Generic), None]);
    }

    #[test]
    fn a_ninth_pad_gives_its_notice() {
        let mut mapper = Mapper::new();
        let (mut out, mut notices) = (Vec::new(), NoticeQueue::default());
        let pads = (1..=9)
            .map(|n| found(n, PadLayout::Gamepad(JoystickCounts::default())))
            .collect();
        let scan = connect_found(
            &mut mapper,
            &PlayerMaps::new(),
            pads,
            &mut out,
            &mut notices,
        );
        assert_eq!(scan.opened.len(), 8);
        assert_eq!(scan.refused, [9]);
        assert!(
            drain(&mut notices).contains(&InputNotice::NoFreePort { pad: info(9) }),
            "the ninth pad is told"
        );
    }
}
