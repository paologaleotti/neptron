//! One scan of a pad backend: what a scan does when its device list
//! fails.

use neptron_types::InputEvent;

use crate::mapper::Mapper;
use crate::notice::{InputNotice, NoticeQueue};

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
    mapper.connect_scan(Vec::new(), out);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device::DeviceIdentity;
    use crate::roster::{PadInfo, PortRule, Roster, RosterEntry};

    fn info(n: u8) -> PadInfo {
        PadInfo {
            identity: DeviceIdentity(u64::from(n)),
            path: DeviceIdentity(100 + u64::from(n)),
            name: format!("Pad {n}"),
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
        // The next scan is not a first scan: a held port of a game run
        // would stay held.
        let mut game = Mapper::with_rule(PortRule::Game, &earlier);
        list_failed(&mut game, "x", String::new(), &mut out, &mut notices);
        assert_eq!(game.roster().get(0).map(|e| e.present), Some(false));
    }
}
