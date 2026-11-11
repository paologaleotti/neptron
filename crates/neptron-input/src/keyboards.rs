//! evdev keyboards, grabbed, mapped to the RetroPad, with hotplug.

use std::io;
use std::path::PathBuf;

use evdev::{Device, EventType};
use neptron_backend::InputSource;
use neptron_types::InputEvent;

use crate::key_map::{looks_like_keyboard, mapped_keys_down};
use crate::mapper::Mapper;
use crate::notice::{InputNotice, NoticeQueue};
use crate::raw::{KeyAction, KeyCode, RawInput};

/// The folder of the input device nodes.
pub const INPUT_DIR: &str = "/dev/input";

/// The number of polls between two scans of [`INPUT_DIR`] for new
/// keyboards. At 60 frames a second a new keyboard is found within two
/// seconds.
pub const KEYBOARD_SCAN_INTERVAL_POLLS: u32 = 120;

/// The most reads of one keyboard in one poll. A keyboard with more
/// events waiting gives the rest at the next poll, so a flood cannot hold
/// the frame.
const MAX_READS_PER_POLL: usize = 8;

/// Every keyboard in [`INPUT_DIR`], mapped to the RetroPad on port 0.
///
/// Each keyboard is opened without blocking and grabbed, so that its keys
/// reach no other reader, such as a console. A keyboard that cannot be
/// grabbed is still read, with a notice. A node that cannot be opened (no
/// permission) gives a notice once and is not tried again while it exists.
/// A keyboard whose read fails (it was unplugged) is dropped, its keys are
/// released, and a notice says why.
///
/// No call blocks and none panics. The scan reads the folder, which is in
/// memory (devtmpfs), every [`KEYBOARD_SCAN_INTERVAL_POLLS`] polls.
pub struct EvdevKeyboards {
    dir: PathBuf,
    open: Vec<OpenKeyboard>,
    /// Nodes that are not read: not a keyboard, or they failed to open.
    skipped: Vec<PathBuf>,
    mapper: Mapper,
    polls_to_scan: u32,
    notices: NoticeQueue,
}

impl EvdevKeyboards {
    /// A source for the keyboards of [`INPUT_DIR`]. The first `poll`
    /// opens them.
    pub fn new() -> Self {
        Self::in_dir(INPUT_DIR)
    }

    /// A source for the keyboards of `dir`.
    pub fn in_dir(dir: impl Into<PathBuf>) -> Self {
        Self {
            dir: dir.into(),
            open: Vec::with_capacity(usize::from(crate::MAX_KEYBOARDS)),
            skipped: Vec::new(),
            mapper: Mapper::new(),
            polls_to_scan: 0,
            notices: NoticeQueue::default(),
        }
    }

    /// Moves the notices since the last call to `out`.
    pub fn take_notices(&mut self, out: &mut Vec<InputNotice>) {
        self.notices.drain_into(out);
    }

    fn scan(&mut self, out: &mut Vec<InputEvent>) {
        let entries = match std::fs::read_dir(&self.dir) {
            Ok(entries) => entries,
            Err(e) => {
                self.notices.push(InputNotice::Ignored {
                    what: self.dir.display().to_string(),
                    reason: e.to_string(),
                });
                return;
            }
        };
        let mut present = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            let is_event_node = path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("event"));
            if !is_event_node {
                continue;
            }
            let known = self.open.iter().any(|kb| kb.path == path) || self.skipped.contains(&path);
            if !known {
                self.try_open(path.clone(), out);
            }
            present.push(path);
        }
        self.skipped.retain(|path| present.contains(path));
    }

    fn try_open(&mut self, path: PathBuf, out: &mut Vec<InputEvent>) {
        let mut dev = match Device::open(&path) {
            Ok(dev) => dev,
            Err(e) => return self.skip(path, &e.to_string()),
        };
        let is_keyboard = dev.supported_keys().is_some_and(|keys| {
            looks_like_keyboard(|code| keys.contains(evdev::KeyCode::new(code)))
        });
        if !is_keyboard {
            self.skipped.push(path);
            return;
        }
        if let Err(e) = dev.set_nonblocking(true) {
            return self.skip(path, &e.to_string());
        }
        let device = match self.mapper.connect_keyboard() {
            Ok(device) => device,
            Err(e) => return self.skip(path, &e.to_string()),
        };
        // The grab comes after every check that can drop the device: the
        // drop of a grabbed device ungrabs it, and the crate prints the
        // error of that ungrab on stderr.
        if let Err(e) = dev.grab() {
            self.notices.push(InputNotice::NotGrabbed {
                device,
                reason: e.to_string(),
            });
        }
        self.press_held_keys(&dev, device, out);
        let what = format!("{} ({})", path.display(), dev.name().unwrap_or("no name"));
        self.notices.push(InputNotice::Connected { device, what });
        self.open.push(OpenKeyboard { dev, path, device });
    }

    /// Sends a press for each mapped key that is down when the keyboard
    /// opens, such as a combination held from power-on.
    fn press_held_keys(&mut self, dev: &Device, device: u32, out: &mut Vec<InputEvent>) {
        let keys = match dev.get_key_state() {
            Ok(keys) => keys,
            Err(e) => {
                self.notices.push(InputNotice::Ignored {
                    what: format!("the held keys of device {device:#x}"),
                    reason: e.to_string(),
                });
                return;
            }
        };
        for code in mapped_keys_down(|code| keys.contains(evdev::KeyCode::new(code))) {
            let input = RawInput::Key {
                code,
                action: KeyAction::Press,
            };
            if let Err(e) = self.mapper.apply(device, input, out) {
                self.notices.push(InputNotice::Ignored {
                    what: format!("device {device:#x}"),
                    reason: e.to_string(),
                });
            }
        }
    }

    fn skip(&mut self, path: PathBuf, reason: &str) {
        self.notices.push(InputNotice::Ignored {
            what: path.display().to_string(),
            reason: reason.to_owned(),
        });
        self.skipped.push(path);
    }

    fn read(&mut self, out: &mut Vec<InputEvent>) {
        let mut i = 0;
        while let Some(keyboard) = self.open.get_mut(i) {
            let Err(e) = read_keyboard(keyboard, &mut self.mapper, out) else {
                i += 1;
                continue;
            };
            let lost = self.open.remove(i);
            if let Err(mapper_error) = self.mapper.disconnect(lost.device, out) {
                self.notices.push(InputNotice::Ignored {
                    what: lost.path.display().to_string(),
                    reason: mapper_error.to_string(),
                });
            }
            self.notices.push(InputNotice::Lost {
                device: lost.device,
                reason: e.to_string(),
            });
        }
    }
}

impl Default for EvdevKeyboards {
    fn default() -> Self {
        Self::new()
    }
}

impl InputSource for EvdevKeyboards {
    fn poll(&mut self, out: &mut Vec<InputEvent>) {
        if self.polls_to_scan == 0 {
            self.scan(out);
            self.polls_to_scan = KEYBOARD_SCAN_INTERVAL_POLLS;
        }
        self.polls_to_scan -= 1;
        self.read(out);
    }
}

impl std::fmt::Debug for EvdevKeyboards {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let devices: Vec<u32> = self.open.iter().map(|kb| kb.device).collect();
        f.debug_struct("EvdevKeyboards")
            .field("dir", &self.dir)
            .field("devices", &devices)
            .finish_non_exhaustive()
    }
}

struct OpenKeyboard {
    dev: Device,
    path: PathBuf,
    device: u32,
}

/// Reads what the kernel holds for one keyboard into the mapper. `Ok` when
/// nothing more waits, or after [`MAX_READS_PER_POLL`] reads.
fn read_keyboard(
    keyboard: &mut OpenKeyboard,
    mapper: &mut Mapper,
    out: &mut Vec<InputEvent>,
) -> io::Result<()> {
    for _ in 0..MAX_READS_PER_POLL {
        let events = match keyboard.dev.fetch_events() {
            Ok(events) => events,
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                ) =>
            {
                return Ok(());
            }
            Err(e) => return Err(e),
        };
        for event in events {
            if event.event_type() != EventType::KEY {
                continue;
            }
            let Some(action) = key_action(event.value()) else {
                continue;
            };
            let input = RawInput::Key {
                code: KeyCode(event.code()),
                action,
            };
            mapper
                .apply(keyboard.device, input, out)
                .map_err(io::Error::other)?;
        }
    }
    Ok(())
}

/// The action of a key event value, or `None` for a value the kernel does
/// not define.
fn key_action(value: i32) -> Option<KeyAction> {
    match value {
        0 => Some(KeyAction::Release),
        1 => Some(KeyAction::Press),
        2 => Some(KeyAction::Repeat),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("neptron-input-{name}-{}", std::process::id()));
        if dir.exists() {
            std::fs::remove_dir_all(&dir).unwrap();
        }
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn poll_scan(source: &mut EvdevKeyboards) -> (Vec<InputEvent>, Vec<InputNotice>) {
        let mut events = Vec::new();
        let mut notices = Vec::new();
        for _ in 0..KEYBOARD_SCAN_INTERVAL_POLLS {
            source.poll(&mut events);
        }
        source.take_notices(&mut notices);
        (events, notices)
    }

    #[test]
    fn a_missing_folder_gives_a_notice_per_scan_and_no_event() {
        let mut source = EvdevKeyboards::in_dir("/nonexistent/neptron/input");
        let (events, notices) = poll_scan(&mut source);
        assert!(events.is_empty());
        assert!(matches!(notices.as_slice(), [InputNotice::Ignored { .. }]));
    }

    #[test]
    fn a_node_that_does_not_open_as_a_device_gives_one_notice() {
        let dir = scratch_dir("not-a-device");
        std::fs::write(dir.join("event0"), b"not a device").unwrap();
        std::fs::write(dir.join("mouse0"), b"not an event node").unwrap();
        let mut source = EvdevKeyboards::in_dir(&dir);
        let (events, notices) = poll_scan(&mut source);
        assert!(events.is_empty());
        assert_eq!(notices.len(), 1, "{notices:?}");
        assert!(
            matches!(&notices[0], InputNotice::Ignored { what, .. } if what.ends_with("event0"))
        );
        let (events, notices) = poll_scan(&mut source);
        assert!(events.is_empty());
        assert!(notices.is_empty(), "{notices:?}");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn key_values_map_to_actions() {
        assert_eq!(key_action(0), Some(KeyAction::Release));
        assert_eq!(key_action(1), Some(KeyAction::Press));
        assert_eq!(key_action(2), Some(KeyAction::Repeat));
        assert_eq!(key_action(3), None);
        assert_eq!(key_action(-1), None);
    }
}
