//! The picture pair: the picture mode and the refresh rate. The app writes
//! both keys at one level in one write, or removes both, so the pair that
//! a game resolves to comes whole from one level, the pair that was judged
//! there. The keep prompt of the quick menu moves the pair of the game.

use neptron_backend::{AudioSink, Clock, Display, InputSource};
use neptron_input::PadRoster;
use neptron_settings::{DISPLAY_STYLE, Level, Value};
use neptron_ui::Intent;
use neptron_ui::snapshot::{Level as UiLevel, NoticeKind, SettingKey};
use neptron_video::{AnswerRequest, Phase, REFERENCE_SOURCES};

use super::{PendingChange, Shell, Stage};
use crate::config::PicturePair;
use crate::pages::{self, Gate};

/// The start of the line in the log of each write of the picture pair: one
/// write at one level.
pub const PAIR_WRITE_LOG: &str = "picture pair written at";

/// The keep of the batch of intents in work.
#[derive(Debug, Default)]
pub(super) struct KeepBatch {
    /// The keys that the keep prompt moves in this batch, with the level
    /// they move to: a set at that level that a reset of the same key at
    /// the game level follows in the batch.
    keys: Vec<(SettingKey, UiLevel)>,
    /// The picture pair was judged for its move in this batch.
    judged: bool,
    /// The level refused the move: the pair stays at the game level.
    refused: bool,
}

impl KeepBatch {
    /// Marks the keeps of `intents`. The keep prompt sends the set at the
    /// new level before the reset at the game level, so a refused set can
    /// stop the reset.
    pub(super) fn start(&mut self, intents: &[Intent]) {
        self.keys.clear();
        self.judged = false;
        self.refused = false;
        for (i, intent) in intents.iter().enumerate() {
            let Intent::SetValue { key, level, .. } = *intent else {
                continue;
            };
            if level == UiLevel::Game {
                continue;
            }
            let reset = intents.iter().skip(i + 1).any(|later| {
                *later
                    == Intent::ResetValue {
                        key,
                        level: UiLevel::Game,
                    }
            });
            if reset {
                self.keys.push((key, level));
            }
        }
    }

    /// Whether the set of `key` at `level` is a keep of this batch.
    pub(super) fn moves(&self, key: SettingKey, level: UiLevel) -> bool {
        self.keys.contains(&(key, level))
    }

    /// Whether the reset of `key` at the game level belongs to a keep that
    /// the new level refused.
    pub(super) fn stays(&self, key: SettingKey) -> bool {
        self.refused && self.keys.iter().any(|(k, _)| *k == key)
    }
}

impl<D: Display, S: AudioSink, I: InputSource + PadRoster, C: Clock + Clone> Shell<D, S, I, C> {
    /// A pick of `value` for `key` (the picture mode or the refresh rate)
    /// at `level`, by `gate`: the pair of the pick and the value of the
    /// other row that the pick was judged with (the value stored up to
    /// `level`, or Auto when the gate says so) goes to `level` in one
    /// write. A pick of the pair that the level holds writes nothing.
    pub(super) fn store_pick(
        &mut self,
        key: &'static str,
        level: Level,
        value: Value,
        gate: Gate,
        now_ns: u64,
    ) {
        let Some((other, auto)) = pages::other_row(key) else {
            return;
        };
        let (other_value, note, reset) = match gate {
            Gate::Set { note } => (
                self.config.resolved_up_to(level, other).map(|r| r.value),
                note,
                false,
            ),
            Gate::SetWithReset { note, .. } => (Some(Value::Text(auto.to_owned())), note, true),
            Gate::Refused(reason) => {
                self.refuse_value(key, &value, &reason);
                return;
            }
        };
        let Some(other_value) = other_value else {
            return;
        };
        let pair = pages::pair_of(key, value, other_value);
        if self.config.holds_pair(level, &pair) || !self.write_pair(level, Some(pair), now_ns) {
            return;
        }
        if reset {
            self.notice(NoticeKind::Info, pages::reset_notice(other, level));
        }
        if let Some(note) = note {
            self.notice(NoticeKind::Info, note);
        }
        self.note_own_settings_above(level);
    }

    /// The Menu button on the picture mode or the refresh row at `level`:
    /// both keys go, in one write. The pair that then resolves comes from
    /// a level below, the player's stored choice; when the display does
    /// not give it, the notice of a stored choice says what shows.
    pub(super) fn reset_pair(&mut self, level: Level, now_ns: u64) {
        if self.config.holds_picture(level) {
            self.write_pair(level, None, now_ns);
        }
    }

    /// The keep prompt moves the pair of the game to `level`: one write of
    /// the pair there, when the answers of that level give it, then the
    /// reset of the game level in the same batch removes both keys there.
    /// A move to every game while the system level holds its own pair is
    /// refused: the player asked for this game to keep its picture. A
    /// refused move leaves the pair at the game level, and says why.
    pub(super) fn keep_pair(&mut self, level: Level, now_ns: u64) {
        if std::mem::replace(&mut self.keep_batch.judged, true) {
            return;
        }
        let Some(pair) = self.config.resolved_pair() else {
            return;
        };
        let judged = if level == Level::Global && self.config.holds_picture(Level::System) {
            Err(pages::SYSTEM_OWN_PICTURE.to_owned())
        } else {
            self.keep_judged(level, &pair)
        };
        match judged {
            Ok(note) => {
                if !self.config.holds_pair(level, &pair) {
                    self.write_pair(level, Some(pair), now_ns);
                }
                if let Some(note) = note {
                    self.notice(NoticeKind::Info, note);
                }
            }
            Err(reason) => {
                self.keep_batch.refused = true;
                self.log(format!(
                    "the picture pair is not kept at {}: {reason}",
                    level.name()
                ));
                self.notice(NoticeKind::Info, pages::not_kept_notice(level, &reason));
            }
        }
    }

    /// What the answers of the keep request of `level` say of `pair`: the
    /// option of its picture mode, judged with the pair stored, gives the
    /// note for some games, or the reason of the refusal. Only settled
    /// answers count; the prompt does not take a level whose answers are
    /// in work.
    fn keep_judged(&self, level: Level, pair: &PicturePair) -> Result<Option<String>, String> {
        let request = self
            .keep_asks
            .iter()
            .find(|(l, _)| *l == level)
            .map(|(_, r)| r);
        let answers = request.and_then(|r| self.engine_ref()?.answers(r));
        match answers {
            Some(Ok(choices)) => {
                pages::keep_answer(&pair.style.to_string(), &choices, self.scope_of(level))
            }
            Some(Err(_)) | None => Err(pages::NO_DECISION.to_owned()),
        }
    }

    /// The requests of the keep prompt: for each level that the prompt can
    /// move the pair of the game to, the answers with the values up to
    /// that level after the move. Empty with no game, or when the game
    /// level holds no picture key. No request for every game while the
    /// system level holds its own pair: that move is refused anyway.
    pub(super) fn keep_requests(&self) -> Vec<(Level, AnswerRequest)> {
        let Stage::Game(game) = &self.stage else {
            return Vec::new();
        };
        if !self.config.holds_picture(Level::Game) {
            return Vec::new();
        }
        let Some(pair) = self.config.resolved_pair() else {
            return Vec::new();
        };
        let source = game.session.source();
        let system_own = self.config.holds_picture(Level::System);
        [Level::System, Level::Global]
            .into_iter()
            .filter(|level| *level == Level::System || !system_own)
            .filter_map(|level| {
                let mut after = self.config.clone();
                after.write_pair(level, Some(pair.clone())).ok()?;
                let sources = match level {
                    Level::Global => std::iter::once(source).chain(REFERENCE_SOURCES).collect(),
                    Level::System | Level::Game => vec![source],
                };
                let request = AnswerRequest {
                    sources,
                    settings: after.policy_up_to(level).ok()?,
                    scanlines: after.scanlines_up_to(level).ok()?,
                };
                Some((level, request))
            })
            .collect()
    }

    /// Writes `pair` at `level` in memory, or removes both keys there with
    /// `None`, then in one write to the file, or under the prompt of the
    /// guard while it waits. Returns whether a key changed.
    fn write_pair(&mut self, level: Level, pair: Option<PicturePair>, now_ns: u64) -> bool {
        if self.config.file(level).is_none() {
            return false;
        }
        let changes = match self.config.write_pair(level, pair) {
            Ok(changes) => changes,
            Err(e) => {
                self.notice(
                    NoticeKind::Error,
                    format!("The setting was not changed: {e}."),
                );
                return false;
            }
        };
        if changes.is_empty() {
            return false;
        }
        let keys: Vec<&str> = changes.iter().map(|c| c.key).collect();
        self.log(format!(
            "{PAIR_WRITE_LOG} {}: {}",
            level.name(),
            keys.join(", ")
        ));
        self.note_picture_change(DISPLAY_STYLE);
        if self.guard.phase() == Phase::Confirmed {
            self.write_settings(level, changes.into_iter().map(|c| (c.key, c.new)).collect());
            self.follow(now_ns);
        } else {
            self.pending
                .extend(changes.into_iter().map(|c| PendingChange {
                    level,
                    key: c.key,
                    old: c.old,
                    new: c.new,
                }));
            self.propose(now_ns);
        }
        self.reload_page();
        true
    }

    /// After a write at `level` in a game: a level above it for this game
    /// that holds its own pair keeps it, and the notice says so at once.
    fn note_own_settings_above(&mut self, level: Level) {
        if self.config.context().is_none() {
            return;
        }
        let game = level != Level::Game && self.config.holds_picture(Level::Game);
        let system = level == Level::Global && self.config.holds_picture(Level::System);
        let text = if game {
            pages::GAME_KEEPS_OWN_PICTURE
        } else if system {
            pages::SYSTEM_KEEPS_OWN_PICTURE
        } else {
            return;
        };
        self.notice(NoticeKind::Info, text.to_owned());
    }
}

#[cfg(test)]
mod tests {
    use neptron_ui::Value as UiValue;

    use super::*;

    const STYLE: SettingKey = SettingKey(1);
    const VOLUME: SettingKey = SettingKey(2);

    fn set(key: SettingKey, level: UiLevel) -> Intent {
        Intent::SetValue {
            key,
            level,
            value: UiValue::Choice(1),
        }
    }

    fn reset(key: SettingKey, level: UiLevel) -> Intent {
        Intent::ResetValue { key, level }
    }

    /// The keep prompt sends the set at the new level, then the reset at
    /// the game level. Only that order is a keep: a reset first, or a set
    /// with no reset after it, moves nothing.
    #[test]
    fn a_keep_is_a_set_that_a_reset_at_the_game_level_follows() {
        let mut batch = KeepBatch::default();
        batch.start(&[
            set(STYLE, UiLevel::System),
            reset(STYLE, UiLevel::Game),
            reset(VOLUME, UiLevel::Game),
            set(VOLUME, UiLevel::Global),
        ]);
        assert!(batch.moves(STYLE, UiLevel::System));
        assert!(!batch.moves(STYLE, UiLevel::Global));
        assert!(
            !batch.moves(VOLUME, UiLevel::Global),
            "the reset came first"
        );
        batch.start(&[set(STYLE, UiLevel::Global)]);
        assert!(!batch.moves(STYLE, UiLevel::Global), "no reset after it");
    }

    /// A refused move keeps the reset of its key at the game level from
    /// running, in this batch only.
    #[test]
    fn a_refused_keep_stops_the_reset_of_its_batch() {
        let mut batch = KeepBatch::default();
        let keep = [set(STYLE, UiLevel::Global), reset(STYLE, UiLevel::Game)];
        batch.start(&keep);
        assert!(!batch.stays(STYLE));
        batch.judged = true;
        batch.refused = true;
        assert!(batch.stays(STYLE));
        assert!(!batch.stays(VOLUME));
        batch.start(&keep);
        assert!(!batch.stays(STYLE), "a new batch starts clean");
    }
}
