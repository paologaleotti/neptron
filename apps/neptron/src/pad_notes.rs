//! The notices of a pad for the player: what the input source tells, what
//! the player sees, and the pads told already in this power cycle, in
//! `run/pad-notes.toml`.
//!
//! A run is one process, and each run opens the pads again, so the source
//! gives the notice of a pad again in each run. The file keeps the pads
//! that were told, and the pads that the silent pad rule has checked, so
//! each notice comes once per plug. It lives in the RAM folder: a new
//! power cycle starts with none. A damaged file reads as an empty list,
//! which can only show a notice again.

use neptron_input::{DeviceIdentity, InputNotice, PadActivity, SilentPad, SilentPads};
use neptron_ui::snapshot::NoticeId;
use serde::{Deserialize, Serialize};

use crate::pages::fit_name;

/// The second button of the notice of a pad with the generic layout.
pub const SET_UP_CONTROLLER: &str = "Set up this controller";

/// The most characters of the name of a pad in a notice.
pub const NOTICE_NAME_MAX_CHARS: usize = 32;

/// The version of the file format. The format only grows.
const FORMAT: u32 = 1;

/// The key of a pad: its identity and the hash of its path.
pub type PadKey = (DeviceIdentity, DeviceIdentity);

/// What the player was told of a pad, or that it needs no silent check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NoteKind {
    /// The notice of the generic layout.
    Generic,
    /// The notice of a pad that is not read.
    Ignored,
    /// Heard, or told that it sends no input.
    Checked,
}

/// A notice of a pad for the player.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerNotice {
    /// The text.
    pub text: String,
    /// What it tells.
    pub kind: NoteKind,
    /// The pad.
    pub pad: PadKey,
}

/// The pad notices of a run.
#[derive(Debug, Clone, Default)]
pub struct PadNotes {
    told: Vec<(NoteKind, PadKey)>,
    silent: SilentPads,
    /// The notices in the snapshot that the player has not answered: not
    /// told yet. In a game a notice waits for the menu, and a run can end
    /// before the player sees it.
    shown: Vec<(NoticeId, NoteKind, PadKey)>,
    /// The told count at the last save.
    saved_told: usize,
    /// The checked count of the silent rule at the last save.
    saved_checked: usize,
}

#[derive(Serialize, Deserialize)]
struct File {
    format: u32,
    #[serde(default)]
    pads: Vec<Note>,
}

#[derive(Serialize, Deserialize)]
struct Note {
    kind: NoteKind,
    /// 16 hex digits: TOML has no unsigned 64-bit integer.
    identity: String,
    path: String,
}

impl PadNotes {
    /// The notes in the text of a pad notes file. A text that does not
    /// parse, or a later format, gives no note and the reason. A note with
    /// a key that is not hex is left out.
    pub fn parse(text: &str) -> (Self, Option<String>) {
        let file = match toml::from_str::<File>(text) {
            Ok(file) if file.format == FORMAT => file,
            Ok(file) => {
                let reason = format!(
                    "the pad notes file has format {}, not {FORMAT}; the pad notices can show again",
                    file.format
                );
                return (Self::default(), Some(reason));
            }
            Err(e) => {
                let reason = format!(
                    "the pad notes file does not parse ({e}); the pad notices can show again"
                );
                return (Self::default(), Some(reason));
            }
        };
        let hex = |text: &str| u64::from_str_radix(text, 16).ok().map(DeviceIdentity);
        let notes: Vec<(NoteKind, PadKey)> = file
            .pads
            .iter()
            .filter_map(|n| Some((n.kind, (hex(&n.identity)?, hex(&n.path)?))))
            .collect();
        let checked: Vec<PadKey> = notes
            .iter()
            .filter(|(kind, _)| *kind == NoteKind::Checked)
            .map(|(_, key)| *key)
            .collect();
        let told: Vec<(NoteKind, PadKey)> = notes
            .into_iter()
            .filter(|(kind, _)| *kind != NoteKind::Checked)
            .collect();
        let notes = Self {
            saved_told: told.len(),
            told,
            silent: SilentPads::new(&checked),
            ..Self::default()
        };
        (notes, None)
    }

    /// The notes of the file at `path`: none when the file is missing, and
    /// none with the reason when it cannot be read or parsed.
    pub fn load(path: &std::path::Path) -> (Self, Option<String>) {
        match std::fs::read_to_string(path) {
            Ok(text) => Self::parse(&text),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (Self::default(), None),
            Err(e) => (
                Self::default(),
                Some(format!("cannot read {}: {e}", path.display())),
            ),
        }
    }

    /// The text of the file: the pads told, and the pads checked.
    pub fn to_text(&self) -> String {
        let note = |kind: NoteKind, (identity, path): PadKey| Note {
            kind,
            identity: format!("{:016x}", identity.0),
            path: format!("{:016x}", path.0),
        };
        let checked = self.silent.checked();
        let pads = self
            .told
            .iter()
            .map(|(kind, key)| note(*kind, *key))
            .chain(checked.into_iter().map(|key| note(NoteKind::Checked, key)))
            .collect();
        let file = File {
            format: FORMAT,
            pads,
        };
        // A struct of a number and a list of strings always serializes.
        toml::to_string(&file).unwrap_or_default()
    }

    /// The notice for the player of an input notice, or `None`: for a
    /// notice that only the log needs, and for a pad that was told, or
    /// whose notice waits for an answer. The caller puts the notice in the
    /// snapshot and gives its id to [`PadNotes::show`].
    pub fn player_notice(&self, notice: &InputNotice) -> Option<PlayerNotice> {
        let (kind, pad, text) = match notice {
            InputNotice::GenericLayout { pad, .. } => (
                NoteKind::Generic,
                pad,
                format!(
                    "{} has no known layout, so it uses a generic one. Some buttons can be in other places.",
                    notice_name(&pad.name)
                ),
            ),
            InputNotice::NotAGamepad { pad, .. } => (
                NoteKind::Ignored,
                pad,
                format!(
                    "{} is not used: it does not look like a gamepad.",
                    notice_name(&pad.name)
                ),
            ),
            InputNotice::NoFreePort { pad } => (
                NoteKind::Ignored,
                pad,
                format!(
                    "{} is not used: {} controllers are in use.",
                    notice_name(&pad.name),
                    neptron_input::MAX_PORTS
                ),
            ),
            _ => return None,
        };
        let key = (pad.identity, pad.path);
        let waits = self.shown.iter().any(|(_, k, p)| (*k, *p) == (kind, key));
        if waits || self.told.contains(&(kind, key)) {
            return None;
        }
        Some(PlayerNotice {
            text,
            kind,
            pad: key,
        })
    }

    /// Keeps `notice`, which is in the snapshot as notice `id`, until the
    /// player answers it.
    pub fn show(&mut self, id: NoticeId, notice: &PlayerNotice) {
        self.shown.push((id, notice.kind, notice.pad));
    }

    /// Notice `id` left the snapshot with no answer, because the list was
    /// full: its pad is not told, and its next notice shows.
    pub fn dropped(&mut self, id: NoticeId) {
        self.shown.retain(|(n, _, _)| *n != id);
    }

    /// The player answered notice `id` (any button): its pad is told now.
    /// Gives what it told, or `None` for a notice of no pad.
    pub fn answered(&mut self, id: NoticeId) -> Option<(NoteKind, PadKey)> {
        let at = self.shown.iter().position(|(n, _, _)| *n == id)?;
        let (_, kind, pad) = self.shown.swap_remove(at);
        if !self.told.contains(&(kind, pad)) {
            self.told.push((kind, pad));
        }
        Some((kind, pad))
    }

    /// Follows the silent pad rule for one frame: see
    /// [`SilentPads::update`]. It allocates nothing.
    pub fn silent(
        &mut self,
        activity: &PadActivity,
        presses: u32,
        now_ns: u64,
        check: bool,
    ) -> Option<SilentPad> {
        self.silent.update(activity, presses, now_ns, check)
    }

    /// True when the file must be written again: a notice was answered,
    /// or a pad was heard or told by the silent rule, since the last call.
    /// It allocates nothing.
    pub fn take_changed(&mut self) -> bool {
        let checked = self.silent.checked_count();
        let told = self.told.len();
        let changed = checked > self.saved_checked || told > self.saved_told;
        self.saved_checked = checked;
        self.saved_told = told;
        changed
    }
}

/// The text of the notice of a pad that sends no input.
pub fn silent_text(name: &str) -> String {
    format!(
        "{} is connected but sends no input. Try another USB port or a USB hub.",
        notice_name(name)
    )
}

/// The name of a pad in a notice: at most [`NOTICE_NAME_MAX_CHARS`], with
/// `?` for a character the font does not have.
fn notice_name(name: &str) -> String {
    fit_name(name, NOTICE_NAME_MAX_CHARS)
}

#[cfg(test)]
mod tests {
    use neptron_input::{NotAGamepad, PadInfo, PadSignal};

    use super::*;

    fn pad(n: u64, name: &str) -> PadInfo {
        PadInfo {
            identity: DeviceIdentity(n),
            path: DeviceIdentity(100 + n),
            name: name.to_owned(),
        }
    }

    fn generic(n: u64) -> InputNotice {
        InputNotice::GenericLayout {
            device: 0x100,
            pad: pad(n, "USB Joystick"),
        }
    }

    /// Shows the player notice of `notice` as notice `id`, as the shell
    /// does, and gives its text.
    fn show(notes: &mut PadNotes, notice: &InputNotice, id: u32) -> Option<String> {
        let player = notes.player_notice(notice)?;
        notes.show(NoticeId(id), &player);
        Some(player.text)
    }

    #[test]
    fn each_pad_notice_comes_once_with_its_text() {
        let mut notes = PadNotes::default();
        assert_eq!(
            notes.player_notice(&generic(1)),
            Some(PlayerNotice {
                text: "USB Joystick has no known layout, so it uses a generic one. Some buttons can be in other places.".to_owned(),
                kind: NoteKind::Generic,
                pad: (DeviceIdentity(1), DeviceIdentity(101)),
            })
        );
        assert!(show(&mut notes, &generic(1), 1).is_some());
        assert_eq!(notes.player_notice(&generic(1)), None, "it waits");
        assert!(notes.player_notice(&generic(2)).is_some(), "another pad");
        let not_a_pad = InputNotice::NotAGamepad {
            pad: pad(3, "Mouse"),
            reason: NotAGamepad::NoDirection,
        };
        assert_eq!(
            show(&mut notes, &not_a_pad, 2),
            Some("Mouse is not used: it does not look like a gamepad.".to_owned())
        );
        assert_eq!(notes.player_notice(&not_a_pad), None);
        let ninth = InputNotice::NoFreePort {
            pad: pad(4, "Pad Nine"),
        };
        assert_eq!(
            notes.player_notice(&ninth).map(|n| n.text),
            Some("Pad Nine is not used: 8 controllers are in use.".to_owned())
        );
        let log_only = [
            InputNotice::Disconnected { device: 0x100 },
            InputNotice::MappingFile {
                path: "x".to_owned(),
                line: 2,
                reason: "y".to_owned(),
            },
        ];
        for notice in log_only {
            assert_eq!(notes.player_notice(&notice), None, "{notice}");
        }
    }

    #[test]
    fn a_dropped_notice_lets_its_pad_be_told_again_and_is_not_told() {
        let mut notes = PadNotes::default();
        assert!(show(&mut notes, &generic(1), 7).is_some());
        assert_eq!(notes.player_notice(&generic(1)), None, "it waits");
        notes.dropped(NoticeId(7));
        assert!(notes.player_notice(&generic(1)).is_some());
        assert!(!notes.take_changed(), "a dropped notice is not told");
        assert_eq!(notes.answered(NoticeId(7)), None);
    }

    #[test]
    fn a_notice_is_told_only_when_the_player_answers_it() {
        let mut notes = PadNotes::default();
        assert!(show(&mut notes, &generic(1), 7).is_some());
        assert!(!notes.take_changed(), "shown is not told");
        let (unseen, _) = PadNotes::parse(&notes.to_text());
        assert!(
            unseen.player_notice(&generic(1)).is_some(),
            "a run that ends now tells it again"
        );
        assert_eq!(notes.answered(NoticeId(6)), None, "another notice");
        assert_eq!(
            notes.answered(NoticeId(7)),
            Some((NoteKind::Generic, (DeviceIdentity(1), DeviceIdentity(101))))
        );
        assert_eq!(notes.answered(NoticeId(7)), None, "answered once");
        assert!(notes.take_changed());
        assert!(!notes.take_changed(), "no change since");
        let (seen, _) = PadNotes::parse(&notes.to_text());
        assert_eq!(seen.player_notice(&generic(1)), None);
        assert!(!{ seen }.take_changed(), "a loaded list is saved already");
    }

    #[test]
    fn a_long_name_is_cut_in_the_notice() {
        let long = "A Very Long Gamepad Name From A Vendor Database";
        let text = silent_text(long);
        assert_eq!(
            text,
            "A Very Long Gamepad Name From... is connected but sends no input. Try another USB port or a USB hub."
        );
    }

    #[test]
    fn the_notes_go_to_the_next_run_through_the_file() {
        let mut notes = PadNotes::default();
        assert!(show(&mut notes, &generic(1), 1).is_some());
        assert!(notes.answered(NoticeId(1)).is_some());
        assert!(notes.take_changed());
        let mut activity = PadActivity::default();
        activity.ports[0] = Some(PadSignal {
            plug: 1,
            identity: DeviceIdentity(7),
            path: DeviceIdentity(8),
            heard: true,
        });
        assert_eq!(notes.silent(&activity, 0, 0, true), None);
        assert!(notes.take_changed(), "a heard pad");
        let text = notes.to_text();
        let (mut next, issue) = PadNotes::parse(&text);
        assert_eq!(issue, None);
        assert_eq!(
            next.player_notice(&generic(1)),
            None,
            "told in the run before"
        );
        // The heard pad needs no check in the next run: never told.
        activity.ports[0] = Some(PadSignal {
            plug: 1,
            identity: DeviceIdentity(7),
            path: DeviceIdentity(8),
            heard: false,
        });
        let after = neptron_input::SILENT_PAD_AFTER_NS;
        assert_eq!(next.silent(&activity, 0, 0, true), None);
        assert_eq!(next.silent(&activity, 100, after, true), None);
    }

    #[test]
    fn a_bad_file_shows_the_notices_again() {
        for text in ["", "format = 2\npads = []\n", "pads = 3", "\u{0}"] {
            let (notes, issue) = PadNotes::parse(text);
            assert!(issue.is_some(), "{text:?}");
            assert!(notes.player_notice(&generic(1)).is_some());
        }
        let bad_key = "format = 1\n[[pads]]\nkind = \"generic\"\nidentity = \"zz\"\npath = \"0\"\n";
        let (notes, issue) = PadNotes::parse(bad_key);
        assert_eq!(issue, None);
        assert!(notes.player_notice(&generic(1)).is_some());
        let path = std::env::temp_dir().join(format!(
            "neptron-pad-notes-missing-{}.toml",
            std::process::id()
        ));
        assert_eq!(PadNotes::load(&path).1, None);
    }
}
